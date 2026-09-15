import "relativity";

[shader("vertex")]
VertexOutput vertexMain(uint vertexID: SV_VertexID)
{
    return relativisticVertex(vertexID);
}

[shader("fragment")]
float4 fragmentMain(VertexOutput v) : SV_Target
{
    if (v.visibility <= 0.0)
        discard;

    return float4(shaderParams.wireColor, 1.0);
}
