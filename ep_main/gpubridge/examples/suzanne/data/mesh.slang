// OBJ メッシュを vertex pulling で描く。
// gpubridge の uniform は mat4 を渡せない（f32/i32/u32/vecNf のみ）ので、
// 回転角・カメラ距離・fov から VS 内で変換を組み立てる。

struct Params
{
    float angleX; // 縦回転（ラジアン）
    float angleY; // 横回転（ラジアン）
    float dist;   // カメラ距離（原点からのオフセット）
    float fov;    // 縦の視野角（ラジアン）
    float aspect; // 幅 / 高さ
};

// pde 側の loadObjTriangles() が作る形式。float3 は 16 バイト境界なので
// 1 頂点 = 8 float（pos.xyz + 詰め物, normal.xyz + 詰め物）。
struct Vertex
{
    float3 position;
    float3 normal;
};

ConstantBuffer<Params> params;
StructuredBuffer<Vertex> vertices;

static const float NEAR = 0.1;
static const float FAR = 100.0;

// Y 軸 → X 軸の順で回す
float3 rotate(float3 p)
{
    float cy = cos(params.angleY), sy = sin(params.angleY);
    float cx = cos(params.angleX), sx = sin(params.angleX);
    p = float3(cy * p.x + sy * p.z, p.y, -sy * p.x + cy * p.z);
    return float3(p.x, cx * p.y - sx * p.z, sx * p.y + cx * p.z);
}

struct VSOut
{
    float4 pos : SV_Position;
    float3 normal : NORMAL;
};

[shader("vertex")]
VSOut vsMain(uint vid: SV_VertexID)
{
    Vertex v = vertices[vid];
    float3 p = rotate(v.position);

    // カメラは (0, 0, dist) から原点を見る。zview は視点からの奥行き
    float zview = params.dist - p.z;
    float f = 1.0 / tan(params.fov * 0.5);

    VSOut o;
    o.pos = float4(
        p.x * f / params.aspect,
        p.y * f,
        (zview - NEAR) * FAR / (FAR - NEAR), // 透視深度（0..1 × w）
        zview);
    o.normal = rotate(v.normal);
    return o;
}

[shader("fragment")]
float4 fsMain(VSOut v) : SV_Target
{
    float3 n = normalize(v.normal);
    float3 light = normalize(float3(0.4, 0.8, 0.6));
    float d = max(dot(n, light), 0.0);
    float3 base = float3(0.85, 0.55, 0.35);
    float3 c = base * (0.15 + 0.85 * d);
    return float4(c, 1.0);
}
