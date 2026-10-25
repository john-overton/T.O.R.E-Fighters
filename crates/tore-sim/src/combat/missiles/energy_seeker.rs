//! Fitted seeker slew and terrain-Doppler rejection for the energy profile.
use crate::attitude::{Vector, cross, dot, unit};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tracking {
    pub head: Option<Vector>,
    pub notch_ticks: u32,
    pub clear_ticks: u32,
    pub notched: bool,
}
pub fn slew(from: Vector, to: Vector, step: f64) -> Vector {
    let cosine = dot(from, to).clamp(-1., 1.);
    let angle = cosine.acos();
    if angle <= step {
        return to;
    }
    let mut perpendicular = std::array::from_fn(|i| to[i] - cosine * from[i]);
    if dot(perpendicular, perpendicular) < 1e-16 {
        perpendicular = cross(
            from,
            if from[1].abs() < 0.9 {
                [0., 1., 0.]
            } else {
                [1., 0., 0.]
            },
        );
    }
    let direction = unit(perpendicular);
    let (sin, cos) = step.sin_cos();
    unit(std::array::from_fn(|i| from[i] * cos + direction[i] * sin))
}
impl Tracking {
    /// `line` is the potential sensor return. Clutter-rejected returns do not
    /// move the head; reacquisition requires the return to re-enter its view.
    pub fn observes(
        &mut self,
        nose: Vector,
        line: Vector,
        notch: f64,
        rate: f64,
        fov: f64,
        gimbal: f64,
    ) -> bool {
        let head = self.head.unwrap_or(nose);
        let head = if dot(head, nose) < gimbal.cos() {
            slew(nose, head, gimbal)
        } else {
            head
        };
        let head = if self.notched {
            head
        } else {
            slew(head, line, rate / 120.)
        };
        self.head = Some(head);
        let visible = dot(head, line) >= fov.cos();
        if visible && notch >= 0.75 {
            self.notch_ticks = self.notch_ticks.saturating_add(1);
            self.clear_ticks = 0;
            if self.notch_ticks >= 24 {
                self.notched = true;
            }
        } else if visible && notch < 0.5 {
            self.notch_ticks = 0;
            self.clear_ticks = self.clear_ticks.saturating_add(1);
            if self.clear_ticks >= 30 {
                self.notched = false;
            }
        } else {
            self.notch_ticks = 0;
            self.clear_ticks = 0;
        }
        visible && !self.notched
    }
}
crate::checkpoint_struct!(Tracking {
    head,
    notch_ticks,
    clear_ticks,
    notched
});

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn angular_rate_and_notch_dwell_are_independent() {
        let mut t = Tracking::default();
        let nose = [0., 0., 1.];
        let side = [1., 0., 0.];
        assert!(!t.observes(
            nose,
            side,
            0.,
            30f64.to_radians(),
            5f64.to_radians(),
            100f64.to_radians()
        ));
        assert!((dot(t.head.unwrap(), nose).acos().to_degrees() - 0.25).abs() < 1e-8);
        let mut t = Tracking::default();
        for _ in 0..23 {
            assert!(t.observes(nose, nose, 1., 1., 0.1, 1.));
        }
        assert!(!t.observes(nose, nose, 1., 1., 0.1, 1.));
        for _ in 0..29 {
            assert!(!t.observes(nose, nose, 0., 1., 0.1, 1.));
        }
        assert!(t.observes(nose, nose, 0., 1., 0.1, 1.));
    }
}
