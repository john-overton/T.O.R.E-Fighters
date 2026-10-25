//! Bridge footing. Joined spans share one datum instead of sampling unrelated
//! terrain heights at each piece's origin. Fitted, see docs/spec/bridges.md.
use crate::terrain::Placements;
use tore_formats::{mission::Placement, shape::Shape};
use tore_sim::attitude::Basis;

/// The source bridge inventory is the bridge portion of the map-tied list.
pub fn is_bridge(name: &str) -> bool {
    crate::terrain::MAP_TIED_TYPES[..12]
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name))
}

struct Part<'a> {
    placement: &'a Placement,
    shape: &'a Shape,
    scale: f64,
    basis: Basis,
    bounds: [[f64; 2]; 2],
}

impl<'a> Part<'a> {
    fn new(placement: &'a Placement, shape: &'a Shape, scale: f64) -> Self {
        let basis = Basis::new(
            f64::from(placement.angles[0]).to_radians(),
            f64::from(placement.angles[1]).to_radians(),
            f64::from(placement.angles[2]).to_radians(),
        );
        let mut part = Self {
            placement,
            shape,
            scale,
            basis,
            bounds: [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]],
        };
        for point in super::points(shape) {
            let at = part.at(point.map(f64::from));
            for (axis, world_axis) in [0, 2].into_iter().enumerate() {
                part.bounds[0][axis] = part.bounds[0][axis].min(at[world_axis]);
                part.bounds[1][axis] = part.bounds[1][axis].max(at[world_axis]);
            }
        }
        part
    }
    fn at(&self, p: [f64; 3]) -> [f64; 3] {
        super::placed(
            [p[0] * self.scale, p[2] * self.scale, p[1] * self.scale],
            &self.basis,
            [
                f64::from(self.placement.position[0]),
                0.,
                f64::from(self.placement.position[2]),
            ],
        )
    }
    fn joins(&self, other: &Self) -> bool {
        let name = &self.placement.object_type;
        let other_name = &other.placement.object_type;
        // Only the authored END/MID assemblies join. Complete BRD1..4
        // bridges remain independent even when their envelopes meet.
        fn family(s: &str) -> Option<&str> {
            s.strip_suffix("END.OT")
                .or_else(|| s.strip_suffix("MID.OT"))
        }
        family(name).is_some()
            && family(name) == family(other_name)
            && (self.basis.forward[0] * other.basis.forward[0]
                + self.basis.forward[2] * other.basis.forward[2])
                .abs()
                > 0.99
            && (0..2).all(|a| {
                self.bounds[0][a] <= other.bounds[1][a] + 16.
                    && other.bounds[0][a] <= self.bounds[1][a] + 16.
            })
    }
    fn support(&self, height: &impl Fn(f64, f64) -> f64) -> f64 {
        let mut datum = f64::NEG_INFINITY;
        let mut sample = |p: [f64; 3]| {
            let at = self.at(p);
            datum = datum.max(height(at[0], at[2]).max(0.) - at[1]);
        };
        for point in super::points(self.shape) {
            sample(point.map(f64::from));
        }
        // Faces crossing a ridge need interior samples too. 64 ft is a
        // fitted bound, much smaller than the 8,192 ft source terrain cell.
        for face in &self.shape.faces {
            if face.positions.len() < 3 {
                continue;
            }
            let a = face.positions[0].map(f64::from);
            for edge in face.positions[1..].windows(2) {
                let b = edge[0].map(f64::from);
                let c = edge[1].map(f64::from);
                let length = (b[0] - a[0])
                    .hypot(b[1] - a[1])
                    .max((c[0] - a[0]).hypot(c[1] - a[1]))
                    * self.scale;
                let steps = (length / 64.).ceil().clamp(1., 256.) as usize;
                for i in 0..=steps {
                    for j in 0..=steps - i {
                        sample(std::array::from_fn(|k| {
                            a[k] + (b[k] - a[k]) * i as f64 / steps as f64
                                + (c[k] - a[k]) * j as f64 / steps as f64
                        }));
                    }
                }
            }
        }
        datum
    }
}

/// Absolute vertical datum for this bridge's assembly. Authored relative
/// heights and all three source angles stay intact. Neither terrain nor roads
/// are changed. The intact shape determines the datum of its destroyed copy.
pub fn datum(sources: &Placements, p: &Placement, height: impl Fn(f64, f64) -> f64) -> Option<f64> {
    let mut parts = vec![Part::new(
        p,
        sources.shapes.get(&p.object_type)?,
        sources.shape_scale(&p.object_type),
    )];
    for (_, candidate) in sources.placed().flatten() {
        if candidate.key != p.key
            && is_bridge(&candidate.object_type)
            && let Some(shape) = sources.shapes.get(&candidate.object_type)
        {
            parts.push(Part::new(
                candidate,
                shape,
                sources.shape_scale(&candidate.object_type),
            ));
        }
    }
    let mut included = vec![false; parts.len()];
    included[0] = true;
    loop {
        let mut changed = false;
        for i in 0..parts.len() {
            if !included[i] && (0..parts.len()).any(|j| included[j] && parts[i].joins(&parts[j])) {
                included[i] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    parts
        .iter()
        .zip(included)
        .filter(|(_, keep)| *keep)
        .map(|(part, _)| part.support(&height))
        .reduce(f64::max)
        .filter(|v| v.is_finite())
}

/// Terrain support for zero-height, level authored bridge pieces. The shared
/// foundation builder merges adjoining pieces and blends their perimeter.
/// Lowering the corridor to its lowest sampled terrain avoids raising a river
/// or suspending the low end of a bridge above it.
pub fn foundations(sources: &Placements) -> Vec<super::pads::Pad> {
    sources
        .placed()
        .flatten()
        .filter_map(|(_, p)| {
            if !is_bridge(&p.object_type)
                || p.position[1] != 0
                || p.angles[1] != 0
                || p.angles[2] != 0
            {
                return None;
            }
            let part = Part::new(
                p,
                sources.shapes.get(&p.object_type)?,
                sources.shape_scale(&p.object_type),
            );
            Some(super::pads::Pad {
                min: part.bounds[0].map(|v| v - 2.),
                max: part.bounds[1].map(|v| v + 2.),
                height: 0.,
            })
        })
        .collect()
}

/// A merged foundation that includes bridge support uses the lowest of its
/// nine terrain samples, clamped to the sea plane. Other foundations retain
/// their existing rule. Fitted sampling and 512 ft skirt, not retail grading.
pub fn support(
    pad: &super::pads::Pad,
    bridges: &[super::pads::Pad],
    raw: impl Fn(f32, f32) -> f32,
) -> Option<f64> {
    if !bridges
        .iter()
        .any(|b| (0..2).all(|a| pad.min[a] <= b.max[a] && b.min[a] <= pad.max[a]))
    {
        return None;
    }
    let mut low = f64::INFINITY;
    for u in [0., 0.5, 1.] {
        for v in [0., 0.5, 1.] {
            low = low.min(f64::from(raw(
                (pad.min[0] + (pad.max[0] - pad.min[0]) * u) as f32,
                (pad.min[1] + (pad.max[1] - pad.min[1]) * v) as f32,
            )));
        }
    }
    Some(low.max(0.))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tore_formats::{mission::Layout, shape::Face};

    fn sources() -> Placements {
        let mut sources = Placements::of_types(&BTreeMap::new(), std::iter::empty());
        sources.layout = Layout::parse("TEST.MM", b"textFormat\nobj\n type BR1END.OT\n pos 0 0 0\n angle 0 0 0\n .\nobj\n type BR1MID.OT\n pos 0 0 190\n angle 0 0 0\n .\nobj\n type BR1END.OT\n pos 0 0 380\n angle 180 0 0\n .\nobj\n type BR1MID.OT\n pos 0 0 10000\n angle 0 0 0\n .\n").unwrap();
        let shape = || Shape {
            faces: vec![Face {
                positions: vec![
                    [-10., -100., -20.],
                    [10., -100., -20.],
                    [10., 100., -20.],
                    [-10., 100., -20.],
                ],
                colors: vec![20],
                fog: Default::default(),
                uv: Vec::new(),
                texture: String::new(),
                subtype: 0x41,
                normal: None,
                address: 0,
            }],
            lines: Vec::new(),
            billboards: Vec::new(),
            state_words: Default::default(),
        };
        sources.shapes.insert("BR1END.OT".into(), shape());
        sources.shapes.insert("BR1MID.OT".into(), shape());
        sources
    }

    #[test]
    fn bridge_bottom_clears_ground_and_water_without_losing_authored_height() {
        let mut sources = sources();
        sources.layout.placements.truncate(1);
        sources.layout.placements[0].position[1] = 30;
        let p = &sources.layout.placements[0];
        // The explicit 30 ft remains above the corrected footing.
        assert_eq!(datum(&sources, p, |_, _| 100.), Some(120.));
        assert_eq!(datum(&sources, p, |_, _| -100.), Some(20.));
        assert!(is_bridge("brd3.ot"));
        assert!(!is_bridge("ROAD.OT"));
    }

    #[test]
    fn the_bridge_inventory_is_exactly_the_twelve_br_types() {
        // `is_bridge` slices the map-tied list; reordering it must not
        // silently turn a road into a bridge or drop a bridge type.
        let bridges: Vec<_> = crate::terrain::MAP_TIED_TYPES
            .iter()
            .filter(|n| is_bridge(n))
            .collect();
        assert_eq!(bridges.len(), 12);
        assert!(bridges.iter().all(|n| n.starts_with("BR")));
        assert!(
            crate::terrain::MAP_TIED_TYPES
                .iter()
                .filter(|n| !is_bridge(n))
                .all(|n| n.starts_with("ROAD"))
        );
    }

    #[test]
    fn connected_spans_share_support_in_both_directions() {
        let sources = sources();
        let height = |_: f64, z: f64| if z > 5000. { 1000. } else { z.max(0.) * 0.1 };
        let levels: Vec<_> = sources
            .layout
            .placements
            .iter()
            .map(|p| datum(&sources, p, height).unwrap())
            .collect();
        assert!((levels[0] - 68.).abs() < 1e-8);
        assert_eq!(levels[0], levels[1]);
        assert_eq!(levels[1], levels[2]);
        assert_eq!(levels[3], 1020.);
    }

    #[test]
    fn bridge_corridor_cuts_the_high_bank_and_shares_contact_triangles() {
        let sources = sources();
        let pads = foundations(&sources);
        let raw = |_: f32, z: f32| z.max(0.) * 0.1;
        let mesh = super::super::pads::Mesh::build_with_support(pads.clone(), 3, 3, raw, |p| {
            support(p, &pads, raw)
        });
        for z in [0., 190., 380.] {
            assert_eq!(mesh.height(0., z), Some(0.));
        }
        // Outside the blended skirt, the imported terrain is unchanged.
        assert!(
            mesh.height(2000., 380.)
                .is_none_or(|h| (h - 38.).abs() < 1e-4)
        );
        let p = super::super::pads::Pad {
            min: [2000., 2000.],
            max: [2100., 2100.],
            height: 0.,
        };
        assert_eq!(support(&p, &pads, raw), None);
    }

    #[test]
    fn bridge_support_samples_a_ridge_inside_a_face() {
        let mut sources = sources();
        sources.layout.placements.truncate(1);
        let height = |_: f64, z: f64| (30. - z.abs()).max(0.);
        let level = datum(&sources, &sources.layout.placements[0], height).unwrap();
        assert_eq!(level, 50.);
    }
}
