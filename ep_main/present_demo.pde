// 既存コードと同じく、無限の光速は 1e6 で表す
final float LIGHT_SPEED_INFINITE = 1e6;

String lightSpeedLabel(float c) {
  return c >= LIGHT_SPEED_INFINITE ? "infinite" : nf(c, 0, 2);
}

class Step {
  final String label;
  final float lightSpeed;

  Step(String label, float lightSpeed) {
    this.label = label;
    this.lightSpeed = lightSpeed;
  }
}

abstract class Demo {
  PrefsVec3 eye;
  PrefsFloat yaw, pitch;     // 度

  Demo(String keyPrefix, PVector defaultEye, float defaultYaw, float defaultPitch) {
    eye = new PrefsVec3(keyPrefix + "Eye", defaultEye);
    yaw = new PrefsFloat(keyPrefix + "Yaw", defaultYaw);
    pitch = new PrefsFloat(keyPrefix + "Pitch", defaultPitch);
  }

  abstract String name();
  abstract void build(Stage stage);
  abstract void update(Stage stage, float t);   // t >= 0

  // 現象そのものが終わるまで。デモが定義する。終わりが無いデモは 1 周期を返す
  abstract float eventDuration();

  // 往復のように終わりが無いデモは true。止めずにそのまま流し続ける
  boolean loops() { return false; }

  // カメラ自身が走るデモは true。光行差が見せどころなので、速度を消す設定を無視する
  boolean movesCamera() { return false; }

  // 走りながら上下左右キーで見回せるデモは true。位置と速度は軌道のまま、向きだけ操作する
  boolean steerable() { return false; }

  // 光が最も遠いところから届くまでを足した、ステップごとの長さ。ずっと続くデモは
  // 始まる前から光が届いているので、待たずに 1 周期そのまま
  float duration(Stage stage, float stepLightSpeed) {
    if (loops()) return eventDuration();
    float delay = stepLightSpeed >= LIGHT_SPEED_INFINITE
      ? 0
      : stage.farthestDistance(path().position(0)) / stepLightSpeed;
    return eventDuration() + delay;
  }

  // カメラが動くデモは上書きする
  CameraPath path() {
    return new FixedCameraPath(eye.get(), quatMul(
      quatFromAxisAngle(new PVector(0, 1, 0), radians(yaw.get())),
      quatFromAxisAngle(new PVector(1, 0, 0), radians(pitch.get()))));
  }

  // いまのカメラをこのデモの視点として取り込む。自由視点でないときは
  // cameraYaw/cameraPitch が古いので、姿勢から逆算する
  void captureView() {
    eye.set(camera.position);
    PVector angles = cameraAngles(camera.rotation);
    yaw.set(degrees(angles.x));
    pitch.set(degrees(angles.y));
  }

  boolean cameraUI() {
    boolean changed = false;
    changed |= eye.field();
    changed |= yaw.field();
    changed |= pitch.field();
    return changed;
  }

  // Present パネルに出すパラメータ。値が変わったら true を返す
  boolean ui() { return false; }

  // 開始前の履歴に何を積むか。既定は開始状態のまま静止。
  // 動きながら始まるデモは、開始位置を保ったまま動きだけ回すように上書きする
  void warmup(Stage stage, float t) {
    update(stage, 0);
  }

  Step[] steps() {
    return new Step[] {
      new Step("c = infinite", LIGHT_SPEED_INFINITE),
      new Step("c = " + nf(lightSpeed.get(), 0, 1), lightSpeed.get()),
    };
  }

  // 軌道の t=0 の位置から最も遠いオブジェクトまで光が届く時間。余裕を 1.5 倍取る。
  // 生成した瞬間はどのオブジェクトも光が届いていない扱いで消えるので、光速が無限でも
  // 最低 1 フレームは遡る
  int warmupFrames(Stage stage, float stepLightSpeed) {
    float far = stage.farthestDistance(path().position(0), true);
    int frames = ceil(1.5 * far / stepLightSpeed / engine.KEYFRAME_INTERVAL);
    return constrain(frames, 1, engine.KEYFRAME_CAPACITY - 1);
  }
}
