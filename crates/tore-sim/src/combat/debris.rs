//! Fitted breakup placement and ballistic debris, using inert original geometry.
use crate::attitude::{Basis, Vector};
use tore_formats::{Result, aircraft::AircraftId, shape::Shape};
pub const MAX_PIECES: usize = 256;
pub fn variant(id: AircraftId) -> usize {
    AircraftId::ALL.iter().position(|v| *v == id).unwrap() % 2
}
pub fn scale(id: AircraftId) -> f64 {
    match id {
        AircraftId::F14 => 4. / 3.,
        AircraftId::Mig23 => 2. / 3.,
        _ => 1. / 3.,
    }
}
/// Place the fragment center in the largest extent removed from the intact body.
/// This is a geometric fit, not a recovered original attachment transform.
pub fn attachment(id: AircraftId, mut read: impl FnMut(&str) -> Result<Vec<u8>>) -> Result<Vector> {
    let body_suffix = ["A", "C"][variant(id)];
    let piece_suffix = ["B", "D"][variant(id)];
    let intact = Shape::parse(&read(&format!("{}.SH", id.stem()))?)?;
    let body = Shape::parse(&read(&format!("{}_{body_suffix}.SH", id.stem()))?)?;
    let piece = Shape::parse(&read(&format!("{}_{piece_suffix}.SH", id.stem()))?)?;
    let (lo, hi) = bounds(&intact)?;
    let (damaged_lo, damaged_hi) = bounds(&body)?;
    let (piece_lo, piece_hi) = bounds(&piece)?;
    let mut largest = 0.;
    let mut center = [0.; 3];
    for i in 0..3 {
        for (gap, at) in [
            (damaged_lo[i] - lo[i], (lo[i] + damaged_lo[i]) * 0.5),
            (hi[i] - damaged_hi[i], (hi[i] + damaged_hi[i]) * 0.5),
        ] {
            if gap > largest {
                largest = gap;
                center = [0.; 3];
                center[i] = at;
            }
        }
    }
    let offset: Vector =
        std::array::from_fn(|i| (center[i] - (piece_lo[i] + piece_hi[i]) * 0.5) * scale(id));
    Ok([offset[0], offset[2], offset[1]])
}
fn bounds(shape: &Shape) -> Result<(Vector, Vector)> {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in shape.faces.iter().flat_map(|f| &f.positions) {
        for i in 0..3 {
            lo[i] = lo[i].min(f64::from(p[i]));
            hi[i] = hi[i].max(f64::from(p[i]));
        }
    }
    if !lo.iter().chain(&hi).all(|v| v.is_finite()) {
        return Err(super::invalid("empty debris shape"));
    }
    Ok((lo, hi))
}
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub owner: u32,
    pub position: Vector,
    pub velocity: Vector,
    pub basis: Basis,
}
impl Piece {
    pub fn new(
        owner: u32,
        position: Vector,
        velocity: Vector,
        basis: Basis,
        offset: Vector,
    ) -> Self {
        Self {
            owner,
            position: std::array::from_fn(|i| {
                position[i]
                    + basis.right[i] * offset[0]
                    + basis.up[i] * offset[1]
                    + basis.forward[i] * offset[2]
            }),
            velocity,
            basis,
        }
    }
    /// Returns the first swept terrain contact. The caller removes the piece.
    pub fn step(&mut self, ground: &impl Fn(f64, f64) -> f64) -> Option<Vector> {
        let previous = self.position;
        self.velocity[1] -= 32.174 / 120.;
        for i in 0..3 {
            self.position[i] += self.velocity[i] / 120.;
        }
        self.basis = self.basis.rotated([0.8 / 120., 0.5 / 120., 0.2 / 120.]);
        super::live::terrain_hit(previous, self.position, ground).map(|at| {
            let mut p: Vector =
                std::array::from_fn(|i| previous[i] + (self.position[i] - previous[i]) * at);
            p[1] = ground(p[0], p[2]);
            p
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn debris_inherits_full_motion_and_hits_sloping_ground() {
        let mut p = Piece::new(
            7,
            [0., 100., 0.],
            [80., 10., 300.],
            Basis::new(0., 0., 0.),
            [0., 0., 20.],
        );
        assert_eq!(p.position, [0., 100., 20.]);
        assert_eq!(p.velocity, [80., 10., 300.]);
        let ground = |x: f64, _| x * 0.1;
        assert!(p.step(&ground).is_none());
        assert!((p.position[0] - 80. / 120.).abs() < 1e-9);
        let mut contact = None;
        for _ in 0..1200 {
            if let Some(at) = p.step(&ground) {
                contact = Some(at);
                break;
            }
        }
        let at = contact.expect("debris must fall to terrain");
        assert_eq!(at[1], ground(at[0], at[2]));
        assert!(at[2] > 20.);
    }
}
