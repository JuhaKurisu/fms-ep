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

    return float4(output(shade(v, float3(1.0, 1.0, 1.0))), 1.0);
}
