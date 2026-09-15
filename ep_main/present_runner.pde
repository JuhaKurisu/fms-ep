// プレゼンモードの再生状態。ステップの開始はすべて startStep() を通る
class PresentRunner {
  PrefsFloat rampSeconds = new PrefsFloat("PresentLightSpeedRampSeconds", 3.0);

  List<Demo> demos = new ArrayList<>();
  Stage stage = new Stage();
  int demoIndex = 0;
  int stepIndex = 0;
  int startFrame = 0;
  boolean modeActive = false;
  boolean freeCamera = false;
  boolean prevFreeCamera = false;
  boolean showDevPanel = false;
  boolean startRequested = false;
  float rampFromInverse = 0;    // 1/c
  float rampToInverse = 0;
  int rampStartFrame = -1;      // 負ならランプなし
  boolean playAfterStart = false;   // 作り直したあとそのまま再生するか
  boolean leaveRequested = false;

  PresentRunner() {
    demos.add(swingingRodDemo);
    demos.add(terrellRotationDemo);
    demos.add(rollingWheelDemo);
    demos.add(treeRoadDemo);
    demos.add(waveCubesDemo);
  }

  boolean active() {
    return modeActive;
  }
  Demo demo() {
    return demos.get(demoIndex);
  }
  Step step() {
    return demo().steps()[stepIndex];
  }
  float duration() {
    return demo().duration(stage, step().lightSpeed);
  }
  boolean finished() {
    return !demo().loops() && duration() > 0 && localTime() >= duration();
  }

  boolean ramping() {
    return rampStartFrame >= 0
      && (engine.sceneFrame - rampStartFrame) * engine.KEYFRAME_INTERVAL < rampSeconds.get();
  }

  // 無限は 1e6 なので、c をそのまま動かすとほとんどの時間が「ほぼ無限」のままで
  // 最後に一気に落ちる。1/c を動かせば遅れの時間が一定に伸びる
  float currentLightSpeed() {
    if (!ramping()) return step().lightSpeed;
    float u = (engine.sceneFrame - rampStartFrame) * engine.KEYFRAME_INTERVAL / rampSeconds.get();
    float s = u * u * (3 - 2 * u);
    float c = 1.0 / lerp(rampFromInverse, rampToInverse, s);
    return min(c, LIGHT_SPEED_INFINITE);
  }

  float localTime() {
    float t = (engine.sceneFrame - startFrame) * engine.KEYFRAME_INTERVAL;
    return demo().loops() ? max(t, 0) : constrain(t, 0, duration());
  }

  // 入った瞬間から動いていてほしいので、そのまま再生で始める
  void enter() {
    modeActive = true;
    demoIndex = 0;
    stepIndex = 0;
    requestStart(true);
  }

  // UI は update() の後に動くので、ここで stage.clear() すると
  // その回の render() に渡す objects がすでに dispose 済みになってしまう。
  // 実際の破棄・再構築は次フレームの applyRequests() まで遅らせる
  void leave() {
    leaveRequested = true;
  }

  // 同じデモの中でステップだけ変えるなら、作り直さずに c を連続で動かす。
  // 別のデモへ移るときと、終わりのあるデモは t=0 から作り直す。
  // どちらも選んだらそのまま再生する
  void select(int newDemo, int newStep) {
    int targetDemo = constrain(newDemo, 0, demos.size() - 1);
    int targetStep = constrain(newStep, 0, demos.get(targetDemo).steps().length - 1);
    boolean rampable = targetDemo == demoIndex && targetStep != stepIndex && demo().loops();
    float from = rampable ? currentLightSpeed() : 0;
    demoIndex = targetDemo;
    stepIndex = targetStep;
    if (rampable) {
      startRamp(from);
      engine.paused = false;
    } else {
      requestStart(true);
    }
  }

  void startRamp(float fromLightSpeed) {
    rampFromInverse = 1.0 / fromLightSpeed;
    rampToInverse = 1.0 / step().lightSpeed;
    rampStartFrame = engine.sceneFrame;
  }

  // 作り直すだけ。パラメータ変更は t=0 で止めたまま見せる
  void requestStart() {
    requestStart(false);
  }

  void requestStart(boolean play) {
    startRequested = true;
    playAfterStart = play;
  }

  // 終わったところで押した Play は頭から。それ以外は一時停止の切り替え
  void togglePlay() {
    if (engine.paused && finished()) requestStart(true);
    else engine.paused = !engine.paused;
  }

  void applyRequests() {
    if (leaveRequested) {
      leaveRequested = false;
      modeActive = false;
      freeCamera = false;
      stage.clear();
      engine.paused = false;
    }
    if (startRequested) {
      startRequested = false;
      startStep();
      engine.paused = !playAfterStart;
      playAfterStart = false;
    }
  }

  void next() {
    if (stepIndex + 1 < demo().steps().length) select(demoIndex, stepIndex + 1);
    else select((demoIndex + 1) % demos.size(), 0);
  }

  void previous() {
    if (stepIndex > 0) select(demoIndex, stepIndex - 1);
    else {
      int prev = (demoIndex + demos.size() - 1) % demos.size();
      select(prev, demos.get(prev).steps().length - 1);
    }
  }

  // 作り直して t=0 に戻す。再生するかは applyRequests() が決める。履歴のキー 0 は
  // 生成直後の sceneFrame に対応するので、最初の 1 回だけ advanceFrame() の前に記録する
  void startStep() {
    Demo demo = demo();
    stage.clear();
    demo.build(stage);
    rampStartFrame = -1;
    engine.lightSpeed = step().lightSpeed;

    // ランプで到達しうる一番遅い c の分まで履歴を積む。そのステップの c で足りる分しか
    // 積まないと、c = infinite から下げた瞬間に遠いものへ光が届かず消える
    float interval = engine.KEYFRAME_INTERVAL;
    int n = demo.warmupFrames(stage, slowestLightSpeed(demo));

    demo.warmup(stage, -n * interval);
    stage.recordKeyframes();
    for (int k = n - 1; k >= 0; k--) {
      engine.advanceFrame();
      if (k > 0) demo.warmup(stage, -k * interval);
      else demo.update(stage, 0);
      stage.recordKeyframes();
    }

    // 見回せるデモは、始めるたびに既定の向きへ戻す
    if (demo.steerable()) {
      PVector angles = cameraAngles(demo.path().rotation(0));
      cameraYaw = angles.x;
      cameraPitch = angles.y;
    }

    startFrame = engine.sceneFrame;
  }

  float slowestLightSpeed(Demo demo) {
    float slowest = LIGHT_SPEED_INFINITE;
    for (Step s : demo.steps()) slowest = min(slowest, s.lightSpeed);
    return slowest;
  }

  void update(List<RenderObject> objects) {
    Demo demo = demo();
    engine.lightSpeed = currentLightSpeed();
    float t = localTime();
    demo.update(stage, t);

    // 終わりまで来たらそこで止める
    if (!engine.paused && finished()) engine.paused = true;

    if (freeCamera) {
      // オンにした直後は cameraYaw/Pitch が更新されておらず、rotateCamera() が
      // それらから作った回転で今の向きを上書きしてしまう。切り替わった瞬間だけ
      // 現在の camera.rotation から逆算して引き継ぐ
      if (!prevFreeCamera) {
        PVector angles = cameraAngles(camera.rotation);
        cameraYaw = angles.x;
        cameraPitch = angles.y;
      }
      GpuWindow gpu = engine.gpu;
      PVector moveInput = ui.wantTextInput()
        ? new PVector()
        : new PVector(gpu.axis('a', 'd'), gpu.axis('q', 'e'), gpu.axis('s', 'w'));
      PVector rotateInput = ui.wantTextInput()
        ? new PVector()
        : new PVector(gpu.axis(LEFT, RIGHT), gpu.axis(UP, DOWN));
      rotateCamera(engine.KEYFRAME_INTERVAL, rotateInput);
      moveCamera(engine.KEYFRAME_INTERVAL, moveInput);
    } else {
      CameraPath path = demo.path();
      camera.position.set(path.position(t));
      camera.velocity.set(path.velocity(t));
      if (demo.steerable()) {
        PVector rotateInput = ui.wantTextInput()
          ? new PVector()
          : new PVector(engine.gpu.axis(LEFT, RIGHT), engine.gpu.axis(UP, DOWN));
        rotateCamera(engine.KEYFRAME_INTERVAL, rotateInput);
      } else {
        camera.rotation = path.rotation(t);
      }
    }
    prevFreeCamera = freeCamera;

    stage.collect(objects);
  }
}
