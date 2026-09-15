// 空間分割（uniform grid）による大規模 boids（1,000,000 体）。
//
// O(N^2) の全対全ではなく、世界をセルに切って各 boid を登録し、
// 近傍探索は自分の周り 3x3 セルだけを見る。1 フレームは
//   clearGrid → insert → update → clearCanvas → splat
// の 5 パス。グリッドはセルごとに固定 CAP 個までの近似
// （あふれた boid は近傍として無視されるが、見た目には効かない）。
//
// 性能の要はグリッドに boid の「インデックス」ではなく「実体」を
// コピーすること。近傍走査がセル内の連続メモリ読みになり、
// ランダムアクセス（インデックス経由の間接参照）だと 8fps のところが
// 60fps になる。GPU はキャッシュライン単位でしか読めないため、
// 計算量よりメモリアクセスの局所性が支配的。
//
// 世界 = ウィンドウ = テクスチャ = 2048x2048（同一座標系、1:1 表示）。

struct Params
{
    float sep;
    float ali;
    float coh;
    float2 mouse;
    float spin; // 1: 内へ巻き込む渦（クリック時） / 0: 外へ押し出しながら逆回転
};
struct Boid
{
    float2 pos;
    float2 vel;
};

static const float WORLD = 2048.0;
static const int GRID = 256;
static const float CELL = 8.0;  // WORLD / GRID = 視界半径
static const uint CAP = 32;     // セルあたり最大格納数
static const float SEP_R = 4.0; // 分離半径
static const float MAX_SPEED = 2.5;
static const float MAX_FORCE = 0.08;

ConstantBuffer<Params> params;
StructuredBuffer<Boid> src;
RWStructuredBuffer<Boid> dst;
RWStructuredBuffer<Atomic<uint>> gridCount; // GRID*GRID
RWStructuredBuffer<Boid> gridBoids;         // GRID*GRID*CAP（実体コピーで走査を連続読みに）
[format("rgba8")]
WTexture2D<float4> canvas;

// トーラス上の最短ベクトル（境界をまたぐ相手も正しい向きに見える）
float2 wrapDelta(float2 d)
{
    return d - WORLD * round(d / WORLD);
}

float2 steer(float2 desired, float2 vel)
{
    float len = length(desired);
    if (len < 0.001)
        return float2(0.0);
    float2 s = desired / len * MAX_SPEED - vel;
    float sl = length(s);
    return (sl > MAX_FORCE) ? s / sl * MAX_FORCE : s;
}

// ---- パス 1: グリッドのカウンタを 0 に ----
[shader("compute")]
[numthreads(256, 1, 1)]
void clearGrid(uint3 id: SV_DispatchThreadID)
{
    if (id.x < uint(GRID * GRID))
        gridCount[id.x].store(0);
}

// ---- パス 2: 各 boid を自分のセルに登録 ----
[shader("compute")]
[numthreads(256, 1, 1)]
void insert(uint3 id: SV_DispatchThreadID)
{
    uint n, stride;
    src.GetDimensions(n, stride);
    if (id.x >= n)
        return;
    int2 c = clamp(int2(src[id.x].pos / CELL), int2(0), int2(GRID - 1));
    int ci = c.y * GRID + c.x;
    uint slot = gridCount[ci].add(1);
    if (slot < CAP)
        gridBoids[uint(ci) * CAP + slot] = src[id.x];
}

// ---- パス 3: 近傍 3x3 セルだけを見て速度を更新 ----
[shader("compute")]
[numthreads(256, 1, 1)]
void update(uint3 id: SV_DispatchThreadID)
{
    uint n, stride;
    src.GetDimensions(n, stride);
    if (id.x >= n)
        return;

    Boid me = src[id.x];
    int2 myCell = clamp(int2(me.pos / CELL), int2(0), int2(GRID - 1));

    float2 sepSum = float2(0.0);
    float2 aliSum = float2(0.0);
    float2 cohSum = float2(0.0);
    float count = 0.0;

    for (int dy = -1; dy <= 1; dy++)
    {
        for (int dx = -1; dx <= 1; dx++)
        {
            int2 c = (myCell + int2(dx, dy) + GRID) % GRID; // トーラス
            int ci = c.y * GRID + c.x;
            uint cnt = min(gridCount[ci].load(), CAP);
            for (uint k = 0; k < cnt; k++)
            {
                Boid other = gridBoids[uint(ci) * CAP + k];
                float2 d = wrapDelta(other.pos - me.pos);
                float dist = length(d);
                if (dist < CELL && dist > 0.001)
                {
                    if (dist < SEP_R)
                        sepSum -= d / (dist * dist);
                    aliSum += other.vel;
                    cohSum += d; // 相対位置の和（wrap 済み）
                    count += 1.0;
                }
            }
        }
    }

    float2 acc = steer(sepSum, me.vel) * params.sep;
    if (count > 0.0)
    {
        acc += steer(aliSum / count, me.vel) * params.ali;
        acc += steer(cohSum / count, me.vel) * params.coh;
    }

    // マウス: 常に角速度を与える。クリックで回転方向が反転し、吸引が加わる。
    // 影響力は距離とともに滑らかに減衰させ、効果範囲の縁が見えないようにする。
    float2 dm = wrapDelta(params.mouse - me.pos);
    float md = length(dm);
    if (md < 300.0 && md > 0.001)
    {
        float falloff = 1.0 - smoothstep(0.0, 300.0, md);
        float2 tangent = float2(-dm.y, dm.x); // マウス周りの接線
        if (params.spin > 0.5)
        {
            // クリック時: 接線 + 吸引 → 内へ巻き込む渦
            acc += (steer(tangent, me.vel) * 2.5 + steer(dm, me.vel) * 0.8) * falloff;
        }
        else
        {
            // 非クリック時: 逆向きの接線 + 外向きの圧力 → 押し出しながら逆回転
            acc += (steer(-tangent, me.vel) * 2.5 + steer(-dm, me.vel) * 0.8) * falloff;
        }
    }

    float2 vel = me.vel + acc;
    float speed = length(vel);
    if (speed > MAX_SPEED)
        vel = vel / speed * MAX_SPEED;
    float2 pos = (me.pos + vel + float2(WORLD)) % float2(WORLD);

    Boid outBoid = { pos, vel };
    dst[id.x] = outBoid;
}

// ---- パス 4: 背景 ----
[shader("compute")]
[numthreads(8, 8, 1)]
void clearCanvas(uint3 id: SV_DispatchThreadID)
{
    canvas.Store(int2(id.xy), float4(0.01, 0.01, 0.04, 1.0));
}

// ---- パス 5: 点打ち（速度の向きで色を変える） ----
[shader("compute")]
[numthreads(256, 1, 1)]
void splat(uint3 id: SV_DispatchThreadID)
{
    uint n, stride;
    src.GetDimensions(n, stride);
    if (id.x >= n)
        return;
    Boid b = src[id.x];
    float2 dir = b.vel / max(length(b.vel), 0.001);
    float3 col = float3(0.5 + 0.5 * dir.x, 0.5 + 0.5 * dir.y, 1.0) * 0.85;
    canvas.Store(int2(b.pos), float4(col, 1.0));
}
