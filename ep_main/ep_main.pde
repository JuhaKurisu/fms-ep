import java.util.List;
import java.util.ArrayList;
import gpubridge.*;
import imguibridge.*;
import imgui.ImGui;

public void showSurface() {
}

// env
PrefsFloat lightSpeed = new PrefsFloat("LightSpeed", 8.0);
PrefsBool lightSpeedInfinite = new PrefsBool("LightSpeedInfinite", false);

// light / shadow
PrefsVec3 lightDirection = new PrefsVec3("LightDirection", new PVector(0.4, 0.8, -0.6)); // 光源へ向かう向き
PrefsBool shadowEnabled = new PrefsBool("ShadowEnabled", true);
PrefsBool shadowBoundsVisible = new PrefsBool("ShadowBoundsVisible", false); // 焼く範囲の箱をワイヤで表示
PrefsVec3 shadowCenter = new PrefsVec3("ShadowCenter", new PVector(0, 0, 10));
PrefsFloat shadowExtent = new PrefsFloat("ShadowExtent", 24);
PrefsFloat shadowDepth = new PrefsFloat("ShadowDepth", 32);
PrefsInt shadowResolution = new PrefsInt("ShadowResolution", 512); // 再起動で反映
PrefsInt shadowLayers = new PrefsInt("ShadowLayers", 512);         // 再起動で反映
PrefsFloat shadowBias = new PrefsFloat("ShadowBias", 0.15);
PrefsFloat shadowAmbient = new PrefsFloat("ShadowAmbient", 0.2);
PrefsFloat lightIntensity = new PrefsFloat("LightIntensity", 1.0);
PrefsBool pbrEnabled = new PrefsBool("PbrEnabled", true); // off なら拡散のみ
PrefsBool iblEnabled = new PrefsBool("IblEnabled", true);          // 環境マップ照明と背景
PrefsFloat iblIntensity = new PrefsFloat("IblIntensity", 1.0);
PrefsBool tonemapEnabled = new PrefsBool("TonemapEnabled", true);  // 3D LUT(sRGB 込み)。off なら線形のまま
PrefsInt tonemapLut = new PrefsInt("TonemapLut", 0);               // data/luts/ のファイル名順
PrefsFloat exposure = new PrefsFloat("Exposure", 1.0);
PrefsFloat planeRoughness = new PrefsFloat("PlaneRoughness", 0.5);
PrefsFloat planeMetallic = new PrefsFloat("PlaneMetallic", 0.0);
PrefsFloat shadowSoftRadius = new PrefsFloat("ShadowSoftRadius", 2);   // テクセル単位。0 でハード
PrefsInt shadowSoftSamples = new PrefsInt("ShadowSoftSamples", 5);     // 1 軸あたり

// cube
PrefsFloat cubeAnimationWidth = new PrefsFloat("CubeAnimationWidth", 6.0);
PrefsVec3 cubeOrigin = new PrefsVec3("CubeOrigin", new PVector(0, 0, 15));
PrefsBool cubeVisible = new PrefsBool("CubeVisible", true);
PrefsFloat cubeRoughness = new PrefsFloat("CubeRoughness", 0.5);
PrefsFloat cubeMetallic = new PrefsFloat("CubeMetallic", 0.0);

// long cuboid
PrefsVec3 longCubeOrigin = new PrefsVec3("LongCubeOrigin", new PVector(-4, 0, 15));
PrefsVec3 longCubeSize = new PrefsVec3("LongCubeSize", new PVector(1, 1, 10));
PrefsInt longCubeDivisions = new PrefsInt("LongCubeDivisions", 32);
PrefsFloat longCubeAnimationWidth = new PrefsFloat("LongCubeAnimationWidth", 1);
PrefsFloat longCubeAnimationDuration = new PrefsFloat("LongCubeAnimationDuration", 0.5);
PrefsBool longCubeVisible = new PrefsBool("LongCubeVisible", true);
PrefsFloat longCubeRoughness = new PrefsFloat("LongCubeRoughness", 0.5);
PrefsFloat longCubeMetallic = new PrefsFloat("LongCubeMetallic", 0.0);

// camera
PrefsFloat cameraFOV = new PrefsFloat("CameraFOV", 60);
PrefsFloat cameraSpeed = new PrefsFloat("CameraSpeed", 5.0);
PrefsFloat cameraAccelTime = new PrefsFloat("CameraAccelTime", 0.1);
PrefsFloat cameraRotateSpeed = new PrefsFloat("CameraRotateSpeed", 90);
PrefsBool cameraVelocityZero = new PrefsBool("CameraVelocityZero", false);

// throw
PrefsFloat throwPower = new PrefsFloat("ThrowPower", 5);
PrefsFloat throwObjectGravity = new PrefsFloat("ThrowObjectGravity", 9.8);
PrefsFloat throwObjectScale = new PrefsFloat("ThrowObjectScale", 1.0);
PrefsFloat throwObjectRoughness = new PrefsFloat("ThrowObjectRoughness", 0.5);
PrefsFloat throwObjectMetallic = new PrefsFloat("ThrowObjectMetallic", 0.0);

// wheel
PrefsInt wheelSpokeCount= new PrefsInt("WheelSpokeCount", 8);
PrefsFloat wheelSpokeSize = new PrefsFloat("WheelSpokeSize", 0.1);
PrefsFloat wheelSpokeThickness = new PrefsFloat("WheelSpokeThickness", 0.1);
PrefsInt wheelDivisions = new PrefsInt("WheelDivisions", 32);
PrefsInt wheelSubDivisions = new PrefsInt("WheelSubDivisions", 16);
PrefsFloat wheelRadius = new PrefsFloat("WheelRadius", 2);
PrefsFloat wheelRotateSpeed = new PrefsFloat("WheelRotateSpeed", 120);
PrefsFloat wheelMoveDistance = new PrefsFloat("WheelMoveDistance", 4);
PrefsFloat wheelThickness = new PrefsFloat("WheelThickness", 0.1);
PrefsFloat wheelRadialThickness = new PrefsFloat("WheelRadialThickness", 0.3);
PrefsFloat wheelHubRadius = new PrefsFloat("WheelHubRadius", 0.3);
PrefsFloat wheelHubThickness = new PrefsFloat("WheelHubThickness", 0.1);
PrefsVec3 wheelOrigin = new PrefsVec3("WheelOrigin", new PVector(6, -0.5, 15));
PrefsBool wheelVisible = new PrefsBool("WheelVisible", true);
PrefsFloat wheelRoughness = new PrefsFloat("WheelRoughness", 0.5);
PrefsFloat wheelMetallic = new PrefsFloat("WheelMetallic", 0.0);

// wave cube
PrefsVec3 waveCubeOrigin = new PrefsVec3("WaveCubeOrigin", new PVector(0, 0, 0));
PrefsInt waveCubeXCount = new PrefsInt("WaveCubeXCount", 5);
PrefsInt waveCubeYCount = new PrefsInt("WaveCubeYCount", 5);
PrefsInt waveCubeZCount = new PrefsInt("WaveCubeZCount", 5);
PrefsVec3 waveCubeSpace = new PrefsVec3("WaveCubeSpace", new PVector(1, 1, 1));
PrefsVec3 waveCubeSize = new PrefsVec3("WaveCubeSize", new PVector(1, 1, 1));
PrefsFloat waveCubeSpeed = new PrefsFloat("WaveCubeSpeed", 1);
PrefsFloat waveCubeWidth = new PrefsFloat("WaveCubeWidth", 1);
PrefsInt waveCubeDivisions = new PrefsInt("WaveCubeDivisions", 8);
PrefsBool waveCubeVisible = new PrefsBool("WaveCubeVisible", true);
PrefsFloat waveCubeRoughness = new PrefsFloat("WaveCubeRoughness", 0.5);
PrefsFloat waveCubeMetallic = new PrefsFloat("WaveCubeMetallic", 0.0);
float waveCubeStartTime = -1e5;

// debug
PrefsBool debugFreeCamera = new PrefsBool("DebugFreeCamera", false);
PrefsFloat debugFreeCameraMaxSpeed = new PrefsFloat("DebugFreeCameraMaxSpeed", 20.0);
PrefsBool debugWireframe = new PrefsBool("DebugWireframe", false);
PrefsBool debugFrustumVisible = new PrefsBool("DebugFrustumVisible", false); // 観測者の視錐台をワイヤで表示
PrefsFloat debugFrustumFar = new PrefsFloat("DebugFrustumFar", 20);          // 表示する奥行き
PrefsBool debugGpuProfile = new PrefsBool("DebugGpuProfile", false);   // パスごとの GPU 時間を測る(計測中は少し重い)
PrefsInt renderScaleDivisor = new PrefsInt("RenderScaleDivisor", 1);   // シーンを 1/N 解像度で描いて引き伸ばす(UI は等倍のまま)

// subdivision
PrefsFloat subdivisionErrorPx = new PrefsFloat("SubdivisionErrorPx", 1);      // 分割点の真の位置と直線補間との画面上のずれがこれを超えたら割る
PrefsFloat subdivisionMaxEdgePx = new PrefsFloat("SubdivisionMaxEdgePx", 64); // 辺がこのピクセル数を超えたら誤差に関係なく割る
PrefsInt subdivisionMaxDepth = new PrefsInt("SubdivisionMaxDepth", 12);       // 0 で分割なし
PrefsBool subdivisionAngleBisect = new PrefsBool("SubdivisionAngleBisect", true); // 辺を観測者から見た角度の二等分点で割る(off なら局所座標の中点)
PrefsBool subdivisionFrustumCull = new PrefsBool("SubdivisionFrustumCull", true); // 観測者の視野の外に出た辺は割らない
PrefsInt subdivisionObjectCap = new PrefsInt("SubdivisionObjectCap", 256);           // 以下は再起動で反映
PrefsInt subdivisionQueueCap = new PrefsInt("SubdivisionQueueCap", 1 << 20);         // 1 深さぶんの分割途中の三角形数
PrefsInt subdivisionFinalCap = new PrefsInt("SubdivisionFinalCap", 1 << 21);         // 確定三角形数
PrefsInt subdivisionVertexCap = new PrefsInt("SubdivisionVertexCap", 3 << 21);       // 焼く頂点数(確定三角形 × 3)
PrefsInt subdivisionIndexCap = new PrefsInt("SubdivisionIndexCap", 3 << 21);         // マテリアル 1 つあたりのインデックス数
PrefsInt subdivisionLineCap = new PrefsInt("SubdivisionLineCap", 6 << 21);           // ワイヤーフレーム用の線のインデックス数

Engine engine;
Camera camera;
GpuTexture view;
ImGuiBridge ui;

RenderObject movingCuboid;
RenderObject longCube;
boolean longCubeDirty;
RenderObject plane;
RenderObject shadowBounds;
RenderObject shadowPlaneSection;
RenderObject frustumBounds;
final int SHADOW_SECTION_MAX_SIDES = 6;
final int SHADOW_WIRE_DIVISIONS = 16;
RenderObject wheel;
boolean wheelDirty;
imgui.type.ImFloat speedRatioBuf = new imgui.type.ImFloat();

Observer cameraObserver = new Observer(new PVector(), new PVector(), quatIdentity());

float cameraYaw = 0;
float cameraPitch = 0;
float observerYaw = 0;
float observerPitch = 0;

List<ThrownObject> thrownSpheres = new ArrayList<>();
float[] sphereVertices = makeSphereVertices(16);
int[] sphereIndices = makeSphereIndices(16);
List<RenderObject> wavingCubes = new ArrayList<>();
boolean waveCubeDirty;

WaveCubesDemo waveCubesDemo = new WaveCubesDemo();
RollingWheelDemo rollingWheelDemo = new RollingWheelDemo();
TerrellRotationDemo terrellRotationDemo = new TerrellRotationDemo();
ChasingCubeDemo chasingCubeDemo = new ChasingCubeDemo();
SwingingRodDemo swingingRodDemo = new SwingingRodDemo();
TreeRoadDemo treeRoadDemo = new TreeRoadDemo();
PresentRunner present;

List<RenderObject> objects = new ArrayList<>();

void setup() {
  prefsLoad();

  engine = new Engine(2048, 2048, "Tompkins", lightSpeed.get(), shadowResolution.get(), shadowLayers.get());
  engine.pixelCoverageSubdivision = new PixelCoverageSubdivision(engine, subdivisionObjectCap.get(), subdivisionQueueCap.get(),
    subdivisionFinalCap.get(), subdivisionVertexCap.get(), subdivisionIndexCap.get(), subdivisionLineCap.get());
  ui = new ImGuiBridge(engine.gpu);
  ImGui.getIO().setIniFilename(sketchPath("imgui.ini")); // カレントディレクトリ依存を避けてスケッチフォルダに固定
  camera = new Camera();
  view = engine.gpu.texture(engine.gpu.width(), engine.gpu.height(), GpuFormat.RGBA8);
  updateSceneView();

  float[] cuboidVertices = makeCubeVertices(1);
  int[] cuboidIndices = makeCubeIndices(1);
  movingCuboid = engine.createObject(cuboidVertices, cuboidIndices, cubeOrigin.get());
  movingCuboid.scale.y = 5;

  rebuildLongCube();

  float[] planeVertices = makePlaneVertices(1);
  int[] planeIndices = makePlaneIndices(1);
  Material planeMaterial = engine.createMaterial("plane.slang");
  plane = engine.createObject(planeVertices, planeIndices, new PVector(0, -2.5, 0), planeMaterial);
  plane.scale = new PVector(100, 1, 100);

  shadowBounds = engine.createObject(makeBoxWireVertices(SHADOW_WIRE_DIVISIONS), new int[0], makeBoxWireEdges(SHADOW_WIRE_DIVISIONS), shadowCenter.get(), engine.wireframeMaterial);
  shadowBounds.wireframeOnly = true;
  shadowBounds.shaderParams.set("wireColor", 1, 0.5, 0);
  shadowPlaneSection = engine.createObject(
    makePolygonWireVertices(new ArrayList<>(), SHADOW_SECTION_MAX_SIDES, SHADOW_WIRE_DIVISIONS), new int[0],
    makePolygonWireEdges(SHADOW_SECTION_MAX_SIDES, SHADOW_WIRE_DIVISIONS), new PVector(), engine.wireframeMaterial);
  shadowPlaneSection.wireframeOnly = true;
  shadowPlaneSection.shaderParams.set("wireColor", 1, 0.5, 0);

  // 頂点は毎フレーム世界座標で書き換えるので、ここでは大きさだけ合わせておく
  frustumBounds = engine.createObject(makeBoxWireVertices(SHADOW_WIRE_DIVISIONS), new int[0],
    makeBoxWireEdges(SHADOW_WIRE_DIVISIONS), new PVector(), engine.wireframeMaterial);
  frustumBounds.wireframeOnly = true;
  frustumBounds.shaderParams.set("wireColor", 0.3, 0.8, 1);

  rebuildWheel();
  rebuildWaveCubes();
  present = new PresentRunner();
}

void rebuildLongCube() {
  if (longCube != null) longCube.dispose();
  int divisions = max(1, longCubeDivisions.get());
  longCube = engine.createObject(makeCubeVertices(divisions), makeCubeIndices(divisions), longCubeOrigin.get());
}

void rebuildWheel() {
  if (wheel != null) wheel.dispose();
  int divisions = max(1, wheelDivisions.get());
  int subDivisions = max(1, wheelSubDivisions.get());
  float[] wheelVertices = makeWheelVertices(divisions, subDivisions, wheelRadius.get(), wheelRadialThickness.get(), wheelThickness.get(), wheelSpokeCount.get(), wheelSpokeSize.get(), wheelSpokeThickness.get(), wheelHubRadius.get(), wheelHubThickness.get());
  int[] wheelIndices = makeWheelIndices(divisions, subDivisions, wheelSpokeCount.get());
  wheel = engine.createObject(wheelVertices, wheelIndices, wheelOrigin.get());
}

// origin を先頭キューブとして space 間隔で XCount×YCount×ZCount 個並べる
void rebuildWaveCubes() {
  for (RenderObject o : wavingCubes) o.dispose();
  wavingCubes.clear();
  int divisions = max(1, waveCubeDivisions.get());
  float[] vertices = makeCubeVertices(divisions);
  int[] indices = makeCubeIndices(divisions);
  for (int i = 0; i < max(0, waveCubeXCount.get()); i++) {
    for (int j = 0; j < max(0, waveCubeYCount.get()); j++) {
      for (int k = 0; k < max(0, waveCubeZCount.get()); k++) {
        PVector position = PVector.add(waveCubeOrigin.get(), new PVector(i * waveCubeSpace.get().x, j * waveCubeSpace.get().y, k * waveCubeSpace.get().z));
        wavingCubes.add(engine.createObject(vertices, indices, position));
      }
    }
  }
}

// Space で 1 回だけ y 方向に持ち上がって戻る。全キューブが同時刻に動くので、
// 光の遅延で遠いものほど遅れて見え、波のように繋がる
float waveCubeOffset() {
  float t = engine.sceneTime - waveCubeStartTime;
  float duration = PI * abs(waveCubeWidth.get()) / max(abs(waveCubeSpeed.get()), 1e-4);
  if (t < 0 || t > duration) return 0;
  return waveCubeWidth.get() * 0.5 * (1 - cos(TWO_PI * t / duration));
}

void draw() {
  GpuWindow gpu = engine.gpu;

  ui.newFrame(engine.KEYFRAME_INTERVAL);

  // window resize
  if (view.width() != gpu.width() || view.height() != gpu.height()) {
    view.dispose();
    view = gpu.texture(gpu.width(), gpu.height(), GpuFormat.RGBA8);
  }
  updateSceneView();

  if (!ui.wantTextInput()) {
    if (gpu.keyPressed('p')) engine.paused = !engine.paused;
    if (gpu.keyPressed('.')) {
      engine.paused = true;
      engine.stepOnce = true;
    }
    if (gpu.keyPressed('x')) captureRequested = true;
    // 発表中は Enter だけで次へ送れるようにする。矢印キーは見回しのまま
    if (present.active() && gpu.keyPressed(ENTER)) present.next();
  }
  engine.nextFrame();

  update();

  // debug
  if (present.active()) {
    presentUI();
    drawPresentCaption(present.currentLightSpeed());
    if (present.showDevPanel) debugUI();
  } else {
    debugUI();
  }

  if (!debugFreeCamera.get() || present.active()) {
    cameraObserver.position.set(camera.position);
    cameraObserver.velocity.set(camera.velocity);
    cameraObserver.rotation.set(camera.rotation);
    observerYaw = cameraYaw;
    observerPitch = cameraPitch;
  }
  // 自由視点は見回すのが目的なので光行差を載せない。逆に、走るデモは光行差が
  // 見せどころなので、速度を消す設定のほうを無視する
  boolean keepVelocity = present.active() && present.demo().movesCamera() && !present.freeCamera;
  boolean zeroVelocity = (present.active() && present.freeCamera) || (cameraVelocityZero.get() && !keepVelocity);
  Observer renderObserver = zeroVelocity
    ? new Observer(cameraObserver.position, new PVector(), cameraObserver.rotation)
    : cameraObserver;
  engine.gpu.profile(debugGpuProfile.get());
  engine.render(sceneView, camera, renderObserver, objects, debugWireframe.get());
  if (debugGpuProfile.get() && engine.sceneFrame % 120 == 0) println("gpu " + sceneView.width() + "x" + sceneView.height() + " " + profileSummary().replace("\n", " | "));
  // ImGui を重ねる前に撮る
  if (captureRequested) {
    captureRequested = false;
    capture();
  }

  drawSceneViewToView();
  ui.render(view);

  gpu.show(view);
  gpu.submit();

  if (!gpu.isOpen()) {
    gpu.dispose();
    exit();
  }
}

void update() {
  GpuWindow gpu = engine.gpu;
  objects.clear();
  present.applyRequests();

  if (present.active()) {
    // プレゼン中は影を出さない
    engine.shadow.configure(false, lightDirection.get(), shadowCenter.get(), shadowExtent.get(), shadowDepth.get(), shadowBias.get(), shadowAmbient.get(), shadowSoftRadius.get(), shadowSoftSamples.get());
    engine.tonemap.select(tonemapLut.get());
    engine.shadow.configureLighting(lightIntensity.get(), pbrEnabled.get(), iblEnabled.get(), iblIntensity.get(), tonemapEnabled.get(), exposure.get(), engine.tonemap.size);
    camera.fov = radians(cameraFOV.get());
    engine.pixelCoverageSubdivision.errorPx = subdivisionErrorPx.get();
    engine.pixelCoverageSubdivision.maxEdgePx = subdivisionMaxEdgePx.get();
    engine.pixelCoverageSubdivision.maxDepth = constrain(subdivisionMaxDepth.get(), 0, engine.pixelCoverageSubdivision.MAX_DEPTH);
    engine.pixelCoverageSubdivision.splitAngleBisect = subdivisionAngleBisect.get();
    engine.pixelCoverageSubdivision.frustumCull = subdivisionFrustumCull.get();
    present.update(objects);
    return;
  }

  engine.lightSpeed = lightSpeedInfinite.get() ? 1e6 : lightSpeed.get();
  engine.shadow.configure(shadowEnabled.get(), lightDirection.get(), shadowCenter.get(), shadowExtent.get(), shadowDepth.get(), shadowBias.get(), shadowAmbient.get(), shadowSoftRadius.get(), shadowSoftSamples.get());
  engine.tonemap.select(tonemapLut.get());
  engine.shadow.configureLighting(lightIntensity.get(), pbrEnabled.get(), iblEnabled.get(), iblIntensity.get(), tonemapEnabled.get(), exposure.get(), engine.tonemap.size);
  camera.fov = radians(cameraFOV.get());
  engine.pixelCoverageSubdivision.errorPx = subdivisionErrorPx.get();
  engine.pixelCoverageSubdivision.maxEdgePx = subdivisionMaxEdgePx.get();
  engine.pixelCoverageSubdivision.maxDepth = constrain(subdivisionMaxDepth.get(), 0, engine.pixelCoverageSubdivision.MAX_DEPTH);
  engine.pixelCoverageSubdivision.splitAngleBisect = subdivisionAngleBisect.get();
  engine.pixelCoverageSubdivision.frustumCull = subdivisionFrustumCull.get();

  // shadow が焼く範囲: 光源基底で center を中心に 2*halfExtent × 2*halfExtent × depthRange の箱
  ShadowMap shadow = engine.shadow;
  shadowBounds.position.set(shadow.center);
  shadowBounds.scale.set(2 * shadow.halfExtent, 2 * shadow.halfExtent, shadow.depthRange);
  shadowBounds.rotation = quatFromBasis(shadow.right, shadow.up, shadow.forward);
  if (shadowBoundsVisible.get()) {
    objects.add(shadowBounds);
    // 箱を plane で切った断面。頂点は世界座標で直接書く（transform は恒等）
    PVector[] corners = boxCorners(shadowBounds.position, shadowBounds.scale, shadowBounds.rotation);
    PVector planeNormal = quatRotate(plane.rotation, new PVector(0, 1, 0));
    PVector planePoint = PVector.add(plane.position, PVector.mult(planeNormal, 0.02)); // 床と重ならないよう少し浮かせる
    List<PVector> section = boxPlaneSection(corners, planePoint, planeNormal);
    shadowPlaneSection.verticesBuffer.write(makePolygonWireVertices(section, SHADOW_SECTION_MAX_SIDES, SHADOW_WIRE_DIVISIONS));
    objects.add(shadowPlaneSection);
  }

  // 観測者の視錐台。頂点は世界座標で直接書く(transform は恒等)
  if (debugFrustumVisible.get()) {
    PVector[] corners = frustumCorners(cameraObserver.position, cameraObserver.rotation,
      camera.fov, (float) sceneView.width() / sceneView.height(), 0.1, debugFrustumFar.get());
    frustumBounds.verticesBuffer.write(makeCornerWireVertices(corners, SHADOW_WIRE_DIVISIONS));
    objects.add(frustumBounds);
  }

  PVector moveInput = ui.wantTextInput()
    ? new PVector()
    : new PVector(gpu.axis('a', 'd'), gpu.axis('q', 'e'), gpu.axis('s', 'w'));
  PVector rotateInput = ui.wantTextInput()
    ? new PVector()
    : new PVector(gpu.axis(LEFT, RIGHT), gpu.axis(UP, DOWN));
  rotateCamera(engine.KEYFRAME_INTERVAL, rotateInput);
  moveCamera(engine.KEYFRAME_INTERVAL, moveInput);

  movingCuboid.position.set(cubeOrigin.get());
  movingCuboid.position.x += sin(engine.sceneTime) * cubeAnimationWidth.get();
  movingCuboid.roughness = cubeRoughness.get();
  movingCuboid.metallic = cubeMetallic.get();
  if (cubeVisible.get()) objects.add(movingCuboid);
  plane.roughness = planeRoughness.get();
  plane.metallic = planeMetallic.get();
  objects.add(plane);

  if (longCubeDirty) {
    longCubeDirty = false;
    rebuildLongCube();
  }
  // 始点 origin から +z（カメラの視線方向）に size.z だけ伸びる。メッシュは中心原点なので半分ずらす
  float longCubePeriod = max(abs(longCubeAnimationDuration.get()), 1e-4);
  longCube.scale.set(longCubeSize.get());
  longCube.position.set(longCubeOrigin.get());
  longCube.position.z += longCubeSize.get().z * 0.5;
  longCube.position.x += sin(TWO_PI * engine.sceneTime / longCubePeriod) * longCubeAnimationWidth.get();
  longCube.roughness = longCubeRoughness.get();
  longCube.metallic = longCubeMetallic.get();
  if (longCubeVisible.get()) objects.add(longCube);

  if (wheelDirty) {
    wheelDirty = false;
    rebuildWheel();
  }
  float wheelHalfRange = max(abs(wheelMoveDistance.get()), 1e-4);
  float wheelPeriod = 4 * wheelHalfRange;
  float wheelTravel = radians(wheelRotateSpeed.get()) * wheelRadius.get() * engine.sceneTime;
  float wheelPhase = ((wheelTravel % wheelPeriod) + wheelPeriod) % wheelPeriod;
  float wheelOffset = wheelPhase < wheelHalfRange ? wheelPhase
    : wheelPhase < 3 * wheelHalfRange ? 2 * wheelHalfRange - wheelPhase
    : wheelPhase - wheelPeriod;
  wheel.rotation = quatFromAxisAngle(new PVector(0, 0, 1), -wheelOffset / wheelRadius.get());
  wheel.position.set(wheelOrigin.get());
  wheel.position.x += wheelOffset;
  wheel.roughness = wheelRoughness.get();
  wheel.metallic = wheelMetallic.get();
  if (wheelVisible.get()) objects.add(wheel);

  if (waveCubeDirty) {
    waveCubeDirty = false;
    rebuildWaveCubes();
  }
  if (!ui.wantTextInput() && engine.gpu.keyPressed(' ')) {
    waveCubeStartTime = engine.sceneTime;
  }
  float waveOffset = waveCubeOffset();
  int waveIndex = 0;
  for (int i = 0; i < max(0, waveCubeXCount.get()); i++) {
    for (int j = 0; j < max(0, waveCubeYCount.get()); j++) {
      for (int k = 0; k < max(0, waveCubeZCount.get()); k++) {
        RenderObject cube = wavingCubes.get(waveIndex++);
        cube.scale.set(waveCubeSize.get());
        cube.position.set(waveCubeOrigin.get());
        cube.position.add(i * waveCubeSpace.get().x, j * waveCubeSpace.get().y + waveOffset, k * waveCubeSpace.get().z);
        cube.roughness = waveCubeRoughness.get();
        cube.metallic = waveCubeMetallic.get();
        if (waveCubeVisible.get()) objects.add(cube);
      }
    }
  }

  if (engine.gpu.keyPressed('t')) {
    thrownSpheres.add(new ThrownObject(camera.position, quatRotate(camera.rotation, new PVector(0, 0, 1)).mult(throwPower.get())));
  }

  for (ThrownObject o : thrownSpheres) {
    if (engine.advanced) o.update();
    o.renderObject.roughness = throwObjectRoughness.get();
    o.renderObject.metallic = throwObjectMetallic.get();
    objects.add(o.renderObject);
  }
}

void rotateCamera(float dt, PVector rotateInput) {
  float step = radians(cameraRotateSpeed.get()) * dt;
  cameraYaw += rotateInput.x * step;
  cameraPitch = constrain(cameraPitch + rotateInput.y * step, radians(-89), radians(89));
  camera.rotation = quatMul(
    quatFromAxisAngle(new PVector(0, 1, 0), cameraYaw),
    quatFromAxisAngle(new PVector(1, 0, 0), cameraPitch));
}

void moveCamera(float dt, PVector moveInput) {
  PVector worldInput = quatRotate(camera.rotation, moveInput.normalize());
  float speed = debugFreeCamera.get() && engine.gpu.keyDown(SHIFT) ? debugFreeCameraMaxSpeed.get() : cameraSpeed.get();
  PVector targetVelocity = PVector.mult(worldInput, speed);
  camera.velocity.lerp(targetVelocity, 1.0 - exp(-dt / cameraAccelTime.get()));
  camera.position.add(PVector.mult(camera.velocity, dt));
}

void lightSpeedRatioField(String label, PrefsFloat speed, float fullSpeed) {
  speedRatioBuf.set(speed.get() / fullSpeed);
  ImGui.alignTextToFramePadding();
  ImGui.text(label);
  ImGui.sameLine(prefsLabelWidth());
  ImGui.setNextItemWidth(-1);
  if (ImGui.inputFloat("##" + label, speedRatioBuf)) {
    speed.set(speedRatioBuf.get() * fullSpeed);
  }
}

// 最高速度 = Width * 2π / Duration。比率の入力は Duration に反映する
void longCubeMaxSpeedRatioField() {
  String label = "LongCubeMaxSpeed/c";
  float peak = TWO_PI * longCubeAnimationWidth.get() / max(abs(longCubeAnimationDuration.get()), 1e-4);
  speedRatioBuf.set(peak / lightSpeed.get());
  ImGui.alignTextToFramePadding();
  ImGui.text(label);
  ImGui.sameLine(prefsLabelWidth());
  ImGui.setNextItemWidth(-1);
  if (ImGui.inputFloat("##" + label, speedRatioBuf) && speedRatioBuf.get() != 0) {
    longCubeAnimationDuration.set(TWO_PI * longCubeAnimationWidth.get() / (speedRatioBuf.get() * lightSpeed.get()));
  }
}

String vectorText(PVector v) {
  return nf(v.x, 0, 2) + ", " + nf(v.y, 0, 2) + ", " + nf(v.z, 0, 2);
}

void cameraStateText() {
  ImGui.text("camera position: " + vectorText(camera.position));
  ImGui.text("camera yaw/pitch: " + nf(degrees(cameraYaw), 0, 1) + ", " + nf(degrees(cameraPitch), 0, 1));
  ImGui.text("camera velocity: " + vectorText(camera.velocity));
}

void debugUI() {
  ImGui.begin("Menu");
  if (ImGui.button("Save")) {
    prefsSave();
  }
  if (ImGui.beginTabBar("MainTab")) {
    if (ImGui.beginTabItem("Present")) {
      presentTab();
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("Environment")) {
      lightSpeed.field();
      lightSpeedInfinite.checkbox();
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("Lighting")) {
      lightDirection.field();
      lightIntensity.field();
      shadowAmbient.field();
      pbrEnabled.checkbox();
      iblEnabled.checkbox();
      iblIntensity.field();
      tonemapEnabled.checkbox();
      tonemapLut.combo(engine.tonemap.names);
      exposure.field();
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("Shadow")) {
      shadowEnabled.checkbox();
      shadowBoundsVisible.checkbox();
      shadowCenter.field();
      shadowExtent.field();
      shadowDepth.field();
      shadowBias.field();
      shadowSoftRadius.field();
      shadowSoftSamples.field();
      shadowResolution.field();
      shadowLayers.field();
      ImGui.text("layers in flight: " + engine.shadow.layersInFlight() + " / " + engine.shadow.layerCount);
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("Camera")) {
      cameraFOV.field();
      lightSpeedRatioField("CameraSpeed/c", cameraSpeed, lightSpeed.get());
      cameraSpeed.field();
      cameraAccelTime.field();
      cameraRotateSpeed.field();
      cameraVelocityZero.checkbox();
      cameraStateText();
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("AnimationCube")) {
      cubeVisible.checkbox();
      cubeOrigin.field();
      lightSpeedRatioField("CubeMaxSpeed/c", cubeAnimationWidth, lightSpeed.get());
      cubeAnimationWidth.field();
      cubeRoughness.slider(0, 1);
      cubeMetallic.slider(0, 1);
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("LongCube")) {
      longCubeVisible.checkbox();
      longCubeOrigin.field();
      longCubeSize.field();
      longCubeDirty |= longCubeDivisions.field();
      longCubeMaxSpeedRatioField();
      longCubeAnimationWidth.field();
      longCubeAnimationDuration.field();
      longCubeRoughness.slider(0, 1);
      longCubeMetallic.slider(0, 1);
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("ThrowObject")) {
      throwPower.field();
      throwObjectGravity.field();
      throwObjectScale.field();
      throwObjectRoughness.slider(0, 1);
      throwObjectMetallic.slider(0, 1);
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("Wheel")) {
      wheelVisible.checkbox();
      wheelOrigin.field();
      wheelDirty |= wheelSpokeCount.field();
      wheelDirty |= wheelSpokeSize.field();
      wheelDirty |= wheelSpokeThickness.field();
      wheelDirty |= wheelDivisions.field();
      wheelDirty |= wheelSubDivisions.field();
      wheelDirty |= wheelRadius.field();
      lightSpeedRatioField("WheelRimSpeed/c", wheelRotateSpeed, degrees(lightSpeed.get() / (2 * wheelRadius.get())));
      wheelRotateSpeed.field();
      wheelMoveDistance.field();
      wheelDirty |= wheelThickness.field();
      wheelDirty |= wheelRadialThickness.field();
      wheelDirty |= wheelHubRadius.field();
      wheelDirty |= wheelHubThickness.field();
      wheelRoughness.slider(0, 1);
      wheelMetallic.slider(0, 1);
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("WaveCube")) {
      waveCubeVisible.checkbox();
      if (ImGui.button("Wave (Space)")) {
        waveCubeStartTime = engine.sceneTime;
      }
      waveCubeOrigin.field();
      waveCubeDirty |= waveCubeXCount.field();
      waveCubeDirty |= waveCubeYCount.field();
      waveCubeDirty |= waveCubeZCount.field();
      waveCubeSpace.field();
      waveCubeSize.field();
      lightSpeedRatioField("WaveCubeSpeed/c", waveCubeSpeed, lightSpeed.get());
      waveCubeSpeed.field();
      waveCubeWidth.field();
      waveCubeDirty |= waveCubeDivisions.field();
      waveCubeRoughness.slider(0, 1);
      waveCubeMetallic.slider(0, 1);
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("Plane")) {
      planeRoughness.slider(0, 1);
      planeMetallic.slider(0, 1);
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("Subdivision")) {
      subdivisionErrorPx.field();
      subdivisionMaxEdgePx.field();
      subdivisionMaxDepth.field();
      subdivisionAngleBisect.checkbox();
      subdivisionFrustumCull.checkbox();
      subdivisionObjectCap.field();
      subdivisionQueueCap.field();
      subdivisionFinalCap.field();
      subdivisionVertexCap.field();
      subdivisionIndexCap.field();
      subdivisionLineCap.field();
      PixelCoverageSubdivision sub = engine.pixelCoverageSubdivision;
      if (frameCount % 120 == 0) sub.refreshStats();
      ImGui.text("triangles: base " + sub.totalTriangles + " -> final " + sub.finalCount() + ", vertices " + sub.stats[sub.FS_VERTEX_COUNT]);
      ImGui.text("overflow: queue " + sub.stats[sub.FS_STATS] + ", final " + sub.stats[sub.FS_STATS + 1] + ", vertex " + sub.stats[sub.FS_STATS + 2] + ", index " + sub.stats[sub.FS_STATS + 3]);
      ImGui.endTabItem();
    }
    if (ImGui.beginTabItem("Debug")) {
      if (ImGui.button(engine.paused ? "Resume (P)" : "Pause (P)")) {
        engine.paused = !engine.paused;
      }
      ImGui.sameLine();
      if (ImGui.button("Step (.)")) {
        engine.paused = true;
        engine.stepOnce = true;
      }
      ImGui.text("scene frame: " + engine.sceneFrame);
      if (ImGui.button("Capture (X)")) captureRequested = true;
      cameraStateText();
      if (debugFreeCamera.checkbox()) {
        // offになったらカメラをもとのobserverの位置に戻す
        if (!debugFreeCamera.get()) {
          camera.position.set(cameraObserver.position);
          camera.velocity.set(cameraObserver.velocity);
          camera.rotation.set(cameraObserver.rotation);
          cameraYaw = observerYaw;
          cameraPitch = observerPitch;
        }
      }
      debugWireframe.checkbox();
      debugFrustumVisible.checkbox();
      if (debugFrustumVisible.get()) debugFrustumFar.field();
      debugGpuProfile.checkbox();
      if (debugGpuProfile.get()) ImGui.text(profileSummary());
      renderScaleDivisor.slider(1, 4);
      ImGui.text("scene: " + sceneView.width() + "x" + sceneView.height() + " -> view " + view.width() + "x" + view.height());
      ImGui.endTabItem();
    }
    ImGui.endTabBar();
  }

  ImGui.end();
}

class ThrownObject {
  RenderObject renderObject;
  PVector velocity;

  ThrownObject(PVector position, PVector initialVelocity) {
    renderObject = engine.createObject(sphereVertices, sphereIndices, position);
    renderObject.scale = new PVector(throwObjectScale.get(), throwObjectScale.get(), throwObjectScale.get());
    velocity = initialVelocity;
  }

  void update() {
    velocity.sub(0, throwObjectGravity.get() * engine.KEYFRAME_INTERVAL, 0);
    velocity = velocity.setMag(min(velocity.mag(), engine.lightSpeed * 0.99));
    renderObject.position.add(velocity.copy().mult(engine.KEYFRAME_INTERVAL));
  }
}

// 前フレームのパスごとの GPU 時間を、同じ名前は足し合わせて「名前 ミリ秒」の行にする
String profileSummary() {
  java.util.LinkedHashMap<String, Float> sum = new java.util.LinkedHashMap<>();
  float total = 0;
  for (String line : engine.gpu.profileReport().split("\n")) {
    if (line.isEmpty()) continue;
    String[] t = line.split("\t");
    float ms = Float.parseFloat(t[1]);
    sum.merge(t[0], ms, Float::sum);
    total += ms;
  }
  StringBuilder b = new StringBuilder();
  for (java.util.Map.Entry<String, Float> e : sum.entrySet()) b.append(e.getKey()).append(' ').append(nf(e.getValue(), 0, 2)).append("ms\n");
  b.append("total ").append(nf(total, 0, 2)).append("ms");
  return b.toString();
}
