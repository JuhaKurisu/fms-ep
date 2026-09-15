// 奥へ長く伸びた棒が左右に sin で往復する。光速が有限だと、奥の端ほど古い位置の
// まま見えるので、まっすぐな棒が弓なりに曲がって見える
class SwingingRodDemo extends Demo {
  PrefsVec3 origin = new PrefsVec3("RodDemoOrigin", new PVector(-3, -1.5, 6));
  PrefsVec3 size = new PrefsVec3("RodDemoSize", new PVector(1, 1, 12));
  PrefsInt divisions = new PrefsInt("RodDemoDivisions", 32);
  PrefsFloat width = new PrefsFloat("RodDemoWidth", 1);
  PrefsFloat moveSpeed = new PrefsFloat("RodDemoMoveSpeed", 7.2);
  PrefsBool floorVisible = new PrefsBool("RodDemoFloorVisible", true);
  PrefsFloat floorY = new PrefsFloat("RodDemoFloorY", -2.5);

  RenderObject rod;
  RenderObject floor;

  // 既定値は origin=(0,0,6) から奥へ 12 伸びた棒の中心 (-3,-1.5,12) を、
  // 目の高さ 1.5 から見下ろす視点
  SwingingRodDemo() {
    super("RodDemo", new PVector(0, 1.5, 0), -14.036243, 13.633022);
  }

  String name() { return "Swinging rod"; }

  void build(Stage stage) {
    rod = null;
    floor = null;
    int rodDivisions = max(1, divisions.get());
    rod = stage.add(makeCubeVertices(rodDivisions), makeCubeIndices(rodDivisions), rodPosition(0));
    rod.scale.set(size.get());
    if (floorVisible.get()) {
      Material floorMaterial = engine.createMaterial("plane.slang");
      floor = stage.addStill(makePlaneVertices(1), makePlaneIndices(1), new PVector(0, floorY.get(), 0), floorMaterial);
      floor.scale.set(100, 1, 100);
    }
  }

  void update(Stage stage, float t) {
    rod.position.set(rodPosition(t));
  }

  // origin は棒の手前の端。メッシュの原点は中心なので奥へ半分ずらす
  PVector rodPosition(float t) {
    return new PVector(origin.get().x + moveOffset(t), origin.get().y, origin.get().z + size.get().z * 0.5);
  }

  // 最高速度 = width * 2π / period。speed をそのまま最高速度にするため、
  // period をそこから逆算する
  float period() { return TWO_PI * max(abs(width.get()), 1e-4) / max(abs(moveSpeed.get()), 1e-4); }

  // t=0 は中心を最高速度で通過するところ。周期運動なので負の t でもそのまま続く
  float moveOffset(float t) {
    return width.get() * sin(TWO_PI * t / period());
  }

  float eventDuration() { return period(); }

  boolean loops() { return true; }

  // ずっと揺れ続けていた棒として始める。静止から始めると、最初のうちは
  // 遅れて届く光も静止した姿のままで、見せたい曲がりが出てこない
  void warmup(Stage stage, float t) {
    update(stage, t);
  }

  boolean ui() {
    boolean changed = false;
    changed |= origin.field();
    changed |= size.field();
    changed |= divisions.field();
    changed |= width.field();
    lightSpeedRatioField("RodDemoMaxSpeed/c", moveSpeed, lightSpeed.get());
    changed |= moveSpeed.field();
    changed |= floorVisible.checkbox();
    changed |= floorY.field();
    return changed;
  }
}
