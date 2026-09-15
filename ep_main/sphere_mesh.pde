// UV スフィアメッシュ（直径 1、経度 × 緯度を N² に分割、スムーズ法線）

// (N+1)² 頂点、shader.slang の Vertex に合わせて 8 float/頂点
// (pos.xyz, u, 法線.xyz, v) で並べる。u = 経度 0〜1、v = 緯度 0〜1（v=0 が上極）
static float[] makeSphereVertices(int divisions) {
  int n1 = divisions + 1;
  float[] out = new float[n1 * n1 * 8];
  int o = 0;
  for (int j = 0; j < n1; j++) {
    for (int i = 0; i < n1; i++) {
      float fu = (float) i / divisions;
      float fv = (float) j / divisions;
      float phi = TWO_PI * fu;
      float theta = PI * fv;
      float nx = sin(theta) * cos(phi);
      float ny = cos(theta);
      float nz = sin(theta) * sin(phi);
      out[o] = 0.5 * nx;
      out[o + 1] = 0.5 * ny;
      out[o + 2] = 0.5 * nz;
      out[o + 3] = fu;
      out[o + 4] = nx;
      out[o + 5] = ny;
      out[o + 6] = nz;
      out[o + 7] = fv;
      o += 8;
    }
  }
  return out;
}

int[] makeSphereIndices(int divisions) {
  int n1 = divisions + 1;
  int[] out = new int[divisions * divisions * 6];
  int o = 0;
  for (int j = 0; j < divisions; j++) {
    for (int i = 0; i < divisions; i++) {
      int c00 = j * n1 + i;
      int c10 = c00 + 1;
      int c01 = c00 + n1;
      int c11 = c01 + 1;
      out[o++] = c00;
      out[o++] = c10;
      out[o++] = c11;
      out[o++] = c00;
      out[o++] = c11;
      out[o++] = c01;
    }
  }
  return out;
}
