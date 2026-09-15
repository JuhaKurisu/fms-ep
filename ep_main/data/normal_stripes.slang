import "relativity";
import "baked";

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

    float3 baseColor = normalize(v.normal) * 0.5 + 0.5;
    float stripe = 0.6 + 0.4 * sin(v.retardedWorldPosition.y * 4.0 - params.observerTime * 3.0);
    return float4(baseColor * stripe, 1.0);
}
