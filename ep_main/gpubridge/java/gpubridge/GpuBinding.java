package gpubridge;

/**
 * カーネルとリソースの結線。WGSL の変数名で結ぶ。
 *
 * <p>タイポは候補つきの即時例外、型違いも set() の場で例外になる。
 * そのカーネルが使わない変数を set() することはできない。
 */
public final class GpuBinding {

  final long id;

  GpuBinding(long id) {
    this.id = id;
  }

  public GpuBinding set(String name, GpuUniform u) {
    u.checkAlive();
    GpuBridge.okArg(GpuBridge.nBindingSet(id, name, u.id));
    return this;
  }

  public GpuBinding set(String name, GpuBuffer b) {
    b.checkAlive();
    GpuBridge.okArg(GpuBridge.nBindingSet(id, name, b.id));
    return this;
  }

  public GpuBinding set(String name, GpuTexture t) {
    t.checkAlive();
    GpuBridge.okArg(GpuBridge.nBindingSet(id, name, t.id));
    return this;
  }

  /** スレッド数を指定して実行する。workgroup 割りは WGSL の workgroup_size から自動。 */
  public void dispatch(int x) {
    dispatch(x, 1, 1);
  }

  public void dispatch(int x, int y) {
    dispatch(x, y, 1);
  }

  public void dispatch(int x, int y, int z) {
    GpuBridge.okState(GpuBridge.nDispatch(id, x, y, z, true));
  }

  /** 生のワークグループ数で実行する。 */
  public void dispatchGroups(int x, int y, int z) {
    GpuBridge.okState(GpuBridge.nDispatch(id, x, y, z, false));
  }

  /**
   * ワークグループ数を GPU バッファから読んで実行する（GPU が自分で実行量を
   * 決めるパターン）。{@code args} の先頭から u32×3（x, y, z のワークグループ数）を
   * 使う。{@link #dispatch(int)} と違いスレッド数への換算はしない。
   */
  public void dispatchIndirect(GpuBuffer args) {
    dispatchIndirect(args, 0);
  }

  /** {@link #dispatchIndirect(GpuBuffer)} のオフセット指定版。{@code byteOffset} は 4 の倍数。 */
  public void dispatchIndirect(GpuBuffer args, long byteOffset) {
    args.checkAlive();
    GpuBridge.okState(GpuBridge.nDispatchIndirect(id, args.id, byteOffset));
  }
}
