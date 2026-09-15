public struct Params
{
    public float2 resolution;
    public float3 observerPosition;
    public float4x4 viewProjection;

    public float3 observerVelocity;
    public float observerTime;
    public float lightSpeed;
    public float keyFrameInterval;
    public uint searchSteps;
};

// 指向性光源のシャドウ。光源空間は center を原点に right/up/forward の正規直交基底で、
// 深度 z = dot(p - center, forward) + depthRange/2 が [0, depthRange] に収まる範囲を焼く
public struct ShadowParams
{
    public float3 center;
    public float3 right;
    public float3 up;
    public float3 forward;   // 光の進む向き
    public float3 toLight;   // 光源へ向かう向き（= -forward、シェーディング用）
    public float halfExtent;
    public float depthRange;
    public float bias;
    public float ambient;
    public float lightIntensity;  // シェーディング用
    public uint pbrEnabled;       // 0 なら拡散のみ
    public uint iblEnabled;
    public float iblIntensity;
    public uint tonemapEnabled;
    public float exposure;
    public uint lutSize;      // 0 ならトーンマップなし
    public float softRadius;  // ぼかし半径（光源マップのテクセル単位）。0 でハードシャドウ
    public uint softSamples;  // 1 軸あたりのサンプル数（合計は 2 乗）
    public uint resolution;
    public uint layerCount;
    public int frame;
    public uint warmup;
    public uint enabled;
};


public struct BackgroundParams
{
    public float4 cameraRotation;   // 描画カメラの回転(x, y, z, w)
    public float tanHalfFov;        // 垂直画角の半分の tan
    public float aspect;
};
