package gpubridge;

/**
 * vertex + fragment の 2 エントリポイントを指すレンダラー。{@link GpuKernel} の描画版。
 *
 * <p>頂点データは vertex buffer ではなく storage buffer で渡す（vertex pulling）。
 * 頂点シェーダーが {@code SV_VertexID} / {@code SV_InstanceID} を受け取り、
 * {@code set()} で結んだバッファを自分で添字アクセスして読む。
 * vertex シェーダーから読むバッファは読み取り専用
 * （Slang では {@code RWStructuredBuffer} でなく {@code StructuredBuffer}）。
 */
public final class GpuRenderer {

  final long id;
  private GpuTopology topology = GpuTopology.TRIANGLES;
  private GpuBlend blend = GpuBlend.NONE;
  private boolean depthTest = false;

  GpuRenderer(long id) {
    this.id = id;
  }

  /** 計測報告に出す名前。付けなければ render#id */
  public GpuRenderer label(String label) {
    GpuBridge.okState(GpuBridge.nSetLabel(id, label));
    return this;
  }

  /** プリミティブの種類。既定は {@link GpuTopology#TRIANGLES}。 */
  public GpuRenderer topology(GpuTopology t) {
    topology = t;
    push();
    return this;
  }

  /** ブレンドモード。既定は {@link GpuBlend#NONE}（上書き）。 */
  public GpuRenderer blend(GpuBlend b) {
    blend = b;
    push();
    return this;
  }

  /**
   * 深度テストの有無。既定は false。有効にすると深度バッファ（描画先ごとに
   * ライブラリが自動管理）で手前のものだけが残る。{@code gpu.clear()} は深度も消す。
   */
  public GpuRenderer depthTest(boolean on) {
    depthTest = on;
    push();
    return this;
  }

  private void push() {
    GpuBridge.okState(GpuBridge.nRendererConfig(id, topology.id, blend.id, depthTest));
  }

  /** 空のバインディングを作る。set() でシェーダーの変数名にリソースを結ぶ。 */
  public GpuRenderBinding binding() {
    return new GpuRenderBinding(GpuBridge.okId(GpuBridge.nRenderBindingCreate(id)));
  }
}
