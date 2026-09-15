// テストメッシュ: フラット法線のキューブ（1 辺 1、各面を CUBE_DIVISIONS² に分割）

// 面ごとに (N+1)² 頂点、shader.slang の Vertex に合わせて 8 float/頂点
// (pos.xyz, u, 法線.xyz, v) で並べる。UV は面ごとに 0〜1
float[] makeCubeVertices(int cubeDivisions) {
  int n1 = cubeDivisions + 1;
  float[][] normals = {
    {1, 0, 0}, {-1, 0, 0}, {0, 1, 0}, {0, -1, 0}, {0, 0, 1}, {0, 0, -1},
  };
  float[] out = new float[6 * n1 * n1 * 8];
  int o = 0;
  for (float[] n : normals) {
    // 法線と直交する 2 軸
    float[] u = {n[1], n[2], n[0]};
    float[] v = {
      n[1] * u[2] - n[2] * u[1],
      n[2] * u[0] - n[0] * u[2],
      n[0] * u[1] - n[1] * u[0],
    };
    for (int j = 0; j < n1; j++) {
      for (int i = 0; i < n1; i++) {
        float s0 = 2.0 * i / cubeDivisions - 1;
        float s1 = 2.0 * j / cubeDivisions - 1;
        for (int k = 0; k < 3; k++) {
          out[o + k] = 0.5 * (n[k] + s0 * u[k] + s1 * v[k]);
          out[o + 4 + k] = n[k];
        }
        out[o + 3] = (float) i / cubeDivisions;
        out[o + 7] = (float) j / cubeDivisions;
        o += 8;
      }
    }
  }
  return out;
}

int[] makeCubeIndices(int cubeDivisions) {
  int n1 = cubeDivisions + 1;
  int[] out = new int[6 * cubeDivisions * cubeDivisions * 6];
  int o = 0;
  for (int face = 0; face < 6; face++) {
    int base = face * n1 * n1;
    for (int j = 0; j < cubeDivisions; j++) {
      for (int i = 0; i < cubeDivisions; i++) {
        int c00 = base + j * n1 + i;
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
  }
  return out;
}
