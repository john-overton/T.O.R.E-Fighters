//! Orthonormal attitude math; no Euler-rate singularity at vertical flight.
pub type Vector = [f64; 3];
pub fn dot(a: Vector, b: Vector) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
pub fn cross(a: Vector, b: Vector) -> Vector {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn unit(v: Vector) -> Vector {
    let length = dot(v, v).sqrt().max(1e-12);
    v.map(|x| x / length)
}
#[derive(Clone, Copy)]
pub struct Basis {
    pub right: Vector,
    pub up: Vector,
    pub forward: Vector,
}
impl Basis {
    pub fn new(yaw: f64, pitch: f64, bank: f64) -> Self {
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        let (sb, cb) = bank.sin_cos();
        Self {
            right: [cy * cb + sy * sp * sb, -cp * sb, -sy * cb + cy * sp * sb],
            up: [cy * sb - sy * sp * cb, cp * cb, -sy * sb - cy * sp * cb],
            forward: [sy * cp, sp, cy * cp],
        }
    }
    fn orthogonal(right: Vector, forward: Vector) -> Self {
        let forward = unit(forward);
        let up = unit(cross(forward, right));
        let right = unit(cross(up, forward));
        Self { right, up, forward }
    }
    pub fn rotated(self, rotation: Vector) -> Self {
        let angle = dot(rotation, rotation).sqrt();
        if angle < 1e-14 {
            return self;
        }
        let axis = rotation.map(|x| x / angle);
        let (sin, cos) = angle.sin_cos();
        let rotate = |v: Vector| {
            let c = cross(axis, v);
            let d = dot(axis, v);
            std::array::from_fn(|i| v[i] * cos + c[i] * sin + axis[i] * d * (1. - cos))
        };
        Self::orthogonal(rotate(self.right), rotate(self.forward))
    }
    pub fn blended(self, next: Self, alpha: f64) -> Self {
        Self::orthogonal(
            std::array::from_fn(|i| self.right[i] + (next.right[i] - self.right[i]) * alpha),
            std::array::from_fn(|i| self.forward[i] + (next.forward[i] - self.forward[i]) * alpha),
        )
    }
    pub fn angles(self) -> [f64; 3] {
        let yaw = self.forward[0].atan2(self.forward[2]);
        let pitch = self.forward[1].atan2(self.forward[0].hypot(self.forward[2]));
        let level = Self::new(yaw, pitch, 0.);
        let bank = dot(self.up, level.right).atan2(dot(self.up, level.up));
        [yaw.rem_euclid(std::f64::consts::TAU), pitch, bank]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_loop_and_interpolation_cross_both_verticals() {
        let start = Basis::new(0.3, 0., 0.);
        let mut b = start;
        let mut inverted = false;
        for _ in 0..720 {
            let next = b.rotated(b.right.map(|v| -v * std::f64::consts::TAU / 720.));
            let mid = b.blended(next, 0.5);
            assert!(dot(mid.forward, b.forward) > 0.999);
            let [y, p, r] = next.angles();
            let restored = Basis::new(y, p, r);
            assert!(dot(restored.forward, next.forward) > 0.999999);
            assert!(dot(restored.up, next.up) > 0.999999);
            inverted |= next.up[1] < -0.99;
            b = restored;
        }
        assert!(inverted);
        assert!(dot(b.forward, start.forward) > 0.999999);
        assert!(dot(b.up, start.up) > 0.999999);
    }
}
