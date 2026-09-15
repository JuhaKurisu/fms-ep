// クォータニオン演算の Processing 側実装。data/quaternion.slang と同じ規約:
// xyz を虚部、w を実部として扱い、quatMul(a, b) は「b の回転をしてから a の回転」。
// GPU へは params.set("cameraRotation", q.x, q.y, q.z, q.w) で vec4 として渡す。

class Quaternion {
  float x, y, z, w;

  Quaternion(float x, float y, float z, float w) {
    this.x = x;
    this.y = y;
    this.z = z;
    this.w = w;
  }

  void set(Quaternion quaternion) {
    x = quaternion.x;
    y = quaternion.y;
    z = quaternion.z;
    w = quaternion.w;
  }
}

// 無回転
Quaternion quatIdentity() {
  return new Quaternion(0, 0, 0, 1);
}

// 軸まわりの回転から作る。axis は正規化済みであること。angle はラジアン
Quaternion quatFromAxisAngle(PVector axis, float angle) {
  float half = angle * 0.5;
  float s = sin(half);
  return new Quaternion(axis.x * s, axis.y * s, axis.z * s, cos(half));
}

// オイラー角（ラジアン）から作る。Z * Y * X の順に適用（roll → yaw → pitch）
Quaternion quatFromEuler(float ex, float ey, float ez) {
  float hx = ex * 0.5, hy = ey * 0.5, hz = ez * 0.5;
  float sx = sin(hx), sy = sin(hy), sz = sin(hz);
  float cx = cos(hx), cy = cos(hy), cz = cos(hz);
  return new Quaternion(
    sx * cy * cz - cx * sy * sz,
    cx * sy * cz + sx * cy * sz,
    cx * cy * sz - sx * sy * cz,
    cx * cy * cz + sx * sy * sz);
}

// 合成。quatMul(a, b) は「b の回転をしてから a の回転」（行列積と同じ順序）
Quaternion quatMul(Quaternion a, Quaternion b) {
  return new Quaternion(
    a.w * b.x + b.w * a.x + (a.y * b.z - a.z * b.y),
    a.w * b.y + b.w * a.y + (a.z * b.x - a.x * b.z),
    a.w * b.z + b.w * a.z + (a.x * b.y - a.y * b.x),
    a.w * b.w - (a.x * b.x + a.y * b.y + a.z * b.z));
}

// 正規直交基底（x, y, z は右手系の単位ベクトル）から作る。quatRotate(q, (1,0,0)) == x になる
Quaternion quatFromBasis(PVector x, PVector y, PVector z) {
  float trace = x.x + y.y + z.z;
  if (trace > 0) {
    float s = sqrt(trace + 1) * 2;
    return quatNormalize(new Quaternion((y.z - z.y) / s, (z.x - x.z) / s, (x.y - y.x) / s, 0.25 * s));
  }
  if (x.x > y.y && x.x > z.z) {
    float s = sqrt(1 + x.x - y.y - z.z) * 2;
    return quatNormalize(new Quaternion(0.25 * s, (y.x + x.y) / s, (z.x + x.z) / s, (y.z - z.y) / s));
  }
  if (y.y > z.z) {
    float s = sqrt(1 + y.y - x.x - z.z) * 2;
    return quatNormalize(new Quaternion((y.x + x.y) / s, 0.25 * s, (z.y + y.z) / s, (z.x - x.z) / s));
  }
  float s = sqrt(1 + z.z - x.x - y.y) * 2;
  return quatNormalize(new Quaternion((z.x + x.z) / s, (z.y + y.z) / s, 0.25 * s, (x.y - y.x) / s));
}

// 共役。単位クォータニオンなら逆回転
Quaternion quatConjugate(Quaternion q) {
  return new Quaternion(-q.x, -q.y, -q.z, q.w);
}

// 逆元。非正規化でも正しい逆回転になる
Quaternion quatInverse(Quaternion q) {
  float d = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
  return new Quaternion(-q.x / d, -q.y / d, -q.z / d, q.w / d);
}

// 正規化。累積誤差が溜まったときに掛け直す用
Quaternion quatNormalize(Quaternion q) {
  float len = sqrt(q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w);
  return new Quaternion(q.x / len, q.y / len, q.z / len, q.w / len);
}

// ベクトルに回転を適用する。q は正規化済みであること
PVector quatRotate(Quaternion q, PVector v) {
  // t = 2 * cross(q.xyz, v)
  float tx = 2 * (q.y * v.z - q.z * v.y);
  float ty = 2 * (q.z * v.x - q.x * v.z);
  float tz = 2 * (q.x * v.y - q.y * v.x);
  // v + q.w * t + cross(q.xyz, t)
  return new PVector(
    v.x + q.w * tx + (q.y * tz - q.z * ty),
    v.y + q.w * ty + (q.z * tx - q.x * tz),
    v.z + q.w * tz + (q.x * ty - q.y * tx));
}

// 球面線形補間。t は [0, 1]。a, b は正規化済みであること
Quaternion quatSlerp(Quaternion a, Quaternion b, float t) {
  float d = a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w;
  float bx = b.x, by = b.y, bz = b.z, bw = b.w;
  // 最短経路を通るため、反対側にあれば符号を反転
  if (d < 0) {
    bx = -bx;
    by = -by;
    bz = -bz;
    bw = -bw;
    d = -d;
  }
  // ほぼ同一方向では sin が 0 に近づき不安定なので nlerp に切り替え
  if (d > 0.9995) {
    return quatNormalize(new Quaternion(
      lerp(a.x, bx, t), lerp(a.y, by, t), lerp(a.z, bz, t), lerp(a.w, bw, t)));
  }
  float theta = acos(d);
  float invSin = 1 / sin(theta);
  float wa = sin((1 - t) * theta) * invSin;
  float wb = sin(t * theta) * invSin;
  return new Quaternion(
    wa * a.x + wb * bx, wa * a.y + wb * by, wa * a.z + wb * bz, wa * a.w + wb * bw);
}
