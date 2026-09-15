#!/usr/bin/env bash
# gpubridge をビルドして、指定したスケッチの code/ に配置する。
#
#   ./build.sh                  → ../ep_main に配置
#   ./build.sh ../sketch_3 ...  → 指定したスケッチすべてに配置
set -euo pipefail
cd "$(dirname "$0")"

# --- JDK を探す（Processing 同梱のものを優先。JAVA_HOME があればそれを使う） ---
if [ -n "${JAVA_HOME:-}" ]; then
  JDK="$JAVA_HOME"
elif [ -d "/Applications/Processing.app/Contents/app/resources/jdk" ]; then
  JDK="/Applications/Processing.app/Contents/app/resources/jdk"
else
  JDK="$(dirname "$(dirname "$(readlink -f "$(command -v javac)")")")"
fi
JAVAC="$JDK/bin/javac"
JAR="$JDK/bin/jar"
[ -x "$JAVAC" ] || { echo "javac が見つかりません: $JAVAC" >&2; exit 1; }

# --- OS ごとのライブラリ名 ---
case "$(uname -s)" in
  Darwin) LIB_SRC="libgpubridge.dylib" ;;
  Linux)  LIB_SRC="libgpubridge.so" ;;
  MINGW*|MSYS*|CYGWIN*) LIB_SRC="gpubridge.dll" ;;
  *) echo "未対応の OS: $(uname -s)" >&2; exit 1 ;;
esac

echo "==> Rust (wgpu + JNI) をビルド"
cargo build --release

echo "==> Java をコンパイル"
rm -rf classes && mkdir -p classes
"$JAVAC" -encoding UTF-8 -d classes java/gpubridge/*.java

echo "==> jar を作成"
"$JAR" cf gpubridge.jar -C classes .

# --- 配布先 ---
TARGETS=("$@")
if [ ${#TARGETS[@]} -eq 0 ]; then
  TARGETS=("..")
fi

for sketch in "${TARGETS[@]}"; do
  if [ ! -d "$sketch" ]; then
    echo "スケッチフォルダがありません: $sketch" >&2
    exit 1
  fi
  mkdir -p "$sketch/code"
  # 既存ファイルへの上書き cp はしない(macOS はカーネルの署名キャッシュと
  # 食い違い、dlopen 時に Code Signature Invalid で SIGKILL される)。
  rm -f "$sketch/code/gpubridge.jar" "$sketch/code/$LIB_SRC"
  cp gpubridge.jar "$sketch/code/"
  cp "target/release/$LIB_SRC" "$sketch/code/"
  echo "==> 配置: $sketch/code/ ($LIB_SRC, gpubridge.jar)"
done

echo "完了"
