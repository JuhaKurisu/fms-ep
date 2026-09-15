// Gray-Scott 反応拡散（Karl Sims 版のスキームとパラメータ）。
// field は RGBA32F: r=u（基質）, g=v（触媒）。

struct Params {
    float feed;
    float kill;
    float2 seed;
};

ConstantBuffer<Params> params;
Texture2D<float4> src;
WTexture2D<float4> dst;            // 書き込み先は WTexture2D（RWTexture2D は WebGPU で不可）
[format("rgba8")]
WTexture2D<float4> view;

// Karl Sims 版の 9 点ラプラシアン（中心 -1、辺 0.2、角 0.05）。トーラス境界。
float2 laplacian(int2 p, int2 size) {
    float2 lap = -1.0 * src[p].rg;
    for (int dy = -1; dy <= 1; dy++) {
        for (int dx = -1; dx <= 1; dx++) {
            if (dx == 0 && dy == 0) continue;
            float w = (dx == 0 || dy == 0) ? 0.2 : 0.05;
            int2 q = (p + int2(dx, dy) + size) % size;
            lap += w * src[q].rg;
        }
    }
    return lap;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void evolve(uint3 id: SV_DispatchThreadID) {
    uint2 dim;
    src.GetDimensions(dim.x, dim.y);
    int2 size = int2(dim);
    int2 p = int2(id.xy);
    if (p.x >= size.x || p.y >= size.y) return;

    float2 c = src[p].rg;
    float2 lap = laplacian(p, size);

    // Du=1.0, Dv=0.5, dt=1.0（Karl Sims の標準値）
    float uvv = c.x * c.y * c.y;
    float2 next = c + float2(
        1.0 * lap.x - uvv + params.feed * (1.0 - c.x),
        0.5 * lap.y + uvv - (params.feed + params.kill) * c.y);

    // マウス位置に触媒を注入
    if (distance(float2(p), params.seed) < 16.0) next.y = 0.9;

    dst.Store(p, float4(clamp(next, 0.0, 1.0), 0.0, 1.0));
}

[shader("compute")]
[numthreads(8, 8, 1)]
void show(uint3 id: SV_DispatchThreadID) {
    float v = src[int2(id.xy)].g;
    // v を青緑系に色付け
    float3 c = float3(0.1, 0.2, 0.25) + float3(0.2, 2.5, 3.0) * v;
    view.Store(int2(id.xy), float4(c, 1.0));
}
