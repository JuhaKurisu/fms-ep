// 露出 → log2 のシェーパー → 3D LUT の三線形補間。reference/src/tonemap.rs と lut.rs と同じ式
import "relativity";

public StructuredBuffer<float> tonemapLut;

static const float SHAPER_MIN_EV = -10.0;
static const float SHAPER_RANGE_EV = 16.5;

float3 shaper(float3 x)
{
    return saturate((log2(max(x, exp2(SHAPER_MIN_EV))) - SHAPER_MIN_EV) / SHAPER_RANGE_EV);
}

float3 lutFetch(uint n, uint r, uint g, uint b)
{
    let i = ((b * n + g) * n + r) * 3;
    return float3(tonemapLut[i], tonemapLut[i + 1], tonemapLut[i + 2]);
}

float3 lutSample(float3 t)
{
    let n = shadowParams.lutSize;
    let p = saturate(t) * float(n - 1);
    let i0 = min(uint3(floor(p)), uint3(n - 2));
    let f = p - float3(i0);
    let i1 = i0 + 1;
    let c00 = lerp(lutFetch(n, i0.x, i0.y, i0.z), lutFetch(n, i1.x, i0.y, i0.z), f.x);
    let c10 = lerp(lutFetch(n, i0.x, i1.y, i0.z), lutFetch(n, i1.x, i1.y, i0.z), f.x);
    let c01 = lerp(lutFetch(n, i0.x, i0.y, i1.z), lutFetch(n, i1.x, i0.y, i1.z), f.x);
    let c11 = lerp(lutFetch(n, i0.x, i1.y, i1.z), lutFetch(n, i1.x, i1.y, i1.z), f.x);
    return lerp(lerp(c00, c10, f.y), lerp(c01, c11, f.y), f.z);
}

// 画面に出す直前の色。オフなら今まで通り線形のまま
public float3 output(float3 linear)
{
    if (shadowParams.tonemapEnabled != 0 && shadowParams.lutSize >= 2)
        return lutSample(shaper(linear * shadowParams.exposure));
    return saturate(linear);
}
