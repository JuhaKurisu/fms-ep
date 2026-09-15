// gpubridge サンプル 2: boids（Slang・storage buffer・uniform・3 パス構成）
import gpubridge.*;

// Processing 自身のウィンドウは出さない
public void showSurface() { }

final int N = 8192;

GpuWindow gpu;
GpuUniform params;
GpuBuffer a, b;
GpuTexture canvas;
GpuBinding simAB, simBA, drawA, drawB, clearPass;
boolean ping = false;

void setup() {
  size(200, 200);
  gpu = new GpuWindow(1600, 1600, "boids");

  params = gpu.uniform();     // レイアウトは bind 時に Slang の struct から確定
  a = gpu.buffer(N);          // ストライドも bind 時に確定
  b = gpu.buffer(N);
  canvas = gpu.texture(1600, 1600);

  GpuKernel sim   = gpu.kernelSlang(loadStrings("boids.slang"),  "update");
  GpuKernel clr   = gpu.kernelSlang(loadStrings("render.slang"), "clearPass");
  GpuKernel splat = gpu.kernelSlang(loadStrings("render.slang"), "splat");

  simAB = sim.binding().set("params", params).set("src", a).set("dst", b);
  simBA = sim.binding().set("params", params).set("src", b).set("dst", a);
  clearPass = clr.binding().set("canvas", canvas);
  drawA = splat.binding().set("boids", a).set("canvas", canvas);
  drawB = splat.binding().set("boids", b).set("canvas", canvas);

  params.set("dt", 1.0/60.0).set("ali", 1.0).set("coh", 1.0);

  // 初期配置: ランダムな位置と速度
  float[] seed = new float[N * 4];
  for (int i = 0; i < N; i++) {
    seed[i*4] = random(1600); seed[i*4+1] = random(1600);
    seed[i*4+2] = random(-1, 1); seed[i*4+3] = random(-1, 1);
  }
  a.write(seed);
}

void draw() {
  // GPU 窓上のマウス X で分離の強さをいじる
  params.set("sep", map(gpu.mouseX(), 0, 1600, 0, 2));
  clearPass.dispatch(1600, 1600);
  (ping ? simBA : simAB).dispatch(N);
  (ping ? drawB : drawA).dispatch(N);
  gpu.show(canvas);
  gpu.submit();
  ping = !ping;

  if (!gpu.isOpen()) {
    gpu.dispose();
    exit();
  }
}
