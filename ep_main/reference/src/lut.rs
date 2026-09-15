// .cube 形式の 3D LUT。本体の tonemap.pde / tonemap.slang と同じ並びと補間

pub struct Lut3d {
    pub size: usize,
    pub data: Vec<f64>,
}

pub fn index(size: usize, r: usize, g: usize, b: usize) -> usize {
    ((b * size + g) * size + r) * 3
}

impl Lut3d {
    pub fn from_fn(size: usize, f: impl Fn([f64; 3]) -> [f64; 3]) -> Lut3d {
        let mut data = Vec::with_capacity(size * size * size * 3);
        let step = 1.0 / (size - 1) as f64;
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    data.extend_from_slice(&f([r as f64 * step, g as f64 * step, b as f64 * step]));
                }
            }
        }
        Lut3d { size, data }
    }

    pub fn parse_cube(text: &str) -> Result<Lut3d, String> {
        let mut size = 0usize;
        let mut data = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("TITLE") || line.starts_with("DOMAIN_") {
                continue;
            }
            if let Some(rest) = line.strip_prefix("LUT_3D_SIZE") {
                size = rest.trim().parse().map_err(|e| format!("LUT_3D_SIZE: {}", e))?;
                continue;
            }
            if line.starts_with("LUT_1D_SIZE") {
                return Err("1D LUT は扱いません".into());
            }
            for v in line.split_whitespace() {
                data.push(v.parse::<f64>().map_err(|e| format!("{}: {}", v, e))?);
            }
        }
        if size < 2 {
            return Err("LUT_3D_SIZE がありません".into());
        }
        if data.len() != size * size * size * 3 {
            return Err(format!("値の数が合いません: {} (期待 {})", data.len(), size * size * size * 3));
        }
        Ok(Lut3d { size, data })
    }

    pub fn to_cube(&self, title: &str) -> String {
        let mut s = format!("TITLE \"{}\"\nLUT_3D_SIZE {}\n", title, self.size);
        for c in self.data.chunks(3) {
            s.push_str(&format!("{:.6} {:.6} {:.6}\n", c[0], c[1], c[2]));
        }
        s
    }

    pub fn sample(&self, t: [f64; 3]) -> [f64; 3] {
        let n = self.size;
        let mut i0 = [0usize; 3];
        let mut i1 = [0usize; 3];
        let mut f = [0f64; 3];
        for k in 0..3 {
            let p = t[k].clamp(0.0, 1.0) * (n - 1) as f64;
            let a = (p.floor() as usize).min(n - 2);
            i0[k] = a;
            i1[k] = a + 1;
            f[k] = p - a as f64;
        }
        let mut out = [0f64; 3];
        for (bi, wb) in [(i0[2], 1.0 - f[2]), (i1[2], f[2])] {
            for (gi, wg) in [(i0[1], 1.0 - f[1]), (i1[1], f[1])] {
                for (ri, wr) in [(i0[0], 1.0 - f[0]), (i1[0], f[0])] {
                    let w = wr * wg * wb;
                    let i = index(n, ri, gi, bi);
                    for k in 0..3 {
                        out[k] += self.data[i + k] * w;
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Lut3d {
        Lut3d::from_fn(5, |t| [t[0], t[1] * 0.5, 1.0 - t[2]])
    }

    #[test]
    fn grid_points_return_table_values() {
        let lut = ramp();
        let n = lut.size;
        for b in 0..n {
            for g in 0..n {
                for r in 0..n {
                    let t = [r as f64 / 4.0, g as f64 / 4.0, b as f64 / 4.0];
                    let out = lut.sample(t);
                    let i = index(n, r, g, b);
                    for k in 0..3 {
                        assert!((out[k] - lut.data[i + k]).abs() < 1e-12);
                    }
                }
            }
        }
    }

    #[test]
    fn trilinear_is_linear_between_grid_points() {
        let lut = ramp();
        let out = lut.sample([0.3, 0.6, 0.9]);
        assert!((out[0] - 0.3).abs() < 1e-12);
        assert!((out[1] - 0.3).abs() < 1e-12);
        assert!((out[2] - 0.1).abs() < 1e-12);
    }

    #[test]
    fn cube_roundtrip() {
        let lut = ramp();
        let text = lut.to_cube("test");
        assert!(text.starts_with("TITLE \"test\"\nLUT_3D_SIZE 5\n"));
        let back = Lut3d::parse_cube(&text).unwrap();
        assert_eq!(back.size, 5);
        for (a, b) in lut.data.iter().zip(&back.data) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn parse_skips_comments_and_domain() {
        let text = "# comment\nTITLE \"x\"\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 1 1 1\nLUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n";
        let lut = Lut3d::parse_cube(text).unwrap();
        assert_eq!(lut.size, 2);
        assert_eq!(lut.sample([1.0, 0.0, 1.0]), [1.0, 0.0, 1.0]);
    }

    #[test]
    fn parse_rejects_wrong_count() {
        assert!(Lut3d::parse_cube("LUT_3D_SIZE 2\n0 0 0\n").is_err());
    }
}
