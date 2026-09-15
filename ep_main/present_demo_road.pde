// 並木道を視点が一定の速さで走り抜ける。動くのはカメラだけなので、光速が有限だと
// 光行差で前方の木が視界の中央へ寄り、脇を過ぎた木は後ろへ引き伸ばされて見える。
// 木は Kenney Nature Kit(CC0)の tree_default.obj
class TreeRoadDemo extends Demo {
  final String TREE_MODEL = "models/tree_default.obj";

  PrefsInt count = new PrefsInt("RoadDemoCount", 12);                      // 片側の本数
  PrefsFloat spacing = new PrefsFloat("RoadDemoSpacing", 3.5);
  PrefsFloat treeDistance = new PrefsFloat("RoadDemoTreeDistance", 2.8);   // 道の中心から木まで
  PrefsFloat treeHeight = new PrefsFloat("RoadDemoTreeHeight", 4);
  PrefsFloat sizeVariation = new PrefsFloat("RoadDemoSizeVariation", 0.2);
  PrefsFloat roadWidth = new PrefsFloat("RoadDemoRoadWidth", 4);
  PrefsFloat travelSpeed = new PrefsFloat("RoadDemoTravelSpeed", 5.6);     // 0.7 × 既定の光速 8
  PrefsFloat endMargin = new PrefsFloat("RoadDemoEndMargin", 6);           // 最後の木を過ぎてから止まるまで
  PrefsBool floorVisible = new PrefsBool("RoadDemoFloorVisible", true);

  RenderObject road;
  RenderObject floor;

  // 既定値は道の手前 z=-6、目の高さ 1.6 で奥（+z）を向いた視点
  TreeRoadDemo() {
    super("RoadDemo", new PVector(0, 1.6, -6), 0, 0);
  }

  String name() { return "Tree-lined road"; }

  void build(Stage stage) {
    road = null;
    floor = null;

    List<ObjPart> tree = loadObj(TREE_MODEL);
    Material colorMaterial = engine.createMaterial("color.slang");
    float unitScale = treeHeight.get() / max(objSize(tree).y, 1e-4);
    for (int i = 0; i < max(0, count.get()); i++) {
      for (int side = 0; side < 2; side++) {
        // 左右を半ピッチずらすと真横で対にならず、並木らしく見える
        PVector position = new PVector((side == 0 ? -1 : 1) * treeDistance.get(), 0, (i + side * 0.5) * spacing.get());
        addTree(stage, tree, colorMaterial, position, unitScale, i * 2 + side);
      }
    }

    // 道は床とのぶつかりを避けて少しだけ浮かせる
    road = stage.addStill(makePlaneVertices(1), makePlaneIndices(1), new PVector(0, 0.01, roadCenter()), colorMaterial);
    road.scale.set(roadWidth.get(), 1, roadLength());
    road.baseColor.set(0.2, 0.2, 0.22);
    road.roughness = 0.9;

    if (floorVisible.get()) {
      Material floorMaterial = engine.createMaterial("plane.slang");
      floor = stage.addStill(makePlaneVertices(1), makePlaneIndices(1), new PVector(0, 0, 0), floorMaterial);
      floor.scale.set(200, 1, 200);
    }
  }

  // 幹と葉は mtl ごとに別オブジェクト。頂点配列は読み込みのキャッシュを共有するので、
  // 分割パイプラインへのメッシュのコピーは 1 本ぶんで済む
  void addTree(Stage stage, List<ObjPart> parts, Material material, PVector position, float unitScale, int index) {
    float scale = unitScale * (1 + sizeVariation.get() * (hashUnit(index) * 2 - 1));
    Quaternion rotation = quatFromAxisAngle(new PVector(0, 1, 0), TWO_PI * hashUnit(index + 977));
    for (ObjPart part : parts) {
      RenderObject o = stage.addStill(part.vertices, part.indices, position, material);
      o.scale.set(scale, scale, scale);
      o.rotation = rotation;
      o.baseColor.set(part.baseColor);
      o.roughness = 0.8;
    }
  }

  // 木ごとに大きさと向きを散らすための、番号から決まる 0〜1 の値
  float hashUnit(int index) {
    float v = sin(index * 12.9898) * 43758.5453;
    return v - floor(v);
  }

  float lastTreeZ() {
    return (max(0, count.get()) - 1 + 0.5) * spacing.get();
  }

  float roadLength() {
    return max(lastTreeZ() + endMargin.get() - eye.get().z + spacing.get(), 1);
  }

  float roadCenter() {
    return eye.get().z - spacing.get() * 0.5 + roadLength() * 0.5;
  }

  // 走り抜ける距離。t=0 の位置から最後の木を過ぎるまで
  float travelDistance() {
    return max(lastTreeZ() + endMargin.get() - eye.get().z, 1e-3);
  }

  void update(Stage stage, float t) { }

  CameraPath path() {
    Quaternion look = quatMul(
      quatFromAxisAngle(new PVector(0, 1, 0), radians(yaw.get())),
      quatFromAxisAngle(new PVector(1, 0, 0), radians(pitch.get())));
    PVector motion = PVector.mult(quatRotate(look, new PVector(0, 0, 1)), travelSpeed.get());
    return new StraightCameraPath(eye.get(), look, motion, eventDuration());
  }

  boolean movesCamera() { return true; }

  boolean steerable() { return true; }

  // 走り切ったら道の手前へ戻って、また同じところを走る
  boolean loops() { return true; }

  float eventDuration() {
    return travelDistance() / max(abs(travelSpeed.get()), 1e-4);
  }

  boolean ui() {
    boolean changed = false;
    changed |= count.field();
    changed |= spacing.field();
    changed |= treeDistance.field();
    changed |= treeHeight.field();
    changed |= sizeVariation.field();
    changed |= roadWidth.field();
    lightSpeedRatioField("RoadDemoTravelSpeed/c", travelSpeed, lightSpeed.get());
    changed |= travelSpeed.field();
    changed |= endMargin.field();
    changed |= floorVisible.checkbox();
    return changed;
  }
}

// 一定の速さでまっすぐ進む視点。向きは変えない。loopDuration ごとに出発点へ戻る
class StraightCameraPath implements CameraPath {
  PVector eye;
  PVector motion;
  Quaternion look;
  float loopDuration;

  StraightCameraPath(PVector eye, Quaternion look, PVector motion, float loopDuration) {
    this.eye = eye.copy();
    this.look = look;
    this.motion = motion.copy();
    this.loopDuration = loopDuration;
  }

  PVector position(float t) {
    float lap = loopDuration > 1e-6 ? t - floor(t / loopDuration) * loopDuration : t;
    return PVector.add(eye, PVector.mult(motion, lap));
  }

  Quaternion rotation(float t) { return look; }

  PVector velocity(float t) { return motion.copy(); }
}
