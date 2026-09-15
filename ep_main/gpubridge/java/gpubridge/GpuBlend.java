package gpubridge;

/** レンダーパイプラインのブレンドモード。 */
public enum GpuBlend {
  /** 上書き（既定） */
  NONE(0),
  /** アルファ合成（src_alpha / one_minus_src_alpha） */
  ALPHA(1),
  /** 加算（one / one） */
  ADD(2);

  final int id;

  GpuBlend(int id) {
    this.id = id;
  }
}
