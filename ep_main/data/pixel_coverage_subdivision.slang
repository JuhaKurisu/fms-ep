// 画面占有率に応じた三角形の GPU 再帰分割。seed → split × (maxDepth+1) → emit の 3 カーネル。
// 設計: docs/superpowers/specs/2026-09-12-pixel-coverage-subdivision-design.md
import "relativity";

struct SubdivisionParams
{
    uint totalTriangles;
    uint objectCount;
    float splitAngle;      // 辺の角度(rad)の上限。これより大きく見える辺は誤差に関係なく割る
    uint lineBase;         // bakedIndices 内のワイヤーフレーム用領域の先頭
    uint splitAngleBisect; // 1 なら辺を観測者から見た角度の二等分点で割る。0 なら局所座標の中点
    float errorAngle;      // 分割点の真の位置と直線補間との、観測者から見た角度差がこれを超えたら割る
    uint frustumCull;      // 1 なら視野の外に出た辺は割らない
    uint pad3;
    float4 frustum0;       // 視錐 4 側面の内向き単位法線(xyz)
    float4 frustum1;
    float4 frustum2;
    float4 frustum3;
    uint queueCap;
    uint finalCap;
    uint vertexCap;
    uint indexCap;         // マテリアル 1 つあたり
    uint lineCap;
    uint fsFinalCount;     // frameState 内の各領域の先頭(u32 単位)。深さ d のレコード数は frameState[d]
    uint fsVertexCount;
    uint fsStats;
    uint fsDraw;
    uint fsLineDraw;
};

struct PassParams
{
    uint inDepth;
    uint outDepth;
    uint last;             // 1 なら誰も割らない(深さ上限)
    uint pad;
};

// 分割途中の三角形の隅 1 つ(64B)
struct Corner
{
    float3 local;          // ローカル座標の位置。中点はこれの平均で作る
    uint segmentIndex;
    float3 apparent;       // 見かけの位置(光行差込み)
    float baryU;           // 元三角形内の重心座標。法線と UV の補間に使う
    float3 retarded;       // 遅延位置
    float baryV;
    float visibility;
    float retardedTime;
    uint pad0;
    uint pad1;
};

// 分割途中(または確定)の三角形 1 枚(208B)
struct TriangleData
{
    uint objectIndex;
    uint baseTriangle;
    uint depth;
    uint pad;
    Corner c0;
    Corner c1;
    Corner c2;
};

ConstantBuffer<SubdivisionParams> subdivisionParams;
ConstantBuffer<PassParams> passParams;
StructuredBuffer<Vertex> verticesPool;
StructuredBuffer<uint> indicesPool;
StructuredBuffer<TriangleData> queueIn;
RWStructuredBuffer<TriangleData> queueOut;
RWStructuredBuffer<TriangleData> finalTriangles;
RWStructuredBuffer<BakedVertex> bakedVertices;
RWStructuredBuffer<uint> bakedIndices;   // マテリアル別の領域 × MATERIAL_CAP の後にワイヤーフレーム用の線の領域
RWStructuredBuffer<Atomic<uint>> frameState;
// 間接引数は読み書きストレージと同じ dispatch 内で共存できないので frameState と分ける。
// dispatchOut は次の深さの split の {x,y,z}(深さの偶奇で 2 本を交互に使う)、emitArgs は emit の {x,y,z}
RWStructuredBuffer<Atomic<uint>> dispatchOut;
RWStructuredBuffer<Atomic<uint>> emitArgs;

static const uint WORKGROUP = 64;
static const uint STAT_QUEUE = 0;
static const uint STAT_FINAL = 1;
static const uint STAT_VERTEX = 2;
static const uint STAT_INDEX = 3;
static const uint NONE = 0xFFFFFFFF;

// count 個ぶん確保して先頭を返す。溢れたら戻して統計に記録し NONE。
// 戻すのでカウンタの最終値は成功分の合計に収束し、読む側は clamp が要らない
uint reserve(uint fsIndex, uint count, uint cap, uint stat)
{
    uint slot = frameState[fsIndex].add(count);
    if (slot + count > cap)
    {
        frameState[fsIndex].sub(count);
        frameState[subdivisionParams.fsStats + stat].add(1);
        return NONE;
    }
    return slot;
}

uint workgroups(uint threads)
{
    return (threads + WORKGROUP - 1) / WORKGROUP;
}

float2 bary(Corner c)
{
    return float2(c.baryU, c.baryV);
}

Vertex baseVertex(ObjectData o, uint tri, uint k)
{
    return verticesPool[o.vertexOffset + indicesPool[o.indexOffset + tri * 3 + k]];
}

Corner makeCorner(ObjectData o, float3 local, float2 bary)
{
    PoolSource source;
    source.object = o;
    let point = retardedPoint(source, local);
    Corner c;
    c.local = local;
    c.segmentIndex = point.segmentIndex;
    c.apparent = point.apparent;
    c.baryU = bary.x;
    c.retarded = point.retarded;
    c.baryV = bary.y;
    c.visibility = point.visibility;
    c.retardedTime = point.retardedTime;
    c.pad0 = 0;
    c.pad1 = 0;
    return c;
}

// 局所座標の辞書順。辺の分割点を両側の三角形で同じ計算順にするための順序
bool before(Corner a, Corner b)
{
    if (a.local.x != b.local.x) return a.local.x < b.local.x;
    if (a.local.y != b.local.y) return a.local.y < b.local.y;
    return a.local.z < b.local.z;
}

struct EdgeSplit
{
    Corner point;          // 辺の分割点(変換済み)
    float deviation;       // 分割点の真の位置と、両端の直線補間との、観測者から見た角度差
};

// 辺の分割点を作り、直線補間からのずれを測る。
// 分割点は局所座標の中点(t = 0.5)だと画面上で片寄るので、既定では観測者から見た角度を二等分する点にする。
// 角の二等分線は辺を両端までの距離の比 da:db で分ける(角の二等分線の定理)ので t = da / (da + db)。
// 2 隅を辞書順に並べてから計算するので、隣の三角形が逆順で呼んでもビット単位で同じ値になる
EdgeSplit evalEdge(ObjectData o, Corner a, Corner b)
{
    Corner p = before(a, b) ? a : b;
    Corner q = before(a, b) ? b : a;
    var t = 0.5;
    if (subdivisionParams.splitAngleBisect != 0)
    {
        let dp = length(p.apparent - params.observerPosition);
        let dq = length(q.apparent - params.observerPosition);
        t = dp / max(dp + dq, 1e-9);
    }
    EdgeSplit e;
    e.point = makeCorner(o, p.local + (q.local - p.local) * t, bary(p) + (bary(q) - bary(p)) * t);
    let chord = p.apparent + (q.apparent - p.apparent) * t;
    let dm = normalize(e.point.apparent - params.observerPosition);
    let dc = normalize(chord - params.observerPosition);
    e.deviation = acos(clamp(dot(dm, dc), -1.0, 1.0));
    return e;
}

BakedVertex bake(ObjectData o, uint objectIndex, uint tri, Corner c)
{
    let v0 = baseVertex(o, tri, 0);
    let v1 = baseVertex(o, tri, 1);
    let v2 = baseVertex(o, tri, 2);
    let w = float3(c.baryU, c.baryV, 1.0 - c.baryU - c.baryV);
    let localNormal = normalize(v0.normal * w.x + v1.normal * w.y + v2.normal * w.z);
    PoolSource source;
    source.object = o;

    BakedVertex b;
    b.apparentWorldPosition = c.apparent;
    b.visibility = c.visibility;
    b.normal = transformDirection(source, c.segmentIndex, localNormal);
    b.u = v0.u * w.x + v1.u * w.y + v2.u * w.z;
    b.retardedWorldPosition = c.retarded;
    b.v = v0.v * w.x + v1.v * w.y + v2.v * w.z;
    b.retardedTime = c.retardedTime;
    b.objectIndex = objectIndex;
    b.pad1 = 0;
    b.pad2 = 0;
    return b;
}

void pushFinal(TriangleData r)
{
    uint f = reserve(subdivisionParams.fsFinalCount, 1, subdivisionParams.finalCap, STAT_FINAL);
    if (f == NONE)
        return;
    finalTriangles[f] = r;
    emitArgs[0].max(workgroups(f + 1));
}

// 深さ depth のキューの slot にレコードを置き、そのパスのワークグループ数を伸ばす
void writeQueue(uint depth, uint slot, TriangleData r)
{
    queueOut[slot] = r;
    dispatchOut[depth * 3].max(workgroups(slot + 1));
}

float3 apparentDirection(Corner c)
{
    return normalize(c.apparent - params.observerPosition);
}

// 観測者から見た辺の角度。apparent は光行差適用済みなので方向はそのまま画面上の方向
float edgeAngle(Corner a, Corner b)
{
    return acos(clamp(dot(apparentDirection(a), apparentDirection(b)), -1.0, 1.0));
}

// 視錐 4 側面のどれか 1 枚について、両端が「辺自身の角度より外側」なら視野外。余白は辺の曲がりぶん
bool edgeOutsideView(Corner a, Corner b, float angle)
{
    if (subdivisionParams.frustumCull == 0)
        return false;
    let da = apparentDirection(a);
    let db = apparentDirection(b);
    let s = sin(angle);
    float3 planes[4] = { subdivisionParams.frustum0.xyz, subdivisionParams.frustum1.xyz, subdivisionParams.frustum2.xyz, subdivisionParams.frustum3.xyz };
    for (int p = 0; p < 4; p++)
    {
        if (dot(da, planes[p]) < -s && dot(db, planes[p]) < -s)
            return true;
    }
    return false;
}

// 辺の両端と分割点だけで決める。三角形の情報を混ぜると隣の三角形と判定がずれてクラックになる。
// 主な基準は直線補間からのずれ(平らに見える辺は割らない)。辺の角度の上限は補間の粗さを抑える保険
bool shouldSplitEdge(Corner a, Corner b, float deviation)
{
    let angle = edgeAngle(a, b);
    if (edgeOutsideView(a, b, angle))
        return false;
    return deviation > subdivisionParams.errorAngle || angle > subdivisionParams.splitAngle;
}

TriangleData child(TriangleData r, Corner a, Corner b, Corner c)
{
    TriangleData t = r;
    t.depth = r.depth + 1;
    t.c0 = a;
    t.c1 = b;
    t.c2 = c;
    return t;
}

// (c0, c1, c2) → (c1, c2, c0)。巻き方向は変わらない
TriangleData rotated(TriangleData r)
{
    TriangleData t = r;
    t.c0 = r.c1;
    t.c1 = r.c2;
    t.c2 = r.c0;
    return t;
}

// 割れた辺の数で 2 / 3 / 4 枚の子を slot から書く。1 本なら c0-c1 が、2 本なら割らない辺が c2-c0 に来るよう
// 三角形を回す。分割点 m01 / m12 / m20 も一緒に回す(rotated 1 回で m01 ← m12、m12 ← m20、m20 ← m01)
void writeChildren(TriangleData r, bool s01, bool s12, bool s20, Corner m01, Corner m12, Corner m20, uint count, uint slot)
{
    let depth = r.depth + 1;
    uint turns = 0;
    if (count == 2)
        turns = s12 ? 1 : (s20 ? 2 : 0);
    else if (count == 3)
        turns = !s01 ? 1 : (!s12 ? 2 : 0);
    for (uint k = 0; k < turns; k++)
    {
        r = rotated(r);
        let m = m01;
        m01 = m12;
        m12 = m20;
        m20 = m;
    }
    if (count == 2)
    {
        writeQueue(depth, slot + 0, child(r, r.c0, m01, r.c2));
        writeQueue(depth, slot + 1, child(r, m01, r.c1, r.c2));
    }
    else if (count == 3)
    {
        writeQueue(depth, slot + 0, child(r, m01, r.c1, m12));
        // 残る四角形 c0-m01-m12-c2 は、観測者から見て短い方の対角線で割る(斜めの辺を割りすぎない)。
        // 三角形の内側の選択なので隣には影響しない
        if (edgeAngle(r.c0, m12) <= edgeAngle(m01, r.c2))
        {
            writeQueue(depth, slot + 1, child(r, r.c0, m01, m12));
            writeQueue(depth, slot + 2, child(r, r.c0, m12, r.c2));
        }
        else
        {
            writeQueue(depth, slot + 1, child(r, r.c0, m01, r.c2));
            writeQueue(depth, slot + 2, child(r, m01, m12, r.c2));
        }
    }
    else
    {
        writeQueue(depth, slot + 0, child(r, r.c0, m01, m20));
        writeQueue(depth, slot + 1, child(r, m01, r.c1, m12));
        writeQueue(depth, slot + 2, child(r, m20, m12, r.c2));
        writeQueue(depth, slot + 3, child(r, m01, m12, m20));
    }
}

// 1 スレッド = 元三角形 1 枚。3 隅を変換して深さ 0 のキューへ
[shader("compute")]
[numthreads(64, 1, 1)]
void seed(uint3 id: SV_DispatchThreadID)
{
    let t = id.x;
    if (t >= subdivisionParams.totalTriangles)
        return;
    uint objectIndex = 0;
    while (objectIndex + 1 < subdivisionParams.objectCount && objects[objectIndex + 1].triangleBase <= t)
        objectIndex++;
    let o = objects[objectIndex];
    let tri = t - o.triangleBase;

    TriangleData r;
    r.objectIndex = objectIndex;
    r.baseTriangle = tri;
    r.depth = 0;
    r.pad = 0;
    r.c0 = makeCorner(o, baseVertex(o, tri, 0).position, float2(1, 0));
    r.c1 = makeCorner(o, baseVertex(o, tri, 1).position, float2(0, 1));
    r.c2 = makeCorner(o, baseVertex(o, tri, 2).position, float2(0, 0));
    uint slot = reserve(0, 1, subdivisionParams.queueCap, STAT_QUEUE);   // 深さ 0 のレコード数は frameState[0]
    if (slot == NONE)
        return;
    writeQueue(0, slot, r);
}

// 1 スレッド = 深さ inDepth のレコード 1 つ。辺を割って子を outDepth へ、割らなければ確定リストへ
[shader("compute")]
[numthreads(64, 1, 1)]
void split(uint3 id: SV_DispatchThreadID)
{
    let n = frameState[passParams.inDepth].load();
    if (id.x >= n)
        return;
    var r = queueIn[id.x];
    let o = objects[r.objectIndex];

    if (passParams.last == 0)
    {
        let e01 = evalEdge(o, r.c0, r.c1);
        let e12 = evalEdge(o, r.c1, r.c2);
        let e20 = evalEdge(o, r.c2, r.c0);
        let s01 = shouldSplitEdge(r.c0, r.c1, e01.deviation);
        let s12 = shouldSplitEdge(r.c1, r.c2, e12.deviation);
        let s20 = shouldSplitEdge(r.c2, r.c0, e20.deviation);
        let count = (s01 ? 1u : 0u) + (s12 ? 1u : 0u) + (s20 ? 1u : 0u) + 1u;
        if (count > 1)
        {
            uint slot = reserve(passParams.outDepth, count, subdivisionParams.queueCap, STAT_QUEUE);
            if (slot != NONE)
            {
                writeChildren(r, s01, s12, s20, e01.point, e12.point, e20.point, count, slot);
                return;
            }
            // キュー満杯: 割らずに確定へ(隣と食い違うので溢れ時に限りクラックが出る)
        }
    }
    pushFinal(r);
}

// 1 スレッド = 確定三角形 1 枚。頂点を焼き、マテリアル別のインデックス領域とワイヤーフレーム用の線の領域に書く
[shader("compute")]
[numthreads(64, 1, 1)]
void emit(uint3 id: SV_DispatchThreadID)
{
    let n = frameState[subdivisionParams.fsFinalCount].load();
    if (id.x >= n)
        return;
    let r = finalTriangles[id.x];
    let o = objects[r.objectIndex];
    if (r.c0.visibility <= 0.0 && r.c1.visibility <= 0.0 && r.c2.visibility <= 0.0)
        return;

    uint v = reserve(subdivisionParams.fsVertexCount, 3, subdivisionParams.vertexCap, STAT_VERTEX);
    if (v == NONE)
        return;
    let m = o.materialSlot;
    uint i = reserve(subdivisionParams.fsDraw + m * 5, 3, subdivisionParams.indexCap, STAT_INDEX);
    if (i == NONE)
        return;
    bakedVertices[v + 0] = bake(o, r.objectIndex, r.baseTriangle, r.c0);
    bakedVertices[v + 1] = bake(o, r.objectIndex, r.baseTriangle, r.c1);
    bakedVertices[v + 2] = bake(o, r.objectIndex, r.baseTriangle, r.c2);
    let indexBase = m * subdivisionParams.indexCap + i;
    bakedIndices[indexBase + 0] = v + 0;
    bakedIndices[indexBase + 1] = v + 1;
    bakedIndices[indexBase + 2] = v + 2;

    uint l = reserve(subdivisionParams.fsLineDraw, 6, subdivisionParams.lineCap, STAT_INDEX);
    if (l == NONE)
        return;
    let lineBase = subdivisionParams.lineBase + l;
    bakedIndices[lineBase + 0] = v + 0;
    bakedIndices[lineBase + 1] = v + 1;
    bakedIndices[lineBase + 2] = v + 1;
    bakedIndices[lineBase + 3] = v + 2;
    bakedIndices[lineBase + 4] = v + 2;
    bakedIndices[lineBase + 5] = v + 0;
}
