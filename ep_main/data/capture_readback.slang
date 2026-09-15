// view テクスチャを CPU で読めるよう buffer に写す。gpubridge は float でしか読み戻せないので、
// 1 画素を r * 65536 + g * 256 + b(24 bit、float で正確に表せる)の 1 値にする
Texture2D<float4> source;
RWStructuredBuffer<float> pixels;

[shader("compute")]
[numthreads(8, 8, 1)]
void readback(uint3 id: SV_DispatchThreadID)
{
    uint w, h;
    source.GetDimensions(w, h);
    if (id.x >= w || id.y >= h)
        return;
    let c = uint3(round(saturate(source.Load(int3(id.xy, 0)).rgb) * 255));
    pixels[id.y * w + id.x] = float(c.r * 65536 + c.g * 256 + c.b);
}
