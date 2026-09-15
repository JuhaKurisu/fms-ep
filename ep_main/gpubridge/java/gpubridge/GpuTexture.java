package gpubridge;

/** storage texture。compute から textureStore で書き、textureLoad / sample で読める。 */
public final class GpuTexture {

  final long id;
  private final int w;
  private final int h;
  private final GpuFormat format;
  private final boolean cube;
  private final int mips;
  private boolean disposed = false;

  GpuTexture(long id, int w, int h, GpuFormat format, boolean cube, int mips) {
    this.id = id;
    this.w = w;
    this.h = h;
    this.format = format;
    this.cube = cube;
    this.mips = mips;
  }

  public boolean isCube() {
    return cube;
  }

  public int mips() {
    return mips;
  }

  public int width() {
    return w;
  }

  public int height() {
    return h;
  }

  public GpuFormat format() {
    return format;
  }

  /**
   * ARGB ピクセル（Processing の {@code img.pixels} 形式）を書き込む。RGBA8 テクスチャ専用。
   * PImage は {@code tex.write(img.pixels)} と書けばよい。
   */
  public void write(int[] argb) {
    checkAlive();
    GpuBridge.okState(GpuBridge.nTextureWriteI(id, argb));
  }

  /** float ピクセル列を書き込む。RGBA32F は 4 値/px、R32F は 1 値/px。 */
  public void write(float[] data) {
    checkAlive();
    GpuBridge.okState(GpuBridge.nTextureWriteF(id, data));
  }

  /**
   * 1 面 1 段を float ピクセル列で書く。面は +X, −X, +Y, −Y, +Z, −Z の順（2D なら 0）。
   * 段 mip の大きさは width() >> mip。RGBA32F は 4 値/px、R32F は 1 値/px。
   */
  public void writeFace(int face, int mip, float[] data) {
    checkAlive();
    GpuBridge.okState(GpuBridge.nTextureWriteLayerF(id, face, mip, data));
  }

  /** GPU メモリを解放する。以後このテクスチャを使うと例外。二重呼び出しは無害。 */
  public void dispose() {
    if (disposed) {
      return;
    }
    disposed = true;
    GpuBridge.okState(GpuBridge.nRelease(id));
  }

  void checkAlive() {
    if (disposed) {
      throw new IllegalStateException("この GpuTexture は dispose 済みです");
    }
  }
}
