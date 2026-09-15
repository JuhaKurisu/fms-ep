// boids を 1 匹 = 1 四角形（2 三角形 = 6 頂点）で加算描画する。
// 頂点バッファは使わない: SV_InstanceID で boid を、SV_VertexID で角を選ぶ。

struct Boid { float2 pos; float2 vel; };
StructuredBuffer<Boid> boids;

static const float2 corners[6] = {
    float2(-1.0, -1.0), float2(1.0, -1.0), float2(-1.0, 1.0),
    float2(-1.0,  1.0), float2(1.0, -1.0), float2( 1.0, 1.0),
};

struct VSOut {
    float4 pos : SV_Position;
    float2 local : TEXCOORD0;
};

[shader("vertex")]
VSOut vsMain(uint vid : SV_VertexID, uint iid : SV_InstanceID) {
    Boid b = boids[iid];
    float size = 4.0; // 半径 px
    float2 p = b.pos + corners[vid] * size;
    // 1600px のキャンバス → NDC（y は下向き pixel 座標なので反転）
    float2 ndc = p / 800.0 - 1.0;
    VSOut o;
    o.pos = float4(ndc.x, -ndc.y, 0.0, 1.0);
    o.local = corners[vid];
    return o;
}

[shader("fragment")]
float4 fsMain(VSOut v) : SV_Target {
    // 中心ほど明るい丸いスプラット。ADD ブレンドで重なりが光る。
    float d = saturate(1.0 - length(v.local));
    return float4(0.3, 0.7, 1.0, 1.0) * (d * d);
}
