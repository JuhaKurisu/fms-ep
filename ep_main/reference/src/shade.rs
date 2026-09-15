use crate::ibl::{env_brdf_approx, EnvMaps};
use crate::scene::{Material, Scene};
use crate::trace::{Hit, Tracer};
use crate::vec::Vec3;
use std::f64::consts::PI;

// relativity.slang の aberrateDirection。beta は観測者速度 / c
pub fn aberrate(direction_to_source: Vec3, beta: Vec3) -> Vec3 {
    let v2 = beta.dot(beta);
    if v2 < 1e-9 {
        return direction_to_source;
    }
    let gamma = 1.0 / (1.0 - v2).max(1e-8).sqrt();
    let beta_dir = beta * (1.0 / v2.sqrt());
    let propagation = -direction_to_source;
    let boosted = propagation + beta_dir * ((gamma - 1.0) * beta_dir.dot(propagation)) - beta * gamma;
    -boosted.normalize()
}

// 強さ 1 の平行光が面から視線へ返す色(cos 項込み)。Cook-Torrance(GGX)、relativity.slang の reflectLight と同じ式
pub fn reflect(n: Vec3, l: Vec3, v: Vec3, albedo: [f64; 3], roughness: f64, metallic: f64) -> [f64; 3] {
    let nl = n.dot(l).max(0.0);
    if nl <= 0.0 {
        return [0.0; 3];
    }
    let nv = n.dot(v).max(1e-4);
    let h = (l + v).normalize();
    let nh = n.dot(h).max(0.0);
    let vh = v.dot(h).max(0.0);
    let r = roughness.max(0.045);
    let a2 = r * r * r * r;
    let dd = nh * nh * (a2 - 1.0) + 1.0;
    let d = a2 / (PI * dd * dd);
    let k = (r + 1.0) * (r + 1.0) / 8.0;
    let g = (nl / (nl * (1.0 - k) + k)) * (nv / (nv * (1.0 - k) + k));
    let fresnel_weight = (1.0 - vh).powi(5);
    let mut out = [0.0; 3];
    for i in 0..3 {
        let f0 = 0.04 + (albedo[i] - 0.04) * metallic;
        let f = f0 + (1.0 - f0) * fresnel_weight;
        let spec = d * g * f / (4.0 * nl * nv);
        let kd = (1.0 - f) * (1.0 - metallic);
        out[i] = (kd * albedo[i] + PI * spec) * nl;
    }
    out
}

// 強さ 1 の平行光が返す色。pbr が偽なら以前の拡散のみ(albedo cos)
pub fn direct_light(n: Vec3, l: Vec3, v: Vec3, albedo: [f64; 3], roughness: f64, metallic: f64, pbr: bool) -> [f64; 3] {
    if pbr {
        return reflect(n, l, v, albedo, roughness, metallic);
    }
    let nl = n.dot(l).max(0.0);
    [albedo[0] * nl, albedo[1] * nl, albedo[2] * nl]
}

// 環境マップからの光。方向は実験室系
pub fn ambient_light(env: &EnvMaps, n: Vec3, v: Vec3, albedo: [f64; 3], roughness: f64, metallic: f64, pbr: bool) -> [f64; 3] {
    let irr = env.irradiance(n);
    if !pbr {
        return [irr[0] * albedo[0], irr[1] * albedo[1], irr[2] * albedo[2]];
    }
    let n_v = n.dot(v).max(1e-4);
    let r = n * (2.0 * n.dot(v)) - v;
    let spec = env.specular(r, roughness);
    let (a, b) = env_brdf_approx(roughness, n_v);
    let fw = (1.0 - n_v).powi(5);
    let mut out = [0.0; 3];
    for i in 0..3 {
        let f0 = 0.04 + (albedo[i] - 0.04) * metallic;
        let fr = f0 + ((1.0 - roughness).max(f0) - f0) * fw;
        let kd = (1.0 - fr) * (1.0 - metallic);
        out[i] = irr[i] * albedo[i] * kd + spec[i] * (f0 * a + b);
    }
    out
}

pub fn shade(scene: &Scene, tracer: &mut Tracer, hit: &Hit, observer: Vec3, max_steps: usize) -> [f64; 3] {
    let object = &scene.objects[hit.object];
    let idx = |k: usize| object.mesh.indices[hit.triangle * 3 + k] as usize;
    let (b0, b1, b2) = hit.bary;
    let n = (object.normal_at(idx(0), hit.tau) * b0
        + object.normal_at(idx(1), hit.tau) * b1
        + object.normal_at(idx(2), hit.tau) * b2)
        .normalize();
    let albedo = match object.material {
        Material::Uv => {
            let vs = &object.mesh.vertices;
            let u = vs[idx(0)].u * b0 + vs[idx(1)].u * b1 + vs[idx(2)].u * b2;
            let v = vs[idx(0)].v * b0 + vs[idx(1)].v * b1 + vs[idx(2)].v * b2;
            [u, v, 0.0]
        }
        Material::Checker => {
            let check = (hit.point.x.floor() as i64 + hit.point.z.floor() as i64) % 2 == 0;
            if check { [1.0; 3] } else { [0.0; 3] }
        }
    };
    let s = if n.dot(scene.to_light) > 0.0 { tracer.shadow(scene, hit, max_steps) } else { 1.0 };
    let view = (observer - hit.point).normalize();
    let direct = direct_light(n, scene.to_light, view, albedo, object.roughness, object.metallic, scene.pbr_enabled);
    let mut out = [0.0; 3];
    match &scene.env {
        Some(env) if scene.ibl_enabled => {
            let ibl = ambient_light(env, n, view, albedo, object.roughness, object.metallic, scene.pbr_enabled);
            for i in 0..3 {
                out[i] = direct[i] * scene.light_intensity * s + ibl[i] * scene.ibl_intensity;
            }
        }
        _ => {
            let scale = (1.0 - scene.ambient) * scene.light_intensity * s;
            for i in 0..3 {
                out[i] = albedo[i] * scene.ambient + direct[i] * scale;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ibl::{EnvMaps, Equirect};

    #[test]
    fn ambient_of_uniform_environment_is_albedo_times_radiance_for_rough_dielectric() {
        let env = EnvMaps::build(Equirect::uniform(64, 32, [2.0; 3]));
        let n = Vec3::new(0.0, 1.0, 0.0);
        let out = ambient_light(&env, n, n, [0.5, 0.25, 1.0], 1.0, 0.0, true);
        // 拡散 2 * albedo * kd(≈0.96) + 鏡面 2 * (0.04 A + B)。合計は albedo * 2 の前後
        assert!(out[0] > 0.9 && out[0] < 1.15, "{:?}", out);
        assert!(out[2] > 1.8 && out[2] < 2.2, "{:?}", out);
    }

    #[test]
    fn ambient_without_pbr_is_irradiance_times_albedo() {
        let env = EnvMaps::build(Equirect::uniform(64, 32, [2.0; 3]));
        let n = Vec3::new(0.0, 1.0, 0.0);
        let out = ambient_light(&env, n, n, [0.5, 0.25, 1.0], 0.3, 1.0, false);
        assert!((out[0] - 1.0).abs() < 0.05 && (out[1] - 0.5).abs() < 0.05, "{:?}", out);
    }

    #[test]
    fn aberrate_inverse_restores_direction() {
        let beta = Vec3::new(0.3, 0.1, -0.2);
        for d in [Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 2.0, 3.0).normalize(), Vec3::new(-0.5, 0.2, -0.1).normalize()] {
            let back = aberrate(aberrate(d, beta), -beta);
            assert!((back - d).length() < 1e-12);
        }
    }

    #[test]
    fn aberrate_squeezes_toward_motion() {
        let d = Vec3::new(1.0, 0.0, 0.0);
        let out = aberrate(d, Vec3::new(0.0, 0.0, 0.5));
        assert!(out.z > 0.0);
    }

    // 粗さ 1・非金属で真上から照らして真上から見ると、拡散 0.96 albedo + 鏡面 0.01(F0 = 0.04、D = 1/π、G = 1)
    #[test]
    fn rough_dielectric_at_normal_incidence_is_near_lambert() {
        let n = Vec3::new(0.0, 1.0, 0.0);
        let out = reflect(n, n, n, [0.5, 0.25, 1.0], 1.0, 0.0);
        for (o, a) in out.iter().zip([0.5, 0.25, 1.0]) {
            assert!((o - (0.96 * a + 0.01)).abs() < 1e-9, "{} vs {}", o, a);
        }
    }

    #[test]
    fn reflect_is_reciprocal() {
        let n = Vec3::new(0.0, 1.0, 0.0);
        let l = Vec3::new(0.3, 0.8, -0.2).normalize();
        let v = Vec3::new(-0.6, 0.5, 0.4).normalize();
        let a = reflect(n, l, v, [0.7, 0.4, 0.2], 0.3, 0.5);
        let b = reflect(n, v, l, [0.7, 0.4, 0.2], 0.3, 0.5);
        for i in 0..3 {
            assert!((a[i] / n.dot(l) - b[i] / n.dot(v)).abs() < 1e-9);
        }
    }

    #[test]
    fn smooth_metal_peaks_at_mirror_direction() {
        let n = Vec3::new(0.0, 1.0, 0.0);
        let l = Vec3::new(0.5, 0.7, 0.0).normalize();
        let mirror = Vec3::new(-0.5, 0.7, 0.0).normalize();
        let off = Vec3::new(-0.5, 0.7, 0.4).normalize();
        let peak = reflect(n, l, mirror, [1.0; 3], 0.1, 1.0)[0];
        let side = reflect(n, l, off, [1.0; 3], 0.1, 1.0)[0];
        assert!(peak > 10.0 * side, "{} vs {}", peak, side);
    }

    #[test]
    fn pbr_off_is_plain_diffuse() {
        let n = Vec3::new(0.0, 1.0, 0.0);
        let l = Vec3::new(0.6, 0.8, 0.0);
        let v = Vec3::new(-0.6, 0.8, 0.0);
        let out = direct_light(n, l, v, [0.5, 1.0, 0.25], 0.1, 1.0, false);
        assert_eq!(out, [0.4, 0.8, 0.2]);
    }

    #[test]
    fn light_below_surface_gives_nothing() {
        let n = Vec3::new(0.0, 1.0, 0.0);
        let out = reflect(n, Vec3::new(0.0, -1.0, 0.0), n, [1.0; 3], 0.5, 0.0);
        assert_eq!(out, [0.0; 3]);
    }
}
