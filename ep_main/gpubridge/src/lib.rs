//! Processing から使う N パス compute shader ブリッジ — JNI 層。
//!
//! 実体は engine.rs（リソース・パイプライン・フレーム管理）と
//! reflect.rs（WGSL リフレクション）。ここは JNI の型変換とエラー変換のみ。
//!
//! ウィンドウ管理は Java(AWT) 側が持ち、JAWT で描画面だけを借りる（platform.rs）。

mod engine;
mod jawt;
mod platform;
mod reflect;

use engine::{Engine, OutputTarget};
use jni::JNIEnv;
use jni::objects::{JClass, JFloatArray, JIntArray, JObject, JString};
use jni::sys::{JNI_FALSE, JNI_TRUE, jboolean, jfloat, jint, jlong, jstring};
use std::sync::Mutex;

/// 画面へ出すための blit（フルスクリーン三角形でテクスチャを貼る）。
const BLIT_WGSL: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;

struct VOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) idx: u32) -> VOut {
  var corners = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -3.0),
    vec2<f32>(-1.0,  1.0),
    vec2<f32>( 3.0,  1.0),
  );
  let xy = corners[idx];
  var o: VOut;
  o.pos = vec4<f32>(xy, 0.0, 1.0);
  o.uv = vec2<f32>((xy.x + 1.0) * 0.5, 1.0 - (xy.y + 1.0) * 0.5);
  return o;
}

@fragment
fn fs(v: VOut) -> @location(0) vec4<f32> {
  return textureSample(src, smp, v.uv);
}
"#;

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);
static LAST_ERROR: Mutex<String> = Mutex::new(String::new());

fn set_error(msg: impl Into<String>) {
    if let Ok(mut e) = LAST_ERROR.lock() {
        *e = msg.into();
    }
}

/// Engine を取り出して f を実行。失敗は LAST_ERROR に入れて None。
fn with_engine<T>(f: impl FnOnce(&mut Engine) -> Result<T, String>) -> Option<T> {
    let mut guard = ENGINE.lock().unwrap();
    let Some(engine) = guard.as_mut() else {
        set_error("GPU が初期化されていません");
        return None;
    };
    match f(engine) {
        Ok(v) => {
            set_error("");
            Some(v)
        }
        Err(e) => {
            set_error(e);
            None
        }
    }
}

fn jstr(env: &mut JNIEnv, s: &JString) -> Result<String, String> {
    env.get_string(s)
        .map(Into::into)
        .map_err(|e| format!("文字列の取得に失敗: {e}"))
}

fn bool_of(v: Option<()>) -> jboolean {
    if v.is_some() { JNI_TRUE } else { JNI_FALSE }
}

fn long_of(v: Option<u64>) -> jlong {
    v.map(|x| x as jlong).unwrap_or(0)
}

// ---------------- ライフサイクル ----------------

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nInitGpu(_env: JNIEnv, _c: JClass) -> jboolean {
    match Engine::new() {
        Ok(e) => {
            *ENGINE.lock().unwrap() = Some(e);
            set_error("");
            JNI_TRUE
        }
        Err(e) => {
            set_error(e);
            JNI_FALSE
        }
    }
}

/// AWT コンポーネントの描画面を借りて出力先にする（表示済みであること）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nAttach(
    env: JNIEnv,
    _c: JClass,
    component: JObject,
    scale: jfloat,
    px_w: jint,
    px_h: jint,
) -> jboolean {
    let vm = match env.get_java_vm() {
        Ok(vm) => vm,
        Err(e) => {
            set_error(format!("JavaVM を取得できない: {e}"));
            return JNI_FALSE;
        }
    };
    let global = match env.new_global_ref(&component) {
        Ok(g) => g,
        Err(e) => {
            set_error(format!("グローバル参照を作れない: {e}"));
            return JNI_FALSE;
        }
    };

    bool_of(with_engine(|engine| {
        let (surface, w, h, keepalive) =
            platform::attach_on_ui_thread(&vm, &global, scale, px_w as u32, px_h as u32, &engine.gpu.instance)?;

        let caps = surface.get_capabilities(&engine.gpu.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        surface.configure(
            &engine.gpu.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                color_space: wgpu::SurfaceColorSpace::default(),
                width: w,
                height: h,
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            },
        );

        let module = engine
            .gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("gpubridge.blit"),
                source: wgpu::ShaderSource::Wgsl(BLIT_WGSL.into()),
            });
        let blit_pipeline =
            engine
                .gpu
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("gpubridge.blit_pipeline"),
                    layout: None,
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some("fs"),
                        compilation_options: Default::default(),
                        targets: &[Some(format.into())],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                });
        let sampler = engine.gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("gpubridge.blit_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        engine.output = Some(OutputTarget {
            surface,
            blit_pipeline,
            sampler,
            _keepalive: keepalive,
        });
        Ok(())
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nDestroy(_env: JNIEnv, _c: JClass) {
    *ENGINE.lock().unwrap() = None;
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nBackend(env: JNIEnv, _c: JClass) -> jstring {
    let s = ENGINE
        .lock()
        .unwrap()
        .as_ref()
        .map(|e| e.gpu.backend.clone())
        .unwrap_or_else(|| "(未初期化)".into());
    env.new_string(s)
        .map(|j| j.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nLastError(env: JNIEnv, _c: JClass) -> jstring {
    let s = LAST_ERROR.lock().map(|e| e.clone()).unwrap_or_default();
    env.new_string(s)
        .map(|j| j.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ---------------- リソース ----------------

fn format_of(format: jint) -> Result<wgpu::TextureFormat, String> {
    match format {
        0 => Ok(wgpu::TextureFormat::Rgba8Unorm),
        1 => Ok(wgpu::TextureFormat::Rgba32Float),
        2 => Ok(wgpu::TextureFormat::R32Float),
        _ => Err(format!("不正なフォーマット番号: {format}")),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nCreateTexture(
    _env: JNIEnv,
    _c: JClass,
    w: jint,
    h: jint,
    format: jint,
) -> jlong {
    long_of(with_engine(|e| {
        if w <= 0 || h <= 0 {
            return Err("テクスチャの大きさは 1 以上が必要".into());
        }
        Ok(e.create_texture(w as u32, h as u32, format_of(format)?))
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nCreateTextureCube(
    _env: JNIEnv,
    _c: JClass,
    size: jint,
    mips: jint,
    format: jint,
) -> jlong {
    long_of(with_engine(|e| {
        if size <= 0 {
            return Err("テクスチャの大きさは 1 以上が必要".into());
        }
        let max_mips = 32 - (size as u32).leading_zeros();
        if mips <= 0 || mips as u32 > max_mips {
            return Err(format!("ミップ段数は 1〜{max_mips} が必要（大きさ {size}）"));
        }
        Ok(e.create_texture_ex(size as u32, size as u32, format_of(format)?, true, mips as u32))
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nTextureWriteLayerF(
    mut env: JNIEnv,
    _c: JClass,
    id: jlong,
    layer: jint,
    mip: jint,
    data: JFloatArray,
) -> jboolean {
    let bytes = match read_float_array(&mut env, &data) {
        Ok(b) => b,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    bool_of(with_engine(|e| {
        let bpp = match e.texture_format(id as u64) {
            Some(wgpu::TextureFormat::Rgba32Float) => 16,
            Some(wgpu::TextureFormat::R32Float) => 4,
            Some(f) => {
                return Err(format!(
                    "float[] を書けるのは RGBA32F / R32F テクスチャだけです（これは {f:?}）"
                ));
            }
            None => return Err("不正なテクスチャ id".into()),
        };
        if layer < 0 || mip < 0 {
            return Err("面と段は 0 以上が必要".into());
        }
        e.texture_write_layer(id as u64, layer as u32, mip as u32, &bytes, bpp)
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nCreateBuffer(
    _env: JNIEnv,
    _c: JClass,
    element_count: jint,
) -> jlong {
    long_of(with_engine(|e| {
        if element_count <= 0 {
            return Err("要素数は 1 以上が必要".into());
        }
        Ok(e.create_buffer(element_count as u32))
    }))
}

/// ストライド宣言つきのバッファ生成。bind を待たずに write できる。
#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nCreateBufferStrided(
    _env: JNIEnv,
    _c: JClass,
    element_count: jint,
    stride: jint,
) -> jlong {
    long_of(with_engine(|e| {
        if element_count <= 0 {
            return Err("要素数は 1 以上が必要".into());
        }
        if stride <= 0 {
            return Err("ストライドは 1 以上が必要".into());
        }
        e.create_buffer_strided(element_count as u32, stride as u32)
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nCreateUniform(_env: JNIEnv, _c: JClass) -> jlong {
    long_of(with_engine(|e| Ok(e.create_uniform())))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nKernel(
    mut env: JNIEnv,
    _c: JClass,
    wgsl: JString,
    entry: JString,
) -> jlong {
    let (wgsl, entry) = match (jstr(&mut env, &wgsl), jstr(&mut env, &entry)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            set_error(e);
            return 0;
        }
    };
    long_of(with_engine(|e| e.create_kernel(&wgsl, &entry)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nBindingCreate(
    _env: JNIEnv,
    _c: JClass,
    kernel_id: jlong,
) -> jlong {
    long_of(with_engine(|e| e.create_binding(kernel_id as u64)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nBindingSet(
    mut env: JNIEnv,
    _c: JClass,
    binding_id: jlong,
    name: JString,
    resource_id: jlong,
) -> jboolean {
    let name = match jstr(&mut env, &name) {
        Ok(n) => n,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    bool_of(with_engine(|e| {
        e.binding_set(binding_id as u64, &name, resource_id as u64)
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nDispatch(
    _env: JNIEnv,
    _c: JClass,
    binding_id: jlong,
    x: jint,
    y: jint,
    z: jint,
    threads: jboolean,
) -> jboolean {
    bool_of(with_engine(|e| {
        if x <= 0 || y <= 0 || z <= 0 {
            return Err("dispatch のサイズは 1 以上が必要".into());
        }
        e.dispatch(
            binding_id as u64,
            x as u32,
            y as u32,
            z as u32,
            threads == JNI_TRUE,
        )
    }))
}

/// indirect バッファの offset から u32×3 をワークグループ数として dispatch する。
#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nDispatchIndirect(
    _env: JNIEnv,
    _c: JClass,
    binding_id: jlong,
    buffer_id: jlong,
    byte_offset: jlong,
) -> jboolean {
    bool_of(with_engine(|e| {
        if byte_offset < 0 {
            return Err("バイトオフセットは 0 以上が必要".into());
        }
        e.dispatch_indirect(binding_id as u64, buffer_id as u64, byte_offset as u64)
    }))
}

// ---------------- render ----------------

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nRenderer(
    mut env: JNIEnv,
    _c: JClass,
    wgsl: JString,
    vs_entry: JString,
    fs_entry: JString,
) -> jlong {
    let (wgsl, vs, fs) = match (
        jstr(&mut env, &wgsl),
        jstr(&mut env, &vs_entry),
        jstr(&mut env, &fs_entry),
    ) {
        (Ok(a), Ok(b), Ok(c)) => (a, b, c),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            set_error(e);
            return 0;
        }
    };
    long_of(with_engine(|e| e.create_renderer(&wgsl, &vs, &fs)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nRendererConfig(
    _env: JNIEnv,
    _c: JClass,
    id: jlong,
    topology: jint,
    blend: jint,
    depth_test: jboolean,
) -> jboolean {
    bool_of(with_engine(|e| {
        e.renderer_config(
            id as u64,
            topology as u8,
            blend as u8,
            depth_test == JNI_TRUE,
        )
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nRenderBindingCreate(
    _env: JNIEnv,
    _c: JClass,
    renderer_id: jlong,
) -> jlong {
    long_of(with_engine(|e| e.create_render_binding(renderer_id as u64)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nRenderBindingSet(
    mut env: JNIEnv,
    _c: JClass,
    binding_id: jlong,
    name: JString,
    resource_id: jlong,
) -> jboolean {
    let name = match jstr(&mut env, &name) {
        Ok(n) => n,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    bool_of(with_engine(|e| {
        e.render_binding_set(binding_id as u64, &name, resource_id as u64)
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nDraw(
    _env: JNIEnv,
    _c: JClass,
    binding_id: jlong,
    tex_id: jlong,
    vertex_count: jint,
    instance_count: jint,
    sx: jint,
    sy: jint,
    sw: jint,
    sh: jint,
) -> jboolean {
    bool_of(with_engine(|e| {
        if vertex_count <= 0 || instance_count <= 0 {
            return Err("draw の頂点数・インスタンス数は 1 以上が必要".into());
        }
        let scissor = (sw >= 0).then_some([sx, sy, sw, sh]);
        e.draw(
            binding_id as u64,
            tex_id as u64,
            vertex_count as u32,
            instance_count as u32,
            scissor,
        )
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nDrawIndexed(
    _env: JNIEnv,
    _c: JClass,
    binding_id: jlong,
    tex_id: jlong,
    index_id: jlong,
    first_index: jint,
    index_count: jint,
    instance_count: jint,
    sx: jint,
    sy: jint,
    sw: jint,
    sh: jint,
) -> jboolean {
    bool_of(with_engine(|e| {
        if index_count <= 0 || instance_count <= 0 {
            return Err("draw のインデックス数・インスタンス数は 1 以上が必要".into());
        }
        if first_index < 0 {
            return Err("firstIndex は 0 以上が必要".into());
        }
        let scissor = (sw >= 0).then_some([sx, sy, sw, sh]);
        e.draw_indexed(
            binding_id as u64,
            tex_id as u64,
            index_id as u64,
            first_index as u32,
            index_count as u32,
            instance_count as u32,
            scissor,
        )
    }))
}

/// indirect バッファの offset から u32×5 を引数として drawIndexed する。
#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nDrawIndexedIndirect(
    _env: JNIEnv,
    _c: JClass,
    binding_id: jlong,
    tex_id: jlong,
    index_id: jlong,
    indirect_id: jlong,
    indirect_offset: jlong,
    sx: jint,
    sy: jint,
    sw: jint,
    sh: jint,
) -> jboolean {
    bool_of(with_engine(|e| {
        if indirect_offset < 0 {
            return Err("バイトオフセットは 0 以上が必要".into());
        }
        let scissor = (sw >= 0).then_some([sx, sy, sw, sh]);
        e.draw_indexed_indirect(
            binding_id as u64,
            tex_id as u64,
            index_id as u64,
            indirect_id as u64,
            indirect_offset as u64,
            scissor,
        )
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nClear(
    _env: JNIEnv,
    _c: JClass,
    tex_id: jlong,
    r: jfloat,
    g: jfloat,
    b: jfloat,
    a: jfloat,
) -> jboolean {
    bool_of(with_engine(|e| {
        e.clear(tex_id as u64, r as f64, g as f64, b as f64, a as f64)
    }))
}

// ---------------- uniform ----------------

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nUniformSetF(
    mut env: JNIEnv,
    _c: JClass,
    uid: jlong,
    member: JString,
    values: JFloatArray,
) -> jboolean {
    let member = match jstr(&mut env, &member) {
        Ok(m) => m,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    let len = match env.get_array_length(&values) {
        Ok(l) => l as usize,
        Err(e) => {
            set_error(format!("配列長の取得に失敗: {e}"));
            return JNI_FALSE;
        }
    };
    let mut buf = vec![0f32; len];
    if let Err(e) = env.get_float_array_region(&values, 0, &mut buf) {
        set_error(format!("配列の読み取りに失敗: {e}"));
        return JNI_FALSE;
    }
    bool_of(with_engine(|e| {
        e.uniform_set(uid as u64, &member, &buf, None)
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nUniformSetI(
    mut env: JNIEnv,
    _c: JClass,
    uid: jlong,
    member: JString,
    value: jint,
) -> jboolean {
    let member = match jstr(&mut env, &member) {
        Ok(m) => m,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    bool_of(with_engine(|e| {
        e.uniform_set(uid as u64, &member, &[], Some(value))
    }))
}

// ---------------- buffer / texture I/O ----------------

fn read_float_array(env: &mut JNIEnv, arr: &JFloatArray) -> Result<Vec<u8>, String> {
    let len = env
        .get_array_length(arr)
        .map_err(|e| format!("配列長の取得に失敗: {e}"))? as usize;
    let mut floats = vec![0f32; len];
    env.get_float_array_region(arr, 0, &mut floats)
        .map_err(|e| format!("配列の読み取りに失敗: {e}"))?;
    let mut bytes = Vec::with_capacity(len * 4);
    for f in floats {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    Ok(bytes)
}

fn read_int_array(env: &mut JNIEnv, arr: &JIntArray) -> Result<Vec<i32>, String> {
    let len = env
        .get_array_length(arr)
        .map_err(|e| format!("配列長の取得に失敗: {e}"))? as usize;
    let mut ints = vec![0i32; len];
    env.get_int_array_region(arr, 0, &mut ints)
        .map_err(|e| format!("配列の読み取りに失敗: {e}"))?;
    Ok(ints)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nBufferWriteF(
    mut env: JNIEnv,
    _c: JClass,
    id: jlong,
    element_offset: jint,
    data: JFloatArray,
) -> jboolean {
    let bytes = match read_float_array(&mut env, &data) {
        Ok(b) => b,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    bool_of(with_engine(|e| e.buffer_write(id as u64, element_offset as u32, &bytes)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nBufferWriteI(
    mut env: JNIEnv,
    _c: JClass,
    id: jlong,
    element_offset: jint,
    data: JIntArray,
) -> jboolean {
    let ints = match read_int_array(&mut env, &data) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    let mut bytes = Vec::with_capacity(ints.len() * 4);
    for v in ints {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bool_of(with_engine(|e| e.buffer_write(id as u64, element_offset as u32, &bytes)))
}

/// 配列の [from, from+count) を Vec<u8>（LE）にして返す。範囲は呼び出し側で検証済みの前提にせず、ここで検証する。
fn check_range(len: usize, from: jint, count: jint) -> Result<(), String> {
    if from < 0 || count < 0 || (from as usize) + (count as usize) > len {
        return Err(format!(
            "配列の範囲指定が不正です: from={from}, count={count}（配列長 {len}）"
        ));
    }
    Ok(())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nBufferWriteBytesF(
    env: JNIEnv,
    _c: JClass,
    id: jlong,
    byte_offset: jlong,
    data: JFloatArray,
    from: jint,
    count: jint,
) -> jboolean {
    let bytes = (|| -> Result<Vec<u8>, String> {
        let len = env
            .get_array_length(&data)
            .map_err(|e| format!("配列長の取得に失敗: {e}"))? as usize;
        check_range(len, from, count)?;
        let mut floats = vec![0f32; count as usize];
        env.get_float_array_region(&data, from, &mut floats)
            .map_err(|e| format!("配列の読み取りに失敗: {e}"))?;
        let mut bytes = Vec::with_capacity(floats.len() * 4);
        for f in floats {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        Ok(bytes)
    })();
    let bytes = match bytes {
        Ok(b) => b,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    bool_of(with_engine(|e| {
        if byte_offset < 0 {
            return Err("バイトオフセットは 0 以上が必要".into());
        }
        e.buffer_write_bytes(id as u64, byte_offset as u64, &bytes)
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nBufferWriteBytesI(
    env: JNIEnv,
    _c: JClass,
    id: jlong,
    byte_offset: jlong,
    data: JIntArray,
    from: jint,
    count: jint,
) -> jboolean {
    let bytes = (|| -> Result<Vec<u8>, String> {
        let len = env
            .get_array_length(&data)
            .map_err(|e| format!("配列長の取得に失敗: {e}"))? as usize;
        check_range(len, from, count)?;
        let mut ints = vec![0i32; count as usize];
        env.get_int_array_region(&data, from, &mut ints)
            .map_err(|e| format!("配列の読み取りに失敗: {e}"))?;
        let mut bytes = Vec::with_capacity(ints.len() * 4);
        for v in ints {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        Ok(bytes)
    })();
    let bytes = match bytes {
        Ok(b) => b,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    bool_of(with_engine(|e| {
        if byte_offset < 0 {
            return Err("バイトオフセットは 0 以上が必要".into());
        }
        e.buffer_write_bytes(id as u64, byte_offset as u64, &bytes)
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nBufferSetF(
    mut env: JNIEnv,
    _c: JClass,
    id: jlong,
    index: jint,
    member: JString,
    values: JFloatArray,
) -> jboolean {
    let member = match jstr(&mut env, &member) {
        Ok(m) => m,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    let len = match env.get_array_length(&values) {
        Ok(l) => l as usize,
        Err(e) => {
            set_error(format!("配列長の取得に失敗: {e}"));
            return JNI_FALSE;
        }
    };
    let mut buf = vec![0f32; len];
    if let Err(e) = env.get_float_array_region(&values, 0, &mut buf) {
        set_error(format!("配列の読み取りに失敗: {e}"));
        return JNI_FALSE;
    }
    bool_of(with_engine(|e| {
        e.buffer_set(id as u64, index as u32, &member, &buf, None)
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nBufferSetI(
    mut env: JNIEnv,
    _c: JClass,
    id: jlong,
    index: jint,
    member: JString,
    value: jint,
) -> jboolean {
    let member = match jstr(&mut env, &member) {
        Ok(m) => m,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    bool_of(with_engine(|e| {
        e.buffer_set(id as u64, index as u32, &member, &[], Some(value))
    }))
}

/// バッファ間の GPU コピー（記録コマンド）。
#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nCopyBufferToBuffer(
    _env: JNIEnv,
    _c: JClass,
    src_id: jlong,
    src_offset: jlong,
    dst_id: jlong,
    dst_offset: jlong,
    byte_count: jlong,
) -> jboolean {
    bool_of(with_engine(|e| {
        if src_offset < 0 || dst_offset < 0 || byte_count < 0 {
            return Err("オフセット・バイト数は 0 以上が必要".into());
        }
        e.copy_buffer_to_buffer(
            src_id as u64,
            src_offset as u64,
            dst_id as u64,
            dst_offset as u64,
            byte_count as u64,
        )
    }))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nBufferReadF(
    env: JNIEnv,
    _c: JClass,
    id: jlong,
    dst: JFloatArray,
) -> jboolean {
    let len = match env.get_array_length(&dst) {
        Ok(l) => l as usize,
        Err(e) => {
            set_error(format!("配列長の取得に失敗: {e}"));
            return JNI_FALSE;
        }
    };
    let mut bytes = vec![0u8; len * 4];
    if with_engine(|e| e.buffer_read(id as u64, &mut bytes)).is_none() {
        return JNI_FALSE;
    }
    let floats: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    if let Err(e) = env.set_float_array_region(&dst, 0, &floats) {
        set_error(format!("配列への書き戻しに失敗: {e}"));
        return JNI_FALSE;
    }
    JNI_TRUE
}

/// ARGB (Processing の pixels 形式) → RGBA8 テクスチャ
#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nTextureWriteI(
    mut env: JNIEnv,
    _c: JClass,
    id: jlong,
    argb: JIntArray,
) -> jboolean {
    let ints = match read_int_array(&mut env, &argb) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    bool_of(with_engine(|e| {
        if e.texture_format(id as u64) != Some(wgpu::TextureFormat::Rgba8Unorm) {
            return Err(
                "int[] (ARGB) を書けるのは RGBA8 テクスチャだけです。float 系は write(float[]) を使ってください"
                    .into(),
            );
        }
        let mut bytes = Vec::with_capacity(ints.len() * 4);
        for p in &ints {
            let p = *p as u32;
            bytes.push(((p >> 16) & 0xff) as u8); // R
            bytes.push(((p >> 8) & 0xff) as u8); // G
            bytes.push((p & 0xff) as u8); // B
            bytes.push(((p >> 24) & 0xff) as u8); // A
        }
        e.texture_write(id as u64, &bytes, 4)
    }))
}

/// float 列 → RGBA32F (16 bytes/px) / R32F (4 bytes/px) テクスチャ
#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nTextureWriteF(
    mut env: JNIEnv,
    _c: JClass,
    id: jlong,
    data: JFloatArray,
) -> jboolean {
    let bytes = match read_float_array(&mut env, &data) {
        Ok(b) => b,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    bool_of(with_engine(|e| {
        let bpp = match e.texture_format(id as u64) {
            Some(wgpu::TextureFormat::Rgba32Float) => 16,
            Some(wgpu::TextureFormat::R32Float) => 4,
            Some(f) => {
                return Err(format!(
                    "float[] を書けるのは RGBA32F / R32F テクスチャだけです（これは {f:?}）"
                ));
            }
            None => return Err("不正なテクスチャ id".into()),
        };
        e.texture_write(id as u64, &bytes, bpp)
    }))
}

// ---------------- 画面出力 ----------------

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nShow(
    _env: JNIEnv,
    _c: JClass,
    tex_id: jlong,
) -> jboolean {
    bool_of(with_engine(|e| e.show(tex_id as u64)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nSubmit(_env: JNIEnv, _c: JClass) -> jboolean {
    bool_of(with_engine(|e| e.submit()))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nSetLabel(
    mut env: JNIEnv,
    _c: JClass,
    id: jlong,
    label: JString,
) -> jboolean {
    let label = match jstr(&mut env, &label) {
        Ok(s) => s,
        Err(e) => {
            set_error(e);
            return JNI_FALSE;
        }
    };
    bool_of(with_engine(|e| e.set_label(id as u64, &label)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nProfile(_env: JNIEnv, _c: JClass, on: jboolean) -> jboolean {
    bool_of(with_engine(|e| e.profile_enable(on != JNI_FALSE)))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nProfileReport(env: JNIEnv, _c: JClass) -> jstring {
    let s = ENGINE
        .lock()
        .unwrap()
        .as_ref()
        .map(|e| e.profile_report())
        .unwrap_or_default();
    env.new_string(s)
        .map(|j| j.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// ウィンドウリサイズへの追従。Animation Thread（submit と同じスレッド）から呼ぶこと。
#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nResize(
    _env: JNIEnv,
    _c: JClass,
    scale: jfloat,
    px_w: jint,
    px_h: jint,
) -> jboolean {
    bool_of(with_engine(|e| {
        if px_w <= 0 || px_h <= 0 {
            return Err("リサイズ後の大きさが不正です".into());
        }
        e.resize_surface(scale, px_w as u32, px_h as u32)
    }))
}

/// リソース（texture / buffer / uniform）の個別解放。
#[unsafe(no_mangle)]
pub extern "system" fn Java_gpubridge_GpuBridge_nRelease(
    _env: JNIEnv,
    _c: JClass,
    id: jlong,
) -> jboolean {
    bool_of(with_engine(|e| e.release_resource(id as u64)))
}
