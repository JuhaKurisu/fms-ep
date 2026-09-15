// gpubridge サンプル: GPU カリング + 間接描画
// copyBufferToBuffer / dispatchIndirect / drawIndexedIndirect の 3 つを使う。
// マウス周りの粒子だけを GPU が数えて詰め、その個数のまま描く —
// 「何個描くか」を CPU が一度も知らないまま 1 フレームが完結する。
import gpubridge.*;

// Processing 自身のウィンドウは出さない
public void showSurface() { }

final int N = 30000;
final int W = 1200, H = 1200;

GpuWindow gpu;
GpuUniform simParams, drawParams;
GpuBuffer particles, visible, offsets, drawArgs, resetArgs, dispatchArgs, quadIndices;
GpuTexture canvas;
GpuBinding cull, prepare, wobble;
GpuRenderBinding quads;

void setup() {
  size(200, 200);
  gpu = new GpuWindow(W, H, "indirect cull");

  simParams = gpu.uniform();
  drawParams = gpu.uniform();
  particles = gpu.buffer(N, 8);     // vec2<f32> の固定位置
  visible = gpu.buffer(N, 4);       // 生き残った粒子の添字（cull が詰める）
  offsets = gpu.buffer(N, 8);       // 生き残った分だけ wobble が書く
  drawArgs = gpu.buffer(5, 4);      // {indexCount, instanceCount, firstIndex, baseVertex, firstInstance}
  resetArgs = gpu.buffer(5, 4);     // ↑を毎フレーム初期化するテンプレート
  dispatchArgs = gpu.buffer(3, 4);  // {x, y, z} ワークグループ数
  quadIndices = gpu.buffer(6, 4);
  canvas = gpu.texture(W, H);

  String[] sim = loadStrings("cull.wgsl");
  cull = gpu.kernel(sim, "cull").binding()
      .set("params", simParams)
      .set("particles", particles)
      .set("visible", visible)
      .set("drawArgs", drawArgs);
  prepare = gpu.kernel(sim, "prepare").binding()
      .set("drawArgs", drawArgs)
      .set("dispatchArgs", dispatchArgs);
  wobble = gpu.kernel(sim, "wobble").binding()
      .set("params", simParams)
      .set("drawArgs", drawArgs)
      .set("offsets", offsets);

  quads = gpu.renderer(loadStrings("draw.wgsl"), "vs", "fs").binding()
      .set("rparams", drawParams)
      .set("particles", particles)
      .set("visible", visible)
      .set("offsets", offsets);

  float[] seed = new float[N * 2];
  for (int i = 0; i < N; i++) {
    seed[i * 2] = random(W);
    seed[i * 2 + 1] = random(H);
  }
  particles.write(seed);

  quadIndices.write(new int[] {0, 1, 2, 2, 1, 3});
  // indexCount=6（quad）。instanceCount は cull が 0 から数え直す
  resetArgs.write(new int[] {6, 0, 0, 0, 0});
  drawParams.set("resolution", W, H).set("size", 4.0);
}

void draw() {
  simParams.set("mouse", gpu.mouseX(), gpu.mouseY())
      .set("radius", 250.0)
      .set("time", frameCount / 60.0);

  gpu.copyBufferToBuffer(resetArgs, 0, drawArgs, 0, 20); // instanceCount を 0 に戻す
  cull.dispatch(N);                       // 見える粒子を数えて詰める
  prepare.dispatch(1);                    // 個数 → wobble のワークグループ数
  wobble.dispatchIndirect(dispatchArgs);  // 見えている分だけ実行
  gpu.clear(canvas, 0.03, 0.04, 0.08, 1);
  quads.drawIndexedIndirect(canvas, quadIndices, drawArgs); // 個数は GPU が書いた値のまま
  gpu.show(canvas);
  gpu.submit();

  if (!gpu.isOpen()) {
    gpu.dispose();
    exit();
  }
}
