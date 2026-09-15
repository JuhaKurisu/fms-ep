// 正距円筒の環境マップ。読み出し・縮小・GGX ぼかし・照度は本体の ibl.pde / ibl.slang と同じ手順
use crate::vec::Vec3;
use std::f64::consts::PI;
use std::path::Path;

pub const ROUGHNESS_LEVELS: [f64; 5] = [0.2, 0.4, 0.6, 0.8, 1.0];
pub const SMALL: (usize, usize) = (128, 64);
pub const PREFILTERED: (usize, usize) = (64, 32);
pub const IRRADIANCE: (usize, usize) = (32, 16);

// d.z がちょうど 0 のときの atan2 は GPU で結果が不定(NaN や符号違い)なので手で分ける
pub fn dir_to_uv(d: Vec3) -> (f64, f64) {
    let u = if d.z == 0.0 {
        if d.x > 0.0 { 0.75 } else if d.x < 0.0 { 0.25 } else { 0.5 }
    } else {
        d.x.atan2(-d.z) / (2.0 * PI) + 0.5
    };
    (u, d.y.clamp(-1.0, 1.0).acos() / PI)
}

pub fn uv_to_dir(u: f64, v: f64) -> Vec3 {
    let phi = (u - 0.5) * 2.0 * PI;
    let theta = v * PI;
    Vec3::new(theta.sin() * phi.sin(), theta.cos(), -theta.sin() * phi.cos())
}

pub struct Equirect {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f64>,
}

impl Equirect {
    pub fn uniform(width: usize, height: usize, c: [f64; 3]) -> Equirect {
        let mut data = Vec::with_capacity(width * height * 3);
        for _ in 0..width * height {
            data.extend_from_slice(&c);
        }
        Equirect { width, height, data }
    }

    pub fn load_hdr(path: &Path) -> Result<Equirect, String> {
        let img = image::open(path).map_err(|e| format!("{}: {}", path.display(), e))?.into_rgb32f();
        let (width, height) = (img.width() as usize, img.height() as usize);
        let data = img.as_raw().iter().map(|&v| v as f64).collect();
        Ok(Equirect { width, height, data })
    }

    pub fn texel(&self, x: i64, y: i64) -> [f64; 3] {
        let w = self.width as i64;
        let x = ((x % w) + w) % w;
        let y = y.clamp(0, self.height as i64 - 1);
        let i = (y as usize * self.width + x as usize) * 3;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }

    pub fn sample_uv(&self, u: f64, v: f64) -> [f64; 3] {
        let x = u * self.width as f64 - 0.5;
        let y = v * self.height as f64 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let a = self.texel(x0, y0);
        let b = self.texel(x0 + 1, y0);
        let c = self.texel(x0, y0 + 1);
        let d = self.texel(x0 + 1, y0 + 1);
        let mut out = [0.0; 3];
        for k in 0..3 {
            out[k] = (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy;
        }
        out
    }

    pub fn sample(&self, d: Vec3) -> [f64; 3] {
        let (u, v) = dir_to_uv(d);
        self.sample_uv(u, v)
    }

    // 縮小・拡大どちらでも成り立つように、出力画素ごとに対応する入力画素の範囲を平均する
    pub fn downsample(&self, width: usize, height: usize) -> Equirect {
        let range = |t: usize, target: usize, source: usize| -> (usize, usize) {
            let start = t * source / target;
            let end = ((t + 1) * source + target - 1) / target;
            (start, end.max(start + 1).min(source))
        };
        let mut data = Vec::with_capacity(width * height * 3);
        for j in 0..height {
            let (y0, y1) = range(j, height, self.height);
            for i in 0..width {
                let (x0, x1) = range(i, width, self.width);
                let mut sum = [0.0; 3];
                let mut count = 0u32;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let c = self.texel(x as i64, y as i64);
                        for k in 0..3 {
                            sum[k] += c[k];
                        }
                        count += 1;
                    }
                }
                let n = count.max(1) as f64;
                data.extend_from_slice(&[sum[0] / n, sum[1] / n, sum[2] / n]);
            }
        }
        Equirect { width, height, data }
    }

    // テクセル (x, y) の方向と立体角
    fn texel_dir_solid_angle(&self, x: usize, y: usize) -> (Vec3, f64) {
        let u = (x as f64 + 0.5) / self.width as f64;
        let v = (y as f64 + 0.5) / self.height as f64;
        let theta = v * PI;
        (uv_to_dir(u, v), (2.0 * PI / self.width as f64) * (PI / self.height as f64) * theta.sin())
    }

    fn convolve(&self, width: usize, height: usize, weight: impl Fn(Vec3, Vec3) -> f64, normalize: bool) -> Equirect {
        let mut out = Vec::with_capacity(width * height * 3);
        for j in 0..height {
            for i in 0..width {
                let n = uv_to_dir((i as f64 + 0.5) / width as f64, (j as f64 + 0.5) / height as f64);
                let mut acc = [0.0; 3];
                let mut wsum = 0.0;
                for y in 0..self.height {
                    for x in 0..self.width {
                        let (l, omega) = self.texel_dir_solid_angle(x, y);
                        let w = weight(n, l) * omega;
                        if w <= 0.0 {
                            continue;
                        }
                        let c = self.texel(x as i64, y as i64);
                        for k in 0..3 {
                            acc[k] += c[k] * w;
                        }
                        wsum += w;
                    }
                }
                let scale = if normalize { 1.0 / wsum.max(1e-12) } else { 1.0 / PI };
                out.extend_from_slice(&[acc[0] * scale, acc[1] * scale, acc[2] * scale]);
            }
        }
        Equirect { width, height, data: out }
    }

    // N = V = R とみなした GGX の重み付き平均
    pub fn prefilter(&self, width: usize, height: usize, roughness: f64) -> Equirect {
        let a = roughness * roughness;
        let a2 = a * a;
        self.convolve(width, height, move |r, l| {
            let n_l = r.dot(l);
            if n_l <= 0.0 {
                return 0.0;
            }
            let h = (l + r).normalize();
            let n_h = r.dot(h).max(0.0);
            let dd = n_h * n_h * (a2 - 1.0) + 1.0;
            a2 / (PI * dd * dd) * n_l
        }, true)
    }

    // cos 重みの総和 / π。一様な環境ならその値になる
    pub fn irradiance(&self, width: usize, height: usize) -> Equirect {
        self.convolve(width, height, |n, l| n.dot(l).max(0.0), false)
    }
}

pub const CUBE_SIZE: usize = 256;
pub const CUBE_MIPS: usize = 6;
pub const IRRADIANCE_CUBE_SIZE: usize = 16;

// 面 face のテクセル座標 s, t ∈ [-1, 1] の方向。面の順と向きは WebGPU のキューブマップの規約
pub fn cube_dir(face: usize, s: f64, t: f64) -> Vec3 {
    match face {
        0 => Vec3::new(1.0, -t, -s),
        1 => Vec3::new(-1.0, -t, s),
        2 => Vec3::new(s, 1.0, t),
        3 => Vec3::new(s, -1.0, -t),
        4 => Vec3::new(s, -t, 1.0),
        _ => Vec3::new(-s, -t, -1.0),
    }
}

// 方向 → (面, u, v)。u, v ∈ [0, 1] は面内のテクスチャ座標
pub fn cube_face_uv(d: Vec3) -> (usize, f64, f64) {
    let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
    let (face, sc, tc, ma) = if ax >= ay && ax >= az {
        if d.x > 0.0 { (0, -d.z, -d.y, ax) } else { (1, d.z, -d.y, ax) }
    } else if ay >= az {
        if d.y > 0.0 { (2, d.x, d.z, ay) } else { (3, d.x, -d.z, ay) }
    } else if d.z > 0.0 {
        (4, d.x, -d.y, az)
    } else {
        (5, -d.x, -d.y, az)
    };
    let ma = ma.max(1e-30);
    (face, (sc / ma + 1.0) * 0.5, (tc / ma + 1.0) * 0.5)
}

// 6 面のキューブマップ(RGB)。本体の TextureCube と同じ並び。読み出しは面内を双線形(端はクランプ)
pub struct CubeMap {
    pub size: usize,
    pub faces: Vec<Vec<f64>>,
}

impl CubeMap {
    pub fn from_equirect(src: &Equirect, size: usize) -> CubeMap {
        let faces = (0..6)
            .map(|face| {
                let mut data = Vec::with_capacity(size * size * 3);
                for y in 0..size {
                    for x in 0..size {
                        let s = (x as f64 + 0.5) / size as f64 * 2.0 - 1.0;
                        let t = (y as f64 + 0.5) / size as f64 * 2.0 - 1.0;
                        data.extend_from_slice(&src.sample(cube_dir(face, s, t).normalize()));
                    }
                }
                data
            })
            .collect();
        CubeMap { size, faces }
    }

    pub fn texel(&self, face: usize, x: i64, y: i64) -> [f64; 3] {
        let n = self.size as i64;
        let i = ((y.clamp(0, n - 1) * n + x.clamp(0, n - 1)) * 3) as usize;
        let f = &self.faces[face];
        [f[i], f[i + 1], f[i + 2]]
    }

    pub fn sample(&self, d: Vec3) -> [f64; 3] {
        let (face, u, v) = cube_face_uv(d);
        let x = u * self.size as f64 - 0.5;
        let y = v * self.size as f64 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let a = self.texel(face, x0, y0);
        let b = self.texel(face, x0 + 1, y0);
        let c = self.texel(face, x0, y0 + 1);
        let e = self.texel(face, x0 + 1, y0 + 1);
        let mut out = [0.0; 3];
        for k in 0..3 {
            out[k] = (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + e[k] * fx) * fy;
        }
        out
    }
}


pub struct EnvMaps {
    /// 段 0 は元画像、段 k ≥ 1 は粗さ 0.2k のぼかし。面の大きさは 256 >> k
    pub mips: Vec<CubeMap>,
    pub irradiance_cube: CubeMap,
}

impl EnvMaps {
    pub fn build(source: Equirect) -> EnvMaps {
        let small = source.downsample(SMALL.0, SMALL.1);
        let prefiltered: Vec<Equirect> = ROUGHNESS_LEVELS.iter().map(|&r| small.prefilter(PREFILTERED.0, PREFILTERED.1, r)).collect();
        let irradiance = small.irradiance(IRRADIANCE.0, IRRADIANCE.1);
        let mut mips = vec![CubeMap::from_equirect(&source, CUBE_SIZE)];
        for (k, p) in prefiltered.iter().enumerate() {
            mips.push(CubeMap::from_equirect(p, CUBE_SIZE >> (k + 1)));
        }
        EnvMaps { mips, irradiance_cube: CubeMap::from_equirect(&irradiance, IRRADIANCE_CUBE_SIZE) }
    }

    // 粗さ → ミップ段。0 は段 0、0.2 未満は段 0 と 1 の間、以後 0.2 刻み
    pub fn lod(roughness: f64) -> f64 {
        let step = ROUGHNESS_LEVELS[0];
        let l = if roughness <= 0.0 {
            0.0
        } else if roughness < step {
            roughness / step
        } else {
            1.0 + (roughness - step) / step
        };
        l.min((CUBE_MIPS - 1) as f64)
    }

    pub fn specular(&self, d: Vec3, roughness: f64) -> [f64; 3] {
        let lod = Self::lod(roughness);
        let k = lod.floor() as usize;
        let k1 = (k + 1).min(CUBE_MIPS - 1);
        let t = lod - k as f64;
        let a = self.mips[k].sample(d);
        let b = self.mips[k1].sample(d);
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
    }

    pub fn background(&self, d: Vec3) -> [f64; 3] {
        self.mips[0].sample(d)
    }

    pub fn irradiance(&self, n: Vec3) -> [f64; 3] {
        self.irradiance_cube.sample(n)
    }
}

// Karis の環境 BRDF 近似。(A, B) で specular = prefiltered * (F0 * A + B)
pub fn env_brdf_approx(roughness: f64, n_dot_v: f64) -> (f64, f64) {
    let c0 = [-1.0, -0.0275, -0.572, 0.022];
    let c1 = [1.0, 0.0425, 1.04, -0.04];
    let r = [roughness * c0[0] + c1[0], roughness * c0[1] + c1[1], roughness * c0[2] + c1[2], roughness * c0[3] + c1[3]];
    let a004 = (r[0] * r[0]).min((-9.28 * n_dot_v).exp2()) * r[0] + r[1];
    (-1.04 * a004 + r[2], 1.04 * a004 + r[3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uv_roundtrip() {
        for (u, v) in [(0.1, 0.2), (0.5, 0.5), (0.9, 0.8), (0.0, 0.01)] {
            let (u2, v2) = dir_to_uv(uv_to_dir(u, v));
            assert!((u - u2).abs() < 1e-9 && (v - v2).abs() < 1e-9, "{} {} -> {} {}", u, v, u2, v2);
        }
    }

    // 軸に沿った方向は atan2 の x=0 に当たるので、GPU と同じく手で分ける
    #[test]
    fn axis_aligned_directions_have_fixed_u() {
        assert_eq!(dir_to_uv(Vec3::new(1.0, 0.0, 0.0)).0, 0.75);
        assert_eq!(dir_to_uv(Vec3::new(-1.0, 0.0, 0.0)).0, 0.25);
        assert_eq!(dir_to_uv(Vec3::new(0.0, 1.0, 0.0)).0, 0.5);
        assert_eq!(dir_to_uv(Vec3::new(0.0, -1.0, 0.0)).0, 0.5);
        assert_eq!(dir_to_uv(Vec3::new(1.0, 0.0, -0.0)).0, 0.75);
    }

    #[test]
    fn forward_is_center_and_up_is_top() {
        let (u, v) = dir_to_uv(Vec3::new(0.0, 0.0, -1.0));
        assert!((u - 0.5).abs() < 1e-12 && (v - 0.5).abs() < 1e-12);
        assert!(dir_to_uv(Vec3::new(0.0, 1.0, 0.0)).1 < 1e-9);
    }

    #[test]
    fn sample_at_texel_center_is_texel() {
        let mut e = Equirect::uniform(8, 4, [0.0; 3]);
        e.data[(2 * 8 + 5) * 3] = 1.0;
        let d = uv_to_dir(5.5 / 8.0, 2.5 / 4.0);
        assert!((e.sample(d)[0] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn sample_wraps_horizontally() {
        let mut e = Equirect::uniform(8, 4, [0.0; 3]);
        e.data[(1 * 8 + 0) * 3] = 1.0;
        e.data[(1 * 8 + 7) * 3] = 1.0;
        let d = uv_to_dir(0.0, 1.5 / 4.0);
        assert!((e.sample(d)[0] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn downsample_averages_blocks() {
        let mut e = Equirect::uniform(4, 2, [0.0; 3]);
        e.data[0] = 1.0;
        let s = e.downsample(2, 1);
        assert!((s.texel(0, 0)[0] - 0.25).abs() < 1e-12);
        assert_eq!(s.texel(1, 0)[0], 0.0);
    }

    #[test]
    fn uniform_environment_is_preserved() {
        let e = Equirect::uniform(128, 64, [0.5, 1.0, 2.0]);
        for map in [e.irradiance(32, 16), e.prefilter(64, 32, 0.5), e.prefilter(64, 32, 1.0)] {
            for c in map.data.chunks(3) {
                for k in 0..3 {
                    assert!((c[k] - [0.5, 1.0, 2.0][k]).abs() < 0.02 * [0.5, 1.0, 2.0][k], "{:?}", c);
                }
            }
        }
    }

    #[test]
    fn env_brdf_is_bounded() {
        for r in [0.0, 0.3, 0.7, 1.0] {
            for nv in [0.05, 0.5, 1.0] {
                let (a, b) = env_brdf_approx(r, nv);
                assert!(a >= 0.0 && b >= -0.01 && a + b <= 1.05, "{} {} -> {} {}", r, nv, a, b);
            }
        }
    }

    #[test]
    fn cube_face_roundtrip() {
        for face in 0..6 {
            for (s, t) in [(-0.7, 0.3), (0.0, 0.0), (0.9, -0.9), (0.25, 0.5)] {
                let d = cube_dir(face, s, t);
                let (f2, u, v) = cube_face_uv(d);
                assert_eq!(f2, face, "face {} s {} t {}", face, s, t);
                assert!((u - (s + 1.0) * 0.5).abs() < 1e-12 && (v - (t + 1.0) * 0.5).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn cube_texel_centers_return_texel_values() {
        let mut cube = CubeMap { size: 4, faces: vec![vec![0.0; 4 * 4 * 3]; 6] };
        cube.faces[2][(1 * 4 + 3) * 3] = 5.0;
        let d = cube_dir(2, 3.5 / 4.0 * 2.0 - 1.0, 1.5 / 4.0 * 2.0 - 1.0);
        assert!((cube.sample(d)[0] - 5.0).abs() < 1e-9);
        assert!(cube.sample(d * 3.0)[0] - 5.0 < 1e-9);
    }

    #[test]
    fn uniform_cube_is_uniform() {
        let maps = EnvMaps::build(Equirect::uniform(64, 32, [0.5, 1.0, 2.0]));
        for d in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.3, -0.9, 0.2).normalize(), Vec3::new(0.0, 0.0, -1.0)] {
            for r in [0.0, 0.1, 0.5, 1.0] {
                let c = maps.specular(d, r);
                for k in 0..3 {
                    assert!((c[k] - [0.5, 1.0, 2.0][k]).abs() < 0.03 * [0.5, 1.0, 2.0][k], "{:?}", c);
                }
            }
            let i = maps.irradiance(d);
            assert!((i[1] - 1.0).abs() < 0.03, "{:?}", i);
        }
    }

    #[test]
    fn lod_maps_roughness_steps() {
        assert_eq!(EnvMaps::lod(0.0), 0.0);
        assert!((EnvMaps::lod(0.1) - 0.5).abs() < 1e-12);
        assert!((EnvMaps::lod(0.2) - 1.0).abs() < 1e-12);
        assert!((EnvMaps::lod(0.5) - 2.5).abs() < 1e-12);
        assert_eq!(EnvMaps::lod(1.0), 5.0);
        assert_eq!(EnvMaps::lod(1.5), 5.0);
    }

    #[test]
    fn specular_blends_levels_by_roughness() {
        let mut src = Equirect::uniform(128, 64, [0.0; 3]);
        src.data[(10 * 128 + 64) * 3] = 100.0;
        let maps = EnvMaps::build(src);
        let d = uv_to_dir(64.5 / 128.0, 10.5 / 64.0);
        let sharp = maps.specular(d, 0.0)[0];
        let blur = maps.specular(d, 1.0)[0];
        assert!(sharp > blur * 10.0, "{} vs {}", sharp, blur);
    }

    // c * 2^(e - 136) の変換を、本体 (ibl.pde の loadRgbe) と同じ規約で確認する
    #[test]
    fn load_hdr_decodes_flat_rgbe() {
        let path = std::env::temp_dir().join("ep_rgbe_test.hdr");
        let mut bytes = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 2\n".to_vec();
        bytes.extend_from_slice(&[128, 64, 32, 129, 0, 0, 0, 0]);
        std::fs::write(&path, &bytes).unwrap();
        let e = Equirect::load_hdr(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(e.width, 2);
        assert_eq!(e.height, 1);
        let a = e.texel(0, 0);
        assert!((a[0] - 1.0).abs() < 1e-9 && (a[1] - 0.5).abs() < 1e-9 && (a[2] - 0.25).abs() < 1e-9, "{:?}", a);
        assert_eq!(e.texel(1, 0), [0.0, 0.0, 0.0]);
    }
}

