// テレル回転のデモと同じ場面を、キューブと並んで走るカメラから見る。
// キューブとの相対速度が 0 なのでキューブは歪まず、代わりに止まっている床が
// 流れて歪む。テレル回転が相対速度によるものだと見せるための対のデモ
class ChasingCubeDemo extends TerrellRotationDemo {
  ChasingCubeDemo() {
    super("ChaseDemo", new PVector(0, 1.5, 0), 0, 14.036243);
  }

  String name() { return "Side by side"; }

  // eye は t=0 でのカメラ位置。そこからキューブと同じ sin で横に流れる
  CameraPath path() {
    return new ChaseCameraPath(this, eye.get(), quatMul(
      quatFromAxisAngle(new PVector(0, 1, 0), radians(yaw.get())),
      quatFromAxisAngle(new PVector(1, 0, 0), radians(pitch.get()))));
  }
}

// キューブの x 方向の動きをそのままカメラに写す。向きは変えない
class ChaseCameraPath implements CameraPath {
  TerrellRotationDemo demo;
  PVector eye;
  Quaternion look;

  ChaseCameraPath(TerrellRotationDemo demo, PVector eye, Quaternion look) {
    this.demo = demo;
    this.eye = eye.copy();
    this.look = look;
  }

  PVector position(float t) {
    return new PVector(eye.x + demo.moveOffset(t), eye.y, eye.z);
  }

  Quaternion rotation(float t) { return look; }

  PVector velocity(float t) { return new PVector(demo.moveVelocity(t), 0, 0); }
}
