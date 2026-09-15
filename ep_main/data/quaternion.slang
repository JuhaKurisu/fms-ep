// クォータニオン演算。float4 の xyz を虚部、w を実部として扱う。
// Processing 側からは vec4 として渡す: set("rotation", x, y, z, w)

// 無回転
public float4 quatIdentity()
{
    return float4(0.0, 0.0, 0.0, 1.0);
}

// 軸まわりの回転から作る。axis は正規化済みであること。angle はラジアン
public float4 quatFromAxisAngle(float3 axis, float angle)
{
    float half = angle * 0.5;
    return float4(axis * sin(half), cos(half));
}

// オイラー角（ラジアン）から作る。Z * Y * X の順に適用（roll → yaw → pitch）
public float4 quatFromEuler(float3 euler)
{
    float3 h = euler * 0.5;
    float3 s = sin(h);
    float3 c = cos(h);
    return float4(
        s.x * c.y * c.z - c.x * s.y * s.z,
        c.x * s.y * c.z + s.x * c.y * s.z,
        c.x * c.y * s.z - s.x * s.y * c.z,
        c.x * c.y * c.z + s.x * s.y * s.z);
}

// 合成。quatMul(a, b) は「b の回転をしてから a の回転」（行列積と同じ順序）
public float4 quatMul(float4 a, float4 b)
{
    return float4(
        a.w * b.xyz + b.w * a.xyz + cross(a.xyz, b.xyz),
        a.w * b.w - dot(a.xyz, b.xyz));
}

// 共役。単位クォータニオンなら逆回転
public float4 quatConjugate(float4 q)
{
    return float4(-q.xyz, q.w);
}

// 逆元。非正規化でも正しい逆回転になる
public float4 quatInverse(float4 q)
{
    return quatConjugate(q) / dot(q, q);
}

// 正規化。累積誤差が溜まったときに掛け直す用
public float4 quatNormalize(float4 q)
{
    return q / length(q);
}

// ベクトルに回転を適用する。q は正規化済みであること
public float3 quatRotate(float4 q, float3 v)
{
    float3 t = 2.0 * cross(q.xyz, v);
    return v + q.w * t + cross(q.xyz, t);
}

// 球面線形補間。t は [0, 1]。a, b は正規化済みであること
public float4 quatSlerp(float4 a, float4 b, float t)
{
    float d = dot(a, b);
    // 最短経路を通るため、反対側にあれば符号を反転
    if (d < 0.0)
    {
        b = -b;
        d = -d;
    }
    // ほぼ同一方向では sin が 0 に近づき不安定なので nlerp に切り替え
    if (d > 0.9995)
    {
        return quatNormalize(lerp(a, b, t));
    }
    float theta = acos(d);
    float invSin = 1.0 / sin(theta);
    return (sin((1.0 - t) * theta) * a + sin(t * theta) * b) * invSin;
}
