// HDR(RGBE) の環境マップを読み、縮小・GGX ぼかし・照度に畳み込んでキューブマップに焼く。
// 手順と定数は reference/src/ibl.rs と同じ
class Ibl {
  final int SMALL_W = 128, SMALL_H = 64;
  final int PREF_W = 64, PREF_H = 32;
  final int IRR_W = 32, IRR_H = 16;
  final float[] ROUGHNESS = {0.2, 0.4, 0.6, 0.8, 1.0};
  final int CUBE_SIZE = 256, CUBE_MIPS = 6, IRR_CUBE_SIZE = 16;
  int width0, height0;
  GpuTexture envCube;   // 段 0 は元画像、段 k は粗さ 0.2k のぼかし
  GpuTexture irrCube;

  Ibl(GpuWindow gpu, String file) {
    int[] size = new int[2];
    double[] level0 = loadRgbe(loadBytes(file), size);
    width0 = size[0];
    height0 = size[1];
    long started = System.nanoTime();
    double[] small = downsample(level0, width0, height0, SMALL_W, SMALL_H);
    envCube = gpu.textureCube(CUBE_SIZE, CUBE_MIPS, GpuFormat.RGBA32F);
    writeCube(envCube, 0, level0, width0, height0, CUBE_SIZE);
    for (int l = 0; l < ROUGHNESS.length; l++) {
      double[] p = prefilter(small, SMALL_W, SMALL_H, PREF_W, PREF_H, ROUGHNESS[l]);
      writeCube(envCube, l + 1, p, PREF_W, PREF_H, CUBE_SIZE >> (l + 1));
    }
    double[] irr = irradiance(small, SMALL_W, SMALL_H, IRR_W, IRR_H);
    irrCube = gpu.textureCube(IRR_CUBE_SIZE, 1, GpuFormat.RGBA32F);
    writeCube(irrCube, 0, irr, IRR_W, IRR_H, IRR_CUBE_SIZE);
    println("ibl: " + width0 + "x" + height0 + " prepare " + nf((System.nanoTime() - started) / 1e9, 0, 2) + "s");
  }

  // 正距円筒 src を、面 face のテクセル中心の方向で読んでキューブの 1 段に書く
  void writeCube(GpuTexture cube, int mip, double[] src, int sw, int sh, int size) {
    float[] rgba = new float[size * size * 4];
    for (int face = 0; face < 6; face++) {
      for (int y = 0; y < size; y++) {
        for (int x = 0; x < size; x++) {
          double s = (x + 0.5) / size * 2 - 1, t = (y + 0.5) / size * 2 - 1;
          double[] c = sampleEquirect(src, sw, sh, normalize3(cubeDir(face, s, t)));
          int o = (y * size + x) * 4;
          rgba[o] = (float) c[0]; rgba[o + 1] = (float) c[1]; rgba[o + 2] = (float) c[2]; rgba[o + 3] = 1;
        }
      }
      cube.writeFace(face, mip, rgba);
    }
  }

  // 面 face のテクセル座標 s, t ∈ [-1, 1] の方向。面の順と向きは WebGPU のキューブマップの規約
  double[] cubeDir(int face, double s, double t) {
    switch (face) {
      case 0: return new double[] {1, -t, -s};
      case 1: return new double[] {-1, -t, s};
      case 2: return new double[] {s, 1, t};
      case 3: return new double[] {s, -1, -t};
      case 4: return new double[] {s, -t, 1};
      default: return new double[] {-s, -t, -1};
    }
  }

  double[] normalize3(double[] v) {
    double l = Math.sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    return new double[] {v[0] / l, v[1] / l, v[2] / l};
  }

  // 正距円筒の双線形読み出し。u は巻き込み、v は端で止める。d.z がちょうど 0 の atan2 は手で分ける
  double[] sampleEquirect(double[] src, int w, int h, double[] d) {
    double u = d[2] == 0 ? (d[0] > 0 ? 0.75 : (d[0] < 0 ? 0.25 : 0.5)) : Math.atan2(d[0], -d[2]) / (2 * Math.PI) + 0.5;
    double v = Math.acos(Math.max(-1, Math.min(1, d[1]))) / Math.PI;
    double x = u * w - 0.5, y = v * h - 0.5;
    double x0 = Math.floor(x), y0 = Math.floor(y);
    double fx = x - x0, fy = y - y0;
    int ix = (int) x0, iy = (int) y0;
    double[] out = new double[3];
    for (int k = 0; k < 3; k++) {
      out[k] = (texel(src, w, h, ix, iy, k) * (1 - fx) + texel(src, w, h, ix + 1, iy, k) * fx) * (1 - fy)
             + (texel(src, w, h, ix, iy + 1, k) * (1 - fx) + texel(src, w, h, ix + 1, iy + 1, k) * fx) * fy;
    }
    return out;
  }

  double texel(double[] src, int w, int h, int x, int y, int k) {
    x = ((x % w) + w) % w;
    y = Math.max(0, Math.min(h - 1, y));
    return src[(y * w + x) * 3 + k];
  }

  // 縮小・拡大どちらでも成り立つように、出力画素ごとに対応する入力画素の範囲を平均する
  double[] downsample(double[] src, int sw, int sh, int w, int h) {
    double[] out = new double[w * h * 3];
    for (int j = 0; j < h; j++) {
      int y0 = j * sh / h, y1 = Math.min(Math.max(((j + 1) * sh + h - 1) / h, y0 + 1), sh);
      for (int i = 0; i < w; i++) {
        int x0 = i * sw / w, x1 = Math.min(Math.max(((i + 1) * sw + w - 1) / w, x0 + 1), sw);
        double r = 0, g = 0, b = 0;
        int count = 0;
        for (int y = y0; y < y1; y++) {
          for (int x = x0; x < x1; x++) {
            int s = (y * sw + x) * 3;
            r += src[s]; g += src[s + 1]; b += src[s + 2];
            count++;
          }
        }
        double n = Math.max(count, 1);
        int o = (j * w + i) * 3;
        out[o] = r / n; out[o + 1] = g / n; out[o + 2] = b / n;
      }
    }
    return out;
  }

  // (u, v) の方向。φ = (u - 0.5) 2π、θ = v π
  double[] dir(double u, double v) {
    double phi = (u - 0.5) * 2 * Math.PI, theta = v * Math.PI;
    return new double[] {Math.sin(theta) * Math.sin(phi), Math.cos(theta), -Math.sin(theta) * Math.cos(phi)};
  }

  double[] prefilter(double[] src, int sw, int sh, int w, int h, float roughness) {
    double a = roughness * roughness, a2 = a * a;
    return convolve(src, sw, sh, w, h, a2, true);
  }

  double[] irradiance(double[] src, int sw, int sh, int w, int h) {
    return convolve(src, sw, sh, w, h, 0, false);
  }

  // ggx なら N=V=R の GGX 重み(正規化)、そうでなければ cos 重みの総和 / π
  double[] convolve(double[] src, int sw, int sh, int w, int h, double a2, boolean ggx) {
    double[][] dirs = new double[sw * sh][];
    double[] omega = new double[sw * sh];
    for (int y = 0; y < sh; y++) {
      double v = (y + 0.5) / sh;
      for (int x = 0; x < sw; x++) {
        dirs[y * sw + x] = dir((x + 0.5) / sw, v);
        omega[y * sw + x] = (2 * Math.PI / sw) * (Math.PI / sh) * Math.sin(v * Math.PI);
      }
    }
    double[] out = new double[w * h * 3];
    for (int j = 0; j < h; j++) {
      for (int i = 0; i < w; i++) {
        double[] n = dir((i + 0.5) / w, (j + 0.5) / h);
        double r = 0, g = 0, b = 0, wsum = 0;
        for (int t = 0; t < sw * sh; t++) {
          double[] l = dirs[t];
          double nl = n[0] * l[0] + n[1] * l[1] + n[2] * l[2];
          if (nl <= 0) continue;
          double wgt;
          if (ggx) {
            double hx = l[0] + n[0], hy = l[1] + n[1], hz = l[2] + n[2];
            double hl = Math.sqrt(hx * hx + hy * hy + hz * hz);
            double nh = Math.max((n[0] * hx + n[1] * hy + n[2] * hz) / hl, 0);
            double dd = nh * nh * (a2 - 1) + 1;
            wgt = a2 / (Math.PI * dd * dd) * nl * omega[t];
          } else {
            wgt = nl * omega[t];
          }
          r += src[t * 3] * wgt; g += src[t * 3 + 1] * wgt; b += src[t * 3 + 2] * wgt;
          wsum += wgt;
        }
        double scale = ggx ? 1 / Math.max(wsum, 1e-12) : 1 / Math.PI;
        int o = (j * w + i) * 3;
        out[o] = r * scale; out[o + 1] = g * scale; out[o + 2] = b * scale;
      }
    }
    return out;
  }
}

// Radiance RGBE(.hdr)。新形式の RLE と無圧縮に対応。値は image crate と同じく c * 2^(e - 136)
double[] loadRgbe(byte[] b, int[] sizeOut) {
  int p = 0;
  String line;
  int width = 0, height = 0;
  while (true) {
    if (p >= b.length) throw new RuntimeException("RGBE: ヘッダが見つかりません");
    int start = p;
    while (b[p] != '\n') p++;
    line = new String(b, start, p - start);
    p++;
    if (line.startsWith("-Y")) {
      String[] t = splitTokens(line);
      height = int(t[1]);
      width = int(t[3]);
      break;
    }
  }
  sizeOut[0] = width;
  sizeOut[1] = height;
  double[] out = new double[width * height * 3];
  byte[] scan = new byte[width * 4];
  for (int y = 0; y < height; y++) {
    boolean rle = width >= 8 && width < 32768 && b[p] == 2 && b[p + 1] == 2 && (b[p + 2] & 0x80) == 0;
    if (rle) {
      p += 4;
      for (int c = 0; c < 4; c++) {
        int x = 0;
        while (x < width) {
          int count = b[p++] & 0xFF;
          if (count > 128) {
            count -= 128;
            byte v = b[p++];
            for (int k = 0; k < count; k++) scan[(x++) * 4 + c] = v;
          } else {
            if (count == 0) throw new RuntimeException("RGBE: 不正な RLE");
            for (int k = 0; k < count; k++) scan[(x++) * 4 + c] = b[p++];
          }
        }
      }
    } else {
      System.arraycopy(b, p, scan, 0, width * 4);
      p += width * 4;
    }
    for (int x = 0; x < width; x++) {
      int e = scan[x * 4 + 3] & 0xFF;
      double f = e == 0 ? 0 : Math.scalb(1.0, e - 136);
      int o = (y * width + x) * 3;
      out[o] = (scan[x * 4] & 0xFF) * f;
      out[o + 1] = (scan[x * 4 + 1] & 0xFF) * f;
      out[o + 2] = (scan[x * 4 + 2] & 0xFF) * f;
    }
  }
  return out;
}
