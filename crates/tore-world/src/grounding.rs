//! Grounding: every object stands on the bottom of its shape
//! (docs/formats/objects-and-shapes.md, "Grounding").
//!
//! **The rule.** Every ground object, surface unit, man, wreck, ruin and
//! parked aircraft stands with the lowest point of its drawn shape (faces, lines and
//! sprites, at the scale it is drawn) on the ground: [`lift_ft`] is how far
//! its origin stands above the ground for that. Only three kinds keep their
//! authored rule ([`Footing`]): runways and pavement stand on their pavement
//! plane, bridges and roads on the map's height at their origin, and ships
//! on their waterline (their origin). A presentation scale-up (the far-man
//! boost) scales from the feet, never from the origin ([`scaled_origin`]).
//!
//! Every path that places or draws something on the ground goes through this
//! module: the layout and ground target placements (`terrain::Placements::
//! stance_on_terrain`, which also sets the contact, collision and hit box), moving
//! units, wrecks, ruins, the men's sprites, parked aircraft. Never ground an
//! object any other way: before this module each path did its own grounding
//! (faces only, sprites skipped, men exempt, the boost about the centre) and
//! the same bug came back each time a new path or a new kind of shape
//! appeared. `tore-app --surface-dump --grounding` audits every type of the
//! import against it.
//!
//! Provenance: fitted (B13, 2026-10-12, extended by slice GROUND,
//! 2026-10-13): the lowest point agrees with the shape's F2 ground offset,
//! the word FA 0x42e0c0 reads to stand an object on the ground, within 3 ft
//! for every placed type of the retail data.
use crate::terrain::PlacedSize;
use tore_formats::{
    shape::{Billboard, Shape},
    static_object::Definition,
    surface_unit::class,
};

/// What a placed type stands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Footing {
    /// The bottom of its shape: every building, theater object, land unit,
    /// man, wreck and ruin.
    Bottom,
    /// A runway or strip (`_STRIPProc`): its pavement plane
    /// (`terrain::pavement_height`).
    Pavement,
    /// A bridge or road ([`PlacedSize::MapTied`]): the map's height at its
    /// origin, so it meets the terrain it was drawn for.
    MapHeight,
    /// A ship: its origin is its waterline.
    Waterline,
}

/// What `definition`'s placements stand on.
pub fn footing(definition: &Definition) -> Footing {
    if definition.callbacks.iter().any(|name| name == "_STRIPProc") {
        Footing::Pavement
    } else if PlacedSize::of(definition) == PlacedSize::MapTied {
        Footing::MapHeight
    } else if definition
        .resource_name
        .to_ascii_uppercase()
        .ends_with(".NT")
        && definition.category & class::SHIP != 0
    {
        Footing::Waterline
    } else {
        Footing::Bottom
    }
}

/// The lowest and highest point of a drawn shape along its up axis, in shape
/// units about its origin, or `None` for an empty shape. Faces and lines count every
/// vertex; a viewer-facing sprite spans half its height either side of its
/// centre, as `Billboard::face` draws it.
pub fn height_range(shape: &Shape) -> Option<[f64; 2]> {
    let mut range = [f64::INFINITY, f64::NEG_INFINITY];
    for point in points(shape) {
        let v = f64::from(point[2]);
        range = [range[0].min(v), range[1].max(v)];
    }
    range.iter().all(|v| v.is_finite()).then_some(range)
}

/// Geometry used for footing and contact bounds. Lines are physical geometry
/// too: radar antenna frames can extend below their filled faces. Components
/// retain their decoded offsets within the complete shape.
pub fn points(shape: &Shape) -> impl Iterator<Item = [f32; 3]> + '_ {
    shape
        .faces
        .iter()
        .flat_map(|f| f.positions.iter().copied())
        .chain(shape.lines.iter().flat_map(|l| l.positions))
        .chain(shape.billboards.iter().flat_map(|sprite| {
            let [w, h] = sprite.size.map(|v| v * 0.5);
            let [x, y, z] = sprite.center;
            [[x - w, y - w, z - h], [x + w, y + w, z + h]]
        }))
}

/// Vertical correction for a complete placed shape on uneven terrain. Keep
/// the authored orientation and internal mount offsets; translate the whole
/// assembly until its nearest geometry touches the terrain. The caller adds
/// any authored placement height after this correction. Fitted, agent choice.
/// This measures vertices (including line ends and sprite bounds), not the
/// interior of a face crossing a terrain ridge.
pub fn terrain_correction_ft(
    shape: &Shape,
    scale: f64,
    basis: tore_sim::attitude::Basis,
    origin: [f64; 3],
    height: impl Fn(f64, f64) -> f64,
) -> f64 {
    points(shape)
        .map(|p| {
            let at: [f64; 3] = std::array::from_fn(|i| {
                origin[i]
                    + scale
                        * (basis.right[i] * f64::from(p[0])
                            + basis.forward[i] * f64::from(p[1])
                            + basis.up[i] * f64::from(p[2]))
            });
            height(at[0], at[2]) - at[1]
        })
        .reduce(f64::max)
        .unwrap_or(0.)
}

/// The lowest point of viewer-facing sprites, in shape units about their
/// origin (half each sprite's height below its centre), or `None` for none.
pub fn sprites_bottom(sprites: &[Billboard]) -> Option<f64> {
    sprites
        .iter()
        .map(|sprite| f64::from(sprite.center[2]) - f64::from(sprite.size[1]) * 0.5)
        .reduce(f64::min)
}

/// The bottom of a drawn shape: its lowest point (faces, lines and sprites) in shape
/// units about its origin, or `None` for an empty shape.
pub fn bottom(shape: &Shape) -> Option<f64> {
    height_range(shape).map(|[low, _]| low)
}

/// How far, in world feet, the origin of `shape` drawn at `scale` stands
/// above the ground so its bottom meets it: minus its lowest point at that
/// scale. Negative for a shape wholly above its origin, which is lowered
/// onto the ground the same way; 0 for an empty shape.
pub fn lift_ft(shape: &Shape, scale: f64) -> f64 {
    bottom(shape).map_or(0., |low| -low * scale)
}

/// How far the origin of a `definition` placement drawn as `shape` at
/// `scale` stands above its placement point: [`lift_ft`] for a type that
/// stands on its [bottom](Footing::Bottom), 0 for a ship or a bridge. A
/// runway's pavement rule is `terrain::Placements::stance`'s.
pub fn placement_lift_ft(definition: &Definition, shape: &Shape, scale: f64) -> f64 {
    match footing(definition) {
        Footing::Bottom => lift_ft(shape, scale),
        Footing::Pavement | Footing::MapHeight | Footing::Waterline => 0.,
    }
}

/// The origin of a shape drawn `factor` times larger than at `origin`,
/// keeping its feet where they are: `bottom` is its lowest point in shape
/// units at `scale` feet per unit, `up` its up axis. A presentation scale-up
/// (the far-man boost) grows upward from the ground, never into it.
pub fn scaled_origin(
    origin: [f64; 3],
    up: [f64; 3],
    bottom: f64,
    scale: f64,
    factor: f64,
) -> [f64; 3] {
    // The feet stay put: origin + up * bottom * scale before and after.
    std::array::from_fn(|i| origin[i] + up[i] * bottom * scale * (1. - factor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::shape::Face;

    /// A shape of one triangle spanning `low` to `high` along its up axis.
    fn upright(low: f32, high: f32) -> Shape {
        Shape {
            lines: Vec::new(),
            faces: vec![Face {
                positions: vec![[0., 0., low], [10., 0., low], [0., 0., high]],
                colors: vec![0],
                fog: Default::default(),
                uv: Vec::new(),
                texture: String::new(),
                subtype: 0,
                normal: None,
                address: 0,
            }],
            billboards: Vec::new(),
            state_words: Default::default(),
        }
    }

    /// A man: one sprite `height` tall centred `centre` above the origin.
    fn man(centre: f32, height: f32) -> Shape {
        Shape {
            lines: Vec::new(),
            faces: Vec::new(),
            billboards: vec![Billboard {
                center: [0., 0., centre],
                size: [height * 0.4, height],
                texture: "MAN.PIC".into(),
                uv: None,
                fog: Default::default(),
                address: 0,
            }],
            state_words: Default::default(),
        }
    }

    fn definition(name: &str, callback: &str, category: u16) -> Definition {
        Definition {
            display_name: String::new(),
            class_name: String::new(),
            resource_name: name.into(),
            main_shape: None,
            callbacks: vec![callback.into()],
            hit_points: None,
            category,
            radar_signature: 0,
            infrared_signature: 0,
            explosion: 0,
            crater: 0,
            flags: 0,
            max_visible: 0,
        }
    }

    #[test]
    fn every_ground_object_stands_on_the_bottom_of_its_shape() {
        // A factory hall whose origin is 89 percent of the way up (B13).
        let hall = upright(-128., 16.);
        assert_eq!(height_range(&hall), Some([-128., 16.]));
        let factory = definition("FACTD.OT", "_OBJProc", 0);
        assert_eq!(footing(&factory), Footing::Bottom);
        assert!((placement_lift_ft(&factory, &hall, 1. / 3.) - 128. / 3.).abs() < 1e-9);
        // A shape wholly above its origin comes down onto the ground.
        assert_eq!(lift_ft(&upright(4., 20.), 1.), -4.);
        // A man centred on his origin (the airfield squad TROOPS is too)
        // stands half his height up; a SOLDIER's feet are already on his
        // origin.
        let troops = definition("TROOPS.NT", "_NTProc", class::OTHER);
        assert_eq!(footing(&troops), Footing::Bottom);
        assert!((placement_lift_ft(&troops, &man(0., 20.), 1. / 3.) - 10. / 3.).abs() < 1e-12);
        assert_eq!(lift_ft(&man(6., 12.), 1.), 0.);
        assert_eq!(sprites_bottom(&man(0., 20.).billboards), Some(-10.));
        assert_eq!(sprites_bottom(&[]), None);
        // Land units, launchers and bunkers stand on their bottom too.
        for category in [class::TANK, class::SAM, class::STRUCTURE, class::VEHICLE] {
            assert_eq!(
                footing(&definition("X.NT", "_NTProc", category)),
                Footing::Bottom
            );
        }
        // Runways, bridges and roads, and ships keep their authored rule.
        let strip = definition("STRIP.OT", "_STRIPProc", 0);
        assert_eq!(footing(&strip), Footing::Pavement);
        let bridge = definition("BRDMID.OT", "_OBJProc", 0);
        assert_eq!(footing(&bridge), Footing::MapHeight);
        let ship = definition("KIROV.NT", "_NTProc", class::SHIP);
        assert_eq!(footing(&ship), Footing::Waterline);
        for fixed in [&strip, &bridge, &ship] {
            assert_eq!(placement_lift_ft(fixed, &hall, 1.), 0.);
        }
        let empty = Shape {
            faces: Vec::new(),
            ..upright(0., 1.)
        };
        assert_eq!(lift_ft(&empty, 1.), 0.);
    }

    #[test]
    fn antenna_frame_and_complete_assembly_define_the_footing() {
        let mut shape = upright(-13., 20.);
        shape.lines.push(tore_formats::shape::Line {
            positions: [[0., 0., -14.], [0., 0., 18.]],
            color: 1,
            fog: Default::default(),
        });
        assert_eq!(bottom(&shape), Some(-14.));
        assert!((lift_ft(&shape, 4. / 3.) - 56. / 3.).abs() < 1e-12);
        // The whole dish and frame move together, not separately onto the ground.
        let basis = tore_sim::attitude::Basis::new(0., 0., 0.);
        assert_eq!(
            terrain_correction_ft(&shape, 1., basis, [0., 13., 0.], |_, _| 0.),
            1.
        );
    }

    #[test]
    fn footing_samples_the_slope_under_the_geometry_not_only_the_origin() {
        let shape = upright(0., 20.);
        let basis = tore_sim::attitude::Basis::new(0., 0., 0.);
        // One foot is ten feet east, where the ground rises five feet.
        assert_eq!(
            terrain_correction_ft(&shape, 1., basis, [0.; 3], |x, _| x * 0.5),
            5.
        );
        // A floating complete assembly is lowered, not clamped to upward moves.
        assert_eq!(
            terrain_correction_ft(&shape, 1., basis, [0., 9., 0.], |_, _| 0.),
            -9.
        );
    }

    #[test]
    fn placed_contact_and_draw_origin_rise_together_and_keep_authored_height() {
        use crate::terrain::Placements;
        use std::collections::BTreeMap;
        let resources: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut sources = Placements::of_types(&resources, std::iter::empty());
        sources
            .definitions
            .insert("TEST.OT".into(), definition("TEST.OT", "_OBJProc", 0));
        sources.shapes.insert("TEST.OT".into(), upright(0., 20.));
        let layout = tore_formats::mission::Layout::parse(
            "TEST.MM",
            b"textFormat\nobj\n type TEST.OT\n pos 0 7 0\n.\n",
        )
        .unwrap();
        let p = &layout.placements[0];
        let flat = sources.stance(p, 0.).unwrap();
        let slope = sources.stance_on_terrain(p, |x, _| x * 0.5).unwrap();
        assert_eq!(flat.origin[1], 7.);
        assert_eq!(slope.origin[1], 12.);
        assert_eq!(slope.center[1] - flat.center[1], 5.);
        assert_eq!(slope.support_origin, flat.support_origin);
        assert_eq!(slope.half, flat.half);
    }

    #[test]
    fn a_scale_up_grows_from_the_feet() {
        // A man 20 units tall centred on his origin, lifted 10 ft to stand.
        let up = [0., 1., 0.];
        let origin = [5., 110., 7.];
        let boosted = scaled_origin(origin, up, -10., 1., 6.);
        // Six times larger, his feet stay at 100 ft and his origin rises.
        assert_eq!(boosted, [5., 160., 7.]);
        assert_eq!(boosted[1] - 10. * 6., 100.);
        // Feet already on the origin: nothing moves.
        assert_eq!(scaled_origin(origin, up, 0., 1., 6.), origin);
    }
}
