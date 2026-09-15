package gpubridge;

/**
 * uniform buffer。WGSL の struct メンバ名で値を入れる（部分更新可、値は保持される）。
 *
 * <p>レイアウト（サイズ・オフセット・パディング）は bind 時に WGSL から自動で決まる。
 * bind 前に set() した値は保持され、初回 bind の時点で検証して適用される
 * （メンバ名や型の間違いはその bind の {@code binding().set()} で例外になる）。
 *
 * <p>フレーム内で set → dispatch → set → dispatch と書いた場合、各 dispatch には
 * その時点の値が届く（内部でリングバッファを使う）。
 */
public final class GpuUniform {

  final long id;
  private boolean disposed = false;

  GpuUniform(long id) {
    this.id = id;
  }

  /** f32 メンバに値を入れる。 */
  public GpuUniform set(String member, float v) {
    checkAlive();
    GpuBridge.okArg(GpuBridge.nUniformSetF(id, member, new float[] {v}));
    return this;
  }

  /** vec2&lt;f32&gt; メンバに値を入れる。 */
  public GpuUniform set(String member, float x, float y) {
    checkAlive();
    GpuBridge.okArg(GpuBridge.nUniformSetF(id, member, new float[] {x, y}));
    return this;
  }

  /** vec3&lt;f32&gt; メンバに値を入れる。 */
  public GpuUniform set(String member, float x, float y, float z) {
    checkAlive();
    GpuBridge.okArg(GpuBridge.nUniformSetF(id, member, new float[] {x, y, z}));
    return this;
  }

  /** vec4&lt;f32&gt; メンバに値を入れる。 */
  public GpuUniform set(String member, float x, float y, float z, float w) {
    checkAlive();
    GpuBridge.okArg(GpuBridge.nUniformSetF(id, member, new float[] {x, y, z, w}));
    return this;
  }

  /** mat4x4&lt;f32&gt; メンバに値を入れる。m は列優先（column-major）の 16 要素。 */
  public GpuUniform set(String member, float[] m) {
    checkAlive();
    if (m.length != 16) {
      throw new IllegalArgumentException(
          "行列は列優先 16 要素の float[] で渡してください（" + m.length + " 要素でした）");
    }
    GpuBridge.okArg(GpuBridge.nUniformSetF(id, member, m));
    return this;
  }

  /** i32 / u32 メンバに値を入れる。 */
  public GpuUniform set(String member, int v) {
    checkAlive();
    GpuBridge.okArg(GpuBridge.nUniformSetI(id, member, v));
    return this;
  }

  /** GPU メモリを解放する。以後この uniform を使うと例外。二重呼び出しは無害。 */
  public void dispose() {
    if (disposed) {
      return;
    }
    disposed = true;
    GpuBridge.okState(GpuBridge.nRelease(id));
  }

  void checkAlive() {
    if (disposed) {
      throw new IllegalStateException("この GpuUniform は dispose 済みです");
    }
  }
}
