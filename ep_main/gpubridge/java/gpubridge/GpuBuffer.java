package gpubridge;

/**
 * storage buffer。ストライドは bind 時に WGSL から決まる（{@code gpu.buffer(n)}）か、
 * 生成時に宣言する（{@code gpu.buffer(n, strideBytes)}）。
 *
 * <p>要素が struct なら {@link #at(int)} でメンバ名を指定して書ける（パディングは
 * WGSL のレイアウトから自動で決まるので、呼ぶ側は意識しなくてよい）。
 * 既にフラットな配列を持っているなら {@link #write(float[])} の方が速い。
 *
 * <p>ストライド未宣言のバッファでは、write / read は「先にどこかのカーネルへ
 * bind した後」に呼ぶこと（ストライド未確定だと実体が無い）。at(i).set() は bind 前でも
 * 呼べる（値は保持され、初回 bind の時点で検証して適用される。メンバ名や型の間違いは
 * その bind で例外になる）。宣言済みなら write / read はすぐ使える（at のメンバ名は
 * bind 後から）。
 */
public final class GpuBuffer {

  final long id;
  private final int elementCount;
  private boolean disposed = false;

  GpuBuffer(long id, int elementCount) {
    this.id = id;
    this.elementCount = elementCount;
  }

  public int elementCount() {
    return elementCount;
  }

  /**
   * index 番目の要素を、メンバ名で書き込むためのハンドルを返す。
   *
   * <pre>
   * objectsBuffer.at(0)
   *     .set("type", SPHERE)
   *     .set("position", 0, 0, 5)
   *     .set("param0", 1.0);
   * </pre>
   *
   * <p>set は CPU 側のミラーを更新するだけで、GPU への転送は次の dispatch / submit の
   * 直前にまとめて行われる。連番で埋めれば転送は 1 回にまとまるので、ループで
   * 全要素を書いても構わない。
   */
  public Element at(int index) {
    checkAlive();
    checkOffset(index);
    return new Element(index);
  }

  /** {@link GpuBuffer#at(int)} が返す、1 要素分の書き込み口。 */
  public final class Element {

    private final int index;

    private Element(int index) {
      this.index = index;
    }

    public int index() {
      return index;
    }

    /** f32 メンバに値を入れる。 */
    public Element set(String member, float v) {
      checkAlive();
      GpuBridge.okArg(GpuBridge.nBufferSetF(id, index, member, new float[] {v}));
      return this;
    }

    /** vec2&lt;f32&gt; メンバに値を入れる。 */
    public Element set(String member, float x, float y) {
      checkAlive();
      GpuBridge.okArg(GpuBridge.nBufferSetF(id, index, member, new float[] {x, y}));
      return this;
    }

    /** vec3&lt;f32&gt; メンバに値を入れる。 */
    public Element set(String member, float x, float y, float z) {
      checkAlive();
      GpuBridge.okArg(GpuBridge.nBufferSetF(id, index, member, new float[] {x, y, z}));
      return this;
    }

    /** vec4&lt;f32&gt; メンバに値を入れる。 */
    public Element set(String member, float x, float y, float z, float w) {
      checkAlive();
      GpuBridge.okArg(GpuBridge.nBufferSetF(id, index, member, new float[] {x, y, z, w}));
      return this;
    }

    /** i32 / u32 メンバ（Slang の enum を含む）に値を入れる。 */
    public Element set(String member, int v) {
      checkAlive();
      GpuBridge.okArg(GpuBridge.nBufferSetI(id, index, member, v));
      return this;
    }
  }

  /** 初期データを float 列で書き込む（struct はフラットに並べる）。 */
  public void write(float[] data) {
    write(0, data);
  }

  /** 初期データを int 列で書き込む。 */
  public void write(int[] data) {
    write(0, data);
  }

  /** elementOffset 番目の要素から float 列で書き込む（struct はフラットに並べる）。 */
  public void write(int elementOffset, float[] data) {
    checkAlive();
    checkOffset(elementOffset);
    GpuBridge.okState(GpuBridge.nBufferWriteF(id, elementOffset, data));
  }

  /** elementOffset 番目の要素から int 列で書き込む。 */
  public void write(int elementOffset, int[] data) {
    checkAlive();
    checkOffset(elementOffset);
    GpuBridge.okState(GpuBridge.nBufferWriteI(id, elementOffset, data));
  }

  /**
   * バイトオフセット指定で float 列を書き込む（要素の途中・structの一部メンバだけの
   * 更新など、要素単位の {@link #write(int, float[])} で表せない位置に使う）。
   * オフセットは 4 の倍数。
   */
  public void writeBytes(long byteOffset, float[] data) {
    writeBytes(byteOffset, data, 0, data.length);
  }

  /** {@link #writeBytes(long, float[])} の範囲指定版。data[from] から count 個を書く。 */
  public void writeBytes(long byteOffset, float[] data, int from, int count) {
    checkAlive();
    GpuBridge.okState(GpuBridge.nBufferWriteBytesF(id, byteOffset, data, from, count));
  }

  /** バイトオフセット指定で int 列を書き込む。オフセットは 4 の倍数。 */
  public void writeBytes(long byteOffset, int[] data) {
    writeBytes(byteOffset, data, 0, data.length);
  }

  /** {@link #writeBytes(long, int[])} の範囲指定版。data[from] から count 個を書く。 */
  public void writeBytes(long byteOffset, int[] data, int from, int count) {
    checkAlive();
    GpuBridge.okState(GpuBridge.nBufferWriteBytesI(id, byteOffset, data, from, count));
  }

  /**
   * GPU の内容を読み戻す。<b>同期・低速</b>（保留中のフレームを flush して GPU 完了を待つ）。
   * デバッグや、粒子位置を Processing 側で使いたいときのためのもの。毎フレーム呼ぶ用途には
   * 向かない。
   */
  public void read(float[] dst) {
    checkAlive();
    GpuBridge.okState(GpuBridge.nBufferReadF(id, dst));
  }

  /** GPU メモリを解放する。以後このバッファを使うと例外。二重呼び出しは無害。 */
  public void dispose() {
    if (disposed) {
      return;
    }
    disposed = true;
    GpuBridge.okState(GpuBridge.nRelease(id));
  }

  private void checkOffset(int elementOffset) {
    if (elementOffset < 0 || elementOffset >= elementCount) {
      throw new IllegalArgumentException(
          "要素番号が範囲外です: " + elementOffset + "（要素数 " + elementCount + "）");
    }
  }

  void checkAlive() {
    if (disposed) {
      throw new IllegalStateException("この GpuBuffer は dispose 済みです");
    }
  }
}
