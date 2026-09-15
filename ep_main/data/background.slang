// 全画面の三角形 1 枚で環境マップを背景に描く。見かけの方向を光行差で実験室系に戻してから引く
import "types";
import "relativity";
import "quaternion";
import "ibl";
import "tonemap";

public ConstantBuffer<BackgroundParams> backgroundParams;

struct BackgroundVertex
{
    float4 position : SV_Position;
    float2 ndc : TEXCOORD0;
};

[shader("vertex")]
BackgroundVertex vertexMain(uint vertexID: SV_VertexID)
{
    float2 p = float2(vertexID == 1 ? 3.0 : -1.0, vertexID == 2 ? 3.0 : -1.0);
    BackgroundVertex out;
    out.position = float4(p, 1.0, 1.0);
    out.ndc = p;
    return out;
}

[shader("fragment")]
float4 fragmentMain(BackgroundVertex v) : SV_Target
{
    let t = backgroundParams.tanHalfFov;
    let dirView = normalize(float3(v.ndc.x * backgroundParams.aspect * t, v.ndc.y * t, 1.0));
    let apparent = quatRotate(backgroundParams.cameraRotation, dirView);
    let beta = params.observerVelocity / params.lightSpeed;
    let lab = aberrateDirection(apparent, -beta);
    if (shadowParams.iblEnabled == 0)
        return float4(output(float3(1.0)), 1.0);
    return float4(output(envLevel0(lab)), 1.0);
}
