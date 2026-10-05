//! Friend or foe cues: which side a target is on (the lock box's X and the IFF
//! squawk), and Show Target Info's labels under every visible aircraft and
//! object. Everything here reads the flight's own copy of the mission, so a
//! networked flight and single player answer the same way
//! ([architecture](../../../docs/ARCHITECTURE.md#friend-or-foe)).
use crate::{
    ai_wings::AiWings,
    camera::Camera,
    seats::{PlaneId, Roster},
    snapshot::{AircraftPose, RenderSnapshot},
    target_window::{Pilot, TargetBrief},
};
use tore_formats::font::Font;
use tore_sim::{
    ai::launch::Side,
    airport::{Allegiance, Scene},
};

/// Who is on which side, as the flight's copy of the mission knows it.
pub struct Sides<'a> {
    pub roster: &'a Roster,
    pub wings: Option<&'a AiWings>,
    pub scene: &'a Scene,
}
impl Sides<'_> {
    /// The side of the plane the screen presents. Single player's flight and a
    /// plane the roster does not list are the friendly side's.
    pub fn viewer(&self, plane: PlaneId) -> Side {
        self.roster
            .plane(plane)
            .map_or(Side::Friendly, |plane| plane.slot.wing.side)
    }
    /// The side of aircraft or runway `id`: a plane the roster lists, an AI
    /// aircraft the wings list, or a runway of an airport that is friendly or
    /// hostile. `None` for anything else, and for neutral and unknown airports.
    pub fn of(&self, id: u32) -> Option<Side> {
        if let Some(plane) = self.roster.plane(PlaneId(id)) {
            return Some(plane.slot.wing.side);
        }
        if let Some(slot) = self.wings.and_then(|wings| wings.slot(id)) {
            return Some(slot.side);
        }
        let runway = self.scene.runway(id)?;
        let airport = self
            .scene
            .airports
            .iter()
            .find(|airport| airport.id == runway.airport)?;
        match airport.allegiance {
            Allegiance::Friendly => Some(Side::Friendly),
            Allegiance::Hostile => Some(Side::Enemy),
            Allegiance::Neutral | Allegiance::Unknown => None,
        }
    }
    /// Whether target `id` is on the presented plane's own side: the lock
    /// box's X, and IFF's Friendly.
    pub fn friendly_to(&self, viewer: PlaneId, id: u32) -> bool {
        self.of(id) == Some(self.viewer(viewer))
    }
}

/// What the IFF squawk (U) answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Iff {
    /// Nothing is designated.
    NoTarget,
    /// The target is on the player's side.
    Friendly,
    /// Anything else (agent decision: the retail manual names only the
    /// Friendly answer).
    NoReply,
}
impl Iff {
    /// The cockpit message line.
    pub fn message(self) -> &'static str {
        match self {
            Iff::NoTarget => "IFF: no target",
            Iff::Friendly => "IFF: Friendly",
            Iff::NoReply => "IFF: no reply",
        }
    }
}
/// The squawk's answer for the displayed `target` of the plane `viewer`.
pub fn iff(sides: &Sides, viewer: PlaneId, target: Option<u32>) -> Iff {
    match target {
        None => Iff::NoTarget,
        Some(id) if sides.friendly_to(viewer, id) => Iff::Friendly,
        Some(_) => Iff::NoReply,
    }
}

/// Show Target Info's text colours: orange, and red for an object that
/// targets the player (the retail manual).
pub const ORANGE: [u8; 3] = [255, 150, 40];
pub const RED: [u8; 3] = [255, 40, 40];
/// How far a label reaches, in feet: ten nautical miles (agent decision; the
/// manual says "visible").
pub const REACH: f64 = 10. * 6076.115;
/// The most labels at once, aircraft first and then the nearest (agent
/// decision), so a crowded airfield stays readable.
pub const MOST: usize = 24;

/// One line of text, drawn with its top left at `at`, in view pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub at: [f64; 2],
}
/// The text below one aircraft or object.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub id: u32,
    pub color: [u8; 3],
    pub lines: Vec<Line>,
}

/// The name an aircraft or ground object is labelled with: a ground object's
/// own name, else the aircraft's exact identity. `None` for an object nothing
/// names.
pub fn identity(pose: &AircraftPose, ground: Option<&str>) -> Option<String> {
    ground
        .map(str::to_owned)
        .or_else(|| pose.aircraft.map(|aircraft| aircraft.label().to_owned()))
        .map(|name| crate::replay::panels::ascii(&name))
}

/// Everything the labels are built from.
pub struct View<'a> {
    pub picture: &'a RenderSnapshot,
    pub camera: &'a Camera,
    /// The flight view's size in pixels.
    pub size: [u32; 2],
    pub font: &'a Font,
    /// One font pixel in view pixels.
    pub scale: f64,
    /// What the AI says about the displayed target: its manoeuvre, and whether
    /// it aims at the player.
    pub brief: Option<&'a TargetBrief>,
}

/// The labels for `scene`, nearest aircraft first. `name` gives an object's
/// identity (see [`identity`]) and `callsign` the callsign of the human flying
/// a plane, in a networked game.
pub fn labels(
    scene: &View,
    name: &dyn Fn(&AircraftPose) -> Option<String>,
    callsign: &dyn Fn(u32) -> Option<String>,
) -> Vec<Label> {
    let eye = scene.camera.position;
    let mut found: Vec<(bool, f64, Label)> = Vec::new();
    for pose in &scene.picture.targets {
        if Some(pose.id) == scene.camera.hidden_target || pose.crashed {
            continue;
        }
        let distance = (0..3)
            .map(|i| (pose.position[i] - eye[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        if distance > REACH {
            continue;
        }
        let Some([x, y]) = scene.camera.project(scene.size, pose.position) else {
            continue;
        };
        let Some(identity) = name(pose) else {
            continue;
        };
        let mut texts = vec![identity];
        let mut color = ORANGE;
        if let Some(brief) = scene.brief.filter(|brief| brief.id == pose.id)
            && let Pilot::Ai {
                activity,
                aims_at_viewer,
                ..
            } = brief.pilot
        {
            texts.push(activity.label().to_owned());
            if aims_at_viewer {
                color = RED;
            }
        }
        if let Some(callsign) = callsign(pose.id) {
            texts.push(crate::replay::panels::ascii(&callsign));
        }
        let height = (scene.font.height as f64 + 1.) * scene.scale;
        let lines = texts
            .into_iter()
            .enumerate()
            .map(|(row, text)| {
                let width =
                    crate::flight_canvas::FlightCanvas::text_width(scene.font, &text, scene.scale);
                Line {
                    at: [
                        (x - width / 2.).round(),
                        (y + (scene.font.height as f64 + 4.) * scene.scale + row as f64 * height)
                            .round(),
                    ],
                    text,
                }
            })
            .collect();
        found.push((
            pose.aircraft.is_none(),
            distance,
            Label {
                id: pose.id,
                color,
                lines,
            },
        ));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    found.truncate(MOST);
    let mut labels: Vec<Label> = found.into_iter().map(|(_, _, label)| label).collect();
    separate(&mut labels, scene.font, scene.scale);
    labels
}

/// A label's box: left, top, width and height in view pixels.
fn bounds(label: &Label, font: &Font, scale: f64) -> [f64; 4] {
    let left = label
        .lines
        .iter()
        .map(|line| line.at[0])
        .fold(f64::MAX, f64::min);
    let top = label
        .lines
        .iter()
        .map(|line| line.at[1])
        .fold(f64::MAX, f64::min);
    let right = label
        .lines
        .iter()
        .map(|line| {
            line.at[0] + crate::flight_canvas::FlightCanvas::text_width(font, &line.text, scale)
        })
        .fold(f64::MIN, f64::max);
    let height = label.lines.len() as f64 * (font.height as f64 + 1.) * scale;
    [left, top, right - left, height]
}

/// Moves each label down past any earlier label it would overprint, so two
/// aircraft that sit close together on the view each keep a readable name.
/// Labels keep their order, so the same picture always stacks the same way.
fn separate(labels: &mut [Label], font: &Font, scale: f64) {
    let gap = 2. * scale;
    for i in 1..labels.len() {
        for _ in 0..16 {
            let [x, y, w, h] = bounds(&labels[i], font, scale);
            let Some(bottom) = labels[..i]
                .iter()
                .map(|other| bounds(other, font, scale))
                .filter(|[ox, oy, ow, oh]| {
                    x < ox + ow && *ox < x + w && y < oy + oh + gap && *oy < y + h + gap
                })
                .map(|[_, oy, _, oh]| oy + oh + gap)
                .reduce(f64::max)
            else {
                break;
            };
            let shift = (bottom - y).round().max(1.);
            for line in &mut labels[i].lines {
                line.at[1] += shift;
            }
        }
    }
}

/// Draws the labels on the flight view, in the HUD's font.
pub fn draw(
    canvas: &mut crate::flight_canvas::FlightCanvas,
    font: &Font,
    scale: f64,
    labels: &[Label],
) {
    for label in labels {
        for line in &label.lines {
            canvas.text(font, &line.text, line.at, scale, label.color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seats::{Pilot as Flier, SeatId, Slot};
    use tore_formats::aircraft::AircraftId;
    use tore_sim::ai::controller::Activity;
    use tore_sim::ai::launch::WingId;

    fn font() -> Font {
        Font {
            height: 8,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 6,
                    pixels: vec![(0, 0)],
                })
                .collect(),
        }
    }
    fn slot(side: Side, index: u8, member: u8) -> Slot {
        Slot {
            wing: WingId { side, index },
            member,
        }
    }
    /// Plane 0 (the viewer, friendly lead), 1 (friendly wingman), 2 and 3
    /// (enemy), 4 (an enemy flown by a human).
    fn roster() -> Roster {
        Roster::with_humans(
            [
                (PlaneId(0), slot(Side::Friendly, 0, 0), SeatId(0), None),
                (PlaneId(4), slot(Side::Enemy, 1, 0), SeatId(1), None),
            ],
            [
                (PlaneId(1), slot(Side::Friendly, 0, 1)),
                (PlaneId(2), slot(Side::Enemy, 0, 0)),
                (PlaneId(3), slot(Side::Enemy, 0, 1)),
            ],
        )
    }
    fn scene() -> tore_sim::airport::Scene {
        Default::default()
    }

    #[test]
    fn the_x_follows_the_presented_planes_side() {
        let roster = roster();
        let scene = scene();
        let sides = Sides {
            roster: &roster,
            wings: None,
            scene: &scene,
        };
        // A friendly viewer: the X is on the friendly wingman only.
        assert!(sides.friendly_to(PlaneId(0), 1));
        assert!(!sides.friendly_to(PlaneId(0), 2));
        // An enemy viewer (plane 4): the X moves to the enemy's aircraft.
        assert!(sides.friendly_to(PlaneId(4), 2));
        assert!(sides.friendly_to(PlaneId(4), 3));
        assert!(!sides.friendly_to(PlaneId(4), 1));
        assert_eq!(sides.viewer(PlaneId(4)), Side::Enemy);
        // Nothing known about an id: no X.
        assert!(!sides.friendly_to(PlaneId(0), 99));
    }

    #[test]
    fn a_plane_the_roster_does_not_list_is_flown_for_the_friendly_side() {
        let roster = Roster::single_player(None, []);
        let scene = scene();
        let sides = Sides {
            roster: &roster,
            wings: None,
            scene: &scene,
        };
        assert_eq!(sides.viewer(PlaneId(0)), Side::Friendly);
        assert_eq!(sides.viewer(PlaneId(77)), Side::Friendly);
    }

    #[test]
    fn iff_answers_friendly_no_reply_and_no_target() {
        let roster = roster();
        let scene = scene();
        let sides = Sides {
            roster: &roster,
            wings: None,
            scene: &scene,
        };
        assert_eq!(iff(&sides, PlaneId(0), Some(1)), Iff::Friendly);
        assert_eq!(iff(&sides, PlaneId(0), Some(2)), Iff::NoReply);
        assert_eq!(iff(&sides, PlaneId(0), Some(99)), Iff::NoReply);
        assert_eq!(iff(&sides, PlaneId(0), None), Iff::NoTarget);
        // The enemy's own squawk answers for its side.
        assert_eq!(iff(&sides, PlaneId(4), Some(2)), Iff::Friendly);
        assert_eq!(iff(&sides, PlaneId(4), Some(1)), Iff::NoReply);
        assert_eq!(Iff::Friendly.message(), "IFF: Friendly");
        assert_eq!(Iff::NoReply.message(), "IFF: no reply");
        assert_eq!(Iff::NoTarget.message(), "IFF: no target");
    }

    fn pose(id: u32, z: f64, aircraft: Option<AircraftId>) -> AircraftPose {
        AircraftPose {
            id,
            aircraft,
            position: [0., 0., z],
            airborne: true,
            ..Default::default()
        }
    }
    fn camera() -> Camera {
        let mut camera = Camera::new();
        camera.position = [0.; 3];
        camera.yaw = 0.;
        camera.pitch = 0.;
        camera.roll = 0.;
        camera.zoom = 1.;
        camera
    }
    fn names(pose: &AircraftPose) -> Option<String> {
        match pose.id {
            9 => Some("Hangar".into()),
            _ => pose.aircraft.map(|aircraft| aircraft.label().to_owned()),
        }
    }
    fn built(picture: &RenderSnapshot, brief: Option<&TargetBrief>) -> Vec<Label> {
        let font = font();
        let camera = camera();
        labels(
            &View {
                picture,
                camera: &camera,
                size: [640, 480],
                font: &font,
                scale: 1.,
                brief,
            },
            &names,
            &|plane| (plane == 4).then(|| "Hawk".to_owned()),
        )
    }

    #[test]
    fn labels_name_every_visible_aircraft_and_object_below_it() {
        let picture = RenderSnapshot {
            targets: vec![
                pose(2, 6_000., Some(AircraftId::F18)),
                pose(9, 3_000., None),
                // Behind the camera, out of reach and unnamed: no label.
                pose(3, -5_000., Some(AircraftId::F18)),
                pose(5, 10. * 6076.115 + 500., Some(AircraftId::F18)),
                pose(10, 3_000., None),
            ],
            ..Default::default()
        };
        let labels = built(&picture, None);
        let ids: Vec<_> = labels.iter().map(|label| label.id).collect();
        // Aircraft come first, then ground objects, each by distance.
        assert_eq!(ids, [2, 9]);
        let aircraft = &labels[0];
        assert_eq!(aircraft.color, ORANGE);
        assert_eq!(aircraft.lines.len(), 1);
        assert_eq!(aircraft.lines[0].text, AircraftId::F18.label());
        // Centred on the aircraft's screen point and below it.
        let [x, y] = camera().project([640, 480], [0., 0., 6_000.]).unwrap();
        let width =
            crate::flight_canvas::FlightCanvas::text_width(&font(), &aircraft.lines[0].text, 1.);
        assert_eq!(aircraft.lines[0].at[0], (x - width / 2.).round());
        assert!(aircraft.lines[0].at[1] > y);
        assert_eq!(labels[1].lines[0].text, "Hangar");
    }

    #[test]
    fn the_displayed_targets_manoeuvre_follows_its_name_in_red_when_it_aims_at_you() {
        let picture = RenderSnapshot {
            targets: vec![pose(2, 6_000., Some(AircraftId::F18))],
            ..Default::default()
        };
        let brief = |aims_at_viewer| TargetBrief {
            id: 2,
            objective: None,
            pilot: Pilot::Ai {
                activity: Activity::Attacking,
                skill: 2,
                aims_at_viewer,
            },
        };
        let calm = built(&picture, Some(&brief(false)));
        assert_eq!(calm[0].color, ORANGE);
        assert_eq!(calm[0].lines.len(), 2);
        assert_eq!(calm[0].lines[1].text, Activity::Attacking.label());
        let aimed = built(&picture, Some(&brief(true)));
        assert_eq!(aimed[0].color, RED);
        // A brief about another aircraft changes nothing here.
        let other = TargetBrief {
            id: 3,
            ..brief(true)
        };
        assert_eq!(built(&picture, Some(&other))[0].lines.len(), 1);
        // The lines stack downward.
        assert!(aimed[0].lines[1].at[1] > aimed[0].lines[0].at[1]);
    }

    #[test]
    fn a_human_flown_aircraft_shows_its_callsign_beneath() {
        let picture = RenderSnapshot {
            targets: vec![pose(4, 6_000., Some(AircraftId::F18))],
            ..Default::default()
        };
        let labels = built(&picture, None);
        let texts: Vec<_> = labels[0].lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, [AircraftId::F18.label(), "Hawk"]);
        // The roster knows who flies it; the fixture's human is plane 4.
        assert!(matches!(
            roster().plane(PlaneId(4)).map(|p| p.pilot),
            Some(Flier::Human(SeatId(1)))
        ));
    }

    #[test]
    fn no_more_than_the_most_labels_are_drawn_and_aircraft_win() {
        let mut targets: Vec<_> = (100..140)
            .map(|id| pose(id, 2_000. + f64::from(id), None))
            .collect();
        targets.push(pose(2, 9_000., Some(AircraftId::F18)));
        let picture = RenderSnapshot {
            targets,
            ..Default::default()
        };
        let font = font();
        let camera = camera();
        let labels = labels(
            &View {
                picture: &picture,
                camera: &camera,
                size: [640, 480],
                font: &font,
                scale: 1.,
                brief: None,
            },
            &|pose| Some(format!("Object {}", pose.id)),
            &|_| None,
        );
        assert_eq!(labels.len(), MOST);
        // The aircraft, though farthest, comes first.
        assert_eq!(labels[0].id, 2);
    }

    #[test]
    fn labels_draw_in_their_colour_with_a_shadow() {
        let mut canvas = crate::flight_canvas::FlightCanvas::default();
        canvas.size = [64, 48];
        canvas.pixels = vec![0; 64 * 48 * 4];
        let label = Label {
            id: 1,
            color: RED,
            lines: vec![Line {
                text: "A".into(),
                at: [10., 10.],
            }],
        };
        draw(&mut canvas, &font(), 1., &[label]);
        let lit: Vec<_> = canvas
            .pixels
            .chunks_exact(4)
            .filter(|p| p[3] != 0)
            .collect();
        assert!(lit.iter().any(|p| p[..3] == RED));
        assert!(lit.iter().any(|p| p[..3] == [0, 0, 0]));
    }

    #[test]
    fn a_headless_render_puts_each_labels_text_under_its_aircraft_in_its_colour() {
        let picture = RenderSnapshot {
            targets: vec![
                pose(2, 6_000., Some(AircraftId::F18)),
                // Off to the right, on its own, and aimed at the player.
                AircraftPose {
                    position: [900., 0., 6_000.],
                    ..pose(3, 6_000., Some(AircraftId::F18))
                },
            ],
            ..Default::default()
        };
        let brief = TargetBrief {
            id: 3,
            objective: None,
            pilot: Pilot::Ai {
                activity: Activity::Attacking,
                skill: 2,
                aims_at_viewer: true,
            },
        };
        let labels = built(&picture, Some(&brief));
        let font = font();
        let mut canvas = crate::flight_canvas::FlightCanvas::default();
        canvas.size = [640, 480];
        canvas.pixels = vec![0; 640 * 480 * 4];
        draw(&mut canvas, &font, 1., &labels);
        let camera = camera();
        for (label, color) in labels.iter().zip([ORANGE, RED]) {
            let [x, y] = camera
                .project(
                    [640, 480],
                    picture.target(label.id).expect("a target").position,
                )
                .expect("on screen");
            // Every drawn pixel of this colour lies below the aircraft and
            // within a label's width of it; none above.
            let mut lit = 0;
            for (i, p) in canvas.pixels.chunks_exact(4).enumerate() {
                if p[3] != 0 && p[..3] == color {
                    let (px, py) = ((i % 640) as f64, (i / 640) as f64);
                    if (px - x).abs() < 80. {
                        assert!(py > y, "{} at {px},{py} is above {x},{y}", label.id);
                        lit += 1;
                    }
                }
            }
            assert!(lit > 0, "label {} drew nothing", label.id);
        }
        // The aimed one's second line is its manoeuvre.
        assert_eq!(labels[1].lines[1].text, Activity::Attacking.label());
    }

    #[test]
    fn a_runway_is_on_the_side_of_its_airport_and_a_neutral_one_on_none() {
        use tore_sim::airport::{Airport, OrientedBox, Runway};
        let airport = |id, allegiance| Airport {
            id,
            name: format!("Field {id}"),
            runway_objects: vec![100 + id],
            allegiance,
            neutral_permission: false,
        };
        let runway = |airport: u32| Runway {
            object: 100 + airport,
            airport,
            name: "09".into(),
            surface: OrientedBox {
                center: [0.; 3],
                half: [1.; 3],
                heading: 0.,
                pitch: 0.,
                bank: 0.,
            },
            approach_center: [0.; 3],
            elevation_ft: 0.,
            heading: 0.,
            length_ft: 8_000.,
        };
        let scene = tore_sim::airport::Scene {
            objects: vec![],
            runways: vec![runway(1), runway(2), runway(3)],
            airports: vec![
                airport(1, Allegiance::Friendly),
                airport(2, Allegiance::Hostile),
                airport(3, Allegiance::Neutral),
            ],
        };
        let roster = roster();
        let sides = Sides {
            roster: &roster,
            wings: None,
            scene: &scene,
        };
        assert_eq!(sides.of(101), Some(Side::Friendly));
        assert_eq!(sides.of(102), Some(Side::Enemy));
        assert_eq!(sides.of(103), None);
        // The X for a friendly viewer sits on the friendly field, an
        // enemy viewer's on the hostile one.
        assert!(sides.friendly_to(PlaneId(0), 101));
        assert!(!sides.friendly_to(PlaneId(0), 102));
        assert!(sides.friendly_to(PlaneId(4), 102));
        assert!(!sides.friendly_to(PlaneId(4), 103));
        assert_eq!(iff(&sides, PlaneId(4), Some(102)), Iff::Friendly);
    }

    #[test]
    fn labels_that_would_overprint_each_other_stack_downward() {
        // Two aircraft almost on the same line of sight, and one far to the side.
        let picture = RenderSnapshot {
            targets: vec![
                pose(2, 6_000., Some(AircraftId::F18)),
                AircraftPose {
                    position: [5., 0., 6_050.],
                    ..pose(3, 6_050., Some(AircraftId::F18))
                },
                AircraftPose {
                    position: [2_000., 0., 6_000.],
                    ..pose(5, 6_000., Some(AircraftId::F18))
                },
            ],
            ..Default::default()
        };
        let labels = built(&picture, None);
        assert_eq!(labels.len(), 3);
        let font = font();
        let boxes: Vec<_> = labels.iter().map(|l| bounds(l, &font, 1.)).collect();
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                let apart = a[0] + a[2] <= b[0]
                    || b[0] + b[2] <= a[0]
                    || a[1] + a[3] <= b[1]
                    || b[1] + b[3] <= a[1];
                assert!(apart, "{a:?} overprints {b:?}");
            }
        }
        // The nearer aircraft keeps its place; the next is pushed below it.
        assert!(boxes[1][1] > boxes[0][1]);
        // The one off to the side is not moved.
        let [_, y] = camera().project([640, 480], [2_000., 0., 6_000.]).unwrap();
        assert_eq!(
            labels[2].lines[0].at[1],
            (y + (font.height as f64 + 4.)).round()
        );
    }
}
