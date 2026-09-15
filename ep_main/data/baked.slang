// 分割パイプラインが焼いた頂点を読む頂点シェーダー。マテリアルの vertexMain から呼ぶ
import "relativity";

public StructuredBuffer<BakedVertex> bakedVertices;

public VertexOutput bakedVertex(uint vertexID)
{
    let b = bakedVertices[vertexID];
    VertexOutput out;
    out.position = mul(params.viewProjection, float4(b.apparentWorldPosition, 1.0));
    out.apparentWorldPosition = b.apparentWorldPosition;
    out.retardedWorldPosition = b.retardedWorldPosition;
    out.normal = b.normal;
    out.uv = float2(b.u, b.v);
    out.visibility = b.visibility;
    out.retardedTime = b.retardedTime;
    out.objectIndex = b.objectIndex;
    return out;
}
