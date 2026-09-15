package gpubridge;

/** テクスチャのピクセルフォーマット。WGSL 側の宣言と一致している必要がある。 */
public enum GpuFormat {
  /** rgba8unorm — 表示用。show() できるのはこれだけ */
  RGBA8(0),
  /** rgba32float — シミュレーション状態の保持用 */
  RGBA32F(1),
  /** r32float — スカラー場 */
  R32F(2);

  final int id;

  GpuFormat(int id) {
    this.id = id;
  }
}
