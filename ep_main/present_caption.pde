// シーンの上に重ねる c の表示。Processing で文字を描いたオフスクリーンをテクスチャにして、
// ImGui の最前面 draw list で貼る。テクスチャは 1 枚を使い回すので登録も 1 回きり
final int CAPTION_W = 640;
final int CAPTION_H = 96;

PrefsFloat captionHeight = new PrefsFloat("PresentCaptionHeight", 0.06);   // view の高さに対する比
PrefsFloat captionMargin = new PrefsFloat("PresentCaptionMargin", 0.03);

PGraphics captionGraphics;
PFont captionFont;
GpuTexture captionTexture;
long captionTextureId;
String captionShown;

void drawPresentCaption(float c) {
  String text = "c = " + (c >= LIGHT_SPEED_INFINITE ? "∞" : nf(c, 0, 1)) + " m/s";

  if (captionTexture == null) {
    captionGraphics = createGraphics(CAPTION_W, CAPTION_H);
    captionFont = createFont("SansSerif", CAPTION_H * 0.62, true);
    // オフスクリーンはスケッチの pixelDensity を継ぐので、実際の画素数でテクスチャを作る
    captionGraphics.beginDraw();
    captionGraphics.endDraw();
    captionTexture = engine.gpu.texture(captionGraphics.pixelWidth, captionGraphics.pixelHeight, GpuFormat.RGBA8);
    captionTextureId = ui.registerTexture(captionTexture);
  }

  if (!text.equals(captionShown)) {
    captionShown = text;
    float offset = CAPTION_H * 0.04;
    PGraphics pg = captionGraphics;
    pg.beginDraw();
    pg.clear();
    pg.textFont(captionFont);
    pg.textAlign(LEFT, BASELINE);
    // 明るい背景でも読めるように影を先に敷く
    pg.fill(0, 160);
    pg.text(text, CAPTION_H * 0.1 + offset, CAPTION_H * 0.78 + offset);
    pg.fill(255);
    pg.text(text, CAPTION_H * 0.1, CAPTION_H * 0.78);
    pg.endDraw();
    pg.loadPixels();
    captionTexture.write(pg.pixels);
  }

  float h = view.height() * captionHeight.get();
  float w = h * CAPTION_W / CAPTION_H;
  float m = view.height() * captionMargin.get();
  ImGui.getForegroundDrawList().addImage(captionTextureId, m, m, m + w, m + h);
}
