use crate::vec::Vec3;
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

// ep_main.pde の Prefs* と同じキー・既定値
pub struct Prefs {
    pub light_speed: f64,
    pub light_speed_infinite: bool,
    pub light_direction: Vec3,
    pub shadow_enabled: bool,
    pub shadow_ambient: f64,
    pub light_intensity: f64,
    pub pbr_enabled: bool,
    pub plane_roughness: f64,
    pub plane_metallic: f64,
    pub cube_roughness: f64,
    pub cube_metallic: f64,
    pub long_cube_roughness: f64,
    pub long_cube_metallic: f64,
    pub wheel_roughness: f64,
    pub wheel_metallic: f64,
    pub wave_cube_roughness: f64,
    pub wave_cube_metallic: f64,
    pub camera_fov: f64,
    pub camera_velocity_zero: bool,
    pub cube_visible: bool,
    pub cube_origin: Vec3,
    pub cube_animation_width: f64,
    pub long_cube_visible: bool,
    pub long_cube_origin: Vec3,
    pub long_cube_size: Vec3,
    pub long_cube_animation_width: f64,
    pub long_cube_animation_duration: f64,
    pub wheel_visible: bool,
    pub wheel_origin: Vec3,
    pub wheel_radius: f64,
    pub wheel_rotate_speed: f64,
    pub wheel_move_distance: f64,
    pub wheel_divisions: usize,
    pub wheel_sub_divisions: usize,
    pub wheel_spoke_count: usize,
    pub wheel_spoke_size: f64,
    pub wheel_spoke_thickness: f64,
    pub wheel_thickness: f64,
    pub wheel_radial_thickness: f64,
    pub wheel_hub_radius: f64,
    pub wheel_hub_thickness: f64,
    pub wave_cube_visible: bool,
    pub wave_cube_origin: Vec3,
    pub wave_cube_x_count: usize,
    pub wave_cube_y_count: usize,
    pub wave_cube_z_count: usize,
    pub wave_cube_space: Vec3,
    pub wave_cube_size: Vec3,
    pub wave_cube_speed: f64,
    pub wave_cube_width: f64,
    pub ibl_enabled: bool,
    pub ibl_intensity: f64,
    pub tonemap_enabled: bool,
    pub tonemap_lut: usize,
    pub exposure: f64,
}

fn f(json: &Value, key: &str, default: f64) -> f64 {
    json.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn b(json: &Value, key: &str, default: bool) -> bool {
    json.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn i(json: &Value, key: &str, default: i64) -> usize {
    json.get(key).and_then(Value::as_i64).unwrap_or(default).max(0) as usize
}

fn v(json: &Value, key: &str, default: Vec3) -> Vec3 {
    match json.get(key).and_then(Value::as_array) {
        Some(a) if a.len() == 3 => {
            let g = |k: usize| a[k].as_f64().unwrap_or(0.0);
            Vec3::new(g(0), g(1), g(2))
        }
        _ => default,
    }
}

impl Prefs {
    pub fn load(path: &Path) -> Result<Prefs, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let json: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))?;
        Ok(Prefs::from_json(&json))
    }

    pub fn from_json(j: &Value) -> Prefs {
        Prefs {
            light_speed: f(j, "LightSpeed", 8.0),
            light_speed_infinite: b(j, "LightSpeedInfinite", false),
            light_direction: v(j, "LightDirection", Vec3::new(0.4, 0.8, -0.6)),
            shadow_enabled: b(j, "ShadowEnabled", true),
            shadow_ambient: f(j, "ShadowAmbient", 0.2),
            light_intensity: f(j, "LightIntensity", 1.0),
            pbr_enabled: b(j, "PbrEnabled", true),
            plane_roughness: f(j, "PlaneRoughness", 0.5),
            plane_metallic: f(j, "PlaneMetallic", 0.0),
            cube_roughness: f(j, "CubeRoughness", 0.5),
            cube_metallic: f(j, "CubeMetallic", 0.0),
            long_cube_roughness: f(j, "LongCubeRoughness", 0.5),
            long_cube_metallic: f(j, "LongCubeMetallic", 0.0),
            wheel_roughness: f(j, "WheelRoughness", 0.5),
            wheel_metallic: f(j, "WheelMetallic", 0.0),
            wave_cube_roughness: f(j, "WaveCubeRoughness", 0.5),
            wave_cube_metallic: f(j, "WaveCubeMetallic", 0.0),
            camera_fov: f(j, "CameraFOV", 60.0),
            camera_velocity_zero: b(j, "CameraVelocityZero", false),
            cube_visible: b(j, "CubeVisible", true),
            cube_origin: v(j, "CubeOrigin", Vec3::new(0.0, 0.0, 15.0)),
            cube_animation_width: f(j, "CubeAnimationWidth", 6.0),
            long_cube_visible: b(j, "LongCubeVisible", true),
            long_cube_origin: v(j, "LongCubeOrigin", Vec3::new(-4.0, 0.0, 15.0)),
            long_cube_size: v(j, "LongCubeSize", Vec3::new(1.0, 1.0, 10.0)),
            long_cube_animation_width: f(j, "LongCubeAnimationWidth", 1.0),
            long_cube_animation_duration: f(j, "LongCubeAnimationDuration", 0.5),
            wheel_visible: b(j, "WheelVisible", true),
            wheel_origin: v(j, "WheelOrigin", Vec3::new(6.0, -0.5, 15.0)),
            wheel_radius: f(j, "WheelRadius", 2.0),
            wheel_rotate_speed: f(j, "WheelRotateSpeed", 120.0),
            wheel_move_distance: f(j, "WheelMoveDistance", 4.0),
            wheel_divisions: i(j, "WheelDivisions", 32).max(1),
            wheel_sub_divisions: i(j, "WheelSubDivisions", 16).max(1),
            wheel_spoke_count: i(j, "WheelSpokeCount", 8),
            wheel_spoke_size: f(j, "WheelSpokeSize", 0.1),
            wheel_spoke_thickness: f(j, "WheelSpokeThickness", 0.1),
            wheel_thickness: f(j, "WheelThickness", 0.1),
            wheel_radial_thickness: f(j, "WheelRadialThickness", 0.3),
            wheel_hub_radius: f(j, "WheelHubRadius", 0.3),
            wheel_hub_thickness: f(j, "WheelHubThickness", 0.1),
            wave_cube_visible: b(j, "WaveCubeVisible", true),
            wave_cube_origin: v(j, "WaveCubeOrigin", Vec3::new(0.0, 0.0, 0.0)),
            wave_cube_x_count: i(j, "WaveCubeXCount", 5),
            wave_cube_y_count: i(j, "WaveCubeYCount", 5),
            wave_cube_z_count: i(j, "WaveCubeZCount", 5),
            wave_cube_space: v(j, "WaveCubeSpace", Vec3::new(1.0, 1.0, 1.0)),
            wave_cube_size: v(j, "WaveCubeSize", Vec3::new(1.0, 1.0, 1.0)),
            wave_cube_speed: f(j, "WaveCubeSpeed", 1.0),
            wave_cube_width: f(j, "WaveCubeWidth", 1.0),
            ibl_enabled: b(j, "IblEnabled", true),
            ibl_intensity: f(j, "IblIntensity", 1.0),
            tonemap_enabled: b(j, "TonemapEnabled", true),
            tonemap_lut: i(j, "TonemapLut", 0),
            exposure: f(j, "Exposure", 1.0),
        }
    }

    pub fn light_speed_effective(&self) -> f64 {
        if self.light_speed_infinite { 1e6 } else { self.light_speed }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    #[allow(dead_code)]
    pub scene_frame: i64,
    pub scene_time: f64,
    pub camera_position: [f64; 3],
    pub camera_velocity: [f64; 3],
    pub camera_rotation: [f64; 4],
    pub wave_cube_start_time: f64,
    pub view_width: u32,
    pub view_height: u32,
    #[serde(default)]
    pub render_camera: Option<RenderCamera>,
    #[serde(default)]
    pub thrown_spheres: Vec<serde_json::Value>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RenderCamera {
    pub position: [f64; 3],
    pub rotation: [f64; 4],
}

impl Snapshot {
    pub fn load(path: &Path) -> Result<Snapshot, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_json_gives_ep_main_defaults() {
        let p = Prefs::from_json(&serde_json::json!({}));
        assert_eq!(p.cube_origin, Vec3::new(0.0, 0.0, 15.0));
        assert_eq!(p.wheel_divisions, 32);
        assert_eq!(p.light_direction, Vec3::new(0.4, 0.8, -0.6));
        assert_eq!(p.light_speed_effective(), 8.0);
    }

    #[test]
    fn infinite_light_speed_is_1e6() {
        let p = Prefs::from_json(&serde_json::json!({"LightSpeedInfinite": true, "LightSpeed": 3.0}));
        assert_eq!(p.light_speed_effective(), 1e6);
    }

    #[test]
    fn vec3_key_is_read() {
        let p = Prefs::from_json(&serde_json::json!({"WaveCubeSize": [2, 0.5, 2]}));
        assert_eq!(p.wave_cube_size, Vec3::new(2.0, 0.5, 2.0));
    }
}
