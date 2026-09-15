// captures/<日時>/ に、描画結果と、同じ場面を再現するための状態を書き出す。
// snapshot.json と prefs.json は参照レイトレーサ(reference/)の --snapshot / --prefs にそのまま渡せる

boolean captureRequested;
GpuKernel captureKernel;
GpuBinding captureBinding;
GpuTexture captureSource;
GpuBuffer capturePixels;

void capture() {
  File dir = captureDirectory();
  saveScreen(new File(dir, "screen.png"));
  saveJSONObject(prefsCurrentJson(), new File(dir, "prefs.json").getPath());
  saveJSONObject(snapshotJson(), new File(dir, "snapshot.json").getPath());
  saveStrings(new File(dir, "git.txt").getPath(), new String[] {
    "commit " + runGit("rev-parse", "HEAD").trim(), "", "git status --short:", runGit("status", "--short")
  });
  println("captured: " + dir + " (frame " + engine.sceneFrame + ")");
}

File captureDirectory() {
  String stamp = nf(year(), 4) + "-" + nf(month(), 2) + "-" + nf(day(), 2) + "_" + nf(hour(), 2) + nf(minute(), 2) + nf(second(), 2);
  File dir = new File(sketchPath("captures"), stamp);
  for (int n = 2; dir.exists(); n++) dir = new File(sketchPath("captures"), stamp + "_" + n);
  dir.mkdirs();
  return dir;
}

void saveScreen(File file) {
  GpuWindow gpu = engine.gpu;
  int w = sceneView.width(), h = sceneView.height();
  if (captureKernel == null) captureKernel = gpu.kernelSlang(loadStrings("capture_readback.slang"), "readback");
  if (captureSource != sceneView) {
    if (capturePixels != null) capturePixels.dispose();
    capturePixels = gpu.buffer(w * h, 4);
    captureBinding = captureKernel.binding().set("source", sceneView).set("pixels", capturePixels);
    captureSource = sceneView;
  }
  captureBinding.dispatch(w, h);
  float[] packed = new float[w * h];
  capturePixels.read(packed);

  PImage image = createImage(w, h, RGB);
  image.loadPixels();
  for (int i = 0; i < packed.length; i++) image.pixels[i] = 0xFF000000 | (int) packed[i];
  image.updatePixels();
  image.save(file.getPath());
}

// 参照レイトレーサが使うのは cameraPosition / Velocity / Rotation(観測者)、sceneTime、
// waveCubeStartTime、viewWidth / Height。残りは本体側で場面を戻すための記録
JSONObject snapshotJson() {
  JSONObject json = new JSONObject();
  json.setInt("sceneFrame", engine.sceneFrame);
  json.setFloat("sceneTime", engine.sceneTime);
  json.setBoolean("paused", engine.paused);
  json.setFloat("lightSpeedEffective", engine.lightSpeed);
  json.setJSONArray("cameraPosition", vec3Json(cameraObserver.position));
  json.setJSONArray("cameraVelocity", vec3Json(cameraObserver.velocity));
  json.setJSONArray("cameraRotation", quatJson(cameraObserver.rotation));
  json.setFloat("observerYaw", observerYaw);
  json.setFloat("observerPitch", observerPitch);
  json.setFloat("waveCubeStartTime", waveCubeStartTime);
  json.setInt("viewWidth", sceneView.width());
  json.setInt("viewHeight", sceneView.height());

  // 描画に使ったカメラ。自由カメラが off なら観測者と同じ
  JSONObject renderCamera = new JSONObject();
  renderCamera.setJSONArray("position", vec3Json(camera.position));
  renderCamera.setJSONArray("velocity", vec3Json(camera.velocity));
  renderCamera.setJSONArray("rotation", quatJson(camera.rotation));
  renderCamera.setFloat("yaw", cameraYaw);
  renderCamera.setFloat("pitch", cameraPitch);
  json.setJSONObject("renderCamera", renderCamera);

  JSONArray spheres = new JSONArray();
  for (ThrownObject o : thrownSpheres) {
    JSONObject s = new JSONObject();
    s.setInt("spawnFrame", o.renderObject.spawnFrame);
    s.setJSONArray("position", vec3Json(o.renderObject.position));
    s.setJSONArray("velocity", vec3Json(o.velocity));
    s.setJSONArray("scale", vec3Json(o.renderObject.scale));
    spheres.append(s);
  }
  json.setJSONArray("thrownSpheres", spheres);
  return json;
}

JSONArray vec3Json(PVector v) {
  JSONArray a = new JSONArray();
  a.append(v.x);
  a.append(v.y);
  a.append(v.z);
  return a;
}

JSONArray quatJson(Quaternion q) {
  JSONArray a = new JSONArray();
  a.append(q.x);
  a.append(q.y);
  a.append(q.z);
  a.append(q.w);
  return a;
}

String runGit(String... args) {
  List<String> command = new ArrayList<>();
  command.add("git");
  command.addAll(java.util.Arrays.asList(args));
  try {
    Process p = new ProcessBuilder(command).directory(new File(sketchPath())).redirectErrorStream(true).start();
    String out = new String(p.getInputStream().readAllBytes(), "UTF-8");
    p.waitFor();
    return out;
  } catch (Exception e) {
    return "git を実行できません: " + e;
  }
}
