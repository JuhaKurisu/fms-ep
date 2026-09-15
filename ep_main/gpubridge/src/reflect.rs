//! WGSL のリフレクション。
//!
//! naga で WGSL を解析し、Java 側 API の検証に必要な情報
//! （binding の名前・種別、uniform struct のレイアウト、配列ストライド、
//! workgroup_size、エントリポイントごとの使用 binding）を抽出する。
//! ここで読める情報は Java 側に再宣言させない、が設計原則。

/// 1 つの WGSL ソースから抽出した全情報。
#[derive(Debug)]
pub struct ShaderInfo {
    pub entry_points: Vec<EntryInfo>,
    /// vertex / fragment のエントリポイント。renderer(vs, fs) の解決に使う。
    pub render_entries: Vec<RenderEntry>,
    /// group(0) の binding 一覧（binding 番号順とは限らない）
    pub bindings: Vec<BindingInfo>,
}

#[derive(Debug)]
pub struct EntryInfo {
    pub name: String,
    pub workgroup_size: [u32; 3],
    /// このエントリポイントが（ヘルパー関数経由も含め）使う binding。
    /// ShaderInfo.bindings への index。
    pub used_bindings: Vec<usize>,
}

/// vertex または fragment のエントリポイント。
#[derive(Debug)]
pub struct RenderEntry {
    pub name: String,
    pub stage: naga::ShaderStage,
    /// このエントリポイントが（ヘルパー関数経由も含め）使う binding。
    /// ShaderInfo.bindings への index。
    pub used_bindings: Vec<usize>,
}

#[derive(Debug)]
pub struct BindingInfo {
    pub name: String,
    pub binding: u32,
    pub kind: BindingKind,
}

#[derive(Debug)]
pub enum BindingKind {
    Uniform {
        layout: StructLayout,
    },
    Storage {
        read_only: bool,
        /// array<T> なら T のストライド。非配列なら型全体のサイズ。
        stride: u32,
        /// 要素が struct なら、そのメンバのレイアウト。
        /// スカラー要素や、非対応のメンバ型を含む struct では `None`
        /// （その場合はメンバ名指定の書き込みができず、生の write のみ）。
        layout: Option<StructLayout>,
    },
    StorageTexture {
        format: wgpu::TextureFormat,
        write_only: bool,
    },
    SampledTexture {
        /// texture_cube なら true（2D 以外はキューブのみ対応）
        cube: bool,
    },
    Sampler,
}

#[derive(Debug, Clone)]
pub struct StructLayout {
    pub size: u32,
    pub members: Vec<MemberInfo>,
}

#[derive(Debug, Clone)]
pub struct MemberInfo {
    pub name: String,
    pub offset: u32,
    pub kind: MemberKind,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum MemberKind {
    F32,
    I32,
    U32,
    /// vec2/vec3/vec4 of f32
    VecF(u8),
    /// mat4x4 of f32（列優先の連続 16 f32。Slang の列優先ラッパー struct も含む）
    Mat4,
}

/// WGSL をパース・検証し、リフレクション情報を返す。
/// エラーは人間が読める日本語メッセージ。
pub fn reflect(wgsl: &str) -> Result<ShaderInfo, String> {
    let module = naga::front::wgsl::parse_str(wgsl)
        .map_err(|e| format!("シェーダの構文エラー:\n{}", e.emit_to_string(wgsl)))?;

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    let info = validator
        .validate(&module)
        .map_err(|e| format!("シェーダの検証エラー:\n{}", e.emit_to_string(wgsl)))?;

    // ---- bindings ----
    let mut bindings = Vec::new();
    // naga の global handle → bindings の index
    let mut handle_to_index = std::collections::HashMap::new();

    for (handle, var) in module.global_variables.iter() {
        let Some(rb) = &var.binding else { continue };
        let name = var.name.clone().unwrap_or_default();
        if rb.group != 0 {
            return Err(format!(
                "'{name}' が @group({}) にありますが、group(0) のみ対応しています",
                rb.group
            ));
        }
        let kind = classify(&module, var, &name)?;
        handle_to_index.insert(handle, bindings.len());
        bindings.push(BindingInfo {
            name,
            binding: rb.binding,
            kind,
        });
    }

    // ---- entry points ----
    let mut entry_points = Vec::new();
    let mut render_entries = Vec::new();
    for (i, ep) in module.entry_points.iter().enumerate() {
        let ep_info = info.get_entry_point(i);
        let used_bindings: Vec<usize> = module
            .global_variables
            .iter()
            .filter(|(h, v)| v.binding.is_some() && !ep_info[*h].is_empty())
            .map(|(h, _)| handle_to_index[&h])
            .collect();
        match ep.stage {
            naga::ShaderStage::Compute => entry_points.push(EntryInfo {
                name: ep.name.clone(),
                workgroup_size: ep.workgroup_size,
                used_bindings,
            }),
            naga::ShaderStage::Vertex | naga::ShaderStage::Fragment => {
                render_entries.push(RenderEntry {
                    name: ep.name.clone(),
                    stage: ep.stage,
                    used_bindings,
                });
            }
            _ => {}
        }
    }

    Ok(ShaderInfo {
        entry_points,
        render_entries,
        bindings,
    })
}

/// global 変数を BindingKind に分類する。
fn classify(
    module: &naga::Module,
    var: &naga::GlobalVariable,
    name: &str,
) -> Result<BindingKind, String> {
    use naga::TypeInner as T;
    let ty = &module.types[var.ty].inner;

    match (var.space, ty) {
        (naga::AddressSpace::Uniform, T::Struct { members, span }) => Ok(BindingKind::Uniform {
            layout: struct_layout(module, members, *span, name)?,
        }),
        (naga::AddressSpace::Uniform, _) => Err(format!(
            "uniform '{name}' は struct にしてください（スカラー単体の uniform は非対応）"
        )),
        (naga::AddressSpace::Storage { access }, inner) => {
            let element = match inner {
                T::Array { base, .. } => &module.types[*base].inner,
                other => other,
            };
            let stride = match inner {
                T::Array { stride, .. } => *stride,
                other => type_size(module, other)
                    .ok_or_else(|| format!("storage '{name}' の型サイズを決定できません"))?,
            };
            // 非対応のメンバ型を含む struct は None に落とす（生の write は使えるので、
            // ここで shader 全体を弾く理由はない）。
            let layout = match element {
                T::Struct { members, span } => struct_layout(module, members, *span, name).ok(),
                _ => None,
            };
            Ok(BindingKind::Storage {
                read_only: !access.contains(naga::StorageAccess::STORE),
                stride,
                layout,
            })
        }
        (_, T::Image { dim, arrayed, class }) => match class {
            naga::ImageClass::Storage { format, access } => Ok(BindingKind::StorageTexture {
                format: convert_format(*format, name)?,
                write_only: !access.contains(naga::StorageAccess::LOAD),
            }),
            _ => match (dim, arrayed) {
                (naga::ImageDimension::D2, false) => Ok(BindingKind::SampledTexture { cube: false }),
                (naga::ImageDimension::Cube, false) => Ok(BindingKind::SampledTexture { cube: true }),
                _ => Err(format!("texture '{name}' は 2D かキューブ（配列なし）のみ対応です")),
            },
        },
        (_, T::Sampler { .. }) => Ok(BindingKind::Sampler),
        _ => Err(format!("binding '{name}' の種別を判別できません")),
    }
}

fn struct_layout(
    module: &naga::Module,
    members: &[naga::StructMember],
    span: u32,
    struct_name: &str,
) -> Result<StructLayout, String> {
    let mut out = Vec::new();
    for m in members {
        let name = m.name.clone().unwrap_or_default();
        let kind = match &module.types[m.ty].inner {
            naga::TypeInner::Scalar(s) => match s.kind {
                naga::ScalarKind::Float => MemberKind::F32,
                naga::ScalarKind::Sint => MemberKind::I32,
                naga::ScalarKind::Uint => MemberKind::U32,
                other => {
                    return Err(format!(
                        "uniform '{struct_name}.{name}' の型 {other:?} は非対応です"
                    ));
                }
            },
            naga::TypeInner::Vector { size, scalar } if scalar.kind == naga::ScalarKind::Float => {
                MemberKind::VecF(*size as u8)
            }
            naga::TypeInner::Matrix { columns, rows, scalar }
                if *columns == naga::VectorSize::Quad
                    && *rows == naga::VectorSize::Quad
                    && scalar.kind == naga::ScalarKind::Float =>
            {
                MemberKind::Mat4
            }
            naga::TypeInner::Struct { members: inner, span }
                if is_mat4_storage(module, inner, *span) =>
            {
                MemberKind::Mat4
            }
            other => {
                return Err(format!(
                    "uniform '{struct_name}.{name}' の型は非対応です（f32/i32/u32/vecNf/mat4x4f のみ）: {other:?}"
                ));
            }
        };
        out.push(MemberInfo {
            name,
            offset: m.offset,
            kind,
        });
    }
    Ok(StructLayout {
        size: span,
        members: out,
    })
}

/// Slang が float4x4 の uniform メンバとして吐くラッパー struct
/// （data: array&lt;vec4&lt;f32&gt;, 4&gt; だけを持つ span 64 の struct）かどうか。
fn is_mat4_storage(module: &naga::Module, members: &[naga::StructMember], span: u32) -> bool {
    if span != 64 || members.len() != 1 || members[0].offset != 0 {
        return false;
    }
    let naga::TypeInner::Array { base, size, .. } = &module.types[members[0].ty].inner else {
        return false;
    };
    let is_vec4f = matches!(
        &module.types[*base].inner,
        naga::TypeInner::Vector { size: naga::VectorSize::Quad, scalar }
            if scalar.kind == naga::ScalarKind::Float
    );
    is_vec4f && matches!(size, naga::ArraySize::Constant(n) if n.get() == 4)
}

fn type_size(_module: &naga::Module, inner: &naga::TypeInner) -> Option<u32> {
    use naga::TypeInner as T;
    match inner {
        T::Scalar(s) => Some(s.width as u32),
        T::Vector { size, scalar } => Some(*size as u32 * scalar.width as u32),
        T::Struct { span, .. } => Some(*span),
        _ => None,
    }
    .or_else(|| {
        // atomic<u32> など
        if let T::Atomic(s) = inner {
            Some(s.width as u32)
        } else {
            None
        }
    })
}

/// naga の StorageFormat → wgpu::TextureFormat（対応分のみ）
fn convert_format(f: naga::StorageFormat, name: &str) -> Result<wgpu::TextureFormat, String> {
    use naga::StorageFormat as S;
    use wgpu::TextureFormat as W;
    Ok(match f {
        S::Rgba8Unorm => W::Rgba8Unorm,
        S::Rgba32Float => W::Rgba32Float,
        S::R32Float => W::R32Float,
        S::Rgba16Float => W::Rgba16Float,
        S::Rg32Float => W::Rg32Float,
        other => {
            return Err(format!(
                "storage texture '{name}' のフォーマット {other:?} は非対応です\
                 （rgba8unorm / rgba16float / rgba32float / rg32float / r32float）"
            ));
        }
    })
}

/// 「'srcc' はありません。候補: src」形式のエラー文を作る。
pub fn suggest(name: &str, candidates: &[&str]) -> String {
    let best = candidates
        .iter()
        .min_by_key(|c| levenshtein(name, c))
        .copied();
    match best {
        Some(b) => format!("'{name}' はありません。候補: {b}"),
        None => format!("'{name}' はありません"),
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn entry_points_and_workgroup_size() {
        let info = reflect(SRC).unwrap();
        let update = info.entry_points.iter().find(|e| e.name == "update").unwrap();
        assert_eq!(update.workgroup_size, [64, 1, 1]);
        let splat = info.entry_points.iter().find(|e| e.name == "splat").unwrap();
        assert_eq!(splat.workgroup_size, [8, 8, 1]);
    }

    #[test]
    fn per_entry_used_bindings_includes_helper_usage() {
        let info = reflect(SRC).unwrap();
        let update = info.entry_points.iter().find(|e| e.name == "update").unwrap();
        let names: Vec<&str> = update
            .used_bindings
            .iter()
            .map(|&i| info.bindings[i].name.as_str())
            .collect();
        assert!(names.contains(&"params")); // helper 経由の使用
        assert!(names.contains(&"src"));
        assert!(names.contains(&"dst"));
        assert!(!names.contains(&"canvas"));
    }

    #[test]
    fn uniform_layout_has_padding_aware_offsets() {
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
        assert_eq!(layout.size, 24);
        let g = layout.members.iter().find(|m| m.name == "gravity").unwrap();
        assert_eq!(g.offset, 8); // vec2 アライメント
        assert!(matches!(g.kind, MemberKind::VecF(2)));
    }

    /// Slang が float4x4 を吐く形（列優先ラッパー struct）と、素の mat4x4 の両方が
    /// Mat4 として見えること
    #[test]
    fn uniform_matrix_members_are_mat4() {
        let src = r#"
struct MatStorage { @align(16) data: array<vec4<f32>, 4> };
struct P { a: f32, @align(16) m: MatStorage, n: mat4x4<f32> };
@group(0) @binding(0) var<uniform> params: P;
@compute @workgroup_size(1) fn main() { let x = params.a; }
"#;
        let info = reflect(src).unwrap();
        let BindingKind::Uniform { layout } = &info.bindings[0].kind else {
            panic!()
        };
        let m = layout.members.iter().find(|m| m.name == "m").unwrap();
        assert_eq!(m.offset, 16);
        assert!(matches!(m.kind, MemberKind::Mat4));
        let n = layout.members.iter().find(|m| m.name == "n").unwrap();
        assert_eq!(n.offset, 80);
        assert!(matches!(n.kind, MemberKind::Mat4));
        assert_eq!(layout.size, 144);
    }

    #[test]
    fn storage_stride_and_access() {
        let info = reflect(SRC).unwrap();
        let BindingKind::Storage { read_only, stride, .. } =
            &info.bindings.iter().find(|b| b.name == "src").unwrap().kind
        else {
            panic!()
        };
        assert!(*read_only);
        assert_eq!(*stride, 16);
    }

    #[test]
    fn storage_element_layout_is_exposed() {
        let info = reflect(SRC).unwrap();
        let BindingKind::Storage { layout, .. } =
            &info.bindings.iter().find(|b| b.name == "src").unwrap().kind
        else {
            panic!()
        };
        let layout = layout.as_ref().expect("Boid の要素レイアウトが取れること");
        assert_eq!(layout.size, 16);
        let vel = layout.members.iter().find(|m| m.name == "vel").unwrap();
        assert_eq!(vel.offset, 8);
        assert!(matches!(vel.kind, MemberKind::VecF(2)));
    }

    /// Slang が float3 を含む struct から吐く形。type の後ろに 12 バイトの穴が空く。
    #[test]
    fn storage_element_layout_has_padding_aware_offsets() {
        let src = r#"
struct Object { @align(16) ty: i32, @align(16) position: vec3<f32>, @align(4) param0: f32 };
@group(0) @binding(0) var<storage, read_write> objects: array<Object>;
@compute @workgroup_size(1) fn m() { objects[0].param0 = 1.0; }
"#;
        let info = reflect(src).unwrap();
        let BindingKind::Storage { stride, layout, .. } = &info.bindings[0].kind else {
            panic!()
        };
        assert_eq!(*stride, 32);
        let layout = layout.as_ref().unwrap();
        assert_eq!(layout.size, 32);
        let by = |n: &str| layout.members.iter().find(|m| m.name == n).unwrap().offset;
        assert_eq!(by("ty"), 0);
        assert_eq!(by("position"), 16);
        assert_eq!(by("param0"), 28);
    }

    /// 要素が struct でない storage buffer はレイアウト無し（write(float[]) だけが使える）。
    #[test]
    fn scalar_element_storage_has_no_layout() {
        let src = r#"
@group(0) @binding(0) var<storage, read_write> data: array<f32>;
@compute @workgroup_size(1) fn m() { data[0] = 1.0; }
"#;
        let info = reflect(src).unwrap();
        let BindingKind::Storage { stride, layout, .. } = &info.bindings[0].kind else {
            panic!()
        };
        assert_eq!(*stride, 4);
        assert!(layout.is_none());
    }

    #[test]
    fn storage_texture_format() {
        let info = reflect(SRC).unwrap();
        let BindingKind::StorageTexture { format, .. } = &info
            .bindings
            .iter()
            .find(|b| b.name == "canvas")
            .unwrap()
            .kind
        else {
            panic!()
        };
        assert_eq!(*format, wgpu::TextureFormat::Rgba8Unorm);
    }

    #[test]
    fn parse_error_is_readable() {
        let err = reflect("@compute fn broken(){").unwrap_err();
        assert!(err.contains("シェーダ"));
    }

    #[test]
    fn cube_texture_is_recognized() {
        let src = "@group(0) @binding(0) var env: texture_cube<f32>;\n@group(0) @binding(1) var s: sampler;\n@group(0) @binding(2) var flat: texture_2d<f32>;\n@fragment fn m() -> @location(0) vec4<f32> { return textureSampleLevel(env, s, vec3<f32>(0.0, 1.0, 0.0), 0.0) + textureSampleLevel(flat, s, vec2<f32>(0.5), 0.0); }";
        let info = reflect(src).unwrap();
        let kind = |n: &str| &info.bindings.iter().find(|b| b.name == n).unwrap().kind;
        assert!(matches!(kind("env"), BindingKind::SampledTexture { cube: true }));
        assert!(matches!(kind("flat"), BindingKind::SampledTexture { cube: false }));
    }

    #[test]
    fn nonzero_group_is_rejected() {
        let src = "struct P { v: f32 };\n@group(1) @binding(0) var<uniform> p: P;\n@compute @workgroup_size(1) fn m() { let x = p.v; }";
        assert!(reflect(src).unwrap_err().contains("group(0)"));
    }

    #[test]
    fn suggest_gives_closest() {
        assert!(suggest("srcc", &["src", "dst", "params"]).contains("src"));
    }

    #[test]
    fn demangle_strips_slang_suffix() {
        assert_eq!(demangle("src_0"), "src");
        assert_eq!(demangle("feed_12"), "feed");
        assert_eq!(demangle("src"), "src");       // 手書き名はそのまま
        assert_eq!(demangle("vec_2d"), "vec_2d"); // 数字でない末尾は剥がさない
        assert_eq!(demangle("_0"), "_0");         // 先頭アンダースコアだけの名前は保護
    }

    #[test]
    fn resolve_prefers_exact_then_demangled() {
        assert_eq!(resolve_name(&["src_0", "dst_0"], "src"), Ok(0));
        assert_eq!(resolve_name(&["src", "src_0"], "src"), Ok(0));    // 完全一致優先
        assert!(resolve_name(&["src_0", "src_1"], "src").unwrap_err().is_some()); // 曖昧
        assert!(resolve_name(&["a", "b"], "c").unwrap_err().is_none());           // 不在
    }

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

    #[test]
    fn render_entries_are_collected_with_stage() {
        let info = reflect(RENDER_SRC).unwrap();
        let vs = info.render_entries.iter().find(|e| e.name == "vsMain").unwrap();
        assert_eq!(vs.stage, naga::ShaderStage::Vertex);
        let fs = info.render_entries.iter().find(|e| e.name == "fsMain").unwrap();
        assert_eq!(fs.stage, naga::ShaderStage::Fragment);
        // render エントリは compute の一覧には現れない
        assert!(info.entry_points.iter().all(|e| e.name != "vsMain"));
    }

    #[test]
    fn render_entry_used_bindings_are_per_stage() {
        let info = reflect(RENDER_SRC).unwrap();
        let names = |e: &RenderEntry| -> Vec<&str> {
            e.used_bindings.iter().map(|&i| info.bindings[i].name.as_str()).collect()
        };
        let vs = info.render_entries.iter().find(|e| e.name == "vsMain").unwrap();
        assert!(names(vs).contains(&"params"));
        assert!(names(vs).contains(&"boids"));
        let fs = info.render_entries.iter().find(|e| e.name == "fsMain").unwrap();
        assert!(names(fs).contains(&"params"));
        assert!(!names(fs).contains(&"boids"));
    }

    #[test]
    fn compute_only_source_has_no_render_entries() {
        let info = reflect(SRC).unwrap();
        assert!(info.render_entries.is_empty());
    }
}

/// Slang が WGSL 出力時に付ける `_数字` サフィックスを除いた名前。
/// 手書き WGSL の名前はそのまま返る。
pub fn demangle(name: &str) -> &str {
    if let Some(pos) = name.rfind('_') {
        let digits = &name[pos + 1..];
        if pos > 0 && !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
            return &name[..pos];
        }
    }
    name
}

/// 名前解決: 完全一致を優先し、なければマングル除去後の一致を探す。
/// 戻り値: Ok(index)。0 件は Err(None)、複数一致は Err(Some(曖昧エラー文))。
pub fn resolve_name(names: &[&str], want: &str) -> Result<usize, Option<String>> {
    if let Some(i) = names.iter().position(|n| *n == want) {
        return Ok(i);
    }
    let hits: Vec<usize> = names
        .iter()
        .enumerate()
        .filter(|(_, n)| demangle(n) == want)
        .map(|(i, _)| i)
        .collect();
    match hits.len() {
        1 => Ok(hits[0]),
        0 => Err(None),
        _ => Err(Some(format!(
            "'{want}' に一致する候補が複数あります: {}",
            hits.iter().map(|&i| names[i]).collect::<Vec<_>>().join(", ")
        ))),
    }
}

