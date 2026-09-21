//! Deterministic airport surfaces, player landing service, and static identity.
//! Geometry and policy follow docs/spec/airports.md. No autonomous traffic lives here.
use std::collections::{BTreeMap, BTreeSet};

pub const ILS_RANGE_FT: f64 = 5.0 * 6_076.12;
pub const ILS_ALTITUDE_AGL_FT: f64 = 4_000.0;
pub const LANDING_SPEED_FPS: f64 = 30.0 * 6_076.12 / 3_600.0;
pub const LANDING_TICKS: u16 = 240;

pub type ObjectId = u32;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceKey {
    pub layout: String,
    pub ordinal: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allegiance {
    Friendly,
    Neutral,
    Hostile,
    Unknown,
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
        let (s, c) = self.heading.sin_cos();
        let dx = x - self.center[0];
        let dz = z - self.center[2];
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
    pub fn departure_pose(&self) -> ([f64; 3], f64) {
        let mut position = self.threshold(ApproachEnd::Near);
        let inset = (self.length_ft * 0.05).min(100.);
        position[0] += self.heading.sin() * inset;
        position[2] += self.heading.cos() * inset;
        (position, self.heading)
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
    pub nav_mode: bool,
    pub gear_down: bool,
    pub supported: bool,
    pub alive: bool,
    pub speed_fps: f64,
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
            .filter(|id| self.usable(**id))
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
                if scene.airports.iter().any(|a| a.id == id) {
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
                let reason = match a.allegiance {
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
                scene
                    .airports
                    .iter()
                    .filter_map(|a| {
                        let (runway, end) = self.choose(scene, aircraft, a.id)?;
                        let distance =
                            distance2(aircraft.position, scene.runway(runway)?.threshold(end));
                        (distance <= ILS_RANGE_FT.powi(2)).then_some((a.id, runway, end, distance))
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
        let dx = aircraft.position[0] - threshold[0];
        let dz = aircraft.position[2] - threshold[2];
        let forward = -(dx * heading.sin() + dz * heading.cos());
        if forward <= 0. {
            return None;
        }
        let lateral = dx * heading.cos() - dz * heading.sin();
        let range = (dx * dx + dz * dz).sqrt();
        let localizer = (lateral / forward).atan().to_degrees();
        let glide = ((aircraft.position[1] - r.elevation_ft) / forward)
            .atan()
            .to_degrees()
            - 3.;
        let active = aircraft.alive
            && aircraft.nav_mode
            && aircraft.gear_down
            && range <= ILS_RANGE_FT
            && aircraft.position[1] - r.elevation_ft <= ILS_ALTITUDE_AGL_FT;
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
            nav_mode: true,
            gear_down: true,
            supported: false,
            alive: true,
            speed_fps: 100.,
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
        assert!(!x.guidance(&s, plane(-10000., 4100.01)).unwrap().active);
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
        assert!(!x.guidance(&s, p).unwrap().active);
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
        assert!(!x.guidance(&s, p).unwrap().active);
        p.position[1] = 1000.;
        p.nav_mode = false;
        assert!(!x.guidance(&s, p).unwrap().active);
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
}
