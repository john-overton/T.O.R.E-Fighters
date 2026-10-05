//! Airport and wing status cues. Observes flight only; never flies aircraft.
//! Behavior and provenance: docs/spec/airfield-radio.md.
//!
//! Every notice queued, coalesced, pushed out, expired or cancelled is
//! written to the channel's journal with its trigger.
//!
//! Each human-flown plane has an [`AirfieldRadio`] for its own side of the
//! tower conversation: departure, approach and the notices that wait to be
//! sent to its seat. What its wingmen report is decided once for the whole
//! mission by [`WingStatus`], and every seat whose wing it is queues the same
//! report.
use crate::{
    ai_wings::{AiWings, Member},
    comms::journal::{Audience, Cause, Entry, Origin, Outcome, Reason, Roll, Source, TowerEvent},
    comms::{Call, Comms, Hearer, Kind, Phrase, Phrases},
    seats::SeatId,
    terrain::Terrain,
};
use std::collections::{BTreeMap, VecDeque};
use tore_sim::flight;
use tore_sim::{
    ai::airfield::{Phase, RunwayView},
    airport::{ApproachEnd, Reply, Service},
};

const INTERVAL: f64 = 3.;
const EXPIRES: f64 = 15.;
const RANGE_FT: f64 = 7. * 6076.12;
/// The actor a plane's own notices are filed under in its queue, apart from
/// its wingmen's ids.
const PLAYER: u32 = u32::MAX;
/// Notices waiting at most; a new one pushes out the oldest.
const QUEUE: usize = 24;

struct Notice {
    actor: u32,
    key: u8,
    until: f64,
    /// When it was queued, for the journal.
    queued: f64,
    call: Call,
}

/// A tower line to the player about `event`.
fn tower_origin(event: TowerEvent) -> Origin {
    Origin::of(Source::Tower, Cause::Tower(event)).to(Audience::Player)
}
#[derive(Default)]
struct Departure {
    runway: Option<RunwayView>,
    cleared: bool,
    airborne: bool,
    farewell: bool,
    hold: bool,
}
#[derive(Default)]
struct Approach {
    runway: Option<RunwayView>,
    wind: bool,
    landed: bool,
    welcomed: bool,
}
/// One human-flown plane's tower conversation and the notices waiting for its
/// seat.
#[derive(Default)]
pub struct AirfieldRadio {
    /// The seat it speaks to, and the plane that seat flies.
    seat: SeatId,
    plane: u32,
    departure: Departure,
    approach: Approach,
    clearance_announced: Option<u32>,
    landing_count: u32,
    landing_score: u32,
    pending: VecDeque<Notice>,
    next: f64,
    /// Journal entries made away from the channel (queueing, tower
    /// replies), handed to it at the next delivery.
    notes: Vec<Entry>,
    /// The latest time seen, for entries made by methods given none.
    clock: f64,
}

fn recorded(phrases: &Phrases, stem: &str) -> Phrase {
    let mut phrase = Phrase::stem(phrases, stem);
    // The reviewed table includes sentence fragments after a callsign.
    phrase.text = phrase.text.trim_start_matches([',', ' ']).to_owned();
    phrase
}
fn call(
    phrases: &Phrases,
    tower: &str,
    stem: &str,
    recipient: Option<&str>,
    important: bool,
) -> Call {
    let phrase = recorded(phrases, stem);
    let label = recipient.map_or_else(|| tower.to_owned(), |who| format!("{tower} to {who}"));
    Call::new(
        label,
        phrase,
        if important {
            Kind::Important
        } else {
            Kind::Chatter
        },
    )
    .airport()
}
fn busy(wings: Option<&AiWings>, runway: RunwayView, except: u32) -> bool {
    wings.is_some_and(|w| {
        w.mission().actors().iter().any(|a| {
            a.id() != except
                && a.alive()
                && a.airfield()
                    .is_some_and(|s| s.runway().airport == runway.airport && s.blocks_runway())
        })
    })
}
fn tower(world: &Terrain, airport: u32) -> String {
    world
        .airport_scene
        .airports
        .iter()
        .find(|a| a.id == airport)
        .map_or_else(|| "Tower".into(), |a| format!("{} tower", a.name))
}
impl AirfieldRadio {
    /// The tower conversation of the plane with id `plane`, spoken to `seat`.
    pub fn for_seat(seat: SeatId, plane: u32) -> Self {
        Self {
            seat,
            plane,
            ..Self::default()
        }
    }
    pub fn reset(&mut self, departure: Option<RunwayView>) {
        *self = Self::for_seat(self.seat, self.plane);
        self.departure.runway = departure;
    }
    /// Keep manual requests authoritative and avoid speaking clearance twice.
    pub fn reply(&mut self, reply: &Reply) {
        match reply {
            Reply::Cleared { runway, .. } => self.clearance_announced = Some(*runway),
            Reply::Repeated(inner) => self.reply(inner),
            Reply::Selected { .. } | Reply::Cancelled { .. } | Reply::Declined { .. } => {
                self.approach = Approach::default();
                self.clearance_announced = None;
                self.retain(
                    self.clock,
                    |n| n.actor != PLAYER,
                    |_| Outcome::Cancelled(Reason::TowerReply),
                );
            }
            Reply::Landed { .. } => self.approach.welcomed = true,
        }
    }
    /// The player's aircraft is lost: forget the player's airfield state.
    pub fn invalidate(&mut self) {
        self.retain(
            self.clock,
            |n| n.actor != PLAYER,
            |_| Outcome::Cancelled(Reason::AircraftLost),
        );
        self.approach = Approach::default();
        self.departure = Departure::default();
        self.clearance_announced = None;
    }
    /// Keep the notices `keep` accepts; journal each other one with `gone`.
    fn retain(
        &mut self,
        at: f64,
        keep: impl Fn(&Notice) -> bool,
        gone: impl Fn(&Notice) -> Outcome,
    ) {
        let mut kept = VecDeque::with_capacity(self.pending.len());
        for notice in self.pending.drain(..) {
            if keep(&notice) {
                kept.push_back(notice);
            } else {
                self.notes
                    .push(Entry::call(at, None, &notice.call, gone(&notice)).heard_by([self.seat]));
            }
        }
        self.pending = kept;
    }
    fn queue(&mut self, now: f64, actor: u32, key: u8, call: Call) {
        self.clock = now;
        self.retain(
            now,
            |n| n.actor != actor || n.key != key,
            |_| Outcome::Replaced(Reason::Coalesced),
        );
        if self.pending.len() >= QUEUE
            && let Some(oldest) = self.pending.pop_front()
        {
            self.notes.push(
                Entry::call(
                    now,
                    None,
                    &oldest.call,
                    Outcome::Dropped(Reason::QueueFull { limit: QUEUE }),
                )
                .heard_by([self.seat]),
            );
        }
        self.notes.push(
            Entry::call(
                now,
                None,
                &call,
                Outcome::Queued {
                    due: now,
                    expires: Some(now + EXPIRES),
                },
            )
            .heard_by([self.seat]),
        );
        self.pending.push_back(Notice {
            actor,
            key,
            until: now + EXPIRES,
            queued: now,
            call,
        });
    }
    /// Send the next waiting notice to the seat, if the channel is free.
    pub fn deliver(&mut self, now: f64, comms: &mut Comms) {
        self.clock = now;
        self.retain(
            now,
            |n| n.until >= now,
            |n| {
                Outcome::Expired(Reason::OlderThan {
                    seconds: EXPIRES,
                    age: now - n.queued,
                })
            },
        );
        comms.record_all(self.notes.drain(..));
        if now < self.next || !comms.channel_free(self.seat, now) {
            return;
        }
        let next = self
            .pending
            .iter()
            .position(|n| n.actor == PLAYER)
            .unwrap_or(0);
        if let Some(mut notice) = self.pending.remove(next) {
            notice.call.origin.since = Some(notice.queued);
            let heard = !comms.radio_silence(self.seat) || notice.call.kind == Kind::Important;
            comms.send(now, notice.call, &[Hearer::seat(self.seat)]);
            if heard {
                // Reserve the channel now, before this tick's crew-comment
                // producer runs. Playback will retain the same three-second hold.
                comms.spoken(self.seat, now);
            }
            self.next = now + INTERVAL;
        }
    }
    /// The plane's own tower conversation for one tick: departure, approach
    /// and landing calls are queued. When the aircraft is lost the queue is
    /// cancelled instead and this returns false, and nothing else of the
    /// airfield radio runs for it this tick.
    #[allow(clippy::too_many_arguments)]
    pub fn step_player(
        &mut self,
        now: f64,
        phrases: &Phrases,
        comms: &mut Comms,
        flight: &flight::State,
        world: &Terrain,
        service: &Service,
        wings: Option<&AiWings>,
    ) -> bool {
        self.clock = now;
        if flight.crashed || flight.escape.is_some() || flight.systems.pilot.dead {
            self.invalidate();
            self.retain(now, |_| false, |_| Outcome::Cancelled(Reason::AircraftLost));
            comms.record_all(self.notes.drain(..));
            comms.cancel_airport_because(self.seat, Reason::AircraftLost);
            return false;
        }
        self.player(now, phrases, comms, flight, world, service, wings);
        true
    }
    /// Queue what the wingmen reported this tick, or cancel what a wingman's
    /// change or death made stale. `events` are those of this plane's wing.
    pub fn apply_wing(&mut self, now: f64, events: &[WingEvent]) {
        self.clock = now;
        for event in events {
            match event {
                WingEvent::Down { member } => self.retain(
                    now,
                    |n| n.actor != *member,
                    |_| Outcome::Cancelled(Reason::WingmanDown),
                ),
                WingEvent::Changed { member, report } => {
                    // Free flight has no airfield status and invalidates pending taxi reports.
                    self.retain(
                        now,
                        |n| n.actor != *member,
                        |_| Outcome::Replaced(Reason::StatusChanged),
                    );
                    if let Some(report) = report {
                        self.queue(now, *member, 0, report.clone());
                    }
                }
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn player(
        &mut self,
        now: f64,
        phrases: &Phrases,
        comms: &mut Comms,
        f: &flight::State,
        world: &Terrain,
        service: &Service,
        wings: Option<&AiWings>,
    ) {
        let supported = f.supported_at(world.surface(f.position[0], f.position[2]).height);
        if let Some(runway) = self.departure.runway {
            let tower = tower(world, runway.airport);
            if !service.usable(runway.object) {
                self.departure = Departure::default();
                self.retain(
                    now,
                    |n| n.actor != PLAYER,
                    |_| Outcome::Cancelled(Reason::RunwayUnusable),
                );
            } else if supported && !self.departure.cleared {
                if !busy(wings, runway, self.plane) {
                    self.departure.cleared = true;
                    let stem = "^TAKOFF1";
                    let clearance = call(phrases, &tower, stem, None, true)
                        .because(tower_origin(TowerEvent::TakeoffClearance));
                    self.queue(now, PLAYER, 1, clearance);
                } else if !self.departure.hold {
                    self.departure.hold = true;
                    self.queue(
                        now,
                        PLAYER,
                        1,
                        Call::new(
                            tower,
                            Phrase::default().raw("Hold position, runway occupied", None),
                            Kind::Important,
                        )
                        .airport()
                        .because(tower_origin(TowerEvent::RunwayOccupied)),
                    );
                }
            } else if !supported && !self.departure.airborne {
                self.departure.airborne = true;
                self.queue(
                    now,
                    PLAYER,
                    2,
                    call(phrases, &tower, "^AIRBORN", None, true)
                        .because(tower_origin(TowerEvent::Airborne)),
                );
            } else if !supported
                && f.position[1] - runway.elevation_ft > 10.
                && !self.departure.farewell
            {
                self.departure.farewell = true;
                let roll = comms.roll();
                let stem = Comms::pick(roll, &["^GDLUCK", "^GDHUNT"]);
                let origin = tower_origin(TowerEvent::Farewell).rolls(vec![Roll::pick(
                    "farewell call",
                    roll,
                    2,
                )]);
                self.queue(
                    now,
                    PLAYER,
                    3,
                    call(phrases, &tower, stem, None, false).because(origin),
                );
            }
        }
        if self.approach.runway.is_none()
            && !supported
            && f.gear_down
            && self
                .departure
                .runway
                .is_none_or(|r| self.departure.farewell && distance(f.position, r.center) > 3000.)
        {
            self.approach.runway = approach_runway(f, world, service);
        }
        if let Some(runway) = self.approach.runway {
            if !service.usable(runway.object)
                || (!supported
                    && (!f.gear_down || distance(f.position, runway.center) > RANGE_FT * 1.2))
            {
                self.approach = Approach::default();
                self.clearance_announced = None;
                let reason = if service.usable(runway.object) {
                    Reason::ApproachLeft
                } else {
                    Reason::RunwayUnusable
                };
                self.retain(
                    now,
                    |n| n.actor != PLAYER || n.key < 4,
                    |_| Outcome::Cancelled(reason.clone()),
                );
                return;
            }
            let tower = tower(world, runway.airport);
            if self.clearance_announced != Some(runway.object) && !busy(wings, runway, self.plane) {
                self.clearance_announced = Some(runway.object);
                self.queue(
                    now,
                    PLAYER,
                    4,
                    call(phrases, &tower, "^CLRLAND", None, true)
                        .because(tower_origin(TowerEvent::LandingClearance)),
                );
            } else if self.clearance_announced == Some(runway.object) && !self.approach.wind {
                self.approach.wind = true;
                let w = world.wind();
                let knots = (w[0].hypot(w[2]) / 1.68781).round().clamp(0., 200.) as u32;
                let phrase = recorded(phrases, "^WINDAT")
                    .join(crate::comms::number(knots, false))
                    .then(phrases, "^KNOTS");
                self.queue(
                    now,
                    PLAYER,
                    5,
                    Call::new(tower.clone(), phrase, Kind::Important)
                        .airport()
                        .because(tower_origin(TowerEvent::Wind { knots })),
                );
            }
            if let Some(r) = &f.research
                && r.landings.count > self.landing_count
            {
                let score = r.landings.score.saturating_sub(self.landing_score)
                    / (r.landings.count - self.landing_count);
                self.approach.landed = true;
                let stem = if score >= 100 {
                    "^GDLAND"
                } else if score >= 50 {
                    "^FRLAND"
                } else {
                    "^BADLAND"
                };
                let origin = tower_origin(TowerEvent::LandingGrade { score });
                self.queue(
                    now,
                    PLAYER,
                    6,
                    call(phrases, &tower, stem, None, true).because(origin),
                );
            }
            if self.approach.landed
                && supported
                && f.velocity[0].hypot(f.velocity[2]) < 10.
                && !self.approach.welcomed
            {
                self.approach.welcomed = true;
                let roll = comms.roll();
                let stem = Comms::pick(roll, &["^WELBACK", "^WELHOME"]);
                let origin = tower_origin(TowerEvent::Welcome).rolls(vec![Roll::pick(
                    "welcome call",
                    roll,
                    2,
                )]);
                self.queue(
                    now,
                    PLAYER,
                    7,
                    call(phrases, &tower, stem, None, true).because(origin),
                );
            }
        }
        if let Some(r) = &f.research {
            self.landing_count = r.landings.count;
            self.landing_score = r.landings.score;
        }
    }
}
/// What one wingman's status report does to the queue of every seat in its
/// wing.
#[derive(Clone, Debug)]
pub enum WingEvent {
    /// The wingman is down: its waiting reports are cancelled.
    Down { member: u32 },
    /// The wingman's status changed: its waiting reports are replaced by
    /// `report`, when it has a new one.
    Changed { member: u32, report: Option<Call> },
}

/// A [`WingEvent`] and the flight (radio flight number) whose seats get it.
#[derive(Clone, Debug)]
pub struct WingReport {
    pub flight: u8,
    pub event: WingEvent,
}

/// The airfield status of the AI wingmen, watched once for the mission. A
/// change is turned into a report once, with one variant roll, and every
/// seat in the wingman's flight queues that same report.
#[derive(Default)]
pub struct WingStatus {
    /// Each wingman's last phase and go-around count.
    memory: BTreeMap<u32, (Option<Phase>, u32)>,
}

impl WingStatus {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// One tick. `members` are the mission's radio members; the flights in
    /// `listening` are those with a human seat, and only their wingmen make
    /// reports.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        phrases: &Phrases,
        comms: &mut Comms,
        world: &Terrain,
        wings: &AiWings,
        members: &[Member],
        listening: &[u8],
    ) -> Vec<WingReport> {
        let mut reports = Vec::new();
        for member in members {
            let heard = listening.contains(&member.flight);
            if !member.alive {
                if heard {
                    reports.push(WingReport {
                        flight: member.flight,
                        event: WingEvent::Down { member: member.id },
                    });
                }
                continue;
            }
            let Some(actor) = wings.mission().actor(member.id) else {
                continue;
            };
            let phase = actor.airfield_phase();
            let turns = actor.airfield().map_or(0, |s| s.go_arounds());
            let previous = self.memory.insert(member.id, (phase, turns));
            if previous == Some((phase, turns)) || !heard {
                continue;
            }
            let mut change = |report| {
                reports.push(WingReport {
                    flight: member.flight,
                    event: WingEvent::Changed {
                        member: member.id,
                        report,
                    },
                });
            };
            let Some(sequence) = actor.airfield() else {
                change(None);
                continue;
            };
            let label = crate::radio_calls::label(member);
            let tower = tower(world, sequence.runway().airport);
            let go_around = previous.is_some_and(|(_, old)| turns > old);
            let mut rolls = Vec::new();
            let (text, stem) = if go_around {
                ("Going around", Some("^GOARND"))
            } else {
                match phase {
                    Some(Phase::Waiting) => ("Holding short for takeoff", None),
                    Some(Phase::Taxi) => ("Taxiing to the runway", None),
                    Some(Phase::LineUp) => ("Cleared for takeoff", {
                        let roll = comms.roll();
                        rolls.push(Roll::pick("takeoff call", roll, 2));
                        Some(Comms::pick(roll, &["^TAKOFF1", "^RDYROLL"]))
                    }),
                    Some(Phase::TakeoffRoll) => ("Taking off", None),
                    Some(Phase::ClimbOut) => ("Airborne", Some("^AIRBORN")),
                    Some(Phase::Inbound) => ("Returning to base", None),
                    Some(Phase::Marshal) => ("Holding at marshal", None),
                    Some(Phase::Approach) => ("Cleared to land", Some("^CLRLAND")),
                    Some(Phase::Final) => ("On final", None),
                    Some(Phase::Rollout) => (
                        "Landed, slowing on the runway",
                        actor
                            .flight()
                            .research
                            .as_ref()
                            .and_then(|r| r.landings.grade())
                            .map(|grade| if grade >= 100 { "^GDLAND" } else { "^FRLAND" }),
                    ),
                    Some(Phase::TaxiClear) => ("Taxiing clear", None),
                    Some(Phase::Parked) => ("Parked", Some("^WELHOME")),
                    None => {
                        change(None);
                        continue;
                    }
                }
            };
            let report = stem.map_or_else(
                || {
                    Call::new(
                        label.clone(),
                        Phrase::default().raw(text, None),
                        Kind::Chatter,
                    )
                },
                |stem| call(phrases, &tower, stem, Some(&label), false),
            );
            let origin = Origin::of(Source::Tower, Cause::WingStatus { phase, go_around })
                .by(member.id)
                .to(Audience::Airport)
                .rolls(rolls);
            change(Some(report.because(origin)));
        }
        reports
    }
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}
fn approach_runway(f: &flight::State, world: &Terrain, service: &Service) -> Option<RunwayView> {
    world
        .airport_scene
        .airports
        .iter()
        .filter(|a| service.selected().is_none_or(|id| id == a.id))
        .filter(|a| {
            matches!(a.allegiance, tore_sim::airport::Allegiance::Friendly)
                || (a.allegiance == tore_sim::airport::Allegiance::Neutral && a.neutral_permission)
        })
        .flat_map(|a| &a.runway_objects)
        .filter(|id| {
            service.usable(**id)
                && !world.airport_scene.vertical_pad(**id)
                // The tower does not work a short strip (John, 2026-09-30).
                && !world.airport_scene.short_strip(**id)
        })
        .filter_map(|id| world.airport_scene.runway(*id))
        .filter(|r| {
            f.position[1] - r.elevation_ft < 4000.
                && [ApproachEnd::Near, ApproachEnd::Far].iter().any(|&end| {
                    let p = r.threshold(end);
                    let dx = p[0] - f.position[0];
                    let dz = p[2] - f.position[2];
                    let range = dx.hypot(dz);
                    let h = r.approach_heading(end);
                    range <= RANGE_FT
                        && dx * h.sin() + dz * h.cos() > 0.
                        && (dx * f.yaw.sin() + dz * f.yaw.cos()) / range.max(1.)
                            >= std::f64::consts::FRAC_1_SQRT_2
                })
        })
        .min_by(|a, b| {
            distance(f.position, a.approach_center)
                .total_cmp(&distance(f.position, b.approach_center))
        })
        .and_then(|r| world.runway_view(r.object))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::airport::{
        Airport, Allegiance, OrientedBox, Runway, Scene, SourceKey, StaticObject,
    };
    fn fixture() -> (Terrain, Service, flight::State, Phrases) {
        let mut world = crate::test_support::terrain();
        let bounds = OrientedBox {
            center: [0., 100., 0.],
            half: [500., 10., 5000.],
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        };
        world.airport_scene = Scene {
            objects: vec![StaticObject {
                id: 1000,
                source: SourceKey {
                    layout: "TEST.MM".into(),
                    ordinal: 0,
                },
                name: "Fixture".into(),
                object_type: "STRIP.OT".into(),
                bounds,
                hit_points: 100,
                category: 0x100,
                radar_signature: 0.,
                infrared_signature: 0.,
                runway: true,
            }],
            runways: vec![Runway {
                object: 1000,
                airport: 7,
                name: "Fixture".into(),
                surface: bounds,
                approach_center: bounds.center,
                elevation_ft: 100.,
                heading: 0.,
                length_ft: 10000.,
            }],
            airports: vec![Airport {
                id: 7,
                name: "Fixture".into(),
                runway_objects: vec![1000],
                allegiance: Allegiance::Friendly,
                neutral_permission: false,
            }],
        };
        let service = Service::new(&world.airport_scene).unwrap();
        let mut f =
            flight::State::new(&crate::test_support::profile(), [0., 108., -4900.]).unwrap();
        f.enable_research(1).unwrap();
        f.start_on_runway([0., 100., -4900.], 0.).unwrap();
        let phrases = tore_formats::radio::STEMS
            .iter()
            .map(|(stem, _)| (stem.to_string(), format!("fixture {stem}")))
            .collect();
        (world, service, f, phrases)
    }
    fn tick(
        r: &mut AirfieldRadio,
        c: &mut Comms,
        p: &Phrases,
        w: &Terrain,
        s: &Service,
        f: &flight::State,
        t: f64,
    ) -> Vec<Call> {
        if r.step_player(t, p, c, f, w, s, None) {
            r.deliver(t, c);
        }
        c.due(t).into_iter().map(|d| d.call).collect()
    }
    #[test]
    fn startup_departure_and_restart_have_clearance_without_false_landings() {
        let (w, s, mut f, p) = fixture();
        let mut r = AirfieldRadio::default();
        let mut c = Comms::new(1);
        r.reset(w.runway_view(1000));
        assert_eq!(
            tick(&mut r, &mut c, &p, &w, &s, &f, 0.)[0].stems,
            ["^TAKOFF1"]
        );
        for t in 1..10 {
            assert!(tick(&mut r, &mut c, &p, &w, &s, &f, f64::from(t)).is_empty());
        }
        f.research.as_mut().unwrap().on_ground = false;
        f.position[1] = 130.;
        f.velocity = [0., 20., 300.];
        assert_eq!(
            tick(&mut r, &mut c, &p, &w, &s, &f, 10.)[0].stems,
            ["^AIRBORN"]
        );
        assert!(tick(&mut r, &mut c, &p, &w, &s, &f, 11.).is_empty());
        let farewell = tick(&mut r, &mut c, &p, &w, &s, &f, 13.);
        assert!(matches!(
            farewell[0].stems[0].as_str(),
            "^GDLUCK" | "^GDHUNT"
        ));
        r.reset(w.runway_view(1000));
        c.restart(1);
        f.start_on_runway([0., 100., -4900.], 0.).unwrap();
        assert_eq!(
            tick(&mut r, &mut c, &p, &w, &s, &f, 0.)[0].stems,
            ["^TAKOFF1"]
        );
    }
    #[test]
    fn approach_wind_grade_and_welcome_are_one_shot_and_cancel_cleanly() {
        let (w, s, mut f, p) = fixture();
        let mut r = AirfieldRadio::default();
        let mut c = Comms::new(1);
        f.research.as_mut().unwrap().on_ground = false;
        f.position = [0., 1100., -10_000.];
        f.velocity = [0., -20., 300.];
        assert_eq!(
            tick(&mut r, &mut c, &p, &w, &s, &f, 0.)[0].stems,
            ["^CLRLAND"]
        );
        assert!(tick(&mut r, &mut c, &p, &w, &s, &f, 1.).is_empty());
        let wind = tick(&mut r, &mut c, &p, &w, &s, &f, 3.);
        assert_eq!(wind[0].stems.first().unwrap(), "^WINDAT");
        assert_eq!(wind[0].stems.last().unwrap(), "^KNOTS");
        f.position = [0., 108., 0.];
        f.velocity = [0., 0., 50.];
        f.research.as_mut().unwrap().on_ground = true;
        f.research.as_mut().unwrap().landings.count = 1;
        f.research.as_mut().unwrap().landings.score = 50;
        assert_eq!(
            tick(&mut r, &mut c, &p, &w, &s, &f, 6.)[0].stems,
            ["^FRLAND"]
        );
        f.velocity = [0.; 3];
        let welcome = tick(&mut r, &mut c, &p, &w, &s, &f, 9.);
        assert!(matches!(
            welcome[0].stems[0].as_str(),
            "^WELBACK" | "^WELHOME"
        ));
        assert!(tick(&mut r, &mut c, &p, &w, &s, &f, 12.).is_empty());
        r.queue(12., PLAYER, 4, call(&p, "Fixture", "^CLRLAND", None, true));
        r.reply(&Reply::Cancelled { airport: Some(7) });
        r.deliver(15., &mut c);
        assert!(c.due(15.).is_empty());
    }
    #[test]
    fn status_coalescing_expiry_priority_and_radio_silence() {
        let mut r = AirfieldRadio::default();
        let mut c = Comms::new(1);
        let line = |text: &str, kind| Call::new("Fixture", Phrase::default().raw(text, None), kind);
        r.queue(0., 2, 0, line("Taxi", Kind::Chatter));
        r.queue(0., 2, 0, line("Airborne", Kind::Chatter));
        r.queue(0., PLAYER, 1, line("Clearance", Kind::Important).airport());
        r.deliver(0., &mut c);
        assert!(
            !c.channel_free(SeatId::default(), 0.),
            "a delivered airport line holds off crew comments immediately"
        );
        assert_eq!(c.due(0.)[0].call.text, "Clearance");
        r.deliver(1., &mut c);
        assert!(c.due(1.).is_empty());
        r.deliver(3., &mut c);
        assert_eq!(c.due(3.)[0].call.text, "Airborne");
        r.queue(3., 2, 0, line("Old", Kind::Chatter));
        r.deliver(20., &mut c);
        assert!(c.due(20.).is_empty());
        c.toggle_silence(SeatId::default());
        r.queue(20., 2, 0, line("Silent", Kind::Chatter));
        r.deliver(20., &mut c);
        assert!(c.due(20.).is_empty());
        r.queue(23., PLAYER, 1, line("Important", Kind::Important).airport());
        r.deliver(23., &mut c);
        assert_eq!(c.due(23.)[0].call.text, "Important");
    }
    #[test]
    fn tower_notices_journal_coalescing_the_full_queue_and_expiry() {
        let mut r = AirfieldRadio::default();
        let mut c = Comms::new(1);
        let line =
            |text: &str| Call::new("Fixture", Phrase::default().raw(text, None), Kind::Chatter);
        r.queue(0., 2, 0, line("Taxi"));
        r.queue(0., 2, 0, line("Airborne"));
        r.queue(0., 3, 0, line("Old"));
        r.deliver(0., &mut c);
        assert_eq!(c.due(0.)[0].call.text, "Airborne");
        r.deliver(16., &mut c);
        for actor in 10..10 + 25 {
            r.queue(20., actor, 0, line(&format!("Status {actor}")));
        }
        r.deliver(20., &mut c);
        let entries = c.take_journal();
        let outcomes = |text: &str| {
            entries
                .iter()
                .filter(|e| e.text == text)
                .map(|e| e.outcome.clone())
                .collect::<Vec<_>>()
        };
        let queued = |at: f64| Outcome::Queued {
            due: at,
            expires: Some(at + EXPIRES),
        };
        assert_eq!(
            outcomes("Taxi"),
            [queued(0.), Outcome::Replaced(Reason::Coalesced)]
        );
        assert_eq!(
            outcomes("Old"),
            [
                queued(0.),
                Outcome::Expired(Reason::OlderThan {
                    seconds: 15.,
                    age: 16.
                })
            ]
        );
        assert_eq!(
            outcomes("Status 10"),
            [
                queued(20.),
                Outcome::Dropped(Reason::QueueFull { limit: 24 })
            ]
        );
        // The tower's queue, then the channel's.
        let airborne: Vec<_> = entries.iter().filter(|e| e.text == "Airborne").collect();
        assert_eq!(airborne.len(), 3);
        assert_eq!(airborne[0].outcome, queued(0.));
        assert_eq!(airborne[2].outcome, Outcome::Delivered { waited: 0. });
        assert_eq!(
            airborne[2].origin.since,
            Some(0.),
            "queued at the tower at 0 s"
        );
        assert_eq!(airborne[1].call, airborne[2].call);
    }
    #[test]
    fn tower_replies_and_a_lost_aircraft_cancel_the_players_notices() {
        let mut r = AirfieldRadio::default();
        let mut c = Comms::new(1);
        let line = |text: &str| {
            Call::new(
                "Fixture",
                Phrase::default().raw(text, None),
                Kind::Important,
            )
            .airport()
        };
        r.queue(0., PLAYER, 4, line("Cleared to land"));
        r.queue(0., 2, 0, line("Wing status"));
        r.reply(&Reply::Cancelled { airport: Some(7) });
        r.queue(1., PLAYER, 6, line("Grade"));
        r.invalidate();
        r.deliver(1., &mut c);
        let cancelled: Vec<_> = c
            .take_journal()
            .into_iter()
            .filter(|e| matches!(e.outcome, Outcome::Cancelled(_)))
            .map(|e| (e.text, e.at, e.outcome))
            .collect();
        assert_eq!(
            cancelled,
            [
                (
                    "Cleared to land".to_string(),
                    0.,
                    Outcome::Cancelled(Reason::TowerReply)
                ),
                (
                    "Grade".to_string(),
                    1.,
                    Outcome::Cancelled(Reason::AircraftLost)
                ),
            ]
        );
        assert_eq!(
            c.due(1.)[0].call.text,
            "Wing status",
            "a wingman's notice stays"
        );
    }
    #[test]
    fn tower_calls_carry_their_trigger() {
        let (w, s, f, p) = fixture();
        let mut r = AirfieldRadio::default();
        let mut c = Comms::new(1);
        r.reset(w.runway_view(1000));
        tick(&mut r, &mut c, &p, &w, &s, &f, 0.);
        let delivered = c
            .take_journal()
            .into_iter()
            .find(|e| matches!(e.outcome, Outcome::Delivered { .. }))
            .unwrap();
        assert_eq!(delivered.stems, ["^TAKOFF1"]);
        assert_eq!(
            delivered.origin.cause,
            Cause::Tower(TowerEvent::TakeoffClearance)
        );
        assert_eq!(delivered.origin.audience, Audience::Player);
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "airfield_radio_checkpoint.rs"]
mod checkpoint;
