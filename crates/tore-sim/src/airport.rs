//! Deterministic airport surfaces, player landing service, and static identity.
//! Geometry and policy follow docs/spec/airports.md. No autonomous traffic lives here.
use std::collections::{BTreeMap, BTreeSet};

pub const ILS_RANGE_FT: f64 = 5.0 * 6_076.12;
pub const ILS_ALTITUDE_AGL_FT: f64 = 4_000.0;
/// The ILS glide path, degrees. Fitted: the retail angle is not recovered (the
/// AI's recovered landing path is 6 degrees); John chose the usual 3 on
/// 2026-09-29. The player's ILS and the AI landing controller share this.
pub const GLIDE_SLOPE_DEGREES: f64 = 3.0;
/// Where the glide path meets the runway surface, feet past the threshold: the
/// touchdown zone. The path therefore crosses the threshold about 52 ft up.
pub const AIM_PAST_THRESHOLD_FT: f64 = 1_000.0;
/// Height of the ideal glide path above the runway surface, feet, for a wheel
/// contact point `before_threshold_ft` short of the threshold along the
/// approach (negative past it). Zero at the aim point; the AI controller and
/// the player ILS both fly to this.
pub fn glide_path_height_ft(before_threshold_ft: f64) -> f64 {
    (before_threshold_ft + AIM_PAST_THRESHOLD_FT).max(0.0) * GLIDE_SLOPE_DEGREES.to_radians().tan()
}
/// The path's height over the threshold itself, feet (about 52).
pub fn threshold_crossing_height_ft() -> f64 {
    glide_path_height_ft(0.0)
}
/// The longest runway a strip can have and still count as a short strip,
/// feet. `fitted`, agent decision 2026-09-30 (John decided on 2026-09-30 that
/// short strips leave the Quick Mission takeoff choice and the in-flight
/// airport list; the number is the agent's): the 22 strips of Cuba, the
/// Falklands, Pakistan, Panama and the Persian Gulf are about 1,074 ft and
/// every other airport in the retail theaters is 4,060 ft or longer, so any
/// line inside that gap picks the same airports. 2,000 ft sits well clear of
/// both. A runway under it is too short for any aircraft's takeoff run at a
/// normal load, for the player and far more for a wing.
pub const SHORT_STRIP_FT: f64 = 2_000.0;
/// Whether a runway of `length_ft` is a short strip.
pub fn short_strip_length(length_ft: f64) -> bool {
    length_ft < SHORT_STRIP_FT
}
pub const LANDING_SPEED_FPS: f64 = 30.0 * 6_076.12 / 3_600.0;
pub const LANDING_TICKS: u16 = 240;

pub type ObjectId = u32;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceKey {
    pub layout: String,
    pub ordinal: u32,
}

/// An airport's owner, recorded from Blue's point of view: `Friendly` is
/// Blue's, `Hostile` is Redfor's. [`Allegiance::seen_by`] gives a Redfor
/// pilot's view (docs/spec/airports.md, "Allegiance").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allegiance {
    Friendly,
    Neutral,
    Hostile,
    Unknown,
}
impl Allegiance {
    /// The allegiance of a runway placed with layout owner `redfor` (the
    /// nationality's side bit; `None` without an owner field): Blue's is
    /// friendly, Redfor's hostile, an unowned one neutral.
    pub fn of_owner(redfor: Option<bool>) -> Self {
        match redfor {
            Some(false) => Self::Friendly,
            Some(true) => Self::Hostile,
            None => Self::Neutral,
        }
    }
    /// This allegiance as a pilot of one side sees it: Blue's view is the
    /// recorded one, a Redfor pilot's swaps friendly and hostile.
    pub fn seen_by(self, redfor: bool) -> Self {
        match (self, redfor) {
            (Self::Friendly, true) => Self::Hostile,
            (Self::Hostile, true) => Self::Friendly,
            (other, _) => other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrientedBox {
    pub center: [f64; 3],
    pub half: [f64; 3],
    pub heading: f64,
    pub pitch: f64,
    pub bank: f64,
}
impl OrientedBox {
    pub fn valid(&self) -> bool {
        self.center.iter().all(|v| v.is_finite())
            && self.half.iter().all(|v| v.is_finite() && *v > 0.)
            && [self.heading, self.pitch, self.bank]
                .iter()
                .all(|v| v.is_finite())
    }
    pub fn contains_horizontal(&self, x: f64, z: f64) -> bool {
        let dx = x - self.center[0];
        let dz = z - self.center[2];
        // A point inside lies within the box's horizontal half diagonal at any
        // heading. The margin is far above rounding, so this early answer never
        // differs from the full test below; it only spares the trigonometry for
        // the many runways nowhere near the point.
        let diagonal = self.half[0] * self.half[0] + self.half[2] * self.half[2];
        if dx * dx + dz * dz > diagonal * (1. + 1e-9) + 1e-6 {
            return false;
        }
        let (s, c) = self.heading.sin_cos();
        let local_x = dx * c - dz * s;
        let local_z = dx * s + dz * c;
        local_x.abs() <= self.half[0] && local_z.abs() <= self.half[2]
    }
    pub fn segment_fraction(&self, from: [f64; 3], to: [f64; 3]) -> Option<f64> {
        let basis = crate::attitude::Basis::new(self.heading, self.pitch, self.bank);
        let local = |p: [f64; 3]| {
            let delta = std::array::from_fn(|i| p[i] - self.center[i]);
            [
                crate::attitude::dot(delta, basis.right),
                crate::attitude::dot(delta, basis.up),
                crate::attitude::dot(delta, basis.forward),
            ]
        };
        let a = local(from);
        let b = local(to);
        let d = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
        let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
        for i in 0..3 {
            if d[i].abs() < 1e-12 {
                if a[i].abs() > self.half[i] {
                    return None;
                }
            } else {
                let mut p = (-self.half[i] - a[i]) / d[i];
                let mut q = (self.half[i] - a[i]) / d[i];
                if p > q {
                    std::mem::swap(&mut p, &mut q);
                }
                enter = enter.max(p);
                leave = leave.min(q);
                if enter > leave {
                    return None;
                }
            }
        }
        Some(enter)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StaticObject {
    pub id: ObjectId,
    pub source: SourceKey,
    pub name: String,
    pub object_type: String,
    pub bounds: OrientedBox,
    pub hit_points: i32,
    pub category: u16,
    pub radar_signature: f64,
    pub infrared_signature: f64,
    pub runway: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Runway {
    pub object: ObjectId,
    pub airport: u32,
    pub name: String,
    pub surface: OrientedBox,
    /// Fitted approach centerline using the reviewed source runway anchor.
    pub approach_center: [f64; 3],
    pub elevation_ft: f64,
    pub heading: f64,
    pub length_ft: f64,
}
impl Runway {
    pub fn threshold(&self, end: ApproachEnd) -> [f64; 3] {
        let sign = if end == ApproachEnd::Near { -1. } else { 1. };
        [
            self.approach_center[0] + self.heading.sin() * self.length_ft * 0.5 * sign,
            self.elevation_ft,
            self.approach_center[2] + self.heading.cos() * self.length_ft * 0.5 * sign,
        ]
    }
    /// The runway plane is independent of buildings included in its shape bounds.
    pub fn support_height(&self, x: f64, z: f64) -> Option<f64> {
        let up = crate::attitude::Basis::new(
            self.surface.heading,
            self.surface.pitch,
            self.surface.bank,
        )
        .up;
        if up[1].abs() < 1e-6 {
            return None;
        }
        Some(
            self.approach_center[1]
                - (up[0] * (x - self.approach_center[0]) + up[2] * (z - self.approach_center[2]))
                    / up[1],
        )
    }
    /// Where [`Scene::runway_surface`] can answer for this runway and the
    /// highest it answers there: the horizontal box around its surface
    /// rectangle (x, z; a foot wider on each side than its corners) and the
    /// highest corner of its plane, which a plane over a rectangle never
    /// passes. `None` when it never answers.
    pub fn surface_extent(&self) -> Option<([f64; 2], [f64; 2], f64)> {
        let b = &self.surface;
        let (s, c) = b.heading.sin_cos();
        let corners = [(1., 1.), (1., -1.), (-1., 1.), (-1., -1.)].map(|(i, j)| {
            let (x, z) = (b.half[0] * i, b.half[2] * j);
            [b.center[0] + x * c + z * s, b.center[2] - x * s + z * c]
        });
        let top = corners
            .iter()
            .map(|p| self.support_height(p[0], p[1]))
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .reduce(f64::max)?;
        let lo = std::array::from_fn(|i| {
            corners.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min) - 1.
        });
        let hi = std::array::from_fn(|i| {
            corners
                .iter()
                .map(|p| p[i])
                .fold(f64::NEG_INFINITY, f64::max)
                + 1.
        });
        Some((lo, hi, top))
    }
    /// A short strip (see [`SHORT_STRIP_FT`]): nobody starts or lands here.
    pub fn short_strip(&self) -> bool {
        short_strip_length(self.length_ft)
    }
    pub fn departure_pose(&self) -> ([f64; 3], f64) {
        let mut position = self.threshold(ApproachEnd::Near);
        let inset = (self.length_ft * 0.05).min(100.);
        position[0] += self.heading.sin() * inset;
        position[2] += self.heading.cos() * inset;
        (position, self.heading)
    }
    /// The glide path's aim point for an approach to `end`: the touchdown zone,
    /// [`AIM_PAST_THRESHOLD_FT`] past the threshold, on the runway plane.
    pub fn aim_point(&self, end: ApproachEnd) -> [f64; 3] {
        let t = self.threshold(end);
        let heading = self.approach_heading(end);
        let (x, z) = (
            t[0] + heading.sin() * AIM_PAST_THRESHOLD_FT,
            t[2] + heading.cos() * AIM_PAST_THRESHOLD_FT,
        );
        [x, self.support_height(x, z).unwrap_or(t[1]), z]
    }
    pub fn approach_heading(&self, end: ApproachEnd) -> f64 {
        if end == ApproachEnd::Near {
            self.heading
        } else {
            (self.heading + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Airport {
    pub id: u32,
    pub name: String,
    pub runway_objects: Vec<ObjectId>,
    pub allegiance: Allegiance,
    pub neutral_permission: bool,
}
impl Airport {
    /// The airport's allegiance as a pilot of one side sees it.
    pub fn allegiance_for(&self, redfor: bool) -> Allegiance {
        self.allegiance.seen_by(redfor)
    }
    /// The tower's rule: a pilot of this side may use the airport when it is
    /// the side's own or a neutral one that grants permission. Every list a
    /// pilot or an AI aircraft picks a field from (the tower's, NAV's, the
    /// wing's landing order, homes, ground starts) uses it, so an enemy
    /// field is never offered.
    pub fn serves(&self, redfor: bool) -> bool {
        match self.allegiance_for(redfor) {
            Allegiance::Friendly => true,
            Allegiance::Neutral => self.neutral_permission,
            Allegiance::Hostile | Allegiance::Unknown => false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub objects: Vec<StaticObject>,
    pub runways: Vec<Runway>,
    pub airports: Vec<Airport>,
}
impl Scene {
    pub fn validate(&self) -> Result<(), &'static str> {
        let mut ids = BTreeSet::new();
        for o in &self.objects {
            if o.id == 0
                || o.hit_points <= 0
                || !o.bounds.valid()
                || !o.radar_signature.is_finite()
                || o.radar_signature < 0.
                || !o.infrared_signature.is_finite()
                || o.infrared_signature < 0.
                || !ids.insert(o.id)
            {
                return Err("invalid or duplicate static object");
            }
        }
        for r in &self.runways {
            if !ids.contains(&r.object)
                || !r.length_ft.is_finite()
                || r.length_ft <= 0.
                || !r.elevation_ft.is_finite()
                || !r.heading.is_finite()
                || !r.surface.valid()
            {
                return Err("runway references invalid object");
            }
        }
        Ok(())
    }
    pub fn runway(&self, id: ObjectId) -> Option<&Runway> {
        self.runways.iter().find(|r| r.object == id)
    }
    /// Whether a runway object is on a short strip (see [`SHORT_STRIP_FT`]).
    /// An object that is not a known runway is not.
    pub fn short_strip(&self, runway: ObjectId) -> bool {
        self.runway(runway).is_some_and(Runway::short_strip)
    }
    /// Whether every runway of the airport is a short strip. An airport with
    /// no runway in the scene is not: the scene says nothing about it.
    pub fn airport_is_short_strip(&self, airport: &Airport) -> bool {
        let mut runways = airport
            .runway_objects
            .iter()
            .filter_map(|id| self.runway(*id));
        let Some(first) = runways.next() else {
            return false;
        };
        first.short_strip() && runways.all(Runway::short_strip)
    }
    /// Whether airport `id` is a short strip.
    pub fn airport_id_is_short_strip(&self, id: u32) -> bool {
        self.airports
            .iter()
            .find(|a| a.id == id)
            .is_some_and(|a| self.airport_is_short_strip(a))
    }
    /// A vertical-landing pad (the DTSTRP type, type flags `$108021`), where
    /// conventional aircraft can neither take off nor land
    /// (docs/formats/native-strip.md, `@APLandingType@8` returns -1).
    pub fn vertical_pad(&self, runway: ObjectId) -> bool {
        self.objects.iter().any(|o| {
            o.id == runway
                && o.object_type
                    .rsplit(['/', '\\'])
                    .next()
                    .is_some_and(|name| name.eq_ignore_ascii_case("DTSTRP.OT"))
        })
    }
    pub fn runway_surface(&self, x: f64, z: f64) -> Option<(ObjectId, f64)> {
        self.runways
            .iter()
            .filter(|r| r.surface.contains_horizontal(x, z))
            .filter_map(|r| r.support_height(x, z).map(|height| (r.object, height)))
            .min_by_key(|(object, _)| *object)
    }
    pub fn earliest_object_hit(&self, from: [f64; 3], to: [f64; 3]) -> Option<(ObjectId, f64)> {
        self.objects
            .iter()
            .filter_map(|o| o.bounds.segment_fraction(from, to).map(|t| (o.id, t)))
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApproachEnd {
    Near,
    Far,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    SelectAirport(u32),
    RequestLanding,
    RepeatReply,
    CancelApproach,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclineReason {
    NoAirport,
    NoRunway,
    Hostile,
    NeutralPermission,
    UnknownAllegiance,
    RunwayDisabled,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Selected {
        airport: u32,
    },
    Cleared {
        airport: u32,
        runway: ObjectId,
        end: ApproachEnd,
    },
    Landed {
        airport: u32,
        runway: ObjectId,
    },
    Declined {
        airport: Option<u32>,
        reason: DeclineReason,
    },
    Repeated(Box<Reply>),
    Cancelled {
        airport: Option<u32>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Reply(Reply),
    TargetDestroyed(ObjectId),
    ClearanceInvalidated(ObjectId),
    LandingComplete { airport: u32, runway: ObjectId },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aircraft {
    pub position: [f64; 3],
    pub forward: [f64; 3],
    pub nav_mode: bool,
    pub gear_down: bool,
    pub supported: bool,
    pub alive: bool,
    pub speed_fps: f64,
    /// Height of the aircraft's origin above its wheels' contact plane, feet.
    /// The glide path is flown by the wheels, so it is taken off the height.
    pub ground_clearance_ft: f64,
    /// The aircraft flies for Redfor: the tower reads airport allegiance from
    /// its side ([`Airport::allegiance_for`]).
    pub redfor: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Guidance {
    pub airport: u32,
    pub runway: ObjectId,
    pub end: ApproachEnd,
    pub threshold: [f64; 3],
    pub range_ft: f64,
    pub bearing: f64,
    pub localizer_degrees: f64,
    pub glide_degrees: f64,
    pub localizer_normalized: f64,
    pub glide_normalized: f64,
    pub active: bool,
}

fn ils_eligible(
    aircraft: Aircraft,
    runway: &Runway,
    end: ApproachEnd,
) -> Option<(f64, f64, f64, f64)> {
    if aircraft
        .position
        .iter()
        .chain(&aircraft.forward)
        .any(|v| !v.is_finite())
    {
        return None;
    }
    let threshold = runway.threshold(end);
    let toward = std::array::from_fn::<_, 3, _>(|i| threshold[i] - aircraft.position[i]);
    let forward_length = aircraft.forward.iter().map(|v| v * v).sum::<f64>().sqrt();
    let toward_length = toward.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !forward_length.is_finite()
        || !toward_length.is_finite()
        || forward_length <= 1e-12
        || toward_length <= 1e-12
    {
        return None;
    }
    let facing = aircraft
        .forward
        .iter()
        .zip(toward)
        .map(|(a, b)| a * b)
        .sum::<f64>()
        / (forward_length * toward_length);
    // A full 90-degree cone includes 45 degrees either side of the nose.
    if !facing.is_finite() || facing + 1e-12 < std::f64::consts::FRAC_1_SQRT_2 {
        return None;
    }
    let heading = runway.approach_heading(end);
    let dx = aircraft.position[0] - threshold[0];
    let dz = aircraft.position[2] - threshold[2];
    let approach_forward = -(dx * heading.sin() + dz * heading.cos());
    let range = dx.hypot(dz);
    if approach_forward <= 0.
        || range > ILS_RANGE_FT
        || aircraft.position[1] - runway.elevation_ft > ILS_ALTITUDE_AGL_FT
    {
        return None;
    }
    Some((dx, dz, approach_forward, range))
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Clearance {
    airport: u32,
    runway: ObjectId,
    end: ApproachEnd,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    health: BTreeMap<ObjectId, i32>,
    destroyed: BTreeSet<ObjectId>,
    selected: Option<u32>,
    clearance: Option<Clearance>,
    last_reply: Option<Reply>,
    landing_ticks: u16,
}
impl Service {
    pub fn new(scene: &Scene) -> Result<Self, &'static str> {
        scene.validate()?;
        Ok(Self {
            health: scene.objects.iter().map(|o| (o.id, o.hit_points)).collect(),
            destroyed: BTreeSet::new(),
            selected: None,
            clearance: None,
            last_reply: None,
            landing_ticks: 0,
        })
    }
    pub fn reset(&mut self, scene: &Scene) -> Result<(), &'static str> {
        *self = Self::new(scene)?;
        Ok(())
    }
    pub fn last_reply(&self) -> Option<&Reply> {
        self.last_reply.as_ref()
    }
    pub fn clearance(&self) -> Option<(u32, ObjectId, ApproachEnd)> {
        self.clearance
            .as_ref()
            .map(|c| (c.airport, c.runway, c.end))
    }
    /// Combat owns health. This service only mirrors it to resolve availability.
    pub fn synchronize_health(
        &mut self,
        values: impl IntoIterator<Item = (ObjectId, i32)>,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        for (id, value) in values {
            if let Some(old) = self.health.get(&id).copied() {
                if value < old {
                    events.extend(self.damage(id, old.saturating_sub(value)));
                } else if value > old {
                    self.health.insert(id, value);
                    if value > 0 {
                        self.destroyed.remove(&id);
                    }
                }
            }
        }
        events
    }
    pub fn selected(&self) -> Option<u32> {
        self.selected
    }
    /// The scene's objects out of action (no hit points left), in id order:
    /// with the selected airport and the clearance, what a networked client
    /// needs to rebuild this service's guidance ([`Self::presented`]).
    pub fn out_of_action(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.health
            .iter()
            .filter(|(_, hp)| **hp <= 0)
            .map(|(id, _)| *id)
    }
    /// A service as a networked client presents it: the scene's objects with
    /// those in `out_of_action` down, the `selected` airport and the
    /// `clearance` (airport, runway, end). It answers [`Self::guidance`] as
    /// the host's does; it has no tower reply or landing count, which only
    /// the host's service steps.
    pub fn presented(
        scene: &Scene,
        selected: Option<u32>,
        clearance: Option<(u32, ObjectId, ApproachEnd)>,
        out_of_action: impl IntoIterator<Item = ObjectId>,
    ) -> Result<Self, &'static str> {
        let mut service = Self::new(scene)?;
        for id in out_of_action {
            if let Some(hp) = service.health.get_mut(&id) {
                *hp = 0;
                service.destroyed.insert(id);
            }
        }
        service.selected = selected;
        service.clearance = clearance.map(|(airport, runway, end)| Clearance {
            airport,
            runway,
            end,
        });
        Ok(service)
    }
    pub fn usable(&self, runway: ObjectId) -> bool {
        self.health.get(&runway).is_some_and(|hp| *hp > 0)
    }
    pub fn damage(&mut self, id: ObjectId, amount: i32) -> Vec<Event> {
        let mut out = vec![];
        if amount <= 0 {
            return out;
        }
        if let Some(hp) = self.health.get_mut(&id) {
            let was = *hp > 0;
            *hp = hp.saturating_sub(amount).max(0);
            if was && *hp == 0 && self.destroyed.insert(id) {
                out.push(Event::TargetDestroyed(id));
                if self.clearance.as_ref().is_some_and(|c| c.runway == id) {
                    self.clearance = None;
                    self.landing_ticks = 0;
                    self.last_reply = Some(Reply::Declined {
                        airport: self.selected,
                        reason: DeclineReason::RunwayDisabled,
                    });
                    out.push(Event::ClearanceInvalidated(id));
                }
            }
        }
        out
    }
    fn choose(
        &self,
        scene: &Scene,
        aircraft: Aircraft,
        airport: u32,
    ) -> Option<(ObjectId, ApproachEnd)> {
        let a = scene.airports.iter().find(|a| a.id == airport)?;
        a.runway_objects
            .iter()
            .filter(|id| self.usable(**id) && !scene.short_strip(**id))
            .filter_map(|id| {
                scene.runway(*id).map(|r| {
                    let near = distance2(aircraft.position, r.threshold(ApproachEnd::Near));
                    let far = distance2(aircraft.position, r.threshold(ApproachEnd::Far));
                    (
                        *id,
                        if near <= far {
                            ApproachEnd::Near
                        } else {
                            ApproachEnd::Far
                        },
                        near.min(far),
                    )
                })
            })
            .min_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)))
            .map(|(id, end, _)| (id, end))
    }
    pub fn command(&mut self, scene: &Scene, aircraft: Aircraft, command: Command) -> Vec<Event> {
        let reply = match command {
            Command::SelectAirport(id) => {
                self.clearance = None;
                self.landing_ticks = 0;
                // A short strip is not on the tower's list (John, 2026-09-30).
                if scene.airports.iter().any(|a| a.id == id) && !scene.airport_id_is_short_strip(id)
                {
                    self.selected = Some(id);
                    Reply::Selected { airport: id }
                } else {
                    self.selected = None;
                    Reply::Declined {
                        airport: None,
                        reason: DeclineReason::NoAirport,
                    }
                }
            }
            Command::CancelApproach => {
                self.clearance = None;
                self.landing_ticks = 0;
                Reply::Cancelled {
                    airport: self.selected,
                }
            }
            Command::RepeatReply => {
                return vec![Event::Reply(Reply::Repeated(Box::new(
                    self.last_reply.clone().unwrap_or(Reply::Declined {
                        airport: self.selected,
                        reason: DeclineReason::NoAirport,
                    }),
                )))];
            }
            Command::RequestLanding => {
                let Some(id) = self.selected else {
                    return self.finish_reply(Reply::Declined {
                        airport: None,
                        reason: DeclineReason::NoAirport,
                    });
                };
                if let Some(c) = self
                    .clearance
                    .clone()
                    .filter(|c| c.airport == id && self.usable(c.runway))
                {
                    return self.finish_reply(Reply::Cleared {
                        airport: id,
                        runway: c.runway,
                        end: c.end,
                    });
                }
                let Some(a) = scene.airports.iter().find(|a| a.id == id) else {
                    return self.finish_reply(Reply::Declined {
                        airport: Some(id),
                        reason: DeclineReason::NoAirport,
                    });
                };
                let reason = match a.allegiance_for(aircraft.redfor) {
                    Allegiance::Hostile => Some(DeclineReason::Hostile),
                    Allegiance::Unknown => Some(DeclineReason::UnknownAllegiance),
                    Allegiance::Neutral if !a.neutral_permission => {
                        Some(DeclineReason::NeutralPermission)
                    }
                    _ => None,
                };
                if let Some(reason) = reason {
                    Reply::Declined {
                        airport: Some(id),
                        reason,
                    }
                } else if let Some((runway, end)) = self.choose(scene, aircraft, id) {
                    self.clearance = Some(Clearance {
                        airport: id,
                        runway,
                        end,
                    });
                    Reply::Cleared {
                        airport: id,
                        runway,
                        end,
                    }
                } else {
                    Reply::Declined {
                        airport: Some(id),
                        reason: DeclineReason::NoRunway,
                    }
                }
            }
        };
        self.finish_reply(reply)
    }
    fn finish_reply(&mut self, reply: Reply) -> Vec<Event> {
        self.last_reply = Some(reply.clone());
        vec![Event::Reply(reply)]
    }
    pub fn guidance(&self, scene: &Scene, aircraft: Aircraft) -> Option<Guidance> {
        let automatic;
        let c = if let Some(clearance) = &self.clearance {
            clearance
        } else {
            let (airport, runway, end) = if let Some(id) = self.selected {
                let (runway, end) = self.choose(scene, aircraft, id)?;
                (id, runway, end)
            } else {
                // Automatic guidance finds only fields the aircraft's side
                // may use: an enemy field is never offered.
                scene
                    .airports
                    .iter()
                    .filter(|a| a.serves(aircraft.redfor))
                    .filter_map(|a| {
                        let (runway, end) = self.choose(scene, aircraft, a.id)?;
                        let r = scene.runway(runway)?;
                        let (_, _, _, range) = ils_eligible(aircraft, r, end)?;
                        Some((a.id, runway, end, range * range))
                    })
                    .min_by(|a, b| a.3.total_cmp(&b.3).then(a.1.cmp(&b.1)))
                    .map(|(airport, runway, end, _)| (airport, runway, end))?
            };
            automatic = Clearance {
                airport,
                runway,
                end,
            };
            &automatic
        };
        if !self.usable(c.runway) {
            return None;
        }
        let r = scene.runway(c.runway)?;
        let threshold = r.threshold(c.end);
        let heading = r.approach_heading(c.end);
        let (dx, dz, forward, range) = ils_eligible(aircraft, r, c.end)?;
        let lateral = dx * heading.cos() - dz * heading.sin();
        let localizer = (lateral / forward).atan().to_degrees();
        // The wheels fly the path to the touchdown zone: elevation angle of the
        // wheels' height over the aim point against the glide slope. Zero on the
        // path, positive high, negative low.
        let aim = r.aim_point(c.end);
        let wheels = aircraft.position[1] - aircraft.ground_clearance_ft;
        let glide = ((wheels - aim[1]) / (forward + AIM_PAST_THRESHOLD_FT))
            .atan()
            .to_degrees()
            - GLIDE_SLOPE_DEGREES;
        let active = aircraft.alive && aircraft.nav_mode && aircraft.gear_down;
        Some(Guidance {
            airport: c.airport,
            runway: c.runway,
            end: c.end,
            threshold,
            range_ft: range,
            bearing: (-dx).atan2(-dz),
            localizer_degrees: localizer,
            glide_degrees: glide,
            localizer_normalized: (localizer / 2.5).clamp(-1., 1.),
            glide_normalized: (glide / 0.7).clamp(-1., 1.),
            active,
        })
    }
    pub fn step(&mut self, scene: &Scene, aircraft: Aircraft) -> Vec<Event> {
        let Some(c) = self.clearance.clone() else {
            self.landing_ticks = 0;
            return vec![];
        };
        if !self.usable(c.runway) {
            self.clearance = None;
            self.landing_ticks = 0;
            return vec![Event::ClearanceInvalidated(c.runway)];
        }
        let on_runway = scene.runway(c.runway).is_some_and(|r| {
            r.surface
                .contains_horizontal(aircraft.position[0], aircraft.position[2])
        });
        if aircraft.supported
            && aircraft.alive
            && on_runway
            && aircraft.speed_fps < LANDING_SPEED_FPS
        {
            self.landing_ticks = self.landing_ticks.saturating_add(1);
        } else {
            self.landing_ticks = 0;
        }
        if self.landing_ticks >= LANDING_TICKS {
            self.clearance = None;
            self.landing_ticks = 0;
            self.last_reply = Some(Reply::Landed {
                airport: c.airport,
                runway: c.runway,
            });
            vec![Event::LandingComplete {
                airport: c.airport,
                runway: c.runway,
            }]
        } else {
            vec![]
        }
    }
}
fn distance2(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The early distance answer agrees with the rotated test everywhere,
    /// including points on and just outside the corners.
    #[test]
    fn contains_horizontal_matches_the_rotated_test() {
        let rotated = |b: &OrientedBox, x: f64, z: f64| {
            let (s, c) = b.heading.sin_cos();
            let (dx, dz) = (x - b.center[0], z - b.center[2]);
            (dx * c - dz * s).abs() <= b.half[0] && (dx * s + dz * c).abs() <= b.half[2]
        };
        let mut seed = 0x1234_5678_9abc_def1_u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % 2_000_000) as f64 / 100. - 10_000.
        };
        let mut inside = 0;
        for _ in 0..200_000 {
            let b = OrientedBox {
                center: [next() * 50., 0., next() * 50.],
                half: [next().abs() / 10. + 1., 10., next().abs() + 1.],
                heading: next(),
                pitch: 0.,
                bank: 0.,
            };
            // Points near the box, and exactly on a corner.
            let (s, c) = b.heading.sin_cos();
            let corner = [
                b.center[0] + b.half[0] * c + b.half[2] * s,
                b.center[2] - b.half[0] * s + b.half[2] * c,
            ];
            for (x, z) in [
                (b.center[0] + next() / 2., b.center[2] + next()),
                (corner[0], corner[1]),
            ] {
                let expected = rotated(&b, x, z);
                inside += usize::from(expected);
                assert_eq!(b.contains_horizontal(x, z), expected, "{b:?} {x} {z}");
            }
        }
        assert!(inside > 1000, "too few points inside: {inside}");
    }

    /// Every point a sloped, turned runway answers for lies in its extent's
    /// box, no higher than its highest corner.
    #[test]
    fn a_runway_surface_extent_holds_every_point_of_it() {
        let mut scene = scene();
        let (lo, hi, top) = scene.runways[0].surface_extent().unwrap();
        assert_eq!((lo, hi, top), ([-101., -5001.], [101., 5001.], 100.));
        let runway = &mut scene.runways[0];
        runway.surface.heading = 0.7;
        runway.surface.pitch = 0.02;
        runway.surface.bank = -0.01;
        let (lo, hi, top) = scene.runways[0].surface_extent().unwrap();
        assert!(
            top > 100. + 50.,
            "a slope over 5,000 ft raises a corner: {top}"
        );
        let mut answered = 0;
        for x in (-6000..=6000).step_by(50) {
            for z in (-6000..=6000).step_by(50) {
                let (x, z) = (f64::from(x), f64::from(z));
                if let Some((_, height)) = scene.runway_surface(x, z) {
                    answered += 1;
                    assert!(height <= top + 1e-6, "{height} above {top}");
                    assert!(lo[0] <= x && x <= hi[0] && lo[1] <= z && z <= hi[1]);
                }
            }
        }
        assert!(answered > 100, "{answered}");
    }

    fn scene() -> Scene {
        let bounds = OrientedBox {
            center: [0., 100., 0.],
            half: [100., 10., 5000.],
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        };
        Scene {
            objects: vec![StaticObject {
                id: 1000,
                source: SourceKey {
                    layout: "X.MM".into(),
                    ordinal: 0,
                },
                name: "Test".into(),
                object_type: "STRIP.OT".into(),
                bounds,
                hit_points: 100,
                category: 0x100,
                radar_signature: 100.,
                infrared_signature: 100.,
                runway: true,
            }],
            runways: vec![Runway {
                object: 1000,
                airport: 7,
                name: "09/27".into(),
                surface: bounds,
                approach_center: bounds.center,
                elevation_ft: 100.,
                heading: 0.,
                length_ft: 10000.,
            }],
            airports: vec![Airport {
                id: 7,
                name: "Field".into(),
                runway_objects: vec![1000],
                allegiance: Allegiance::Friendly,
                neutral_permission: false,
            }],
        }
    }
    fn plane(z: f64, alt: f64) -> Aircraft {
        Aircraft {
            position: [0., alt, z],
            forward: [0., 0., 1.],
            nav_mode: true,
            gear_down: true,
            supported: false,
            alive: true,
            speed_fps: 100.,
            ground_clearance_ft: 0.,
            redfor: false,
        }
    }
    #[test]
    fn runway_plane_does_not_use_the_height_of_attached_buildings() {
        let mut s = scene();
        s.runways[0].surface.center[1] = 500.;
        s.runways[0].surface.half[1] = 400.;
        assert_eq!(s.runway_surface(0., 0.), Some((1000, 100.)));
        s.runways[0].surface.pitch = 0.1;
        let height = s.runways[0].support_height(0., 100.).unwrap();
        assert!(height.is_finite() && (height - 100.).abs() > 1.);
    }
    #[test]
    fn departure_pose_is_inset_on_the_primary_runway() {
        let s = scene();
        let r = &s.runways[0];
        assert_eq!(r.departure_pose(), ([0., 100., -4900.], 0.));
        let mut short = r.clone();
        short.length_ft = 600.;
        assert_eq!(short.departure_pose().0, [0., 100., -270.]);
        short.heading = std::f64::consts::FRAC_PI_2;
        let (point, heading) = short.departure_pose();
        assert!((point[0] + 270.).abs() < 1e-9 && point[2].abs() < 1e-9);
        assert_eq!(heading, short.heading);
    }
    #[test]
    fn ils_uses_selected_airport_elevation_and_inclusive_4000_gate() {
        let s = scene();
        let mut x = Service::new(&s).unwrap();
        x.command(&s, plane(-10000., 4100.), Command::SelectAirport(7));
        x.command(&s, plane(-10000., 4100.), Command::RequestLanding);
        assert!(x.guidance(&s, plane(-10000., 4100.)).unwrap().active);
        assert!(x.guidance(&s, plane(-10000., 4100.01)).is_none());
    }
    #[test]
    fn the_glide_path_aims_at_the_touchdown_zone_and_reads_zero_on_the_path() {
        let s = scene();
        let r = &s.runways[0];
        // North-facing near end: the threshold is the runway's south end.
        let t = r.threshold(ApproachEnd::Near);
        let aim = r.aim_point(ApproachEnd::Near);
        assert_eq!(aim[1], 100.);
        assert!((aim[2] - t[2] - AIM_PAST_THRESHOLD_FT).abs() < 1e-9);
        // About 52 ft over the threshold, and zero at the aim point.
        assert!((threshold_crossing_height_ft() - 52.4).abs() < 0.1);
        assert_eq!(glide_path_height_ft(-AIM_PAST_THRESHOLD_FT), 0.);
        let x = Service::new(&s).unwrap();
        for clearance in [0., 8.5, 14.] {
            for range in [30_000., 12_000., 4_000., 900., 300.] {
                let mut p = plane(t[2] - range, 0.);
                p.ground_clearance_ft = clearance;
                // The wheels sit on the path; the origin is `clearance` higher.
                p.position[1] = aim[1] + glide_path_height_ft(range) + clearance;
                let g = x.guidance(&s, p).unwrap();
                assert!(g.glide_degrees.abs() < 1e-9, "{range}: {}", g.glide_degrees);
                assert!(g.localizer_degrees.abs() < 1e-9);
                // High reads positive (the HUD dots go down), low negative.
                let mut high = p;
                high.position[1] += 20.;
                let mut low = p;
                low.position[1] -= 20.;
                assert!(x.guidance(&s, high).unwrap().glide_degrees > 0.);
                assert!(x.guidance(&s, low).unwrap().glide_degrees < 0.);
                // Right of the centre line reads positive (the bar moves left).
                let mut right = p;
                right.position[0] += 30.;
                let mut left = p;
                left.position[0] -= 30.;
                assert!(x.guidance(&s, right).unwrap().localizer_degrees > 0.);
                assert!(x.guidance(&s, left).unwrap().localizer_degrees < 0.);
            }
        }
    }
    #[test]
    fn wheels_on_the_runway_at_the_threshold_are_below_the_path() {
        // The old datum put the origin, not the wheels, on the path over the
        // threshold: an aircraft flying the bar would touch down short.
        let s = scene();
        let x = Service::new(&s).unwrap();
        let t = s.runways[0].threshold(ApproachEnd::Near);
        let mut p = plane(t[2] - 300., 100. + 10.);
        p.ground_clearance_ft = 10.;
        // Wheels on the surface 300 ft short: far below the 3 degree path.
        assert!(x.guidance(&s, p).unwrap().glide_degrees < -2.);
    }
    #[test]
    fn a_tilted_runway_moves_the_aim_point_with_its_plane() {
        let mut s = scene();
        s.runways[0].surface.pitch = 0.02;
        let r = &s.runways[0];
        let aim = r.aim_point(ApproachEnd::Near);
        let t = r.threshold(ApproachEnd::Near);
        assert_eq!(Some(aim[1]), r.support_height(aim[0], aim[2]));
        assert!((aim[1] - t[1]).abs() > 1.0);
    }
    #[test]
    fn gates_behind_and_range_nav_gear() {
        let s = scene();
        let mut x = Service::new(&s).unwrap();
        x.command(&s, plane(-10000., 1000.), Command::SelectAirport(7));
        x.command(&s, plane(-10000., 1000.), Command::RequestLanding);
        assert!(x.guidance(&s, plane(40000., 1000.)).is_none());
        let mut p = plane(-5000. - ILS_RANGE_FT, 1000.);
        assert!(x.guidance(&s, p).unwrap().active);
        p.position[2] -= 0.01;
        assert!(x.guidance(&s, p).is_none());
        p = plane(-10000., 1000.);
        p.gear_down = false;
        assert!(!x.guidance(&s, p).unwrap().active);
    }
    #[test]
    fn automatic_guidance_needs_no_clearance_and_points_toward_airport() {
        let s = scene();
        let x = Service::new(&s).unwrap();
        let mut p = plane(-10000., 4100.);
        let g = x.guidance(&s, p).unwrap();
        assert!(g.active);
        assert!(g.bearing.abs() < 1e-12);
        assert_eq!(x.clearance(), None);
        p.position[1] = 4100.001;
        assert!(x.guidance(&s, p).is_none());
        p.position[1] = 1000.;
        p.nav_mode = false;
        assert!(!x.guidance(&s, p).unwrap().active);
    }
    #[test]
    fn ils_forward_cone_is_inclusive_and_uses_normalized_three_dimensional_vectors() {
        let s = scene();
        let mut x = Service::new(&s).unwrap();
        let mut p = plane(-6000., 100.);
        x.command(&s, p, Command::SelectAirport(7));
        x.command(&s, p, Command::RequestLanding);
        let q = std::f64::consts::FRAC_1_SQRT_2;
        for forward in [[q, 0., q], [-q, 0., q], [10. * q, 0., 10. * q]] {
            p.forward = forward;
            assert!(x.guidance(&s, p).is_some(), "boundary forward={forward:?}");
        }
        p.forward = [q + 1e-6, 0., q - 1e-6];
        assert!(x.guidance(&s, p).is_none());

        p.position[1] = 1100.;
        p.forward = [0., -1., 1.];
        assert!(x.guidance(&s, p).is_some());
        p.forward = [0., 1., 0.];
        assert!(x.guidance(&s, p).is_none());
        for bad in [
            [0.; 3],
            [f64::NAN, 0., 1.],
            [f64::INFINITY, 0., 1.],
            [f64::MAX, 0., f64::MAX],
        ] {
            p.forward = bad;
            assert!(x.guidance(&s, p).is_none());
        }
        p.position[1] = 100.;
        p.forward = [0., 0., 1.];
        assert!(x.guidance(&s, p).is_some());
        assert_eq!(x.clearance(), Some((7, 1000, ApproachEnd::Near)));

        let mut wrap = Service::new(&s).unwrap();
        let mut p = plane(6000., 100.);
        wrap.command(&s, p, Command::SelectAirport(7));
        wrap.command(&s, p, Command::RequestLanding);
        for degrees in [-179_f64, 179.] {
            let yaw = degrees.to_radians();
            p.forward = [yaw.sin(), 0., yaw.cos()];
            assert!(wrap.guidance(&s, p).is_some(), "yaw wrap {degrees}");
        }
    }

    #[test]
    fn ils_leaves_and_reenters_range_and_altitude_band() {
        let s = scene();
        let mut x = Service::new(&s).unwrap();
        let mut p = plane(-5000. - ILS_RANGE_FT, 4100.);
        x.command(&s, p, Command::SelectAirport(7));
        x.command(&s, p, Command::RequestLanding);
        assert!(x.guidance(&s, p).is_some());
        p.position[2] -= 0.01;
        assert!(x.guidance(&s, p).is_none());
        p.position[2] += 0.01;
        p.position[1] += 0.01;
        assert!(x.guidance(&s, p).is_none());
        p.position[1] -= 0.01;
        assert!(x.guidance(&s, p).is_some());
    }

    #[test]
    fn automatic_search_ignores_nearer_airport_behind_aircraft() {
        let mut s = scene();
        let mut behind = s.runways[0].clone();
        behind.object = 2000;
        behind.airport = 8;
        // Its far threshold is behind the aircraft but on a valid approach
        // side, so rejection must exercise the nose cone rather than the
        // already-existing behind-threshold gate.
        behind.surface.center[2] = -16000.;
        behind.approach_center[2] = -16000.;
        let mut object = s.objects[0].clone();
        object.id = 2000;
        object.bounds.center[2] = -16000.;
        s.objects.push(object);
        s.runways.push(behind);
        s.airports.push(Airport {
            id: 8,
            name: "Behind".into(),
            runway_objects: vec![2000],
            allegiance: Allegiance::Friendly,
            neutral_permission: false,
        });
        let x = Service::new(&s).unwrap();
        let guidance = x.guidance(&s, plane(-10000., 1000.)).unwrap();
        assert_eq!(guidance.airport, 7);
    }
    #[test]
    fn a_runways_layout_owner_gives_the_airport_its_side() {
        assert_eq!(Allegiance::of_owner(Some(false)), Allegiance::Friendly);
        assert_eq!(Allegiance::of_owner(Some(true)), Allegiance::Hostile);
        assert_eq!(Allegiance::of_owner(None), Allegiance::Neutral);
        // A Redfor pilot sees friendly and hostile swapped, nothing else.
        assert_eq!(Allegiance::Friendly.seen_by(true), Allegiance::Hostile);
        assert_eq!(Allegiance::Hostile.seen_by(true), Allegiance::Friendly);
        assert_eq!(Allegiance::Neutral.seen_by(true), Allegiance::Neutral);
        assert_eq!(Allegiance::Unknown.seen_by(true), Allegiance::Unknown);
        assert_eq!(Allegiance::Hostile.seen_by(false), Allegiance::Hostile);
        let mut a = scene().airports.remove(0);
        assert!(a.serves(false) && !a.serves(true));
        a.allegiance = Allegiance::Hostile;
        assert!(!a.serves(false) && a.serves(true));
        // A neutral field serves both sides when it grants permission.
        a.allegiance = Allegiance::Neutral;
        assert!(!a.serves(false) && !a.serves(true));
        a.neutral_permission = true;
        assert!(a.serves(false) && a.serves(true));
        a.allegiance = Allegiance::Unknown;
        assert!(!a.serves(false) && !a.serves(true));
    }
    #[test]
    fn the_tower_clears_a_pilot_only_at_its_own_sides_field() {
        let s = scene();
        let mut blue = plane(-10000., 1000.);
        let mut red = blue;
        red.redfor = true;
        // Blue's field: Blue is cleared, Redfor is told it is hostile.
        let mut x = Service::new(&s).unwrap();
        x.command(&s, red, Command::SelectAirport(7));
        assert_eq!(
            x.command(&s, red, Command::RequestLanding),
            vec![Event::Reply(Reply::Declined {
                airport: Some(7),
                reason: DeclineReason::Hostile
            })]
        );
        x.command(&s, blue, Command::RequestLanding);
        assert_eq!(x.clearance(), Some((7, 1000, ApproachEnd::Near)));
        // Redfor's field: the other way round.
        let mut s = s;
        s.airports[0].allegiance = Allegiance::Hostile;
        let mut x = Service::new(&s).unwrap();
        x.command(&s, blue, Command::SelectAirport(7));
        assert!(matches!(
            x.command(&s, blue, Command::RequestLanding)[0],
            Event::Reply(Reply::Declined {
                reason: DeclineReason::Hostile,
                ..
            })
        ));
        x.command(&s, red, Command::RequestLanding);
        assert_eq!(x.clearance(), Some((7, 1000, ApproachEnd::Near)));
        // Automatic guidance finds only the side's own fields.
        let x = Service::new(&s).unwrap();
        blue.position[1] = 1000.;
        assert!(x.guidance(&s, blue).is_none());
        assert_eq!(x.guidance(&s, red).unwrap().airport, 7);
    }
    #[test]
    fn repeating_preserves_reply_and_approach_end() {
        let s = scene();
        let mut x = Service::new(&s).unwrap();
        let p = plane(-10000., 1000.);
        x.command(&s, p, Command::SelectAirport(7));
        let original = x.command(&s, p, Command::RequestLanding);
        assert_eq!(
            x.command(&s, plane(10000., 1000.), Command::RequestLanding),
            original
        );
        let stored = x.last_reply.clone();
        for _ in 0..1000 {
            x.command(&s, p, Command::RepeatReply);
        }
        assert_eq!(x.last_reply, stored);
        assert_eq!(x.clearance(), Some((7, 1000, ApproachEnd::Near)));
    }
    #[test]
    fn external_combat_health_controls_availability_once() {
        let s = scene();
        let mut x = Service::new(&s).unwrap();
        let p = plane(-10000., 1000.);
        x.command(&s, p, Command::SelectAirport(7));
        x.command(&s, p, Command::RequestLanding);
        assert!(x.synchronize_health([(1000, 50)]).is_empty());
        assert!(x.usable(1000));
        assert_eq!(
            x.synchronize_health([(1000, 0)]),
            vec![
                Event::TargetDestroyed(1000),
                Event::ClearanceInvalidated(1000)
            ]
        );
        assert!(x.synchronize_health([(1000, 0)]).is_empty());
        assert!(x.guidance(&s, p).is_none());
        x.reset(&s).unwrap();
        assert!(x.usable(1000));
        assert!(x.clearance().is_none());
    }
    #[test]
    fn tilted_contact_uses_all_three_source_angles() {
        let b = OrientedBox {
            center: [20., 30., 40.],
            half: [5., 1., 20.],
            heading: 0.4,
            pitch: 0.5,
            bank: 0.6,
        };
        let basis = crate::attitude::Basis::new(b.heading, b.pitch, b.bank);
        let point = |distance: f64| std::array::from_fn(|i| b.center[i] + basis.up[i] * distance);
        assert!((b.segment_fraction(point(10.), point(-10.)).unwrap() - 0.45).abs() < 1e-12);
        let mut bad = b;
        bad.center[0] = f64::NAN;
        assert!(!bad.valid());
    }
    #[test]
    fn destruction_is_once_and_invalidates() {
        let s = scene();
        let mut x = Service::new(&s).unwrap();
        x.command(&s, plane(-10000., 1000.), Command::SelectAirport(7));
        x.command(&s, plane(-10000., 1000.), Command::RequestLanding);
        assert_eq!(
            x.damage(1000, 100),
            vec![
                Event::TargetDestroyed(1000),
                Event::ClearanceInvalidated(1000)
            ]
        );
        assert!(x.damage(1000, 100).is_empty());
    }
    #[test]
    fn landing_requires_240_consecutive_ticks() {
        let s = scene();
        let mut x = Service::new(&s).unwrap();
        let mut p = plane(0., 100.);
        p.supported = true;
        p.speed_fps = 10.;
        x.command(&s, p, Command::SelectAirport(7));
        x.command(&s, p, Command::RequestLanding);
        for _ in 0..239 {
            assert!(x.step(&s, p).is_empty());
        }
        assert_eq!(
            x.step(&s, p),
            vec![Event::LandingComplete {
                airport: 7,
                runway: 1000
            }]
        );
    }
    #[test]
    fn repeat_after_landing_returns_completion_not_old_clearance() {
        let s = scene();
        let mut service = Service::new(&s).unwrap();
        let mut p = plane(0., 100.);
        p.supported = true;
        p.speed_fps = 10.;
        service.command(&s, p, Command::SelectAirport(7));
        service.command(&s, p, Command::RequestLanding);
        for _ in 0..LANDING_TICKS {
            service.step(&s, p);
        }
        assert_eq!(
            service.command(&s, p, Command::RepeatReply),
            vec![Event::Reply(Reply::Repeated(Box::new(Reply::Landed {
                airport: 7,
                runway: 1000
            })))]
        );
        assert!(service.step(&s, p).is_empty());
        service.reset(&s).unwrap();
        assert!(service.last_reply().is_none());
    }
    #[test]
    fn oriented_box_segment_uses_earliest_contact() {
        let s = scene();
        assert_eq!(
            s.earliest_object_hit([0., 100., -6000.], [0., 100., 6000.]),
            Some((1000, 1. / 12.))
        );
        assert_eq!(
            s.earliest_object_hit([101., 100., -6000.], [101., 100., 6000.]),
            None
        );
    }
    #[test]
    fn a_short_strip_is_off_the_tower_and_the_guidance() {
        // The retail strips are 1,074 ft and every other airport is 4,060 ft or
        // longer; the line sits in the gap.
        assert!(short_strip_length(1_074.));
        assert!(!short_strip_length(4_060.));
        assert!(!short_strip_length(SHORT_STRIP_FT));
        let mut s = scene();
        assert!(!s.short_strip(1000));
        assert!(!s.airport_id_is_short_strip(7));
        s.runways[0].length_ft = 1_074.;
        assert!(s.short_strip(1000));
        assert!(s.airport_id_is_short_strip(7));
        assert!(s.airport_is_short_strip(&s.airports[0]));
        // An unknown runway or airport is not a short strip.
        assert!(!s.short_strip(9));
        assert!(!s.airport_id_is_short_strip(99));
        let mut service = Service::new(&s).unwrap();
        let p = plane(-20_000., 2_000.);
        // The airport cannot be selected, so no clearance follows.
        assert_eq!(
            service.command(&s, p, Command::SelectAirport(7)),
            vec![Event::Reply(Reply::Declined {
                airport: None,
                reason: DeclineReason::NoAirport
            })]
        );
        assert_eq!(service.selected(), None);
        assert_eq!(
            service.command(&s, p, Command::RequestLanding),
            vec![Event::Reply(Reply::Declined {
                airport: None,
                reason: DeclineReason::NoAirport
            })]
        );
        // Automatic guidance never picks it, even lined up on its centre line.
        assert!(service.guidance(&s, p).is_none());
        // The same field at full length is served as before.
        s.runways[0].length_ft = 10_000.;
        assert!(service.guidance(&s, p).is_some());
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "airport_checkpoint.rs"]
mod checkpoint;
