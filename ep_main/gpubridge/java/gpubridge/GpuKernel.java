package gpubridge;

/**
 * WGSL の 1 エントリポイントを指すカーネル。
 *
 * <p>同じカーネルから {@link #binding()} で何個でもバインディングを作れる
 * （ping-pong は 2 個作って三項演算子で選ぶ）。
 */
public final class GpuKernel {

  final long id;

  GpuKernel(long id) {
    this.id = id;
  }

  /** 計測報告に出す名前。付けなければエントリポイント名 */
  public GpuKernel label(String label) {
    GpuBridge.okState(GpuBridge.nSetLabel(id, label));
    return this;
  }

  /** 空のバインディングを作る。set() で WGSL の変数名にリソースを結ぶ。 */
  public GpuBinding binding() {
    return new GpuBinding(GpuBridge.okId(GpuBridge.nBindingCreate(id)));
  }
}
