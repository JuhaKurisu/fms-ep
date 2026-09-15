// 前フレームと今フレームの瞬間 depth から、この 1 フレームの間に光が当たった面を
// 「その光が光源を出たフレーム m」の層に書き込む。層 m の光は深度 z に時刻 m*dt + z/c に届く。
// 各テクセルを 1 スレッドが担当し、全層を同じスレッドが触るので競合しない
import "types";

ConstantBuffer<Params> params;
ConstantBuffer<ShadowParams> shadowParams;
Texture2D<float> depthPrev;
Texture2D<float> depthCur;
RWStructuredBuffer<float> shadowLayers;

static const float FAR = 1e9;

uint slotOf(int m, int ring)
{
    return uint(((m % ring) + ring) % ring);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void accumulate(uint3 id: SV_DispatchThreadID)
{
    let res = shadowParams.resolution;
    if (id.x >= res || id.y >= res)
        return;

    let texels = res * res;
    let texel = id.y * res + id.x;
    let frame = shadowParams.frame;
    let ring = int(shadowParams.layerCount);
    let dt = params.keyFrameInterval;
    let c = params.lightSpeed;
    let warmup = shadowParams.warmup != 0;

    if (warmup)
    {
        for (int s = 0; s < ring; s++)
            shadowLayers[uint(s) * texels + texel] = FAR;
    }
    else
    {
        // 今フレーム光源を出た光の層を空にして使い回す
        shadowLayers[slotOf(frame, ring) * texels + texel] = FAR;
    }

    var dCur = depthCur[int2(id.xy)];
    if (dCur >= FAR)
        return;
    var dPrev = depthPrev[int2(id.xy)];
    // 横から入ってきた面は前フレームも同じ深度にいたとみなす。初期化時は全層を静止扱いで埋める。
    // 面の深度が 1 フレームで変われる量は光速でも c*dt までなので、それを超える差は別の面が
    // 入れ替わったということ。同じ面の移動として結ぶと間の層が全部遮蔽で埋まる
    if (dPrev >= FAR || warmup || abs(dPrev - dCur) > c * dt)
        dPrev = dCur;

    // 光が前フレームで面の手前、今フレームで面の奥にある層 m の範囲
    var mLow = int(ceil(float(frame - 1) - dPrev / (c * dt)));
    var mHigh = int(floor(float(frame) - dCur / (c * dt)));
    if (warmup)
        mLow = frame - ring + 1;
    mLow = max(mLow, frame - ring + 1);
    mHigh = min(mHigh, frame);

    let lower = min(dPrev, dCur);
    let upper = max(dPrev, dCur);
    for (int m = mLow; m <= mHigh; m++)
    {
        let at = slotOf(m, ring) * texels + texel;
        if (shadowLayers[at] < FAR)
            continue;   // 先に別の面に当たっている
        let wavePrev = c * (float(frame - 1) - float(m)) * dt;
        let a = dPrev - wavePrev;
        let b = dCur - (wavePrev + c * dt);
        let f = (a - b) > 1e-6 ? saturate(a / (a - b)) : 0.0;
        shadowLayers[at] = clamp(wavePrev + f * c * dt, lower, upper);
    }
}
