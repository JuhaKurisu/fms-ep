// カメラの状態とビュープロジェクション行列

class Camera {
  PVector position = new PVector(0, 0, 0);
  Quaternion rotation = quatFromAxisAngle(new PVector(0, 1, 0), 0);
  float fov = radians(120);
  PVector velocity = new PVector();

  // projection * view（列優先 float[16]）。view はカメラ回転の逆 + 平行移動、
  // projection は WebGPU クリップ空間（z: 0..1、カメラ前方 +z）
  float[] viewProjection(float aspect) {
    Quaternion inv = quatConjugate(rotation);
    // R^-1 の 3x3（列優先で並べる）
    float xx = inv.x * inv.x, yy = inv.y * inv.y, zz = inv.z * inv.z;
    float xy = inv.x * inv.y, xz = inv.x * inv.z, yz = inv.y * inv.z;
    float wx = inv.w * inv.x, wy = inv.w * inv.y, wz = inv.w * inv.z;
    float[] c = {
      1 - 2 * (yy + zz), 2 * (xy + wz), 2 * (xz - wy),
      2 * (xy - wz), 1 - 2 * (xx + zz), 2 * (yz + wx),
      2 * (xz + wy), 2 * (yz - wx), 1 - 2 * (xx + yy),
    };
    PVector t = quatRotate(inv, PVector.mult(position, -1));
    float[] viewM = {
      c[0], c[1], c[2], 0,
      c[3], c[4], c[5], 0,
      c[6], c[7], c[8], 0,
      t.x, t.y, t.z, 1,
    };

    float f = 1.0 / tan(fov * 0.5);
    float near = 0.1, far = 500;
    float[] proj = new float[16];
    proj[0] = f / aspect;
    proj[5] = f;
    proj[10] = far / (far - near);
    proj[11] = 1;
    proj[14] = -far * near / (far - near);

    return mat4Mul(proj, viewM);
  }
}

float[] mat4Mul(float[] a, float[] b) {
  float[] out = new float[16];
  for (int col = 0; col < 4; col++) {
    for (int row = 0; row < 4; row++) {
      float s = 0;
      for (int k = 0; k < 4; k++) {
        s += a[k * 4 + row] * b[col * 4 + k];
      }
      out[col * 4 + row] = s;
    }
  }
  return out;
}

// 視錐台の 8 隅（世界座標）。添字は boxCorners と同じ bit0=x, bit1=y, bit2=z で、z が near/far
PVector[] frustumCorners(PVector position, Quaternion rotation, float fov, float aspect, float near, float far) {
  PVector[] out = new PVector[8];
  for (int i = 0; i < 8; i++) {
    float d = (i & 4) == 0 ? near : far;
    float halfY = d * tan(fov * 0.5);
    float halfX = halfY * aspect;
    PVector local = new PVector(
      (i & 1) == 0 ? -halfX : halfX,
      (i & 2) == 0 ? -halfY : halfY,
      d);
    out[i] = quatRotate(rotation, local).add(position);
  }
  return out;
}
