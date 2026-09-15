package imguibridge;

import gpubridge.GpuBlend;
import gpubridge.GpuBuffer;
import gpubridge.GpuFormat;
import gpubridge.GpuRenderBinding;
import gpubridge.GpuRenderer;
import gpubridge.GpuTexture;
import gpubridge.GpuTopology;
import gpubridge.GpuUniform;
import gpubridge.GpuWindow;
import imgui.ImDrawData;
import imgui.ImFontConfig;
import imgui.ImGui;
import imgui.ImGuiIO;
import imgui.ImVec4;
import imgui.flag.ImGuiConfigFlags;
import imgui.flag.ImGuiKey;
import imgui.internal.ImGuiContext;
import imgui.type.ImInt;
import java.awt.event.KeyEvent;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.IntBuffer;
import java.nio.ShortBuffer;
import java.util.HashMap;
import java.util.Map;

/**
 * Dear ImGui(imgui-java)と gpubridge の橋渡し。UI 面 1 つにつき 1 インスタンス。
 *
 * <p>ウィジェット呼び出しは imgui-java の API({@code ImGui.begin()} …)を直接使い、
 * このクラスは毎フレームの入力注入({@link #newFrame})と描画({@link #render})だけを担う。
 * 複数インスタンスを作ればそれぞれ独立した UI になる(context はインスタンスごと。
 * ウィジェット呼び出しは直近に {@link #newFrame} した bridge に入る)。
 *
 * <pre>
 * ImGuiBridge ui = new ImGuiBridge(gpu);
 * // draw() 内:
 * ui.newFrame(dt);
 * ImGui.begin("params"); ... ImGui.end();
 * ui.render(view);       // 任意の GpuTexture に描ける
 * gpu.show(view);
 * gpu.submit();
 * </pre>
 */
public final class ImGuiBridge {

  private final GpuWindow gpu;
  private final ImGuiContext ctx;
  // この UI が target 上で占める矩形。vw < 0 なら target 全面
  private int vx, vy, vw = -1, vh;
  // ウィンドウ上でこの UI が表示されている矩形(マウス変換元)。iw < 0 なら viewport と同じ
  private int ix, iy, iw = -1, ih;

  private final GpuRenderer renderer;
  private final GpuUniform uiParams;
  private GpuBuffer vtxBuf; // u32×5/頂点。足りなくなったら作り直す
  private GpuBuffer idxBuf; // u32。同上
  private int vtxCapInts;
  private int idxCap;
  private final GpuTexture fontTex;
  // ImTextureID(自前採番)→ テクスチャ / binding。binding はテクスチャごとに 1 個
  //(bind group は flush 時に解決されるため、フレーム途中の set() 差し替えは不可)
  private final Map<Long, GpuTexture> textures = new HashMap<>();
  private final Map<Long, GpuRenderBinding> bindings = new HashMap<>();
  private long nextTexId = 1;

  private static final int INITIAL_VTX = 4096; // 頂点数
  private static final int INITIAL_IDX = 8192; // インデックス数

  /** ImGui context を作り、GPU リソース(フォント・シェーダー)を初期化する。 */
  public ImGuiBridge(GpuWindow gpu) {
    this.gpu = gpu;
    ctx = ImGui.createContext();
    ImGui.setCurrentContext(ctx);
    ImGui.getIO().addConfigFlags(ImGuiConfigFlags.DockingEnable);

    float s = gpu.scale();
    ImFontConfig cfg = new ImFontConfig();
    cfg.setSizePixels(Math.round(13 * s)); // 物理ピクセル解像度に合わせて焼く
    ImGui.getIO().getFonts().addFontDefault(cfg);
    cfg.destroy();
    ImGui.getStyle().scaleAllSizes(s);

    renderer =
        gpu.renderer(UiShader.WGSL, "vsMain", "fsMain")
            .topology(GpuTopology.TRIANGLES)
            .blend(GpuBlend.ALPHA)
            .depthTest(false);
    uiParams = gpu.uniform();
    vtxCapInts = INITIAL_VTX * 5;
    idxCap = INITIAL_IDX;
    vtxBuf = gpu.buffer(vtxCapInts, 4);
    idxBuf = gpu.buffer(idxCap, 4);

    // フォントアトラス: RGBA バイト列 → ARGB(GpuTexture.write の形式)
    ImInt fw = new ImInt();
    ImInt fh = new ImInt();
    ByteBuffer px = ImGui.getIO().getFonts().getTexDataAsRGBA32(fw, fh);
    int[] argb = new int[fw.get() * fh.get()];
    for (int i = 0; i < argb.length; i++) {
      int r = px.get(i * 4) & 0xff;
      int g = px.get(i * 4 + 1) & 0xff;
      int b = px.get(i * 4 + 2) & 0xff;
      int a = px.get(i * 4 + 3) & 0xff;
      argb[i] = (a << 24) | (r << 16) | (g << 8) | b;
    }
    fontTex = gpu.texture(fw.get(), fh.get(), GpuFormat.RGBA8);
    fontTex.write(argb);
    long fontId = registerTexture(fontTex);
    ImGui.getIO().getFonts().setTexID(fontId);
  }

  /** この bridge の context を current にする。ウィジェット呼び出しの宛先を切り替えたいときに使う。 */
  public void makeCurrent() {
    ImGui.setCurrentContext(ctx);
  }

  /**
   * この UI が target 上で占める矩形を指定する(物理ピクセル)。displaySize は
   * (w, h) になり、描画はその矩形内に収まる。指定しなければ target 全面。
   */
  public ImGuiBridge viewport(int x, int y, int w, int h) {
    if (w <= 0 || h <= 0) {
      throw new IllegalArgumentException("viewport の幅・高さは 1 以上が必要です: " + w + "x" + h);
    }
    vx = x;
    vy = y;
    vw = w;
    vh = h;
    return this;
  }

  /**
   * ウィンドウ上でこの UI が表示されている矩形(マウス座標の変換元)。
   * 省略時は {@link #viewport} と同じ — view を 1:1 で表示している間はそれでよい。
   */
  public ImGuiBridge inputRect(int x, int y, int w, int h) {
    if (w <= 0 || h <= 0) {
      throw new IllegalArgumentException("inputRect の幅・高さは 1 以上が必要です: " + w + "x" + h);
    }
    ix = x;
    iy = y;
    iw = w;
    ih = h;
    return this;
  }

  /**
   * GpuTexture を ImGui から使えるように登録し、ImTextureID を返す。
   * {@code ImGui.image(id, w, h)} にそのまま渡せる。
   */
  public long registerTexture(GpuTexture t) {
    long id = nextTexId++;
    textures.put(id, t);
    return id;
  }

  /** テクスチャごとの描画 binding(なければ作る)。バッファ作り直し時は全部捨てる。 */
  private GpuRenderBinding bindingFor(long texId) {
    GpuRenderBinding b = bindings.get(texId);
    if (b == null) {
      GpuTexture t = textures.get(texId);
      if (t == null) {
        throw new IllegalStateException(
            "未登録の ImTextureID です: " + texId + "(registerTexture で登録してください)");
      }
      b = renderer.binding().set("params", uiParams).set("verts", vtxBuf).set("tex", t);
      bindings.put(texId, b);
    }
    return b;
  }

  /** 入力を ImGui に渡してフレームを開始する。dt は前フレームからの秒数。 */
  public void newFrame(float dt) {
    ImGui.setCurrentContext(ctx);
    ImGuiIO io = ImGui.getIO();
    // viewport 未指定なら target 全面 = ウィンドウサイズ(view を 1:1 表示する前提の既定値)
    float dw = vw < 0 ? gpu.width() : vw;
    float dh = vw < 0 ? gpu.height() : vh;
    io.setDisplaySize(dw, dh);
    io.setDeltaTime(dt > 0 ? dt : 1f / 60);

    // マウス座標を inputRect(省略時 viewport)基準の UI 座標に変換する。
    // 矩形外は displaySize の外の座標になるだけで、ImGui のホバー判定が自然に無視する。
    float rx = iw < 0 ? (vw < 0 ? 0 : vx) : ix;
    float ry = iw < 0 ? (vw < 0 ? 0 : vy) : iy;
    float rw = iw < 0 ? dw : iw;
    float rh = iw < 0 ? dh : ih;
    io.addMousePosEvent((gpu.mouseX() - rx) * dw / rw, (gpu.mouseY() - ry) * dh / rh);
    // AWT 1=左 2=中 3=右 → ImGui 0=左 1=右 2=中
    feedButton(io, 1, 0);
    feedButton(io, 3, 1);
    feedButton(io, 2, 2);
    float wheel = gpu.wheel();
    if (wheel != 0) {
      io.addMouseWheelEvent(0, wheel);
    }
    String chars = gpu.typedChars();
    if (!chars.isEmpty()) {
      io.addInputCharactersUTF8(chars);
    }
    // 修飾キーはキーイベントより先に届ける(ImGui の推奨順)
    io.addKeyEvent(ImGuiKey.ImGuiMod_Ctrl, gpu.keyDown(KeyEvent.VK_CONTROL));
    io.addKeyEvent(ImGuiKey.ImGuiMod_Shift, gpu.keyDown(KeyEvent.VK_SHIFT));
    io.addKeyEvent(ImGuiKey.ImGuiMod_Alt, gpu.keyDown(KeyEvent.VK_ALT));
    io.addKeyEvent(ImGuiKey.ImGuiMod_Super, gpu.keyDown(KeyEvent.VK_META));
    for (int vk : AwtKeyMap.awtCodes()) {
      if (gpu.keyPressed(vk)) {
        io.addKeyEvent(AwtKeyMap.imguiKey(vk), true);
      }
      if (gpu.keyReleased(vk)) {
        io.addKeyEvent(AwtKeyMap.imguiKey(vk), false);
      }
    }

    ImGui.newFrame();
  }

  private void feedButton(ImGuiIO io, int awtButton, int imguiButton) {
    if (gpu.mouseHit(awtButton)) {
      io.addMouseButtonEvent(imguiButton, true);
    }
    if (gpu.mouseOff(awtButton)) {
      io.addMouseButtonEvent(imguiButton, false);
    }
  }

  /** ImGui がマウスを使っているか。true の間はスケッチ側のドラッグ操作等を止めるとよい。 */
  public boolean wantCaptureMouse() {
    ImGui.setCurrentContext(ctx);
    return ImGui.getIO().getWantCaptureMouse();
  }

  /** ImGui がキーボードを使っているか(テキスト入力中など)。 */
  public boolean wantCaptureKeyboard() {
    ImGui.setCurrentContext(ctx);
    return ImGui.getIO().getWantCaptureKeyboard();
  }

  /**
   * ImGui がテキスト入力中か。{@link #wantCaptureKeyboard()} はウィジェットをマウスで押している間も
   * true になるので、WASD 移動のような常時入力を止める判定にはこちらを使う。
   */
  public boolean wantTextInput() {
    ImGui.setCurrentContext(ctx);
    return ImGui.getIO().getWantTextInput();
  }

  /** ここまでの UI を確定し、target に描画する。 */
  public void render(GpuTexture target) {
    ImGui.setCurrentContext(ctx);
    ImGui.render();
    ImDrawData dd = ImGui.getDrawData();
    int lists = dd.getCmdListsCount();
    if (lists <= 0) {
      return;
    }

    // 総量を数えて容量を確保(バッファ作り直しは draw 記録前でないと危険)
    int totalVtx = 0;
    int totalIdx = 0;
    for (int n = 0; n < lists; n++) {
      totalVtx += dd.getCmdListVtxBufferSize(n);
      totalIdx += dd.getCmdListIdxBufferSize(n);
    }
    ensureCapacity(totalVtx * 5, totalIdx);

    int offX = vw < 0 ? 0 : vx;
    int offY = vw < 0 ? 0 : vy;
    int vpW = vw < 0 ? target.width() : vw;
    int vpH = vw < 0 ? target.height() : vh;
    uiParams
        .set("targetSize", (float) target.width(), (float) target.height())
        .set("offset", (float) offX, (float) offY);

    ImVec4 clip = new ImVec4();
    int vtxBase = 0; // 頂点単位
    int idxBase = 0; // インデックス単位
    for (int n = 0; n < lists; n++) {
      int vtxCount = dd.getCmdListVtxBufferSize(n);
      int idxCount = dd.getCmdListIdxBufferSize(n);

      // 頂点: ImDrawVert(20B)は u32×5 としてそのままコピーできる
      IntBuffer vb =
          dd.getCmdListVtxBufferData(n).order(ByteOrder.nativeOrder()).asIntBuffer();
      int[] vtxChunk = new int[vtxCount * 5];
      vb.get(vtxChunk);
      vtxBuf.writeBytes((long) vtxBase * 20L, vtxChunk);

      // インデックス: u16 → u32。RendererHasVtxOffset を立てていないので
      // コマンドの vtxOffset は常に 0。リストの頂点基底だけ足せばよい。
      ShortBuffer ib =
          dd.getCmdListIdxBufferData(n).order(ByteOrder.nativeOrder()).asShortBuffer();
      int[] idxChunk = new int[idxCount];
      for (int i = 0; i < idxCount; i++) {
        idxChunk[i] = (ib.get(i) & 0xffff) + vtxBase;
      }
      idxBuf.writeBytes((long) idxBase * 4L, idxChunk);

      int cmds = dd.getCmdListCmdBufferSize(n);
      for (int c = 0; c < cmds; c++) {
        dd.getCmdListCmdBufferClipRect(clip, n, c);
        // clip は UI 座標。target 座標へ viewport オフセットを足し、UI が viewport の外へ
        // はみ出さないよう矩形全体を viewport にクランプする(ウィンドウは displaySize の
        // 外へドラッグできるため、clip が viewport を超えることがある)
        int cx = Math.max(offX, (int) clip.x + offX);
        int cy = Math.max(offY, (int) clip.y + offY);
        int cw = Math.min(offX + vpW, (int) Math.ceil(clip.z) + offX) - cx;
        int ch = Math.min(offY + vpH, (int) Math.ceil(clip.w) + offY) - cy;
        if (cw <= 0 || ch <= 0) {
          continue;
        }
        GpuRenderBinding b = bindingFor(dd.getCmdListCmdBufferTextureId(n, c));
        b.scissor(cx, cy, cw, ch)
            .drawIndexed(
                target,
                idxBuf,
                idxBase + dd.getCmdListCmdBufferIdxOffset(n, c),
                dd.getCmdListCmdBufferElemCount(n, c));
      }

      vtxBase += vtxCount;
      idxBase += idxCount;
    }
  }

  /** 頂点(u32 単位)とインデックスの容量を確保する。作り直したら binding も作り直し。 */
  private void ensureCapacity(int neededVtxInts, int neededIdx) {
    boolean recreated = false;
    if (neededVtxInts > vtxCapInts) {
      while (vtxCapInts < neededVtxInts) {
        vtxCapInts *= 2;
      }
      vtxBuf.dispose();
      vtxBuf = gpu.buffer(vtxCapInts, 4);
      recreated = true;
    }
    if (neededIdx > idxCap) {
      while (idxCap < neededIdx) {
        idxCap *= 2;
      }
      idxBuf.dispose();
      idxBuf = gpu.buffer(idxCap, 4);
    }
    if (recreated) {
      bindings.clear(); // 古い vtxBuf を参照している binding は使えない
    }
  }
}
