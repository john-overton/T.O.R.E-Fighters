//! Start geometry against the completed scene, including merged airports.
use super::{Cells, Footprint, Front, Ground, Placed};
use crate::surface::{StartPoints, Surface};
use tore_sim::airport::Scene;

/// Apply the existing deterministic start rules to final target positions and
/// actual runway centers. No object geometry is changed by this calculation.
pub fn place(
    surface: &Surface,
    theater: &dyn Cells,
    scene: &Scene,
    front: Option<Front>,
) -> Option<StartPoints> {
    let runways = scene
        .runways
        .iter()
        .map(|r| Placed {
            id: r.object,
            at: [
                r.approach_center[0].round() as i64,
                r.approach_center[2].round() as i64,
            ],
            heading: r.heading.to_degrees().round() as i32,
            footprint: Footprint::default(),
        })
        .collect();
    super::starts::place(surface, &Ground::new(theater, Vec::new(), runways, front))
}
