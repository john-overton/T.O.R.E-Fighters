//! The AC-130 aim-point box on every view (docs/spec/ac130-linked-guns.md,
//! opinionated, John 2026-10-09). Always one box, at every time: on the
//! tracked target, the pinned ground point, or the free-slew point. A small
//! diamond marks where the guns will really hit, but only while they cannot
//! put rounds on the box (slewing, out of arc or out of range). A box off
//! screen becomes an edge arrow; a diamond off screen is not drawn.
//!
//! This module holds what every view shares: what to mark, where it falls on
//! a view, and the strokes of each mark. The HUD (`weapon_hud`), the floating
//! marks over the world (`flight_canvas`) and the Front View and Other View
//! pages (`instruments::aim_marks`) each draw those strokes in their own
//! pixels. Other aircraft have no gunsight and get nothing here.
use crate::camera::{Camera, Locate};
use tore_sim::{
    attitude::Vector,
    combat::{
        gunship::{self, Sight},
        live::{Configuration, Readiness},
    },
};
use tore_world::readout::{CockpitReadout, GunsightReadout};

/// How the box is drawn, by what it marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The HUD's target square, with its friendly X.
    Tracked { friendly: bool },
    /// The same square with a centre dot.
    Pinned,
    /// Four corner brackets: a point the sight sweeps over, not held.
    Free,
}

/// What the box and the diamond mark, in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marks {
    pub style: Style,
    /// The aim point: where the guns try to hit.
    pub aim: Vector,
    /// Where the guns will hit, when they cannot put rounds on the aim point.
    pub diamond: Option<Vector>,
}

/// A line from one point to another, in the units of the view that draws it.
pub type Stroke = ((f64, f64), (f64, f64));

/// Half the box, in HUD pixels (the HUD target square is 14 across).
pub const HALF: f64 = 7.;
/// Half a diamond's width, in HUD pixels.
pub const DIAMOND_HALF: f64 = 3.5;
/// The box's width, in HUD pixels.
pub const WIDTH: f64 = 2. * HALF;
/// The length of the arrow at a full-screen view's edge, in HUD pixels: the
/// HUD's own arrow shape (8) at twice the size, since it has no HUD around it
/// to read against.
pub const EDGE_ARROW: f64 = 16.;
/// The box's width on the Front View and Other View pages, in page pixels.
pub const PAGE_BOX: f64 = 7.;

/// The gun the diamond follows: the candidate gun when it has a pipper, else
/// the lowest linked gun that has one.
fn follows(
    readout: &CockpitReadout,
    config: &Configuration,
    sight: &GunsightReadout,
) -> Option<usize> {
    let candidate = config
        .stations
        .get(readout.stores.selected())
        .and_then(|station| {
            gunship::GUNS
                .iter()
                .position(|source| station.weapon.source.eq_ignore_ascii_case(source))
        });
    candidate
        .filter(|slot| sight.impacts[*slot].is_some())
        .or_else(|| {
            (0..3).find(|slot| {
                readout.stores.gun_group & (1 << slot) != 0 && sight.impacts[*slot].is_some()
            })
        })
}

/// Whether a gun in this state cannot put rounds on the aim point.
pub fn cannot_hit(status: Readiness) -> bool {
    matches!(
        status,
        Readiness::GunSlewing
            | Readiness::GunArc
            | Readiness::MaximumRange
            | Readiness::MinimumRange
    )
}

/// The marks of a seat on an AC-130, from the host's gunsight readout (the
/// same in single player and online); `None` on any other aircraft. `friendly`
/// says the tracked object is on the seat's side.
pub fn marks(readout: &CockpitReadout, config: &Configuration, friendly: bool) -> Option<Marks> {
    let sight = readout.gunsight.as_ref()?;
    let aim = sight.aim?;
    let style = match sight.sight {
        Sight::Tracked(_) => Style::Tracked { friendly },
        Sight::Pinned(_) => Style::Pinned,
        Sight::Free => Style::Free,
    };
    let diamond = follows(readout, config, sight)
        .filter(|slot| cannot_hit(sight.status[*slot]))
        .and_then(|slot| sight.impacts[slot])
        .map(|impact| impact.point());
    Some(Marks {
        style,
        aim,
        diamond,
    })
}

/// Where a mark falls on a view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cue {
    On([f64; 2]),
    /// Off screen, towards this unit direction (x right, y down).
    Edge([f64; 2]),
}

/// The marks on one view, in its pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub style: Style,
    pub boxed: Cue,
    /// On screen and clear of the box; an off-screen diamond is dropped.
    pub diamond: Option<[f64; 2]>,
}

/// Place the marks on a `size` pixel view of `camera`. A diamond within
/// `box_width` pixels of the box would sit on it, so it is not drawn.
pub fn layout(camera: &Camera, size: [u32; 2], marks: &Marks, box_width: f64) -> Option<Layout> {
    let boxed = match camera.locate(size, marks.aim)? {
        Locate::On(point) => Cue::On(point),
        Locate::Edge(direction) => Cue::Edge(direction),
    };
    let diamond = marks
        .diamond
        .and_then(|point| match camera.locate(size, point)? {
            Locate::On(point) => Some(point),
            Locate::Edge(_) => None,
        })
        .filter(|point| match boxed {
            Cue::On(b) => (point[0] - b[0]).hypot(point[1] - b[1]) > box_width,
            Cue::Edge(_) => true,
        });
    Some(Layout {
        style: marks.style,
        boxed,
        diamond,
    })
}

/// The strokes of the box about its centre, in HUD pixels.
pub fn box_strokes(style: Style) -> Vec<Stroke> {
    let h = HALF;
    match style {
        Style::Tracked { friendly } => {
            let mut strokes = square();
            if friendly {
                strokes.push(((-3., -3.), (3., 3.)));
                strokes.push(((-3., 3.), (3., -3.)));
            }
            strokes
        }
        Style::Pinned => {
            let mut strokes = square();
            strokes.push(((0., 0.), (0., 0.)));
            strokes
        }
        Style::Free => {
            const ARM: f64 = 4.;
            let mut strokes = Vec::with_capacity(8);
            for (sx, sy) in [(-1., -1.), (1., -1.), (1., 1.), (-1., 1.)] {
                let corner = (sx * h, sy * h);
                strokes.push((corner, (corner.0 - sx * ARM, corner.1)));
                strokes.push((corner, (corner.0, corner.1 - sy * ARM)));
            }
            strokes
        }
    }
}

fn square() -> Vec<Stroke> {
    vec![
        ((-HALF, -HALF), (HALF, -HALF)),
        ((HALF, -HALF), (HALF, HALF)),
        ((HALF, HALF), (-HALF, HALF)),
        ((-HALF, HALF), (-HALF, -HALF)),
    ]
}

/// The diamond's four sides about its centre, for a half width `half`.
pub fn diamond_strokes(half: f64) -> [Stroke; 4] {
    [
        ((0., -half), (half, 0.)),
        ((half, 0.), (0., half)),
        ((0., half), (-half, 0.)),
        ((-half, 0.), (0., -half)),
    ]
}

/// The two arms of an edge arrow whose tip is the origin and which points
/// along the unit `direction`; each arm is `length` long and leans out
/// `spread` to its side.
pub fn chevron_strokes(direction: [f64; 2], length: f64, spread: f64) -> [Stroke; 2] {
    let [dx, dy] = direction;
    [-1., 1.].map(|side| {
        (
            (
                -dx * length - dy * side * spread,
                -dy * length + dx * side * spread,
            ),
            (0., 0.),
        )
    })
}

/// A rectangle on a view: x, y, width, height.
pub type Rect = (f64, f64, f64, f64);

/// Where the ray from the middle of a `size` view along the unit `direction`
/// leaves the view shrunk by `margin` pixels on every side.
pub fn edge_point(size: [f64; 2], margin: f64, direction: [f64; 2]) -> [f64; 2] {
    edge_point_clear(size, margin, direction, &[], 0.)
}

/// [`edge_point`], pulled back along the ray until it is `pad` pixels clear
/// of every `blocked` rectangle (the instrument windows hide anything drawn
/// under them), or at the middle of the view if none is.
pub fn edge_point_clear(
    size: [f64; 2],
    margin: f64,
    [dx, dy]: [f64; 2],
    blocked: &[Rect],
    pad: f64,
) -> [f64; 2] {
    let (cx, cy) = (size[0] / 2., size[1] / 2.);
    let reach = |half: f64, d: f64| {
        if d.abs() < 1e-12 {
            f64::INFINITY
        } else {
            (half - margin).max(1.) / d.abs()
        }
    };
    let far = reach(cx, dx).min(reach(cy, dy));
    let at = |t: f64| [cx + dx * t, cy + dy * t];
    let covered = |[x, y]: [f64; 2]| {
        blocked.iter().any(|&(left, top, w, h)| {
            x > left - pad && x < left + w + pad && y > top - pad && y < top + h + pad
        })
    };
    let mut t = far;
    while t > 0. && covered(at(t)) {
        t -= 2.;
    }
    at(t.max(0.))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A camera at the origin looking along +Z (north) with the renderer's
    /// 60 degree vertical field of view.
    fn camera() -> Camera {
        let mut camera = Camera::new();
        camera.position = [0.; 3];
        camera.yaw = 0.;
        camera.pitch = 0.;
        camera.roll = 0.;
        camera.zoom = 1.;
        camera
    }
    const SIZE: [u32; 2] = [800, 600];
    fn marks_at(aim: Vector, diamond: Option<Vector>) -> Marks {
        Marks {
            style: Style::Free,
            aim,
            diamond,
        }
    }

    #[test]
    fn a_box_in_front_sits_on_its_pixel() {
        let layout = layout(&camera(), SIZE, &marks_at([0., 0., 5000.], None), 20.).unwrap();
        assert_eq!(layout.boxed, Cue::On([400., 300.]));
        assert_eq!(layout.diamond, None);
    }

    #[test]
    fn a_box_off_screen_becomes_an_edge_direction() {
        // Due east is 90 degrees off the 30 degree half field of view.
        let Cue::Edge([dx, dy]) = layout(&camera(), SIZE, &marks_at([9000., 0., 100.], None), 20.)
            .unwrap()
            .boxed
        else {
            panic!("expected an edge arrow");
        };
        assert!((dx - 1.).abs() < 1e-9 && dy.abs() < 1e-9);
        // Above and to the left: up on screen is negative y.
        let Cue::Edge([dx, dy]) =
            layout(&camera(), SIZE, &marks_at([-4000., 4000., 100.], None), 20.)
                .unwrap()
                .boxed
        else {
            panic!("expected an edge arrow");
        };
        assert!(dx < 0. && dy < 0. && (dx.hypot(dy) - 1.).abs() < 1e-9);
    }

    #[test]
    fn a_box_behind_keeps_the_side_of_its_bearing() {
        let Cue::Edge([dx, _]) = layout(&camera(), SIZE, &marks_at([3000., 0., -5000.], None), 20.)
            .unwrap()
            .boxed
        else {
            panic!("expected an edge arrow");
        };
        assert!(dx > 0.9, "right of the nose stays right behind it: {dx}");
        // Exactly behind has no side: a fixed direction, not nothing.
        assert!(matches!(
            layout(&camera(), SIZE, &marks_at([0., 0., -5000.], None), 20.)
                .unwrap()
                .boxed,
            Cue::Edge([1., 0.])
        ));
    }

    #[test]
    fn a_diamond_near_the_box_is_dropped_and_a_far_one_kept() {
        let aim = [0., 0., 5000.];
        // 50 ft right at 5000 ft is about 4.8 pixels from the box.
        let near = layout(&camera(), SIZE, &marks_at(aim, Some([50., 0., 5000.])), 20.).unwrap();
        assert_eq!(near.diamond, None);
        let far = layout(
            &camera(),
            SIZE,
            &marks_at(aim, Some([800., 0., 5000.])),
            20.,
        )
        .unwrap();
        let [x, y] = far.diamond.expect("a clear diamond is drawn");
        assert!(x > 460. && (y - 300.).abs() < 1e-6, "{x},{y}");
    }

    #[test]
    fn a_diamond_off_screen_disappears_even_when_the_box_is_an_arrow() {
        let on_box = layout(
            &camera(),
            SIZE,
            &marks_at([0., 0., 5000.], Some([9000., 0., 100.])),
            20.,
        )
        .unwrap();
        assert_eq!(on_box.diamond, None);
        let both_off = layout(
            &camera(),
            SIZE,
            &marks_at([9000., 0., 100.], Some([-9000., 0., 100.])),
            20.,
        )
        .unwrap();
        assert!(matches!(both_off.boxed, Cue::Edge(_)));
        assert_eq!(both_off.diamond, None);
        // The box off screen and the diamond on screen: the diamond stays.
        let some = layout(
            &camera(),
            SIZE,
            &marks_at([9000., 0., 100.], Some([0., 0., 5000.])),
            20.,
        )
        .unwrap();
        assert_eq!(some.diamond, Some([400., 300.]));
    }

    #[test]
    fn only_a_gun_that_cannot_hit_earns_a_diamond() {
        for (status, wanted) in [
            (Readiness::Ready, false),
            (Readiness::GunSlewing, true),
            (Readiness::GunArc, true),
            (Readiness::MaximumRange, true),
            (Readiness::MinimumRange, true),
            (Readiness::TerrainMask, false),
            (Readiness::NoTarget, false),
            (Readiness::Empty, false),
        ] {
            assert_eq!(cannot_hit(status), wanted, "{status:?}");
        }
    }

    #[test]
    fn the_styles_are_square_dot_and_brackets() {
        let tracked = box_strokes(Style::Tracked { friendly: false });
        assert_eq!(tracked.len(), 4);
        assert_eq!(box_strokes(Style::Tracked { friendly: true }).len(), 6);
        let pinned = box_strokes(Style::Pinned);
        assert_eq!(&pinned[..4], &tracked[..]);
        assert_eq!(pinned[4], ((0., 0.), (0., 0.)));
        let free = box_strokes(Style::Free);
        assert_eq!(free.len(), 8);
        // Brackets stay inside the square and never reach its sides' middles.
        for (a, b) in free {
            for (x, y) in [a, b] {
                assert!(x.abs() <= HALF && y.abs() <= HALF);
            }
            assert!(a.0.abs() == HALF && a.1.abs() == HALF);
        }
    }

    #[test]
    fn the_chevron_points_along_its_direction() {
        let [first, second] = chevron_strokes([1., 0.], 8., 4.);
        assert_eq!(first, ((-8., -4.), (0., 0.)));
        assert_eq!(second, ((-8., 4.), (0., 0.)));
        let [up, _] = chevron_strokes([0., -1.], 8., 4.);
        assert!(up.0.1 > 0., "the arm trails behind an upward tip");
    }

    #[test]
    fn an_edge_arrow_slides_in_from_behind_an_instrument_window() {
        let size = [800., 600.];
        // A window in the bottom-left corner, 200 by 200.
        let window = [(0., 400., 200., 200.)];
        let free = edge_point_clear(size, 20., [-1., 0.], &window, 10.);
        assert_eq!(free, [20., 300.], "a side the window does not reach");
        // Down and left lands in the window; the arrow backs up along its
        // ray to just outside it, still on that bearing.
        let (dx, dy) = (-0.6, 0.8);
        let [x, y] = edge_point_clear(size, 20., [dx, dy], &window, 10.);
        assert!(!(x > -10. && x < 210. && y > 390. && y < 610.), "{x},{y}");
        assert!(((x - 400.) / (y - 300.) - dx / dy).abs() < 1e-6);
        // Nothing to hide behind: the plain border point.
        assert_eq!(
            edge_point_clear(size, 20., [dx, dy], &[], 10.),
            edge_point(size, 20., [dx, dy])
        );
    }
    #[test]
    fn the_edge_point_lies_on_the_inset_border() {
        let size = [800., 600.];
        assert_eq!(edge_point(size, 20., [1., 0.]), [780., 300.]);
        assert_eq!(edge_point(size, 20., [0., -1.]), [400., 20.]);
        let [x, y] = edge_point(size, 20., [0.6, 0.8]);
        // The steeper axis reaches its border first.
        assert!((y - 580.).abs() < 1e-9 && x < 780., "{x},{y}");
    }

    mod world {
        use super::*;
        use tore_formats::aircraft::AircraftId;
        use tore_sim::combat::live;
        use tore_world::{
            mission::{MissionSpec, Start},
            seats::{SeatCommand, SeatId, SeatInput},
            test_support::resources::{THEATER, gunship_resources},
            world::{Seating, TickOutput, World},
        };

        /// A synthetic AC-130 at 5,000 feet, stepped `ticks` ticks with
        /// `input` before each, and its readout and configuration.
        fn gunship(
            ticks: u64,
            input: impl Fn(u64) -> SeatInput,
        ) -> (CockpitReadout, Configuration) {
            let mut spec = MissionSpec::new(THEATER, AircraftId::Ac130);
            spec.start = Start::Airborne { altitude_ft: 5_000 };
            let mut world = World::new(&spec, &gunship_resources(), Seating::SinglePlayer).unwrap();
            let mut out = TickOutput::default();
            for tick in 0..ticks {
                let mut frame = input(tick);
                frame.tick = tick;
                world.step(&[frame], &mut out).unwrap();
            }
            let readout = world
                .cockpit_readout(
                    SeatId(0),
                    crate::combat::launcher(&world.cockpits[0].flight),
                )
                .unwrap();
            (readout, world.combat.state.own().configuration().clone())
        }

        #[test]
        fn a_gunship_always_has_a_box() {
            // Free slew in the default view, at every moment from the start.
            for ticks in [1, 3, 120, 600] {
                let (readout, config) = gunship(ticks, |_| SeatInput::default());
                let sight = readout.gunsight.clone().unwrap();
                let marks = marks(&readout, &config, false).expect("one box at every time");
                assert_eq!(marks.style, Style::Free, "tick {ticks}");
                assert_eq!(Some(marks.aim), sight.aim, "tick {ticks}");
            }
        }

        #[test]
        fn a_trained_gun_has_no_diamond_and_a_gun_out_of_range_has_one() {
            // The synthetic guns reach about 5,600 feet; the default view
            // from 5,000 feet looks 11,000 feet away, so they fall short.
            let (mut readout, config) = gunship(600, |_| SeatInput::default());
            let sight = readout.gunsight.clone().unwrap();
            assert_eq!(sight.status[0], Readiness::MaximumRange);
            let short = marks(&readout, &config, false).unwrap();
            let Some(tore_sim::combat::gunship_impact::Impact::Spent { point, .. }) =
                sight.impacts[0]
            else {
                panic!("rounds run out: {:?}", sight.impacts);
            };
            assert_eq!(short.diamond, Some(point));
            // The same readout with its guns trained and in range: only the box.
            readout.gunsight.as_mut().unwrap().status = [Readiness::Ready; 3];
            assert_eq!(marks(&readout, &config, false).unwrap().diamond, None);
        }

        #[test]
        fn guns_still_slewing_earn_a_diamond_at_their_impact() {
            // A tick after the start the guns are far from the default view.
            let (readout, config) = gunship(3, |_| SeatInput::default());
            let sight = readout.gunsight.clone().unwrap();
            let slot = (0..3)
                .find(|slot| sight.impacts[*slot].is_some())
                .expect("a linked gun has a pipper");
            assert!(cannot_hit(sight.status[slot]), "{:?}", sight.status);
            let marks = marks(&readout, &config, false).unwrap();
            assert_eq!(marks.diamond, sight.impacts[slot].map(|i| i.point()));
        }

        #[test]
        fn a_sight_swung_out_of_arc_keeps_its_box_and_gains_a_diamond() {
            // Full deflection aft at the widest step for four seconds: past
            // the C_25's 150 degree limit.
            let (readout, config) = gunship(900, |tick| SeatInput {
                sight: if tick < 480 { [-127, 0] } else { [0, 0] },
                sight_zoom: 1,
                ..SeatInput::default()
            });
            let sight = readout.gunsight.clone().unwrap();
            assert!(
                !(-150f64.to_radians()..=-30f64.to_radians()).contains(&sight.look[0]),
                "the sight looks aft of every arc: {:?}",
                sight.look
            );
            let marks = marks(&readout, &config, false).expect("the box stays");
            let diamond = marks.diamond.expect("rounds cannot reach the box");
            let gap = (0..3)
                .map(|i| (diamond[i] - marks.aim[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            assert!(gap > 100., "the diamond parks away from the box: {gap} ft");
        }

        #[test]
        fn a_pin_and_a_track_take_their_own_box_styles() {
            let (readout, config) = gunship(60, |tick| SeatInput {
                commands: if tick == 5 {
                    vec![SeatCommand::Combat(live::Command::SightPinGround)]
                } else {
                    vec![]
                },
                ..SeatInput::default()
            });
            let mut sight = readout.gunsight.clone().unwrap();
            assert!(matches!(
                sight.sight,
                tore_sim::combat::gunship::Sight::Pinned(_)
            ));
            assert_eq!(
                marks(&readout, &config, false).unwrap().style,
                Style::Pinned
            );
            sight.sight = tore_sim::combat::gunship::Sight::Tracked(7);
            let mut tracked = readout.clone();
            tracked.gunsight = Some(sight);
            for friendly in [false, true] {
                assert_eq!(
                    marks(&tracked, &config, friendly).unwrap().style,
                    Style::Tracked { friendly }
                );
            }
        }

        #[test]
        fn other_aircraft_get_no_box() {
            let state = tore_world::test_support::combat_fixture(false);
            let flight =
                crate::flight::State::new(&tore_world::test_support::profile(), [0., 5000., 0.])
                    .unwrap();
            let readout =
                tore_world::readout::build(&state, 0, crate::combat::launcher(&flight), None, None)
                    .unwrap();
            assert!(readout.gunsight.is_none());
            assert_eq!(marks(&readout, state.own().configuration(), false), None);
        }
    }
}
