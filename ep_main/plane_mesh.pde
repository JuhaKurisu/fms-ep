// 上向き 1 枚の平面メッシュ（1 辺 1、N² に分割、y = 0、法線 (0, 1, 0)）

// (N+1)² 頂点、shader.slang の Vertex に合わせて 8 float/頂点
// (pos.xyz, u, 法線.xyz, v) で並べる。UV は 0〜1
float[] makePlaneVertices(int divisions) {
  int n1 = divisions + 1;
  float[] out = new float[n1 * n1 * 8];
  int o = 0;
  for (int j = 0; j < n1; j++) {
    for (int i = 0; i < n1; i++) {
      float fu = (float) i / divisions;
      float fv = (float) j / divisions;
      out[o] = fu - 0.5;
      out[o + 2] = fv - 0.5;
      out[o + 3] = fu;
      out[o + 5] = 1;
      out[o + 7] = fv;
      o += 8;
    }
  }
  return out;
}

int[] makePlaneIndices(int divisions) {
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
