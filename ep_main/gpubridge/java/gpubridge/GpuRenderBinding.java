package gpubridge;

/**
 * レンダラーとリソースの結線。{@link GpuBinding} の描画版で、set() の流儀は同じ。
 *
 * <p>タイポは候補つきの即時例外、型違いも set() の場で例外になる。
 */
public final class GpuRenderBinding {

  final long id;

  private int scX, scY, scW = -1, scH; // scW < 0 はシザーなし

  GpuRenderBinding(long id) {
    this.id = id;
  }

  /**
   * 以後の draw に適用するシザー矩形を指定する(物理ピクセル)。描画は
   * この矩形内に切り抜かれる。{@link #scissorOff()} で解除するまで有効。
   */
  public GpuRenderBinding scissor(int x, int y, int w, int h) {
    if (w <= 0 || h <= 0) {
      throw new IllegalArgumentException("シザー矩形の幅・高さは 1 以上が必要です: " + w + "x" + h);
    }
    scX = x;
    scY = y;
    scW = w;
    scH = h;
    return this;
  }

  /** シザーを解除してターゲット全面に戻す。 */
  public GpuRenderBinding scissorOff() {
    scW = -1;
    return this;
  }

  public GpuRenderBinding set(String name, GpuUniform u) {
    u.checkAlive();
    GpuBridge.okArg(GpuBridge.nRenderBindingSet(id, name, u.id));
    return this;
  }

  public GpuRenderBinding set(String name, GpuBuffer b) {
    b.checkAlive();
    GpuBridge.okArg(GpuBridge.nRenderBindingSet(id, name, b.id));
    return this;
  }

  public GpuRenderBinding set(String name, GpuTexture t) {
    t.checkAlive();
    GpuBridge.okArg(GpuBridge.nRenderBindingSet(id, name, t.id));
    return this;
  }

  /** {@code target} へ 1 インスタンス描く。 */
  public void draw(GpuTexture target, int vertexCount) {
    draw(target, vertexCount, 1);
  }

  /**
   * {@code target} へ instanceCount 個描く。同じターゲットへの連続 draw は
   * 内部で 1 つの render pass にまとめられる。
   */
  public void draw(GpuTexture target, int vertexCount, int instanceCount) {
    target.checkAlive();
    GpuBridge.okState(GpuBridge.nDraw(id, target.id, vertexCount, instanceCount, scX, scY, scW, scH));
  }

  /**
   * インデックスバッファ経由で 1 インスタンス描く。{@code indices} はストライド 4（u32）で
   * 確定した {@link GpuBuffer}（{@code gpu.buffer(n, 4)} で作るか、{@code array<u32>} として
   * カーネルへ bind 済みのもの）。頂点シェーダーの {@code SV_VertexID} には
   * インデックス値が入るので、vertex pulling がそのまま共有頂点の参照になる。
   *
   * <p>strip 系トポロジではインデックス {@code 0xFFFFFFFF} が帯の切れ目になる。
   */
  public void drawIndexed(GpuTexture target, GpuBuffer indices, int indexCount) {
    drawIndexedRange(target, indices, 0, indexCount, 1);
  }

  /**
   * インデックスバッファの {@code firstIndex} 要素目から {@code indexCount} 個で描く。
   * 1 本のバッファに複数の描画範囲を詰めて切り替えながら描くときに使う。
   */
  public void drawIndexed(GpuTexture target, GpuBuffer indices, int firstIndex, int indexCount) {
    if (firstIndex < 0) {
      throw new IllegalArgumentException("firstIndex は 0 以上が必要です: " + firstIndex);
    }
    drawIndexedRange(target, indices, firstIndex, indexCount, 1);
  }

  private void drawIndexedRange(
      GpuTexture target, GpuBuffer indices, int firstIndex, int indexCount, int instanceCount) {
    target.checkAlive();
    indices.checkAlive();
    GpuBridge.okState(
        GpuBridge.nDrawIndexed(
            id, target.id, indices.id, firstIndex, indexCount, instanceCount, scX, scY, scW, scH));
  }

  /**
   * 描画引数を GPU バッファから読んで drawIndexed する（GPU が自分で描画量を
   * 決めるパターン）。{@code args} の先頭から u32×5
   * （indexCount, instanceCount, firstIndex, baseVertex, firstInstance）を使う。
   * {@code indices} の要件は {@link #drawIndexed} と同じ（ストライド 4 の u32 バッファ）。
   *
   * <p>firstInstance は 0 にしておくこと（バックエンドによっては非対応）。
   */
  public void drawIndexedIndirect(GpuTexture target, GpuBuffer indices, GpuBuffer args) {
    drawIndexedIndirect(target, indices, args, 0);
  }

  /** {@link #drawIndexedIndirect(GpuTexture, GpuBuffer, GpuBuffer)} のオフセット指定版。{@code byteOffset} は 4 の倍数。 */
  public void drawIndexedIndirect(
      GpuTexture target, GpuBuffer indices, GpuBuffer args, long byteOffset) {
    target.checkAlive();
    indices.checkAlive();
    args.checkAlive();
    GpuBridge.okState(
        GpuBridge.nDrawIndexedIndirect(
            id, target.id, indices.id, args.id, byteOffset, scX, scY, scW, scH));
  }
}
