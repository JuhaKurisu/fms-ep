// GPU カリングの compute 3 パス。
// cull が「見える粒子」を visible に詰めながら drawArgs[1]（instanceCount）を数え、
// prepare がその個数から wobble のワークグループ数を作る。
// CPU はこのフレームで何個描くかを一度も知らない。

struct Params {
  mouse: vec2<f32>,
  radius: f32,
  time: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> particles: array<vec2<f32>>;
@group(0) @binding(2) var<storage, read_write> visible: array<u32>;
// drawIndexedIndirect の引数そのもの:
// {indexCount, instanceCount, firstIndex, baseVertex, firstInstance}
@group(0) @binding(3) var<storage, read_write> drawArgs: array<atomic<u32>>;
// dispatchIndirect の引数そのもの: {x, y, z} ワークグループ数
@group(0) @binding(4) var<storage, read_write> dispatchArgs: array<u32>;
@group(0) @binding(5) var<storage, read_write> offsets: array<vec2<f32>>;

// マウス周りの粒子だけを visible に詰める
@compute @workgroup_size(64)
fn cull(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if (i >= arrayLength(&particles)) {
    return;
  }
  if (distance(particles[i], params.mouse) < params.radius) {
    let slot = atomicAdd(&drawArgs[1], 1u);
    visible[slot] = i;
  }
}

// 数え終わった個数 → wobble のワークグループ数
@compute @workgroup_size(1)
fn prepare() {
  let count = atomicLoad(&drawArgs[1]);
  dispatchArgs[0] = (count + 63u) / 64u;
  dispatchArgs[1] = 1u;
  dispatchArgs[2] = 1u;
}

// 見えている粒子の分だけ実行される（dispatchIndirect）
@compute @workgroup_size(64)
fn wobble(@builtin(global_invocation_id) gid: vec3<u32>) {
  let n = atomicLoad(&drawArgs[1]);
  if (gid.x >= n) {
    return;
  }
  let a = params.time * 4.0 + f32(gid.x) * 0.61;
  offsets[gid.x] = vec2<f32>(cos(a), sin(a)) * 5.0;
}
