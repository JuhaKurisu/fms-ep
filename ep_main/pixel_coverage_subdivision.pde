import java.util.List;
import java.util.IdentityHashMap;

// 画面占有率に応じた三角形の GPU 再帰分割(docs/superpowers/specs/2026-09-12-pixel-coverage-subdivision-design.md)。
// 毎フレーム全オブジェクトの頂点・インデックス・履歴をプールへ GPU コピーし、
// seed → split × (maxDepth+1) → emit で焼いた頂点をマテリアルごとに間接描画する
class PixelCoverageSubdivision {
  final int WORKGROUP = 64;
  final int VERTEX_POOL_CAP = 1 << 20;
  final int INDEX_POOL_CAP = 3 << 20;
  final int MAX_DEPTH = 12;
  // 容量は Prefs から(再起動で反映)。深さ 0 のキューと確定リストは元メッシュの三角形数以上が要る。
  // TriangleData は 208B なので、キュー 2 本 + 確定リストで既定値だと約 830MB
  final int OBJECT_CAP;
  final int QUEUE_CAP;
  final int FINAL_CAP;
  final int VERTEX_CAP;
  final int INDEX_CAP;    // マテリアル 1 つあたり
  final int LINE_CAP;     // ワイヤーフレーム用の線(bakedIndices の末尾領域)

  // frameState のレイアウト(u32 単位)。深さ d のレコード数は [d]。
  // dispatch の間接引数は同じ dispatch 内で読み書きストレージと共存できないので別バッファ(dispatchArgs / emitArgs)
  final int FS_QUEUE_COUNT = 0;
  final int FS_FINAL_COUNT = FS_QUEUE_COUNT + (MAX_DEPTH + 1);
  final int FS_VERTEX_COUNT = FS_FINAL_COUNT + 1;
  final int FS_STATS = FS_VERTEX_COUNT + 1;   // queueOverflow, finalOverflow, vertexOverflow, indexOverflow
  final int FS_DRAW = FS_STATS + 4;           // {indexCount, instanceCount, firstIndex, baseVertex, firstInstance} × MATERIAL_CAP
  final int FS_LINE_DRAW;
  final int FS_SIZE;

  Engine engine;
  GpuBuffer verticesPool, indicesPool, historyPool, objects;
  GpuBuffer queueA, queueB, finalTriangles;
  GpuBuffer bakedVertices, bakedIndices;
  GpuBuffer frameState, frameStateTemplate;
  GpuBuffer[] dispatchArgs = new GpuBuffer[2];   // 深さ d の split の {x,y,z} × (MAX_DEPTH+1)。d の偶奇で使い分ける
  GpuBuffer emitArgs, dispatchTemplate;
  GpuUniform subdivisionParams, lineParams;
  GpuUniform[] passParams = new GpuUniform[MAX_DEPTH + 1];
  GpuBinding seed, emit;
  GpuBinding[] split = new GpuBinding[MAX_DEPTH + 1];
  GpuRenderBinding[] materialBindings;
  boolean[] usedMaterials;
  GpuRenderBinding lineBinding;

  float errorPx = 1;        // 分割点の真の位置と直線補間との画面上のずれがこれを超えたら割る
  float maxEdgePx = 64;     // 辺がこのピクセル数を超えたら誤差に関係なく割る
  boolean splitAngleBisect = true;   // 辺を角度の二等分点で割る(false なら局所座標の中点)
  boolean frustumCull = true;        // 観測者の視野の外に出た辺は割らない
  int maxDepth = MAX_DEPTH;
  int totalTriangles;
  int objectCount;
  int[] stats;              // 直近に読み戻した frameState

  PixelCoverageSubdivision(Engine engine, int objectCap, int queueCap, int finalCap, int vertexCap, int indexCap, int lineCap) {
    this.engine = engine;
    OBJECT_CAP = max(objectCap, 1);
    QUEUE_CAP = max(queueCap, 1);
    FINAL_CAP = max(finalCap, 1);
    VERTEX_CAP = max(vertexCap, 3);
    INDEX_CAP = max(indexCap, 3);
    LINE_CAP = max(lineCap, 6);
    FS_LINE_DRAW = FS_DRAW + engine.MATERIAL_CAP * 5;
    FS_SIZE = FS_LINE_DRAW + 5;
    materialBindings = new GpuRenderBinding[engine.MATERIAL_CAP];
    usedMaterials = new boolean[engine.MATERIAL_CAP];
    stats = new int[FS_SIZE];

    GpuWindow gpu = engine.gpu;
    verticesPool = gpu.buffer(VERTEX_POOL_CAP, 32);
    indicesPool = gpu.buffer(INDEX_POOL_CAP, 4);
    historyPool = gpu.buffer(engine.KEYFRAME_CAPACITY * OBJECT_CAP, 48);
    objects = gpu.buffer(OBJECT_CAP, 52);
    queueA = gpu.buffer(QUEUE_CAP, 208);
    queueB = gpu.buffer(QUEUE_CAP, 208);
    finalTriangles = gpu.buffer(FINAL_CAP, 208);
    bakedVertices = gpu.buffer(VERTEX_CAP, 64);
    bakedIndices = gpu.buffer(INDEX_CAP * engine.MATERIAL_CAP + LINE_CAP, 4);
    frameState = gpu.buffer(FS_SIZE, 4);
    frameStateTemplate = gpu.buffer(FS_SIZE, 4);
    frameStateTemplate.write(makeTemplate());
    for (int i = 0; i < 2; i++) dispatchArgs[i] = gpu.buffer((MAX_DEPTH + 1) * 3, 4);
    emitArgs = gpu.buffer(3, 4);
    dispatchTemplate = gpu.buffer((MAX_DEPTH + 1) * 3, 4);
    int[] ones = new int[(MAX_DEPTH + 1) * 3];
    for (int d = 0; d <= MAX_DEPTH; d++) {
      ones[d * 3 + 1] = 1;
      ones[d * 3 + 2] = 1;
    }
    dispatchTemplate.write(ones);

    subdivisionParams = gpu.uniform()
      .set("queueCap", QUEUE_CAP)
      .set("finalCap", FINAL_CAP)
      .set("vertexCap", VERTEX_CAP)
      .set("indexCap", INDEX_CAP)
      .set("lineCap", LINE_CAP)
      .set("lineBase", INDEX_CAP * engine.MATERIAL_CAP)
      .set("fsFinalCount", FS_FINAL_COUNT)
      .set("fsVertexCount", FS_VERTEX_COUNT)
      .set("fsStats", FS_STATS)
      .set("fsDraw", FS_DRAW)
      .set("fsLineDraw", FS_LINE_DRAW);

    String[] source = loadStrings("pixel_coverage_subdivision.slang");
    seed = gpu.kernelSlang(source, "seed").binding()
      .set("params", engine.params)
      .set("subdivisionParams", subdivisionParams)
      .set("objects", objects)
      .set("verticesPool", verticesPool)
      .set("indicesPool", indicesPool)
      .set("historyPool", historyPool)
      .set("queueOut", queueA)
      .set("frameState", frameState)
      .set("dispatchOut", dispatchArgs[0])
      .set("emitArgs", emitArgs);
    emit = gpu.kernelSlang(source, "emit").binding()
      .set("params", engine.params)
      .set("subdivisionParams", subdivisionParams)
      .set("objects", objects)
      .set("verticesPool", verticesPool)
      .set("indicesPool", indicesPool)
      .set("historyPool", historyPool)
      .set("finalTriangles", finalTriangles)
      .set("bakedVertices", bakedVertices)
      .set("bakedIndices", bakedIndices)
      .set("frameState", frameState);
    // パス d は深さ d のキューを読み、深さ d+1 のキューと dispatch 引数に書く。キューも引数も d の偶奇で交互
    for (int d = 0; d <= MAX_DEPTH; d++) {
      passParams[d] = gpu.uniform().set("inDepth", d).set("outDepth", d + 1).set("last", 0);
      split[d] = gpu.kernelSlang(source, "split").binding()
        .set("params", engine.params)
        .set("subdivisionParams", subdivisionParams)
        .set("passParams", passParams[d])
        .set("objects", objects)
        .set("historyPool", historyPool)
        .set("queueIn", d % 2 == 0 ? queueA : queueB)
        .set("queueOut", d % 2 == 0 ? queueB : queueA)
        .set("finalTriangles", finalTriangles)
        .set("frameState", frameState)
        .set("dispatchOut", dispatchArgs[(d + 1) % 2])
        .set("emitArgs", emitArgs);
    }

    lineParams = gpu.uniform().set("wireColor", 0, 0, 0);
    Material lineMaterial = engine.createMaterial("wireframe_baked.slang", false);
    lineMaterial.renderer.topology(GpuTopology.LINES);
    lineBinding = lineMaterial.renderer.binding()
      .set("params", engine.params)
      .set("bakedVertices", bakedVertices)
      .set("lineParams", lineParams);
  }

  // フレーム先頭にコピーする初期値。描画引数は {0, 1, 領域の先頭, 0, 0}
  int[] makeTemplate() {
    int[] t = new int[FS_SIZE];
    for (int m = 0; m < engine.MATERIAL_CAP; m++) {
      t[FS_DRAW + m * 5 + 1] = 1;
      t[FS_DRAW + m * 5 + 2] = m * INDEX_CAP;
    }
    t[FS_LINE_DRAW + 1] = 1;
    t[FS_LINE_DRAW + 2] = INDEX_CAP * engine.MATERIAL_CAP;
    return t;
  }

  GpuRenderBinding materialBinding(Material m) {
    if (materialBindings[m.slot] == null) {
      GpuRenderBinding b = m.renderer.binding()
        .set("params", engine.params)
        .set("bakedVertices", bakedVertices);
      if (m.lit) {
        b.set("shadowParams", engine.shadow.shadowParams)
         .set("shadowLayers", engine.shadow.layers)
         .set("objects", objects)
         .set("envCube", engine.ibl.envCube)
         .set("irrCube", engine.ibl.irrCube)
         .set("tonemapLut", engine.tonemap.buffer);
      }
      materialBindings[m.slot] = b;
    }
    return materialBindings[m.slot];
  }

  // プールと objects を組み、seed → split → emit を積む。描画は draw() で。
  // 視野の判定は描画カメラでなく観測者の向きで行う(free camera 中も観測者が見る形で割る)
  void calculate(List<RenderObject> objectList, Quaternion viewRotation, float fov, float aspect) {
    GpuWindow gpu = engine.gpu;
    if (engine.sceneFrame % 600 == 120) logStats();
    gpu.copyBufferToBuffer(frameStateTemplate, 0, frameState, 0, (long) FS_SIZE * 4);
    for (int i = 0; i < 2; i++) gpu.copyBufferToBuffer(dispatchTemplate, 0, dispatchArgs[i], 0, (long) (MAX_DEPTH + 1) * 12);
    gpu.copyBufferToBuffer(dispatchTemplate, 0, emitArgs, 0, 12);

    IdentityHashMap<float[], int[]> meshOffsets = new IdentityHashMap<>();   // 同じ頂点配列のメッシュはプールを共有する
    java.util.Arrays.fill(usedMaterials, false);
    int vOff = 0, iOff = 0, triBase = 0, count = 0;
    long historyBytes = (long) engine.KEYFRAME_CAPACITY * 48;
    for (RenderObject o : objectList) {
      if (o.wireframeOnly || o.indexData.length == 0) continue;
      if (count >= OBJECT_CAP) throw new IllegalStateException("分割対象のオブジェクトが OBJECT_CAP(" + OBJECT_CAP + ")を超えました");
      int[] off = meshOffsets.get(o.vertexData);
      if (off == null) {
        int nVerts = o.vertexData.length / 8;
        if (vOff + nVerts > VERTEX_POOL_CAP || iOff + o.indexData.length > INDEX_POOL_CAP) {
          throw new IllegalStateException("メッシュプールが足りません: vertices " + (vOff + nVerts) + "/" + VERTEX_POOL_CAP + ", indices " + (iOff + o.indexData.length) + "/" + INDEX_POOL_CAP);
        }
        gpu.copyBufferToBuffer(o.verticesBuffer, 0, verticesPool, (long) vOff * 32, (long) nVerts * 32);
        gpu.copyBufferToBuffer(o.indicesBuffer, 0, indicesPool, (long) iOff * 4, (long) o.indexData.length * 4);
        off = new int[] {vOff, iOff};
        meshOffsets.put(o.vertexData, off);
        vOff += nVerts;
        iOff += o.indexData.length;
      }
      gpu.copyBufferToBuffer(o.transformHistory, 0, historyPool, o.historySlot * historyBytes, historyBytes);
      int tris = o.indexData.length / 3;
      objects.at(count)
        .set("vertexOffset", off[0])
        .set("indexOffset", off[1])
        .set("triangleBase", triBase)
        .set("triangleCount", tris)
        .set("historyOffset", o.historySlot * engine.KEYFRAME_CAPACITY)
        .set("spawnTime", o.spawnTime())
        .set("materialSlot", o.material.slot)
        .set("historyCapacity", engine.KEYFRAME_CAPACITY)
        .set("roughness", o.roughness)
        .set("metallic", o.metallic)
        .set("colorR", o.baseColor.x)
        .set("colorG", o.baseColor.y)
        .set("colorB", o.baseColor.z);
      usedMaterials[o.material.slot] = true;
      triBase += tris;
      count++;
    }
    totalTriangles = triBase;
    objectCount = count;

    // 視錐 4 側面の内向き法線(ワールド座標)。視野角は垂直、水平は aspect で広げる
    PVector right = quatRotate(viewRotation, new PVector(1, 0, 0));
    PVector up = quatRotate(viewRotation, new PVector(0, 1, 0));
    PVector forward = quatRotate(viewRotation, new PVector(0, 0, 1));
    float hy = fov * 0.5;
    float hx = atan(tan(hy) * aspect);
    PVector left = PVector.add(PVector.mult(right, cos(hx)), PVector.mult(forward, sin(hx)));
    PVector rightPlane = PVector.add(PVector.mult(right, -cos(hx)), PVector.mult(forward, sin(hx)));
    PVector bottom = PVector.add(PVector.mult(up, cos(hy)), PVector.mult(forward, sin(hy)));
    PVector top = PVector.add(PVector.mult(up, -cos(hy)), PVector.mult(forward, sin(hy)));
    float pxToAngle = 2 * tan(hy) / gpu.height();
    float splitAngle = maxEdgePx * pxToAngle;
    subdivisionParams
      .set("totalTriangles", triBase)
      .set("objectCount", count)
      .set("splitAngle", splitAngle)
      .set("errorAngle", errorPx * pxToAngle)
      .set("splitAngleBisect", splitAngleBisect ? 1 : 0)
      .set("frustumCull", frustumCull ? 1 : 0)
      .set("frustum0", left.x, left.y, left.z, 0)
      .set("frustum1", rightPlane.x, rightPlane.y, rightPlane.z, 0)
      .set("frustum2", bottom.x, bottom.y, bottom.z, 0)
      .set("frustum3", top.x, top.y, top.z, 0);

    seed.dispatch(max(triBase, 1));
    for (int d = 0; d <= maxDepth; d++) {
      passParams[d].set("last", d >= maxDepth ? 1 : 0);
      split[d].dispatchIndirect(dispatchArgs[d % 2], (long) d * 12);
    }
    emit.dispatchIndirect(emitArgs);
  }

  void draw(GpuTexture view, boolean wireframe) {
    if (wireframe) {
      lineBinding.drawIndexedIndirect(view, bakedIndices, frameState, (long) FS_LINE_DRAW * 4);
      return;
    }
    for (Material m : engine.materialsBySlot) {
      if (!usedMaterials[m.slot]) continue;
      materialBinding(m).drawIndexedIndirect(view, bakedIndices, frameState, (long) (FS_DRAW + m.slot * 5) * 4);
    }
  }

  // frameState を読み戻す。同期なので数秒に 1 回だけ呼ぶ
  void refreshStats() {
    float[] raw = new float[FS_SIZE];
    frameState.read(raw);
    for (int i = 0; i < FS_SIZE; i++) stats[i] = Float.floatToRawIntBits(raw[i]);
  }

  void logStats() {
    refreshStats();
    println("subdivision: depth=" + maxDepth + " base=" + totalTriangles + " final=" + finalCount() + " vertices=" + stats[FS_VERTEX_COUNT]
      + " overflow=" + stats[FS_STATS] + "/" + stats[FS_STATS + 1] + "/" + stats[FS_STATS + 2] + "/" + stats[FS_STATS + 3]
      + " fps=" + nf(frameRate, 0, 1));
  }

  int finalCount() {
    return stats[FS_FINAL_COUNT];
  }
}
