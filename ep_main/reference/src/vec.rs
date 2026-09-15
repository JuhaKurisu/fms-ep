use std::ops::{Add, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    pub fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }

    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn normalize(self) -> Vec3 {
        let l = self.length();
        if l == 0.0 { self } else { self * (1.0 / l) }
    }

    pub fn hadamard(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }

    pub fn min(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }

    pub fn max(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

// quaternion.pde と同じ規約。xyz が虚部、w が実部、mul(a, b) は b してから a
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Quat {
    pub const fn new(x: f64, y: f64, z: f64, w: f64) -> Quat {
        Quat { x, y, z, w }
    }

    pub const fn identity() -> Quat {
        Quat::new(0.0, 0.0, 0.0, 1.0)
    }

    pub fn from_axis_angle(axis: Vec3, angle: f64) -> Quat {
        let (s, c) = (angle * 0.5).sin_cos();
        Quat::new(axis.x * s, axis.y * s, axis.z * s, c)
    }

    #[allow(dead_code)]
    pub fn mul(self, b: Quat) -> Quat {
        let a = self;
        Quat::new(
            a.w * b.x + b.w * a.x + (a.y * b.z - a.z * b.y),
            a.w * b.y + b.w * a.y + (a.z * b.x - a.x * b.z),
            a.w * b.z + b.w * a.z + (a.x * b.y - a.y * b.x),
            a.w * b.w - (a.x * b.x + a.y * b.y + a.z * b.z),
        )
    }

    #[allow(dead_code)]
    pub fn conjugate(self) -> Quat {
        Quat::new(-self.x, -self.y, -self.z, self.w)
    }

    pub fn rotate(self, v: Vec3) -> Vec3 {
        let q = Vec3::new(self.x, self.y, self.z);
        let t = q.cross(v) * 2.0;
        v + t * self.w + q.cross(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_90_about_y_maps_z_to_x() {
        let q = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), std::f64::consts::FRAC_PI_2);
        let v = q.rotate(Vec3::new(0.0, 0.0, 1.0));
        assert!((v - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-12);
    }

    #[test]
    fn mul_applies_right_then_left() {
        let a = Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), 0.3);
        let b = Quat::from_axis_angle(Vec3::new(1.0, 0.0, 0.0), 0.7);
        let v = Vec3::new(0.2, -0.5, 0.9);
        let lhs = a.mul(b).rotate(v);
        let rhs = a.rotate(b.rotate(v));
        assert!((lhs - rhs).length() < 1e-12);
    }

    #[test]
    fn conjugate_undoes_rotation() {
        let q = Quat::from_axis_angle(Vec3::new(0.6, 0.0, 0.8), 1.1);
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert!((q.conjugate().rotate(q.rotate(v)) - v).length() < 1e-12);
    }
}
