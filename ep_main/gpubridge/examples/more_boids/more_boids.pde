// gpubridge サンプル 4: 空間分割による大規模 boids（1,000,000 体）
//
// O(N^2) の boids は 4096 体が限度だが、uniform grid（256x256 セル）+
// セルへの実体コピー（走査を連続メモリ読みにする）で 100 万体が 60fps で動く。
// 世界 = ウィンドウ = テクスチャ = 2048x2048 の同一座標系（1:1 表示）。
// 1 フレーム = clearGrid → insert → update → clearCanvas → splat の 5 パス。
//
// マウス: 非クリック時は外へ押し出しながら回転、クリックすると逆回転で内へ巻き込む。
import gpubridge.*;

// Processing 自身のウィンドウは出さない
public void showSurface() { }

final int N = 1000000;
final int SIZE = 2048;              // ウィンドウ。世界はこの 2 倍（2048px）
final int CELLS = 256 * 256;        // Slang 側の GRID と一致させる
final int CAP = 32;                 // Slang 側の CAP と一致させる

GpuWindow gpu;
GpuUniform params;
GpuBuffer a, b, gridCount, gridBoids;
GpuTexture canvas;
GpuBinding clearGridPass, insertA, insertB, simAB, simBA, clearCanvasPass, splatA, splatB;
boolean ping = false;

void setup() {
  size(200, 200);
  gpu = new GpuWindow(SIZE, SIZE, "more boids (1M)");
  println("backend = " + gpu.backend());

  params = gpu.uniform();
  a = gpu.buffer(N);
  b = gpu.buffer(N);
  gridCount = gpu.buffer(CELLS);
  gridBoids = gpu.buffer(CELLS * CAP);
  canvas = gpu.texture(SIZE, SIZE);

  String[] src = loadStrings("more_boids.slang");
  GpuKernel clearGrid   = gpu.kernelSlang(src, "clearGrid");
  GpuKernel insert      = gpu.kernelSlang(src, "insert");
  GpuKernel update      = gpu.kernelSlang(src, "update");
  GpuKernel clearCanvas = gpu.kernelSlang(src, "clearCanvas");
  GpuKernel splat       = gpu.kernelSlang(src, "splat");

  clearGridPass = clearGrid.binding().set("gridCount", gridCount);
  insertA = insert.binding().set("src", a).set("gridCount", gridCount).set("gridBoids", gridBoids);
  insertB = insert.binding().set("src", b).set("gridCount", gridCount).set("gridBoids", gridBoids);
  simAB = update.binding().set("params", params).set("src", a).set("dst", b)
                .set("gridCount", gridCount).set("gridBoids", gridBoids);
  simBA = update.binding().set("params", params).set("src", b).set("dst", a)
                .set("gridCount", gridCount).set("gridBoids", gridBoids);
  clearCanvasPass = clearCanvas.binding().set("canvas", canvas);
  splatA = splat.binding().set("src", a).set("canvas", canvas);
  splatB = splat.binding().set("src", b).set("canvas", canvas);

  params.set("sep", 1.2).set("ali", 1.0).set("coh", 0.8);

  float[] seed = new float[N * 4];
  for (int i = 0; i < N; i++) {
    seed[i*4] = random(SIZE); seed[i*4+1] = random(SIZE);
    seed[i*4+2] = random(-1, 1); seed[i*4+3] = random(-1, 1);
  }
  a.write(seed);
}

void draw() {
  params.set("mouse", gpu.mouseX(), gpu.mouseY())
        .set("spin", gpu.mousePressed() ? 1.0 : 0.0);

  clearGridPass.dispatch(CELLS);
  (ping ? insertB : insertA).dispatch(N);       // 今の最新側でグリッドを作る
  (ping ? simBA : simAB).dispatch(N);
  clearCanvasPass.dispatch(SIZE, SIZE);
  (ping ? splatA : splatB).dispatch(N);         // 更新後の側を描く
  gpu.show(canvas);
  gpu.submit();
  ping = !ping;

  if (!gpu.isOpen()) {
    gpu.dispose();
    exit();
  }
}
