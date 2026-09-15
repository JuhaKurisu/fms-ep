//! リソース管理と実行エンジン。
//!
//! このファイルの前半は GPU に依存しない検証ロジック（純関数）で、
//! cargo test で単体テストできる。binding / uniform の突合はすべてここを通る。

use crate::reflect::{
    BindingInfo, BindingKind, MemberKind, ShaderInfo, StructLayout, demangle, resolve_name, suggest,
};
use std::collections::{BTreeSet, HashMap};

/// Java 側リソースの内部記述（検証に必要な情報だけ）。
pub enum ResourceDesc {
    Uniform,
    Buffer,
    Texture {
        format: wgpu::TextureFormat,
        cube: bool,
    },
}

/// `binding.set(name, resource)` の突合。成功なら WGSL 側の binding 番号を返す。
///
/// 宣言済みであればこのエントリで未使用の binding への set も受け付ける
/// （シェーダー実装途中でも Java 側コードが落ちないように。dispatch では
/// used_bindings に含まれないため単に無視される）。型チェックは常に行う。
pub fn check_set(
    bindings: &[BindingInfo],
    name: &str,
    res: &ResourceDesc,
) -> Result<u32, String> {
    let all_names: Vec<&str> = bindings.iter().map(|b| b.name.as_str()).collect();
    let target = match resolve_name(&all_names, name) {
        Ok(i) => &bindings[i],
        Err(Some(ambiguous)) => return Err(ambiguous),
        Err(None) => {
            // 候補はマングル前の名前で提示する
            let shown: Vec<&str> = all_names.iter().map(|n| demangle(n)).collect();
            return Err(suggest(name, &shown));
        }
    };

    match (&target.kind, res) {
        (BindingKind::Uniform { .. }, ResourceDesc::Uniform) => Ok(target.binding),
        (BindingKind::Storage { .. }, ResourceDesc::Buffer) => Ok(target.binding),
        (BindingKind::StorageTexture { format, .. }, ResourceDesc::Texture { format: have, .. }) => {
            if format == have {
                Ok(target.binding)
            } else {
                Err(format!(
                    "'{name}' のフォーマットが合いません: WGSL は {format:?}、渡されたテクスチャは {have:?}"
                ))
            }
        }
        (BindingKind::SampledTexture { cube }, ResourceDesc::Texture { cube: have, .. }) => {
            if cube == have {
                Ok(target.binding)
            } else if *cube {
                Err(format!("'{name}' は texture_cube です（gpu.textureCube で作ったテクスチャを渡してください）"))
            } else {
                Err(format!("'{name}' は 2D の texture です（キューブマップは渡せません）"))
            }
        }
        (kind, _) => Err(format!(
            "'{name}' の種別が合いません: WGSL 側は {} です",
            kind_name(kind)
        )),
    }
}

fn kind_name(kind: &BindingKind) -> &'static str {
    match kind {
        BindingKind::Uniform { .. } => "uniform（GpuUniform を渡してください）",
        BindingKind::Storage { .. } => "storage buffer（GpuBuffer を渡してください）",
        BindingKind::StorageTexture { .. } | BindingKind::SampledTexture { .. } => {
            "texture（GpuTexture を渡してください）"
        }
        BindingKind::Sampler => "sampler（自動供給されるため set 不要です）",
    }
}

/// dispatch / draw 前の未バインド検査。`bound` は binding 番号 → リソース id。
pub fn check_complete(
    entry_name: &str,
    used: &[usize],
    bindings: &[BindingInfo],
    bound: &HashMap<u32, u64>,
) -> Result<(), String> {
    let missing: Vec<&str> = used
        .iter()
        .map(|&i| &bindings[i])
        .filter(|b| !matches!(b.kind, BindingKind::Sampler)) // sampler は自動供給
        .filter(|b| !bound.contains_key(&b.binding))
        .map(|b| b.name.as_str())
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "'{}' の実行に必要な binding が未指定です: {}",
            entry_name,
            missing.join(", ")
        ))
    }
}

/// renderer(vs, fs) のエントリ解決と、両ステージの used binding の合成。
///
/// 戻り値: (render_entries 内の vs index, fs index,
///          マージ済み used binding の (ShaderInfo.bindings への index, visibility) 列)。
/// WebGPU の制約で、vertex stage から見える storage buffer は読み取り専用のみ。
pub fn check_render_pair(
    info: &ShaderInfo,
    vs: &str,
    fs: &str,
) -> Result<(usize, usize, Vec<(usize, wgpu::ShaderStages)>), String> {
    let find = |want: &str, stage: naga::ShaderStage, label: &str| -> Result<usize, String> {
        info.render_entries
            .iter()
            .position(|e| e.stage == stage && e.name == want)
            .ok_or_else(|| {
                let names: Vec<&str> = info
                    .render_entries
                    .iter()
                    .filter(|e| e.stage == stage)
                    .map(|e| e.name.as_str())
                    .collect();
                format!("{label} エントリポイント {}", suggest(want, &names))
            })
    };
    let vs_index = find(vs, naga::ShaderStage::Vertex, "vertex")?;
    let fs_index = find(fs, naga::ShaderStage::Fragment, "fragment")?;

    // vertex 側で read_write の storage buffer は WebGPU で使えない
    for &i in &info.render_entries[vs_index].used_bindings {
        if let BindingKind::Storage { read_only: false, .. } = info.bindings[i].kind {
            return Err(format!(
                "'{}' は read_write の storage buffer なので vertex シェーダーから使えません。\
                 読み取り専用（Slang では RWStructuredBuffer ではなく StructuredBuffer）にしてください",
                crate::reflect::demangle(&info.bindings[i].name)
            ));
        }
    }

    // visibility の合成（vs → VERTEX, fs → FRAGMENT, 両方 → OR）
    let mut used: Vec<(usize, wgpu::ShaderStages)> = Vec::new();
    let mut add = |indices: &[usize], stage: wgpu::ShaderStages| {
        for &i in indices {
            match used.iter_mut().find(|(j, _)| *j == i) {
                Some((_, v)) => *v |= stage,
                None => used.push((i, stage)),
            }
        }
    };
    add(&info.render_entries[vs_index].used_bindings, wgpu::ShaderStages::VERTEX);
    add(&info.render_entries[fs_index].used_bindings, wgpu::ShaderStages::FRAGMENT);

    Ok((vs_index, fs_index, used))
}

/// `uniform.set(member, ...)` / `buffer.at(i).set(member, ...)` の突合。
/// 成功なら struct 先頭からの (バイトオフセット, バイト長)。
/// `float_count` は渡された float の個数（int の場合は 1 で is_int=true）。
pub fn check_member_set(
    layout: &StructLayout,
    member: &str,
    float_count: usize,
    is_int: bool,
) -> Result<(u32, u32), String> {
    let names: Vec<&str> = layout.members.iter().map(|m| m.name.as_str()).collect();
    let m = match resolve_name(&names, member) {
        Ok(i) => &layout.members[i],
        Err(Some(ambiguous)) => return Err(ambiguous),
        Err(None) => {
            let shown: Vec<&str> = names.iter().map(|n| demangle(n)).collect();
            return Err(suggest(member, &shown));
        }
    };
    match m.kind {
        MemberKind::F32 => {
            if is_int {
                Err(format!(
                    "'{member}' は f32 です。float で渡してください"
                ))
            } else if float_count != 1 {
                Err(format!(
                    "'{member}' は f32 なので値は 1 個です（{float_count} 個渡されました）"
                ))
            } else {
                Ok((m.offset, 4))
            }
        }
        MemberKind::I32 | MemberKind::U32 => {
            if !is_int {
                Err(format!(
                    "'{member}' は整数型（{:?}）です。int で渡してください",
                    m.kind
                ))
            } else {
                Ok((m.offset, 4))
            }
        }
        MemberKind::VecF(n) => {
            if is_int {
                Err(format!("'{member}' は vec{n}<f32> です。float で渡してください"))
            } else if float_count != n as usize {
                Err(format!(
                    "'{member}' は vec{n}<f32> なので値は {n} 個です（{float_count} 個渡されました）"
                ))
            } else {
                Ok((m.offset, 4 * n as u32))
            }
        }
        MemberKind::Mat4 => {
            if is_int {
                Err(format!("'{member}' は mat4x4<f32> です。float で渡してください"))
            } else if float_count != 16 {
                Err(format!(
                    "'{member}' は mat4x4<f32> なので値は 16 個（列優先）です（{float_count} 個渡されました）"
                ))
            } else {
                Ok((m.offset, 64))
            }
        }
    }
}

/// bind 前（レイアウト未確定）に set された値。レイアウトが決まった時点で検証して適用する。
pub enum PendingValue {
    F(Vec<f32>),
    I(i32),
}

impl PendingValue {
    fn new(floats: &[f32], int_value: Option<i32>) -> PendingValue {
        match int_value {
            Some(v) => PendingValue::I(v),
            None => PendingValue::F(floats.to_vec()),
        }
    }

    fn parts(&self) -> (&[f32], Option<i32>) {
        match self {
            PendingValue::F(v) => (v.as_slice(), None),
            PendingValue::I(v) => (&[], Some(*v)),
        }
    }
}

/// check_member_set を通してから bytes の base 位置以降へリトルエンディアンで書き込む。
fn write_member(
    layout: &StructLayout,
    bytes: &mut [u8],
    base: usize,
    member: &str,
    floats: &[f32],
    int_value: Option<i32>,
) -> Result<(), String> {
    let is_int = int_value.is_some();
    let count = if is_int { 1 } else { floats.len() };
    let (offset, _len) = check_member_set(layout, member, count, is_int)?;
    let off = base + offset as usize;
    if let Some(v) = int_value {
        bytes[off..off + 4].copy_from_slice(&v.to_le_bytes());
    } else {
        for (i, f) in floats.iter().enumerate() {
            bytes[off + i * 4..off + i * 4 + 4].copy_from_slice(&f.to_le_bytes());
        }
    }
    Ok(())
}

/// bind 前に `at(i).set()` された値を、レイアウト確定後のミラーへ適用する。
fn apply_pending_buffer(
    layout: Option<&StructLayout>,
    stride: u32,
    element_count: u32,
    mirror: &mut Vec<u8>,
    dirty: &mut BTreeSet<u32>,
    pending: &mut Vec<(u32, String, PendingValue)>,
) -> Result<(), String> {
    if pending.is_empty() {
        return Ok(());
    }
    let Some(layout) = layout else {
        return Err(
            "この GpuBuffer の要素は struct ではないか、メンバ名で指定できない型を含んでいます。write(float[]) を使ってください"
                .into(),
        );
    };
    if mirror.is_empty() {
        *mirror = vec![0u8; element_count as usize * stride as usize];
    }
    for (index, member, v) in std::mem::take(pending) {
        let (floats, int_value) = v.parts();
        write_member(layout, mirror, index as usize * stride as usize, &member, floats, int_value)
            .map_err(|e| format!("bind 前に at({index}).set() されていたメンバの適用に失敗: {e}"))?;
        dirty.insert(index);
    }
    Ok(())
}

/// dirty な要素番号を、連続した (開始要素, 要素数) の並びにまとめる。
/// 1 run = write_buffer 1 回。連番で埋めた場合は 1 回に畳まれる。
pub fn dirty_runs(dirty: &BTreeSet<u32>) -> Vec<(u32, u32)> {
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for &i in dirty {
        match runs.last_mut() {
            Some((start, count)) if *start + *count == i => *count += 1,
            _ => runs.push((i, 1)),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reflect::{reflect, EntryInfo};

    #[test]
    fn dirty_runs_coalesces_consecutive_elements() {
        let set = |v: &[u32]| v.iter().copied().collect::<BTreeSet<u32>>();
        assert_eq!(dirty_runs(&set(&[])), vec![]);
        assert_eq!(dirty_runs(&set(&[0, 1, 2, 3])), vec![(0, 4)]);
        assert_eq!(dirty_runs(&set(&[5])), vec![(5, 1)]);
        // 飛び飛びは run が分かれる（間に挟まった未編集の要素を潰さないため）
        assert_eq!(dirty_runs(&set(&[0, 1, 5, 6, 7, 9])), vec![(0, 2), (5, 3), (9, 1)]);
        // 挿入順に関係なく昇順にまとまる
        assert_eq!(dirty_runs(&set(&[3, 1, 2])), vec![(1, 3)]);
    }

    const SRC: &str = r#"
struct Params { t: f32, gravity: vec2<f32>, n: u32 };
struct Boid { pos: vec2<f32>, vel: vec2<f32> };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> src: array<Boid>;
@group(0) @binding(2) var<storage, read_write> dst: array<Boid>;
@group(0) @binding(3) var canvas: texture_storage_2d<rgba8unorm, write>;
fn helper() -> f32 { return params.t; }
@compute @workgroup_size(64)
fn update(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= arrayLength(&src)) { return; }
  dst[id.x].pos = src[id.x].pos + src[id.x].vel * helper();
}
@compute @workgroup_size(8, 8, 1)
fn splat(@builtin(global_invocation_id) id: vec3<u32>) {
  textureStore(canvas, vec2<i32>(id.xy), vec4<f32>(1.0));
}
"#;

    const RENDER_SRC: &str = r#"
struct Params { size: vec2<f32> };
struct Boid { pos: vec2<f32>, vel: vec2<f32> };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> boids: array<Boid>;
struct VOut { @builtin(position) pos: vec4<f32>, @location(0) local: vec2<f32> };
@vertex
fn vsMain(@builtin(vertex_index) vid: u32, @builtin(instance_index) iid: u32) -> VOut {
  var o: VOut;
  o.pos = vec4<f32>(boids[iid].pos / params.size, 0.0, 1.0);
  o.local = vec2<f32>(0.0);
  return o;
}
@fragment
fn fsMain(v: VOut) -> @location(0) vec4<f32> {
  return vec4<f32>(v.local, params.size.x * 0.0, 1.0);
}
"#;

    fn entry<'a>(info: &'a crate::reflect::ShaderInfo, name: &str) -> &'a EntryInfo {
        info.entry_points.iter().find(|e| e.name == name).unwrap()
    }

    #[test]
    fn set_unknown_name_suggests() {
        let info = reflect(SRC).unwrap();
        let err = check_set(
            &info.bindings,
            "srcc",
            &ResourceDesc::Buffer,
        )
        .unwrap_err();
        assert!(err.contains("srcc"));
        assert!(err.contains("src")); // 候補提示
    }

    #[test]
    fn set_type_mismatch_texture_for_buffer() {
        let info = reflect(SRC).unwrap();
        let err = check_set(
            &info.bindings,
            "src",
            &ResourceDesc::Texture { format: wgpu::TextureFormat::Rgba8Unorm, cube: false },
        )
        .unwrap_err();
        assert!(err.contains("GpuBuffer"));
    }

    #[test]
    fn set_format_mismatch_storage_texture() {
        let info = reflect(SRC).unwrap();
        let err = check_set(
            &info.bindings,
            "canvas",
            &ResourceDesc::Texture { format: wgpu::TextureFormat::Rgba32Float, cube: false },
        )
        .unwrap_err();
        assert!(err.contains("Rgba8Unorm"));
    }

    #[test]
    fn set_declared_but_unused_by_entry_is_accepted() {
        // シェーダー実装途中でも Java 側コードが安定するよう、
        // 宣言済みならエントリ未使用でも set は成功する（dispatch では単に無視される）
        let info = reflect(SRC).unwrap();
        let b = check_set(
            &info.bindings,
            "src",
            &ResourceDesc::Buffer,
        )
        .unwrap();
        assert_eq!(b, 1);
    }

    #[test]
    fn set_declared_but_unused_still_type_checks() {
        // 未使用でも型違いはエラーにする
        let info = reflect(SRC).unwrap();
        let err = check_set(
            &info.bindings,
            "src",
            &ResourceDesc::Texture { format: wgpu::TextureFormat::Rgba8Unorm, cube: false },
        )
        .unwrap_err();
        assert!(err.contains("GpuBuffer"));
    }

    #[test]
    fn set_ok_returns_binding_number() {
        let info = reflect(SRC).unwrap();
        let b = check_set(
            &info.bindings,
            "dst",
            &ResourceDesc::Buffer,
        )
        .unwrap();
        assert_eq!(b, 2);
    }

    #[test]
    fn complete_reports_missing() {
        let info = reflect(SRC).unwrap();
        let e = entry(&info, "update");
        let mut bound = HashMap::new();
        bound.insert(0u32, 1u64); // params だけ束ねた
        let err = check_complete(&e.name, &e.used_bindings, &info.bindings, &bound).unwrap_err();
        assert!(err.contains("src") && err.contains("dst"));
    }

    #[test]
    fn complete_ok() {
        let info = reflect(SRC).unwrap();
        let e = entry(&info, "update");
        let mut bound = HashMap::new();
        bound.insert(0u32, 1u64);
        bound.insert(1u32, 2u64);
        bound.insert(2u32, 3u64);
        assert!(check_complete(&e.name, &e.used_bindings, &info.bindings, &bound).is_ok());
    }

    #[test]
    fn set_resolves_mangled_names() {
        // Slang 生成風のマングル名を含む WGSL
        let src = r#"
struct P_std140_0 { feed_0: f32, n_0: u32 };
@group(0) @binding(0) var<uniform> params_0: P_std140_0;
@group(0) @binding(1) var<storage, read_write> buf_0: array<f32>;
@compute @workgroup_size(64)
fn run(@builtin(global_invocation_id) id: vec3<u32>) { buf_0[id.x] = params_0.feed_0; }
"#;
        let info = reflect(src).unwrap();
        let _e = &info.entry_points[0];
        // マングル前の名前で解決できる
        assert!(check_set(&info.bindings, "params", &ResourceDesc::Uniform).is_ok());
        assert!(check_set(&info.bindings, "buf", &ResourceDesc::Buffer).is_ok());
        // 完全一致（マングル名そのまま）も通る
        assert!(check_set(&info.bindings, "buf_0", &ResourceDesc::Buffer).is_ok());
        // 候補提示はマングル前の名前
        let err = check_set(&info.bindings, "buff", &ResourceDesc::Buffer).unwrap_err();
        assert!(err.contains("buf") && !err.contains("buf_0"));
        // uniform メンバもマングル前の名前で
        let BindingKind::Uniform { layout } = &info.bindings[0].kind else { panic!() };
        assert!(check_member_set(layout, "feed", 1, false).is_ok());
        assert!(check_member_set(layout, "n", 1, true).is_ok());
    }

    #[test]
    fn preserve_params_style_wgsl_accepts_unused_set() {
        // slangc -preserve-params が出す形: マングル名 + エントリ未使用の storage buffer。
        // set は通り、dispatch（check_complete）は使用中の binding だけで成立する。
        let src = r#"
struct Params_std140_0 { @align(16) viewWidth_0: u32, @align(4) viewHeight_0: u32 };
struct Object_std430_0 { @align(4) type_0: i32 };
@binding(0) @group(0) var<uniform> params_0: Params_std140_0;
@binding(2) @group(0) var view_0: texture_storage_2d<rgba8unorm, write>;
@binding(1) @group(0) var<storage, read_write> objectsBuffer_0: array<Object_std430_0>;
@compute @workgroup_size(8, 8, 1)
fn rendering(@builtin(global_invocation_id) id: vec3<u32>) {
  if (id.x >= params_0.viewWidth_0) { return; }
  textureStore(view_0, vec2<i32>(id.xy), vec4<f32>(1.0));
}
"#;
        let info = reflect(src).unwrap();
        let b = check_set(&info.bindings, "objectsBuffer", &ResourceDesc::Buffer).unwrap();
        assert_eq!(b, 1);

        let e = entry(&info, "rendering");
        let mut bound = HashMap::new();
        for &i in &e.used_bindings {
            bound.insert(info.bindings[i].binding, 1u64);
        }
        assert!(!bound.contains_key(&b)); // objectsBuffer は未使用
        assert!(check_complete(&e.name, &e.used_bindings, &info.bindings, &bound).is_ok());
    }

    #[test]
    fn uniform_set_checks_name_type_count() {
        let info = reflect(SRC).unwrap();
        let BindingKind::Uniform { layout } = &info
            .bindings
            .iter()
            .find(|b| b.name == "params")
            .unwrap()
            .kind
        else {
            panic!()
        };
        assert_eq!(check_member_set(layout, "t", 1, false).unwrap(), (0, 4));
        assert_eq!(check_member_set(layout, "gravity", 2, false).unwrap(), (8, 8));
        assert!(check_member_set(layout, "t", 2, false).is_err()); // 個数違い
        assert!(check_member_set(layout, "n", 1, false).is_err()); // u32 に float
        assert!(check_member_set(layout, "n", 1, true).is_ok());
        assert!(check_member_set(layout, "feedd", 1, false).is_err()); // 名前なし
    }

    #[test]
    fn uniform_set_checks_matrix_member() {
        let src = r#"
struct MatStorage { @align(16) data: array<vec4<f32>, 4> };
struct P { @align(16) vp: MatStorage };
@group(0) @binding(0) var<uniform> params: P;
@compute @workgroup_size(1) fn main() { let x = params.vp.data[0]; }
"#;
        let info = reflect(src).unwrap();
        let BindingKind::Uniform { layout } = &info.bindings[0].kind else {
            panic!()
        };
        assert_eq!(check_member_set(layout, "vp", 16, false).unwrap(), (0, 64));
        assert!(check_member_set(layout, "vp", 4, false).is_err()); // 個数違い
        assert!(check_member_set(layout, "vp", 1, true).is_err()); // int は不可
    }

    fn params_layout() -> StructLayout {
        let info = reflect(SRC).unwrap();
        let BindingKind::Uniform { layout } = &info
            .bindings
            .iter()
            .find(|b| b.name == "params")
            .unwrap()
            .kind
        else {
            panic!()
        };
        layout.clone()
    }

    #[test]
    fn uniform_set_before_bind_is_applied_on_bind() {
        let layout = params_layout();
        let mut u = UniformState::new();
        // bind 前（layout 未確定）の set は pending に積まれて成功する
        u.set_member("t", &[1.5], None).unwrap();
        u.set_member("n", &[], Some(7)).unwrap();
        u.set_member("t", &[2.5], None).unwrap(); // 同じメンバは後の set が勝つ

        // bind でレイアウト確定 → pending が staging に適用される
        u.staging = vec![0u8; layout.size as usize];
        u.layout = Some(layout);
        u.dirty = false;
        u.apply_pending().unwrap();
        assert_eq!(f32::from_le_bytes(u.staging[0..4].try_into().unwrap()), 2.5); // t @0
        assert_eq!(i32::from_le_bytes(u.staging[16..20].try_into().unwrap()), 7); // n @16
        assert!(u.pending.is_empty());
        assert!(u.dirty);
    }

    #[test]
    fn uniform_pending_error_names_member_on_bind() {
        let layout = params_layout();
        let mut u = UniformState::new();
        // bind 前は名前・型を検証できないのでいったん通る
        u.set_member("typo", &[1.0], None).unwrap();
        u.staging = vec![0u8; layout.size as usize];
        u.layout = Some(layout);
        // bind 時に、どのメンバの set が悪かったか分かるエラーになる
        let err = u.apply_pending().unwrap_err();
        assert!(err.contains("typo"), "{err}");
    }

    #[test]
    fn buffer_set_before_bind_is_applied_on_bind() {
        let info = reflect(SRC).unwrap();
        let BindingKind::Storage { stride, layout, .. } = &info
            .bindings
            .iter()
            .find(|b| b.name == "src")
            .unwrap()
            .kind
        else {
            panic!()
        };
        let mut mirror = Vec::new();
        let mut dirty = BTreeSet::new();
        let mut pending = vec![
            (1u32, "pos".to_string(), PendingValue::F(vec![3.0, 4.0])),
            (2u32, "vel".to_string(), PendingValue::F(vec![5.0, 6.0])),
        ];
        apply_pending_buffer(layout.as_ref(), *stride, 4, &mut mirror, &mut dirty, &mut pending)
            .unwrap();
        let s = *stride as usize;
        assert_eq!(mirror.len(), 4 * s);
        assert_eq!(f32::from_le_bytes(mirror[s..s + 4].try_into().unwrap()), 3.0); // [1].pos @0
        assert_eq!(
            f32::from_le_bytes(mirror[2 * s + 8..2 * s + 12].try_into().unwrap()),
            5.0
        ); // [2].vel @8
        assert!(dirty.contains(&1) && dirty.contains(&2));
        assert!(pending.is_empty());
    }

    #[test]
    fn render_pair_merges_visibility() {
        let info = reflect(RENDER_SRC).unwrap();
        let (_, _, used) = check_render_pair(&info, "vsMain", "fsMain").unwrap();
        let vis = |name: &str| {
            used.iter()
                .find(|(i, _)| info.bindings[*i].name == name)
                .map(|(_, v)| *v)
                .unwrap()
        };
        assert_eq!(vis("params"), wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT);
        assert_eq!(vis("boids"), wgpu::ShaderStages::VERTEX);
    }

    #[test]
    fn render_pair_unknown_vs_suggests() {
        let info = reflect(RENDER_SRC).unwrap();
        let err = check_render_pair(&info, "vsMai", "fsMain").unwrap_err();
        assert!(err.contains("vsMain"), "候補が出ること: {err}");
    }

    #[test]
    fn render_pair_rejects_fs_name_as_vs() {
        let info = reflect(RENDER_SRC).unwrap();
        assert!(check_render_pair(&info, "fsMain", "fsMain").is_err());
    }

    #[test]
    fn render_pair_rejects_rw_storage_in_vertex() {
        let src = r#"
struct Boid { pos: vec2<f32>, vel: vec2<f32> };
@group(0) @binding(0) var<storage, read_write> boids: array<Boid>;
@vertex
fn vsMain(@builtin(instance_index) iid: u32) -> @builtin(position) vec4<f32> {
  return vec4<f32>(boids[iid].pos, 0.0, 1.0);
}
@fragment
fn fsMain() -> @location(0) vec4<f32> { return vec4<f32>(1.0); }
"#;
        let info = reflect(src).unwrap();
        let err = check_render_pair(&info, "vsMain", "fsMain").unwrap_err();
        assert!(err.contains("StructuredBuffer"), "直し方を案内すること: {err}");
    }
}

// ============================================================================
// ここから GPU 実行部。上の検証ロジックとは違い、実 GPU が必要（結合テストで検証）。
// ============================================================================

use crate::reflect::reflect;

/// uniform リングバッファのスロット数（1 フレーム内の set→dispatch 回数の上限）。
const UNIFORM_SLOTS: u32 = 1024;
/// dynamic offset のアライメント。wgpu の既定 min_uniform_buffer_offset_alignment。
const UNIFORM_ALIGN: u32 = 256;

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub backend: String,
}

/// AWT の描画面に結びついた出力先。
pub struct OutputTarget {
    pub surface: wgpu::Surface<'static>,
    pub blit_pipeline: wgpu::RenderPipeline,
    pub sampler: wgpu::Sampler,
    pub _keepalive: crate::platform::LayerKeepAlive,
}

pub struct UniformState {
    pub layout: Option<StructLayout>,
    staging: Vec<u8>,
    dirty: bool,
    ring: Option<wgpu::Buffer>,
    slot_size: u32,
    /// 現在参照すべきスロット。フレーム開始時は「未 flush」状態から始まる。
    slot: u32,
    next_slot: u32,
    /// bind 前に set された値。レイアウト確定時（初回 bind）に検証して staging へ流し込む。
    pending: Vec<(String, PendingValue)>,
}

impl UniformState {
    fn new() -> UniformState {
        UniformState {
            layout: None,
            staging: Vec::new(),
            dirty: true,
            ring: None,
            slot_size: 0,
            slot: 0,
            next_slot: 0,
            pending: Vec::new(),
        }
    }

    /// `uniform.set(member, ...)`。bind 前（レイアウト未確定）は pending に積み、確定後は staging へ書く。
    fn set_member(&mut self, member: &str, floats: &[f32], int_value: Option<i32>) -> Result<(), String> {
        let Some(layout) = &self.layout else {
            self.pending.retain(|(m, _)| m != member);
            self.pending.push((member.to_string(), PendingValue::new(floats, int_value)));
            return Ok(());
        };
        write_member(layout, &mut self.staging, 0, member, floats, int_value)?;
        self.dirty = true;
        Ok(())
    }

    /// bind でレイアウトが確定した直後に、bind 前の set を適用する。
    fn apply_pending(&mut self) -> Result<(), String> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let layout = self.layout.as_ref().unwrap();
        for (member, v) in std::mem::take(&mut self.pending) {
            let (floats, int_value) = v.parts();
            write_member(layout, &mut self.staging, 0, &member, floats, int_value)
                .map_err(|e| format!("bind 前に set() されていたメンバの適用に失敗: {e}"))?;
        }
        self.dirty = true;
        Ok(())
    }
}

pub enum Resource {
    Texture {
        storage_view: wgpu::TextureView,
        sampled_view: wgpu::TextureView,
        tex: wgpu::Texture,
        format: wgpu::TextureFormat,
        w: u32,
        h: u32,
        cube: bool,
        mips: u32,
    },
    Buffer {
        element_count: u32,
        stride: Option<u32>,
        buf: Option<wgpu::Buffer>,
        /// 要素 struct のメンバレイアウト。stride と同じく初回 bind で確定する。
        layout: Option<StructLayout>,
        /// `at(i).set(...)` の書き込み先。GPU と同じ内容を CPU 側に持つ。
        /// メンバ単位の部分更新を成立させるため、初回の at() でバッファ全体分を確保する。
        /// write(float[]) しか使わない場合は空のまま。
        mirror: Vec<u8>,
        /// ミラー上で書き換えられ、まだ GPU に送っていない要素。
        dirty: BTreeSet<u32>,
        /// bind 前に at(i).set() された値。レイアウト確定時（初回 bind）に検証してミラーへ流し込む。
        pending: Vec<(u32, String, PendingValue)>,
    },
    Uniform(UniformState),
}

struct ShaderState {
    info: ShaderInfo,
    module: wgpu::ShaderModule,
}

struct KernelState {
    shader_key: u64,
    entry_index: usize,
}

struct PipelineCache {
    pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
    /// dynamic offset を渡す順（binding 番号昇順の uniform リソース id）
    uniform_ids: Vec<u64>,
    workgroup_size: [u32; 3],
}

struct BindingState {
    kernel_id: u64,
    bound: HashMap<u32, u64>,
    cache: Option<PipelineCache>,
}

struct RendererState {
    shader_key: u64,
    /// info.render_entries への index
    vs_index: usize,
    fs_index: usize,
    /// check_render_pair の結果（bindings index と visibility）
    used: Vec<(usize, wgpu::ShaderStages)>,
    /// 0=TriangleList / 1=LineList / 2=PointList
    topology: u8,
    /// 0=上書き / 1=アルファ / 2=加算
    blend: u8,
    depth_test: bool,
}

/// パイプラインバリアントのキー。設定かターゲットフォーマットが変わると別エントリになる。
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
struct RenderVariantKey {
    format: wgpu::TextureFormat,
    topology: u8,
    blend: u8,
    depth: bool,
}

/// バインドグループは設定バリアントに依存しないので分けて持つ。
struct RenderBindCache {
    bgl: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    /// dynamic offset を渡す順（binding 番号昇順の uniform リソース id）
    uniform_ids: Vec<u64>,
}

struct RenderBindingState {
    renderer_id: u64,
    bound: HashMap<u32, u64>,
    cache: Option<RenderBindCache>,
    pipelines: HashMap<RenderVariantKey, wgpu::RenderPipeline>,
}

/// 1 回の draw の記録。flush 時に render pass へ書き出す。
struct DrawCmd {
    binding_id: u64,
    /// 非インデックス時は頂点数、インデックス時はインデックス数
    vertices: u32,
    /// インデックスバッファ内の開始位置(非インデックス時は 0)
    first_index: u32,
    instances: u32,
    /// この draw 時点の uniform dynamic offset（dispatch と同じ仕組み）
    offsets: Vec<u32>,
    variant: RenderVariantKey,
    /// drawIndexed のインデックスバッファ（u32・ストライド 4 で検証済み）
    index: Option<u64>,
    /// indirect 引数バッファ（id, バイトオフセット）。Some なら vertices /
    /// first_index / instances は使わず、GPU バッファの u32×5 で描く。
    indirect: Option<(u64, u64)>,
    /// シザー矩形 [x, y, w, h]（物理ピクセル）。None ならターゲット全面。
    scissor: Option<[u32; 4]>,
}

/// dispatch の実行サイズの指定方法。
#[derive(Clone, Copy)]
enum DispatchArgs {
    Direct { x: u32, y: u32, z: u32, threads: bool },
    /// GPU バッファ内の u32×3 をワークグループ数として使う
    Indirect { buffer_id: u64, offset: u64 },
}

/// まだ encoder に書き出していない、同一ターゲットへの draw の列。
struct PendingPass {
    target: u64,
    /// clear() 由来の loadOp=Clear。None なら Load。
    clear: Option<wgpu::Color>,
    /// このパスの深度の有無。最初の draw で確定する（clear 直後は未確定）。
    depth: Option<bool>,
    draws: Vec<DrawCmd>,
}

/// パスごとの GPU 時間の計測。有効なあいだは描画パスをレンダラごとに分け、
/// 各パスの前後にタイムスタンプを書いて submit の最後に読み戻す
struct Profiler {
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    capacity: u32,
    /// このフレームで使った (ラベル, 開始 index)。終了 index は +1
    used: Vec<(String, u32)>,
    /// 前フレームの結果 (ラベル, ミリ秒)
    last: Vec<(String, f64)>,
}

impl Profiler {
    fn slot(&mut self, label: String) -> Option<(u32, u32)> {
        let begin = self.used.len() as u32 * 2;
        if begin + 2 > self.capacity {
            return None;
        }
        self.used.push((label, begin));
        Some((begin, begin + 1))
    }
}

pub struct Engine {
    pub gpu: Gpu,
    pub output: Option<OutputTarget>,
    shaders: HashMap<u64, ShaderState>,
    kernels: HashMap<u64, KernelState>,
    resources: HashMap<u64, Resource>,
    bindings: HashMap<u64, BindingState>,
    renderers: HashMap<u64, RendererState>,
    render_bindings: HashMap<u64, RenderBindingState>,
    shared_sampler: wgpu::Sampler,
    frame: Option<wgpu::CommandEncoder>,
    show_tex: Option<u64>,
    next_id: u64,
    /// ターゲット tex id → (深度ビュー, 幅, 高さ)。サイズが変わったら作り直す。
    depth_textures: HashMap<u64, (wgpu::TextureView, u32, u32)>,
    pending_pass: Option<PendingPass>,
    profile: Option<Profiler>,
    /// kernel / renderer id → 計測報告に出す名前
    labels: HashMap<u64, String>,
}

fn align_up(v: u32, a: u32) -> u32 {
    v.div_ceil(a) * a
}

/// uniform / buffer の初回 bind 時の実体化。2 回目以降はレイアウト整合だけ確認する。
fn realize_on_bind(
    device: &wgpu::Device,
    kind: &BindingKind,
    res: &mut Resource,
) -> Result<(), String> {
    match (kind, res) {
        (BindingKind::Uniform { layout }, Resource::Uniform(u)) => {
            if let Some(existing) = &u.layout {
                if existing.size != layout.size
                    || existing.members.len() != layout.members.len()
                {
                    return Err(format!(
                        "この GpuUniform は別のレイアウト（size={}）で既に使われています",
                        existing.size
                    ));
                }
            } else {
                let slot_size = align_up(layout.size.max(4), UNIFORM_ALIGN);
                u.staging = vec![0u8; layout.size as usize];
                u.slot_size = slot_size;
                u.ring = Some(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gpubridge.uniform_ring"),
                    size: slot_size as u64 * UNIFORM_SLOTS as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                u.layout = Some(layout.clone());
            }
            u.apply_pending()?;
        }
        (
            BindingKind::Storage { stride, layout, .. },
            Resource::Buffer { element_count, stride: st, buf, layout: lay, mirror, dirty, pending },
        ) => {
            if let Some(existing) = st {
                if existing != stride {
                    return Err(format!(
                        "この GpuBuffer は別のストライド（{existing} バイト）で既に使われています（今回: {stride}）"
                    ));
                }
            } else {
                *st = Some(*stride);
                *buf = Some(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gpubridge.user_buffer"),
                    size: *element_count as u64 * *stride as u64,
                    usage: wgpu::BufferUsages::STORAGE
                        | wgpu::BufferUsages::INDEX
                        | wgpu::BufferUsages::INDIRECT
                        | wgpu::BufferUsages::COPY_DST
                        | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }));
            }
            // gpu.buffer(n, stride) で先にストライドを宣言したバッファも、
            // bind した時点でメンバレイアウトを知り、at(i).set() が使えるようになる
            if lay.is_none() {
                *lay = layout.clone();
            }
            apply_pending_buffer(lay.as_ref(), *stride, *element_count, mirror, dirty, pending)?;
        }
        _ => {}
    }
    Ok(())
}

impl Engine {
    pub fn new() -> Result<Engine, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .map_err(|e| format!("adapter の取得に失敗: {e}"))?;
        let info = adapter.get_info();
        let backend = format!("{:?} / {}", info.backend, info.name);
        // 大きな storage buffer（シャドウ履歴など）を bind できるよう、バッファ関連の上限だけ adapter の値にする
        let adapter_limits = adapter.limits();
        let required_limits = wgpu::Limits {
            max_buffer_size: adapter_limits.max_buffer_size,
            max_storage_buffer_binding_size: adapter_limits.max_storage_buffer_binding_size,
            ..wgpu::Limits::default()
        };
        let required_features = adapter.features()
            & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::FLOAT32_FILTERABLE);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits,
            required_features,
            ..Default::default()
        }))
        .map_err(|e| format!("device の取得に失敗: {e}"))?;
        let shared_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("gpubridge.shared_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Ok(Engine {
            gpu: Gpu { instance, adapter, device, queue, backend },
            output: None,
            shaders: HashMap::new(),
            kernels: HashMap::new(),
            resources: HashMap::new(),
            bindings: HashMap::new(),
            renderers: HashMap::new(),
            render_bindings: HashMap::new(),
            shared_sampler,
            frame: None,
            show_tex: None,
            next_id: 1,
            depth_textures: HashMap::new(),
            pending_pass: None,
            profile: None,
            labels: HashMap::new(),
        })
    }

    // ---------------- 計測 ----------------

    pub fn set_label(&mut self, id: u64, label: &str) -> Result<(), String> {
        if !self.kernels.contains_key(&id) && !self.renderers.contains_key(&id) {
            return Err("不正な kernel / renderer id".into());
        }
        self.labels.insert(id, label.to_string());
        Ok(())
    }

    pub fn profile_enable(&mut self, on: bool) -> Result<(), String> {
        if on == self.profile.is_some() {
            return Ok(());
        }
        if !on {
            self.profile = None;
            return Ok(());
        }
        if !self.gpu.device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return Err("この GPU ではタイムスタンプが使えません".into());
        }
        let capacity = 512;
        let query_set = self.gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("gpubridge.profile"),
            ty: wgpu::QueryType::Timestamp,
            count: capacity,
        });
        let bytes = capacity as u64 * 8;
        let resolve = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpubridge.profile_resolve"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpubridge.profile_read"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        self.profile = Some(Profiler { query_set, resolve, read, capacity, used: Vec::new(), last: Vec::new() });
        Ok(())
    }

    /// 前フレームの計測結果。1 行が「ラベル<TAB>ミリ秒」
    pub fn profile_report(&self) -> String {
        let mut s = String::new();
        if let Some(p) = &self.profile {
            for (label, ms) in &p.last {
                s.push_str(&format!("{label}\t{ms:.3}\n"));
            }
        }
        s
    }

    fn kernel_label(&self, binding_id: u64) -> String {
        let kernel_id = self.bindings[&binding_id].kernel_id;
        if let Some(l) = self.labels.get(&kernel_id) {
            return l.clone();
        }
        let k = &self.kernels[&kernel_id];
        self.shaders[&k.shader_key].info.entry_points[k.entry_index].name.clone()
    }

    fn renderer_label(&self, binding_id: u64) -> String {
        let renderer_id = self.render_bindings[&binding_id].renderer_id;
        self.labels.get(&renderer_id).cloned().unwrap_or_else(|| format!("render#{renderer_id}"))
    }

    /// フレーム末尾で計測結果を読み戻す（GPU の完了を待つ）
    fn collect_profile(&mut self) -> Result<(), String> {
        let Some(p) = self.profile.as_mut() else { return Ok(()) };
        let n = p.used.len() as u64 * 2;
        if n == 0 {
            p.last.clear();
            return Ok(());
        }
        let (tx, rx) = std::sync::mpsc::channel();
        p.read.map_async(wgpu::MapMode::Read, ..(n * 8), move |r| {
            let _ = tx.send(r);
        });
        self.gpu
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("poll に失敗: {e}"))?;
        match rx.recv() {
            Ok(Ok(())) => {}
            other => return Err(format!("計測バッファの map に失敗: {other:?}")),
        }
        let period = self.gpu.queue.get_timestamp_period() as f64;
        let p = self.profile.as_mut().unwrap();
        let mut last = Vec::with_capacity(p.used.len());
        {
            let data = p
                .read
                .get_mapped_range(..(n * 8))
                .map_err(|e| format!("mapped range の取得に失敗: {e}"))?;
            for (label, begin) in &p.used {
                let at = |i: u32| {
                    let o = i as usize * 8;
                    u64::from_le_bytes(data[o..o + 8].try_into().unwrap())
                };
                let (b, e) = (at(*begin), at(*begin + 1));
                last.push((label.clone(), e.saturating_sub(b) as f64 * period / 1e6));
            }
        }
        p.read.unmap();
        p.last = last;
        p.used.clear();
        Ok(())
    }

    fn fresh_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    // ---------------- リソース生成 ----------------

    pub fn create_texture(&mut self, w: u32, h: u32, format: wgpu::TextureFormat) -> u64 {
        self.create_texture_ex(w, h, format, false, 1)
    }

    /// cube なら 6 面（+X, −X, +Y, −Y, +Z, −Z）、mips 段のミップを持つ。
    /// キューブは sample 専用で、描画先・clear・show には使えない
    pub fn create_texture_ex(&mut self, w: u32, h: u32, format: wgpu::TextureFormat, cube: bool, mips: u32) -> u64 {
        let usage = if cube {
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC
        } else {
            wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
        };
        let tex = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpubridge.user_texture"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: if cube { 6 } else { 1 } },
            mip_level_count: mips.max(1),
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        });
        // storage 用のビューは 2D の 1 面 1 段（キューブでは使わないが型は揃えておく）
        let storage_view = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: 0,
            array_layer_count: Some(1),
            base_mip_level: 0,
            mip_level_count: Some(1),
            ..Default::default()
        });
        let sampled_view = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(if cube { wgpu::TextureViewDimension::Cube } else { wgpu::TextureViewDimension::D2 }),
            ..Default::default()
        });
        let id = self.fresh_id();
        self.resources.insert(
            id,
            Resource::Texture { storage_view, sampled_view, tex, format, w, h, cube, mips: mips.max(1) },
        );
        id
    }

    fn require_2d(&self, tex_id: u64) -> Result<(), String> {
        match self.resources.get(&tex_id) {
            Some(Resource::Texture { cube: true, .. }) => Err("キューブマップは描画先・clear・show に使えません".into()),
            _ => Ok(()),
        }
    }

    pub fn create_buffer(&mut self, element_count: u32) -> u64 {
        let id = self.fresh_id();
        self.resources.insert(
            id,
            Resource::Buffer {
                element_count,
                stride: None,
                buf: None,
                layout: None,
                mirror: Vec::new(),
                dirty: BTreeSet::new(),
                pending: Vec::new(),
            },
        );
        id
    }

    /// ストライドを宣言して storage buffer を確保する。bind を待たずに write できる。
    /// 後からカーネルへ bind した場合は、WGSL 側のストライドとの一致を検証する。
    pub fn create_buffer_strided(&mut self, element_count: u32, stride: u32) -> Result<u64, String> {
        if stride == 0 || stride % 4 != 0 {
            return Err(format!(
                "ストライドは 4 の倍数（1 以上）である必要があります: {stride}"
            ));
        }
        let buf = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpubridge.user_buffer"),
            size: element_count as u64 * stride as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDEX
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let id = self.fresh_id();
        self.resources.insert(
            id,
            Resource::Buffer {
                element_count,
                stride: Some(stride),
                buf: Some(buf),
                layout: None,
                mirror: Vec::new(),
                dirty: BTreeSet::new(),
                pending: Vec::new(),
            },
        );
        Ok(id)
    }

    pub fn create_uniform(&mut self) -> u64 {
        let id = self.fresh_id();
        self.resources.insert(id, Resource::Uniform(UniformState::new()));
        id
    }

    pub fn create_kernel(&mut self, wgsl: &str, entry: &str) -> Result<u64, String> {
        // 同一ソースは 1 回だけ reflect + コンパイル
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        wgsl.hash(&mut hasher);
        let key = hasher.finish();

        if !self.shaders.contains_key(&key) {
            let info = reflect(wgsl)?;
            let module = self
                .gpu
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("gpubridge.user_shader"),
                    source: wgpu::ShaderSource::Wgsl(wgsl.into()),
                });
            self.shaders.insert(key, ShaderState { info, module });
        }

        let shader = &self.shaders[&key];
        let Some(entry_index) = shader
            .info
            .entry_points
            .iter()
            .position(|e| e.name == entry)
        else {
            let names: Vec<&str> = shader
                .info
                .entry_points
                .iter()
                .map(|e| e.name.as_str())
                .collect();
            return Err(format!(
                "エントリポイント {}",
                suggest(entry, &names)
            ));
        };

        let id = self.fresh_id();
        self.kernels.insert(id, KernelState { shader_key: key, entry_index });
        Ok(id)
    }

    pub fn create_binding(&mut self, kernel_id: u64) -> Result<u64, String> {
        if !self.kernels.contains_key(&kernel_id) {
            return Err("不正なカーネル id".into());
        }
        let id = self.fresh_id();
        self.bindings.insert(
            id,
            BindingState { kernel_id, bound: HashMap::new(), cache: None },
        );
        Ok(id)
    }

    pub fn create_renderer(&mut self, wgsl: &str, vs: &str, fs: &str) -> Result<u64, String> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        wgsl.hash(&mut hasher);
        let key = hasher.finish();

        if !self.shaders.contains_key(&key) {
            let info = reflect(wgsl)?;
            let module = self
                .gpu
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("gpubridge.user_shader"),
                    source: wgpu::ShaderSource::Wgsl(wgsl.into()),
                });
            self.shaders.insert(key, ShaderState { info, module });
        }

        let (vs_index, fs_index, used) = check_render_pair(&self.shaders[&key].info, vs, fs)?;
        let id = self.fresh_id();
        self.renderers.insert(
            id,
            RendererState {
                shader_key: key,
                vs_index,
                fs_index,
                used,
                topology: 0,
                blend: 0,
                depth_test: false,
            },
        );
        Ok(id)
    }

    pub fn renderer_config(
        &mut self,
        id: u64,
        topology: u8,
        blend: u8,
        depth_test: bool,
    ) -> Result<(), String> {
        let r = self.renderers.get_mut(&id).ok_or("不正なレンダラー id")?;
        if topology > 4 {
            return Err(format!("不正なトポロジ番号: {topology}"));
        }
        if blend > 2 {
            return Err(format!("不正なブレンド番号: {blend}"));
        }
        r.topology = topology;
        r.blend = blend;
        r.depth_test = depth_test;
        // パイプラインは RenderVariantKey で引くので、既存キャッシュの無効化は不要
        Ok(())
    }

    pub fn create_render_binding(&mut self, renderer_id: u64) -> Result<u64, String> {
        if !self.renderers.contains_key(&renderer_id) {
            return Err("不正なレンダラー id".into());
        }
        let id = self.fresh_id();
        self.render_bindings.insert(
            id,
            RenderBindingState {
                renderer_id,
                bound: HashMap::new(),
                cache: None,
                pipelines: HashMap::new(),
            },
        );
        Ok(id)
    }

    pub fn render_binding_set(
        &mut self,
        binding_id: u64,
        name: &str,
        res_id: u64,
    ) -> Result<(), String> {
        // 保留中の draw が古い bind group/pipeline を参照したまま flush されないよう、
        // cache を組み直す前に先に書き出しておく。
        self.flush_pending_pass();
        let binding = self
            .render_bindings
            .get(&binding_id)
            .ok_or("不正なバインディング id")?;
        let renderer = &self.renderers[&binding.renderer_id];
        let shader = &self.shaders[&renderer.shader_key];

        let desc = match self.resources.get(&res_id).ok_or("不正なリソース id")? {
            Resource::Uniform(_) => ResourceDesc::Uniform,
            Resource::Buffer { .. } => ResourceDesc::Buffer,
            Resource::Texture { format, cube, .. } => ResourceDesc::Texture { format: *format, cube: *cube },
        };
        let bnum = check_set(&shader.info.bindings, name, &desc)?;
        let kind = &shader
            .info
            .bindings
            .iter()
            .find(|b| b.binding == bnum)
            .unwrap()
            .kind;
        realize_on_bind(&self.gpu.device, kind, self.resources.get_mut(&res_id).unwrap())?;

        let binding = self.render_bindings.get_mut(&binding_id).unwrap();
        binding.bound.insert(bnum, res_id);
        // BindGroup が変わるので組み直し。BindGroupLayout の同一性を保証するため
        // パイプラインも一緒に捨てる（次の draw で必要なバリアントだけ再構築される）
        binding.cache = None;
        binding.pipelines.clear();
        Ok(())
    }

    // ---------------- bind ----------------

    pub fn binding_set(&mut self, binding_id: u64, name: &str, res_id: u64) -> Result<(), String> {
        let binding = self
            .bindings
            .get(&binding_id)
            .ok_or("不正なバインディング id")?;
        let kernel = &self.kernels[&binding.kernel_id];
        let shader = &self.shaders[&kernel.shader_key];

        let desc = match self.resources.get(&res_id).ok_or("不正なリソース id")? {
            Resource::Uniform(_) => ResourceDesc::Uniform,
            Resource::Buffer { .. } => ResourceDesc::Buffer,
            Resource::Texture { format, cube, .. } => ResourceDesc::Texture { format: *format, cube: *cube },
        };

        let bnum = check_set(&shader.info.bindings, name, &desc)?;

        // uniform / buffer は初回 bind で実体化・レイアウト確定
        // （check_set が返した binding 番号から引く。名前はマングルされている可能性がある）
        let kind = &shader
            .info
            .bindings
            .iter()
            .find(|b| b.binding == bnum)
            .unwrap()
            .kind;

        realize_on_bind(&self.gpu.device, kind, self.resources.get_mut(&res_id).unwrap())?;

        let binding = self.bindings.get_mut(&binding_id).unwrap();
        binding.bound.insert(bnum, res_id);
        binding.cache = None; // 変更されたら組み直す
        Ok(())
    }

    // ---------------- uniform set ----------------

    pub fn uniform_set(
        &mut self,
        uid: u64,
        member: &str,
        floats: &[f32],
        int_value: Option<i32>,
    ) -> Result<(), String> {
        let Some(Resource::Uniform(u)) = self.resources.get_mut(&uid) else {
            return Err("不正な uniform id".into());
        };
        u.set_member(member, floats, int_value)
    }

    // ---------------- dispatch ----------------

    /// dirty uniform をリングの次スロットへ送る。dispatch / draw の直前に呼ぶ。
    fn flush_uniforms(&mut self, uniform_ids: &[u64]) -> Result<(), String> {
        for uid in uniform_ids {
            let Some(Resource::Uniform(u)) = self.resources.get_mut(uid) else { continue };
            if u.dirty {
                if u.next_slot >= UNIFORM_SLOTS {
                    return Err(format!(
                        "1 フレーム内の uniform 更新が上限（{UNIFORM_SLOTS} 回）を超えました。submit() を挟んでください"
                    ));
                }
                u.slot = u.next_slot;
                u.next_slot += 1;
                self.gpu.queue.write_buffer(
                    u.ring.as_ref().unwrap(),
                    u.slot as u64 * u.slot_size as u64,
                    &u.staging,
                );
                u.dirty = false;
            }
        }
        Ok(())
    }

    /// 現在のスロットに対応する dynamic offset 列。
    fn uniform_offsets(&self, uniform_ids: &[u64]) -> Vec<u32> {
        uniform_ids
            .iter()
            .map(|uid| {
                let Resource::Uniform(u) = &self.resources[uid] else { unreachable!() };
                u.slot * u.slot_size
            })
            .collect()
    }

    pub fn dispatch(
        &mut self,
        binding_id: u64,
        x: u32,
        y: u32,
        z: u32,
        threads: bool,
    ) -> Result<(), String> {
        self.dispatch_common(binding_id, DispatchArgs::Direct { x, y, z, threads })
    }

    /// indirect バッファの offset から u32×3 をワークグループ数として実行する。
    /// スレッド数への換算はしない（GPU が書いた値をそのまま使う）。
    pub fn dispatch_indirect(
        &mut self,
        binding_id: u64,
        buffer_id: u64,
        offset: u64,
    ) -> Result<(), String> {
        self.dispatch_common(binding_id, DispatchArgs::Indirect { buffer_id, offset })
    }

    fn dispatch_common(&mut self, binding_id: u64, args: DispatchArgs) -> Result<(), String> {
        self.flush_pending_pass();

        // 1) パイプラインキャッシュを（必要なら）構築
        if self
            .bindings
            .get(&binding_id)
            .ok_or("不正なバインディング id")?
            .cache
            .is_none()
        {
            self.build_pipeline(binding_id)?;
        }

        // 2) dirty uniform をリングの次スロットへ flush
        let uniform_ids = self.bindings[&binding_id]
            .cache
            .as_ref()
            .unwrap()
            .uniform_ids
            .clone();
        self.flush_uniforms(&uniform_ids)?;

        // 3) dirty なバッファミラーを flush（保留フレームがあれば先に submit される）
        self.flush_dirty_buffers();

        // indirect 引数バッファの検証（エンコード前に済ませる）
        if let DispatchArgs::Indirect { buffer_id, offset } = args {
            self.check_indirect_args(buffer_id, offset, 12)?;
        }

        // 4) エンコード
        if self.frame.is_none() {
            self.frame = Some(
                self.gpu
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("gpubridge.frame"),
                    }),
            );
        }
        let label = if self.profile.is_some() { self.kernel_label(binding_id) } else { String::new() };
        let cache = self.bindings[&binding_id].cache.as_ref().unwrap();
        let offsets = self.uniform_offsets(&uniform_ids);
        let slot = self.profile.as_mut().and_then(|p| p.slot(label));
        let timestamp_writes = slot.map(|(b, e)| wgpu::ComputePassTimestampWrites {
            query_set: &self.profile.as_ref().unwrap().query_set,
            beginning_of_pass_write_index: Some(b),
            end_of_pass_write_index: Some(e),
        });
        let enc = self.frame.as_mut().unwrap();
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes,
            });
            pass.set_pipeline(&cache.pipeline);
            pass.set_bind_group(0, &cache.bind_group, &offsets);
            match args {
                DispatchArgs::Direct { x, y, z, threads } => {
                    let [wx, wy, wz] = cache.workgroup_size;
                    let (gx, gy, gz) = if threads {
                        (x.div_ceil(wx), y.max(1).div_ceil(wy), z.max(1).div_ceil(wz))
                    } else {
                        (x, y, z)
                    };
                    pass.dispatch_workgroups(gx.max(1), gy.max(1), gz.max(1));
                }
                DispatchArgs::Indirect { buffer_id, offset } => {
                    let Some(Resource::Buffer { buf: Some(ib), .. }) =
                        self.resources.get(&buffer_id)
                    else {
                        unreachable!("検証済み");
                    };
                    pass.dispatch_workgroups_indirect(ib, offset);
                }
            }
        }
        Ok(())
    }

    /// indirect 引数バッファの検証。realize 済みで、offset から need バイトが収まること。
    fn check_indirect_args(&self, id: u64, offset: u64, need: u64) -> Result<(), String> {
        let Some(Resource::Buffer { buf, .. }) = self.resources.get(&id) else {
            return Err("不正な indirect バッファ id".into());
        };
        let Some(buf) = buf else {
            return Err(
                "indirect バッファのストライドが未確定です（先にカーネルへ bind するか、gpu.buffer(n, stride) で作ってください）"
                    .into(),
            );
        };
        if offset % 4 != 0 {
            return Err(format!("indirect のバイトオフセットは 4 の倍数が必要です: {offset}"));
        }
        if offset + need > buf.size() {
            return Err(format!(
                "indirect 引数がバッファ外です: オフセット {offset} + {need} バイト（バッファは {} バイト）",
                buf.size()
            ));
        }
        Ok(())
    }

    fn build_pipeline(&mut self, binding_id: u64) -> Result<(), String> {
        let binding = &self.bindings[&binding_id];
        let kernel = &self.kernels[&binding.kernel_id];
        let shader = &self.shaders[&kernel.shader_key];
        let entry = &shader.info.entry_points[kernel.entry_index];

        check_complete(&entry.name, &entry.used_bindings, &shader.info.bindings, &binding.bound)?;

        // BindGroupLayout: このエントリが使う binding のみ。binding 番号昇順。
        let mut used: Vec<&BindingInfo> = entry
            .used_bindings
            .iter()
            .map(|&i| &shader.info.bindings[i])
            .collect();
        used.sort_by_key(|b| b.binding);

        let mut layout_entries = Vec::new();
        let mut uniform_ids = Vec::new();
        for b in &used {
            let ty = match &b.kind {
                BindingKind::Uniform { layout } => {
                    uniform_ids.push(binding.bound[&b.binding]);
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(layout.size as u64),
                    }
                }
                BindingKind::Storage { read_only, .. } => wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: *read_only },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                BindingKind::StorageTexture { format, write_only } => {
                    wgpu::BindingType::StorageTexture {
                        access: if *write_only {
                            wgpu::StorageTextureAccess::WriteOnly
                        } else {
                            wgpu::StorageTextureAccess::ReadWrite
                        },
                        format: *format,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    }
                }
                BindingKind::SampledTexture { .. } => {
                    // filterable かどうかは実際に bind されたテクスチャの format から決める
                    let res_id = binding.bound[&b.binding];
                    let Resource::Texture { format, cube, .. } = &self.resources[&res_id] else {
                        return Err(format!("'{}' に texture 以外が bind されています", b.name));
                    };
                    let float32 = matches!(
                        format,
                        wgpu::TextureFormat::Rgba32Float
                            | wgpu::TextureFormat::R32Float
                            | wgpu::TextureFormat::Rg32Float
                    );
                    let filterable = !float32
                        || self.gpu.device.features().contains(wgpu::Features::FLOAT32_FILTERABLE);
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable },
                        view_dimension: if *cube {
                            wgpu::TextureViewDimension::Cube
                        } else {
                            wgpu::TextureViewDimension::D2
                        },
                        multisampled: false,
                    }
                }
                BindingKind::Sampler => {
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                }
            };
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding: b.binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty,
                count: None,
            });
        }

        let bgl = self
            .gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("gpubridge.bgl"),
                entries: &layout_entries,
            });
        let pl = self
            .gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("gpubridge.pl"),
                bind_group_layouts: &[Some(&bgl)],
                immediate_size: 0,
            });

        let scope = self.gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let pipeline = self
            .gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("gpubridge.pipeline"),
                layout: Some(&pl),
                module: &shader.module,
                entry_point: Some(&entry.name),
                compilation_options: Default::default(),
                cache: None,
            });
        if let Some(err) = pollster::block_on(scope.pop()) {
            return Err(format!("パイプラインの構築に失敗:\n{err}"));
        }

        // BindGroup
        let mut group_entries = Vec::new();
        for b in &used {
            let resource = match &b.kind {
                BindingKind::Sampler => wgpu::BindingResource::Sampler(&self.shared_sampler),
                BindingKind::Uniform { layout } => {
                    let Resource::Uniform(u) = &self.resources[&binding.bound[&b.binding]] else {
                        unreachable!()
                    };
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: u.ring.as_ref().unwrap(),
                        offset: 0,
                        size: wgpu::BufferSize::new(layout.size as u64),
                    })
                }
                BindingKind::Storage { .. } => {
                    let Resource::Buffer { buf, .. } = &self.resources[&binding.bound[&b.binding]]
                    else {
                        unreachable!()
                    };
                    buf.as_ref().unwrap().as_entire_binding()
                }
                BindingKind::StorageTexture { .. } => {
                    let Resource::Texture { storage_view, .. } =
                        &self.resources[&binding.bound[&b.binding]]
                    else {
                        unreachable!()
                    };
                    wgpu::BindingResource::TextureView(storage_view)
                }
                BindingKind::SampledTexture { .. } => {
                    let Resource::Texture { sampled_view, .. } =
                        &self.resources[&binding.bound[&b.binding]]
                    else {
                        unreachable!()
                    };
                    wgpu::BindingResource::TextureView(sampled_view)
                }
            };
            group_entries.push(wgpu::BindGroupEntry { binding: b.binding, resource });
        }
        let bind_group = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpubridge.bg"),
            layout: &bgl,
            entries: &group_entries,
        });

        let workgroup_size = entry.workgroup_size;
        self.bindings.get_mut(&binding_id).unwrap().cache = Some(PipelineCache {
            pipeline,
            bind_group,
            uniform_ids,
            workgroup_size,
        });
        Ok(())
    }

    fn build_render_bind_cache(&mut self, binding_id: u64) -> Result<(), String> {
        let binding = &self.render_bindings[&binding_id];
        let renderer = &self.renderers[&binding.renderer_id];
        let shader = &self.shaders[&renderer.shader_key];
        let entry_name = &shader.info.render_entries[renderer.vs_index].name;

        let used_indices: Vec<usize> = renderer.used.iter().map(|(i, _)| *i).collect();
        check_complete(entry_name, &used_indices, &shader.info.bindings, &binding.bound)?;

        // binding 番号昇順
        let mut used: Vec<(&BindingInfo, wgpu::ShaderStages)> = renderer
            .used
            .iter()
            .map(|(i, v)| (&shader.info.bindings[*i], *v))
            .collect();
        used.sort_by_key(|(b, _)| b.binding);

        let mut layout_entries = Vec::new();
        let mut uniform_ids = Vec::new();
        for (b, vis) in &used {
            // ty の分岐は build_pipeline() と同じ。visibility だけが per-binding になる。
            let ty = match &b.kind {
                BindingKind::Uniform { layout } => {
                    uniform_ids.push(binding.bound[&b.binding]);
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(layout.size as u64),
                    }
                }
                BindingKind::Storage { read_only, .. } => wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: *read_only },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                BindingKind::StorageTexture { format, write_only } => {
                    wgpu::BindingType::StorageTexture {
                        access: if *write_only {
                            wgpu::StorageTextureAccess::WriteOnly
                        } else {
                            wgpu::StorageTextureAccess::ReadWrite
                        },
                        format: *format,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    }
                }
                BindingKind::SampledTexture { .. } => {
                    // filterable かどうかは実際に bind されたテクスチャの format から決める
                    let res_id = binding.bound[&b.binding];
                    let Resource::Texture { format, cube, .. } = &self.resources[&res_id] else {
                        return Err(format!("'{}' に texture 以外が bind されています", b.name));
                    };
                    let float32 = matches!(
                        format,
                        wgpu::TextureFormat::Rgba32Float
                            | wgpu::TextureFormat::R32Float
                            | wgpu::TextureFormat::Rg32Float
                    );
                    let filterable = !float32
                        || self.gpu.device.features().contains(wgpu::Features::FLOAT32_FILTERABLE);
                    wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable },
                        view_dimension: if *cube {
                            wgpu::TextureViewDimension::Cube
                        } else {
                            wgpu::TextureViewDimension::D2
                        },
                        multisampled: false,
                    }
                }
                BindingKind::Sampler => {
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                }
            };
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding: b.binding,
                visibility: *vis,
                ty,
                count: None,
            });
        }
        let bgl = self.gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpubridge.render_bgl"),
            entries: &layout_entries,
        });

        let mut group_entries = Vec::new();
        for (b, _) in &used {
            // resource の分岐も build_pipeline() の BindGroup 構築と同一
            let resource = match &b.kind {
                BindingKind::Sampler => wgpu::BindingResource::Sampler(&self.shared_sampler),
                BindingKind::Uniform { layout } => {
                    let Resource::Uniform(u) = &self.resources[&binding.bound[&b.binding]] else {
                        unreachable!()
                    };
                    wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: u.ring.as_ref().unwrap(),
                        offset: 0,
                        size: wgpu::BufferSize::new(layout.size as u64),
                    })
                }
                BindingKind::Storage { .. } => {
                    let Resource::Buffer { buf, .. } = &self.resources[&binding.bound[&b.binding]]
                    else {
                        unreachable!()
                    };
                    buf.as_ref().unwrap().as_entire_binding()
                }
                BindingKind::StorageTexture { .. } => {
                    let Resource::Texture { storage_view, .. } =
                        &self.resources[&binding.bound[&b.binding]]
                    else {
                        unreachable!()
                    };
                    wgpu::BindingResource::TextureView(storage_view)
                }
                BindingKind::SampledTexture { .. } => {
                    let Resource::Texture { sampled_view, .. } =
                        &self.resources[&binding.bound[&b.binding]]
                    else {
                        unreachable!()
                    };
                    wgpu::BindingResource::TextureView(sampled_view)
                }
            };
            group_entries.push(wgpu::BindGroupEntry { binding: b.binding, resource });
        }
        let bind_group = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpubridge.render_bg"),
            layout: &bgl,
            entries: &group_entries,
        });

        self.render_bindings.get_mut(&binding_id).unwrap().cache =
            Some(RenderBindCache { bgl, bind_group, uniform_ids });
        Ok(())
    }

    fn blend_state(blend: u8) -> Option<wgpu::BlendState> {
        match blend {
            1 => Some(wgpu::BlendState::ALPHA_BLENDING),
            2 => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            _ => None,
        }
    }

    fn build_render_pipeline(
        &mut self,
        binding_id: u64,
        variant: RenderVariantKey,
    ) -> Result<(), String> {
        let binding = &self.render_bindings[&binding_id];
        let renderer = &self.renderers[&binding.renderer_id];
        let shader = &self.shaders[&renderer.shader_key];
        let vs = &shader.info.render_entries[renderer.vs_index].name;
        let fs = &shader.info.render_entries[renderer.fs_index].name;
        let bgl = &binding.cache.as_ref().unwrap().bgl;

        let pl = self.gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpubridge.render_pl"),
            bind_group_layouts: &[Some(bgl)],
            immediate_size: 0,
        });
        let topology = match variant.topology {
            1 => wgpu::PrimitiveTopology::LineList,
            2 => wgpu::PrimitiveTopology::PointList,
            3 => wgpu::PrimitiveTopology::TriangleStrip,
            4 => wgpu::PrimitiveTopology::LineStrip,
            _ => wgpu::PrimitiveTopology::TriangleList,
        };
        let scope = self.gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let pipeline = self.gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gpubridge.render_pipeline"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader.module,
                entry_point: Some(vs),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader.module,
                entry_point: Some(fs),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: variant.format,
                    blend: Self::blend_state(variant.blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology,
                // strip 系は index format の指定が必須。0xFFFFFFFF が primitive restart になる
                strip_index_format: matches!(variant.topology, 3 | 4)
                    .then_some(wgpu::IndexFormat::Uint32),
                ..Default::default()
            },
            depth_stencil: variant.depth.then(|| wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        if let Some(err) = pollster::block_on(scope.pop()) {
            return Err(format!("レンダーパイプラインの構築に失敗:\n{err}"));
        }
        self.render_bindings
            .get_mut(&binding_id)
            .unwrap()
            .pipelines
            .insert(variant, pipeline);
        Ok(())
    }

    // ---------------- render pass ----------------

    /// 溜まっている draw 列を 1 つの render pass として encoder に書き出す。
    ///
    /// clear だけで draw が無い場合も空のパスを回してクリアを効かせる。
    /// clear 指定があるのに深度なしのパスだった場合、そのターゲットの深度
    /// キャッシュが存在すれば、深度だけの空パスを追加してクリアする
    /// （clear() は「色も深度も消す」約束のため）。
    fn flush_pending_pass(&mut self) {
        let Some(p) = self.pending_pass.take() else { return };
        let Some(Resource::Texture { sampled_view, w, h, .. }) = self.resources.get(&p.target)
        else {
            return; // ターゲットが解放済みならこのパスは捨てる
        };
        let (tw, th) = (*w, *h);
        if self.frame.is_none() {
            self.frame = Some(self.gpu.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("gpubridge.frame") },
            ));
        }
        let use_depth = p.depth == Some(true);
        let depth_view = if use_depth {
            Some(&self.depth_textures[&p.target].0)
        } else {
            None
        };
        let label = match (&self.profile, p.draws.first()) {
            (Some(_), Some(d)) => self.renderer_label(d.binding_id),
            (Some(_), None) => "clear".to_string(),
            _ => String::new(),
        };
        let slot = self.profile.as_mut().and_then(|pr| pr.slot(label));
        let timestamp_writes = slot.map(|(b, e)| wgpu::RenderPassTimestampWrites {
            query_set: &self.profile.as_ref().unwrap().query_set,
            beginning_of_pass_write_index: Some(b),
            end_of_pass_write_index: Some(e),
        });
        let enc = self.frame.as_mut().unwrap();
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpubridge.user_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: sampled_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: match p.clear {
                            Some(c) => wgpu::LoadOp::Clear(c),
                            None => wgpu::LoadOp::Load,
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: depth_view.map(|v| {
                    wgpu::RenderPassDepthStencilAttachment {
                        view: v,
                        depth_ops: Some(wgpu::Operations {
                            load: if p.clear.is_some() {
                                wgpu::LoadOp::Clear(1.0)
                            } else {
                                wgpu::LoadOp::Load
                            },
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }
                }),
                timestamp_writes,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for cmd in &p.draws {
                let b = &self.render_bindings[&cmd.binding_id];
                pass.set_pipeline(&b.pipelines[&cmd.variant]);
                pass.set_bind_group(0, &b.cache.as_ref().unwrap().bind_group, &cmd.offsets);
                // シザー: 指定があればターゲット内にクランプ、なければ全面に戻す
                match cmd.scissor {
                    Some([sx, sy, sw, sh]) => {
                        let sx = sx.min(tw);
                        let sy = sy.min(th);
                        let sw = sw.min(tw - sx);
                        let sh = sh.min(th - sy);
                        if sw == 0 || sh == 0 {
                            continue; // クランプ後に消えた矩形は描くものがない
                        }
                        pass.set_scissor_rect(sx, sy, sw, sh);
                    }
                    None => pass.set_scissor_rect(0, 0, tw, th),
                }
                match cmd.index {
                    Some(idx) => {
                        // 記録後に解放された場合は release_resource が先に flush している
                        // ため通常ここには来ないが、来ても静かに捨てるだけにする
                        let Some(Resource::Buffer { buf: Some(ib), .. }) =
                            self.resources.get(&idx)
                        else {
                            continue;
                        };
                        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                        match cmd.indirect {
                            Some((ind_id, ind_off)) => {
                                let Some(Resource::Buffer { buf: Some(ab), .. }) =
                                    self.resources.get(&ind_id)
                                else {
                                    continue;
                                };
                                pass.draw_indexed_indirect(ab, ind_off);
                            }
                            None => pass.draw_indexed(
                                cmd.first_index..cmd.first_index + cmd.vertices,
                                0,
                                0..cmd.instances,
                            ),
                        }
                    }
                    None => pass.draw(0..cmd.vertices, 0..cmd.instances),
                }
            }
        }
        // clear 約束の残り: 深度なしパスだったが深度キャッシュがある場合
        if p.clear.is_some() && !use_depth {
            if let Some((view, _, _)) = self.depth_textures.get(&p.target) {
                let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("gpubridge.depth_clear"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            }
        }
    }

    // ---------------- CPU との出入り ----------------

    /// `buffer.at(index).set(member, ...)`。CPU ミラーに書いて dirty を立てるだけで、
    /// GPU への転送は次の flush（dispatch 直前・read 前・submit 前）にまとめて行う。
    pub fn buffer_set(
        &mut self,
        id: u64,
        index: u32,
        member: &str,
        floats: &[f32],
        int_value: Option<i32>,
    ) -> Result<(), String> {
        let Some(Resource::Buffer { element_count, stride, layout, mirror, dirty, pending, .. }) =
            self.resources.get_mut(&id)
        else {
            return Err("不正なバッファ id".into());
        };
        if index >= *element_count {
            return Err(format!(
                "要素番号が範囲外です: {index}（要素数 {element_count}）"
            ));
        }
        // bind 前（ストライド未確定）はレイアウトを検証できないので pending に積み、
        // 初回 bind（realize_on_bind）で検証して適用する
        let Some(stride) = *stride else {
            pending.retain(|(i, m, _)| !(*i == index && m == member));
            pending.push((index, member.to_string(), PendingValue::new(floats, int_value)));
            return Ok(());
        };
        let Some(layout) = layout else {
            return Err(
                "この GpuBuffer の要素は struct ではないか、メンバ名で指定できない型を含んでいます。write(float[]) を使ってください"
                    .into(),
            );
        };
        if mirror.is_empty() {
            *mirror = vec![0u8; *element_count as usize * stride as usize];
        }
        write_member(layout, mirror, index as usize * stride as usize, member, floats, int_value)?;
        dirty.insert(index);
        Ok(())
    }

    /// dirty なミラーの内容を GPU へ送る。連続した要素は 1 回の write_buffer にまとめる。
    ///
    /// `queue.write_buffer` は積んである command encoder より先に効いてしまうので、
    /// 保留中のフレームがあれば先に submit して順序を保つ。
    fn flush_dirty_buffers(&mut self) {
        let ids: Vec<u64> = self
            .resources
            .iter()
            .filter(|(_, r)| matches!(r, Resource::Buffer { dirty, .. } if !dirty.is_empty()))
            .map(|(&id, _)| id)
            .collect();
        if ids.is_empty() {
            return;
        }
        self.flush_pending_pass();
        if let Some(enc) = self.frame.take() {
            self.gpu.queue.submit(Some(enc.finish()));
        }
        for id in ids {
            let Some(Resource::Buffer { buf, stride, mirror, dirty, .. }) =
                self.resources.get_mut(&id)
            else {
                continue;
            };
            let (Some(buf), Some(stride)) = (buf.as_ref(), *stride) else {
                continue;
            };
            for (start, count) in dirty_runs(dirty) {
                let off = start as usize * stride as usize;
                let len = count as usize * stride as usize;
                self.gpu
                    .queue
                    .write_buffer(buf, off as u64, &mirror[off..off + len]);
            }
            dirty.clear();
        }
    }

    /// 生のバイト列を elementOffset 番目の要素から書く。
    pub fn buffer_write(&mut self, id: u64, element_offset: u32, bytes: &[u8]) -> Result<(), String> {
        let stride = match self.resources.get(&id) {
            Some(Resource::Buffer { stride: Some(s), .. }) => *s,
            Some(Resource::Buffer { stride: None, .. }) => {
                return Err(
                    "ストライド未確定です（先にカーネルへ bind するか、gpu.buffer(n, stride) で作ってください）"
                        .into(),
                );
            }
            _ => return Err("不正なバッファ id".into()),
        };
        self.buffer_write_bytes(id, element_offset as u64 * stride as u64, bytes)
    }

    /// 生のバイト列をバイトオフセット指定で書く（こちらは即時転送）。
    ///
    /// 保留中のミラーを先に流し、書いた内容をミラーにも反映しておくことで、
    /// これと `buffer_set` を混ぜても呼び出し順どおりの結果になる。
    pub fn buffer_write_bytes(
        &mut self,
        id: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), String> {
        if byte_offset % 4 != 0 {
            return Err(format!(
                "バイトオフセットは 4 の倍数が必要です: {byte_offset}"
            ));
        }
        self.flush_dirty_buffers();
        let Some(Resource::Buffer { buf, stride, mirror, .. }) = self.resources.get_mut(&id) else {
            return Err("不正なバッファ id".into());
        };
        let (Some(buf), Some(_)) = (buf.as_ref(), *stride) else {
            return Err(
                "ストライド未確定です（先にカーネルへ bind するか、gpu.buffer(n, stride) で作ってください）"
                    .into(),
            );
        };
        if byte_offset + bytes.len() as u64 > buf.size() {
            return Err(format!(
                "書き込み範囲がバッファを超えています: オフセット {} + {} バイト（バッファは {} バイト）",
                byte_offset,
                bytes.len(),
                buf.size()
            ));
        }
        // ミラーを持っておかないと、後から at().set() された要素の
        // 「今回書いた分」が空で上書きされてしまう。
        if mirror.is_empty() {
            *mirror = vec![0u8; buf.size() as usize];
        }
        let off = byte_offset as usize;
        mirror[off..off + bytes.len()].copy_from_slice(bytes);
        self.gpu.queue.write_buffer(buf, byte_offset, bytes);
        Ok(())
    }

    pub fn buffer_read(&mut self, id: u64, out: &mut [u8]) -> Result<(), String> {
        self.flush_pending_pass();
        self.flush_dirty_buffers();
        // 保留中のコマンドを先に流す（present はしない）
        if let Some(enc) = self.frame.take() {
            self.gpu.queue.submit(Some(enc.finish()));
        }
        let Some(Resource::Buffer { buf, .. }) = self.resources.get(&id) else {
            return Err("不正なバッファ id".into());
        };
        let Some(buf) = buf else {
            return Err("ストライド未確定です（先にカーネルへ bind してください）".into());
        };
        let n = (out.len() as u64).min(buf.size());
        let staging = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpubridge.readback"),
            size: n,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(buf, 0, &staging, 0, n);
        self.gpu.queue.submit(Some(enc.finish()));
        let (tx, rx) = std::sync::mpsc::channel();
        staging.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        self.gpu
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("poll に失敗: {e}"))?;
        match rx.recv() {
            Ok(Ok(())) => {}
            other => return Err(format!("バッファの map に失敗: {other:?}")),
        }
        let data = staging
            .get_mapped_range(..)
            .map_err(|e| format!("mapped range の取得に失敗: {e}"))?;
        out[..n as usize].copy_from_slice(&data[..n as usize]);
        Ok(())
    }

    /// バッファ間の GPU コピー（記録コマンド）。呼び出し順どおり、先行する
    /// dispatch / draw の後に実行される。コピー先の CPU ミラーは更新されない
    /// （compute がバッファへ書く場合と同じ扱い）。
    pub fn copy_buffer_to_buffer(
        &mut self,
        src_id: u64,
        src_offset: u64,
        dst_id: u64,
        dst_offset: u64,
        byte_count: u64,
    ) -> Result<(), String> {
        if src_offset % 4 != 0 || dst_offset % 4 != 0 || byte_count % 4 != 0 {
            return Err(format!(
                "コピーのオフセット・バイト数は 4 の倍数が必要です: srcOffset={src_offset}, dstOffset={dst_offset}, byteCount={byte_count}"
            ));
        }
        if src_id == dst_id {
            return Err("同じバッファ内でのコピーはできません".into());
        }
        let size_of = |res: Option<&Resource>, what: &str| match res {
            Some(Resource::Buffer { buf: Some(b), .. }) => Ok(b.size()),
            Some(Resource::Buffer { buf: None, .. }) => Err(format!(
                "{what}バッファのストライドが未確定です（先にカーネルへ bind するか、gpu.buffer(n, stride) で作ってください）"
            )),
            _ => Err(format!("不正な{what}バッファ id")),
        };
        let src_size = size_of(self.resources.get(&src_id), "コピー元")?;
        let dst_size = size_of(self.resources.get(&dst_id), "コピー先")?;
        if src_offset + byte_count > src_size {
            return Err(format!(
                "コピー範囲がコピー元を超えています: オフセット {src_offset} + {byte_count} バイト（バッファは {src_size} バイト）"
            ));
        }
        if dst_offset + byte_count > dst_size {
            return Err(format!(
                "コピー範囲がコピー先を超えています: オフセット {dst_offset} + {byte_count} バイト（バッファは {dst_size} バイト）"
            ));
        }
        if byte_count == 0 {
            return Ok(());
        }
        // 記録順を保つ: 保留中の draw と dirty ミラーを先に流してからエンコードする
        self.flush_pending_pass();
        self.flush_dirty_buffers();
        if self.frame.is_none() {
            self.frame = Some(self.gpu.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("gpubridge.frame") },
            ));
        }
        let (
            Some(Resource::Buffer { buf: Some(src), .. }),
            Some(Resource::Buffer { buf: Some(dst), .. }),
        ) = (self.resources.get(&src_id), self.resources.get(&dst_id))
        else {
            unreachable!("検証済み");
        };
        self.frame
            .as_mut()
            .unwrap()
            .copy_buffer_to_buffer(src, src_offset, dst, dst_offset, byte_count);
        Ok(())
    }

    pub fn texture_write(&mut self, id: u64, bytes: &[u8], bytes_per_pixel: u32) -> Result<(), String> {
        let Some(Resource::Texture { tex, w, h, format, .. }) = self.resources.get(&id) else {
            return Err("不正なテクスチャ id".into());
        };
        let expected = *w as usize * *h as usize * bytes_per_pixel as usize;
        if bytes.len() < expected {
            return Err(format!(
                "データが足りません: {} バイト（{}x{} の {format:?} には {expected} バイト必要）",
                bytes.len(),
                w,
                h
            ));
        }
        self.gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes[..expected],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * bytes_per_pixel),
                rows_per_image: Some(*h),
            },
            wgpu::Extent3d { width: *w, height: *h, depth_or_array_layers: 1 },
        );
        Ok(())
    }

    /// 1 面（layer）1 段（mip）を書く。段 mip の大きさは w >> mip, h >> mip（最小 1）
    pub fn texture_write_layer(&mut self, id: u64, layer: u32, mip: u32, bytes: &[u8], bytes_per_pixel: u32) -> Result<(), String> {
        let Some(Resource::Texture { tex, w, h, format, cube, mips, .. }) = self.resources.get(&id) else {
            return Err("不正なテクスチャ id".into());
        };
        let layers = if *cube { 6 } else { 1 };
        if layer >= layers || mip >= *mips {
            return Err(format!("面 {layer} 段 {mip} はありません（面 {layers}、段 {mips}）"));
        }
        let mw = (*w >> mip).max(1);
        let mh = (*h >> mip).max(1);
        let expected = mw as usize * mh as usize * bytes_per_pixel as usize;
        if bytes.len() < expected {
            return Err(format!(
                "データが足りません: {} バイト（{mw}x{mh} の {format:?} には {expected} バイト必要）",
                bytes.len()
            ));
        }
        self.gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: mip,
                origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
                aspect: wgpu::TextureAspect::All,
            },
            &bytes[..expected],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(mw * bytes_per_pixel),
                rows_per_image: Some(mh),
            },
            wgpu::Extent3d { width: mw, height: mh, depth_or_array_layers: 1 },
        );
        Ok(())
    }

    pub fn texture_format(&self, id: u64) -> Option<wgpu::TextureFormat> {
        match self.resources.get(&id) {
            Some(Resource::Texture { format, .. }) => Some(*format),
            _ => None,
        }
    }

    // ---------------- render ----------------

    /// テクスチャをこの色で塗りつぶす（記録コマンド）。直後の draw の loadOp に畳まれる。
    pub fn clear(&mut self, tex_id: u64, r: f64, g: f64, b: f64, a: f64) -> Result<(), String> {
        self.require_2d(tex_id)?;
        if !matches!(self.resources.get(&tex_id), Some(Resource::Texture { .. })) {
            return Err("不正なテクスチャ id".into());
        }
        self.flush_pending_pass();
        self.pending_pass = Some(PendingPass {
            target: tex_id,
            clear: Some(wgpu::Color { r, g, b, a }),
            depth: None,
            draws: Vec::new(),
        });
        Ok(())
    }

    pub fn draw(
        &mut self,
        binding_id: u64,
        tex_id: u64,
        vertices: u32,
        instances: u32,
        scissor: Option<[i32; 4]>,
    ) -> Result<(), String> {
        self.draw_common(binding_id, tex_id, vertices, 0, instances, None, None, scissor)
    }

    /// インデックスバッファ経由の draw。`index_id` は u32（ストライド 4）で確定済みの
    /// GpuBuffer であること。`@builtin(vertex_index)` にはインデックス値が入るので、
    /// vertex pulling のシェーダーはそのまま共有頂点の参照になる。
    /// `first_index` はバッファ内の描き始め位置（要素単位）。
    pub fn draw_indexed(
        &mut self,
        binding_id: u64,
        tex_id: u64,
        index_id: u64,
        first_index: u32,
        index_count: u32,
        instances: u32,
        scissor: Option<[i32; 4]>,
    ) -> Result<(), String> {
        self.draw_common(binding_id, tex_id, index_count, first_index, instances, Some(index_id), None, scissor)
    }

    /// indirect 引数バッファ（offset から u32×5: indexCount, instanceCount,
    /// firstIndex, baseVertex, firstInstance）で drawIndexed する。
    /// インデックスバッファの要件は `draw_indexed` と同じ。
    pub fn draw_indexed_indirect(
        &mut self,
        binding_id: u64,
        tex_id: u64,
        index_id: u64,
        indirect_id: u64,
        indirect_offset: u64,
        scissor: Option<[i32; 4]>,
    ) -> Result<(), String> {
        self.draw_common(
            binding_id,
            tex_id,
            0,
            0,
            1,
            Some(index_id),
            Some((indirect_id, indirect_offset)),
            scissor,
        )
    }

    fn draw_common(
        &mut self,
        binding_id: u64,
        tex_id: u64,
        vertices: u32,
        first_index: u32,
        instances: u32,
        index: Option<u64>,
        indirect: Option<(u64, u64)>,
        scissor: Option<[i32; 4]>,
    ) -> Result<(), String> {
        let binding = self
            .render_bindings
            .get(&binding_id)
            .ok_or("不正なバインディング id")?;
        let renderer = &self.renderers[&binding.renderer_id];
        let depth_test = renderer.depth_test;
        let Some(Resource::Texture { format, w, h, cube, .. }) = self.resources.get(&tex_id) else {
            return Err("不正なテクスチャ id".into());
        };
        if *cube {
            return Err("キューブマップは描画先に使えません".into());
        }
        let (format, w, h) = (*format, *w, *h);

        // インデックスバッファの検証（ストライド 4 = u32 のみ）
        if let Some(idx) = index {
            match self.resources.get(&idx) {
                Some(Resource::Buffer { stride: Some(4), buf: Some(_), element_count, .. }) => {
                    let elems = *element_count;
                    let end = first_index.saturating_add(vertices);
                    if end > elems {
                        return Err(format!(
                            "インデックス範囲がバッファ外です: {first_index}..{end} (要素数 {elems})"
                        ));
                    }
                }
                Some(Resource::Buffer { stride: Some(s), .. }) => {
                    return Err(format!(
                        "インデックスに使うバッファはストライド 4（u32）が必要です（このバッファは {s} バイト）"
                    ));
                }
                Some(Resource::Buffer { stride: None, .. }) => {
                    return Err(
                        "インデックスバッファのストライドが未確定です。gpu.buffer(n, 4) で作るか、先に array<u32> としてカーネルへ bind してください"
                            .into(),
                    );
                }
                _ => return Err("不正なインデックスバッファ id".into()),
            }
        }
        let variant = RenderVariantKey {
            format,
            topology: renderer.topology,
            blend: renderer.blend,
            depth: depth_test,
        };

        // 1) キャッシュ構築（bind group → パイプラインバリアント）
        if self.render_bindings[&binding_id].cache.is_none() {
            self.build_render_bind_cache(binding_id)?;
        }
        if !self.render_bindings[&binding_id].pipelines.contains_key(&variant) {
            self.build_render_pipeline(binding_id, variant)?;
        }

        // 2) 深度テクスチャの確保（サイズ追従）
        if depth_test {
            let stale = match self.depth_textures.get(&tex_id) {
                Some((_, dw, dh)) => (*dw, *dh) != (w, h),
                None => true,
            };
            if stale {
                let tex = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("gpubridge.depth"),
                    size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth32Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                self.depth_textures.insert(tex_id, (view, w, h));

                // 新規テクスチャは 0.0 初期化のままだと depth_compare=Less で
                // 全フラグメントが不合格になり何も描けない。1.0 へ初期化する
                // 深度のみの空パスをこの場で即時エンコードする（このテクスチャを
                // 参照し得る保留 draw はまだ存在しないので順序上問題ない）。
                if self.frame.is_none() {
                    self.frame = Some(self.gpu.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor { label: Some("gpubridge.frame") },
                    ));
                }
                let init_view = &self.depth_textures[&tex_id].0;
                let enc = self.frame.as_mut().unwrap();
                {
                    let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("gpubridge.depth_init"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: init_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                }
            }
        }

        // 3) uniform / バッファの flush（dispatch と同じ順序保証）
        let uniform_ids = self.render_bindings[&binding_id]
            .cache
            .as_ref()
            .unwrap()
            .uniform_ids
            .clone();
        self.flush_uniforms(&uniform_ids)?;
        self.flush_dirty_buffers();

        // indirect 引数バッファの検証（u32×5 = 20 バイト）
        if let Some((ind_id, ind_off)) = indirect {
            self.check_indirect_args(ind_id, ind_off, 20)?;
        }

        // 4) パス結合の判定: ターゲットか深度の有無が変わったら別パス。計測中はレンダラが変わっても別パス
        let renderer_id = self.render_bindings[&binding_id].renderer_id;
        let same_renderer = |p: &PendingPass| match p.draws.last() {
            Some(d) => self.render_bindings[&d.binding_id].renderer_id == renderer_id,
            None => true,
        };
        let compatible = matches!(
            &self.pending_pass,
            Some(p) if p.target == tex_id && (p.depth.is_none() || p.depth == Some(depth_test))
                && (self.profile.is_none() || same_renderer(p))
        );
        if !compatible {
            self.flush_pending_pass();
            self.pending_pass = Some(PendingPass {
                target: tex_id,
                clear: None,
                depth: None,
                draws: Vec::new(),
            });
        }
        // シザー矩形の検証（負値・ゼロ面積は拒否。ターゲットからのはみ出しは flush 時にクランプ）
        let scissor = match scissor {
            None => None,
            Some([x, y, sw, sh]) => {
                if x < 0 || y < 0 || sw <= 0 || sh <= 0 {
                    return Err(format!("不正なシザー矩形: ({x}, {y}, {sw}, {sh})"));
                }
                Some([x as u32, y as u32, sw as u32, sh as u32])
            }
        };

        let offsets = self.uniform_offsets(&uniform_ids);
        let p = self.pending_pass.as_mut().unwrap();
        p.depth = Some(depth_test);
        p.draws.push(DrawCmd { binding_id, vertices, first_index, instances, offsets, variant, index, indirect, scissor });
        Ok(())
    }

    // ---------------- 画面出力 ----------------

    pub fn show(&mut self, tex_id: u64) -> Result<(), String> {
        self.require_2d(tex_id)?;
        match self.resources.get(&tex_id) {
            Some(Resource::Texture { format, .. }) => {
                if *format != wgpu::TextureFormat::Rgba8Unorm {
                    return Err(format!(
                        "show() は RGBA8 テクスチャのみ対応です（渡されたのは {format:?}）"
                    ));
                }
                self.show_tex = Some(tex_id);
                Ok(())
            }
            _ => Err("不正なテクスチャ id".into()),
        }
    }

    pub fn submit(&mut self) -> Result<(), String> {
        self.flush_pending_pass();
        // dispatch されないまま set された分も、フレーム終了時には GPU に届けておく
        self.flush_dirty_buffers();
        let mut enc = match self.frame.take() {
            Some(e) => e,
            None => self
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None }),
        };

        // show 指定があれば blit + present
        let mut presented = None;
        if let (Some(tex_id), Some(out)) = (self.show_tex, self.output.as_ref()) {
            let Resource::Texture { sampled_view, .. } = &self.resources[&tex_id] else {
                unreachable!()
            };
            use wgpu::CurrentSurfaceTexture as Cst;
            match out.surface.get_current_texture() {
                Cst::Success(f) | Cst::Suboptimal(f) => {
                    let view = f.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let bg = self.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("gpubridge.blit_bg"),
                        layout: &out.blit_pipeline.get_bind_group_layout(0),
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(sampled_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&out.sampler),
                            },
                        ],
                    });
                    {
                        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("gpubridge.blit"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                        pass.set_pipeline(&out.blit_pipeline);
                        pass.set_bind_group(0, &bg, &[]);
                        pass.draw(0..3, 0..1);
                    }
                    presented = Some(f);
                }
                // 一時的な状態はこのフレームの表示だけ諦める
                Cst::Timeout | Cst::Occluded | Cst::Outdated | Cst::Lost => {}
                other => return Err(format!("surface texture を取得できない: {other:?}")),
            }
        }

        if let Some(p) = &self.profile {
            let n = p.used.len() as u32 * 2;
            if n > 0 {
                enc.resolve_query_set(&p.query_set, 0..n, &p.resolve, 0);
                enc.copy_buffer_to_buffer(&p.resolve, 0, &p.read, 0, n as u64 * 8);
            }
        }
        self.gpu.queue.submit(Some(enc.finish()));
        if let Some(f) = presented {
            self.gpu.queue.present(f);
        }
        self.collect_profile()?;

        // uniform リングをフレーム先頭に巻き戻す（値は staging に保持されているので、
        // 次フレーム最初の dispatch で slot 0 に書き直される）
        for r in self.resources.values_mut() {
            if let Resource::Uniform(u) = r {
                u.next_slot = 0;
                u.dirty = true;
            }
        }
        Ok(())
    }
}

impl Engine {
    /// ウィンドウリサイズに追従して描画面を作り直す。
    pub fn resize_surface(&mut self, scale: f32, px_w: u32, px_h: u32) -> Result<(), String> {
        let Some(out) = self.output.as_ref() else {
            return Err("出力先が attach されていません".into());
        };
        crate::platform::resize_layer(&out._keepalive, scale, px_w, px_h);
        let caps = out.surface.get_capabilities(&self.gpu.adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        out.surface.configure(
            &self.gpu.device,
            &wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                color_space: wgpu::SurfaceColorSpace::default(),
                width: px_w.max(1),
                height: px_h.max(1),
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            },
        );
        Ok(())
    }
}

impl Engine {
    /// リソースの個別解放。
    ///
    /// wgpu の BindGroup はバッファ/テクスチャを内部で保持するため、テーブルから
    /// 消すだけでは VRAM が返らない。参照しているバインディングのキャッシュも
    /// 無効化し、bound からも外す（以後の dispatch は「未指定」エラーになる）。
    pub fn release_resource(&mut self, id: u64) -> Result<(), String> {
        self.flush_pending_pass();
        if self.resources.remove(&id).is_none() {
            return Err("不正なリソース id です（既に解放済みの可能性）".into());
        }
        self.depth_textures.remove(&id);
        if self.show_tex == Some(id) {
            self.show_tex = None;
        }
        for b in self.bindings.values_mut() {
            if b.bound.values().any(|&r| r == id) {
                b.bound.retain(|_, r| *r != id);
                b.cache = None;
            }
        }
        for b in self.render_bindings.values_mut() {
            if b.bound.values().any(|&r| r == id) {
                b.bound.retain(|_, r| *r != id);
                b.cache = None;
                b.pipelines.clear();
            }
        }
        Ok(())
    }
}
