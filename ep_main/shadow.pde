// 指向性光源のシャドウ。毎フレーム光源視点の瞬間 depth を焼き、前フレームとの差分から
// 「光源を出たフレーム m」ごとの層（光がその面に当たった瞬間の深度）をリングバッファに積む。
// フラグメントは遅延時刻と光源方向の深度から m を出し、層を 1 回引いて影を判定する

class ShadowMap {
  final float FAR = 1e9;

  Engine engine;
  GpuUniform shadowParams;
  GpuBuffer layers;
  GpuTexture[] depth = new GpuTexture[2];
  GpuBinding[] accumulate = new GpuBinding[2];
  Material depthMaterial;
  int current = 0;
  int resolution;
  int layerCount;
  boolean warmup = true;
  boolean enabled = true;
  boolean lastEnabled = true;
  float lastLightSpeed = -1;
  PVector lastToLight = new PVector();
  PVector center = new PVector();
  PVector right = new PVector();
  PVector up = new PVector();
  PVector forward = new PVector();
  float halfExtent;
  float depthRange;

  ShadowMap(Engine engine, int resolution, int layerCount) {
    this.engine = engine;
    this.resolution = resolution;
    this.layerCount = layerCount;
    GpuWindow gpu = engine.gpu;

    shadowParams = gpu.uniform();
    layers = gpu.buffer(resolution * resolution * layerCount, 4);
    for (int i = 0; i < 2; i++) {
      depth[i] = gpu.texture(resolution, resolution, GpuFormat.R32F);
    }
    depthMaterial = engine.createMaterial("shadow_depth.slang", false);

    GpuKernel kernel = gpu.kernelSlang(loadStrings("shadow_accumulate.slang"), "accumulate");
    for (int i = 0; i < 2; i++) {
      accumulate[i] = kernel.binding()
        .set("params", engine.params)
        .set("shadowParams", shadowParams)
        .set("depthPrev", depth[1 - i])
        .set("depthCur", depth[i])
        .set("shadowLayers", layers);
    }
    shadowParams
      .set("resolution", resolution)
      .set("layerCount", layerCount);
  }

  // 光源方向・焼く範囲などを設定する。層の意味が変わる変更（光の向き、c）は履歴を作り直す
  void configure(boolean enabled, PVector toLight, PVector center, float halfExtent, float depthRange, float bias, float ambient, float softRadius, int softSamples) {
    this.enabled = enabled;
    this.halfExtent = halfExtent;
    this.depthRange = depthRange;
    this.center.set(center);
    PVector up = toLight.copy().normalize();
    forward = PVector.mult(up, -1);
    PVector worldUp = abs(forward.y) > 0.99 ? new PVector(0, 0, 1) : new PVector(0, 1, 0);
    right = worldUp.cross(forward).normalize();
    PVector lightUp = forward.cross(right).normalize();
    this.up = lightUp;

    // オフ→オンで戻ったとき、リングバッファに残った古い層を読まないようにする
    if (engine.lightSpeed != lastLightSpeed || up.dist(lastToLight) > 1e-6 || enabled != lastEnabled) {
      warmup = true;
      lastLightSpeed = engine.lightSpeed;
      lastToLight = up;
      lastEnabled = enabled;
    }

    shadowParams
      .set("center", center.x, center.y, center.z)
      .set("right", right.x, right.y, right.z)
      .set("up", lightUp.x, lightUp.y, lightUp.z)
      .set("forward", forward.x, forward.y, forward.z)
      .set("toLight", up.x, up.y, up.z)
      .set("halfExtent", halfExtent)
      .set("depthRange", depthRange)
      .set("bias", bias)
      .set("ambient", ambient)
      .set("softRadius", softRadius)
      .set("softSamples", max(softSamples, 1))
      .set("enabled", enabled ? 1 : 0);
  }

  void configureLighting(float lightIntensity, boolean pbrEnabled, boolean iblEnabled, float iblIntensity, boolean tonemapEnabled, float exposure, int lutSize) {
    shadowParams
      .set("lightIntensity", lightIntensity)
      .set("pbrEnabled", pbrEnabled ? 1 : 0)
      .set("iblEnabled", iblEnabled ? 1 : 0)
      .set("iblIntensity", iblIntensity)
      .set("tonemapEnabled", tonemapEnabled ? 1 : 0)
      .set("exposure", exposure)
      .set("lutSize", lutSize);
  }

  // 光が焼く範囲を通り抜けるのにかかるフレーム数。これがリングの層数を超えると奥の影が抜ける
  int layersInFlight() {
    return ceil(depthRange / (engine.lightSpeed * engine.KEYFRAME_INTERVAL));
  }

  void bake(List<RenderObject> objects) {
    if (!enabled) return;
    GpuWindow gpu = engine.gpu;
    current = 1 - current;
    GpuTexture target = depth[current];

    shadowParams
      .set("frame", engine.sceneFrame)
      .set("warmup", warmup ? 1 : 0);

    gpu.clear(target, FAR, 0, 0, 0);
    for (RenderObject object : objects) {
      if (object.wireframeOnly) continue;
      object.shadowBinding.drawIndexed(target, object.indicesBuffer, object.indexData.length);
    }
    accumulate[current].dispatch(resolution, resolution);
    warmup = false;
  }
}
