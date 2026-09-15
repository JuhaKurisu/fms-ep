import "relativity";
import "baked";
import "tonemap";

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

    bool isCheck = (int(floor(v.retardedWorldPosition.x)) + int(floor(v.retardedWorldPosition.z))) % 2 == 0;
    float3 color = float3(isCheck ? 1 : 0);
    return float4(output(shade(v, color)), 1.0);
}
