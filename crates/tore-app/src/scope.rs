//! Scope and exposure presentation inputs. The simulation decides what is
//! observable; this module only reprojects shared observations for drawing and
//! picking. It can never make a hidden target selectable, and it carries
//! bearing and intensity for received noise rather than an emitter's range.
use crate::flight;
use tore_sim::{
    attitude::{Basis, Vector, dot},
    sensors::{self, passive},
};
use tore_world::{
    datalink::TrackSource,
    readout::{CockpitReadout, LinkMark},
};

/// One plotted return. A stale plot is the final observation of a lost
/// contact: visibly old, never selectable.
#[derive(Clone, Debug, PartialEq)]
pub struct Contact {
    pub id: u32,
    pub bearing_rad: f64,
    pub distance_ft: f64,
    /// Observed horizontal motion relative to own heading; absent at rest.
    pub heading_rad: Option<f64>,
    pub track_eligible: bool,
    pub destroyed: bool,
    pub selected: bool,
    pub acquired: bool,
    pub stale: bool,
    /// Past observations reprojected through the current scope transform,
    /// oldest first. Never connected across a missing sample.
    pub trail: Vec<(f64, f64)>,
}

/// Received interference in one direction. Passive noise supplies no range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strobe {
    pub bearing_rad: f64,
    pub density: f64,
    pub half_width_rad: f64,
    pub sidelobe: f64,
}

/// Whether a blinking link cue is lit: on for half of each second, a cycle of
/// 120 simulation ticks (the warning receiver's blink).
pub fn link_blink_on(tick: u64) -> bool {
    tick % 120 < 60
}

/// What the flight data link adds to one target on the scope: the flightmates
/// that hold a lock on it and were assigned it, and whether the lead assigned
/// it to the player. Numbers are the pilots' own, from one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkMarks {
    pub id: u32,
    pub locked_by: Vec<u8>,
    pub assigned_to: Vec<u8>,
    /// The lead assigned it to the player: whether the player has locked it
    /// since (the diamond then stops blinking).
    pub assigned: Option<bool>,
}

/// A hostile aircraft only another member of the side holds: the player's own
/// radar has no contact on it. Drawn as a hollow square; never selectable
/// (designating link tracks is a follow-up, docs/DATALINK.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RemoteTrack {
    pub id: u32,
    pub bearing_rad: f64,
    pub distance_ft: f64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Scope {
    /// Simulation tick, used only for the deterministic noise texture phase.
    pub tick: u64,
    pub channel: &'static str,
    /// The passive infrared channel is selected, so the radar is not emitting.
    pub infrared: bool,
    pub mode: Option<&'static str>,
    pub range_nmi: f64,
    /// The selected channel is installed, powered and undamaged.
    pub operating: bool,
    pub unavailable: Option<&'static str>,
    pub history: bool,
    pub contacts: Vec<Contact>,
    pub strobes: Vec<Strobe>,
    pub selected: Option<u32>,
    /// Weapon support for the selected target, or None when nothing is selected.
    pub status: Option<&'static str>,
    /// The flight data link's marks, by target. Empty for an aircraft with no
    /// radar.
    pub marks: Vec<LinkMarks>,
    /// Link tracks the player's own radar does not hold, nearest first.
    pub remote: Vec<RemoteTrack>,
}

impl Scope {
    /// The link's marks on target `id`.
    pub fn marks_on(&self, id: u32) -> Option<&LinkMarks> {
        self.marks.iter().find(|marks| marks.id == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    pub bearing_rad: f64,
    pub distance_nmi: Option<f64>,
    pub symbol: passive::Symbol,
}

/// Passive exposure instrument inputs. The contour is a reference-radar
/// estimate of directional vulnerability, not a detection boundary.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Rcs {
    pub contour: Vec<(f64, f64)>,
    pub emitters: Vec<Emitter>,
    pub scale_nmi: f64,
    pub signature: f64,
}

/// Player-facing warning-receiver snapshot. The simulation owns detection and
/// threat classification; this value only carries permitted presentation data.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Rwr {
    /// Fixed simulation tick, used for the one-second warning blink cycle.
    pub tick: u64,
    pub operating: bool,
    pub emitters: Vec<RwrEmitter>,
    pub missiles: Vec<RwrMissile>,
    pub radar_indicator: Indicator,
    pub infrared_indicator: Indicator,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Indicator {
    #[default]
    Off,
    Detected,
    /// Manual-defined seeker tracking state, awaiting its simulation producer.
    #[allow(dead_code)]
    Tracking,
    Incoming,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmitterKind {
    Unknown,
    FriendlyAircraft,
    EnemyAircraft,
    Ground,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmitterState {
    Detected,
    /// Manual-defined illumination state: a surface radar's controller is
    /// tracking or firing at this aircraft (steady and bright).
    Painting,
    Tracking,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RwrEmitter {
    pub id: u32,
    pub bearing_rad: f64,
    pub distance_nmi: Option<f64>,
    pub kind: EmitterKind,
    pub state: EmitterState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RwrMissile {
    pub id: u32,
    pub bearing_rad: f64,
    pub distance_nmi: Option<f64>,
    /// True only when this receiver's evidence identifies an incoming threat.
    pub known_targeting_receiver: bool,
    /// Lost-observation grace keeps the last plot steady and visibly dim.
    pub stale: bool,
}

impl Rwr {
    /// Flash an aircraft source only after an incoming supported-radar report
    /// has one independently identified emitter at the same bearing.
    pub fn mark_supported_sources(&mut self, bearings: impl IntoIterator<Item = f64>) {
        for bearing in bearings {
            let mut matches = self.emitters.iter().enumerate().filter(|(_, emitter)| {
                emitter.kind != EmitterKind::Unknown
                    && ((emitter.bearing_rad.to_degrees() - bearing + 180.).rem_euclid(360.) - 180.)
                        .abs()
                        <= 2.
            });
            let selected = matches.next().map(|(index, _)| index);
            if matches.next().is_none()
                && let Some(index) = selected
            {
                self.emitters[index].state = EmitterState::Tracking;
            }
        }
    }
}

/// Agent-proposed noise density, clamped for readability.
fn density(received: f64) -> f64 {
    let value = 0.35 * received / (1. + received);
    if value.is_finite() {
        value.clamp(0., 0.35)
    } else {
        0.
    }
}

fn relative(basis: &Basis, from: Vector, to: Vector) -> (f64, f64) {
    let delta: Vector = std::array::from_fn(|i| to[i] - from[i]);
    (
        dot(delta, basis.right).atan2(dot(delta, basis.forward)),
        dot(delta, delta).sqrt(),
    )
}

pub fn scope(readout: &CockpitReadout, s: &flight::State, controls: sensors::Controls) -> Scope {
    let sensors = &readout.sensors;
    let basis = Basis::new(s.yaw, s.pitch, s.bank);
    // Labels and the plotted scale follow the player's own controls, so they
    // never lag a step behind the switch that was just pressed. Contacts and
    // weapon support stay with the simulation that produced them.
    let channel = controls.channel;
    let selected = sensors.selected;
    let acquired = sensors.acquired;
    let plot = |c: &sensors::Contact, stale: bool, trail: Vec<(f64, f64)>| Contact {
        id: c.id,
        bearing_rad: c.bearing_rad,
        distance_ft: c.distance_ft,
        heading_rad: (c.velocity[0].hypot(c.velocity[2]) > 0.)
            .then(|| c.velocity[0].atan2(c.velocity[2]) - s.yaw),
        track_eligible: c.track_eligible,
        destroyed: c.destroyed,
        selected: !stale && selected == Some(c.id),
        acquired: !stale && acquired == Some(c.id),
        stale,
        trail,
    };
    let mut contacts: Vec<Contact> = sensors
        .contacts
        .iter()
        .map(|c| {
            let trail = sensors
                .trail(c.id)
                .iter()
                .map(|position| {
                    let (bearing, distance) = relative(&basis, s.position, *position);
                    (bearing, distance)
                })
                .collect();
            plot(c, false, trail)
        })
        .collect();
    contacts.extend(
        sensors
            .plots
            .iter()
            .filter(|p| p.channel == channel)
            .map(|p| Contact {
                id: p.id,
                bearing_rad: p.bearing_rad,
                distance_ft: p.distance_ft,
                heading_rad: None,
                track_eligible: false,
                destroyed: false,
                selected: false,
                acquired: false,
                stale: true,
                trail: vec![],
            }),
    );
    let (marks, remote) = link_cues(readout, s, &basis);
    Scope {
        tick: sensors.tick,
        channel: channel.label(),
        infrared: channel == sensors::Channel::Infrared,
        mode: sensors.mode_for(channel, &controls).map(|m| m.label()),
        range_nmi: controls.range_nmi(),
        operating: sensors.operating(channel),
        unavailable: if sensors.available(channel) {
            None
        } else {
            Some("NOT INSTALLED")
        },
        history: controls.history,
        contacts,
        strobes: sensors
            .strobes
            .iter()
            .map(|strobe| Strobe {
                bearing_rad: strobe.bearing_rad,
                density: density(strobe.received),
                half_width_rad: strobe.half_width_rad,
                sidelobe: strobe.sidelobe_floor,
            })
            .collect(),
        selected,
        status: sensors.selected_support.map(|support| support.label()),
        marks,
        remote,
    }
}

/// The link's marks and remote tracks for the scope: nothing when the aircraft
/// has no radar, so no scope (John, 2026-10-05). A target gets marks when a flightmate locked or was assigned
/// it, or the lead assigned it to the player. A track is remote when another
/// member holds it and the player's own radar contacts do not.
fn link_cues(
    readout: &CockpitReadout,
    s: &flight::State,
    basis: &Basis,
) -> (Vec<LinkMarks>, Vec<RemoteTrack>) {
    let link = &readout.link;
    if !link.on_scope() {
        return (Vec::new(), Vec::new());
    }
    let sensors = &readout.sensors;
    let mut marks: Vec<LinkMarks> = link
        .marks
        .iter()
        .filter(|mark| mark.lockers != 0 || mark.assigned_to != 0)
        .map(|mark| LinkMarks {
            id: mark.target,
            locked_by: LinkMark::numbers(mark.lockers),
            assigned_to: LinkMark::numbers(mark.assigned_to),
            assigned: None,
        })
        .collect();
    if let Some(given) = link.assignment() {
        // Locking the target is the acknowledgement; the sensors know it a
        // tick before the picture does.
        let acknowledged = given.acknowledged || sensors.acquired == Some(given.target);
        match marks.iter_mut().find(|m| m.id == given.target) {
            Some(existing) => existing.assigned = Some(acknowledged),
            None => marks.push(LinkMarks {
                id: given.target,
                assigned: Some(acknowledged),
                ..LinkMarks::default()
            }),
        }
    }
    let remote = link
        .tracks
        .iter()
        .filter(|track| track.source != TrackSource::Own)
        .filter(|track| sensors.contact(track.target).is_none())
        .map(|track| {
            let (bearing_rad, distance_ft) = relative(basis, s.position, track.position);
            RemoteTrack {
                id: track.target,
                bearing_rad,
                distance_ft,
            }
        })
        .collect();
    (marks, remote)
}

pub fn rcs(
    config: &tore_sim::combat::live::Configuration,
    readout: &CockpitReadout,
    s: &flight::State,
    scale_nmi: f64,
) -> Rcs {
    let signature = config.sensors.signature;
    let basis = Basis::new(s.yaw, s.pitch, s.bank);
    let configuration = sensors::Configuration {
        gear: s.gear,
        flaps: s.flaps,
        bay: s.bay,
    };
    Rcs {
        contour: signature.exposure_contour(&basis, configuration, 5.),
        emitters: readout
            .rwr
            .emitters
            .iter()
            .filter(|_| !readout.damage.rwr_failed)
            .map(|e| Emitter {
                bearing_rad: e.bearing_rad,
                distance_nmi: e.distance_nmi,
                symbol: e.symbol,
            })
            .collect(),
        scale_nmi,
        // Nose-on exposure against the reference observer, using the same
        // level heading direction as the contour's first sample.
        signature: {
            let heading = basis.angles()[0];
            signature.effective_radar(&basis, [heading.sin(), 0., heading.cos()], configuration)
        },
    }
}

/// Pick the nearest drawn contact to an instrument raster point. Drawing and
/// picking share one projection, so a click selects what the player sees.
/// Stale plots and history dots are never selectable.
pub fn pick(
    contacts: &[Contact],
    project: impl Fn(&Contact) -> Option<(f64, f64)>,
    point: (f64, f64),
    tolerance: f64,
) -> Option<u32> {
    let mut best: Option<(f64, u32)> = None;
    for contact in contacts.iter().filter(|c| !c.stale) {
        let Some((x, y)) = project(contact) else {
            continue;
        };
        let distance = (x - point.0).hypot(y - point.1);
        if distance <= tolerance
            // Equal-distance ties resolve by stable target identity.
            && best.is_none_or(|(d, id)| distance < d || (distance == d && contact.id < id))
        {
            best = Some((distance, contact.id));
        }
    }
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn contact(id: u32, bearing: f64, stale: bool) -> Contact {
        Contact {
            id,
            bearing_rad: bearing,
            distance_ft: 10_000.,
            heading_rad: None,
            track_eligible: true,
            destroyed: false,
            selected: false,
            acquired: false,
            stale,
            trail: vec![],
        }
    }
    #[test]
    fn supported_radar_firing_marks_only_a_unique_identified_emitter() {
        let emitter = RwrEmitter {
            id: 1,
            bearing_rad: 0.,
            distance_nmi: Some(3.),
            kind: EmitterKind::EnemyAircraft,
            state: EmitterState::Detected,
        };
        let mut rwr = Rwr {
            emitters: vec![emitter],
            ..Rwr::default()
        };
        rwr.mark_supported_sources([0.]);
        assert_eq!(rwr.emitters[0].state, EmitterState::Tracking);
        rwr.emitters = vec![emitter, RwrEmitter { id: 2, ..emitter }];
        rwr.mark_supported_sources([0.]);
        assert!(
            rwr.emitters
                .iter()
                .all(|e| e.state == EmitterState::Detected)
        );
        rwr.emitters = vec![RwrEmitter {
            kind: EmitterKind::Unknown,
            ..emitter
        }];
        rwr.mark_supported_sources([0.]);
        assert_eq!(rwr.emitters[0].state, EmitterState::Detected);
    }

    #[test]
    fn picking_uses_the_drawn_projection_and_stable_tie_breaking() {
        let contacts = [
            contact(2, 0., false),
            contact(1, 0., false),
            contact(3, 1., false),
        ];
        let project = |c: &Contact| Some((c.bearing_rad * 50., 0.));
        assert_eq!(pick(&contacts, project, (0., 0.), 7.), Some(1));
        assert_eq!(pick(&contacts, project, (50., 0.), 7.), Some(3));
        assert_eq!(pick(&contacts, project, (25., 0.), 7.), None);
        assert_eq!(pick(&[], project, (0., 0.), 7.), None);
    }
    #[test]
    fn stale_plots_and_hidden_contacts_are_never_picked() {
        let contacts = [contact(1, 0., true), contact(2, 1., false)];
        let project = |c: &Contact| (!c.stale).then_some((c.bearing_rad * 50., 0.));
        assert_eq!(pick(&contacts, project, (0., 0.), 7.), None);
        assert_eq!(pick(&contacts, project, (50., 0.), 7.), Some(2));
    }
    #[test]
    fn noise_density_is_bounded_and_rises_with_received_interference() {
        assert_eq!(density(0.), 0.);
        assert!((density(1.) - 0.175).abs() < 1e-9);
        assert!(density(1000.) <= 0.35 && density(1000.) > 0.34);
        assert_eq!(density(f64::NAN), 0.);
    }

    /// A fixture readout and the flight state it was built for, with `link`
    /// in place of the (empty) link.
    fn linked(link: tore_world::readout::LinkReadout) -> (CockpitReadout, flight::State) {
        let (combat, flight) = tore_world::combat::fixtures::loaded([1, 1]);
        let mut readout = combat
            .cockpit_readout(
                combat.own_id(),
                crate::combat::launcher(&flight),
                None,
                None,
            )
            .expect("the fixture's plane has an ownship");
        readout.link = link;
        (readout, flight)
    }
    fn track(target: u32, source: TrackSource, ahead_ft: f64) -> tore_world::readout::LinkTrack {
        tore_world::readout::LinkTrack {
            target,
            position: [0., 0., ahead_ft],
            velocity: [0.; 3],
            source,
        }
    }
    fn own_contact(id: u32) -> sensors::Contact {
        sensors::Contact {
            id,
            channel: sensors::Channel::Radar,
            bearing_rad: 0.,
            elevation_rad: 0.,
            distance_ft: 10_000.,
            position: [0.; 3],
            velocity: [0.; 3],
            track_eligible: true,
            destroyed: false,
        }
    }
    fn mark(target: u32, lockers: u16, assigned_to: u16) -> LinkMark {
        LinkMark {
            target,
            lockers,
            net_lock: None,
            assigned_to,
        }
    }

    #[test]
    fn marks_number_the_flightmates_and_the_assignment_follows_the_lock() {
        let link = tore_world::readout::LinkReadout {
            radar: true,
            assigned: Some(tore_world::readout::LinkAssigned {
                target: 20,
                by: 0,
                acknowledged: false,
            }),
            marks: vec![mark(10, 0b110, 0), mark(11, 0, 0b10), mark(12, 0, 0)],
            ..Default::default()
        };
        let (mut readout, flight) = linked(link);
        let scope = scope(&readout, &flight, flight.sensors);
        // A mark with nobody on it is no mark; the assigned target gets one.
        let ids: Vec<u32> = scope.marks.iter().map(|m| m.id).collect();
        assert_eq!(ids, [10, 11, 20]);
        assert_eq!(scope.marks_on(10).unwrap().locked_by, [2, 3]);
        assert_eq!(scope.marks_on(11).unwrap().assigned_to, [2]);
        assert_eq!(scope.marks_on(20).unwrap().assigned, Some(false));
        assert_eq!(scope.marks_on(10).unwrap().assigned, None);
        // Locking the assigned target acknowledges it at once, before the
        // picture says so.
        readout.sensors.acquired = Some(20);
        assert_eq!(
            super::scope(&readout, &flight, flight.sensors)
                .marks_on(20)
                .unwrap()
                .assigned,
            Some(true)
        );
        readout.sensors.acquired = None;
        readout.link.assigned.as_mut().unwrap().acknowledged = true;
        assert_eq!(
            super::scope(&readout, &flight, flight.sensors)
                .marks_on(20)
                .unwrap()
                .assigned,
            Some(true)
        );
        // The assignment of a target a flightmate has locked joins its marks.
        readout.link.assigned.as_mut().unwrap().target = 10;
        let joined = super::scope(&readout, &flight, flight.sensors);
        let ten = joined.marks_on(10).unwrap();
        assert_eq!(
            (ten.locked_by.clone(), ten.assigned),
            (vec![2, 3], Some(true))
        );
    }

    #[test]
    fn a_track_only_another_member_holds_is_a_remote_square() {
        let link = tore_world::readout::LinkReadout {
            radar: true,
            tracks: vec![
                track(30, TrackSource::Own, 5_000.),
                track(31, TrackSource::Flight, 6_000.),
                track(32, TrackSource::Network, 7_000.),
                track(33, TrackSource::Flight, 8_000.),
            ],
            ..Default::default()
        };
        let (mut readout, flight) = linked(link);
        // The player's own radar holds 33 too.
        readout.sensors.contacts = vec![own_contact(33)];
        let scope = scope(&readout, &flight, flight.sensors);
        let remote: Vec<u32> = scope.remote.iter().map(|r| r.id).collect();
        assert_eq!(
            remote,
            [31, 32],
            "own tracks and held contacts are not remote"
        );
        assert!(scope.remote.iter().all(|r| r.distance_ft > 0.));
        // A contact the radar holds stays a contact, and none is selectable
        // through the remote list.
        assert_eq!(
            scope.contacts.iter().map(|c| c.id).collect::<Vec<_>>(),
            [33]
        );
    }

    #[test]
    fn an_aircraft_with_no_radar_has_no_link_mark_on_its_scope() {
        let link = tore_world::readout::LinkReadout {
            radar: false,
            assigned: Some(tore_world::readout::LinkAssigned {
                target: 20,
                by: 0,
                acknowledged: false,
            }),
            tracks: vec![track(31, TrackSource::Flight, 6_000.)],
            marks: vec![mark(10, 0b10, 0b100)],
            ..Default::default()
        };
        let (readout, flight) = linked(link);
        let scope = scope(&readout, &flight, flight.sensors);
        assert!(scope.marks.is_empty() && scope.remote.is_empty());
    }
}
