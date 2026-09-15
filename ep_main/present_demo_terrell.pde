// 立方体 1 個が左右に sin で往復する。光速が有限だと、こちらを向いた面だけでなく
// 後ろに回った側面まで一緒に見えて、立方体が回転したように見える（テレル回転）
class TerrellRotationDemo extends Demo {
  final String keyPrefix;
  PrefsVec3 origin;
  PrefsVec3 size;
  PrefsInt divisions;
  PrefsFloat width;
  PrefsFloat moveSpeed;
  PrefsBool floorVisible;
  PrefsFloat floorY;

  RenderObject cube;
  RenderObject floor;

  // 既定値は origin=(0,-1.5,12) の立方体を、目の高さ 1.5 から正面に見る視点
  TerrellRotationDemo() {
    this("TerrellDemo", new PVector(0, 1.5, 0), 0, 14.036243);
  }

  // 同じ場面を使うデモに継承させる。prefs のキーが衝突しないよう接頭辞を分ける
  TerrellRotationDemo(String keyPrefix, PVector defaultEye, float defaultYaw, float defaultPitch) {
    super(keyPrefix, defaultEye, defaultYaw, defaultPitch);
    this.keyPrefix = keyPrefix;
    origin = new PrefsVec3(keyPrefix + "Origin", new PVector(0, -1.5, 12));
    size = new PrefsVec3(keyPrefix + "Size", new PVector(2, 2, 2));
    divisions = new PrefsInt(keyPrefix + "Divisions", 16);
    width = new PrefsFloat(keyPrefix + "Width", 6);
    moveSpeed = new PrefsFloat(keyPrefix + "MoveSpeed", 7.2);
    floorVisible = new PrefsBool(keyPrefix + "FloorVisible", true);
    floorY = new PrefsFloat(keyPrefix + "FloorY", -2.5);
  }

  String name() { return "Terrell rotation"; }

  void build(Stage stage) {
    cube = null;
    floor = null;
    int cubeDivisions = max(1, divisions.get());
    cube = stage.add(makeCubeVertices(cubeDivisions), makeCubeIndices(cubeDivisions), cubePosition(0));
    cube.scale.set(size.get());
    if (floorVisible.get()) {
      Material floorMaterial = engine.createMaterial("plane.slang");
      floor = stage.addStill(makePlaneVertices(1), makePlaneIndices(1), new PVector(0, floorY.get(), 0), floorMaterial);
      floor.scale.set(100, 1, 100);
    }
  }

  void update(Stage stage, float t) {
    cube.position.set(cubePosition(t));
  }

  PVector cubePosition(float t) {
    return new PVector(origin.get().x + moveOffset(t), origin.get().y, origin.get().z);
  }

  // 最高速度 = width * 2π / period。speed をそのまま最高速度にするため、
  // period をそこから逆算する
  float period() { return TWO_PI * max(abs(width.get()), 1e-4) / max(abs(moveSpeed.get()), 1e-4); }

  // t=0 は中心を最高速度で通過するところ。周期運動なので負の t でもそのまま続く
  float moveOffset(float t) {
    return width.get() * sin(TWO_PI * t / period());
  }

  // moveOffset の微分。並走するカメラの速度に使う
  float moveVelocity(float t) {
    return width.get() * TWO_PI / period() * cos(TWO_PI * t / period());
  }

  float eventDuration() { return period(); }

  boolean loops() { return true; }

  // ずっと往復し続けていた立方体として始める。静止から始めると、最初のうちは
  // 遅れて届く光も静止した姿のままで、見せたい回転が出てこない
  void warmup(Stage stage, float t) {
    update(stage, t);
  }

  boolean ui() {
    boolean changed = false;
    changed |= origin.field();
    changed |= size.field();
    changed |= divisions.field();
    changed |= width.field();
    lightSpeedRatioField(keyPrefix + "MaxSpeed/c", moveSpeed, lightSpeed.get());
    changed |= moveSpeed.field();
    changed |= floorVisible.checkbox();
    changed |= floorY.field();
    return changed;
  }
}
