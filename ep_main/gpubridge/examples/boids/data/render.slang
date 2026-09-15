// boids の描画: 背景クリア + 点打ち。
struct Boid {
    float2 pos;
    float2 vel;
};

StructuredBuffer<Boid> boids;
[format("rgba8")]
WTexture2D<float4> canvas;

[shader("compute")]
[numthreads(8, 8, 1)]
void clearPass(uint3 id: SV_DispatchThreadID) {
    canvas.Store(int2(id.xy), float4(0.0, 0.0, 0.05, 1.0));
}

[shader("compute")]
[numthreads(64, 1, 1)]
void splat(uint3 id: SV_DispatchThreadID) {
    uint n, stride;
    boids.GetDimensions(n, stride);
    if (id.x >= n) return;
    canvas.Store(int2(boids[id.x].pos), float4(1.0));
}
