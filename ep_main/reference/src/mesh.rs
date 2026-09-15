use crate::vec::Vec3;
use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub u: f64,
    pub v: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    fn push(&mut self, position: Vec3, u: f64, normal: Vec3, v: f64) {
        self.vertices.push(Vertex { position, normal, u, v });
    }

    fn quad(&mut self, c00: u32, c10: u32, c01: u32, c11: u32) {
        self.indices.extend_from_slice(&[c00, c10, c11, c00, c11, c01]);
    }

    fn grid(&mut self, base: u32, columns: usize, rows: usize) {
        let n1 = (columns + 1) as u32;
        for j in 0..rows as u32 {
            for i in 0..columns as u32 {
                let c00 = base + j * n1 + i;
                self.quad(c00, c00 + 1, c00 + n1, c00 + n1 + 1);
            }
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    // 辺の長さ(scale 込み)が max_edge を超える辺を中点で割る。辺ごとの判定なので隣の三角形と割り方が
    // 食い違わない。割った点は元の三角形の平面上にあるので形は変わらず、頂点ごとの収縮を細かく掛けられる
    pub fn refine(&self, scale: Vec3, max_edge: f64) -> Mesh {
        let mut mesh = self.clone();
        loop {
            let mut midpoints: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
            let mut out = Vec::with_capacity(mesh.indices.len());
            let mut split_any = false;
            for t in 0..mesh.triangle_count() {
                let corners = [mesh.indices[t * 3], mesh.indices[t * 3 + 1], mesh.indices[t * 3 + 2]];
                let mut mids = [None; 3];
                for k in 0..3 {
                    let (a, b) = (corners[k], corners[(k + 1) % 3]);
                    let pa = mesh.vertices[a as usize].position.hadamard(scale);
                    let pb = mesh.vertices[b as usize].position.hadamard(scale);
                    if (pa - pb).length() > max_edge {
                        let key = (a.min(b), a.max(b));
                        let m = *midpoints.entry(key).or_insert_with(|| {
                            let va = mesh.vertices[a as usize];
                            let vb = mesh.vertices[b as usize];
                            mesh.vertices.push(Vertex {
                                position: (va.position + vb.position) * 0.5,
                                normal: (va.normal + vb.normal).normalize(),
                                u: (va.u + vb.u) * 0.5,
                                v: (va.v + vb.v) * 0.5,
                            });
                            (mesh.vertices.len() - 1) as u32
                        });
                        mids[k] = Some(m);
                        split_any = true;
                    }
                }
                let [c0, c1, c2] = corners;
                match mids {
                    [None, None, None] => out.extend_from_slice(&[c0, c1, c2]),
                    [Some(m0), Some(m1), Some(m2)] => out.extend_from_slice(&[c0, m0, m2, m0, c1, m1, m2, m1, c2, m0, m1, m2]),
                    _ => {
                        // 割れた辺が 1 本か 2 本。割れていない辺の向かいの隅から扇状に分ける
                        let mut first = 0;
                        while mids[first].is_none() {
                            first += 1;
                        }
                        let rot = |k: usize| corners[(first + k) % 3];
                        let mid = |k: usize| mids[(first + k) % 3];
                        let (a, b, c) = (rot(0), rot(1), rot(2));
                        let m0 = mid(0).unwrap();
                        match (mid(1), mid(2)) {
                            (None, None) => out.extend_from_slice(&[a, m0, c, m0, b, c]),
                            (Some(m1), None) => out.extend_from_slice(&[a, m0, c, m0, b, m1, m0, m1, c]),
                            (None, Some(m2)) => out.extend_from_slice(&[a, m0, m2, m0, b, c, m0, c, m2]),
                            (Some(_), Some(_)) => unreachable!(),
                        }
                    }
                }
            }
            mesh.indices = out;
            if !split_any {
                return mesh;
            }
        }
    }
}

const CUBE_NORMALS: [Vec3; 6] = [
    Vec3::new(1.0, 0.0, 0.0),
    Vec3::new(-1.0, 0.0, 0.0),
    Vec3::new(0.0, 1.0, 0.0),
    Vec3::new(0.0, -1.0, 0.0),
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(0.0, 0.0, -1.0),
];

fn face_axes(n: Vec3) -> (Vec3, Vec3) {
    let u = Vec3::new(n.y, n.z, n.x);
    (u, n.cross(u))
}

// cube_mesh.pde
pub fn cube(divisions: usize) -> Mesh {
    let mut mesh = Mesh::default();
    let n1 = divisions + 1;
    for n in CUBE_NORMALS {
        let (u, v) = face_axes(n);
        for j in 0..n1 {
            for i in 0..n1 {
                let s0 = 2.0 * i as f64 / divisions as f64 - 1.0;
                let s1 = 2.0 * j as f64 / divisions as f64 - 1.0;
                mesh.push((n + u * s0 + v * s1) * 0.5, i as f64 / divisions as f64, n, j as f64 / divisions as f64);
            }
        }
    }
    for face in 0..6 {
        mesh.grid((face * n1 * n1) as u32, divisions, divisions);
    }
    mesh
}

// plane_mesh.pde
pub fn plane(divisions: usize) -> Mesh {
    let mut mesh = Mesh::default();
    let n1 = divisions + 1;
    for j in 0..n1 {
        for i in 0..n1 {
            let fu = i as f64 / divisions as f64;
            let fv = j as f64 / divisions as f64;
            mesh.push(Vec3::new(fu - 0.5, 0.0, fv - 0.5), fu, Vec3::new(0.0, 1.0, 0.0), fv);
        }
    }
    mesh.grid(0, divisions, divisions);
    mesh
}

// wheel_mesh.pde
#[allow(clippy::too_many_arguments)]
pub fn wheel(
    divisions: usize,
    sub_divisions: usize,
    radius: f64,
    radial_thickness: f64,
    thickness: f64,
    spoke_count: usize,
    spoke_size: f64,
    spoke_thickness: f64,
    hub_radius: f64,
    hub_thickness: f64,
) -> Mesh {
    let mut mesh = Mesh::default();
    let n1 = divisions + 1;
    let m1 = sub_divisions + 1;
    let inner = radius - radial_thickness;
    let hz = thickness * 0.5;
    let hhz = hub_thickness * 0.5;

    let faces = [
        [radius, -hz, radius, hz, 1.0, 0.0],
        [inner, hz, inner, -hz, -1.0, 0.0],
        [radius, hz, inner, hz, 0.0, 1.0],
        [inner, -hz, radius, -hz, 0.0, -1.0],
        [hub_radius, -hhz, hub_radius, hhz, 1.0, 0.0],
        [hub_radius, hhz, 0.0, hhz, 0.0, 1.0],
        [0.0, -hhz, hub_radius, -hhz, 0.0, -1.0],
    ];
    for f in faces {
        for j in 0..m1 {
            let fv = j as f64 / sub_divisions as f64;
            let r = f[0] + (f[2] - f[0]) * fv;
            let z = f[1] + (f[3] - f[1]) * fv;
            for i in 0..n1 {
                let fu = i as f64 / divisions as f64;
                let (s, c) = (TAU * fu).sin_cos();
                mesh.push(Vec3::new(r * c, r * s, z), fu, Vec3::new(f[4] * c, f[4] * s, f[5]), fv);
            }
        }
    }

    let spoke_length = radius - radial_thickness * 0.5;
    for k in 0..spoke_count {
        let a = TAU * k as f64 / spoke_count as f64;
        let (sa, ca) = a.sin_cos();
        for n in CUBE_NORMALS {
            let (u, v) = face_axes(n);
            for j in 0..m1 {
                for i in 0..m1 {
                    let s0 = 2.0 * i as f64 / sub_divisions as f64 - 1.0;
                    let s1 = 2.0 * j as f64 / sub_divisions as f64 - 1.0;
                    let p = (n + u * s0 + v * s1) * 0.5;
                    let lx = (p.x + 0.5) * spoke_length;
                    let ly = p.y * spoke_size;
                    let lz = p.z * spoke_thickness;
                    mesh.push(
                        Vec3::new(lx * ca - ly * sa, lx * sa + ly * ca, lz),
                        i as f64 / sub_divisions as f64,
                        Vec3::new(n.x * ca - n.y * sa, n.x * sa + n.y * ca, n.z),
                        j as f64 / sub_divisions as f64,
                    );
                }
            }
        }
    }

    let mut base = 0u32;
    for _ in 0..7 {
        mesh.grid(base, divisions, sub_divisions);
        base += (n1 * m1) as u32;
    }
    for _ in 0..spoke_count * 6 {
        mesh.grid(base, sub_divisions, sub_divisions);
        base += (m1 * m1) as u32;
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_counts_match_pde() {
        let m = cube(1);
        assert_eq!(m.vertices.len(), 24);
        assert_eq!(m.indices.len(), 36);
        for v in &m.vertices {
            assert!((v.position.x.abs() - 0.5).abs() < 1e-12);
            assert!((v.position.y.abs() - 0.5).abs() < 1e-12);
            assert!((v.position.z.abs() - 0.5).abs() < 1e-12);
        }
        let m = cube(2);
        assert_eq!(m.vertices.len(), 54);
        assert_eq!(m.indices.len(), 144);
    }

    #[test]
    fn plane_counts_match_pde() {
        let m = plane(1);
        assert_eq!(m.vertices.len(), 4);
        assert_eq!(m.indices.len(), 6);
    }

    #[test]
    fn wheel_counts_match_pde() {
        let m = wheel(64, 1, 4.0, 0.3, 0.2, 10, 0.1, 0.1, 0.3, 0.3);
        assert_eq!(m.vertices.len(), 7 * 65 * 2 + 10 * 6 * 4);
        assert_eq!(m.indices.len(), (7 * 64 + 10 * 6) * 6);
        assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
    }

    #[test]
    fn refine_splits_long_edges_without_cracks() {
        let m = cube(1).refine(Vec3::new(1.0, 5.0, 1.0), 1.2);
        for t in 0..m.triangle_count() {
            for k in 0..3 {
                let a = m.vertices[m.indices[t * 3 + k] as usize].position.hadamard(Vec3::new(1.0, 5.0, 1.0));
                let b = m.vertices[m.indices[t * 3 + (k + 1) % 3] as usize].position.hadamard(Vec3::new(1.0, 5.0, 1.0));
                assert!((a - b).length() <= 1.2 + 1e-12);
            }
        }
        for v in &m.vertices {
            assert!((v.position.x.abs() - 0.5).abs() < 1e-12 || (v.position.y.abs() - 0.5).abs() < 1e-12 || (v.position.z.abs() - 0.5).abs() < 1e-12);
        }
        // 面の内側の辺は 2 つの三角形に使われ、1 つにしか使われない辺は箱の稜線上にある(面ごとに頂点が別なので)
        let mut edges: std::collections::HashMap<(u32, u32), i32> = std::collections::HashMap::new();
        for t in 0..m.triangle_count() {
            for k in 0..3 {
                let a = m.indices[t * 3 + k];
                let b = m.indices[t * 3 + (k + 1) % 3];
                *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let on_box_edge = |i: u32| {
            let p = m.vertices[i as usize].position;
            [p.x, p.y, p.z].iter().filter(|c| (c.abs() - 0.5).abs() < 1e-12).count() >= 2
        };
        for (&(a, b), &n) in &edges {
            assert!(n == 2 || (n == 1 && on_box_edge(a) && on_box_edge(b)), "edge {}-{} used {} times", a, b, n);
        }
        assert!(m.triangle_count() > 12);
        let area: f64 = (0..m.triangle_count())
            .map(|t| {
                let p = |k: usize| m.vertices[m.indices[t * 3 + k] as usize].position.hadamard(Vec3::new(1.0, 5.0, 1.0));
                (p(1) - p(0)).cross(p(2) - p(0)).length() * 0.5
            })
            .sum();
        assert!((area - (2.0 * 1.0 + 4.0 * 5.0)).abs() < 1e-9, "{}", area);
    }

    #[test]
    fn cube_faces_wind_outward() {
        let m = cube(1);
        for t in 0..m.triangle_count() {
            let a = m.vertices[m.indices[t * 3] as usize].position;
            let b = m.vertices[m.indices[t * 3 + 1] as usize].position;
            let c = m.vertices[m.indices[t * 3 + 2] as usize].position;
            let n = (b - a).cross(c - a);
            let outward = (a + b + c) * (1.0 / 3.0);
            assert!(n.dot(outward) != 0.0, "triangle {} degenerate", t);
        }
    }
}
