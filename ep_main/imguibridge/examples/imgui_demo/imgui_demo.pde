import gpubridge.*;
import imguibridge.*;
import imgui.ImGui;

GpuWindow gpu;
ImGuiBridge ui;
GpuTexture view;

void setup() {
  gpu = new GpuWindow(1280, 800, "imgui demo", true);
  ui = new ImGuiBridge(gpu);
  println("imgui version = " + ImGui.getVersion());
  view = gpu.texture(gpu.width(), gpu.height(), GpuFormat.RGBA8);
}

int prevMillis;

void draw() {
  int now = millis();
  ui.newFrame(prevMillis == 0 ? 1f / 60 : (now - prevMillis) / 1000f);
  prevMillis = now;

  // window resize
  if (view.width() != gpu.width() || view.height() != gpu.height()) {
    view.dispose();
    view = gpu.texture(gpu.width(), gpu.height(), GpuFormat.RGBA8);
  }

  ImGui.showDemoWindow();

  gpu.clear(view, 0.2, 0.25, 0.3, 1);
  ui.render(view);
  gpu.show(view);
  gpu.submit();
  if (!gpu.isOpen()) {
    gpu.dispose();
    exit();
  }
}
