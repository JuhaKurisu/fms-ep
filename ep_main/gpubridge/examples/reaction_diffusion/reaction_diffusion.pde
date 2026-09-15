// gpubridge サンプル 3: Gray-Scott 反応拡散
// （Slang・RGBA32F・フレーム内反復・マウス入力）
// マウスを窓の上で動かすと、カーソルから紋様が育つ。
import gpubridge.*;

// Processing 自身のウィンドウは出さない。描画は GpuWindow（wgpu compute shader）側。
public void showSurface() { }

final int W = 1024, H = 1024;
final int STEPS = 8;                     // 1 フレームに進めるシミュレーションステップ数

GpuWindow gpu;
GpuUniform params;
GpuTexture fa, fb, view;
GpuBinding evolveAB, evolveBA, showA, showB;
boolean ping = false;                    // false のとき fa が最新

void setup() {
  size(200, 200);
  gpu = new GpuWindow(W, H, "gray-scott");
  println("backend = " + gpu.backend());

  params = gpu.uniform();
  fa = gpu.texture(W, H, GpuFormat.RGBA32F);
  fb = gpu.texture(W, H, GpuFormat.RGBA32F);
  view = gpu.texture(W, H);              // 表示用（RGBA8）

  GpuKernel evolve = gpu.kernelSlang(loadStrings("rd.slang"), "evolve");
  GpuKernel show   = gpu.kernelSlang(loadStrings("rd.slang"), "show");

  evolveAB = evolve.binding().set("params", params).set("src", fa).set("dst", fb);
  evolveBA = evolve.binding().set("params", params).set("src", fb).set("dst", fa);
  showA = show.binding().set("src", fa).set("view", view);
  showB = show.binding().set("src", fb).set("view", view);

  params.set("feed", 0.055).set("kill", 0.062);   // coral growth (Karl Sims)

  // 初期状態: u=1, v=0（触媒はマウスで注入する）
  float[] init = new float[W * H * 4];
  for (int i = 0; i < W * H; i++) init[i * 4] = 1.0;
  fa.write(init);
}

void draw() {
  params.set("seed", gpu.mouseX(), gpu.mouseY());
  for (int i = 0; i < STEPS; i++) {
    (ping ? evolveBA : evolveAB).dispatch(W, H);
    ping = !ping;
  }
  (ping ? showB : showA).dispatch(W, H);   // 直近に書かれた側を表示
  gpu.show(view);
  gpu.submit();

  if (!gpu.isOpen()) {
    gpu.dispose();
    exit();
  }
}
