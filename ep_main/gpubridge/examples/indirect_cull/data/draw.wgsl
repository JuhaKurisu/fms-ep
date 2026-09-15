// 間接描画側。instanceCount は cull パスが drawArgs に書いた値が使われるので、
// ここは visible[] を素直に vertex pulling するだけ。
// （vertex ステージから読むバッファは read 専用で宣言する）

struct DrawParams {
  resolution: vec2<f32>,
  size: f32,
};

@group(0) @binding(0) var<uniform> rparams: DrawParams;
@group(0) @binding(1) var<storage, read> particles: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read> visible: array<u32>;
@group(0) @binding(3) var<storage, read> offsets: array<vec2<f32>>;

struct VOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vid: u32, @builtin(instance_index) iid: u32) -> VOut {
  // インデックスバッファ {0,1,2,2,1,3} で四隅 → quad
  let corner = vec2<f32>(f32(vid & 1u), f32(vid >> 1u)) * 2.0 - 1.0;
  let center = particles[visible[iid]] + offsets[iid];
  let px = center + corner * rparams.size;
  var o: VOut;
  o.pos = vec4<f32>(px / rparams.resolution * 2.0 - 1.0, 0.0, 1.0);
  o.pos.y = -o.pos.y;
  o.uv = corner;
  return o;
}

@fragment
fn fs(v: VOut) -> @location(0) vec4<f32> {
  let d = length(v.uv);
  if (d > 1.0) {
    discard;
  }
  let glow = 1.0 - d;
  return vec4<f32>(0.3 + glow * 0.7, 0.7, 1.0, glow);
}
