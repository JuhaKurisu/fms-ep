float[] makeWheelVertices(int divisions, int subDivisions, float radius, float radialThickness,
                          float thickness, int spokeCount, float spokeSize,
                          float spokeThickness, float hubRadius, float hubThickness) {
  FloatList out = new FloatList();
  int n1 = divisions + 1;
  int m1 = subDivisions + 1;
  float inner = radius - radialThickness;
  float hz = thickness * 0.5;
  float hhz = hubThickness * 0.5;

  float[][] faces = {
    {radius, -hz, radius, hz, 1, 0},
    {inner, hz, inner, -hz, -1, 0},
    {radius, hz, inner, hz, 0, 1},
    {inner, -hz, radius, -hz, 0, -1},
    {hubRadius, -hhz, hubRadius, hhz, 1, 0},
    {hubRadius, hhz, 0, hhz, 0, 1},
    {0, -hhz, hubRadius, -hhz, 0, -1},
  };
  for (float[] f : faces) {
    for (int j = 0; j < m1; j++) {
      float fv = (float) j / subDivisions;
      float r = lerp(f[0], f[2], fv);
      float z = lerp(f[1], f[3], fv);
      for (int i = 0; i < n1; i++) {
        float fu = (float) i / divisions;
        float c = cos(TWO_PI * fu);
        float s = sin(TWO_PI * fu);
        wheelVertex(out, r * c, r * s, z, fu, f[4] * c, f[4] * s, f[5], fv);
      }
    }
  }

  float spokeLength = radius - radialThickness * 0.5;
  float[][] normals = {
    {1, 0, 0}, {-1, 0, 0}, {0, 1, 0}, {0, -1, 0}, {0, 0, 1}, {0, 0, -1},
  };
  for (int k = 0; k < spokeCount; k++) {
    float a = TWO_PI * k / spokeCount;
    float ca = cos(a);
    float sa = sin(a);
    for (float[] n : normals) {
      float[] u = {n[1], n[2], n[0]};
      float[] v = {
        n[1] * u[2] - n[2] * u[1],
        n[2] * u[0] - n[0] * u[2],
        n[0] * u[1] - n[1] * u[0],
      };
      for (int j = 0; j < m1; j++) {
        for (int i = 0; i < m1; i++) {
          float s0 = 2.0 * i / subDivisions - 1;
          float s1 = 2.0 * j / subDivisions - 1;
          float lx = (0.5 * (n[0] + s0 * u[0] + s1 * v[0]) + 0.5) * spokeLength;
          float ly = 0.5 * (n[1] + s0 * u[1] + s1 * v[1]) * spokeSize;
          float lz = 0.5 * (n[2] + s0 * u[2] + s1 * v[2]) * spokeThickness;
          wheelVertex(out, lx * ca - ly * sa, lx * sa + ly * ca, lz, (float) i / subDivisions,
                      n[0] * ca - n[1] * sa, n[0] * sa + n[1] * ca, n[2], (float) j / subDivisions);
        }
      }
    }
  }

  return out.toArray();
}

int[] makeWheelIndices(int divisions, int subDivisions, int spokeCount) {
  IntList out = new IntList();
  int n1 = divisions + 1;
  int m1 = subDivisions + 1;
  int base = 0;

  for (int f = 0; f < 7; f++) {
    for (int j = 0; j < subDivisions; j++) {
      for (int i = 0; i < divisions; i++) {
        int c00 = base + j * n1 + i;
        wheelQuad(out, c00, c00 + 1, c00 + n1, c00 + n1 + 1);
      }
    }
    base += n1 * m1;
  }

  for (int k = 0; k < spokeCount * 6; k++) {
    for (int j = 0; j < subDivisions; j++) {
      for (int i = 0; i < subDivisions; i++) {
        int c00 = base + j * m1 + i;
        wheelQuad(out, c00, c00 + 1, c00 + m1, c00 + m1 + 1);
      }
    }
    base += m1 * m1;
  }

  return out.toArray();
}

void wheelVertex(FloatList out, float x, float y, float z, float u,
                 float nx, float ny, float nz, float v) {
  out.append(x);
  out.append(y);
  out.append(z);
  out.append(u);
  out.append(nx);
  out.append(ny);
  out.append(nz);
  out.append(v);
}

void wheelQuad(IntList out, int c00, int c10, int c01, int c11) {
  out.append(c00);
  out.append(c10);
  out.append(c11);
  out.append(c00);
  out.append(c11);
  out.append(c01);
}
