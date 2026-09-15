// 奥に一列に並んだキューブが同じ時刻に持ち上がって戻る。
// 光速が有限だと遠いものほど遅れて見え、波のように繋がる
class WaveCubesDemo extends Demo {
  PrefsInt count = new PrefsInt("WaveDemoCount", 20);
  PrefsFloat spacing = new PrefsFloat("WaveDemoSpacing", 2);
  PrefsVec3 origin = new PrefsVec3("WaveDemoOrigin", new PVector(0, 0, 6));
  PrefsVec3 size = new PrefsVec3("WaveDemoSize", new PVector(1, 1, 1));
  PrefsInt divisions = new PrefsInt("WaveDemoDivisions", 8);
  PrefsFloat liftHeight = new PrefsFloat("WaveDemoLiftHeight", 1);
  PrefsFloat liftSpeed = new PrefsFloat("WaveDemoLiftSpeed", 1.2);
  PrefsBool floorVisible = new PrefsBool("WaveDemoFloorVisible", true);
  PrefsFloat floorY = new PrefsFloat("WaveDemoFloorY", -2.5);

  List<RenderObject> cubes = new ArrayList<>();
  RenderObject floor;

  // 既定値は origin=(0,0,6), count=20, spacing=2 の列の中心 (0,0,25) を
  // eye=(14,3,4) から見る、これまでの固定視点と同じ向き
  WaveCubesDemo() {
    super("WaveDemo", new PVector(14, 3, 4), -33.690068, 6.778619);
  }

  String name() { return "Wave cubes"; }

  void build(Stage stage) {
    cubes.clear();
    floor = null;
    int cubeDivisions = max(1, divisions.get());
    float[] vertices = makeCubeVertices(cubeDivisions);
    int[] indices = makeCubeIndices(cubeDivisions);
    for (int i = 0; i < max(0, count.get()); i++) {
      RenderObject cube = stage.add(vertices, indices, cubePosition(i, 0));
      cube.scale.set(size.get());
      cubes.add(cube);
    }
    if (floorVisible.get()) {
      Material floorMaterial = engine.createMaterial("plane.slang");
      floor = stage.addStill(makePlaneVertices(1), makePlaneIndices(1), new PVector(0, floorY.get(), 0), floorMaterial);
      floor.scale.set(100, 1, 100);
    }
  }

  void update(Stage stage, float t) {
    float lift = liftOffset(t);
    for (int i = 0; i < cubes.size(); i++) {
      cubes.get(i).position.set(cubePosition(i, lift));
    }
  }

  PVector cubePosition(int index, float lift) {
    return new PVector(origin.get().x, origin.get().y + lift, origin.get().z + index * spacing.get());
  }

  // 最高速度 = liftHeight * PI / liftDuration()。speed をそのまま最高速度にするため、
  // duration をそこから逆算する
  float liftDuration() { return PI * liftHeight.get() / max(abs(liftSpeed.get()), 1e-4); }

  float liftOffset(float t) {
    float duration = liftDuration();
    if (duration <= 1e-6 || t < 0 || t > duration) return 0;
    return liftHeight.get() * 0.5 * (1 - cos(TWO_PI * t / duration));
  }

  float eventDuration() { return liftDuration(); }

  boolean ui() {
    boolean changed = false;
    changed |= count.field();
    changed |= spacing.field();
    changed |= origin.field();
    changed |= size.field();
    changed |= divisions.field();
    changed |= liftHeight.field();
    changed |= liftSpeed.field();
    lightSpeedRatioField("WaveDemoLiftSpeed/c", liftSpeed, lightSpeed.get());
    changed |= floorVisible.checkbox();
    changed |= floorY.field();
    return changed;
  }
}
