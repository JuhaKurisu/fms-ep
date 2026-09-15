// 光源視点の瞬間 depth。現在時刻の位置（ローレンツ収縮込み）を正射影で描き、光源空間の深度を R32F に書く
import "relativity";

struct ShadowVertexOutput
{
    float4 position : SV_Position;
    float depth : TEXCOORD0;
};

[shader("vertex")]
ShadowVertexOutput vertexMain(uint vertexID: SV_VertexID)
{
    let vertex = vertices[vertexID];
    let newestIndex = uint(max(round((params.observerTime - shaderParams.objectSpawnTime) / params.keyFrameInterval), 0.0));
    let world = vertexPositionAtKeyframe(newestIndex, newestIndex, vertex.position);
    let light = lightSpacePosition(world);

    ShadowVertexOutput out;
    out.position = float4(light.x / shadowParams.halfExtent, light.y / shadowParams.halfExtent, light.z / shadowParams.depthRange, 1.0);
    out.depth = light.z;
    return out;
}

[shader("fragment")]
float4 fragmentMain(ShadowVertexOutput v) : SV_Target
{
    return float4(v.depth, 0.0, 0.0, 1.0);
}
