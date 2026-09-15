import "quaternion";
import "types";
import "ibl";

public struct ShaderParams
{
    public float objectSpawnTime;
    public float3 wireColor;   // wireframe.slang の線の色
}

public struct ObjectTransform
{
    public float3 position;
    public float3 scale;
    public float4 rotation;
}

public struct Vertex
{
    public float3 position;
    public float u;
    public float3 normal;
    public float v;
}

public ConstantBuffer<Params> params;
public ConstantBuffer<ShaderParams> shaderParams;
public StructuredBuffer<Vertex> vertices;
public StructuredBuffer<ObjectTransform> objectTransformHistory;
public ConstantBuffer<ShadowParams> shadowParams;
public StructuredBuffer<float> shadowLayers;

// 履歴の読み出し元。オブジェクトごとのバインディング(GlobalSource)と分割用のプール(PoolSource)の
// 2 通りがあり、物理の関数はどちらでも動くようジェネリクスにする
public interface IKeyframeSource
{
    ObjectTransform keyframe(uint index);
    float keyTime(uint index);
    uint capacity();
};

public struct GlobalSource : IKeyframeSource
{
    public ObjectTransform keyframe(uint index) { return objectTransformHistory[index % objectTransformHistory.getCount()]; }
    public float keyTime(uint index) { return shaderParams.objectSpawnTime + float(index) * params.keyFrameInterval; }
    public uint capacity() { return objectTransformHistory.getCount(); }
};

// 分割パイプラインの objects テーブルの 1 要素(pixel_coverage_subdivision.pde と同じ並び)
public struct ObjectData
{
    public uint vertexOffset;      // verticesPool 内の先頭要素
    public uint indexOffset;       // indicesPool 内の先頭要素
    public uint triangleBase;      // 全オブジェクト通しの三角形番号の先頭
    public uint triangleCount;
    public uint historyOffset;     // historyPool 内の先頭要素
    public float spawnTime;
    public uint materialSlot;
    public uint historyCapacity;
    public float roughness;
    public float metallic;
    public float colorR;           // 色は color.slang だけが使う。既定は白
    public float colorG;
    public float colorB;
};

public StructuredBuffer<ObjectData> objects;

public StructuredBuffer<ObjectTransform> historyPool;

public struct PoolSource : IKeyframeSource
{
    public ObjectData object;
    public ObjectTransform keyframe(uint index) { return historyPool[object.historyOffset + index % object.historyCapacity]; }
    public float keyTime(uint index) { return object.spawnTime + float(index) * params.keyFrameInterval; }
    public uint capacity() { return object.historyCapacity; }
};

// 変換結果のうち頂点データに依らない部分
public struct RetardedPoint
{
    public float3 apparent;
    public float3 retarded;
    public uint segmentIndex;
    public float visibility;
    public float retardedTime;
};

// 分割パイプラインが焼いた頂点。VertexOutput のうち fragment が使う分(64B)
public struct BakedVertex
{
    public float3 apparentWorldPosition;
    public float visibility;
    public float3 normal;
    public float u;
    public float3 retardedWorldPosition;
    public float v;
    public float retardedTime;
    public uint objectIndex;
    public float pad1;
    public float pad2;
};

public float3 lightSpacePosition(float3 worldPosition)
{
    let relative = worldPosition - shadowParams.center;
    return float3(
        dot(relative, shadowParams.right),
        dot(relative, shadowParams.up),
        dot(relative, shadowParams.forward) + shadowParams.depthRange * 0.5);
}

/// 遅延時刻 retardedTime に worldPosition にいた面へ、光源からの光が届いた割合（0〜1）。
/// normal は受光面の法線。周辺テクセルの比較深度を面の傾きに沿ってずらすために使う
public float shadowFactor(float3 worldPosition, float3 normal, float retardedTime)
{
    if (shadowParams.enabled == 0)
        return 1.0;

    let light = lightSpacePosition(worldPosition);
    let x = light.x / shadowParams.halfExtent;
    let y = light.y / shadowParams.halfExtent;
    if (abs(x) > 1.0 || abs(y) > 1.0 || light.z < 0.0 || light.z > shadowParams.depthRange)
        return 1.0;

    let res = int(shadowParams.resolution);
    let fx = (x * 0.5 + 0.5) * float(res);
    let fy = (0.5 - y * 0.5) * float(res);

    // この光が光源を出たフレーム
    let ring = int(shadowParams.layerCount);
    let m = int(round(retardedTime / params.keyFrameInterval - light.z / (params.lightSpeed * params.keyFrameInterval)));
    if (m > shadowParams.frame || m <= shadowParams.frame - ring)
        return 1.0;

    let slot = uint(((m % ring) + ring) % ring);
    let base = slot * uint(res * res);
    let threshold = light.z - shadowParams.bias;

    let samples = int(max(shadowParams.softSamples, 1));
    if (samples == 1 || shadowParams.softRadius <= 0.0)
    {
        let px = clamp(int(fx), 0, res - 1);
        let py = clamp(int(fy), 0, res - 1);
        return shadowLayers[base + uint(py * res + px)] < threshold ? 0.0 : 1.0;
    }

    // 受光面を光源空間の平面とみなし、タップ位置での面の深度を比較に使う（自己遮蔽のモアレ防止）。
    // 面の法線 nL に対し dz/dx = -nL.x / nL.z、dz/dy = -nL.y / nL.z。光と平行に近い面は勾配を抑える
    let n = normalize(normal);
    let nL = float3(dot(n, shadowParams.right), dot(n, shadowParams.up), dot(n, shadowParams.forward));
    let texel = 2.0 * shadowParams.halfExtent / float(res);   // 1 テクセルの幅（ワールド単位）
    let nz = (nL.z < 0.0 ? -1.0 : 1.0) * max(abs(nL.z), 0.05);
    let maxSlope = shadowParams.depthRange / texel;
    let dzdx = clamp(-nL.x / nz, -maxSlope, maxSlope) * texel;   // テクセルあたりの深度変化
    let dzdy = clamp(nL.y / nz, -maxSlope, maxSlope) * texel;    // 画像 y は光源空間 up の逆向き

    // 光源マップ上の周辺を引いて、光が届いた割合を返す。各タップは小数位置を囲む 4 テクセルを
    // 比較してから小数部で線形補間する（整数テクセルに丸めると半影がテクセル格子の階段になる）
    let step = 2.0 * shadowParams.softRadius / float(samples - 1);
    var lit = 0.0;
    for (int j = 0; j < samples; j++)
    {
        let sy = fy - shadowParams.softRadius + step * float(j) - 0.5;
        let iy = int(floor(sy));
        let ty = sy - float(iy);
        for (int i = 0; i < samples; i++)
        {
            let sx = fx - shadowParams.softRadius + step * float(i) - 0.5;
            let ix = int(floor(sx));
            let tx = sx - float(ix);
            var tap = 0.0;
            for (int cy = 0; cy < 2; cy++)
            {
                let py = clamp(iy + cy, 0, res - 1);
                let wy = cy == 0 ? 1.0 - ty : ty;
                for (int cx = 0; cx < 2; cx++)
                {
                    let px = clamp(ix + cx, 0, res - 1);
                    let wx = cx == 0 ? 1.0 - tx : tx;
                    let dx = float(px) + 0.5 - fx;
                    let dy = float(py) + 0.5 - fy;
                    if (shadowLayers[base + uint(py * res + px)] >= threshold + dzdx * dx + dzdy * dy)
                        tap += wx * wy;
                }
            }
            lit += tap;
        }
    }
    return lit / float(samples * samples);
}

public float getKeyTime(uint index)
{
    GlobalSource source;
    return source.keyTime(index);
}

public ObjectTransform getKeyframe(uint index)
{
    GlobalSource source;
    return source.keyframe(index);
}

public float3 transformPoint<S : IKeyframeSource>(S source, uint index, float3 localPoint)
{
    ObjectTransform transform = source.keyframe(index);
    return quatRotate(transform.rotation, localPoint * transform.scale) + transform.position;
}

public float3 transformPoint(uint index, float3 localPoint)
{
    GlobalSource source;
    return transformPoint(source, index, localPoint);
}

/// for normal
public float3 transformDirection<S : IKeyframeSource>(S source, uint index, float3 direction)
{
    ObjectTransform transform = source.keyframe(index);
    return normalize(quatRotate(transform.rotation, direction));
}

public float3 transformDirection(uint index, float3 direction)
{
    GlobalSource source;
    return transformDirection(source, index, direction);
}

public float3 aberrateDirection(float3 directionToSource, float3 observerVelocity)
{
    let velocitySquared = dot(observerVelocity, observerVelocity);
    if (velocitySquared < 1e-9)
        return directionToSource;

    let gamma = 1.0 / sqrt(max(1.0 - velocitySquared, 1e-8));
    let betaDirection = observerVelocity / sqrt(velocitySquared);

    let propagationDirection = -directionToSource;
    let boostedMomentum = propagationDirection + (gamma - 1.0) * dot(betaDirection, propagationDirection) * betaDirection - gamma * observerVelocity;

    return -normalize(boostedMomentum);
}

public float solveRetardedOffsetInSegment(float3 startPositionRelativeToObserver, float3 velocity, float timeToObservation, float lightSpeed)
{
    let lightSpeedSquared = lightSpeed * lightSpeed;

    let quadraticCoefficient = lightSpeedSquared - dot(velocity, velocity);
    let linearHalfCoefficient = lightSpeedSquared * timeToObservation + dot(startPositionRelativeToObserver, velocity);
    let constantCoefficient = lightSpeedSquared * timeToObservation * timeToObservation - dot(startPositionRelativeToObserver, startPositionRelativeToObserver);

    let discriminant = max(linearHalfCoefficient * linearHalfCoefficient - quadraticCoefficient * constantCoefficient, 0.0);

    return (linearHalfCoefficient - sqrt(discriminant)) / max(quadraticCoefficient, 1e-6);
}

public float3 vertexPositionAtKeyframe<S : IKeyframeSource>(S source, uint index, uint newestIndex, float3 localPosition)
{
    let indexA = min(index, max(newestIndex, 1) - 1);
    let positionA = transformPoint(source, indexA, localPosition);
    let positionB = transformPoint(source, indexA + 1, localPosition);
    let uncontracted = select(index == indexA, positionA, positionB);

    let velocity = (positionB - positionA) / max(params.keyFrameInterval, 1e-6);
    let speed = length(velocity);
    if (speed < 1e-9)
    {
        return uncontracted;
    }

    let forward = velocity / speed;
    let speedFraction = min(speed / params.lightSpeed, 0.99999);
    let lorentzContractionRatio = sqrt(max(1.0 - speedFraction * speedFraction, 1e-12));

    let objectTransform = source.keyframe(index);
    let offset = uncontracted - objectTransform.position;
    return objectTransform.position + offset + (lorentzContractionRatio - 1.0) * dot(forward, offset) * forward;
}

public float3 vertexPositionAtKeyframe(uint index, uint newestIndex, float3 localPosition)
{
    GlobalSource source;
    return vertexPositionAtKeyframe(source, index, newestIndex, localPosition);
}

public struct VertexOutput
{
    public float4 position : SV_Position;
    public float3 normal : NORMAL;
    public float visibility : TEXCOORD0;
    public float3 apparentWorldPosition : TEXCOORD1;
    public float3 retardedWorldPosition : TEXCOORD2;
    public float2 uv : TEXCOORD3;
    public float retardedTime : TEXCOORD4;
    public nointerpolation uint objectIndex : TEXCOORD5;
}

static const float PI = 3.14159265358979;

// 強さ 1 の平行光が面から視線へ返す色(cos 項込み)。Cook-Torrance(GGX)、reference/src/shade.rs の reflect と同じ式
public float3 reflectLight(float3 n, float3 l, float3 v, float3 albedo, float roughness, float metallic)
{
    let nl = max(dot(n, l), 0.0);
    if (nl <= 0.0)
        return float3(0.0);
    let nv = max(dot(n, v), 1e-4);
    let h = normalize(l + v);
    let nh = max(dot(n, h), 0.0);
    let vh = max(dot(v, h), 0.0);
    let r = max(roughness, 0.045);
    let a2 = r * r * r * r;
    let dd = nh * nh * (a2 - 1.0) + 1.0;
    let d = a2 / (PI * dd * dd);
    let k = (r + 1.0) * (r + 1.0) / 8.0;
    let g = (nl / (nl * (1.0 - k) + k)) * (nv / (nv * (1.0 - k) + k));
    let f0 = lerp(float3(0.04), albedo, metallic);
    let f = f0 + (1.0 - f0) * pow(1.0 - vh, 5.0);
    let spec = d * g * f / (4.0 * nl * nv);
    let kd = (1.0 - f) * (1.0 - metallic);
    return (kd * albedo + PI * spec) * nl;
}

// 環境マップからの光。reference/src/shade.rs の ambient_light と同じ式
public float3 ambientLight(float3 n, float3 v, float3 albedo, float roughness, float metallic, bool pbr)
{
    let irr = envIrradiance(n);
    if (!pbr)
        return irr * albedo;
    let nv = max(dot(n, v), 1e-4);
    let r = 2.0 * dot(n, v) * n - v;
    let spec = envSpecular(r, roughness);
    let ab = envBrdfApprox(roughness, nv);
    let f0 = lerp(float3(0.04), albedo, metallic);
    let fr = f0 + (max(float3(1.0 - roughness), f0) - f0) * pow(1.0 - nv, 5.0);
    let kd = (1.0 - fr) * (1.0 - metallic);
    return irr * albedo * kd + spec * (f0 * ab.x + ab.y);
}

// 視線は光が実際に進んだ向き(遅延位置 → 観測者)。光行差は見える方向だけを変え、面での反射には効かない
public float3 shade(VertexOutput v, float3 albedo)
{
    let n = normalize(v.normal);
    let s = shadowFactor(v.retardedWorldPosition, n, v.retardedTime);
    let view = normalize(params.observerPosition - v.retardedWorldPosition);
    let o = objects[v.objectIndex];
    let pbr = shadowParams.pbrEnabled != 0;
    let direct = pbr
        ? reflectLight(n, shadowParams.toLight, view, albedo, o.roughness, o.metallic)
        : albedo * max(dot(n, shadowParams.toLight), 0.0);
    if (shadowParams.iblEnabled != 0)
        return direct * (shadowParams.lightIntensity * s) + ambientLight(n, view, albedo, o.roughness, o.metallic, pbr) * shadowParams.iblIntensity;
    return albedo * shadowParams.ambient + direct * ((1.0 - shadowParams.ambient) * shadowParams.lightIntensity * s);
}

// ローカル位置 1 点の相対論変換: 二分探索で遅延セグメントを探し、遅延位置・見かけの位置・visibility を出す
public RetardedPoint retardedPoint<S : IKeyframeSource>(S source, float3 localPosition)
{
    let capacity = source.capacity();
    let newestIndex = uint(max(round((params.observerTime - source.keyTime(0)) / params.keyFrameInterval), 0.0));
    let oldestIndex = newestIndex + 1 > capacity ? newestIndex + 1 - capacity : 0;

    uint lowIndex = oldestIndex;
    uint highIndex = newestIndex;

    for (uint step = 0; step < params.searchSteps; step++)
    {
        let middleIndex = (lowIndex + highIndex) / 2;
        let vertexPosition = vertexPositionAtKeyframe(source, middleIndex, newestIndex, localPosition);
        let travelBudgetMinusDistance = (params.observerTime - source.keyTime(middleIndex)) * params.lightSpeed - distance(vertexPosition, params.observerPosition);
        if (travelBudgetMinusDistance >= 0.0)
        {
            lowIndex = middleIndex;
        }
        else
        {
            highIndex = middleIndex;
        }
    }

    let segmentIndex = lowIndex;
    let nextIndex = min(segmentIndex + 1, newestIndex);

    let positionAtSegmentStart = vertexPositionAtKeyframe(source, segmentIndex, newestIndex, localPosition);
    let positionAtSegmentEnd = vertexPositionAtKeyframe(source, nextIndex, newestIndex, localPosition);

    let rawVelocity = (positionAtSegmentEnd - positionAtSegmentStart) / max(params.keyFrameInterval, 1e-6);
    let maxVertexSpeed = params.lightSpeed * 0.99;
    let vertexVelocity = rawVelocity * min(1.0, maxVertexSpeed / max(length(rawVelocity), 1e-9));

    let retardedOffset = solveRetardedOffsetInSegment(positionAtSegmentStart - params.observerPosition, vertexVelocity, params.observerTime - source.keyTime(segmentIndex), params.lightSpeed);
    let retardedPosition = positionAtSegmentStart + vertexVelocity * retardedOffset;

    let relativePosition = retardedPosition - params.observerPosition;
    let distanceToVertex = max(length(relativePosition), 1e-4);
    var apparentDirection = relativePosition / distanceToVertex;
    apparentDirection = aberrateDirection(apparentDirection, params.observerVelocity / params.lightSpeed);

    let oldestPosition = vertexPositionAtKeyframe(source, oldestIndex, newestIndex, localPosition);

    RetardedPoint out;
    out.apparent = params.observerPosition + apparentDirection * distanceToVertex;
    out.retarded = retardedPosition;
    out.segmentIndex = segmentIndex;
    out.visibility = (params.observerTime - source.keyTime(oldestIndex)) * params.lightSpeed - distance(oldestPosition, params.observerPosition);
    out.retardedTime = source.keyTime(segmentIndex) + retardedOffset;
    return out;
}

public VertexOutput relativisticVertex(uint vertexID)
{
    let vertex = vertices[vertexID];
    GlobalSource source;
    let point = retardedPoint(source, vertex.position);

    VertexOutput out;
    out.position = mul(params.viewProjection, float4(point.apparent, 1.0));
    out.apparentWorldPosition = point.apparent;
    out.retardedWorldPosition = point.retarded;
    out.normal = transformDirection(source, point.segmentIndex, vertex.normal);
    out.uv = float2(vertex.u, vertex.v);
    out.visibility = point.visibility;
    out.retardedTime = point.retardedTime;
    out.objectIndex = 0;
    return out;
}
