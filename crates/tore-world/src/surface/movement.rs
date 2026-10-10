//! Surface movement: the ground vehicles and ships that follow a route
//! (docs/spec/surface-defenses.md, "Movement").
//!
//! Five Quick Mission templates carry routes: `~QUCOL` (nine tanks), `~QTCARGO`
//! (three cargo ships), `~QUFACT` and `~QUBUNK` (one truck each) and the
//! unreferenced `~QFACT`. A unit with a route gets a [`Course`] when the
//! surface resolves (the route's legs, speeds in feet per second, and the
//! unit's turn rate and top speed from its NT record) and, once the mission
//! runs, a [`Mover`] in its [`SurfaceUnitState`](super::SurfaceUnitState): the
//! whole changing state of a moving unit, in whole units so that it is exact,
//! compares with `Eq`, and checkpoints without rounding.
//!
//! The follower (fitted, the retail waypoint consumer is untraced): steer
//! toward the next leg's point at the record's turn rate, accelerate or brake
//! at a fixed rate toward the leg's speed, slow on a sharp turn, take the next
//! leg inside a turning-circle radius of the point, brake to a halt on the
//! last point and stay there. Land units follow the terrain height and pitch
//! and bank to its slope; ships keep the water level they started at and stay
//! level (their routes are authored on water; the terrain grid's 8,192 foot
//! cells are too coarse to say where a harbour or river ends). A unit with no
//! hit points stops
//! where it is. Nothing here reads the clock: a tick advances the state by
//! one fixed step, so the path is a function of the mission and the tick, and
//! a checkpoint resumes it exactly.
//!
//! [`Combat::step_surface`] runs it once per tick before combat, and keeps
//! each moving unit's combat target (aim point, velocity, orientation) and
//! hit box in step with its pose. The pose is also the hook for the units'
//! own fire: [`unit_pose`] answers the origin and orientation of any surface
//! unit now, moving or standing, so a mount's world position is the pose plus
//! the mount's offset.
use super::{SurfacePose, SurfaceUnitState, Unit, UnitId};
use crate::combat::Combat;
use crate::terrain::Terrain;
use std::collections::BTreeMap;
use tore_formats::surface_unit::{SurfaceUnit, class};
use tore_sim::airport::OrientedBox;
use tore_sim::attitude::{Basis, Vector, cross, dot, unit};

/// One simulation step, seconds (the fixed 120 Hz tick).
const STEP: f64 = 1. / 120.;
/// Acceleration and braking of land units, feet per second squared (fitted:
/// the record's `_acc` and `_dacc` units are unknown; spec).
pub const GROUND_ACCELERATION: f64 = 5.;
/// Acceleration and braking of ships, feet per second squared (fitted).
pub const SHIP_ACCELERATION: f64 = 1.;
/// A turn sharper than this slows the unit to [`CORNER_SHARE`] of the leg's
/// speed until it points along the leg again (fitted).
const SHARP_TURN: f64 = std::f64::consts::FRAC_PI_4;
/// The share of the leg's speed on a sharp turn (fitted).
const CORNER_SHARE: f64 = 0.25;
/// The last point counts as reached within this many feet; the unit then
/// stands on it exactly (fitted).
const STOP_FEET: f64 = 4.;
/// A leg that is not the last counts as reached inside this many feet at the
/// least, and inside two turning-circle radii at speed, so a point the unit
/// cannot turn onto never makes it circle (fitted).
const REACH_FEET: f64 = 25.;
/// Half the span, in feet, of the height samples that give the terrain slope
/// under a land unit (fitted).
const SLOPE_FEET: f64 = 30.;
/// Position and speed are kept in 1/65536 foot (and foot per second): fine
/// enough that rounding each tick does not bias the acceleration.
const FINE: f64 = 65536.;
/// An angle is kept as a binary angle: 2^32 to the circle.
const CIRCLE: f64 = 4_294_967_296.;

/// One leg of a route: where to go and how fast, in feet and feet per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    pub to: [f64; 2],
    pub speed: f64,
}

/// A moving unit's hit box in the unit's own frame, fixed when the scene is
/// built: the box centre relative to the unit's origin (right, up, forward)
/// and the half extents the scene gave it. Reading both from the scene keeps
/// the contact box at one scale with the standing units' (`placed_shape_scale`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rig {
    pub centre: Vector,
    pub half: Vector,
}

/// What a unit with a route needs to follow it. Fixed when the surface
/// resolves; never changes.
#[derive(Clone, Debug, PartialEq)]
pub struct Course {
    /// The legs in order, after the start. The route's closing waypoint
    /// (flag 2, at 0 0 0) is not a leg.
    pub legs: Vec<Leg>,
    /// Feet and degrees of the placement: the unit's start.
    pub start: [i32; 3],
    pub start_angles: [i32; 3],
    /// Radians per second (the record's `_turnRate`, 182 units per degree).
    pub turn_rate: f64,
    /// Feet per second; 0 when the record limits nothing.
    pub top_speed: f64,
    pub acceleration: f64,
    pub ship: bool,
    /// Set when the scene holds the unit; a unit with no hit box cannot move.
    pub rig: Option<Rig>,
    /// Feet right of the authored path this unit's lane runs ([`set_lanes`]);
    /// 0 for a unit alone on its route. The legs already carry it.
    pub lane: f64,
}

impl Course {
    /// The course of `unit` from its route and its type's record, or `None`
    /// when it has no route, no leg or no speed to travel at.
    pub fn of(unit: &Unit, record: &SurfaceUnit) -> Option<Self> {
        let route = unit.route.as_ref()?;
        let top_speed = f64::from(record.movement.max_speed.max(0));
        let legs: Vec<Leg> = route
            .legs()
            .map(|leg| {
                let wanted = f64::from(leg.speed.max(0));
                let speed = if top_speed > 0. {
                    if wanted > 0. {
                        wanted.min(top_speed)
                    } else {
                        top_speed
                    }
                } else {
                    wanted
                };
                Leg {
                    to: [f64::from(leg.position[0]), f64::from(leg.position[2])],
                    speed,
                }
            })
            .collect();
        if legs.is_empty() || legs.iter().any(|leg| leg.speed <= 0.) {
            return None;
        }
        let ship = unit.class & class::SHIP != 0;
        Some(Self {
            legs,
            start: unit.position,
            start_angles: unit.angles,
            turn_rate: f64::from(record.movement.turn_rate.max(0))
                / f64::from(tore_formats::surface_unit::ANGLE_UNITS_PER_DEGREE)
                * std::f64::consts::PI
                / 180.,
            top_speed,
            acceleration: if ship {
                SHIP_ACCELERATION
            } else {
                GROUND_ACCELERATION
            },
            ship,
            rig: None,
            lane: 0.,
        })
    }

    /// The hit box, set from the scene's box for the unit at its start on
    /// ground `ground` feet high.
    pub fn fit(&mut self, bounds: &OrientedBox, ground: f64) {
        let [heading, pitch, bank] = self.start_angles.map(|a| f64::from(a).to_radians());
        let basis = Basis::new(heading, pitch, bank);
        let origin = [
            f64::from(self.start[0]),
            ground + f64::from(self.start[1]),
            f64::from(self.start[2]),
        ];
        let offset: Vector = std::array::from_fn(|i| bounds.center[i] - origin[i]);
        self.rig = Some(Rig {
            centre: [
                dot(offset, basis.right),
                dot(offset, basis.up),
                dot(offset, basis.forward),
            ],
            half: bounds.half,
        });
    }

    /// The unit's heading at the start, as a binary angle.
    fn start_heading(&self) -> i32 {
        angle_bits(f64::from(self.start_angles[0]).to_radians())
    }

    /// Length of the path from the start through every leg, in feet.
    pub fn length(&self) -> f64 {
        let mut at = [f64::from(self.start[0]), f64::from(self.start[2])];
        let mut total = 0.;
        for leg in &self.legs {
            total += (leg.to[0] - at[0]).hypot(leg.to[1] - at[1]);
            at = leg.to;
        }
        total
    }
}

/// Why a moving unit stands still for good.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Halt {
    /// Still on its way.
    Moving,
    /// Reached the end of its route.
    Arrived,
    /// Destroyed; stopped where it died.
    Destroyed,
}

/// A moving unit's changing state. Positions are in 1/65536 foot east,
/// north and up; angles are binary angles (2^32 to the circle); speed is
/// 1/65536 foot per second. `y`, `pitch` and `bank` follow the terrain and are kept
/// so that the pose is known without it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mover {
    pub x: i64,
    pub y: i64,
    pub z: i64,
    pub heading: i32,
    pub pitch: i32,
    pub bank: i32,
    pub speed: i32,
    /// The leg being driven.
    pub leg: u32,
    pub halt: Halt,
}

fn fine(feet: f64) -> i64 {
    (feet * FINE).round() as i64
}
fn feet(fine: i64) -> f64 {
    fine as f64 / FINE
}
fn angle_bits(radians: f64) -> i32 {
    ((radians / std::f64::consts::TAU).rem_euclid(1.) * CIRCLE).round() as u32 as i32
}
fn angle_radians(bits: i32) -> f64 {
    f64::from(bits) / CIRCLE * std::f64::consts::TAU
}
/// `angle` wrapped into -pi to pi.
fn wrap(angle: f64) -> f64 {
    let turn = std::f64::consts::TAU;
    let a = angle.rem_euclid(turn);
    if a > std::f64::consts::PI {
        a - turn
    } else {
        a
    }
}

impl Mover {
    /// The state at the start of the mission, standing on its start point at
    /// rest. `ground` is the terrain height there.
    pub fn start(course: &Course, ground: f64) -> Self {
        Self {
            x: fine(f64::from(course.start[0])),
            y: fine(ground + f64::from(course.start[1])),
            z: fine(f64::from(course.start[2])),
            heading: course.start_heading(),
            pitch: 0,
            bank: 0,
            speed: 0,
            leg: 0,
            halt: Halt::Moving,
        }
    }

    pub fn position(&self) -> Vector {
        [feet(self.x), feet(self.y), feet(self.z)]
    }
    /// Yaw, pitch and bank in radians (yaw 0 to 2 pi).
    pub fn attitude(&self) -> [f64; 3] {
        [
            angle_radians(self.heading).rem_euclid(std::f64::consts::TAU),
            angle_radians(self.pitch),
            angle_radians(self.bank),
        ]
    }
    pub fn basis(&self) -> Basis {
        let [yaw, pitch, bank] = self.attitude();
        Basis::new(yaw, pitch, bank)
    }
    /// Feet per second along the heading.
    pub fn speed_feet(&self) -> f64 {
        f64::from(self.speed) / FINE
    }
    /// Ground-relative velocity, feet per second.
    pub fn velocity(&self) -> Vector {
        let heading = angle_radians(self.heading);
        let speed = self.speed_feet();
        [heading.sin() * speed, 0., heading.cos() * speed]
    }

    /// One tick of the follower. `alive` is whether the unit has hit points
    /// left. Lifts the state's height and orientation from `height` (the
    /// terrain, feet) afterwards.
    pub fn step(&mut self, course: &Course, alive: bool, height: impl Fn(f64, f64) -> f64) {
        if self.halt != Halt::Moving {
            return;
        }
        if !alive {
            self.halt = Halt::Destroyed;
            self.speed = 0;
            return;
        }
        let Some(leg) = course.legs.get(self.leg as usize).copied() else {
            self.halt = Halt::Arrived;
            self.speed = 0;
            return;
        };
        let last = self.leg as usize + 1 == course.legs.len();
        let (x, z) = (feet(self.x), feet(self.z));
        let (dx, dz) = (leg.to[0] - x, leg.to[1] - z);
        let distance = dx.hypot(dz);

        // Steer, then speed.
        let heading = angle_radians(self.heading);
        let error = wrap(dx.atan2(dz) - heading);
        let turn = error.clamp(-course.turn_rate * STEP, course.turn_rate * STEP);
        let heading = heading + turn;
        self.heading = angle_bits(heading);
        let mut wanted = leg.speed;
        if error.abs() > SHARP_TURN {
            wanted *= CORNER_SHARE;
        }
        if last {
            let room = (distance - STOP_FEET).max(0.);
            wanted = wanted.min((2. * course.acceleration * room).sqrt());
        }
        let speed = self.speed_feet();
        let change =
            (wanted - speed).clamp(-course.acceleration * STEP, course.acceleration * STEP);
        let speed = (speed + change).max(0.);
        self.speed = (speed * FINE).round() as i32;

        // Move.
        let (nx, nz) = (
            x + heading.sin() * speed * STEP,
            z + heading.cos() * speed * STEP,
        );
        self.x = fine(nx);
        self.z = fine(nz);

        // Arrive.
        let left = (leg.to[0] - nx).hypot(leg.to[1] - nz);
        if last {
            if left <= STOP_FEET.max(speed * STEP * 1.5) {
                self.x = fine(leg.to[0]);
                self.z = fine(leg.to[1]);
                self.speed = 0;
                self.halt = Halt::Arrived;
            }
        } else {
            let circle = if course.turn_rate > 0. {
                2. * speed / course.turn_rate
            } else {
                0.
            };
            if left <= circle.max(REACH_FEET) {
                self.leg += 1;
            }
        }
        self.settle(course, height);
    }

    /// Height, pitch and bank from the terrain under the unit: a land unit
    /// stands on the ground and tilts with its slope; a ship keeps its water
    /// level and stays level.
    pub fn settle(&mut self, course: &Course, height: impl Fn(f64, f64) -> f64) {
        if course.ship {
            return;
        }
        let (x, z) = (feet(self.x), feet(self.z));
        let ground = height(x, z);
        self.y = fine(ground + f64::from(course.start[1]));
        let slope_x = (height(x + SLOPE_FEET, z) - height(x - SLOPE_FEET, z)) / (2. * SLOPE_FEET);
        let slope_z = (height(x, z + SLOPE_FEET) - height(x, z - SLOPE_FEET)) / (2. * SLOPE_FEET);
        let up = unit([-slope_x, 1., -slope_z]);
        let heading = angle_radians(self.heading);
        let ahead = [heading.sin(), 0., heading.cos()];
        let forward = unit(std::array::from_fn(|i| ahead[i] - up[i] * dot(ahead, up)));
        let right = unit(cross(up, forward));
        let [_, pitch, bank] = Basis { right, up, forward }.angles();
        self.pitch = angle_bits(pitch);
        self.bank = angle_bits(bank);
    }

    /// The hit box and aim point this pose gives a unit with `rig`.
    pub fn bounds(&self, rig: &Rig) -> OrientedBox {
        let basis = self.basis();
        let origin = self.position();
        let [heading, pitch, bank] = self.attitude();
        OrientedBox {
            center: std::array::from_fn(|i| {
                origin[i]
                    + basis.right[i] * rig.centre[0]
                    + basis.up[i] * rig.centre[1]
                    + basis.forward[i] * rig.centre[2]
            }),
            half: rig.half,
            heading,
            pitch,
            bank,
        }
    }
}

/// Where a surface unit is now: its origin on the ground, its yaw, pitch and
/// bank in radians and its velocity. A unit that has not moved, and one with
/// no route, stands where the mission put it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitPose {
    pub position: Vector,
    pub attitude: [f64; 3],
    pub velocity: Vector,
}

/// The pose of `unit` given its state. Mount positions and the muzzle of a
/// unit's weapons are this pose plus the mount's offset turned by its
/// attitude, so a moving unit fires from where it is.
pub fn unit_pose(unit: &Unit, state: Option<&SurfaceUnitState>, terrain: &Terrain) -> UnitPose {
    if let Some(mover) = state.and_then(|s| s.mover) {
        return UnitPose {
            position: mover.position(),
            attitude: mover.attitude(),
            velocity: mover.velocity(),
        };
    }
    let [x, y, z] = unit.position;
    let ground = f64::from(terrain.height(x as f32, z as f32));
    UnitPose {
        position: [f64::from(x), ground + f64::from(y), f64::from(z)],
        attitude: unit.angles.map(|a| f64::from(a).to_radians()),
        velocity: [0.; 3],
    }
}

/// Lane spacing in hull beams: units that share a route each drive their own
/// lane, this many of the group's widest beam apart (fitted, lead ruling
/// after M1, 2026-10-10).
pub const LANE_BEAMS: f64 = 1.5;
/// A lane's corner offset grows with the turn (a mitre, so lanes stay
/// parallel through it) up to this many lane widths (fitted).
const MITRE_LIMIT: f64 = 2.;

/// Gives every unit that shares its route with others its own lane
/// (docs/spec/surface-defenses.md, "Movement"). Units whose legs run through
/// exactly the same points form a group (the nine `~QUCOL` tanks, the three
/// `~QTCARGO` ships); in ascending id order they take lanes side by side,
/// centred on the authored path, [`LANE_BEAMS`] of the group's widest hull
/// beam apart (the rig's width, so the hit box at its drawn size). Each of
/// the group's points moves right of the direction of travel by the unit's
/// lane: across the leg that arrives at it, mitred at a corner. Starts stay
/// where they are, so a column keeps its order and spacing along the road
/// and the units finish side by side instead of on one spot. Offsets are
/// whole feet, from integer route data and IEEE arithmetic only, so every
/// machine computes the same courses. A unit alone on its route, or one
/// without a rig, keeps its route.
pub fn set_lanes(courses: &mut BTreeMap<UnitId, Course>) {
    let mut groups: BTreeMap<Vec<(i64, i64)>, Vec<UnitId>> = BTreeMap::new();
    for (id, course) in courses.iter() {
        if course.rig.is_none() {
            continue;
        }
        let key = course
            .legs
            .iter()
            .map(|leg| (leg.to[0] as i64, leg.to[1] as i64))
            .collect();
        groups.entry(key).or_default().push(*id);
    }
    for members in groups.values().filter(|members| members.len() > 1) {
        let beam = members
            .iter()
            .filter_map(|id| courses[id].rig)
            .map(|rig| 2. * rig.half[0])
            .fold(0., f64::max);
        let spacing = (LANE_BEAMS * beam).round();
        if spacing <= 0. {
            continue;
        }
        let lead = &courses[&members[0]];
        let first = [f64::from(lead.start[0]), f64::from(lead.start[2])];
        let points: Vec<[f64; 2]> = lead.legs.iter().map(|leg| leg.to).collect();
        let normals: Vec<[f64; 2]> = (0..points.len())
            .map(|j| {
                let before = if j == 0 { first } else { points[j - 1] };
                let arrive = right_of(before, points[j]);
                match points.get(j + 1) {
                    Some(next) => mitre(arrive, right_of(points[j], *next)),
                    None => arrive,
                }
            })
            .collect();
        let middle = (members.len() - 1) as f64 / 2.;
        for (k, id) in members.iter().enumerate() {
            let lane = (k as f64 - middle) * spacing;
            let course = courses.get_mut(id).expect("a member has a course");
            for (leg, normal) in course.legs.iter_mut().zip(&normals) {
                leg.to = [
                    (leg.to[0] + normal[0] * lane).round(),
                    (leg.to[1] + normal[1] * lane).round(),
                ];
            }
            course.lane = lane;
        }
    }
}

/// The unit vector right of travel from `from` to `to` (east, north), or
/// zero for a leg of no length.
fn right_of(from: [f64; 2], to: [f64; 2]) -> [f64; 2] {
    let (dx, dz) = (to[0] - from[0], to[1] - from[1]);
    let length = (dx * dx + dz * dz).sqrt();
    if length == 0. {
        [0., 0.]
    } else {
        [dz / length, -dx / length]
    }
}

/// The corner offset between the lane normals of the leg arriving at a
/// point and the leg leaving it: along their bisector, long enough to keep
/// both lanes at full width, at most [`MITRE_LIMIT`] widths.
fn mitre(arrive: [f64; 2], leave: [f64; 2]) -> [f64; 2] {
    let sum = [arrive[0] + leave[0], arrive[1] + leave[1]];
    let length = (sum[0] * sum[0] + sum[1] * sum[1]).sqrt();
    if length < 1e-9 {
        return arrive;
    }
    let bisector = [sum[0] / length, sum[1] / length];
    let along = bisector[0] * arrive[0] + bisector[1] * arrive[1];
    let stretch = if along > 0. {
        (1. / along).min(MITRE_LIMIT)
    } else {
        MITRE_LIMIT
    };
    [bisector[0] * stretch, bisector[1] * stretch]
}

/// The courses of a resolved surface's units that follow a route, by unit.
pub fn courses<'a>(
    units: impl IntoIterator<Item = (&'a Unit, &'a SurfaceUnit)>,
) -> BTreeMap<UnitId, Course> {
    units
        .into_iter()
        .filter_map(|(unit, record)| Some((unit.id, Course::of(unit, record)?)))
        .collect()
}

impl Combat {
    /// One tick of every moving unit, before combat steps: advances each
    /// follower and puts the unit's combat target (aim point, velocity,
    /// orientation) and hit box where the pose now is. Units that are not in
    /// the scene have no hit box and do not move. Does nothing in a mission
    /// with no routes.
    pub fn step_surface(&mut self, terrain: &Terrain) {
        for (id, course) in &terrain.surface.courses {
            let Some(rig) = course.rig else { continue };
            let Some(alive) = self
                .state
                .targets
                .iter()
                .find(|target| target.id == id.0)
                .map(|target| target.hp > 0)
            else {
                continue;
            };
            let Some(slot) = self.surface.unit_mut(*id) else {
                continue;
            };
            let height = |x: f64, z: f64| f64::from(terrain.height(x as f32, z as f32));
            let before = slot.mover;
            let mover = slot.mover.get_or_insert_with(|| {
                let (x, z) = (f64::from(course.start[0]), f64::from(course.start[2]));
                let mut mover = Mover::start(course, height(x, z));
                mover.settle(course, height);
                mover
            });
            mover.step(course, alive, height);
            if before == Some(*mover) {
                continue;
            }
            let mover = *mover;
            self.state
                .move_ground_target(id.0, mover.bounds(&rig), mover.velocity(), true);
        }
    }

    /// Puts the unit `id` that follows a route in the state `mover`, as a
    /// machine that is told where the unit is (a client, a rejoin) does, and
    /// moves its combat target and hit box there. False if `id` follows no
    /// route or is not in the scene.
    pub fn place_surface_unit(&mut self, terrain: &Terrain, id: UnitId, mover: Mover) -> bool {
        let Some(rig) = terrain.surface.courses.get(&id).and_then(|c| c.rig) else {
            return false;
        };
        let Some(slot) = self.surface.unit_mut(id) else {
            return false;
        };
        if !self
            .state
            .move_ground_target(id.0, mover.bounds(&rig), mover.velocity(), false)
        {
            return false;
        }
        slot.mover = Some(mover);
        true
    }

    /// The poses of the units that follow a route, for the picture: every
    /// one that has begun to move (all of them from the first tick on), with
    /// a wreck flag for those with no hit points left. The shape is left to
    /// the drawing, which knows the unit's look.
    pub fn surface_poses(&self) -> Vec<SurfacePose> {
        self.surface
            .units
            .iter()
            .filter_map(|slot| {
                let mover = slot.mover?;
                let wrecked = self
                    .state
                    .targets
                    .iter()
                    .find(|target| target.id == slot.id.0)
                    .is_some_and(|target| target.hp <= 0);
                Some(SurfacePose {
                    id: slot.id,
                    position: mover.position(),
                    attitude: mover.attitude(),
                    shape: None,
                    wrecked,
                })
            })
            .collect()
    }
}
