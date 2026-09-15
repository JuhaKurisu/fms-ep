use crate::ibl::EnvMaps;
use crate::mesh::{self, Mesh};
use crate::prefs::{Prefs, Snapshot};
use crate::vec::{Quat, Vec3};
use std::f64::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Material {
    Uv,
    Checker,
}

#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
}

// ep_main.pde の update() にある運動の式
#[derive(Clone, Debug)]
pub enum Motion {
    Still { position: Vec3 },
    #[allow(dead_code)]
    Linear { position0: Vec3, velocity: Vec3 },
    Sway { origin: Vec3, amplitude: f64, period: f64 },
    Wheel { origin: Vec3, radius: f64, rotate_speed: f64, half_range: f64 },
    Wave { origin: Vec3, width: f64, speed: f64, start_time: f64 },
}

impl Motion {
    pub fn pose(&self, t: f64) -> Pose {
        let zero = Vec3::default();
        match *self {
            Motion::Still { position } => Pose { position, rotation: Quat::identity(), velocity: zero, angular_velocity: zero },
            Motion::Linear { position0, velocity } => {
                Pose { position: position0 + velocity * t, rotation: Quat::identity(), velocity, angular_velocity: zero }
            }
            Motion::Sway { origin, amplitude, period } => {
                let w = TAU / period;
                Pose {
                    position: origin + Vec3::new((w * t).sin() * amplitude, 0.0, 0.0),
                    rotation: Quat::identity(),
                    velocity: Vec3::new((w * t).cos() * amplitude * w, 0.0, 0.0),
                    angular_velocity: zero,
                }
            }
            Motion::Wheel { origin, radius, rotate_speed, half_range } => {
                let h = half_range;
                let period = 4.0 * h;
                let speed = rotate_speed * radius;
                let travel = speed * t;
                let phase = travel.rem_euclid(period);
                let (offset, slope) = if phase < h {
                    (phase, 1.0)
                } else if phase < 3.0 * h {
                    (2.0 * h - phase, -1.0)
                } else {
                    (phase - period, 1.0)
                };
                Pose {
                    position: origin + Vec3::new(offset, 0.0, 0.0),
                    rotation: Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), -offset / radius),
                    velocity: Vec3::new(slope * speed, 0.0, 0.0),
                    angular_velocity: Vec3::new(0.0, 0.0, -slope * speed / radius),
                }
            }
            Motion::Wave { origin, width, speed, start_time } => {
                let tt = t - start_time;
                let duration = PI * width.abs() / speed.abs().max(1e-4);
                let (y, vy) = if tt < 0.0 || tt > duration {
                    (0.0, 0.0)
                } else {
                    let w = TAU / duration;
                    (width * 0.5 * (1.0 - (w * tt).cos()), width * 0.5 * w * (w * tt).sin())
                };
                Pose {
                    position: origin + Vec3::new(0.0, y, 0.0),
                    rotation: Quat::identity(),
                    velocity: Vec3::new(0.0, vy, 0.0),
                    angular_velocity: zero,
                }
            }
        }
    }
}

// 静止形状(scale 込み)の格子で三角形を分けた組。center / radius はその組の頂点を囲む球
pub struct Group {
    pub triangles: Vec<u32>,
    pub vertices: Vec<u32>,
    pub center: Vec3,
    pub radius: f64,
    pub slack: f64,   // 収縮で頂点が静止時の球からはみ出す量の最大(prepare で実測)
}

pub struct Object {
    pub name: String,
    pub mesh: Mesh,
    pub motion: Motion,
    pub scale: Vec3,
    pub material: Material,
    pub groups: Vec<Group>,
    pub bound_center: Vec3,
    pub bound_radius: f64,
    pub local_radius: f64,
    pub k: f64,
    pub roughness: f64,
    pub metallic: f64,
}

impl Object {
    pub fn new(name: &str, mesh: Mesh, motion: Motion, scale: Vec3, material: Material) -> Object {
        let local_radius = mesh.vertices.iter().map(|v| v.position.hadamard(scale).length()).fold(0.0, f64::max);
        let groups = make_groups(&mesh, scale);
        Object { name: name.to_string(), mesh, motion, scale, material, groups, bound_center: Vec3::default(), bound_radius: 0.0, local_radius, k: 1.0, roughness: 0.5, metallic: 0.0 }
    }

    pub fn surface(mut self, roughness: f64, metallic: f64) -> Object {
        self.roughness = roughness;
        self.metallic = metallic;
        self
    }

    // 組の頂点をその時刻に囲む球。中心は無収縮位置、半径は静止時の半径に収縮ではみ出す量を足す
    pub fn group_sphere(&self, group: &Group, pose: &Pose) -> (Vec3, f64) {
        (pose.position + pose.rotation.rotate(group.center), group.radius + group.slack)
    }

    // relativity.slang の vertexPositionAtKeyframe と同じモデル。速度は差分でなく式の微分
    pub fn vertex_at_pose(&self, index: usize, pose: &Pose, c: f64) -> Vec3 {
        let local = self.mesh.vertices[index].position.hadamard(self.scale);
        let uncontracted = pose.rotation.rotate(local) + pose.position;
        let offset = uncontracted - pose.position;
        let w = pose.velocity + pose.angular_velocity.cross(offset);
        let speed = w.length();
        if speed < 1e-9 {
            return uncontracted;
        }
        let forward = w * (1.0 / speed);
        let beta = (speed / c).min(0.99999);
        let ratio = (1.0 - beta * beta).max(1e-12).sqrt();
        pose.position + offset + forward * ((ratio - 1.0) * forward.dot(offset))
    }

    #[allow(dead_code)]
    pub fn vertex_at(&self, index: usize, tau: f64, c: f64) -> Vec3 {
        self.vertex_at_pose(index, &self.motion.pose(tau), c)
    }

    pub fn vertices_at(&self, tau: f64, c: f64, out: &mut Vec<Vec3>) {
        let pose = self.motion.pose(tau);
        out.clear();
        out.extend((0..self.mesh.vertices.len()).map(|i| self.vertex_at_pose(i, &pose, c)));
    }

    // 頂点の無収縮位置は中心から local_radius 以内にあり、収縮は中心へ寄せるだけなので、
    // その時刻の中心を囲む球は面全体を覆う
    pub fn sphere_lower_bound(&self, x: Vec3, pose: &Pose) -> f64 {
        (x - pose.position).length() - self.local_radius
    }

    pub fn normal_at(&self, index: usize, tau: f64) -> Vec3 {
        self.motion.pose(tau).rotation.rotate(self.mesh.vertices[index].normal).normalize()
    }

    // 時刻範囲 [tau_min, tau_max] で、レイが飛び越えないための K と全時刻を覆う境界球を出す
    pub fn prepare(&mut self, c: f64, tau_min: f64, tau_max: f64) {
        const SAMPLES: usize = 4096;
        const H: f64 = 1e-5;
        let mut lo = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut hi = -lo;
        let mut max_speed: f64 = 0.0;
        let mut a = Vec::new();
        let mut b = Vec::new();
        let mut slack = vec![0.0f64; self.groups.len()];
        for s in 0..=SAMPLES {
            let tau = tau_min + (tau_max - tau_min) * s as f64 / SAMPLES as f64;
            let p = self.motion.pose(tau).position;
            lo = lo.min(p);
            hi = hi.max(p);
            self.vertices_at(tau, c, &mut a);
            self.vertices_at(tau + H, c, &mut b);
            for (pa, pb) in a.iter().zip(&b) {
                max_speed = max_speed.max((*pb - *pa).length() / H);
            }
            let pose = self.motion.pose(tau);
            for (gi, group) in self.groups.iter().enumerate() {
                let center = pose.position + pose.rotation.rotate(group.center);
                for &v in &group.vertices {
                    slack[gi] = slack[gi].max((a[v as usize] - center).length() - group.radius);
                }
            }
        }
        let sample_interval = (tau_max - tau_min) / SAMPLES as f64;
        self.bound_center = (lo + hi) * 0.5;
        self.bound_radius = (hi - lo).length() * 0.5 + self.local_radius + max_speed * sample_interval;
        self.k = 1.0 + 1.5 * max_speed / c;
        // サンプルの間の動きぶんを足す(中心も頂点も max_speed 以下でしか動かない)
        for (group, s) in self.groups.iter_mut().zip(slack) {
            group.slack = s.max(0.0) * 1.1 + 2.0 * max_speed * sample_interval;
        }
    }
}

// 三角形の重心を静止形状の格子に入れて組にする。1 組が十数個の三角形になる目安で格子幅を決める
fn make_groups(mesh: &Mesh, scale: Vec3) -> Vec<Group> {
    let positions: Vec<Vec3> = mesh.vertices.iter().map(|v| v.position.hadamard(scale)).collect();
    let triangle_count = mesh.indices.len() / 3;
    let target = 12.0;
    let cells = ((triangle_count as f64 / target).cbrt().ceil() as usize).max(1);
    let lo = positions.iter().fold(Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY), |a, &b| a.min(b));
    let hi = positions.iter().fold(-Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY), |a, &b| a.max(b));
    let size = hi - lo;
    let cell_of = |p: Vec3| -> usize {
        let axis = |v: f64, lo: f64, size: f64| -> usize {
            if size <= 0.0 { 0 } else { (((v - lo) / size * cells as f64) as usize).min(cells - 1) }
        };
        (axis(p.x, lo.x, size.x) * cells + axis(p.y, lo.y, size.y)) * cells + axis(p.z, lo.z, size.z)
    };
    let mut buckets: std::collections::BTreeMap<usize, Vec<u32>> = std::collections::BTreeMap::new();
    for t in 0..triangle_count {
        let a = positions[mesh.indices[t * 3] as usize];
        let b = positions[mesh.indices[t * 3 + 1] as usize];
        let c = positions[mesh.indices[t * 3 + 2] as usize];
        buckets.entry(cell_of((a + b + c) * (1.0 / 3.0))).or_default().push(t as u32);
    }
    let positions = &positions;
    buckets
        .into_values()
        .map(|triangles| {
            let verts: Vec<Vec3> = triangles
                .iter()
                .flat_map(|&t| (0..3).map(move |k| positions[mesh.indices[t as usize * 3 + k] as usize]))
                .collect();
            let center = verts.iter().fold(Vec3::default(), |a, &b| a + b) * (1.0 / verts.len() as f64);
            let radius = verts.iter().map(|&v| (v - center).length()).fold(0.0, f64::max);
            let mut vertices: Vec<u32> = triangles.iter().flat_map(|&t| (0..3).map(move |k| mesh.indices[t as usize * 3 + k])).collect();
            vertices.sort_unstable();
            vertices.dedup();
            Group { triangles, vertices, center, radius, slack: 0.0 }
        })
        .collect()
}

pub struct Scene {
    pub objects: Vec<Object>,
    pub c: f64,
    pub to_light: Vec3,
    pub ambient: f64,
    pub light_intensity: f64,
    pub pbr_enabled: bool,
    pub ibl_enabled: bool,
    pub ibl_intensity: f64,
    pub env: Option<EnvMaps>,
    pub shadow_enabled: bool,
}

impl Scene {
    pub fn from_prefs(prefs: &Prefs, snapshot: &Snapshot, refine_edge: f64) -> Scene {
        let mut objects = Vec::new();

        if prefs.cube_visible {
            objects.push(Object::new(
                "cube",
                mesh::cube(1),
                Motion::Sway { origin: prefs.cube_origin, amplitude: prefs.cube_animation_width, period: TAU },
                Vec3::new(1.0, 5.0, 1.0),
                Material::Uv,
            ).surface(prefs.cube_roughness, prefs.cube_metallic));
        }

        objects.push(Object::new(
            "plane",
            mesh::plane(1),
            Motion::Still { position: Vec3::new(0.0, -2.5, 0.0) },
            Vec3::new(100.0, 1.0, 100.0),
            Material::Checker,
        ).surface(prefs.plane_roughness, prefs.plane_metallic));

        if prefs.long_cube_visible {
            let size = prefs.long_cube_size;
            objects.push(Object::new(
                "longCube",
                mesh::cube(1),
                Motion::Sway {
                    origin: prefs.long_cube_origin + Vec3::new(0.0, 0.0, size.z * 0.5),
                    amplitude: prefs.long_cube_animation_width,
                    period: prefs.long_cube_animation_duration.abs().max(1e-4),
                },
                size,
                Material::Uv,
            ).surface(prefs.long_cube_roughness, prefs.long_cube_metallic));
        }

        if prefs.wheel_visible {
            // 回転する物体は面上で速度が変わり、頂点ごとの収縮を直線補間すると中ほどがずれる。
            // 静止形状の辺を短く割ってから縮める(本体の分割が割った点を個別に縮めるのと同じ極限に近づける)
            let wheel = mesh::wheel(
                    prefs.wheel_divisions,
                    prefs.wheel_sub_divisions,
                    prefs.wheel_radius,
                    prefs.wheel_radial_thickness,
                    prefs.wheel_thickness,
                    prefs.wheel_spoke_count,
                    prefs.wheel_spoke_size,
                    prefs.wheel_spoke_thickness,
                    prefs.wheel_hub_radius,
                    prefs.wheel_hub_thickness,
                )
                .refine(Vec3::new(1.0, 1.0, 1.0), refine_edge);
            objects.push(Object::new(
                "wheel",
                wheel,
                Motion::Wheel {
                    origin: prefs.wheel_origin,
                    radius: prefs.wheel_radius,
                    rotate_speed: prefs.wheel_rotate_speed.to_radians(),
                    half_range: prefs.wheel_move_distance.abs().max(1e-4),
                },
                Vec3::new(1.0, 1.0, 1.0),
                Material::Uv,
            ).surface(prefs.wheel_roughness, prefs.wheel_metallic));
        }

        if prefs.wave_cube_visible {
            let cube = mesh::cube(1);
            for i in 0..prefs.wave_cube_x_count {
                for j in 0..prefs.wave_cube_y_count {
                    for k in 0..prefs.wave_cube_z_count {
                        let space = prefs.wave_cube_space;
                        let origin = prefs.wave_cube_origin
                            + Vec3::new(i as f64 * space.x, j as f64 * space.y, k as f64 * space.z);
                        objects.push(Object::new(
                            &format!("wave[{},{},{}]", i, j, k),
                            cube.clone(),
                            Motion::Wave {
                                origin,
                                width: prefs.wave_cube_width,
                                speed: prefs.wave_cube_speed,
                                start_time: snapshot.wave_cube_start_time,
                            },
                            prefs.wave_cube_size,
                            Material::Uv,
                        ).surface(prefs.wave_cube_roughness, prefs.wave_cube_metallic));
                    }
                }
            }
        }

        Scene {
            objects,
            c: prefs.light_speed_effective(),
            to_light: prefs.light_direction.normalize(),
            ambient: prefs.shadow_ambient,
            light_intensity: prefs.light_intensity,
            pbr_enabled: prefs.pbr_enabled,
            ibl_enabled: prefs.ibl_enabled,
            ibl_intensity: prefs.ibl_intensity,
            env: None,
            shadow_enabled: prefs.shadow_enabled,
        }
    }

    pub fn prepare(&mut self, observer: Vec3, time: f64) {
        for o in &mut self.objects {
            o.prepare(self.c, time, time);
        }
        let far = self
            .objects
            .iter()
            .map(|o| (o.bound_center - observer).length() + o.bound_radius)
            .fold(0.0, f64::max);
        let diameter = self.objects.iter().map(|o| o.bound_radius * 2.0).fold(0.0, f64::max)
            + self
                .objects
                .iter()
                .flat_map(|a| self.objects.iter().map(move |b| (a.bound_center - b.bound_center).length()))
                .fold(0.0, f64::max);
        let tau_min = time - (far + diameter) * 1.1 / self.c;
        for o in &mut self.objects {
            o.prepare(self.c, tau_min, time);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn still_box(position: Vec3, scale: Vec3) -> Object {
        Object::new("box", mesh::cube(1), Motion::Still { position }, scale, Material::Uv)
    }

    #[test]
    fn still_vertex_is_scaled_and_moved() {
        let o = still_box(Vec3::new(1.0, 2.0, 3.0), Vec3::new(2.0, 4.0, 6.0));
        for i in 0..o.mesh.vertices.len() {
            let expect = o.mesh.vertices[i].position.hadamard(o.scale) + Vec3::new(1.0, 2.0, 3.0);
            assert!((o.vertex_at(i, 5.0, 8.0) - expect).length() < 1e-12);
        }
    }

    #[test]
    fn sway_velocity_matches_finite_difference() {
        let m = Motion::Sway { origin: Vec3::default(), amplitude: 3.0, period: TAU };
        for &t in &[0.0, 0.7, 2.9] {
            let h = 1e-6;
            let fd = (m.pose(t + h).position - m.pose(t - h).position) * (1.0 / (2.0 * h));
            assert!((fd - m.pose(t).velocity).length() < 1e-6);
        }
    }

    #[test]
    fn linear_motion_contracts_along_velocity() {
        let o = Object::new(
            "box",
            mesh::cube(1),
            Motion::Linear { position0: Vec3::default(), velocity: Vec3::new(4.0, 0.0, 0.0) },
            Vec3::new(1.0, 1.0, 1.0),
            Material::Uv,
        );
        let c = 8.0;
        let front = o.mesh.vertices.iter().position(|v| v.position == Vec3::new(0.5, 0.5, 0.5)).unwrap();
        let back = o.mesh.vertices.iter().position(|v| v.position == Vec3::new(-0.5, 0.5, 0.5)).unwrap();
        let d = o.vertex_at(front, 1.0, c) - o.vertex_at(back, 1.0, c);
        assert!((d.x - (1.0f64 - 0.25).sqrt()).abs() < 1e-12);
        assert!(d.y.abs() < 1e-12 && d.z.abs() < 1e-12);
        assert!((o.vertex_at(front, 1.0, c).x - (4.0 + 0.5 * (0.75f64).sqrt())).abs() < 1e-12);
    }

    #[test]
    fn wheel_offset_is_triangle_wave() {
        let m = Motion::Wheel { origin: Vec3::default(), radius: 2.0, rotate_speed: 1.0, half_range: 3.0 };
        let speed = 2.0;
        let at = |travel: f64| m.pose(travel / speed);
        assert!((at(0.0).position.x - 0.0).abs() < 1e-12);
        assert!((at(3.0).position.x - 3.0).abs() < 1e-12);
        assert!((at(6.0).position.x - 0.0).abs() < 1e-12);
        assert!((at(9.0).position.x + 3.0).abs() < 1e-12);
        assert!((at(12.0).position.x - 0.0).abs() < 1e-12);
        assert!((at(1.0).velocity.x - speed).abs() < 1e-12);
        assert!((at(5.0).velocity.x + speed).abs() < 1e-12);
        let p = at(1.0);
        let rim = p.rotation.rotate(Vec3::new(0.0, 2.0, 0.0));
        let angle = -1.0 / 2.0f64;
        assert!((rim - Vec3::new(-2.0 * angle.sin(), 2.0 * angle.cos(), 0.0)).length() < 1e-12);
        let w = p.angular_velocity;
        assert!((w.z + speed / 2.0).abs() < 1e-12);
    }

    #[test]
    fn wave_is_zero_outside_and_peaks_at_width() {
        let m = Motion::Wave { origin: Vec3::default(), width: 2.0, speed: 8.0, start_time: 10.0 };
        assert_eq!(m.pose(9.0).position.y, 0.0);
        let duration = PI * 2.0 / 8.0;
        assert!((m.pose(10.0 + duration * 0.5).position.y - 2.0).abs() < 1e-12);
        assert_eq!(m.pose(10.0 + duration + 0.01).position.y, 0.0);
    }

    #[test]
    fn groups_cover_every_triangle_once() {
        let o = Object::new(
            "wheel",
            mesh::wheel(64, 1, 4.0, 0.3, 0.2, 10, 0.1, 0.1, 0.3, 0.3),
            Motion::Still { position: Vec3::default() },
            Vec3::new(1.0, 1.0, 1.0),
            Material::Uv,
        );
        let mut seen = vec![0; o.mesh.triangle_count()];
        for g in &o.groups {
            for &t in &g.triangles {
                seen[t as usize] += 1;
                for k in 0..3 {
                    let v = o.mesh.vertices[o.mesh.indices[t as usize * 3 + k] as usize].position;
                    assert!((v - g.center).length() <= g.radius + 1e-12);
                }
            }
        }
        assert!(seen.iter().all(|&n| n == 1));
        assert!(o.groups.len() > 20, "{}", o.groups.len());
    }

    #[test]
    fn group_sphere_contains_contracted_vertices() {
        let mut o = Object::new(
            "wheel",
            mesh::wheel(64, 1, 4.0, 0.3, 0.2, 10, 0.1, 0.1, 0.3, 0.3),
            Motion::Wheel { origin: Vec3::new(0.0, 1.5, -10.0), radius: 4.0, rotate_speed: 1.0, half_range: 20.0 },
            Vec3::new(1.0, 1.0, 1.0),
            Material::Uv,
        );
        let c = 8.0;
        o.prepare(c, 0.0, 20.0);
        for &tau in &[0.3, 7.4, 13.9, 15.123] {
            let pose = o.motion.pose(tau);
            for g in &o.groups {
                let (center, radius) = o.group_sphere(g, &pose);
                for &t in &g.triangles {
                    for k in 0..3 {
                        let v = o.vertex_at_pose(o.mesh.indices[t as usize * 3 + k] as usize, &pose, c);
                        assert!((v - center).length() <= radius + 1e-9);
                    }
                }
            }
        }
    }

    #[test]
    fn prepare_bounds_cover_motion() {
        let mut o = Object::new(
            "box",
            mesh::cube(1),
            Motion::Sway { origin: Vec3::new(0.0, 0.0, 15.0), amplitude: 3.0, period: TAU },
            Vec3::new(1.0, 5.0, 1.0),
            Material::Uv,
        );
        o.prepare(8.0, 0.0, 20.0);
        assert!((o.bound_center - Vec3::new(0.0, 0.0, 15.0)).length() < 1e-2);
        assert!(o.bound_radius >= 3.0 + Vec3::new(0.5, 2.5, 0.5).length());
        assert!(o.k >= 1.0 + 1.5 * 3.0 / 8.0, "{}", o.k);
        assert!(o.k < 1.0 + 1.5 * 3.2 / 8.0, "{}", o.k);
    }
}
