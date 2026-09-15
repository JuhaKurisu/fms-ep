package gpubridge;

import java.awt.Component;

/**
 * Rust(wgpu) 側への native 宣言の集約。パッケージ外からは使わない。
 *
 * <p>すべての native は失敗時に false / 0 を返し、理由は {@link #nLastError()} に入る。
 * 例外への変換は各公開クラスが行う。
 */
final class GpuBridge {

  static {
    String explicit = System.getProperty("gpubridge.library");
    if (explicit != null) {
      System.load(explicit);
    } else {
      System.loadLibrary("gpubridge");
    }
  }

  private GpuBridge() {}

  // ---- ライフサイクル ----
  static native boolean nInitGpu();

  static native boolean nAttach(Component component, float scale, int pxW, int pxH);

  static native void nDestroy();

  static native String nBackend();

  static native String nLastError();

  // ---- リソース ----
  static native long nCreateTexture(int w, int h, int format);

  static native long nCreateTextureCube(int size, int mips, int format);

  static native long nCreateBuffer(int elementCount);

  static native long nCreateBufferStrided(int elementCount, int strideBytes);

  static native long nCreateUniform();

  static native long nKernel(String wgsl, String entryPoint);

  static native long nBindingCreate(long kernelId);

  static native boolean nBindingSet(long bindingId, String name, long resourceId);

  static native boolean nDispatch(long bindingId, int x, int y, int z, boolean threads);

  static native boolean nDispatchIndirect(long bindingId, long bufferId, long byteOffset);

  // ---- render ----
  static native long nRenderer(String wgsl, String vsEntry, String fsEntry);

  static native boolean nRendererConfig(long id, int topology, int blend, boolean depthTest);

  static native long nRenderBindingCreate(long rendererId);

  static native boolean nRenderBindingSet(long bindingId, String name, long resourceId);

  static native boolean nDraw(
      long bindingId, long texId, int vertexCount, int instanceCount,
      int sx, int sy, int sw, int sh);

  static native boolean nDrawIndexed(
      long bindingId, long texId, long indexId, int firstIndex, int indexCount,
      int instanceCount, int sx, int sy, int sw, int sh);

  static native boolean nDrawIndexedIndirect(
      long bindingId, long texId, long indexId, long indirectId, long indirectOffset,
      int sx, int sy, int sw, int sh);

  static native boolean nClear(long texId, float r, float g, float b, float a);

  // ---- uniform ----
  static native boolean nUniformSetF(long uid, String member, float[] values);

  static native boolean nUniformSetI(long uid, String member, int value);

  // ---- buffer / texture I/O ----
  static native boolean nBufferSetF(long id, int index, String member, float[] values);

  static native boolean nBufferSetI(long id, int index, String member, int value);

  static native boolean nBufferWriteF(long id, int elementOffset, float[] data);

  static native boolean nBufferWriteI(long id, int elementOffset, int[] data);

  static native boolean nBufferWriteBytesF(
      long id, long byteOffset, float[] data, int from, int count);

  static native boolean nBufferWriteBytesI(long id, long byteOffset, int[] data, int from, int count);

  static native boolean nBufferReadF(long id, float[] dst);

  static native boolean nCopyBufferToBuffer(
      long srcId, long srcOffset, long dstId, long dstOffset, long byteCount);

  static native boolean nTextureWriteI(long id, int[] argb);

  static native boolean nTextureWriteF(long id, float[] data);

  static native boolean nTextureWriteLayerF(long id, int layer, int mip, float[] data);

  // ---- 画面出力 ----
  static native boolean nShow(long texId);

  static native boolean nSubmit();

  static native boolean nSetLabel(long id, String label);

  static native boolean nProfile(boolean on);

  static native String nProfileReport();

  static native boolean nResize(float scale, int pxW, int pxH);

  static native boolean nRelease(long id);

  // ---- 例外変換ヘルパ ----

  /** 構築・実行系の失敗を IllegalStateException に。 */
  static void okState(boolean ok) {
    if (!ok) {
      throw new IllegalStateException(nLastError());
    }
  }

  /** 名前・型の間違いを IllegalArgumentException に。 */
  static void okArg(boolean ok) {
    if (!ok) {
      throw new IllegalArgumentException(nLastError());
    }
  }

  /** id 生成の失敗（0）を例外に。 */
  static long okId(long id) {
    if (id == 0) {
      throw new IllegalArgumentException(nLastError());
    }
    return id;
  }
}
