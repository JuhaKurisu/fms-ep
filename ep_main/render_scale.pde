// シーンだけ 1/N 解像度で描き、表示のときに画面いっぱいへ引き伸ばす。ImGui は
// 引き伸ばしたあとの view に等倍で重ねるので、UI はぼけない

GpuTexture sceneView;        // シーンの描画先。等倍のときは view そのもの
GpuTexture sceneViewOwned;   // 縮小のために自前で確保したもの(等倍のときは null)
GpuRenderer upscaleRenderer;
GpuRenderBinding upscaleBinding;
GpuTexture upscaleSource;

// view のサイズか縮小率が変わったら sceneView を作り直す
void updateSceneView() {
  int divisor = max(renderScaleDivisor.get(), 1);
  if (divisor == 1) {
    if (sceneViewOwned != null) {
      sceneViewOwned.dispose();
      sceneViewOwned = null;
    }
    sceneView = view;
    return;
  }
  int w = max(view.width() / divisor, 1);
  int h = max(view.height() / divisor, 1);
  if (sceneViewOwned == null || sceneViewOwned.width() != w || sceneViewOwned.height() != h) {
    if (sceneViewOwned != null) sceneViewOwned.dispose();
    sceneViewOwned = engine.gpu.texture(w, h, GpuFormat.RGBA8);
  }
  sceneView = sceneViewOwned;
}

void drawSceneViewToView() {
  if (sceneView == view) return;
  if (upscaleRenderer == null) {
    upscaleRenderer = engine.gpu.rendererSlang(loadStrings("upscale.slang"), "vertexMain", "fragmentMain")
      .label("upscale.slang").depthTest(false);
  }
  if (upscaleSource != sceneView) {
    upscaleBinding = upscaleRenderer.binding().set("source", sceneView);
    upscaleSource = sceneView;
  }
  upscaleBinding.draw(view, 3);
}
