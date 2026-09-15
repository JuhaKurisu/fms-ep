import gpubridge.*;
import java.util.HashMap;
import java.util.Map;
import java.util.List;
import java.util.ArrayList;

class Engine {
  GpuWindow gpu;
  GpuUniform params;
  Map<String, Material> materials = new HashMap<>();
  List<Material> materialsBySlot = new ArrayList<>();
  final int MATERIAL_CAP = 8;   // 分割パイプラインのインデックス領域の数
  java.util.ArrayDeque<Integer> freeHistorySlots = new java.util.ArrayDeque<>();
  int nextHistorySlot = 0;
  Material defaultMaterial;
  Material wireframeMaterial;
  ShadowMap shadow;
  Ibl ibl;
  Tonemap tonemap;
  GpuRenderer backgroundRenderer;
  GpuRenderBinding backgroundBinding;
  GpuUniform backgroundParams;

  final float TARGET_FPS = 60;
  final float KEYFRAME_INTERVAL = 1.0 / TARGET_FPS;
  final int KEYFRAME_CAPACITY = 8192;
  final int SEARCH_STEPS = ceil(log(KEYFRAME_CAPACITY) / log(2));

  int sceneFrame = 0;
  float sceneTime = 0;
  boolean paused = false;
  boolean stepOnce = false;     // 一時停止中に 1 フレームだけ進める
  boolean advanced = true;      // このフレームで sceneFrame が進んだか
  float lightSpeed = 1e9;

  PixelCoverageSubdivision pixelCoverageSubdivision;   // Engine 生成後に ep_main が容量を渡して作る

  Engine(int width, int height, String title, float lightSpeed, int shadowResolution, int shadowLayers) {
    gpu = new GpuWindow(width, height, title, true);
    println("backend = " + gpu.backend());

    gpu.slangSearchPath(dataPath(""));

    params = gpu.uniform();

    this.lightSpeed = lightSpeed;
    shadow = new ShadowMap(this, shadowResolution, shadowLayers);
    ibl = new Ibl(gpu, "env.hdr");
    tonemap = new Tonemap(gpu);
    backgroundRenderer = gpu.rendererSlang(loadStrings("background.slang"), "vertexMain", "fragmentMain").label("background.slang").depthTest(false);
    backgroundParams = gpu.uniform();
    backgroundBinding = backgroundRenderer.binding()
      .set("params", params)
      .set("shadowParams", shadow.shadowParams)
      .set("backgroundParams", backgroundParams)
      .set("envCube", ibl.envCube)
      .set("irrCube", ibl.irrCube)
      .set("tonemapLut", tonemap.buffer);
    defaultMaterial = createMaterial("shader.slang");
    wireframeMaterial = createMaterial("wireframe.slang", false);
    wireframeMaterial.renderer.topology(GpuTopology.LINES);

    frameRate(TARGET_FPS);

    params
      .set("keyFrameInterval", KEYFRAME_INTERVAL)
      .set("searchSteps", SEARCH_STEPS)
      .set("lightSpeed", lightSpeed);
  }

  void nextFrame() {
    advanced = !paused || stepOnce;
    stepOnce = false;
    if (!advanced) return;
    advanceFrame();
  }

  // 一時停止に関係なく 1 フレーム進める
  void advanceFrame() {
    sceneFrame++;
    sceneTime = sceneFrame * KEYFRAME_INTERVAL;
  }

  void render(GpuTexture view, Camera camera, List<RenderObject> objects) {
    render(view, camera, new Observer(camera.position, camera.velocity, camera.rotation), objects);
  }

  void render(GpuTexture view, Camera camera, Observer observer, List<RenderObject> objects) {
    render(view, camera, observer, objects, false);
  }

  void render(GpuTexture view, Camera camera, Observer observer, List<RenderObject> objects, boolean wireframe) {
    float aspect = (float) view.width() / view.height();

    params
      .set("resolution", view.width(), view.height())
      .set("observerPosition", observer.position.x, observer.position.y, observer.position.z)
      .set("observerVelocity", observer.velocity.x, observer.velocity.y, observer.velocity.z)
      .set("observerTime", sceneTime)
      .set("viewProjection", camera.viewProjection(aspect))
      .set("lightSpeed", lightSpeed);

    for (RenderObject object : objects) {
      object.recordKeyframes();
    }
    shadow.bake(objects);
    pixelCoverageSubdivision.calculate(objects, observer.rotation, camera.fov, aspect);

    gpu.clear(view, 1, 1, 1, 1);
    backgroundParams
      .set("cameraRotation", camera.rotation.x, camera.rotation.y, camera.rotation.z, camera.rotation.w)
      .set("tanHalfFov", tan(camera.fov * 0.5))
      .set("aspect", aspect);
    backgroundBinding.draw(view, 3);
    pixelCoverageSubdivision.draw(view, wireframe);

    for (RenderObject object : objects) {
      if (object.wireframeOnly) {
        object.wireframeBinding.drawIndexed(view, object.edgesBuffer, object.edgeData.length);
      }
    }
  }

  // 三角形インデックスから重複を除いた辺のインデックス列を作る
  int[] makeEdgeIndices(int[] indexData) {
    java.util.LinkedHashSet<Long> edges = new java.util.LinkedHashSet<>();
    for (int i = 0; i + 2 < indexData.length; i += 3) {
      for (int k = 0; k < 3; k++) {
        int a = indexData[i + k];
        int b = indexData[i + (k + 1) % 3];
        edges.add(((long) min(a, b) << 32) | (max(a, b) & 0xFFFFFFFFL));
      }
    }
    int[] result = new int[edges.size() * 2];
    int n = 0;
    for (long e : edges) {
      result[n++] = (int) (e >>> 32);
      result[n++] = (int) e;
    }
    return result;
  }

  Material createMaterial(String shaderFile) {
    return createMaterial(shaderFile, true);
  }

  Material createMaterial(String shaderFile, boolean lit) {
    Material cached = materials.get(shaderFile);
    if (cached != null) return cached;

    GpuRenderer renderer = gpu
      .rendererSlang(loadStrings(shaderFile), "vertexMain", "fragmentMain")
      .label(shaderFile)
      .depthTest(true)
      .topology(GpuTopology.TRIANGLES);
    if (materialsBySlot.size() >= MATERIAL_CAP) throw new IllegalStateException("マテリアルが MATERIAL_CAP(" + MATERIAL_CAP + ")を超えました");
    Material m = new Material(renderer, lit, materialsBySlot.size());
    materialsBySlot.add(m);
    materials.put(shaderFile, m);
    return m;
  }

  RenderObject createObject(float[] vertexData, int[] indexData, PVector position) {
    return createObject(vertexData, indexData, position, defaultMaterial);
  }

  RenderObject createObject(float[] vertexData, int[] indexData, PVector position, Material material) {
    return createObject(vertexData, indexData, null, position, material);
  }

  // edgeData を渡すとワイヤーフレームの辺をそれで置き換える（null なら三角形から作る）
  RenderObject createObject(float[] vertexData, int[] indexData, int[] edgeData, PVector position, Material material) {
    RenderObject o = new RenderObject();
    o.material = material;
    o.vertexData = vertexData;
    o.indexData = indexData;
    o.position.set(position);
    o.spawnFrame = sceneFrame;

    o.verticesBuffer = gpu.buffer(o.vertexData.length / 8, 32);
    o.verticesBuffer.write(o.vertexData);
    o.indicesBuffer = gpu.buffer(max(o.indexData.length, 1), 4);
    if (o.indexData.length > 0) o.indicesBuffer.write(o.indexData);
    o.edgeData = edgeData != null ? edgeData : makeEdgeIndices(o.indexData);
    o.edgesBuffer = gpu.buffer(o.edgeData.length, 4);
    o.edgesBuffer.write(o.edgeData);

    o.transformHistory = gpu.buffer(KEYFRAME_CAPACITY, 48);
    o.historySlot = allocHistorySlot();

    o.shaderParams = gpu.uniform();
    o.shadowBinding = shadow.depthMaterial.renderer.binding()
      .set("params", params)
      .set("shaderParams", o.shaderParams)
      .set("vertices", o.verticesBuffer)
      .set("objectTransformHistory", o.transformHistory)
      .set("shadowParams", shadow.shadowParams);
    o.wireframeBinding = wireframeMaterial.renderer.binding()
      .set("params", params)
      .set("shaderParams", o.shaderParams)
      .set("vertices", o.verticesBuffer)
      .set("objectTransformHistory", o.transformHistory);
    o.shaderParams
      .set("objectSpawnTime", o.spawnTime())
      .set("wireColor", 0, 0, 0);

    return o;
  }

  int allocHistorySlot() {
    if (!freeHistorySlots.isEmpty()) return freeHistorySlots.pop();
    return nextHistorySlot++;
  }

  void freeHistorySlot(int slot) {
    freeHistorySlots.push(slot);
  }
}
