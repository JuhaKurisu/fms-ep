package gpubridge;

/** レンダーパイプラインのプリミティブトポロジ。 */
public enum GpuTopology {
  /** 3 頂点ごとに 1 三角形（既定） */
  TRIANGLES(0),
  /** 2 頂点ごとに 1 線分 */
  LINES(1),
  /** 1 頂点 1 点 */
  POINTS(2),
  /**
   * 帯状の三角形（最初の 3 頂点で 1 枚、以後 1 頂点ごとに 1 枚）。
   * drawIndexed でインデックス 0xFFFFFFFF を挟むと帯を切れる（primitive restart）。
   */
  TRIANGLE_STRIP(3),
  /** 折れ線（最初の 2 頂点で 1 本、以後 1 頂点ごとに 1 本）。restart は同上 */
  LINE_STRIP(4);

  final int id;

  GpuTopology(int id) {
    this.id = id;
  }
}
