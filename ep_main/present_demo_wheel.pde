// 車輪が転がりながら一定区間を往復する。光速が有限だと、遠い側のスポークほど
// 古い角度のまま見えて、輪がねじれて見える
class RollingWheelDemo extends Demo {
  PrefsVec3 origin = new PrefsVec3("WheelDemoOrigin", new PVector(0, -0.5, 12));
  PrefsFloat radius = new PrefsFloat("WheelDemoRadius", 2);
  PrefsFloat radialThickness = new PrefsFloat("WheelDemoRadialThickness", 0.3);
  PrefsFloat thickness = new PrefsFloat("WheelDemoThickness", 0.1);
  PrefsInt spokeCount = new PrefsInt("WheelDemoSpokeCount", 8);
  PrefsFloat spokeSize = new PrefsFloat("WheelDemoSpokeSize", 0.1);
  PrefsFloat spokeThickness = new PrefsFloat("WheelDemoSpokeThickness", 0.1);
  PrefsFloat hubRadius = new PrefsFloat("WheelDemoHubRadius", 0.3);
  PrefsFloat hubThickness = new PrefsFloat("WheelDemoHubThickness", 0.1);
  PrefsInt divisions = new PrefsInt("WheelDemoDivisions", 32);
  PrefsInt subDivisions = new PrefsInt("WheelDemoSubDivisions", 16);
  PrefsFloat rotateSpeed = new PrefsFloat("WheelDemoRotateSpeed", 120);   // 度/秒
  PrefsFloat moveDistance = new PrefsFloat("WheelDemoMoveDistance", 4);
  PrefsBool floorVisible = new PrefsBool("WheelDemoFloorVisible", true);
  PrefsFloat floorY = new PrefsFloat("WheelDemoFloorY", -2.5);

  RenderObject wheelObject;
  RenderObject floor;

  // 既定値は origin=(0,-0.5,12) の車輪を、車軸の方向から正面に見る視点
  RollingWheelDemo() {
    super("WheelDemo", new PVector(0, 1.5, 0), 0, 9.462322);
  }

  String name() { return "Rolling wheel"; }

  void build(Stage stage) {
    wheelObject = null;
    floor = null;
    int n = max(1, divisions.get());
    int m = max(1, subDivisions.get());
    int spokes = max(0, spokeCount.get());
    float[] vertices = makeWheelVertices(n, m, radius.get(), radialThickness.get(), thickness.get(),
                                         spokes, spokeSize.get(), spokeThickness.get(),
                                         hubRadius.get(), hubThickness.get());
    int[] indices = makeWheelIndices(n, m, spokes);
    wheelObject = stage.add(vertices, indices, origin.get());
    if (floorVisible.get()) {
      Material floorMaterial = engine.createMaterial("plane.slang");
      floor = stage.addStill(makePlaneVertices(1), makePlaneIndices(1), new PVector(0, floorY.get(), 0), floorMaterial);
      floor.scale.set(100, 1, 100);
    }
  }

  void update(Stage stage, float t) {
    float offset = rollOffset(t);
    wheelObject.position.set(origin.get().x + offset, origin.get().y, origin.get().z);
    wheelObject.rotation = quatFromAxisAngle(new PVector(0, 0, 1), -offset / wheelRadius());
  }

  float wheelRadius() { return max(abs(radius.get()), 1e-4); }

  // 転がった距離を moveDistance の往復に折り返す三角波。周期運動なので負の t でも続く
  float rollOffset(float t) {
    float half = max(abs(moveDistance.get()), 1e-4);
    float period = 4 * half;
    float travel = radians(rotateSpeed.get()) * wheelRadius() * t;
    float phase = ((travel % period) + period) % period;
    return phase < half ? phase
      : phase < 3 * half ? 2 * half - phase
      : phase - period;
  }

  // 三角波が一巡する時間。転がる速さは radians(rotateSpeed) * radius
  float eventDuration() {
    float rollSpeed = radians(abs(rotateSpeed.get())) * wheelRadius();
    return 4 * max(abs(moveDistance.get()), 1e-4) / max(rollSpeed, 1e-4);
  }

  boolean loops() { return true; }

  // ずっと転がり続けていた車輪として始める。静止から始めると、最初のうちは
  // 遅れて届く光も静止した姿のままで、見せたいねじれが出てこない
  void warmup(Stage stage, float t) {
    update(stage, t);
  }

  boolean ui() {
    boolean changed = false;
    changed |= origin.field();
    changed |= radius.field();
    changed |= radialThickness.field();
    changed |= thickness.field();
    changed |= spokeCount.field();
    changed |= spokeSize.field();
    changed |= spokeThickness.field();
    changed |= hubRadius.field();
    changed |= hubThickness.field();
    changed |= divisions.field();
    changed |= subDivisions.field();
    // 転がる車輪は最上点が 2ωr で最も速い。この比が 1 のとき最上点が光速に届く
    lightSpeedRatioField("WheelDemoRimSpeed/c", rotateSpeed, degrees(lightSpeed.get() / (2 * wheelRadius())));
    changed |= rotateSpeed.field();
    changed |= moveDistance.field();
    changed |= floorVisible.checkbox();
    changed |= floorY.field();
    return changed;
  }
}
