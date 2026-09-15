class Material {
  GpuRenderer renderer;
  boolean lit;   // shade() を使う（shadowParams / shadowLayers を結ぶ）
  int slot;      // 分割パイプラインのインデックス領域・間接描画引数の番号

  Material(GpuRenderer renderer, boolean lit, int slot) {
    this.renderer = renderer;
    this.lit = lit;
    this.slot = slot;
  }
}
