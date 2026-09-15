// gpubridge サンプル 1: ライフゲーム（WGSL・texture ping-pong の最小構成）
import gpubridge.*;

// Processing 自身のウィンドウは出さない
public void showSurface() { }

GpuWindow gpu;
GpuTexture a, b;
GpuBinding ab, ba;
boolean ping = false;

void setup() {
  size(200, 200);
  gpu = new GpuWindow(1024, 1024, "life");
  a = gpu.texture(1024, 1024);
  b = gpu.texture(1024, 1024);

  GpuKernel step = gpu.kernel(loadStrings("life.wgsl"), "step");
  ab = step.binding().set("src", a).set("dst", b);
  ba = step.binding().set("src", b).set("dst", a);

  // 初期状態: 30% の確率でセルを立てる
  int[] seed = new int[1024 * 1024];
  for (int i = 0; i < seed.length; i++) seed[i] = random(1) < 0.3 ? 0xFFFFFFFF : 0xFF000000;
  a.write(seed);
}

void draw() {
  (ping ? ba : ab).dispatch(1024, 1024);   // スレッド数指定（workgroup 割りは自動）
  gpu.show(ping ? a : b);
  gpu.submit();
  ping = !ping;

  if (!gpu.isOpen()) {
    gpu.dispose();
    exit();
  }
}
