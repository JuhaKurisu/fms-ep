// 1 辺 1 の箱の 12 辺だけを線で描くワイヤメッシュ。辺は divisions 分割して、
// 光の遅延や収差で歪んでも線として追従するようにする

// 頂点は辺ごとに (divisions+1) 個。shader.slang の Vertex に合わせて 8 float/頂点（法線は 0）
float[] makeBoxWireVertices(int divisions) {
  int n1 = divisions + 1;
  float[] out = new float[12 * n1 * 8];
  int o = 0;
  for (int axis = 0; axis < 3; axis++) {
    for (int corner = 0; corner < 4; corner++) {
      float a = (corner & 1) == 0 ? -0.5 : 0.5;
      float b = (corner & 2) == 0 ? -0.5 : 0.5;
      for (int i = 0; i < n1; i++) {
        float s = (float) i / divisions - 0.5;
        out[o + axis] = s;
        out[o + (axis + 1) % 3] = a;
        out[o + (axis + 2) % 3] = b;
        o += 8;
      }
    }
  }
  return out;
}

int[] makeBoxWireEdges(int divisions) {
  int n1 = divisions + 1;
  int[] out = new int[12 * divisions * 2];
  int o = 0;
  for (int edge = 0; edge < 12; edge++) {
    int base = edge * n1;
    for (int i = 0; i < divisions; i++) {
      out[o++] = base + i;
      out[o++] = base + i + 1;
    }
  }
  return out;
}

// 1 辺 1 の箱の 8 頂点を transform で世界座標にしたもの。添字は bit0=x, bit1=y, bit2=z の符号
PVector[] boxCorners(PVector position, PVector scale, Quaternion rotation) {
  PVector[] out = new PVector[8];
  for (int i = 0; i < 8; i++) {
    PVector local = new PVector(
      ((i & 1) == 0 ? -0.5 : 0.5) * scale.x,
      ((i & 2) == 0 ? -0.5 : 0.5) * scale.y,
      ((i & 4) == 0 ? -0.5 : 0.5) * scale.z);
    out[i] = quatRotate(rotation, local).add(position);
  }
  return out;
}

// 箱の 12 辺と平面の交点を、重心まわりの角度順に並べた凸多角形（0〜6 点）
List<PVector> boxPlaneSection(PVector[] corners, PVector planePoint, PVector planeNormal) {
  PVector n = planeNormal.copy().normalize();
  float[] d = new float[8];
  for (int i = 0; i < 8; i++) d[i] = PVector.sub(corners[i], planePoint).dot(n);
  List<PVector> points = new ArrayList<>();
  for (int a = 0; a < 8; a++) {
    for (int bit = 0; bit < 3; bit++) {
      int b = a | (1 << bit);
      if (b == a) continue;
      if (d[a] == 0) addUnique(points, corners[a]);
      if (d[b] == 0) addUnique(points, corners[b]);
      if (d[a] * d[b] < 0) {
        addUnique(points, PVector.lerp(corners[a], corners[b], d[a] / (d[a] - d[b])));
      }
    }
  }
  if (points.size() < 3) {
    points.clear();
    return points;
  }
  PVector centroid = new PVector();
  for (PVector p : points) centroid.add(p);
  centroid.div(points.size());
  PVector u = (abs(n.y) > 0.99 ? new PVector(1, 0, 0) : new PVector(0, 1, 0)).cross(n).normalize();
  PVector v = n.cross(u);
  points.sort((p, q) -> Float.compare(planeAngle(p, centroid, u, v), planeAngle(q, centroid, u, v)));
  return points;
}

float planeAngle(PVector p, PVector centroid, PVector u, PVector v) {
  PVector r = PVector.sub(p, centroid);
  return atan2(r.dot(v), r.dot(u));
}

void addUnique(List<PVector> points, PVector p) {
  for (PVector q : points) {
    if (q.dist(p) < 1e-4) return;
  }
  points.add(p);
}

// 多角形を閉じた線で描くワイヤメッシュ。maxSides 辺ぶんの頂点を確保し、
// 各辺を divisions 分割する。頂点は makePolygonWireVertices で毎フレーム書き換える
int[] makePolygonWireEdges(int maxSides, int divisions) {
  int n = maxSides * divisions;
  int[] out = new int[n * 2];
  for (int i = 0; i < n; i++) {
    out[i * 2] = i;
    out[i * 2 + 1] = (i + 1) % n;
  }
  return out;
}

// 辺数が maxSides に満たないぶんは先頭の点に潰す（長さ 0 の線になり見えない）
float[] makePolygonWireVertices(List<PVector> polygon, int maxSides, int divisions) {
  float[] out = new float[maxSides * divisions * 8];
  int k = polygon.size();
  int o = 0;
  for (int side = 0; side < maxSides; side++) {
    for (int i = 0; i < divisions; i++) {
      PVector p = k == 0 ? new PVector()
        : side >= k ? polygon.get(0)
        : PVector.lerp(polygon.get(side), polygon.get((side + 1) % k), (float) i / divisions);
      out[o] = p.x;
      out[o + 1] = p.y;
      out[o + 2] = p.z;
      o += 8;
    }
  }
  return out;
}

// 8 隅を結ぶ 12 辺の分割頂点を世界座標のまま書き出す。並びは makeBoxWireVertices と同じなので、
// 辺の index は makeBoxWireEdges を使い回せる。隅の添字は boxCorners と同じ bit0=x, bit1=y, bit2=z
float[] makeCornerWireVertices(PVector[] corners, int divisions) {
  int n1 = divisions + 1;
  float[] out = new float[12 * n1 * 8];
  int o = 0;
  for (int axis = 0; axis < 3; axis++) {
    for (int corner = 0; corner < 4; corner++) {
      int base = ((corner & 1) << ((axis + 1) % 3)) | (((corner >> 1) & 1) << ((axis + 2) % 3));
      PVector p0 = corners[base];
      PVector p1 = corners[base | (1 << axis)];
      for (int i = 0; i < n1; i++) {
        PVector p = PVector.lerp(p0, p1, (float) i / divisions);
        out[o] = p.x;
        out[o + 1] = p.y;
        out[o + 2] = p.z;
        o += 8;
      }
    }
  }
  return out;
}
