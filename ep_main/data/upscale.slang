// 縮小して描いたシーンを、全画面の三角形 1 枚で画面いっぱいに引き伸ばす
Texture2D<float4> source;
SamplerState sourceSampler;

struct UpscaleVertex
{
    float4 position : SV_Position;
    float2 uv : TEXCOORD0;
};

[shader("vertex")]
UpscaleVertex vertexMain(uint vertexID: SV_VertexID)
{
    float2 p = float2(vertexID == 1 ? 3.0 : -1.0, vertexID == 2 ? 3.0 : -1.0);
    UpscaleVertex out;
    out.position = float4(p, 1.0, 1.0);
    out.uv = float2(p.x, -p.y) * 0.5 + 0.5;
    return out;
}

[shader("fragment")]
float4 fragmentMain(UpscaleVertex v) : SV_Target
{
    return source.SampleLevel(sourceSampler, v.uv, 0.0);
}
