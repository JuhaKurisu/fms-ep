import gpubridge.*;
import imguibridge.*;
import imgui.ImGui;

GpuWindow gpu;
ImGuiBridge uiLeft;
ImGuiBridge uiRight;
GpuTexture view;
float[] valueLeft = {0.5};
float[] valueRight = {0.5};

void setup() {
  gpu = new GpuWindow(1600, 800, "imgui split", false);
  int half = gpu.width() / 2;
  uiLeft = new ImGuiBridge(gpu).viewport(0, 0, half, gpu.height());
  uiRight = new ImGuiBridge(gpu).viewport(half, 0, half, gpu.height());
  view = gpu.texture(gpu.width(), gpu.height(), GpuFormat.RGBA8);
}

void draw() {
  uiLeft.newFrame(1f / 60);
  ImGui.begin("left");
  ImGui.text("independent context A"); // デフォルトフォントは ASCII のみ(日本語グリフなし)
  ImGui.sliderFloat("value", valueLeft, 0, 1);
  ImGui.end();

  uiRight.newFrame(1f / 60);
  ImGui.begin("right");
  ImGui.text("independent context B");
  ImGui.sliderFloat("value", valueRight, 0, 1);
  ImGui.end();

  gpu.clear(view, 0.15, 0.15, 0.2, 1);
  uiLeft.render(view);
  uiRight.render(view);
  gpu.show(view);
  gpu.submit();
  if (!gpu.isOpen()) {
    gpu.dispose();
    exit();
  }
}
