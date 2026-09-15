imgui.type.ImInt presentDemoBuf = new imgui.type.ImInt();
imgui.type.ImBoolean presentFreeCameraBuf = new imgui.type.ImBoolean();
imgui.type.ImBoolean presentShowDevPanelBuf = new imgui.type.ImBoolean();

// プレゼンモード中に出す操作パネル。位置とサイズは imgui.ini に残る
void presentUI() {
  ImGui.begin("Present");

  String[] names = new String[present.demos.size()];
  for (int i = 0; i < names.length; i++) names[i] = present.demos.get(i).name();
  presentDemoBuf.set(present.demoIndex);
  if (ImGui.combo("Demo", presentDemoBuf, names)) present.select(presentDemoBuf.get(), 0);

  ImGui.separator();
  Step[] steps = present.demo().steps();
  for (int i = 0; i < steps.length; i++) {
    if (ImGui.radioButton((i + 1) + ". " + steps[i].label, present.stepIndex == i)) {
      present.select(present.demoIndex, i);
    }
  }
  present.rampSeconds.field();
  ImGui.text("c = " + lightSpeedLabel(present.currentLightSpeed()));

  ImGui.separator();
  if (ImGui.button("< Prev")) present.previous();
  ImGui.sameLine();
  if (ImGui.button(engine.paused ? "Play" : "Pause")) present.togglePlay();
  ImGui.sameLine();
  if (ImGui.button("Next >")) present.next();
  ImGui.sameLine();
  if (ImGui.button("Restart")) present.requestStart(true);

  ImGui.separator();
  float t = present.localTime();
  float duration = present.duration();
  boolean loops = present.demo().loops();
  ImGui.text(loops
    ? "t = " + nf(t, 0, 2) + " (period " + nf(duration, 0, 2) + ")"
    : "t = " + nf(t, 0, 2) + " / " + nf(duration, 0, 2));
  ImGui.progressBar(duration > 1e-6 ? (loops ? t % duration : t) / duration : 0);

  ImGui.separator();
  if (present.demo().ui()) present.requestStart();

  ImGui.separator();
  presentFreeCameraBuf.set(present.freeCamera);
  if (ImGui.checkbox("Free camera", presentFreeCameraBuf)) present.freeCamera = presentFreeCameraBuf.get();

  ImGui.separator();
  present.demo().cameraUI();
  if (ImGui.button("Use current view")) {
    present.demo().captureView();
    present.freeCamera = false;
  }

  presentShowDevPanelBuf.set(present.showDevPanel);
  if (ImGui.checkbox("Show dev panel", presentShowDevPanelBuf)) present.showDevPanel = presentShowDevPanelBuf.get();

  ImGui.separator();
  if (ImGui.button("Exit to dev mode")) present.leave();
  ImGui.sameLine();
  if (ImGui.button("Save")) prefsSave();

  ImGui.end();
}

// 開発モードの Menu に出すタブ。プレゼン中は Show dev panel 経由でも開けるので、
// そこでは「入る」ボタンではなく状態だけ出す
void presentTab() {
  if (!present.active()) {
    if (ImGui.button("Enter present mode")) present.enter();
  } else {
    ImGui.text("present mode active (demo " + present.demoIndex + ", step " + present.stepIndex + ")");
  }
}
