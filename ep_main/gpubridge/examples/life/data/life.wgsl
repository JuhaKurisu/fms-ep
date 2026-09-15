// ライフゲーム。ping-pong する 2 枚のテクスチャを src/dst として受ける。
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn step(@builtin(global_invocation_id) id: vec3u) {
  let size = vec2i(textureDimensions(src));
  let p = vec2i(id.xy);
  var n = 0;
  for (var dy = -1; dy <= 1; dy++) {
    for (var dx = -1; dx <= 1; dx++) {
      if (dx == 0 && dy == 0) { continue; }
      let q = (p + vec2i(dx, dy) + size) % size;
      n += i32(textureLoad(src, q, 0).r > 0.5);
    }
  }
  let alive = textureLoad(src, p, 0).r > 0.5;
  let next = select(n == 3, n == 2 || n == 3, alive);
  textureStore(dst, p, vec4f(f32(next)));
}
