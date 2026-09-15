// boids シミュレーション（Craig Reynolds 流のステアリング。O(N^2) の素朴実装）。
// 各力を max_force でクランプして同じオーダーに揃えているので、
// sep/ali/coh は 0〜2 くらいの混合比として全域で効く。
struct Params {
    float dt;
    float sep;
    float ali;
    float coh;
};
struct Boid {
    float2 pos;
    float2 vel;
};

ConstantBuffer<Params> params;
StructuredBuffer<Boid> src;
RWStructuredBuffer<Boid> dst;

static const float MAX_SPEED = 3.0;
static const float MAX_FORCE = 0.06;
static const float VIEW_R = 50.0;   // 整列・結合の近傍半径
static const float SEP_R = 25.0;    // 分離の半径（近いものだけ避ける）

// desired 方向へ全速で向かうための操舵力（上限つき）
float2 steer(float2 desired, float2 vel) {
    float len = length(desired);
    if (len < 0.001) return float2(0.0);
    float2 s = desired / len * MAX_SPEED - vel;
    float sl = length(s);
    return (sl > MAX_FORCE) ? s / sl * MAX_FORCE : s;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void update(uint3 id: SV_DispatchThreadID) {
    uint n, stride;
    src.GetDimensions(n, stride);
    if (id.x >= n) return;

    Boid me = src[id.x];
    float2 sepSum = float2(0.0);
    float2 aliSum = float2(0.0);
    float2 cohSum = float2(0.0);
    float count = 0.0;
    for (uint i = 0; i < n; i++) {
        if (i == id.x) continue;
        float2 d = src[i].pos - me.pos;
        float dist = length(d);
        if (dist < VIEW_R && dist > 0.001) {
            if (dist < SEP_R) sepSum -= d / (dist * dist);
            aliSum += src[i].vel;
            cohSum += src[i].pos;
            count += 1.0;
        }
    }

    float2 acc = steer(sepSum, me.vel) * params.sep;
    if (count > 0.0) {
        acc += steer(aliSum / count, me.vel) * params.ali;
        acc += steer(cohSum / count - me.pos, me.vel) * params.coh;
    }

    float2 vel = me.vel + acc * params.dt * 60.0;
    float speed = length(vel);
    if (speed > MAX_SPEED) vel = vel / speed * MAX_SPEED;
    float2 pos = (me.pos + vel + float2(1600.0)) % float2(1600.0);

    Boid outBoid = { pos, vel };
    dst[id.x] = outBoid;
}
