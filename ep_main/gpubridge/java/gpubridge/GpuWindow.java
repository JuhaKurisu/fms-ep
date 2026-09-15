package gpubridge;

import java.awt.Canvas;
import java.awt.Dimension;
import java.awt.GraphicsEnvironment;
import java.awt.event.ComponentAdapter;
import java.awt.event.ComponentEvent;
import java.awt.event.FocusAdapter;
import java.awt.event.FocusEvent;
import java.awt.event.KeyAdapter;
import java.awt.event.KeyEvent;
import java.awt.event.MouseAdapter;
import java.awt.event.MouseEvent;
import java.awt.event.WindowAdapter;
import java.awt.event.WindowEvent;
import java.lang.reflect.InvocationTargetException;
import java.nio.file.Path;
import javax.swing.JFrame;
import javax.swing.SwingUtilities;

/**
 * compute shader の出力先ウィンドウと、GPU リソースのファクトリ。
 *
 * <p>Processing の API には依存しない。ウィンドウ管理は AWT が持ち、
 * 描画は Rust(wgpu) が JAWT で描画面を直接借りて行う。
 *
 * <pre>
 * GpuWindow gpu = new GpuWindow(512, 512, "title");
 * GpuTexture a = gpu.texture(512, 512);
 * GpuKernel k = gpu.kernel(loadStrings("shader.wgsl"), "main");
 * GpuBinding b = k.binding().set("out", a);
 * // draw() 内:
 * b.dispatch(512, 512);
 * gpu.show(a);
 * gpu.submit();
 * </pre>
 */
public final class GpuWindow {

  private JFrame frame;
  private Canvas canvas;
  private final float scale;
  private volatile boolean open = false;
  private volatile int curW, curH;              // 現在の物理ピクセルサイズ
  private volatile boolean resizePending = false;
  // マウスが窓に入るまでの初期値は窓中央（0 だと係数に写したとき端に張り付く）
  private volatile float mx, my;
  private volatile boolean pressed = false;

  // ---- ホイール・文字入力・ボタン別マウス ----
  // EDT が書く生の値。keyLock で守り、submit() がフレーム境界で回収する。
  private float wheelLive = 0;
  private float wheelFrame = 0;
  private final StringBuilder charsLive = new StringBuilder();
  private String charsFrame = "";
  // AWT のボタン番号(1=左 2=中 3=右)を添字にする
  private static final int BUTTON_MAX = 8;
  private final boolean[] buttonLive = new boolean[BUTTON_MAX];
  private final boolean[] buttonLatchDown = new boolean[BUTTON_MAX];
  private final boolean[] buttonLatchUp = new boolean[BUTTON_MAX];
  private final boolean[] buttonState = new boolean[BUTTON_MAX];
  private final boolean[] buttonHit = new boolean[BUTTON_MAX];
  private final boolean[] buttonOff = new boolean[BUTTON_MAX];
  private final boolean[] buttonPrev = new boolean[BUTTON_MAX];

  // ---- キー入力 ----
  // AWT の keyCode を添字にした表。VK_* は 0x0200 台まであるので余裕を取る。
  private static final int KEY_MAX = 0x400;

  private final Object keyLock = new Object();
  // EDT が書く生の状態と、押下／解放のラッチ。submit() がフレーム境界で回収する。
  private final boolean[] keyLive = new boolean[KEY_MAX];
  private final boolean[] keyLatchDown = new boolean[KEY_MAX];
  private final boolean[] keyLatchUp = new boolean[KEY_MAX];
  // 以下は Animation Thread 専用のスナップショット。draw() の間は値が変わらない。
  private final boolean[] keyState = new boolean[KEY_MAX];
  private final boolean[] keyHit = new boolean[KEY_MAX];
  private final boolean[] keyOff = new boolean[KEY_MAX];
  private final boolean[] keyPrev = new boolean[KEY_MAX];

  /**
   * ウィンドウを開き、GPU を初期化する。
   *
   * <p>{@code w} / {@code h} は<b>物理ピクセル</b>。ウィンドウの描画面・テクスチャ・
   * {@link #mouseX()} がすべてこの同じ座標系になる（Retina では画面上の見かけは
   * w/2 pt になるが、描画は指定ピクセルちょうどで行われ、拡大によるぼやけがない）。
   *
   * @throws IllegalStateException GPU の初期化や描画面への接続に失敗した場合
   */
  public GpuWindow(int w, int h, String title) {
    this(w, h, title, false);
  }

  /**
   * リサイズ可否を指定して開く。リサイズ後の物理サイズは {@link #width()} /
   * {@link #height()} で取れる。表示は blit が伸縮を吸収するので、シャープさを
   * 保ちたければサイズ変化を見てテクスチャを作り直すこと。
   */
  public GpuWindow(int w, int h, String title, boolean resizable) {
    curW = w;
    curH = h;
    mx = w / 2f;
    my = h / 2f;
    scale =
        (float)
            GraphicsEnvironment.getLocalGraphicsEnvironment()
                .getDefaultScreenDevice()
                .getDefaultConfiguration()
                .getDefaultTransform()
                .getScaleX();
    int lw = Math.max(1, Math.round(w / scale));
    int lh = Math.max(1, Math.round(h / scale));
    GpuBridge.okState(GpuBridge.nInitGpu());

    runOnEdt(
        () -> {
          frame = new JFrame(title);
          canvas = new Canvas();
          canvas.setPreferredSize(new Dimension(lw, lh));
          canvas.setIgnoreRepaint(true);
          MouseAdapter mouse =
              new MouseAdapter() {
                @Override
                public void mouseMoved(MouseEvent e) {
                  mx = e.getX() * scale;
                  my = e.getY() * scale;
                }

                @Override
                public void mouseDragged(MouseEvent e) {
                  mx = e.getX() * scale;
                  my = e.getY() * scale;
                }

                @Override
                public void mousePressed(MouseEvent e) {
                  pressed = true;
                  setButton(e.getButton(), true);
                  canvas.requestFocusInWindow(); // クリックでキー入力の宛先になる
                }

                @Override
                public void mouseReleased(MouseEvent e) {
                  pressed = false;
                  setButton(e.getButton(), false);
                }
              };
          canvas.addMouseListener(mouse);
          canvas.addMouseMotionListener(mouse);

          canvas.addMouseWheelListener(
              e -> {
                synchronized (keyLock) {
                  // AWT は手前回転(下スクロール)が正。上=正に揃える。
                  wheelLive -= (float) e.getPreciseWheelRotation();
                }
              });

          // Tab を AWT のフォーカス移動に食わせず、キーとして受け取る。
          canvas.setFocusable(true);
          canvas.setFocusTraversalKeysEnabled(false);
          canvas.addKeyListener(
              new KeyAdapter() {
                @Override
                public void keyPressed(KeyEvent e) {
                  setKey(e.getKeyCode(), true);
                }

                @Override
                public void keyReleased(KeyEvent e) {
                  setKey(e.getKeyCode(), false);
                }

                @Override
                public void keyTyped(KeyEvent e) {
                  char c = e.getKeyChar();
                  if (c >= 32 && c != 127) { // 制御文字は文字入力ではない
                    synchronized (keyLock) {
                      charsLive.append(c);
                    }
                  }
                }
              });
          canvas.addFocusListener(
              new FocusAdapter() {
                @Override
                public void focusLost(FocusEvent e) {
                  // これがないと Cmd+Tab で切り替えた後にキーが押されたまま残る。
                  releaseAllKeys();
                }
              });

          frame.setResizable(resizable);
          canvas.addComponentListener(
              new ComponentAdapter() {
                @Override
                public void componentResized(ComponentEvent e) {
                  // EDT で発生する。ここでは記録だけして、サーフェスの再構成は
                  // submit()（Animation Thread）で行う — EDT からネイティブの
                  // リサイズを呼ぶと AppKit メインスレッドと循環待ちになるため。
                  curW = Math.max(1, Math.round(canvas.getWidth() * scale));
                  curH = Math.max(1, Math.round(canvas.getHeight() * scale));
                  resizePending = true;
                }
              });
          frame.setDefaultCloseOperation(JFrame.DISPOSE_ON_CLOSE);
          frame.addWindowListener(
              new WindowAdapter() {
                @Override
                public void windowClosing(WindowEvent e) {
                  open = false;
                }
              });
          frame.add(canvas);
          frame.pack();
          frame.setLocationRelativeTo(null);
          frame.setVisible(true);
          canvas.requestFocusInWindow();
        });

    // attach は EDT から呼んではいけない（macOS では AppKit メインスレッドとの
    // 循環待ちで JVM ごと固まる）。ネイティブ側が必要に応じて回送する。
    if (!GpuBridge.nAttach(canvas, scale, w, h)) {
      String reason = GpuBridge.nLastError();
      runOnEdt(() -> frame.dispose());
      GpuBridge.nDestroy();
      throw new IllegalStateException("描画面への接続に失敗しました:\n" + reason);
    }
    open = true;
  }

  // ---- リソースのファクトリ ----

  /** rgba8unorm の storage texture を確保する。 */
  public GpuTexture texture(int w, int h) {
    return texture(w, h, GpuFormat.RGBA8);
  }

  public GpuTexture texture(int w, int h, GpuFormat f) {
    return new GpuTexture(GpuBridge.okId(GpuBridge.nCreateTexture(w, h, f.id)), w, h, f, false, 1);
  }

  /**
   * キューブマップ（6 面・mips 段）。sample 専用で、描画先・clear・show には使えない。
   * 面は {@link GpuTexture#writeFace} で書く。
   */
  public GpuTexture textureCube(int size, int mips, GpuFormat f) {
    return new GpuTexture(GpuBridge.okId(GpuBridge.nCreateTextureCube(size, mips, f.id)), size, size, f, true, mips);
  }

  /**
   * storage buffer を確保する。1 要素のバイト数（ストライド）は、このバッファを
   * カーネルへ bind した時点で WGSL の配列要素型から自動で決まる。
   */
  public GpuBuffer buffer(int elementCount) {
    return new GpuBuffer(GpuBridge.okId(GpuBridge.nCreateBuffer(elementCount)), elementCount);
  }

  /**
   * ストライド（1 要素のバイト数）を宣言して storage buffer を確保する。
   * bind を待たずに write できるのが利点。後からカーネルへ bind すると、
   * WGSL 側のストライドと一致するかがその場で検証される。
   *
   * <p>インデックスバッファは「ストライド 4 のバッファ」として作る:
   * {@code gpu.buffer(n, 4)} → {@code write(int[])} → {@code drawIndexed(...)}。
   */
  public GpuBuffer buffer(int elementCount, int strideBytes) {
    return new GpuBuffer(
        GpuBridge.okId(GpuBridge.nCreateBufferStrided(elementCount, strideBytes)), elementCount);
  }

  /** uniform を確保する。レイアウトは bind 時に WGSL の struct から自動で決まる。 */
  public GpuUniform uniform() {
    return new GpuUniform(GpuBridge.okId(GpuBridge.nCreateUniform()));
  }

  /** WGSL をコンパイルし、エントリポイントを指すカーネルを作る。 */
  public GpuKernel kernel(String source, String entryPoint) {
    return new GpuKernel(GpuBridge.okId(GpuBridge.nKernel(source, entryPoint)));
  }

  /** Processing の loadStrings() をそのまま渡せるオーバーロード。 */
  public GpuKernel kernel(String[] source, String entryPoint) {
    return kernel(String.join("\n", source), entryPoint);
  }

  private volatile Path slangSearchPath;

  /**
   * {@link #kernelSlang} の import 解決に使うディレクトリを設定する。
   * ここにある {@code *.slang} がコンパイル時に参照できるようになるので、
   * Slang ソース内の {@code import "foo";} が {@code dir/foo.slang} を読める。
   * Processing からは {@code gpu.slangSearchPath(dataPath(""))} のように渡す。
   */
  public void slangSearchPath(String dir) {
    slangSearchPath = (dir == null) ? null : Path.of(dir);
  }

  /**
   * Slang ソースからカーネルを作る（slangc で WGSL に変換して {@link #kernel} へ渡す）。
   *
   * <p>binding や uniform メンバは Slang に書いた元の名前で {@code set()} できる
   * （変換時のリネームはライブラリが吸収する）。書き込み先テクスチャは
   * {@code WTexture2D} + {@code .Store()} を使うこと（{@code RWTexture2D} は
   * WebGPU の制約で弾かれる）。
   */
  public GpuKernel kernelSlang(String source, String entryPoint) {
    return kernel(SlangCompiler.toWgsl(source, slangSearchPath), entryPoint);
  }

  public GpuKernel kernelSlang(String[] source, String entryPoint) {
    return kernelSlang(String.join("\n", source), entryPoint);
  }

  /** WGSL をコンパイルし、vertex / fragment のエントリを指すレンダラーを作る。 */
  public GpuRenderer renderer(String source, String vsEntry, String fsEntry) {
    return new GpuRenderer(GpuBridge.okId(GpuBridge.nRenderer(source, vsEntry, fsEntry)));
  }

  /** Processing の loadStrings() をそのまま渡せるオーバーロード。 */
  public GpuRenderer renderer(String[] source, String vsEntry, String fsEntry) {
    return renderer(String.join("\n", source), vsEntry, fsEntry);
  }

  /** Slang ソースからレンダラーを作る（slangc で WGSL に変換して {@link #renderer} へ渡す）。 */
  public GpuRenderer rendererSlang(String source, String vsEntry, String fsEntry) {
    return renderer(SlangCompiler.toWgsl(source, slangSearchPath), vsEntry, fsEntry);
  }

  public GpuRenderer rendererSlang(String[] source, String vsEntry, String fsEntry) {
    return rendererSlang(String.join("\n", source), vsEntry, fsEntry);
  }

  // ---- フレーム ----

  /**
   * テクスチャをこの色で塗りつぶす（記録コマンド）。深度テストを使う描画先なら
   * 深度バッファも一緒にクリアされる。値は 0..1。
   */
  public void clear(GpuTexture t, float r, float g, float b, float a) {
    t.checkAlive();
    GpuBridge.okArg(GpuBridge.nClear(t.id, r, g, b, a));
  }

  /**
   * バッファ間の GPU コピー（記録コマンド）。呼び出し順どおり、先行する
   * dispatch / draw の後に実行される。オフセット・バイト数はすべて 4 の倍数。
   * コピー先を後から {@code at(i).set()} で部分更新しても、コピーで届いた
   * 他の要素は上書きされない（compute がバッファへ書く場合と同じ扱い）。
   */
  public void copyBufferToBuffer(
      GpuBuffer src, long srcOffset, GpuBuffer dst, long dstOffset, long byteCount) {
    src.checkAlive();
    dst.checkAlive();
    GpuBridge.okState(
        GpuBridge.nCopyBufferToBuffer(src.id, srcOffset, dst.id, dstOffset, byteCount));
  }

  /** このフレームで画面に表示するテクスチャを指定する（RGBA8 のみ）。 */
  public void show(GpuTexture t) {
    t.checkAlive();
    GpuBridge.okArg(GpuBridge.nShow(t.id));
  }

  /** ここまでの dispatch 列を 1 コマンドバッファとして投げ、画面を更新する。 */
  public void submit() {
    if (resizePending) {
      resizePending = false;
      GpuBridge.okState(GpuBridge.nResize(scale, curW, curH));
    }
    snapshotKeys();
    GpuBridge.okState(GpuBridge.nSubmit());
  }

  /** 現在のウィンドウ描画面の幅（物理ピクセル）。リサイズすると変わる。 */
  public int width() {
    return curW;
  }

  /** 現在のウィンドウ描画面の高さ（物理ピクセル）。 */
  public int height() {
    return curH;
  }

  // ---- 入力・状態 ----

  /** GPU ウィンドウ上のマウス X（物理ピクセル座標。テクスチャと同じ座標系）。 */
  public float mouseX() {
    return mx;
  }

  public float mouseY() {
    return my;
  }

  public boolean mousePressed() {
    return pressed;
  }

  /**
   * そのマウスボタンが今押されているか。ボタン番号は AWT と同じ(1=左 2=中 3=右)。
   * 値は 1 フレーム分に固定される({@link #keyDown(int)} と同じ流儀)。
   */
  public boolean mouseDown(int button) {
    return button >= 0 && button < BUTTON_MAX && buttonState[button];
  }

  /** このフレームでそのボタンが押され始めたか。 */
  public boolean mouseHit(int button) {
    return button >= 0 && button < BUTTON_MAX && buttonHit[button];
  }

  /** このフレームでそのボタンが離されたか。 */
  public boolean mouseOff(int button) {
    return button >= 0 && button < BUTTON_MAX && buttonOff[button];
  }

  /**
   * このフレームのホイール移動量(上方向が正、ノッチ単位)。トラックパッドでは
   * 細かい連続値になる。値は {@link #submit()} がフレーム境界で確定する。
   */
  public float wheel() {
    return wheelFrame;
  }

  /**
   * このフレームに入力された文字列(IME や Shift 込みの確定文字)。
   * キーの押下状態を見たいなら {@link #keyDown(int)} を使う。
   */
  public String typedChars() {
    return charsFrame;
  }

  /** OS の表示スケール(Retina なら 2.0)。物理ピクセル = 論理 pt × この値。 */
  public float scale() {
    return scale;
  }

  /**
   * そのキーが今押されているか。
   *
   * <p>キーの指定は 2 通り。英数字と記号は文字で（{@code keyDown('w')} — 大文字小文字は
   * 区別しない）、特殊キーは Processing の定数で（{@code keyDown(LEFT)},
   * {@code keyDown(SHIFT)}, {@code keyDown(ESC)} …）。Processing のキー定数は AWT の
   * {@code KeyEvent.VK_*} と同じ値なので、そのまま渡せる。
   *
   * <p>返す値は 1 フレーム分に固定されている（{@link #submit()} が更新する）。同じ
   * {@code draw()} の中で何度読んでも結果は変わらない。
   */
  public boolean keyDown(int keyCode) {
    return inRange(keyCode) && keyState[keyCode];
  }

  /** {@link #keyDown(int)} の文字版。{@code 'w'} と {@code 'W'} は同じキーを指す。 */
  public boolean keyDown(char key) {
    return keyDown(codeOf(key));
  }

  /**
   * このフレームでそのキーが押され始めたか（押した瞬間だけ true）。
   *
   * <p>Processing のグローバルな {@code mousePressed}（押されている「状態」）とは意味が
   * 違うので注意。押しっぱなしを見たいなら {@link #keyDown(int)} を使う。
   */
  public boolean keyPressed(int keyCode) {
    return inRange(keyCode) && keyHit[keyCode];
  }

  /** {@link #keyPressed(int)} の文字版。 */
  public boolean keyPressed(char key) {
    return keyPressed(codeOf(key));
  }

  /** このフレームでそのキーが離されたか（離した瞬間だけ true）。 */
  public boolean keyReleased(int keyCode) {
    return inRange(keyCode) && keyOff[keyCode];
  }

  /** {@link #keyReleased(int)} の文字版。 */
  public boolean keyReleased(char key) {
    return keyReleased(codeOf(key));
  }

  /**
   * 向かい合う 2 キーを -1 / 0 / +1 に写す。両方押されていれば 0。
   *
   * <pre>
   * float dx = gpu.axis('a', 'd');   // 左右移動が 1 行で書ける
   * </pre>
   */
  public float axis(int negKeyCode, int posKeyCode) {
    return (keyDown(posKeyCode) ? 1f : 0f) - (keyDown(negKeyCode) ? 1f : 0f);
  }

  /** {@link #axis(int, int)} の文字版。 */
  public float axis(char negKey, char posKey) {
    return axis(codeOf(negKey), codeOf(posKey));
  }

  // ---- キー入力の内部 ----

  private static boolean inRange(int keyCode) {
    return keyCode >= 0 && keyCode < KEY_MAX;
  }

  /**
   * 文字を AWT の keyCode に写す。英数字と大半の記号は大文字の ASCII が VK_* と一致する
   * （{@code 'w'} → {@code VK_W}）。一致しない 2 つだけ個別に対応する。
   */
  private static int codeOf(char key) {
    switch (key) {
      case '\'':
        return KeyEvent.VK_QUOTE;
      case '`':
        return KeyEvent.VK_BACK_QUOTE;
      default:
        return Character.toUpperCase(key);
    }
  }

  /** EDT から呼ばれる。状態を更新し、押下／解放をフレーム境界まで溜める。 */
  private void setKey(int keyCode, boolean down) {
    if (!inRange(keyCode)) {
      return;
    }
    synchronized (keyLock) {
      if (keyLive[keyCode] == down) {
        return; // macOS/Windows のオートリピート（keyPressed の連打）を吸収する
      }
      keyLive[keyCode] = down;
      if (down) {
        keyLatchDown[keyCode] = true;
      } else {
        keyLatchUp[keyCode] = true;
      }
    }
  }

  /** EDT から呼ばれる。ボタン状態を更新し、押下／解放をフレーム境界まで溜める。 */
  private void setButton(int button, boolean down) {
    if (button < 0 || button >= BUTTON_MAX) {
      return;
    }
    synchronized (keyLock) {
      if (buttonLive[button] == down) {
        return;
      }
      buttonLive[button] = down;
      if (down) {
        buttonLatchDown[button] = true;
      } else {
        buttonLatchUp[button] = true;
      }
    }
  }

  private void releaseAllKeys() {
    synchronized (keyLock) {
      for (int i = 0; i < KEY_MAX; i++) {
        if (keyLive[i]) {
          keyLive[i] = false;
          keyLatchUp[i] = true;
        }
      }
      for (int i = 0; i < BUTTON_MAX; i++) {
        if (buttonLive[i]) {
          buttonLive[i] = false;
          buttonLatchUp[i] = true;
        }
      }
    }
  }

  /**
   * ラッチを 1 フレーム分のスナップショットに切り出す。{@link #submit()} から呼ばれる。
   *
   * <p>フレーム内で解放と押下が両方起きたキーは、最終状態を見てエッジを打ち消す。これで
   * X11 のオートリピート（keyReleased と keyPressed が対で届く）が押し直しに化けない。
   */
  private void snapshotKeys() {
    synchronized (keyLock) {
      for (int i = 0; i < KEY_MAX; i++) {
        boolean live = keyLive[i];
        keyHit[i] = keyLatchDown[i] && !keyPrev[i]; // 前フレームから押しっぱなしなら新規ではない
        keyOff[i] = keyLatchUp[i] && !live; // 押し直されているなら離してはいない
        keyLatchDown[i] = false;
        keyLatchUp[i] = false;
        keyPrev[i] = live;
        keyState[i] = live;
      }
      for (int i = 0; i < BUTTON_MAX; i++) {
        boolean live = buttonLive[i];
        buttonHit[i] = buttonLatchDown[i] && !buttonPrev[i];
        buttonOff[i] = buttonLatchUp[i] && !live;
        buttonLatchDown[i] = false;
        buttonLatchUp[i] = false;
        buttonPrev[i] = live;
        buttonState[i] = live;
      }
      wheelFrame = wheelLive;
      wheelLive = 0;
      charsFrame = charsLive.toString();
      charsLive.setLength(0);
    }
  }

  /** ウィンドウがまだ開いているか。閉じられたら false。 */
  public boolean isOpen() {
    return open;
  }

  /** 実際に使われているバックエンド名（"Metal / Apple M3" など）。 */
  /**
   * パスごとの GPU 時間の計測を切り替える。有効なあいだは描画パスがレンダラごとに分かれ、
   * 毎フレーム末尾で GPU の完了を待つので、計測そのものが少し重くなる。
   */
  public void profile(boolean on) {
    GpuBridge.okState(GpuBridge.nProfile(on));
  }

  /** 前フレームの計測結果。1 行が「ラベル<TAB>ミリ秒」。計測が無効なら空 */
  public String profileReport() {
    return GpuBridge.nProfileReport();
  }

  public String backend() {
    return GpuBridge.nBackend();
  }

  /** ウィンドウを閉じ、GPU リソースをすべて解放する。 */
  public void dispose() {
    open = false;
    GpuBridge.nDestroy();
    SwingUtilities.invokeLater(
        () -> {
          if (frame != null) {
            frame.dispose();
          }
        });
  }

  private static void runOnEdt(Runnable r) {
    if (SwingUtilities.isEventDispatchThread()) {
      r.run();
      return;
    }
    try {
      SwingUtilities.invokeAndWait(r);
    } catch (InterruptedException e) {
      Thread.currentThread().interrupt();
      throw new IllegalStateException("ウィンドウ生成が中断されました", e);
    } catch (InvocationTargetException e) {
      throw new IllegalStateException("ウィンドウの生成に失敗しました", e.getCause());
    }
  }
}
