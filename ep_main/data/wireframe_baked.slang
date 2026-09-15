// 分割後の三角形のワイヤーフレーム。全オブジェクトが 1 ドローに混ざるので色は 1 つ
import "relativity";
import "baked";

struct LineParams
{
    float3 wireColor;
};

ConstantBuffer<LineParams> lineParams;

[shader("vertex")]
VertexOutput vertexMain(uint vertexID: SV_VertexID)
{
    return bakedVertex(vertexID);
}

[shader("fragment")]
float4 fragmentMain(VertexOutput v) : SV_Target
{
    if (v.visibility <= 0.0)
        discard;

    return float4(lineParams.wireColor, 1.0);
}
