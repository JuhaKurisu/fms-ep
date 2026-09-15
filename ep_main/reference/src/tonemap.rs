// 本体の tonemap.slang と bake_luts が共有する式

pub fn srgb_encode(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.0031308 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 }
}

pub const SHAPER_MIN_EV: f64 = -10.0;
pub const SHAPER_RANGE_EV: f64 = 16.5;

pub fn shaper(x: f64) -> f64 {
    ((x.max(2f64.powf(SHAPER_MIN_EV)).log2() - SHAPER_MIN_EV) / SHAPER_RANGE_EV).clamp(0.0, 1.0)
}

pub fn shaper_inverse(t: f64) -> f64 {
    2f64.powf(t * SHAPER_RANGE_EV + SHAPER_MIN_EV)
}

fn map3(c: [f64; 3], f: impl Fn(f64) -> f64) -> [f64; 3] {
    [f(c[0]), f(c[1]), f(c[2])]
}

pub fn aces(c: [f64; 3]) -> [f64; 3] {
    map3(c, |x| ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0))
}

pub fn reinhard(c: [f64; 3]) -> [f64; 3] {
    let w = 4.0;
    map3(c, |x| (x * (1.0 + x / (w * w)) / (1.0 + x)).clamp(0.0, 1.0))
}

pub fn hable(c: [f64; 3]) -> [f64; 3] {
    fn f(x: f64) -> f64 {
        let (a, b, cc, d, e, ff) = (0.15, 0.50, 0.10, 0.20, 0.02, 0.30);
        ((x * (a * x + cc * b) + d * e) / (x * (a * x + b) + d * ff)) - e / ff
    }
    let white = f(11.2);
    map3(c, |x| (f(x * 2.0) / white).clamp(0.0, 1.0))
}

// 列優先の 3x3（GLSL の mat3 と同じ並び）を掛ける
fn mul_col_major(m: [f64; 9], v: [f64; 3]) -> [f64; 3] {
    [
        m[0] * v[0] + m[3] * v[1] + m[6] * v[2],
        m[1] * v[0] + m[4] * v[1] + m[7] * v[2],
        m[2] * v[0] + m[5] * v[1] + m[8] * v[2],
    ]
}

// AgX(Blender 4 の既定)。出力は表示値なので sRGB 変換を重ねない
pub fn agx(c: [f64; 3]) -> [f64; 3] {
    const INSET: [f64; 9] = [
        0.842479062253094, 0.0423282422610123, 0.0423756549057051,
        0.0784335999999992, 0.878468636469772, 0.0784336,
        0.0792237451477643, 0.0791661274605434, 0.879142973793104,
    ];
    const OUTSET: [f64; 9] = [
        1.19687900512017, -0.0528968517574562, -0.0529716355144438,
        -0.0980208811401368, 1.15190312990417, -0.0980434501171241,
        -0.0990297440797205, -0.0989611768448433, 1.15107367264116,
    ];
    let (min_ev, max_ev) = (-12.47393, 4.026069);
    let v = mul_col_major(INSET, map3(c, |x| x.max(1e-10)));
    let v = map3(v, |x| (x.log2().clamp(min_ev, max_ev) - min_ev) / (max_ev - min_ev));
    let v = map3(v, |x| {
        let x2 = x * x;
        let x4 = x2 * x2;
        15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232
    });
    map3(mul_col_major(OUTSET, v), |x| x.clamp(0.0, 1.0))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operator {
    Aces,
    Reinhard,
    Hable,
    Agx,
}

pub fn display(op: Operator, linear: [f64; 3]) -> [f64; 3] {
    match op {
        Operator::Aces => map3(aces(linear), srgb_encode),
        Operator::Reinhard => map3(reinhard(linear), srgb_encode),
        Operator::Hable => map3(hable(linear), srgb_encode),
        Operator::Agx => agx(linear),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shaper_roundtrip() {
        for x in [0.001, 0.18, 1.0, 4.0, 60.0] {
            assert!((shaper_inverse(shaper(x)) - x).abs() < 1e-9 * x.max(1.0));
        }
        assert_eq!(shaper(0.0), 0.0);
        assert_eq!(shaper(1e9), 1.0);
    }

    #[test]
    fn srgb_endpoints() {
        assert_eq!(srgb_encode(0.0), 0.0);
        assert!((srgb_encode(1.0) - 1.0).abs() < 1e-12);
        assert!((srgb_encode(0.5) - 0.7353569830524495).abs() < 1e-9);
    }

    #[test]
    fn operators_are_monotonic_and_bounded() {
        for op in [Operator::Aces, Operator::Reinhard, Operator::Hable, Operator::Agx] {
            let mut prev = -1.0;
            for i in 0..200 {
                let x = shaper_inverse(i as f64 / 199.0);
                let y = display(op, [x; 3])[1];
                assert!(y >= prev - 1e-9, "{:?} not monotonic at {}", op, x);
                assert!((0.0..=1.0).contains(&y), "{:?} out of range at {}: {}", op, x, y);
                prev = y;
            }
        }
    }

    #[test]
    fn aces_known_values() {
        assert!((aces([0.0; 3])[0]).abs() < 1e-12);
        let mid = aces([0.18; 3])[0];
        assert!((mid - 0.18 * (2.51 * 0.18 + 0.03) / (0.18 * (2.43 * 0.18 + 0.59) + 0.14)).abs() < 1e-12);
    }
}
