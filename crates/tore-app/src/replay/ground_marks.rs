//! The air-to-ground marks of the replay viewer (slice FX-R, 2026-10-11, John:
//! "replay drawing laser spots might be useful to troubleshoot failed
//! drops"). A recording of a bombing pass holds laser, designation and guided
//! weapon events ([`vocab`](tore_replay::vocab), REPLAYS.md "Air-to-ground
//! events"); this reads them into a timeline and, for any tick, says what to
//! draw in the 3D view:
//!
//! - each laser spot, in the colour of whose it is (the recording's player,
//!   a buddy, anyone else by side), with a line from the lasing aircraft
//!   while it lases, and a hollow mark at the last spot while it is masked;
//! - each designated ground object or pinned point;
//! - each guided weapon's aim point with a line from the weapon, and its
//!   mode: guiding, waiting for a spot, or blind with the reason it gave;
//! - a bomb's impact for twenty seconds, with how far it landed from the
//!   point it was steering for and from the designated target.
//!
//! The mapping from events to marks is plain data (no renderer), so it is
//! tested without a display. Colours, shapes and sizes are agent choices.
use crate::camera::Camera;
use crate::replay::trails;
use std::collections::BTreeMap;
use tore_replay::{
    Side, TimedEvent,
    vocab::{designate, field, guide, kind, laser},
};

/// How long a bomb's impact stays on the view after it lands: twenty seconds.
pub const IMPACT_SHOWN_TICKS: u64 = 20 * 120;

/// The recording player's own laser.
pub const OWN: [u8; 3] = [110, 255, 110];
/// A friendly buddy's laser.
pub const BUDDY: [u8; 3] = [90, 230, 235];
/// A laser weapon released with no spot yet.
pub const WAITING: [u8; 3] = [255, 225, 80];
/// A weapon that has gone blind, and the point it last steered for.
pub const BLIND: [u8; 3] = [255, 140, 40];
/// A JDAM's stored point.
pub const STORED: [u8; 3] = [240, 240, 240];
/// Where a bomb landed.
pub const IMPACT: [u8; 3] = [255, 110, 255];

/// A laser designator's state as the events give it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Off,
    Ready,
    Lasing,
    Masked,
}

impl Phase {
    fn of(state: &str) -> Self {
        match state {
            laser::LASING => Self::Lasing,
            laser::MASKED => Self::Masked,
            laser::READY => Self::Ready,
            _ => Self::Off,
        }
    }
}

/// One change of an aircraft's laser, or a move of its spot.
#[derive(Clone, Debug, PartialEq)]
struct LaserKey {
    tick: u64,
    phase: Phase,
    mask: Option<String>,
    spot: Option<[f64; 3]>,
}

/// A guided weapon's mode, as `weapon.guided` gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Waiting,
    Guiding,
    Blind,
}

impl Mode {
    fn of(name: &str) -> Self {
        match name {
            guide::GUIDING => Self::Guiding,
            guide::BLIND => Self::Blind,
            _ => Self::Waiting,
        }
    }
}

/// How a guided weapon is guided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guidance {
    Laser,
    Coordinate,
    Command,
}

impl Guidance {
    fn of(name: &str) -> Self {
        match name {
            tore_replay::vocab::guidance::COORDINATE => Self::Coordinate,
            tore_replay::vocab::guidance::COMMAND => Self::Command,
            _ => Self::Laser,
        }
    }
}

/// A change of a guided weapon's mode.
#[derive(Clone, Debug, PartialEq)]
struct ModeKey {
    tick: u64,
    mode: Mode,
    spot_owner: Option<u32>,
    own: bool,
    reason: String,
    target: Option<u32>,
}

/// Where a bomb or guided weapon landed.
#[derive(Clone, Debug, PartialEq)]
struct Landing {
    tick: u64,
    at: [f64; 3],
    aim: Option<[f64; 3]>,
    miss_ft: Option<f64>,
    target_ft: Option<f64>,
    blind: bool,
}

/// What the events say of one bomb or guided weapon.
#[derive(Clone, Debug, Default, PartialEq)]
struct Flight {
    owner: u32,
    guidance: Option<Guidance>,
    modes: Vec<ModeKey>,
    /// The point it steered for, in tick order.
    aims: Vec<(u64, [f64; 3])>,
    /// The JDAM's point stored at its release.
    stored: Option<[f64; 3]>,
    landing: Option<Landing>,
    dud: bool,
}

/// What an aircraft designated.
#[derive(Clone, Debug, PartialEq)]
struct Designated {
    object: Option<u32>,
    at: Option<[f64; 3]>,
    channel: String,
}

/// Every mark of a recording, ready to ask for any tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Marks {
    lasers: BTreeMap<u32, Vec<LaserKey>>,
    flights: BTreeMap<u32, Flight>,
    designations: BTreeMap<u32, Vec<(u64, Option<Designated>)>>,
}

fn point(event: &tore_replay::Event) -> Option<[f64; 3]> {
    let [x, y, z] = field::POSITION;
    Some([event.num(x)?, event.num(y)?, event.num(z)?])
}

fn aim(event: &tore_replay::Event) -> Option<[f64; 3]> {
    let [x, y, z] = field::AIM;
    Some([event.num(x)?, event.num(y)?, event.num(z)?])
}

impl Marks {
    /// The marks of a recording's events, which are in tick order. A
    /// recording from before the air-to-ground round has none; one from
    /// before FX-R has the spot where the laser came on and the point where a
    /// weapon started guiding, and no landings.
    pub fn from_events(events: &[TimedEvent]) -> Self {
        let mut marks = Self::default();
        for entry in events {
            let (tick, event) = (entry.tick, &entry.event);
            let Some(subject) = event.subject else {
                continue;
            };
            match event.kind.as_str() {
                kind::PLAYER_LASER => {
                    marks.lasers.entry(subject).or_default().push(LaserKey {
                        tick,
                        phase: Phase::of(event.string(field::STATE).unwrap_or_default()),
                        mask: event.string(field::MASK).map(str::to_owned),
                        spot: point(event),
                    });
                }
                kind::PLAYER_LASER_SPOT => {
                    marks.lasers.entry(subject).or_default().push(LaserKey {
                        tick,
                        phase: Phase::Lasing,
                        mask: None,
                        spot: point(event),
                    });
                }
                kind::PLAYER_DESIGNATE => {
                    let channel = event.string(field::CHANNEL).unwrap_or_default();
                    let now = (channel != designate::CLEARED).then(|| Designated {
                        object: event.object,
                        at: point(event),
                        channel: channel.to_owned(),
                    });
                    marks
                        .designations
                        .entry(subject)
                        .or_default()
                        .push((tick, now));
                }
                kind::WEAPON_GUIDED => {
                    let Some(id) = event.id(field::PROJECTILE) else {
                        continue;
                    };
                    let flight = marks.flights.entry(id).or_default();
                    flight.owner = subject;
                    flight.guidance = Some(Guidance::of(
                        event.string(field::GUIDANCE).unwrap_or_default(),
                    ));
                    let mode = Mode::of(event.string(field::MODE).unwrap_or_default());
                    flight.modes.push(ModeKey {
                        tick,
                        mode,
                        spot_owner: event.id(field::SPOT_OWNER),
                        own: event.flag(field::OWN) == Some(true),
                        reason: event.string(field::REASON).unwrap_or_default().to_owned(),
                        target: event.object,
                    });
                    if let Some(at) = point(event) {
                        flight.aims.push((tick, at));
                        if event.flag(field::STORED) == Some(true) {
                            flight.stored = Some(at);
                        }
                    }
                }
                kind::WEAPON_AIM => {
                    if let (Some(id), Some(at)) = (event.id(field::PROJECTILE), point(event)) {
                        let flight = marks.flights.entry(id).or_default();
                        flight.owner = subject;
                        flight.aims.push((tick, at));
                    }
                }
                kind::WEAPON_IMPACT => {
                    if let (Some(id), Some(at)) = (event.id(field::PROJECTILE), point(event)) {
                        let flight = marks.flights.entry(id).or_default();
                        flight.owner = subject;
                        flight.landing = Some(Landing {
                            tick,
                            at,
                            aim: aim(event),
                            miss_ft: event.num(field::MISS_FT),
                            target_ft: event.num(field::TARGET_FT),
                            blind: event.string(field::MODE) == Some(guide::BLIND),
                        });
                    }
                }
                kind::WEAPON_DUD => {
                    if let Some(id) = event.id(field::PROJECTILE) {
                        marks.flights.entry(id).or_default().dud = true;
                    }
                }
                _ => {}
            }
        }
        marks
    }

    /// Whether `owner`'s laser shows in the recording at all: it has a
    /// laser sight (a pod or a built-in designator).
    pub fn lased(&self, owner: u32) -> bool {
        self.lasers.contains_key(&owner)
    }

    /// `owner`'s laser at `tick` as a cockpit says it (LASE, LSR RDY, MASK
    /// or OFF) with why it is masked; `None` before its first entry.
    pub fn laser_state(&self, owner: u32, tick: u64) -> Option<(&'static str, Option<String>)> {
        let keys = self.lasers.get(&owner)?;
        let at = keys.partition_point(|k| k.tick <= tick).checked_sub(1)?;
        let key = &keys[at];
        Some(match key.phase {
            Phase::Lasing => ("LASE", None),
            Phase::Ready => ("LSR RDY", None),
            Phase::Masked => ("MASK", key.mask.clone()),
            Phase::Off => ("OFF", None),
        })
    }

    /// `owner`'s guided weapons and bombs with a recorded landing, each with
    /// the tick it was released (its first mode change, else its first aim)
    /// and the tick it landed, in projectile order.
    pub fn timeline_of(&self, owner: u32) -> Vec<Timed> {
        self.flights
            .iter()
            .filter(|(_, f)| f.owner == owner)
            .filter_map(|(&projectile, f)| {
                let released = f
                    .modes
                    .first()
                    .map(|m| m.tick)
                    .or_else(|| f.aims.first().map(|a| a.0))?;
                Some(Timed {
                    projectile,
                    released,
                    landed: f.landing.as_ref().map(|l| l.tick),
                    guided: !f.modes.is_empty(),
                })
            })
            .collect()
    }

    /// Whether the recording has anything to draw: the overlay is on by
    /// default only for one that does.
    pub fn any(&self) -> bool {
        !(self.lasers.is_empty() && self.flights.is_empty() && self.designations.is_empty())
    }

    /// What is on the view at `tick`.
    pub fn scene(&self, tick: u64) -> Scene {
        let mut scene = Scene::default();
        for (&owner, keys) in &self.lasers {
            let Some(at) = keys.partition_point(|k| k.tick <= tick).checked_sub(1) else {
                continue;
            };
            let key = &keys[at];
            // The spot as far as the events follow it: the newest one up to
            // this key.
            let spot = keys[..=at].iter().rev().find_map(|k| k.spot);
            match (key.phase, spot) {
                (Phase::Lasing, Some(spot)) => scene.spots.push(Spot {
                    owner,
                    at: spot,
                    lasing: true,
                    mask: None,
                }),
                (Phase::Masked, Some(spot)) => scene.spots.push(Spot {
                    owner,
                    at: spot,
                    lasing: false,
                    mask: key.mask.clone(),
                }),
                _ => {}
            }
        }
        for (&owner, changes) in &self.designations {
            let Some(at) = changes.partition_point(|(t, _)| *t <= tick).checked_sub(1) else {
                continue;
            };
            if let Some(now) = &changes[at].1 {
                scene.designations.push(Designation {
                    aircraft: owner,
                    object: now.object,
                    at: now.at,
                    channel: now.channel.clone(),
                });
            }
        }
        for (&projectile, flight) in &self.flights {
            if let Some(landing) = &flight.landing
                && landing.tick <= tick
            {
                if tick - landing.tick <= IMPACT_SHOWN_TICKS {
                    scene.impacts.push(Impact {
                        projectile,
                        owner: flight.owner,
                        at: landing.at,
                        aim: landing.aim,
                        miss_ft: landing.miss_ft,
                        target_ft: landing.target_ft,
                        blind: landing.blind,
                        dud: flight.dud,
                    });
                }
                continue;
            }
            let Some(at) = flight
                .modes
                .partition_point(|m| m.tick <= tick)
                .checked_sub(1)
            else {
                continue;
            };
            let key = &flight.modes[at];
            let guidance = flight.guidance.unwrap_or(Guidance::Laser);
            let last = flight
                .aims
                .partition_point(|(t, _)| *t <= tick)
                .checked_sub(1)
                .map(|i| flight.aims[i].1);
            scene.weapons.push(Weapon {
                projectile,
                owner: flight.owner,
                guidance,
                mode: key.mode,
                aim: if key.mode == Mode::Waiting {
                    None
                } else {
                    last
                },
                spot_owner: key.spot_owner,
                own: key.own,
                target: key.target,
                reason: key.reason.clone(),
            });
        }
        scene
    }
}

/// One of an aircraft's guided weapons on the timeline: when it left and
/// when it landed (the Sensor View's time to impact).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timed {
    pub projectile: u32,
    pub released: u64,
    pub landed: Option<u64>,
    /// It has guidance modes (a laser bomb, a JDAM, an AT-2).
    pub guided: bool,
}

/// A laser spot on the view.
#[derive(Clone, Debug, PartialEq)]
pub struct Spot {
    /// The lasing aircraft.
    pub owner: u32,
    pub at: [f64; 3],
    /// On the target; else masked, and this is where it last was.
    pub lasing: bool,
    pub mask: Option<String>,
}

/// A designated ground object, or pinned point.
#[derive(Clone, Debug, PartialEq)]
pub struct Designation {
    pub aircraft: u32,
    pub object: Option<u32>,
    pub at: Option<[f64; 3]>,
    pub channel: String,
}

/// A guided weapon in flight.
#[derive(Clone, Debug, PartialEq)]
pub struct Weapon {
    pub projectile: u32,
    pub owner: u32,
    pub guidance: Guidance,
    pub mode: Mode,
    /// The point it steers for now, or last steered for.
    pub aim: Option<[f64; 3]>,
    pub spot_owner: Option<u32>,
    pub own: bool,
    /// The object an AT-2 is steered at.
    pub target: Option<u32>,
    /// The reason of its last change of mode, in words.
    pub reason: String,
}

/// A bomb that has landed.
#[derive(Clone, Debug, PartialEq)]
pub struct Impact {
    pub projectile: u32,
    pub owner: u32,
    pub at: [f64; 3],
    pub aim: Option<[f64; 3]>,
    pub miss_ft: Option<f64>,
    pub target_ft: Option<f64>,
    pub blind: bool,
    pub dud: bool,
}

impl Impact {
    /// The words beside the mark.
    pub fn label(&self) -> String {
        let mut text = if self.dud {
            "DUD".to_owned()
        } else if self.blind {
            "BLIND IMPACT".to_owned()
        } else {
            "IMPACT".to_owned()
        };
        if let Some(miss) = self.miss_ft {
            text.push_str(&format!(" {} FT FROM AIM", feet(miss)));
        }
        if let Some(target) = self.target_ft {
            let sep = if self.miss_ft.is_some() { "," } else { "" };
            text.push_str(&format!("{sep} {} FT FROM TARGET", feet(target)));
        }
        text
    }
}

fn feet(v: f64) -> String {
    crate::replay::trees::thousands(v)
}

/// Everything on the view at one tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub spots: Vec<Spot>,
    pub designations: Vec<Designation>,
    pub weapons: Vec<Weapon>,
    pub impacts: Vec<Impact>,
}

/// Where things are in the picture being drawn.
pub struct Positions<'a> {
    pub aircraft: &'a dyn Fn(u32) -> Option<[f64; 3]>,
    pub projectile: &'a dyn Fn(u32) -> Option<[f64; 3]>,
    pub object: &'a dyn Fn(u32) -> Option<[f64; 3]>,
}

/// Whose is whose, for colours and words.
pub struct Who<'a> {
    /// The recording's player: its laser is "your laser".
    pub player: u32,
    pub side: &'a dyn Fn(u32) -> Side,
    pub name: &'a dyn Fn(u32) -> String,
}

impl Who<'_> {
    fn color(&self, aircraft: u32) -> [u8; 3] {
        if aircraft == self.player {
            OWN
        } else {
            // Only same-side aircraft share a spot, so one with no side
            // recorded is a buddy too.
            match (self.side)(aircraft) {
                Side::Enemy => trails::side_color(Side::Enemy),
                Side::Neutral => trails::side_color(Side::Neutral),
                Side::Friendly | Side::Unknown => BUDDY,
            }
        }
    }

    /// "Your", or "Friendly 1-2's".
    fn possessive(&self, aircraft: u32) -> String {
        if aircraft == self.player {
            "Your".to_owned()
        } else {
            format!("{}'s", (self.name)(aircraft))
        }
    }
}

/// The shapes drawn at a point, a fixed size on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A laser spot: a cross in a ring.
    Spot,
    /// A hollow ring: a spot that is masked, or a weapon's aim.
    Ring,
    Diamond,
    Square,
    /// A landing.
    Impact,
}

/// One thing to draw.
#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    Line {
        from: [f64; 3],
        to: [f64; 3],
        color: [u8; 3],
        dashed: bool,
    },
    Glyph {
        at: [f64; 3],
        shape: Shape,
        color: [u8; 3],
    },
    Text {
        at: [f64; 3],
        text: String,
        color: [u8; 3],
    },
}

/// What to draw for `scene`, given where things are.
pub fn draw(scene: &Scene, positions: &Positions, who: &Who) -> Vec<Prim> {
    let mut out = Vec::new();
    for spot in &scene.spots {
        let color = who.color(spot.owner);
        let name = if spot.owner == who.player {
            "YOUR LASER".to_owned()
        } else {
            format!("{} LASER", (who.name)(spot.owner).to_uppercase())
        };
        if let Some(from) = (positions.aircraft)(spot.owner) {
            out.push(Prim::Line {
                from,
                to: spot.at,
                color,
                dashed: !spot.lasing,
            });
        }
        if spot.lasing {
            out.push(Prim::Glyph {
                at: spot.at,
                shape: Shape::Spot,
                color,
            });
            out.push(Prim::Text {
                at: spot.at,
                text: name,
                color,
            });
        } else {
            let cause = spot
                .mask
                .as_deref()
                .map(|m| format!(": {m}"))
                .unwrap_or_default();
            out.push(Prim::Glyph {
                at: spot.at,
                shape: Shape::Ring,
                color: dim(color),
            });
            out.push(Prim::Text {
                at: spot.at,
                text: format!("{name} MASKED{}", cause.to_uppercase()),
                color: dim(color),
            });
        }
    }
    for designation in &scene.designations {
        let at = designation
            .object
            .and_then(|id| (positions.object)(id))
            .or(designation.at);
        let Some(at) = at else { continue };
        let color = trails::side_color((who.side)(designation.aircraft));
        out.push(Prim::Glyph {
            at,
            shape: Shape::Square,
            color,
        });
        let owner = if designation.aircraft == who.player {
            "YOUR".to_owned()
        } else {
            format!("{}'s", (who.name)(designation.aircraft).to_uppercase())
        };
        out.push(Prim::Text {
            at,
            text: format!("{owner} TARGET ({})", designation.channel.to_uppercase()),
            color,
        });
    }
    for weapon in &scene.weapons {
        let Some(from) = (positions.projectile)(weapon.projectile) else {
            continue;
        };
        // An AT-2 is steered at an object, wherever it stands now.
        let aim = weapon
            .target
            .and_then(|id| (positions.object)(id))
            .filter(|_| weapon.guidance == Guidance::Command)
            .or(weapon.aim);
        let spot_color = weapon.spot_owner.map(|owner| who.color(owner));
        let (color, words) = match weapon.mode {
            Mode::Waiting => (WAITING, "WAITING: NO LASER SPOT YET".to_owned()),
            Mode::Guiding => match weapon.guidance {
                Guidance::Laser => {
                    let follows = match weapon.spot_owner {
                        Some(owner) if weapon.own || owner == who.player => "YOUR LASER".to_owned(),
                        Some(owner) => format!("{} LASER", who.possessive(owner).to_uppercase()),
                        None => "A LASER SPOT".to_owned(),
                    };
                    (spot_color.unwrap_or(OWN), format!("GUIDING ON {follows}"))
                }
                Guidance::Coordinate => (STORED, "GUIDING TO STORED POINT".to_owned()),
                Guidance::Command => (
                    trails::side_color((who.side)(weapon.owner)),
                    "GUIDING: STEERED BY LAUNCHER".to_owned(),
                ),
            },
            Mode::Blind => {
                let why = if weapon.reason.is_empty() {
                    String::new()
                } else {
                    format!(": {}", weapon.reason.to_uppercase())
                };
                (BLIND, format!("BLIND{why}"))
            }
        };
        if let Some(aim) = aim {
            out.push(Prim::Line {
                from,
                to: aim,
                color,
                dashed: weapon.mode == Mode::Blind,
            });
            out.push(Prim::Glyph {
                at: aim,
                shape: match (weapon.mode, weapon.guidance) {
                    (Mode::Guiding, Guidance::Coordinate) | (Mode::Blind, _) => Shape::Diamond,
                    _ => Shape::Ring,
                },
                color,
            });
        }
        out.push(Prim::Text {
            at: from,
            text: words,
            color,
        });
    }
    for impact in &scene.impacts {
        if let Some(aim) = impact.aim {
            out.push(Prim::Line {
                from: impact.at,
                to: aim,
                color: IMPACT,
                dashed: true,
            });
            out.push(Prim::Glyph {
                at: aim,
                shape: Shape::Diamond,
                color: IMPACT,
            });
        }
        out.push(Prim::Glyph {
            at: impact.at,
            shape: Shape::Impact,
            color: IMPACT,
        });
        out.push(Prim::Text {
            at: impact.at,
            text: impact.label(),
            color: IMPACT,
        });
    }
    out
}

/// A colour half way to grey, for a mark that is not live.
fn dim(color: [u8; 3]) -> [u8; 3] {
    color.map(|c| ((u16::from(c) + 110) / 2) as u8)
}

/// Dashes in a dashed line: the same however long it is.
const DASHES: usize = 24;
/// The width of a mark's line, in pixels of a 480-line view.
const LINE_WIDTH: f64 = 2.;

fn mix(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// The corners of a shape of radius `r` pixels, as lines of (right, up)
/// pixel offsets.
fn outline(shape: Shape, r: f64) -> Vec<Vec<[f64; 2]>> {
    let ring = |radius: f64| -> Vec<[f64; 2]> {
        (0..=16)
            .map(|i| {
                let a = i as f64 / 16. * std::f64::consts::TAU;
                [radius * a.cos(), radius * a.sin()]
            })
            .collect()
    };
    match shape {
        Shape::Spot => vec![
            vec![[-r, 0.], [r, 0.]],
            vec![[0., -r], [0., r]],
            ring(r * 0.6),
        ],
        Shape::Ring => vec![ring(r * 0.6)],
        Shape::Diamond => vec![vec![[0., r], [r, 0.], [0., -r], [-r, 0.], [0., r]]],
        Shape::Square => vec![vec![[-r, -r], [r, -r], [r, r], [-r, r], [-r, -r]]],
        Shape::Impact => vec![
            vec![[-r, -r], [r, r]],
            vec![[-r, r], [r, -r]],
            ring(r * 1.3),
        ],
    }
}

/// Radius of a shape in pixels of a 480-line view.
fn radius(shape: Shape) -> f64 {
    match shape {
        Shape::Spot => 16.,
        Shape::Ring => 11.,
        Shape::Diamond => 9.,
        Shape::Square => 12.,
        Shape::Impact => 11.,
    }
}

/// Appends camera-facing ribbons for `prims` to combat geometry (the
/// vertex format of [`trails::ribbon`]): lines and shapes a fixed width and
/// size on screen on a view `height` pixels tall. Text is not drawn here.
/// Combat geometry is relative to the render `origin`, so every point and
/// the eye are moved by it first.
pub fn geometry(
    prims: &[Prim],
    camera: &Camera,
    origin: [f64; 3],
    height: f64,
    out: &mut Vec<f32>,
) {
    let shift = |p: [f64; 3]| trails::relative(p, origin);
    let relative = trails::relative_camera(camera, origin);
    let camera = &relative;
    let prims: Vec<Prim> = prims
        .iter()
        .map(|prim| match prim.clone() {
            Prim::Line {
                from,
                to,
                color,
                dashed,
            } => Prim::Line {
                from: shift(from),
                to: shift(to),
                color,
                dashed,
            },
            Prim::Glyph { at, shape, color } => Prim::Glyph {
                at: shift(at),
                shape,
                color,
            },
            text => text,
        })
        .collect();
    let (right, up, forward) = camera.axes();
    let eye = camera.position;
    let focal = height / 2. * 3f64.sqrt() * f64::from(camera.zoom.max(0.01));
    let scale = height / 480.;
    let world_per_pixel = |p: [f64; 3]| {
        let d: [f64; 3] = std::array::from_fn(|i| p[i] - eye[i]);
        let depth = (d[0] * forward[0] + d[1] * forward[1] + d[2] * forward[2]).max(1.);
        depth / focal
    };
    for prim in &prims {
        match prim {
            Prim::Line {
                from,
                to,
                color,
                dashed,
            } => {
                if *dashed {
                    for i in (0..DASHES).step_by(2) {
                        let a = mix(*from, *to, i as f64 / DASHES as f64);
                        let b = mix(*from, *to, (i + 1) as f64 / DASHES as f64);
                        trails::ribbon_wide(out, &[a, b], *color, camera, height, LINE_WIDTH);
                    }
                } else {
                    trails::ribbon_wide(out, &[*from, *to], *color, camera, height, LINE_WIDTH);
                }
            }
            Prim::Glyph { at, shape, color } => {
                let unit = world_per_pixel(*at);
                let r = radius(*shape) * scale;
                // Lifted by its own radius so a mark on the ground does not
                // sink into it when seen from low.
                let centre = [at[0], at[1] + r * unit * 0.5, at[2]];
                for line in outline(*shape, r) {
                    let points: Vec<[f64; 3]> = line
                        .iter()
                        .map(|[x, y]| {
                            std::array::from_fn(|i| centre[i] + (right[i] * x + up[i] * y) * unit)
                        })
                        .collect();
                    trails::ribbon_wide(out, &points, *color, camera, height, LINE_WIDTH);
                }
            }
            Prim::Text { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_replay::{Event, Value};

    fn at(tick: u64, event: Event) -> TimedEvent {
        TimedEvent { tick, event }
    }

    fn pos(event: Event, p: [f64; 3]) -> Event {
        let mut event = event;
        for (name, v) in field::POSITION.into_iter().zip(p) {
            event = event.with(name, v);
        }
        event
    }

    fn laser(state: &str, p: Option<[f64; 3]>) -> Event {
        let e = Event::new(kind::PLAYER_LASER)
            .with_subject(0)
            .with(field::STATE, state);
        p.map_or(e.clone(), |p| pos(e, p))
    }

    fn guided(mode: &str, owner: Option<u32>, p: Option<[f64; 3]>, reason: &str) -> Event {
        let mut e = Event::new(kind::WEAPON_GUIDED)
            .with_subject(0)
            .with(field::PROJECTILE, Value::Id(7))
            .with(field::GUIDANCE, "laser")
            .with(field::MODE, mode)
            .with(field::REASON, reason);
        if let Some(owner) = owner {
            e = e
                .with(field::SPOT_OWNER, Value::Id(owner))
                .with(field::OWN, owner == 0);
        }
        p.map_or(e.clone(), |p| pos(e, p))
    }

    fn positions<'a>(
        aircraft: &'a dyn Fn(u32) -> Option<[f64; 3]>,
        projectile: &'a dyn Fn(u32) -> Option<[f64; 3]>,
        object: &'a dyn Fn(u32) -> Option<[f64; 3]>,
    ) -> Positions<'a> {
        Positions {
            aircraft,
            projectile,
            object,
        }
    }

    /// A pass: the laser comes on at 120, moves at 240, a bomb leaves at 300
    /// guiding on it, the laser goes off at 480 and the bomb goes blind,
    /// lands at 600.
    fn pass() -> Vec<TimedEvent> {
        let spot = [1_000., 0., 5_000.];
        let moved = [1_020., 0., 5_030.];
        vec![
            at(120, laser("lasing", Some(spot))),
            at(
                240,
                pos(
                    Event::new(kind::PLAYER_LASER_SPOT)
                        .with_subject(0)
                        .with_object(0x4000_0001),
                    moved,
                ),
            ),
            at(300, guided("guiding", Some(0), Some(moved), "it follows")),
            at(
                360,
                pos(
                    Event::new(kind::WEAPON_AIM)
                        .with_subject(0)
                        .with(field::PROJECTILE, Value::Id(7)),
                    [1_021., 0., 5_031.],
                ),
            ),
            at(480, laser("off", None)),
            at(
                480,
                guided("blind", Some(0), None, "Your laser went off").with(field::FROM, "guiding"),
            ),
            at(
                600,
                pos(
                    Event::new(kind::WEAPON_IMPACT)
                        .with_subject(0)
                        .with(field::PROJECTILE, Value::Id(7))
                        .with(field::MISS_FT, 940.5)
                        .with(field::TARGET_FT, 950.)
                        .with(field::MODE, "blind"),
                    [1_900., 0., 5_500.],
                )
                .with(field::AIM[0], 1_021.)
                .with(field::AIM[1], 0.)
                .with(field::AIM[2], 5_031.),
            ),
        ]
    }

    #[test]
    fn the_spot_follows_the_laser_through_its_states() {
        let marks = Marks::from_events(&pass());
        assert!(marks.any());
        assert!(marks.scene(119).spots.is_empty());
        let on = marks.scene(120);
        assert_eq!(on.spots.len(), 1);
        assert!(on.spots[0].lasing && on.spots[0].at == [1_000., 0., 5_000.]);
        // A later entry moves it.
        assert_eq!(marks.scene(400).spots[0].at, [1_020., 0., 5_030.]);
        // Off: nothing is drawn.
        assert!(marks.scene(480).spots.is_empty());
    }

    #[test]
    fn a_masked_laser_keeps_a_hollow_mark_at_the_last_spot() {
        let mut events = pass();
        events.insert(
            3,
            at(320, laser("masked", None).with(field::MASK, "gimbal")),
        );
        let marks = Marks::from_events(&events);
        let masked = marks.scene(330).spots;
        assert_eq!(masked.len(), 1);
        assert!(!masked[0].lasing);
        assert_eq!(masked[0].mask.as_deref(), Some("gimbal"));
        assert_eq!(masked[0].at, [1_020., 0., 5_030.]);
    }

    #[test]
    fn a_weapon_guides_then_goes_blind_with_its_reason_and_lands() {
        let marks = Marks::from_events(&pass());
        assert!(marks.scene(299).weapons.is_empty());
        let guiding = marks.scene(400).weapons;
        assert_eq!(guiding.len(), 1);
        let w = &guiding[0];
        assert_eq!(
            (w.mode, w.guidance, w.own),
            (Mode::Guiding, Guidance::Laser, true)
        );
        // The newest aim point by then.
        assert_eq!(w.aim, Some([1_021., 0., 5_031.]));
        let blind = marks.scene(500).weapons;
        assert_eq!(blind[0].mode, Mode::Blind);
        assert_eq!(blind[0].reason, "Your laser went off");
        // It keeps the point it last steered for.
        assert_eq!(blind[0].aim, Some([1_021., 0., 5_031.]));
        // Landed: the weapon is gone and its impact shows, then fades.
        let landed = marks.scene(600);
        assert!(landed.weapons.is_empty());
        assert_eq!(landed.impacts.len(), 1);
        assert_eq!(landed.impacts[0].miss_ft, Some(940.5));
        assert!(landed.impacts[0].blind);
        assert!(marks.scene(600 + IMPACT_SHOWN_TICKS).impacts.len() == 1);
        assert!(marks.scene(601 + IMPACT_SHOWN_TICKS).impacts.is_empty());
    }

    #[test]
    fn a_weapon_released_with_no_spot_waits_and_has_no_aim() {
        let marks = Marks::from_events(&[at(10, guided("waiting", None, None, "no spot yet"))]);
        let w = &marks.scene(20).weapons[0];
        assert_eq!((w.mode, w.aim), (Mode::Waiting, None));
    }

    #[test]
    fn a_jdam_flies_to_its_stored_point() {
        let stored = [4_000., 0., 9_000.];
        let event = pos(
            Event::new(kind::WEAPON_GUIDED)
                .with_subject(0)
                .with(field::PROJECTILE, Value::Id(3))
                .with(field::GUIDANCE, "coordinate")
                .with(field::MODE, "guiding")
                .with(field::STORED, true),
            stored,
        );
        let marks = Marks::from_events(&[at(5, event)]);
        let w = &marks.scene(6).weapons[0];
        assert_eq!((w.guidance, w.aim), (Guidance::Coordinate, Some(stored)));
        let who = Who {
            player: 0,
            side: &|_| Side::Friendly,
            name: &|id| format!("A{id}"),
        };
        let prims = draw(
            &marks.scene(6),
            &positions(&|_| None, &|_| Some([0., 3_000., 0.]), &|_| None),
            &who,
        );
        assert!(prims.contains(&Prim::Line {
            from: [0., 3_000., 0.],
            to: stored,
            color: STORED,
            dashed: false
        }));
        assert!(prims.iter().any(|p| matches!(
            p,
            Prim::Glyph { shape: Shape::Diamond, at, .. } if *at == stored
        )));
        assert!(prims.iter().any(|p| matches!(
            p,
            Prim::Text { text, .. } if text == "GUIDING TO STORED POINT"
        )));
    }

    #[test]
    fn designations_come_and_go() {
        let designate = |channel: &str, object: Option<u32>| {
            let e = Event::new(kind::PLAYER_DESIGNATE)
                .with_subject(0)
                .with(field::CHANNEL, channel);
            object.map_or(e.clone(), |o| e.with_object(o))
        };
        let marks = Marks::from_events(&[
            at(10, designate("a-g radar", Some(0x4000_0002))),
            at(50, designate("cleared", None)),
        ]);
        assert!(marks.scene(9).designations.is_empty());
        let d = &marks.scene(10).designations[0];
        assert_eq!(
            (d.object, d.channel.as_str()),
            (Some(0x4000_0002), "a-g radar")
        );
        assert!(marks.scene(50).designations.is_empty());
    }

    #[test]
    fn a_recording_without_these_events_has_no_marks() {
        let marks = Marks::from_events(&[at(10, Event::new(kind::WEAPON_LAUNCH).with_subject(0))]);
        assert!(!marks.any());
        assert_eq!(marks.scene(10), Scene::default());
    }

    #[test]
    fn an_older_recording_draws_the_spot_where_the_laser_came_on() {
        // Before FX-R: the state change carries the spot, nothing follows it.
        let marks = Marks::from_events(&[
            at(100, laser("lasing", Some([1., 2., 3.]))),
            at(900, laser("off", None)),
        ]);
        assert_eq!(marks.scene(500).spots[0].at, [1., 2., 3.]);
        assert!(marks.scene(900).spots.is_empty());
        assert!(marks.scene(500).impacts.is_empty());
    }

    fn who_is_whose() -> Who<'static> {
        Who {
            player: 0,
            side: &|id| if id < 3 { Side::Friendly } else { Side::Enemy },
            name: &|id| format!("Friendly {id}"),
        }
    }

    #[test]
    fn own_buddy_and_enemy_spots_get_their_own_colours_and_words() {
        let scene = Scene {
            spots: vec![
                Spot {
                    owner: 0,
                    at: [0.; 3],
                    lasing: true,
                    mask: None,
                },
                Spot {
                    owner: 1,
                    at: [10.; 3],
                    lasing: true,
                    mask: None,
                },
                Spot {
                    owner: 4,
                    at: [20.; 3],
                    lasing: true,
                    mask: None,
                },
            ],
            ..Scene::default()
        };
        let here = |id: u32| Some([id as f64 * 100., 1_000., 0.]);
        let prims = draw(
            &scene,
            &positions(&here, &|_| None, &|_| None),
            &who_is_whose(),
        );
        let color_of = |text: &str| {
            prims.iter().find_map(|p| match p {
                Prim::Text { text: t, color, .. } if t == text => Some(*color),
                _ => None,
            })
        };
        assert_eq!(color_of("YOUR LASER"), Some(OWN));
        assert_eq!(color_of("FRIENDLY 1 LASER"), Some(BUDDY));
        assert_eq!(
            color_of("FRIENDLY 4 LASER"),
            Some(trails::side_color(Side::Enemy))
        );
        // A line from each lasing aircraft to its spot.
        assert!(prims.contains(&Prim::Line {
            from: [100., 1_000., 0.],
            to: [10.; 3],
            color: BUDDY,
            dashed: false
        }));
    }

    #[test]
    fn a_blind_weapon_draws_a_dashed_line_to_its_last_aim_and_says_why() {
        let marks = Marks::from_events(&pass());
        let prims = draw(
            &marks.scene(500),
            &positions(&|_| None, &|_| Some([1_500., 2_000., 5_200.]), &|_| None),
            &who_is_whose(),
        );
        assert!(prims.contains(&Prim::Line {
            from: [1_500., 2_000., 5_200.],
            to: [1_021., 0., 5_031.],
            color: BLIND,
            dashed: true
        }));
        assert!(prims.iter().any(|p| matches!(
            p,
            Prim::Text { text, color, .. }
                if text == "BLIND: YOUR LASER WENT OFF" && *color == BLIND
        )));
    }

    #[test]
    fn a_weapon_not_in_the_picture_draws_nothing_and_an_impact_says_how_far() {
        let marks = Marks::from_events(&pass());
        let none = positions(&|_| None, &|_| None, &|_| None);
        assert!(
            draw(&marks.scene(400), &none, &who_is_whose())
                .iter()
                .all(|p| !matches!(p, Prim::Text { text, .. } if text.starts_with("GUIDING")))
        );
        let prims = draw(&marks.scene(610), &none, &who_is_whose());
        assert!(prims.iter().any(|p| matches!(
            p,
            Prim::Text { text, .. }
                if text == "BLIND IMPACT 941 FT FROM AIM, 950 FT FROM TARGET"
        )));
        // The impact joins to the aim point it missed.
        assert!(prims.contains(&Prim::Line {
            from: [1_900., 0., 5_500.],
            to: [1_021., 0., 5_031.],
            color: IMPACT,
            dashed: true
        }));
    }

    #[test]
    fn a_dud_says_so() {
        let landing = Event::new(kind::WEAPON_IMPACT)
            .with_subject(0)
            .with(field::PROJECTILE, Value::Id(9));
        let dud = Event::new(kind::WEAPON_DUD)
            .with_subject(0)
            .with(field::PROJECTILE, Value::Id(9));
        let marks = Marks::from_events(&[at(5, pos(landing, [0.; 3])), at(5, dud)]);
        assert_eq!(marks.scene(6).impacts[0].label(), "DUD");
    }

    #[test]
    fn geometry_draws_lines_and_shapes_and_skips_text() {
        let mut camera = Camera::new();
        camera.position = [0., 1_000., -2_000.];
        camera.yaw = 0.;
        camera.pitch = 0.;
        camera.roll = 0.;
        let prims = vec![
            Prim::Line {
                from: [0., 900., 0.],
                to: [500., 0., 4_000.],
                color: OWN,
                dashed: false,
            },
            Prim::Glyph {
                at: [500., 0., 4_000.],
                shape: Shape::Spot,
                color: OWN,
            },
            Prim::Text {
                at: [0.; 3],
                text: "X".into(),
                color: OWN,
            },
        ];
        let mut out = Vec::new();
        geometry(&prims, &camera, [0.; 3], 960., &mut out);
        // One ribbon, then a cross (2 ribbons) and a ring (16).
        assert_eq!(out.len(), (1 + 2 + 16) * 6 * 10);
        assert!(out.iter().all(|v| v.is_finite()));
        let mut dashed = Vec::new();
        geometry(
            &[Prim::Line {
                from: [0.; 3],
                to: [0., 0., 4_000.],
                color: BLIND,
                dashed: true,
            }],
            &camera,
            [0.; 3],
            960.,
            &mut dashed,
        );
        assert_eq!(dashed.len(), DASHES / 2 * 6 * 10);
    }

    #[test]
    fn geometry_is_relative_to_the_render_origin() {
        let mut camera = Camera::new();
        camera.position = [1_000_000., 1_000., 1_000_000.];
        camera.yaw = 0.;
        camera.pitch = 0.;
        camera.roll = 0.;
        let line = Prim::Line {
            from: [1_000_000., 900., 1_002_000.],
            to: [1_000_500., 900., 1_004_000.],
            color: OWN,
            dashed: false,
        };
        let origin = [1_000_448., 0., 999_936.];
        let mut out = Vec::new();
        geometry(&[line], &camera, origin, 960., &mut out);
        assert!(!out.is_empty());
        // Vertices lie near the eye's own offset from the origin, not near
        // the world position.
        assert!(out.chunks_exact(10).all(|v| v[2] > 2_000. && v[2] < 4_200.));
        assert!(out.chunks_exact(10).all(|v| v[0].abs() < 1_000.));
    }

    #[test]
    fn a_shape_keeps_its_size_on_screen_near_and_far() {
        let mut camera = Camera::new();
        camera.position = [0., 1_000., 0.];
        camera.yaw = 0.;
        camera.pitch = 0.;
        camera.roll = 0.;
        let width = |z: f64| {
            let mut out = Vec::new();
            geometry(
                &[Prim::Glyph {
                    at: [0., 1_000., z],
                    shape: Shape::Diamond,
                    color: OWN,
                }],
                &camera,
                [0.; 3],
                960.,
                &mut out,
            );
            let xs: Vec<f64> = out
                .chunks_exact(10)
                .map(|v| {
                    camera
                        .project(
                            [1280, 960],
                            [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])],
                        )
                        .unwrap()[0]
                })
                .collect();
            xs.iter().cloned().fold(f64::MIN, f64::max)
                - xs.iter().cloned().fold(f64::MAX, f64::min)
        };
        let (near, far) = (width(2_000.), width(20_000.));
        assert!((near - far).abs() < 1.5, "{near} {far}");
        assert!(near > 10.);
    }
}
