// gpubridge サンプル: boids を render pipeline で描く
// （シミュレーションは compute、描画は vertex/fragment + ADD ブレンド）
import gpubridge.*;

// Processing 自身のウィンドウは出さない
public void showSurface() { }

final int N = 8192;

GpuWindow gpu;
GpuUniform params;
GpuBuffer a, b;
GpuTexture canvas;
GpuBinding simAB, simBA;
GpuRenderBinding drawA, drawB;
boolean ping = false;

void setup() {
  size(200, 200);
  gpu = new GpuWindow(1600, 1600, "boids (render pipeline)");
  gpu.slangSearchPath(dataPath(""));

  params = gpu.uniform();
  a = gpu.buffer(N);
  b = gpu.buffer(N);
  canvas = gpu.texture(1600, 1600);

  GpuKernel sim = gpu.kernelSlang(loadStrings("boids.slang"), "update");
  GpuRenderer splat = gpu.rendererSlang(loadStrings("splat.slang"), "vsMain", "fsMain");
  splat.blend(GpuBlend.ADD);

  simAB = sim.binding().set("params", params).set("src", a).set("dst", b);
  simBA = sim.binding().set("params", params).set("src", b).set("dst", a);
  drawA = splat.binding().set("boids", a);
  drawB = splat.binding().set("boids", b);

  params.set("dt", 1.0/60.0).set("ali", 1.0).set("coh", 1.0);

  float[] seed = new float[N * 4];
  for (int i = 0; i < N; i++) {
    seed[i*4] = random(1600); seed[i*4+1] = random(1600);
    seed[i*4+2] = random(-1, 1); seed[i*4+3] = random(-1, 1);
  }
  a.write(seed);
}

void draw() {
  params.set("sep", map(gpu.mouseX(), 0, 1600, 0, 2));
  (ping ? simBA : simAB).dispatch(N);
  gpu.clear(canvas, 0, 0, 0, 1);
  (ping ? drawA : drawB).draw(canvas, 6, N);
  gpu.show(canvas);
  gpu.submit();
  ping = !ping;

  if (!gpu.isOpen()) {
    gpu.dispose();
    exit();
  }
}
