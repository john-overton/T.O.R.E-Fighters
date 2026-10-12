//! Pylons and the stores still on them, drawn under every aircraft that
//! carries a combat loadout (B3, docs/spec/stores-on-pylons.md).
//!
//! Retail draws no stores on aircraft; this is an opinionated addition John
//! asked for on 2026-10-12. What each station holds comes from the render
//! snapshot ([`StorePose`]); this module only turns it into vertices:
//!
//! - **Where.** A station's stores hang at its source hardpoint position, a
//!   paired station's split between that position and its mirror, as
//!   [`Rack::sides`] counts them, which is also the side the next one leaves
//!   from. So a released store appears in flight where it hung.
//! - **Several on one side** sit on a fitted rack: two side by side, three as
//!   a triple ejector rack (two shoulders, one below), more as such racks one
//!   behind another. The slots keep the full load's layout, and the
//!   highest-numbered slot empties first.
//! - **Stores** use the weapon's own shape. Retail has no rocket pod or fuel
//!   tank shape, so those, and a store with no shape, are fitted bodies.
//! - **Pylons** are fitted slabs from the top of the store or rack up to the
//!   model's underside straight above the hardpoint
//!   ([`Airframe::pylon_top`]); with nothing above (a wingtip rail) a short
//!   rail. A pylon stays when its stores are gone.
//!
//! All geometry is solid colour in the aircraft vertex format, so it joins
//! the batch of the aircraft it hangs from with no extra draw call.
use crate::{aircraft::Airframe, render_snapshot::CombatArt};
use tore_formats::shape::Shape;
use tore_sim::attitude::{Basis, Vector};
use tore_world::snapshot::{StoreLook, StorePose};

/// Pylon and rack grey, linear RGB (fitted).
const PYLON: [f32; 3] = [0.5, 0.52, 0.54];
/// Tank and pod body colours (fitted).
const TANK: [f32; 3] = [0.55, 0.57, 0.6];
const POD: [f32; 3] = [0.4, 0.43, 0.4];
/// A store with no shape: length and diameter, feet (fitted).
const PLAIN: [f64; 2] = [8., 0.9];
/// A pylon's height when nothing of the model is above it (a rail), feet.
const RAIL: f64 = 0.35;
/// A pylon's thickness, feet.
const PYLON_WIDTH: f64 = 0.35;

/// A store's size: length and largest cross-section width, feet, and where
/// its body sits against its origin: how far its top is above it and how far
/// ahead of it its middle is.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Dims {
    length: f64,
    diameter: f64,
    top: f64,
    middle: f64,
}

impl Dims {
    /// A fitted body centred on its origin.
    const fn body([length, diameter]: [f64; 2]) -> Self {
        Self {
            length,
            diameter,
            top: diameter / 2.,
            middle: 0.,
        }
    }
    /// A weapon shape's own size; positions are right, forward and up in
    /// source units, a third of a foot each.
    fn of(shape: &Shape) -> Self {
        let mut low = [f64::MAX; 3];
        let mut high = [f64::MIN; 3];
        // Not the texture-only faces (a missile's exhaust sheet), which the
        // drawing leaves out too.
        for q in shape
            .faces
            .iter()
            .filter(|f| !texture_only(f))
            .flat_map(|f| f.positions.iter())
        {
            for k in 0..3 {
                low[k] = low[k].min(f64::from(q[k]) / 3.);
                high[k] = high[k].max(f64::from(q[k]) / 3.);
            }
        }
        if low[0] > high[0] {
            return Self::body(PLAIN);
        }
        Self {
            length: high[1] - low[1],
            diameter: (high[0] - low[0]).max(high[2] - low[2]).max(0.2),
            top: high[2].max(0.),
            middle: (high[1] + low[1]) / 2.,
        }
    }
}

/// Where slot `slot` of a side loaded with `loaded` stores of `size`
/// (length, diameter) sits against the side's pylon point: feet right, up
/// and forward.
pub fn slot_offset(slot: u16, loaded: u16, length: f64, diameter: f64) -> Vector {
    if loaded <= 1 {
        return [0.; 3];
    }
    let spread = 0.55 * diameter;
    let groups = loaded.div_ceil(3);
    let group = slot / 3;
    let forward = (f64::from(group) - f64::from(groups - 1) / 2.) * 1.05 * length;
    let [right, up] = match slot % 3 {
        0 => [-spread, 0.],
        1 => [spread, 0.],
        _ => [0., -0.95 * diameter],
    };
    [right, up, forward]
}

/// The vertices of `stores`, hung under an aircraft at `position` (relative
/// to the render origin) with `attitude` (yaw, pitch, bank), appended to
/// `out`. `airframe` gives the pylon heights; `palette` the shapes' colours.
pub fn vertices(
    out: &mut Vec<f32>,
    stores: &[StorePose],
    position: Vector,
    [yaw, pitch, bank]: [f64; 3],
    airframe: Option<&Airframe>,
    art: &CombatArt,
    palette: &[[u8; 3]; 256],
) {
    if stores.is_empty() {
        return;
    }
    let basis = Basis::new(yaw, pitch, bank);
    let at = |offset: Vector| -> Vector {
        std::array::from_fn(|i| {
            position[i]
                + basis.right[i] * offset[0]
                + basis.up[i] * offset[1]
                + basis.forward[i] * offset[2]
        })
    };
    for store in stores {
        let shape = store
            .shape
            .as_deref()
            .and_then(|name| art.shape(name))
            .filter(|_| store.look == StoreLook::Store);
        let dims = match (store.look, shape) {
            (StoreLook::Pod | StoreLook::Tank, _) if store.body[0] > 0. => Dims::body(store.body),
            (StoreLook::Pod, _) => Dims::body(tore_world::snapshot::POD_BODY),
            (StoreLook::Tank, _) => Dims::body(tore_world::snapshot::tank_body(2_000.)),
            (StoreLook::Store, Some(shape)) => Dims::of(shape),
            (StoreLook::Store, None) => Dims::body(PLAIN),
        };
        let top = airframe.and_then(|a| a.pylon_top(usize::from(store.hardpoint)));
        let rack = store.rack;
        let loaded = rack.sides(store.mount, store.loaded.max(store.items));
        let held = rack.sides(store.mount, store.items);
        for side in 0..2 {
            if loaded[side] == 0 {
                continue;
            }
            let mirror = if side == 1 { -1. } else { 1. };
            let point = [store.mount[0] * mirror, store.mount[1], store.mount[2]];
            // The pylon: from the top of the highest store up to the model.
            let store_top = point[1] + dims.top;
            let height = top.map_or(RAIL, |top| (point[1] + top - store_top).max(0.));
            if height > 0.05 {
                let length = (0.55 * dims.length).clamp(1.5, 8.);
                let width = if loaded[side] > 1 {
                    1.1 * dims.diameter + PYLON_WIDTH
                } else {
                    PYLON_WIDTH
                };
                block(
                    out,
                    &at,
                    [point[0], store_top + height / 2., point[2] + dims.middle],
                    [width.min(PYLON_WIDTH * 4.), height, length],
                    PYLON,
                );
            }
            for slot in 0..held[side] {
                let offset = slot_offset(slot, loaded[side], dims.length, dims.diameter);
                let centre = at(std::array::from_fn(|i| point[i] + offset[i]));
                match (store.look, shape) {
                    (StoreLook::Store, Some(shape)) => mesh(out, shape, centre, &basis, palette),
                    (look, _) => body(
                        out,
                        &basis,
                        centre,
                        [dims.length, dims.diameter],
                        match look {
                            StoreLook::Tank => TANK,
                            StoreLook::Pod => POD,
                            StoreLook::Store => PYLON,
                        },
                    ),
                }
            }
        }
    }
}

/// The texture-only faces (the missile exhaust sheets) the flying store
/// omits too.
fn texture_only(face: &tore_formats::shape::Face) -> bool {
    matches!(face.subtype, 0x4c | 0x5c | 0x6c | 0x7c)
}

/// A weapon shape at `centre`, along the aircraft's axes: what
/// `render_snapshot` draws for the same store once it is released.
fn mesh(
    out: &mut Vec<f32>,
    shape: &Shape,
    centre: Vector,
    basis: &Basis,
    palette: &[[u8; 3]; 256],
) {
    for face in &shape.faces {
        if texture_only(face) {
            continue;
        }
        for i in 1..face.positions.len().saturating_sub(1) {
            for j in [0, i, i + 1] {
                let q = face.positions[j];
                let pos = std::array::from_fn(|k| {
                    centre[k]
                        + (basis.right[k] * f64::from(q[0])
                            + basis.up[k] * f64::from(q[2])
                            + basis.forward[k] * f64::from(q[1]))
                            / 3.
                });
                vertex(
                    out,
                    pos,
                    palette[face.colors[j] as usize].map(|c| f32::from(c) / 255.),
                );
            }
        }
    }
}

/// A box centred on `centre` (aircraft frame, feet) of `size` right, up and
/// forward, both windings so either cull mode shows it.
fn block(
    out: &mut Vec<f32>,
    at: &impl Fn(Vector) -> Vector,
    centre: Vector,
    size: Vector,
    color: [f32; 3],
) {
    let corner = |sx: f64, sy: f64, sz: f64| {
        at([
            centre[0] + sx * size[0] / 2.,
            centre[1] + sy * size[1] / 2.,
            centre[2] + sz * size[2] / 2.,
        ])
    };
    let faces: [[[f64; 3]; 4]; 6] = [
        [[1., -1., -1.], [1., 1., -1.], [1., 1., 1.], [1., -1., 1.]],
        [
            [-1., -1., 1.],
            [-1., 1., 1.],
            [-1., 1., -1.],
            [-1., -1., -1.],
        ],
        [[-1., 1., -1.], [-1., 1., 1.], [1., 1., 1.], [1., 1., -1.]],
        [
            [-1., -1., 1.],
            [-1., -1., -1.],
            [1., -1., -1.],
            [1., -1., 1.],
        ],
        [[-1., -1., 1.], [1., -1., 1.], [1., 1., 1.], [-1., 1., 1.]],
        [
            [1., -1., -1.],
            [-1., -1., -1.],
            [-1., 1., -1.],
            [1., 1., -1.],
        ],
    ];
    for quad in faces {
        let p = quad.map(|[x, y, z]| corner(x, y, z));
        quad_both(out, p, color);
    }
}

/// A fitted round body (tank or pod) centred on `centre`: an eight-sided
/// tube of `[length, diameter]` along the aircraft's forward axis, with
/// pointed ends a fifth of its length each.
fn body(
    out: &mut Vec<f32>,
    basis: &Basis,
    centre: Vector,
    [length, diameter]: [f64; 2],
    color: [f32; 3],
) {
    const SIDES: usize = 8;
    let radius = diameter / 2.;
    let ring = |k: usize, along: f64, scale: f64| -> Vector {
        let angle = k as f64 * std::f64::consts::TAU / SIDES as f64;
        let (s, c) = angle.sin_cos();
        std::array::from_fn(|i| {
            centre[i]
                + basis.right[i] * c * radius * scale
                + basis.up[i] * s * radius * scale
                + basis.forward[i] * along
        })
    };
    let half = length / 2.;
    let shoulder = half - length / 5.;
    let tip =
        |along: f64| -> Vector { std::array::from_fn(|i| centre[i] + basis.forward[i] * along) };
    for k in 0..SIDES {
        let n = (k + 1) % SIDES;
        quad_both(
            out,
            [
                ring(k, -shoulder, 1.),
                ring(n, -shoulder, 1.),
                ring(n, shoulder, 1.),
                ring(k, shoulder, 1.),
            ],
            color,
        );
        for (end, along) in [(half, shoulder), (-half, -shoulder)] {
            let t = [tip(end), ring(k, along, 1.), ring(n, along, 1.)];
            for p in [t, [t[0], t[2], t[1]]] {
                for q in p {
                    vertex(out, q, color);
                }
            }
        }
    }
}

fn quad_both(out: &mut Vec<f32>, p: [Vector; 4], color: [f32; 3]) {
    for tri in [
        [p[0], p[1], p[2]],
        [p[0], p[2], p[3]],
        [p[0], p[2], p[1]],
        [p[0], p[3], p[2]],
    ] {
        for q in tri {
            vertex(out, q, color);
        }
    }
}

/// One lit solid-colour vertex in the aircraft format, as a flying weapon's.
fn vertex(out: &mut Vec<f32>, pos: Vector, color: [f32; 3]) {
    out.extend([
        pos[0] as f32,
        pos[1] as f32,
        pos[2] as f32,
        0.,
        0.,
        -1.,
        color[0],
        color[1],
        color[2],
        -1.,
    ]);
}

/// The stores of the flown aircraft drawn with `airframe` over its presented
/// flight state, as a new vertex list: none once its wreck is gone.
pub fn for_flown(
    stores: &[StorePose],
    state: &crate::flight::State,
    airframe: &Airframe,
    art: &CombatArt,
    scenery: &crate::scenery::Scenery,
) -> Vec<f32> {
    let mut out = Vec::new();
    if !state.wreck_gone() {
        vertices(
            &mut out,
            stores,
            scenery.relative(state.position),
            [state.yaw, state.pitch, state.bank],
            Some(airframe),
            art,
            &airframe.palette,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::combat::hardpoints::Rack;

    #[test]
    fn rack_slots_spread_side_by_side_then_below_then_behind() {
        let dims = |slot, loaded| slot_offset(slot, loaded, 8., 1.);
        assert_eq!(dims(0, 1), [0.; 3]);
        let two = [dims(0, 2), dims(1, 2)];
        assert!(two[0][0] < 0. && two[1][0] > 0. && two[0][1] == 0.);
        let third = dims(2, 3);
        assert_eq!(third[0], 0.);
        assert!(third[1] < -0.9);
        // Six: two triple racks, one ahead of the other.
        assert!(dims(0, 6)[2] < 0. && dims(3, 6)[2] > 0.);
        assert!(
            Rack {
                location: 4,
                paired: true
            }
            .two_sided([10., 0., 0.])
        );
    }

    #[test]
    fn a_pair_draws_one_store_a_side_and_its_pylons_stay() {
        let art = CombatArt::synthetic(Default::default());
        let pair = StorePose {
            hardpoint: 6,
            mount: [10., -2., 0.],
            rack: Rack {
                location: 4,
                paired: true,
            },
            shape: None,
            look: StoreLook::Store,
            items: 2,
            loaded: 2,
            body: [0.; 2],
        };
        let draw = |items: u16| {
            let mut out = Vec::new();
            vertices(
                &mut out,
                &[StorePose {
                    items,
                    ..pair.clone()
                }],
                [0.; 3],
                [0.; 3],
                None,
                &art,
                &[[0; 3]; 256],
            );
            out
        };
        let full = draw(2);
        let one = draw(1);
        let none = draw(0);
        assert!(full.len() > one.len() && one.len() > none.len());
        // Both pylons remain with nothing on them, one a side.
        let xs: Vec<f32> = none.chunks_exact(10).map(|v| v[0]).collect();
        assert!(xs.iter().any(|x| *x > 9.) && xs.iter().any(|x| *x < -9.));
        // With one left, it hangs on the right: the left went first.
        let count =
            |v: &[f32], right: bool| v.chunks_exact(10).filter(|v| (v[0] > 0.) == right).count();
        assert!(count(&one, true) > count(&none, true));
        assert_eq!(count(&one, false), count(&none, false));
    }
}
