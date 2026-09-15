package gpubridge;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.Map;
import java.util.TreeMap;

/**
 * slangc を起動して Slang ソースを WGSL に変換する。
 *
 * <p>slangc の場所は次の順で探す:
 *
 * <ol>
 *   <li>システムプロパティ {@code gpubridge.slangc}
 *   <li>PATH 上の {@code slangc}
 *   <li>{@code ~/bin/slang/bin/slangc}
 * </ol>
 */
final class SlangCompiler {

  /** 同一ソースを何度もコンパイルしないためのキャッシュ（source + 依存ファイル → wgsl）。 */
  private static final Map<String, String> CACHE = new HashMap<>();

  private SlangCompiler() {}

  static String toWgsl(String slangSource) {
    return toWgsl(slangSource, null);
  }

  /**
   * @param searchDir import 解決に使うディレクトリ（null 可）。以下にある {@code *.slang} を
   *     階層ごとコンパイル用の一時ディレクトリへコピーするので、ソース内の {@code import "foo";} や
   *     {@code import "sub/foo";} が {@code searchDir} からの相対パスで参照できる。
   */
  static synchronized String toWgsl(String slangSource, Path searchDir) {
    // 依存ファイルの内容もキーに含める。import 先だけ書き換えた場合に古い結果を返さないため。
    Map<String, String> deps = readDeps(searchDir);
    StringBuilder key = new StringBuilder(slangSource);
    deps.forEach((name, body) -> key.append('\0').append(name).append('\0').append(body));
    String cached = CACHE.get(key.toString());
    if (cached != null) {
      return cached;
    }
    String wgsl = compile(slangSource, deps);
    CACHE.put(key.toString(), wgsl);
    return wgsl;
  }

  /**
   * searchDir 以下の {@code *.slang} を再帰的に読む（searchDir が null や存在しない場合は空）。
   * キーは searchDir からの相対パスで、一時ディレクトリにも同じ階層で書き出される。
   */
  private static Map<String, String> readDeps(Path searchDir) {
    Map<String, String> deps = new TreeMap<>();
    if (searchDir == null || !Files.isDirectory(searchDir)) {
      return deps;
    }
    try (var files = Files.walk(searchDir)) {
      for (Path f : files.sorted().toList()) {
        if (Files.isRegularFile(f) && f.getFileName().toString().endsWith(".slang")) {
          deps.put(searchDir.relativize(f).toString(), Files.readString(f, StandardCharsets.UTF_8));
        }
      }
    } catch (IOException e) {
      throw new IllegalStateException(
          "Slang の検索ディレクトリを読めません: " + searchDir + "（" + e.getMessage() + "）", e);
    }
    return deps;
  }

  private static String compile(String slangSource, Map<String, String> deps) {
    Path dir = null;
    try {
      dir = Files.createTempDirectory("gpubridge-slang");
      Path in = dir.resolve("shader.slang");
      Path out = dir.resolve("shader.wgsl");
      for (Map.Entry<String, String> dep : deps.entrySet()) {
        Path target = dir.resolve(dep.getKey());
        Files.createDirectories(target.getParent());
        Files.writeString(target, dep.getValue(), StandardCharsets.UTF_8);
      }
      // エントリを最後に書く。依存側に shader.slang があってもこちらを優先する
      Files.writeString(in, slangSource, StandardCharsets.UTF_8);

      // -preserve-params: 未使用のグローバルリソースもデッドコード除去せず WGSL に残す。
      // これがないと「宣言したが未実装」のリソースがリフレクションから消えて set() できない。
      Process p =
          new ProcessBuilder(
                  slangcPath(),
                  in.toString(),
                  "-target",
                  "wgsl",
                  "-preserve-params",
                  "-o",
                  out.toString())
              .redirectErrorStream(true)
              .start();
      String log = new String(p.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
      int code = p.waitFor();
      if (code != 0 || !Files.exists(out)) {
        throw new IllegalArgumentException("Slang のコンパイルに失敗しました:\n" + log);
      }
      return Files.readString(out, StandardCharsets.UTF_8);
    } catch (IOException e) {
      throw new IllegalStateException(
          "slangc を起動できません（"
              + e.getMessage()
              + "）。PATH に slangc を通すか、-Dgpubridge.slangc=/path/to/slangc を指定してください",
          e);
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
      throw new IllegalStateException("slangc の実行が中断されました", e);
    } finally {
      if (dir != null) {
        try (var files = Files.walk(dir)) {
          files
              .sorted((a, b) -> b.getNameCount() - a.getNameCount())
              .forEach(
                  f -> {
                    try {
                      Files.deleteIfExists(f);
                    } catch (IOException ignored) {
                      // 一時ファイルの掃除失敗は無視
                    }
                  });
        } catch (IOException ignored) {
          // 同上
        }
      }
    }
  }

  private static String slangcPath() {
    String explicit = System.getProperty("gpubridge.slangc");
    if (explicit != null) {
      return explicit;
    }
    // PATH で見つかるならそのまま。GUI アプリ（Processing IDE）はシェルの PATH を
    // 引き継がないことがあるので、よくある置き場所も候補にする。
    Path home = Path.of(System.getProperty("user.home"), "bin", "slang", "bin", "slangc");
    if (Files.isExecutable(home)) {
      return home.toString();
    }
    return "slangc";
  }
}
