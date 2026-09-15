#!/usr/bin/env bash
# imguibridge をビルドして、指定したスケッチの code/ に配置する。
#
#   ./build.sh                  → ../（ep_main）に配置
#   ./build.sh ../sketch_3 ...  → 指定したスケッチすべてに配置
#
# imgui-java の jar は初回に Maven Central から libs/ へ取得する。
set -euo pipefail
cd "$(dirname "$0")"

IMGUI_VER="1.92.7.1"

# --- OS ごとの natives ---
case "$(uname -s)" in
  Darwin) NATIVES_OS="macos" ;;
  MINGW*|MSYS*|CYGWIN*) NATIVES_OS="windows" ;;
  *) echo "未対応の OS: $(uname -s)" >&2; exit 1 ;;
esac

BINDING="libs/imgui-java-binding-$IMGUI_VER.jar"
NATIVES="libs/imgui-java-natives-$NATIVES_OS-$IMGUI_VER.jar"
GPUBRIDGE="../gpubridge/gpubridge.jar"

# --- JDK を探す（gpubridge/build.sh と同じ） ---
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
[ -f "$GPUBRIDGE" ] || { echo "先に gpubridge をビルドしてください: $GPUBRIDGE" >&2; exit 1; }

echo "==> imgui-java の jar を確認"
mkdir -p libs
for j in "imgui-java-binding" "imgui-java-natives-$NATIVES_OS"; do
  f="libs/$j-$IMGUI_VER.jar"
  if [ ! -f "$f" ]; then
    echo "    取得: $j-$IMGUI_VER"
    curl -sfL -o "$f" \
      "https://repo1.maven.org/maven2/io/github/spair/$j/$IMGUI_VER/$j-$IMGUI_VER.jar"
  fi
done

echo "==> Java をコンパイル"
rm -rf classes && mkdir -p classes
"$JAVAC" -encoding UTF-8 -cp "$GPUBRIDGE:$BINDING" -d classes java/imguibridge/*.java

echo "==> jar を作成"
"$JAR" cf imguibridge.jar -C classes .

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
  # 上書き cp は避ける(gpubridge/build.sh の署名キャッシュ注意に倣う)
  rm -f "$sketch/code/imguibridge.jar" \
        "$sketch/code/imgui-java-binding-"*.jar "$sketch/code/imgui-java-natives-"*.jar
  cp imguibridge.jar "$BINDING" "$NATIVES" "$sketch/code/"
  echo "==> 配置: $sketch/code/"
done

echo "完了"
