//! Source HIND.SH gear cards with documented fitted rigid retraction.
use crate::{AppResult, flight::State};
use std::collections::BTreeSet;
use tore_formats::shape::{Face, Shape};
const GEAR: [usize; 6] = [0x5639, 0x5658, 0x5677, 0x5696, 0x56b5, 0x56d4];
pub struct Rig;
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 24994
            || shape.faces.len() != 387
            || shape.state_words != [0x7190, 0x7196].into()
        {
            return Err("unreviewed HIND.SH gear layout".into());
        }
        let deployed = Shape::with_state(bytes, &[(0x7196, 1)].into())?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let active: BTreeSet<_> = deployed.faces.iter().map(|f| f.address).collect();
        if !original.is_subset(&active)
            || active
                .difference(&original)
                .copied()
                .collect::<BTreeSet<_>>()
                != GEAR.into()
        {
            return Err("unreviewed HIND.SH gear branch".into());
        }
        for face in deployed
            .faces
            .into_iter()
            .filter(|f| GEAR.contains(&f.address))
        {
            let side = if face.address <= 0x5658 { -1. } else { 1. };
            let roots = if face.address >= 0x56b5 {
                [[0., 35., -17.], [0., 43., -17.]]
            } else {
                [[side * 7., -23., -17.], [side * 7., -13., -17.]]
            };
            if face.positions.len() != 4 || !roots.iter().all(|p| face.positions.contains(p)) {
                return Err("unreviewed HIND.SH gear attachment".into());
            }
            shape.faces.push(face);
        }
        Ok((Self, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut face = source.clone();
        if GEAR.contains(&source.address) {
            if state.gear <= 0. {
                return None;
            }
            gear(&mut face, state.gear.clamp(0., 1.));
        }
        Some(face)
    }
}
fn gear(face: &mut Face, deployed: f64) {
    let closing = 1. - deployed;
    if face.address >= 0x56b5 {
        crate::additional_animation::turn(
            face,
            [0., 43., -17.],
            [1., 0., 0.],
            -std::f64::consts::FRAC_PI_2 * closing,
        );
    } else {
        let side = if face.address <= 0x5658 { -1. } else { 1. };
        crate::additional_animation::turn(
            face,
            [side * 7., -18., -17.],
            [0., 1., 0.],
            -side as f64 * 160f64.to_radians() * closing,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn synthetic(address: usize, points: Vec<[f32; 3]>) -> Face {
        let n = points.len();
        Face {
            address,
            positions: points,
            colors: vec![12; n],
            uv: vec![],
            texture: String::new(),
            subtype: 0,
            normal: None,
            fog: Default::default(),
        }
    }
    #[test]
    fn gear_preserves_roots_rigidity_and_side_through_dense_travel() {
        // Invented wheel dimensions and Y extents on the reviewed upper edge.
        for (address, points, root) in [
            (
                0x5639,
                vec![
                    [-7., -21., -17.],
                    [-7., -15., -17.],
                    [-11., -15., -28.],
                    [-11., -21., -28.],
                ],
                [-7., -21., -17.],
            ),
            (
                0x5677,
                vec![
                    [7., -21., -17.],
                    [7., -15., -17.],
                    [11., -15., -28.],
                    [11., -21., -28.],
                ],
                [7., -21., -17.],
            ),
            (
                0x56b5,
                vec![
                    [0., 43., -17.],
                    [0., 37., -17.],
                    [0., 37., -28.],
                    [0., 43., -28.],
                ],
                [0., 43., -17.],
            ),
        ] {
            let source = synthetic(address, points);
            for step in 0..=400 {
                let mut moved = source.clone();
                gear(&mut moved, step as f64 / 400.);
                assert_eq!(moved.positions[0], root);
                assert!(!crate::aircraft_animation_probe::planar_crossing(
                    &moved, 1.
                ));
                for i in 0..4 {
                    for j in i + 1..4 {
                        let d = |a: [f32; 3], b: [f32; 3]| {
                            (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>()
                        };
                        assert!(
                            (d(source.positions[i], source.positions[j])
                                - d(moved.positions[i], moved.positions[j]))
                            .abs()
                                < 0.001
                        );
                    }
                }
                if address < 0x56b5 {
                    assert!(moved.positions.iter().all(|p| p[0] * root[0] > 0.));
                }
            }
        }
    }
}
