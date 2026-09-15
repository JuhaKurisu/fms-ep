package imguibridge;

/** ImGui 描画用の WGSL。頂点は ImDrawVert(20 バイト)を u32×5 として vertex pulling で読む。 */
final class UiShader {

  private UiShader() {}

  static final String WGSL =
      """
      // targetSize: 描画先テクスチャの大きさ / offset: viewport の左上(target 座標)
      struct UiParams {
        targetSize: vec2<f32>,
        offset: vec2<f32>,
      }

      @group(0) @binding(0) var<uniform> params: UiParams;
      @group(0) @binding(1) var<storage, read> verts: array<u32>;
      @group(0) @binding(2) var tex: texture_2d<f32>;
      @group(0) @binding(3) var samp: sampler;

      struct VsOut {
        @builtin(position) pos: vec4<f32>,
        @location(0) uv: vec2<f32>,
        @location(1) col: vec4<f32>,
      }

      @vertex
      fn vsMain(@builtin(vertex_index) vid: u32) -> VsOut {
        let base = vid * 5u;
        let p = params.offset
            + vec2<f32>(bitcast<f32>(verts[base]), bitcast<f32>(verts[base + 1u]));
        let uv = vec2<f32>(bitcast<f32>(verts[base + 2u]), bitcast<f32>(verts[base + 3u]));
        var out: VsOut;
        out.pos = vec4<f32>(
            p.x / params.targetSize.x * 2.0 - 1.0,
            1.0 - p.y / params.targetSize.y * 2.0,
            0.0, 1.0);
        out.uv = uv;
        out.col = unpack4x8unorm(verts[base + 4u]);
        return out;
      }

      @fragment
      fn fsMain(in: VsOut) -> @location(0) vec4<f32> {
        return in.col * textureSample(tex, samp, in.uv);
      }
      """;
}
