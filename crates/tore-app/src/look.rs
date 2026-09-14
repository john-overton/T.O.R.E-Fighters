//! Authored keyboard head-look and exterior orbit, independent of flight dynamics.
use crate::terrain::Camera;
use std::{
    collections::BTreeSet,
    f32::consts::{PI, TAU},
};
use winit::keyboard::ModifiersState;

fn arrow(name: &str) -> bool {
    matches!(name, "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight")
}

pub fn press(keys: &mut BTreeSet<String>, name: &str, modifiers: ModifiersState) {
    // A look arrow remains claimed until its physical release, including key repeats.
    if keys.contains(&format!("Look{name}")) {
        return;
    }
    if modifiers.alt_key() || modifiers.super_key() {
        return;
    }
    if arrow(name) && (modifiers.shift_key() || modifiers.control_key()) {
        keys.remove(name);
        keys.insert(format!("Look{name}"));
    } else if !modifiers.control_key()
        && matches!(
            name,
            "ArrowUp"
                | "ArrowDown"
                | "ArrowLeft"
                | "ArrowRight"
                | "z"
                | "x"
                | "PageUp"
                | "PageDown"
        )
    {
        keys.insert(name.into());
    }
}

pub fn modifiers_changed(keys: &mut BTreeSet<String>, modifiers: ModifiersState) {
    let previous = std::mem::take(keys);
    for key in previous {
        if key.starts_with("LookArrow") {
            keys.insert(key);
        } else if arrow(&key) && (modifiers.shift_key() || modifiers.control_key()) {
            press(keys, &key, modifiers);
        }
    }
}

fn wrap(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

#[cfg(test)]
pub fn step(look: &mut [f32; 2], keys: &BTreeSet<String>, elapsed: f64, external: bool) {
    let held = |key: &str| f32::from(keys.contains(key));
    let yaw = held("LookArrowRight") - held("LookArrowLeft");
    let pitch = held("LookArrowUp") - held("LookArrowDown");
    step_axes(look, [yaw, pitch], elapsed, external);
}
pub fn step_axes(look: &mut [f32; 2], axes: [f32; 2], elapsed: f64, external: bool) {
    let [yaw, pitch] = axes;
    let delta = elapsed.clamp(0., 0.25) as f32; // One radian per second, independent of repeats.
    if yaw != 0. {
        look[0] = wrap(look[0] + yaw * delta);
    }
    if pitch != 0. {
        let next = look[1] + pitch * delta;
        look[1] = if external {
            wrap(next)
        } else {
            next.clamp(0., PI / 2.)
        };
    }
}

pub fn apply(camera: &mut Camera, target: [f32; 3], look: [f32; 2], external: bool) {
    if !external {
        let basis = crate::attitude::Basis::new(
            camera.yaw as f64,
            camera.pitch as f64,
            -camera.roll as f64,
        );
        let turned = basis.rotated(basis.up.map(|v| v * look[0] as f64));
        let viewed = turned.rotated(turned.right.map(|v| -v * look[1] as f64));
        let [yaw, pitch, bank] = viewed.angles();
        camera.yaw = yaw as f32;
        camera.pitch = pitch as f32;
        camera.roll = -bank as f32;
        return;
    }
    let offset = std::array::from_fn::<_, 3, _>(|i| camera.position[i] - target[i]);
    let horizontal = offset[0].hypot(offset[2]);
    let radius = horizontal.hypot(offset[1]);
    let elevation = offset[1].atan2(horizontal) + look[1];
    camera.yaw += look[0];
    camera.pitch = -elevation;
    camera.roll = 0.;
    let forward = [
        camera.yaw.sin() * elevation.cos(),
        -elevation.sin(),
        camera.yaw.cos() * elevation.cos(),
    ];
    camera.position = std::array::from_fn(|i| target[i] - radius * forward[i]);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn look_arrows_never_become_flight_controls_until_released() {
        for modifier in [ModifiersState::SHIFT, ModifiersState::CONTROL] {
            let mut keys = BTreeSet::new();
            press(&mut keys, "ArrowDown", ModifiersState::empty());
            modifiers_changed(&mut keys, modifier);
            assert!(!keys.contains("ArrowDown"));
            assert!(keys.contains("LookArrowDown"));
            modifiers_changed(&mut keys, ModifiersState::empty());
            press(&mut keys, "ArrowDown", ModifiersState::empty());
            assert!(!keys.contains("ArrowDown"));
            keys.remove("LookArrowDown");
            press(&mut keys, "ArrowDown", ModifiersState::empty());
            assert!(keys.contains("ArrowDown"));
        }
        let mut keys = BTreeSet::new();
        press(
            &mut keys,
            "ArrowLeft",
            ModifiersState::SUPER | ModifiersState::SHIFT,
        );
        press(
            &mut keys,
            "ArrowRight",
            ModifiersState::ALT | ModifiersState::CONTROL,
        );
        assert!(keys.is_empty());
    }
    #[test]
    fn cockpit_cannot_look_below_eye_line_and_motion_is_time_based() {
        let down = BTreeSet::from(["LookArrowDown".into()]);
        let up = BTreeSet::from(["LookArrowUp".into(), "LookArrowRight".into()]);
        for hz in [30, 60, 144] {
            let mut look = [0.; 2];
            step(&mut look, &down, 0.25, false);
            assert_eq!(look, [0.; 2]);
            for _ in 0..hz {
                step(&mut look, &up, 1. / hz as f64, false);
            }
            assert!((look[0] - 1.).abs() < 0.0001);
            assert!((look[1] - 1.).abs() < 0.0001);
            for _ in 0..20 {
                step(&mut look, &up, 0.25, false);
            }
            assert_eq!(look[1], PI / 2.);
        }
    }
    #[test]
    fn cockpit_look_uses_aircraft_axes_when_banked() {
        let mut camera = Camera::new();
        camera.yaw = 0.3;
        camera.pitch = 0.4;
        camera.roll = -0.8;
        let base = crate::attitude::Basis::new(
            camera.yaw as f64,
            camera.pitch as f64,
            -camera.roll as f64,
        );
        apply(&mut camera, [0.; 3], [PI / 2., 0.], false);
        let viewed = crate::attitude::Basis::new(
            camera.yaw as f64,
            camera.pitch as f64,
            -camera.roll as f64,
        );
        assert!(crate::attitude::dot(viewed.forward, base.right) > 0.999999);
    }
    #[test]
    fn exterior_orbit_keeps_aircraft_centered_above_below_and_over_poles() {
        let target = [100., 5000., 200.];
        for pitch in [-PI, -PI / 2., -0.8, 0., PI / 2., PI] {
            for yaw in [-PI, -0.5, 0., PI] {
                let mut camera = Camera::new();
                camera.position = [100., 5060., 20.];
                camera.yaw = 0.;
                apply(&mut camera, target, [yaw, pitch], true);
                let direction = [
                    camera.yaw.sin() * camera.pitch.cos(),
                    camera.pitch.sin(),
                    camera.yaw.cos() * camera.pitch.cos(),
                ];
                let radius = 180f32.hypot(60.);
                for i in 0..3 {
                    assert!((camera.position[i] + radius * direction[i] - target[i]).abs() < 0.001);
                }
            }
        }
        let mut look = [0.; 2];
        step(
            &mut look,
            &BTreeSet::from(["LookArrowDown".into()]),
            0.25,
            true,
        );
        assert!(look[1] < 0.);
    }
}
