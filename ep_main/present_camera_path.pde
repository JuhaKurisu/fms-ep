// ローカル時刻からカメラの位置・姿勢・速度を返す。速度は光行差に効くので必ず与える
interface CameraPath {
  PVector position(float t);
  Quaternion rotation(float t);
  PVector velocity(float t);
}

class FixedCameraPath implements CameraPath {
  PVector eye;
  Quaternion look;

  FixedCameraPath(PVector eye, Quaternion look) {
    this.eye = eye.copy();
    this.look = look;
  }

  PVector position(float t) { return eye.copy(); }
  Quaternion rotation(float t) { return look; }
  PVector velocity(float t) { return new PVector(); }
}

// キーフレームを線形補間する。times は昇順で、positions / rotations と同じ長さ
class KeyframeCameraPath implements CameraPath {
  float[] times;
  PVector[] positions;
  Quaternion[] rotations;

  KeyframeCameraPath(float[] times, PVector[] positions, Quaternion[] rotations) {
    this.times = times;
    this.positions = positions;
    this.rotations = rotations;
  }

  // t を含む区間の開始インデックス。範囲外は端に寄せる
  int segment(float t) {
    for (int i = times.length - 2; i >= 0; i--) {
      if (t >= times[i]) return i;
    }
    return 0;
  }

  float fraction(int i, float t) {
    float span = times[i + 1] - times[i];
    return span > 1e-6 ? constrain((t - times[i]) / span, 0, 1) : 0;
  }

  PVector position(float t) {
    if (times.length == 1) return positions[0].copy();
    int i = segment(t);
    return PVector.lerp(positions[i], positions[i + 1], fraction(i, t));
  }

  Quaternion rotation(float t) {
    if (times.length == 1) return rotations[0];
    int i = segment(t);
    return quatSlerp(rotations[i], rotations[i + 1], fraction(i, t));
  }

  PVector velocity(float t) {
    if (times.length == 1) return new PVector();
    int i = segment(t);
    float span = times[i + 1] - times[i];
    if (span <= 1e-6) return new PVector();
    return PVector.sub(positions[i + 1], positions[i]).div(span);
  }
}

// クォータニオンから yaw/pitch(ラジアン)を取り出す。rotateCamera の合成順の逆
PVector cameraAngles(Quaternion rotation) {
  PVector f = quatRotate(rotation, new PVector(0, 0, 1));
  return new PVector(atan2(f.x, f.z), constrain(asin(constrain(-f.y, -1, 1)), radians(-89), radians(89)));
}

// eye から target を見る姿勢。基底の作り方は shadow.pde に合わせる
Quaternion lookRotation(PVector eye, PVector target) {
  PVector forward = PVector.sub(target, eye).normalize();
  PVector worldUp = abs(forward.y) > 0.99 ? new PVector(0, 0, 1) : new PVector(0, 1, 0);
  PVector right = worldUp.cross(forward).normalize();
  PVector up = forward.cross(right).normalize();
  return quatFromBasis(right, up, forward);
}
