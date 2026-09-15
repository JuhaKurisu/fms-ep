// 環境マップの読み出し。ミップ付きキューブマップをハードウェアのサンプラーで引く。段の意味と粗さ → 段の対応は reference/src/ibl.rs と同じ
public TextureCube<float4> envCube;   // 段 0 は元画像、段 k は粗さ 0.2k のぼかし
public TextureCube<float4> irrCube;
public SamplerState envSampler;

static const float ROUGHNESS_STEP = 0.2;
static const float CUBE_LAST_LOD = 5.0;

public float3 envLevel0(float3 d)
{
    return envCube.SampleLevel(envSampler, d, 0.0).rgb;
}

// 粗さ 0 は段 0、0.2 未満は段 0 と 1 の間、以後 0.2 刻み
float envLod(float roughness)
{
    float lod = roughness <= 0.0 ? 0.0
        : roughness < ROUGHNESS_STEP ? roughness / ROUGHNESS_STEP
        : 1.0 + (roughness - ROUGHNESS_STEP) / ROUGHNESS_STEP;
    return min(lod, CUBE_LAST_LOD);
}

public float3 envSpecular(float3 d, float roughness)
{
    return envCube.SampleLevel(envSampler, d, envLod(roughness)).rgb;
}

public float3 envIrradiance(float3 n)
{
    return irrCube.SampleLevel(envSampler, n, 0.0).rgb;
}

// Karis の環境 BRDF 近似。specular = prefiltered * (F0 * A + B)
public float2 envBrdfApprox(float roughness, float NoV)
{
    let c0 = float4(-1.0, -0.0275, -0.572, 0.022);
    let c1 = float4(1.0, 0.0425, 1.04, -0.04);
    let r = roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * NoV)) * r.x + r.y;
    return float2(-1.04, 1.04) * a004 + r.zw;
}
