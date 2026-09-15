// gpubridge サンプル: OBJ メッシュ（スザンヌ）を render pipeline で描く
// （インデックス描画・スムーズ法線・深度テスト。ドラッグで回転、放すと自動回転）
import gpubridge.*;

// Processing 自身のウィンドウは出さない
public void showSurface() {
}

final int SIZE = 1024;

GpuWindow gpu;
GpuUniform params;
GpuBuffer mesh, indices;
GpuTexture view;
GpuRenderBinding drawMesh;
int indexCount;
float angleX = 0, angleY = 0;
float pmx, pmy;

// loadObj() の結果
float[] vertexData;   // 8 float/頂点（pos.xyz + 詰め物, スムーズ法線.xyz + 詰め物）
int[] indexData;

void setup() {
  size(200, 200);
  gpu = new GpuWindow(SIZE, SIZE, "suzanne");
  gpu.slangSearchPath(dataPath(""));

  loadObj("suzanne.obj");
  indexCount = indexData.length;
  println(vertexData.length / 8 + " 頂点, " + indexCount / 3 + " 三角形");

  // ストライドを宣言して作るので、bind を待たずに write できる
  mesh = gpu.buffer(vertexData.length / 8, 32);
  mesh.write(vertexData);
  indices = gpu.buffer(indexCount, 4);   // インデックスは「ストライド 4 のバッファ」
  indices.write(indexData);

  GpuRenderer r = gpu.rendererSlang(loadStrings("mesh.slang"), "vsMain", "fsMain");
  r.depthTest(true);

  params = gpu.uniform();
  view = gpu.texture(SIZE, SIZE);
  drawMesh = r.binding().set("params", params).set("vertices", mesh);

  params.set("dist", 4.0).set("fov", radians(50)).set("aspect", 1.0);
}

void draw() {
  float mx = gpu.mouseX(), my = gpu.mouseY();
  if (gpu.mousePressed()) {
    angleY += (mx - pmx) * 0.01;
    angleX += (my - pmy) * 0.01;
  } else {
    angleY += 0.008;   // 触っていない間はゆっくり回す
  }
  pmx = mx;
  pmy = my;
  angleX = constrain(angleX, -HALF_PI, HALF_PI);

  params.set("angleX", angleX).set("angleY", angleY);
  gpu.clear(view, 0.09, 0.09, 0.11, 1);
  drawMesh.drawIndexed(view, indices, indexCount);
  gpu.show(view);
  gpu.submit();

  if (!gpu.isOpen()) {
    gpu.dispose();
    exit();
  }
}

/**
 * OBJ を読み、共有頂点 + インデックスの形で vertexData / indexData に入れる。
 * 頂点は 8 float（pos.xyz + 詰め物, 法線.xyz + 詰め物。mesh.slang の Vertex と対応）。
 * 法線は隣接面の面法線を面積重みで平均したスムーズ法線。
 *
 * 対応するのは v と f のみ（f の負のインデックス・多角形の扇状分割を含む）。
 * 読み込み後、バウンディングボックスで中心化し、最大辺が 2 になるよう拡縮する。
 */
void loadObj(String filename) {
  ArrayList<PVector> verts = new ArrayList<PVector>();
  ArrayList<int[]> faces = new ArrayList<int[]>();
  for (String line : loadStrings(filename)) {
    String[] t = splitTokens(line);
    if (t.length == 0) continue;
    if (t[0].equals("v")) {
      verts.add(new PVector(float(t[1]), float(t[2]), float(t[3])));
    } else if (t[0].equals("f")) {
      int[] idx = new int[t.length - 1];
      for (int i = 1; i < t.length; i++) {
        int vi = int(split(t[i], '/')[0]);
        idx[i - 1] = vi > 0 ? vi - 1 : verts.size() + vi;
      }
      faces.add(idx);
    }
  }

  // 中心化と正規化（OBJ によっては原点からずれている）
  PVector lo = verts.get(0).copy(), hi = verts.get(0).copy();
  for (PVector v : verts) {
    lo.set(min(lo.x, v.x), min(lo.y, v.y), min(lo.z, v.z));
    hi.set(max(hi.x, v.x), max(hi.y, v.y), max(hi.z, v.z));
  }
  PVector center = PVector.add(lo, hi).mult(0.5);
  float scale = 2.0 / max(max(hi.x - lo.x, hi.y - lo.y), hi.z - lo.z);
  for (PVector v : verts) {
    v.sub(center).mult(scale);
  }

  // 三角形化（扇状分割）と、面法線の頂点への積算
  PVector[] normals = new PVector[verts.size()];
  for (int i = 0; i < normals.length; i++) {
    normals[i] = new PVector();
  }
  IntList idx = new IntList();
  for (int[] f : faces) {
    for (int i = 1; i + 1 < f.length; i++) {
      int ia = f[0], ib = f[i], ic = f[i + 1];
      PVector a = verts.get(ia), b = verts.get(ib), c = verts.get(ic);
      PVector n = PVector.sub(b, a).cross(PVector.sub(c, a)); // 長さ = 面積×2 = 重み
      normals[ia].add(n);
      normals[ib].add(n);
      normals[ic].add(n);
      idx.append(ia);
      idx.append(ib);
      idx.append(ic);
    }
  }

  vertexData = new float[verts.size() * 8];
  for (int i = 0; i < verts.size(); i++) {
    PVector p = verts.get(i);
    PVector n = normals[i].copy();
    n.normalize();
    int o = i * 8;
    vertexData[o] = p.x;
    vertexData[o + 1] = p.y;
    vertexData[o + 2] = p.z;
    vertexData[o + 4] = n.x;
    vertexData[o + 5] = n.y;
    vertexData[o + 6] = n.z;
  }
  indexData = idx.array();
}
