use crate::scene::Scene;
use crate::vec::Vec3;

pub const HIT_EPS: f64 = 1e-8;

pub struct Ray {
    pub origin: Vec3,
    pub dir: Vec3,
    pub time: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub object: usize,
    pub triangle: usize,
    pub s: f64,
    pub tau: f64,
    pub point: Vec3,
    pub bary: (f64, f64, f64),
    pub geometric_normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub enum Outcome {
    Hit(Hit),
    Miss,
    StepLimit,
}

// 点 p から三角形 abc への距離と最近点の重心座標
pub fn point_triangle_distance(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> (f64, (f64, f64, f64)) {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (ap.length(), (1.0, 0.0, 0.0));
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (bp.length(), (0.0, 1.0, 0.0));
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return ((p - (a + ab * v)).length(), (1.0 - v, v, 0.0));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (cp.length(), (0.0, 0.0, 1.0));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return ((p - (a + ac * w)).length(), (1.0 - w, 0.0, w));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return ((p - (b + (c - b) * w)).length(), (0.0, 1.0 - w, w));
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    ((p - (a + ab * v + ac * w)).length(), (1.0 - v - w, v, w))
}

fn sphere_interval(ray: &Ray, center: Vec3, radius: f64) -> Option<(f64, f64)> {
    let oc = ray.origin - center;
    let b = oc.dot(ray.dir);
    let c = oc.dot(oc) - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let root = disc.sqrt();
    let (s0, s1) = (-b - root, -b + root);
    if s1 < 0.0 { None } else { Some((s0.max(0.0), s1)) }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct Stats {
    pub rays: u64,
    pub steps: u64,
    pub sphere_steps: u64,
    pub group_checks: u64,
    pub triangle_measures: u64,
}

impl Stats {
    pub fn add(&mut self, o: &Stats) {
        self.rays += o.rays;
        self.steps += o.steps;
        self.sphere_steps += o.sphere_steps;
        self.group_checks += o.group_checks;
        self.triangle_measures += o.triangle_measures;
    }
}

pub struct Tracer {
    positions: Vec<Vec3>,
    stamps: Vec<u32>,
    bounds: Vec<(f64, f64)>,
    group_order: Vec<(f64, usize)>,
    pub stats: Stats,
}

impl Tracer {
    pub fn new() -> Tracer {
        Tracer { positions: Vec::new(), stamps: Vec::new(), bounds: Vec::new(), group_order: Vec::new(), stats: Stats::default() }
    }

    // 距離 s の点は時刻 time − s / c の光。その時刻の三角形までの距離 / k ずつ進み、HIT_EPS より近づいたら交点。
    // 遠いうちは物体を囲む球までの距離で進む。近づいたら組ごとの球で近い順に見て、球までの距離が
    // それまでの最小距離以上の組は飛ばす。三角形は前に測った距離からの下界(距離の変化率が k 以下なので
    // Δ 進んだ後も g − kΔ より近くにない)が最小距離以上なら飛ばす
    pub fn trace(&mut self, scene: &Scene, ray: &Ray, exclude: Option<(usize, usize)>, max_steps: usize) -> Outcome {
        let mut best: Option<Hit> = None;
        let mut limited_at = f64::INFINITY;
        self.stats.rays += 1;
        for (oi, object) in scene.objects.iter().enumerate() {
            let Some((s_in, s_out)) = sphere_interval(ray, object.bound_center, object.bound_radius) else { continue };
            let indices = &object.mesh.indices;
            self.positions.resize(object.mesh.vertices.len(), Vec3::default());
            self.stamps.clear();
            self.stamps.resize(object.mesh.vertices.len(), 0);
            self.bounds.clear();
            self.bounds.resize(object.mesh.triangle_count(), (f64::NEG_INFINITY, 0.0));
            let mut nearest_triangle = usize::MAX;
            let mut s = s_in;
            let mut steps = 0u32;
            loop {
                if let Some(h) = &best {
                    if s >= h.s {
                        break;
                    }
                }
                let tau = ray.time - s / scene.c;
                let pose = object.motion.pose(tau);
                let x = ray.origin + ray.dir * s;
                let mut g = object.sphere_lower_bound(x, &pose);
                let mut nearest_bary = (0.0, 0.0, 0.0);
                self.stats.steps += 1;
                if g > HIT_EPS {
                    self.stats.sphere_steps += 1;
                } else {
                    let stamp = steps + 1;
                    let vertex = |i: usize, positions: &mut Vec<Vec3>, stamps: &mut Vec<u32>| -> Vec3 {
                        if stamps[i] != stamp {
                            stamps[i] = stamp;
                            positions[i] = object.vertex_at_pose(i, &pose, scene.c);
                        }
                        positions[i]
                    };
                    let measure = |t: usize, positions: &mut Vec<Vec3>, stamps: &mut Vec<u32>, bounds: &mut Vec<(f64, f64)>| {
                        let a = vertex(indices[t * 3] as usize, positions, stamps);
                        let b = vertex(indices[t * 3 + 1] as usize, positions, stamps);
                        let c = vertex(indices[t * 3 + 2] as usize, positions, stamps);
                        let (d, bary) = point_triangle_distance(x, a, b, c);
                        bounds[t] = (d, s);
                        (d, bary)
                    };
                    g = f64::INFINITY;
                    if nearest_triangle != usize::MAX {
                        let (d, bary) = measure(nearest_triangle, &mut self.positions, &mut self.stamps, &mut self.bounds);
                        g = d;
                        nearest_bary = bary;
                    }
                    self.group_order.clear();
                    self.stats.group_checks += object.groups.len() as u64;
                    for (gi, group) in object.groups.iter().enumerate() {
                        let (center, radius) = object.group_sphere(group, &pose);
                        self.group_order.push(((x - center).length() - radius, gi));
                    }
                    self.group_order.sort_by(|a, b| a.0.total_cmp(&b.0));
                    for &(lower, gi) in &self.group_order {
                        if lower >= g {
                            break;
                        }
                        for &t in &object.groups[gi].triangles {
                            let t = t as usize;
                            if exclude == Some((oi, t)) {
                                continue;
                            }
                            let (g_then, s_then) = self.bounds[t];
                            if g_then - object.k * (s - s_then) >= g {
                                continue;
                            }
                            self.stats.triangle_measures += 1;
                            let (d, bary) = measure(t, &mut self.positions, &mut self.stamps, &mut self.bounds);
                            if d < g {
                                g = d;
                                nearest_triangle = t;
                                nearest_bary = bary;
                            }
                        }
                    }
                    if g < HIT_EPS {
                        let t = nearest_triangle;
                        let a = self.positions[indices[t * 3] as usize];
                        let b = self.positions[indices[t * 3 + 1] as usize];
                        let c = self.positions[indices[t * 3 + 2] as usize];
                        let mut n = (b - a).cross(c - a).normalize();
                        if n.dot(ray.dir) > 0.0 {
                            n = -n;
                        }
                        best = Some(Hit { object: oi, triangle: t, s, tau, point: x, bary: nearest_bary, geometric_normal: n });
                        break;
                    }
                }
                s += g / object.k;
                steps += 1;
                if s > s_out {
                    break;
                }
                if steps as usize >= max_steps {
                    limited_at = limited_at.min(s);
                    break;
                }
            }
        }
        match best {
            Some(h) if h.s <= limited_at => Outcome::Hit(h),
            _ if limited_at.is_finite() => Outcome::StepLimit,
            _ => Outcome::Miss,
        }
    }

    pub fn shadow(&mut self, scene: &Scene, hit: &Hit, max_steps: usize) -> f64 {
        if !scene.shadow_enabled {
            return 1.0;
        }
        if hit.geometric_normal.dot(scene.to_light) <= 0.0 {
            return 0.0;
        }
        let ray = Ray { origin: hit.point + hit.geometric_normal * 1e-6, dir: scene.to_light, time: hit.tau };
        match self.trace(scene, &ray, Some((hit.object, hit.triangle)), max_steps) {
            Outcome::Hit(_) => 0.0,
            _ => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh;
    use crate::scene::{Material, Motion, Object};

    fn scene_of(objects: Vec<Object>, c: f64) -> Scene {
        let mut scene = Scene {
            objects,
            c,
            to_light: Vec3::new(0.0, 1.0, 0.0),
            ambient: 0.2,
            light_intensity: 1.0,
            pbr_enabled: true,
            ibl_enabled: false,
            ibl_intensity: 1.0,
            env: None,
            shadow_enabled: true,
        };
        for o in &mut scene.objects {
            o.prepare(c, 0.0, 20.0);
        }
        scene
    }

    #[test]
    fn distance_above_triangle_is_height() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 0.0, 0.0);
        let c = Vec3::new(0.0, 0.0, 1.0);
        let (d, bary) = point_triangle_distance(Vec3::new(0.25, 2.0, 0.25), a, b, c);
        assert!((d - 2.0).abs() < 1e-12);
        assert!((bary.0 - 0.5).abs() < 1e-12 && (bary.1 - 0.25).abs() < 1e-12 && (bary.2 - 0.25).abs() < 1e-12);
        let (d, bary) = point_triangle_distance(Vec3::new(3.0, 0.0, -1.0), a, b, c);
        assert!((d - (5.0f64).sqrt()).abs() < 1e-12);
        assert_eq!(bary, (0.0, 1.0, 0.0));
    }

    #[test]
    fn still_floor_hit_matches_plane_intersection() {
        let floor = Object::new(
            "plane",
            mesh::plane(1),
            Motion::Still { position: Vec3::new(0.0, -2.5, 0.0) },
            Vec3::new(100.0, 1.0, 100.0),
            Material::Checker,
        );
        let scene = scene_of(vec![floor], 1e6);
        let ray = Ray { origin: Vec3::default(), dir: Vec3::new(0.0, -1.0, 1.0).normalize(), time: 10.0 };
        match Tracer::new().trace(&scene, &ray, None, 10000) {
            Outcome::Hit(h) => {
                assert!((h.s - 2.5 * 2.0f64.sqrt()).abs() < 1e-7);
                assert!((h.point.y + 2.5).abs() < 1e-7);
            }
            other => panic!("{:?}", other),
        }
    }

    // 床(y = −2.5)の点に時刻 10 で届く光を真上から追う
    fn floor_hit(scene: &Scene, tracer: &mut Tracer, x: f64, z: f64) -> Hit {
        let ray = Ray { origin: Vec3::new(x, 10.0, z), dir: Vec3::new(0.0, -1.0, 0.0), time: 10.0 + 12.5 / scene.c };
        match tracer.trace(scene, &ray, None, 10000) {
            Outcome::Hit(h) => h,
            other => panic!("{:?}", other),
        }
    }

    fn floor() -> Object {
        Object::new(
            "plane",
            mesh::plane(1),
            Motion::Still { position: Vec3::new(0.0, -2.5, 0.0) },
            Vec3::new(100.0, 1.0, 100.0),
            Material::Checker,
        )
    }

    // 高さ 5 の静止した箱の影の先端は、上面の角から光と逆向きに床まで下ろした点
    #[test]
    fn still_box_shadow_tip_matches_projection() {
        let tower = Object::new("box", mesh::cube(1), Motion::Still { position: Vec3::default() }, Vec3::new(1.0, 5.0, 1.0), Material::Uv);
        let mut scene = scene_of(vec![floor(), tower], 1e6);
        scene.to_light = Vec3::new(0.4, 0.8, 0.6).normalize();
        let mut tracer = Tracer::new();
        let l = scene.to_light;
        let tip = Vec3::new(-0.5, 2.5, -0.5) - l * (5.0 / l.y);
        assert!((tip.y + 2.5).abs() < 1e-12);
        let inward = (Vec3::new(0.0, -2.5, 0.0) - tip).normalize() * 1e-3;
        let inside = tip + inward;
        let outside = tip - inward;
        let h = floor_hit(&scene, &mut tracer, inside.x, inside.z);
        assert_eq!(h.object, 0);
        assert_eq!(tracer.shadow(&scene, &h, 10000), 0.0);
        let h = floor_hit(&scene, &mut tracer, outside.x, outside.z);
        assert_eq!(tracer.shadow(&scene, &h, 10000), 1.0);
        let h = floor_hit(&scene, &mut tracer, 3.0, 3.0);
        assert_eq!(tracer.shadow(&scene, &h, 10000), 1.0);
    }

    // x 方向に動く箱の影は、床に届く光が箱を通った時刻ぶんだけ上ほどずれる
    #[test]
    fn moving_box_shadow_tip_is_shifted_by_light_travel() {
        let v = 2.0;
        let c = 8.0;
        let tower = Object::new(
            "box",
            mesh::cube(1),
            Motion::Linear { position0: Vec3::new(-20.0, 0.0, 0.0), velocity: Vec3::new(v, 0.0, 0.0) },
            Vec3::new(1.0, 5.0, 1.0),
            Material::Uv,
        );
        let mut scene = scene_of(vec![floor(), tower], c);
        scene.to_light = Vec3::new(0.4, 0.8, 0.6).normalize();
        let mut tracer = Tracer::new();
        let l = scene.to_light;
        let u = 5.0 / l.y;
        let half_x = 0.5 * (1.0 - (v / c) * (v / c)).sqrt();
        // 床の点(時刻 10)に届く光は時刻 10 − u / c に箱の上面を通る
        let center_x = -20.0 + v * (10.0 - u / c);
        let tip = Vec3::new(center_x - half_x, 2.5, -0.5) - l * u;
        let inward = (Vec3::new(center_x, 2.5, 0.0) - l * u - tip).normalize() * 1e-3;
        let h = floor_hit(&scene, &mut tracer, tip.x + inward.x, tip.z + inward.z);
        assert_eq!(tracer.shadow(&scene, &h, 10000), 0.0);
        let h = floor_hit(&scene, &mut tracer, tip.x - inward.x, tip.z - inward.z);
        assert_eq!(tracer.shadow(&scene, &h, 10000), 1.0);
    }

    // 等速直線運動する箱の面は平面が一次式で動くので、閉形式で交点が出る
    fn closed_form_box_hit(x0: Vec3, v: Vec3, half: Vec3, dir: Vec3, t: f64, c: f64) -> Option<f64> {
        let mut best: Option<f64> = None;
        for axis in 0..3 {
            for sign in [-1.0, 1.0] {
                let n = match axis {
                    0 => Vec3::new(sign, 0.0, 0.0),
                    1 => Vec3::new(0.0, sign, 0.0),
                    _ => Vec3::new(0.0, 0.0, sign),
                };
                let h = [half.x, half.y, half.z][axis];
                let denom = n.dot(dir) + n.dot(v) / c;
                if denom.abs() < 1e-12 {
                    continue;
                }
                let s = (h + n.dot(x0) + n.dot(v) * t) / denom;
                if s < 0.0 {
                    continue;
                }
                let center = x0 + v * (t - s / c);
                let rel = dir * s - center;
                let inside = [rel.x, rel.y, rel.z]
                    .iter()
                    .zip([half.x, half.y, half.z])
                    .enumerate()
                    .all(|(k, (r, hh))| k == axis || r.abs() <= hh + 1e-12);
                if inside && best.is_none_or(|b| s < b) {
                    best = Some(s);
                }
            }
        }
        best
    }

    #[test]
    fn moving_box_hit_matches_closed_form() {
        let x0 = Vec3::new(-35.0, 0.0, 10.0);
        let v = Vec3::new(4.0, 0.0, 0.0);
        let c = 8.0;
        let cube = Object::new(
            "cube",
            mesh::cube(1),
            Motion::Linear { position0: x0, velocity: v },
            Vec3::new(1.0, 1.0, 1.0),
            Material::Uv,
        );
        let scene = scene_of(vec![cube], c);
        let half = Vec3::new(0.5 * (1.0f64 - 0.25).sqrt(), 0.5, 0.5);
        let mut tracer = Tracer::new();
        let mut hits = 0;
        for &angle in &[0.0f64, 0.03, 0.05, 0.0697, 0.08, 0.3] {
            let dir = Vec3::new(angle.sin(), 0.0, angle.cos());
            let expect = closed_form_box_hit(x0, v, half, dir, 10.0, c);
            let ray = Ray { origin: Vec3::default(), dir, time: 10.0 };
            match (tracer.trace(&scene, &ray, None, 100000), expect) {
                (Outcome::Hit(h), Some(s)) => {
                    assert!((h.s - s).abs() < 1e-7, "angle {}: {} vs {}", angle, h.s, s);
                    hits += 1;
                }
                (Outcome::Miss, None) => {}
                (got, want) => panic!("angle {}: {:?} vs {:?}", angle, got, want),
            }
        }
        assert!(hits >= 3);
    }
}
