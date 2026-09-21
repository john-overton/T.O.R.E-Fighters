//! Acceptance cases for the shared sensor state machine, taken from the
//! authored rules in docs/radar.md: automatic RWS/TWS, persistent selection,
//! weapon-track acquisition, observation loss, stale plots, bounded history,
//! infrared air-to-air, destroyed contacts and received RF interference.
//!
//! Every fixture is a synthetic struct literal. No retail media is read, no
//! wall-clock time is sampled and no random source is consulted, so each case
//! is reproducible from the simulation tick alone.
use super::profile::{
    FEET_PER_NAUTICAL_MILE, Generation, InfraredProfile, JammerProfile, Preset, RANGE_LADDER_NMI,
    RadarProfile, SensorProfiles, Volume,
};
use super::signature::{Configuration, SignatureProfile};
use super::track::{
    ACQUISITION_STEPS, Channel, Contact, Environment, Event, HISTORY_AGE_STEPS,
    HISTORY_INTERVAL_STEPS, HISTORY_SAMPLES, Mode, Observable, Observer, STALE_STEPS, Sample,
    Sensors, Support,
};
use crate::attitude::{Basis, Vector};
use tore_formats::aircraft::AircraftId;

/// One nautical mile in simulation feet.
const NMI: f64 = FEET_PER_NAUTICAL_MILE;
/// Every fixture flies ownship here so a target can also be placed below it.
const OWNSHIP_ALTITUDE_FT: f64 = 20_000.;
/// The authored timings as step counts, in the units the helpers below take.
const ACQUIRE: u64 = ACQUISITION_STEPS as u64;
const STALE: u64 = STALE_STEPS as u64;

static GROUND: fn(f64, f64) -> f64 = |_, _| 0.;
static CLEAR: fn(Vector, Vector) -> bool = |_, _| false;
static MASKED: fn(Vector, Vector) -> bool = |_, _| true;

fn clear_air() -> Environment<'static> {
    Environment {
        ground: &GROUND,
        obscured: &CLEAR,
    }
}

fn masked_air() -> Environment<'static> {
    Environment {
        ground: &GROUND,
        obscured: &MASKED,
    }
}

fn volume(maximum_nmi: f64, azimuth_deg: f64, elevation_deg: f64) -> Volume {
    Volume {
        azimuth_rad: azimuth_deg.to_radians(),
        elevation_rad: elevation_deg.to_radians(),
        minimum_ft: 0.,
        maximum_ft: maximum_nmi * NMI,
        minimum_relative_ft: f64::NEG_INFINITY,
        maximum_relative_ft: f64::INFINITY,
    }
}

fn tuned_radar(search_nmi: f64, track_nmi: f64, preset: Preset, look_down: f64) -> RadarProfile {
    RadarProfile {
        record: "TESTR.SEE".into(),
        search: volume(search_nmi, 60., 60.),
        track: volume(track_nmi, 45., 45.),
        look_down,
        preset,
        notch: preset.notch(),
        resistance: preset.resistance(),
        band: 0,
        source_flags: [0; 2],
        source_doppler: [0; 3],
    }
}

/// The common test set: an advanced receiver with a mid look-down coefficient.
fn radar(search_nmi: f64, track_nmi: f64) -> RadarProfile {
    tuned_radar(search_nmi, track_nmi, Preset::Advanced, 50.)
}

/// Infrared and visual share the passive profile shape.
fn passive(record: &str, search_nmi: f64, track_nmi: f64) -> InfraredProfile {
    InfraredProfile {
        record: record.into(),
        search: volume(search_nmi, 60., 60.),
        track: volume(track_nmi, 60., 60.),
    }
}

/// A powered self-protection emitter at the reference strength.
fn jammer(generation: Generation) -> JammerProfile {
    JammerProfile {
        record: "TEST.ECM".into(),
        generation,
        strength: 0.30,
        band: 0,
        radio_frequency: true,
    }
}

fn profiles(radar: Option<RadarProfile>, infrared: Option<InfraredProfile>) -> SensorProfiles {
    SensorProfiles {
        aircraft: AircraftId::F18,
        radar,
        infrared,
        visual: None,
        jammer: None,
        signature: SignatureProfile::default(),
    }
}

fn with_visual(mut profiles: SensorProfiles, search_nmi: f64) -> SensorProfiles {
    profiles.visual = Some(passive("TESTV.SEE", search_nmi, search_nmi));
    profiles
}

/// The 90/50 nmi reference installation used by most cases.
fn hornet() -> Sensors {
    Sensors::new(profiles(Some(radar(90., 50.)), None))
}

fn observer() -> Observer {
    Observer {
        position: [0., OWNSHIP_ALTITUDE_FT, 0.],
        basis: Basis::new(0., 0., 0.),
        radar_powered: true,
        radar_failed: false,
        infrared_failed: false,
        visual_failed: false,
    }
}

fn object(id: u32, position: Vector) -> Observable {
    Observable {
        id,
        position,
        velocity: [0.; 3],
        basis: Basis::new(0., 0., 0.),
        configuration: Configuration::CLEAN,
        signature: SignatureProfile::default(),
        jammer: None,
        jammer_active: false,
        radar_emitting: false,
        airborne: true,
        destroyed: false,
    }
}

/// Level with ownship, dead ahead, so azimuth and elevation are both zero.
fn ahead(id: u32, distance_nmi: f64) -> Observable {
    object(id, [0., OWNSHIP_ALTITUDE_FT, distance_nmi * NMI])
}

/// Level with ownship at the given body-relative bearing, right positive.
fn bearing(id: u32, bearing_deg: f64, distance_nmi: f64) -> Observable {
    let angle = bearing_deg.to_radians();
    object(
        id,
        [
            distance_nmi * NMI * angle.sin(),
            OWNSHIP_ALTITUDE_FT,
            distance_nmi * NMI * angle.cos(),
        ],
    )
}

/// On the deck ahead of ownship, so clutter exposure is the full 1.0.
fn low(id: u32, ground_distance_nmi: f64) -> Observable {
    object(id, [0., 0., ground_distance_nmi * NMI])
}

fn run(sensors: &mut Sensors, observer: &Observer, targets: &[Observable], steps: u64) {
    let air = clear_air();
    for _ in 0..steps {
        sensors.step(observer, targets, &air);
    }
}

fn ticks(samples: &[Sample]) -> Vec<u64> {
    samples.iter().map(|s| s.tick).collect()
}

fn selection_cleared(events: &[Event]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::SelectionCleared(_)))
}

fn observed(sensors: &Sensors) -> Vec<(u32, f64, bool)> {
    sensors
        .contacts()
        .iter()
        .map(|c| (c.id, c.distance_ft, c.track_eligible))
        .collect()
}

#[test]
fn display_range_picks_the_mode_and_never_changes_the_physical_coverage() {
    let air = clear_air();
    let o = observer();
    let targets = [ahead(1, 30.)];

    let mut hornet = hornet();
    let installed = hornet.profiles.radar.clone();
    let mut modes = Vec::new();
    let mut sets: Vec<Vec<Contact>> = Vec::new();
    for index in 0..RANGE_LADDER_NMI.len() {
        hornet.controls.range_index = index;
        hornet.step(&o, &targets, &air);
        modes.push(hornet.mode().expect("an installed radar reports a mode"));
        sets.push(hornet.contacts().to_vec());
    }
    assert_eq!(
        modes,
        [
            Mode::Tws,
            Mode::Tws,
            Mode::Tws,
            Mode::Tws,
            Mode::Rws,
            Mode::Rws
        ]
    );
    // The 5, 10, 25 and 50 settings track; 100 and 150 only search.
    assert_eq!(hornet.profiles.radar, installed);
    for set in &sets {
        assert_eq!(set, &sets[0]);
        assert_eq!(set.len(), 1);
        assert!(set[0].track_eligible);
    }

    // A 50/25 nmi set changes between the 25 and 50 settings.
    let mut skyhawk = Sensors::new(profiles(Some(radar(50., 25.)), None));
    skyhawk.controls.range_index = 2;
    skyhawk.step(&o, &targets, &air);
    assert_eq!(skyhawk.mode(), Some(Mode::Tws));
    skyhawk.controls.range_index = 3;
    skyhawk.step(&o, &targets, &air);
    assert_eq!(skyhawk.mode(), Some(Mode::Rws));

    // A 62/43 nmi set is search only at the 50 setting.
    let mut fulcrum = Sensors::new(profiles(Some(radar(62., 43.)), None));
    fulcrum.controls.range_index = 2;
    fulcrum.step(&o, &targets, &air);
    assert_eq!(fulcrum.mode(), Some(Mode::Tws));
    fulcrum.controls.range_index = 3;
    fulcrum.step(&o, &targets, &air);
    assert_eq!(fulcrum.mode(), Some(Mode::Rws));
}

#[test]
fn a_designated_contact_stays_selected_without_any_repeat_input() {
    let air = clear_air();
    let o = observer();
    let targets = [ahead(1, 20.), bearing(2, 14., 25.)];
    let mut s = hornet();
    s.step(&o, &targets, &air);

    assert!(s.designate(1));
    assert_eq!(s.selected(), Some(1));

    run(&mut s, &o, &targets, 300);
    assert_eq!(s.selected(), Some(1));

    // An identity the sensors do not currently observe is rejected outright.
    assert!(!s.designate(77));
    assert_eq!(s.selected(), Some(1));

    assert!(s.designate(2));
    assert_eq!(s.selected(), Some(2));

    s.clear_selection();
    assert_eq!(s.selected(), None);
    assert_eq!(s.acquired(), None);
    run(&mut s, &o, &targets, 120);
    assert_eq!(s.selected(), None);
}

#[test]
fn a_tws_weapon_track_acquires_on_the_sixtieth_step_and_not_the_fifty_ninth() {
    let air = clear_air();
    let o = observer();
    let targets = [ahead(1, 30.), bearing(2, 14., 20.)];

    let mut s = hornet();
    s.step(&o, &targets, &air);
    assert_eq!(s.mode(), Some(Mode::Tws));
    assert!(s.designate(1));
    run(&mut s, &o, &targets, ACQUIRE - 1);
    assert_eq!(s.acquired(), None);
    assert_eq!(s.support(1), Support::Acquiring);
    let events = s.step(&o, &targets, &air);
    assert_eq!(s.acquired(), Some(1));
    assert_eq!(s.support(1), Support::Tracked);
    assert!(events.contains(&Event::TrackAcquired(1)));

    // Re-designating the same current target does not restart the timer.
    let mut again = hornet();
    again.step(&o, &targets, &air);
    assert!(again.designate(1));
    run(&mut again, &o, &targets, 30);
    assert!(again.designate(1));
    run(&mut again, &o, &targets, ACQUIRE - 31);
    assert_eq!(again.acquired(), None);
    again.step(&o, &targets, &air);
    assert_eq!(again.acquired(), Some(1));

    // Designating another target releases the old track before the next step.
    assert!(s.designate(2));
    assert_eq!(s.selected(), Some(2));
    assert_eq!(s.acquired(), None);
    run(&mut s, &o, &targets, ACQUIRE - 1);
    assert_eq!(s.acquired(), None);
    s.step(&o, &targets, &air);
    assert_eq!(s.acquired(), Some(2));
}

#[test]
fn range_while_search_selects_a_contact_but_never_locks_it() {
    let air = clear_air();
    let o = observer();
    let targets = [ahead(1, 30.)];

    let mut search_only = hornet();
    search_only.controls.range_index = 4;
    search_only.step(&o, &targets, &air);
    assert_eq!(search_only.mode(), Some(Mode::Rws));
    assert!(search_only.designate(1));
    run(&mut search_only, &o, &targets, 600);
    assert_eq!(search_only.selected(), Some(1));
    assert_eq!(search_only.acquired(), None);
    assert_eq!(search_only.support(1), Support::SearchOnly);
    assert!(!search_only.supports(1));

    // Entering RWS releases an existing track and keeps the selection.
    let mut s = hornet();
    s.controls.range_index = 3;
    s.step(&o, &targets, &air);
    assert!(s.designate(1));
    run(&mut s, &o, &targets, ACQUIRE);
    assert_eq!(s.acquired(), Some(1));
    s.controls.range_index = 4;
    let events = s.step(&o, &targets, &air);
    assert_eq!(s.mode(), Some(Mode::Rws));
    assert_eq!(s.acquired(), None);
    assert_eq!(s.selected(), Some(1));
    assert!(events.contains(&Event::TrackReleased(1)));
    assert!(!selection_cleared(&events));

    // Returning to TWS starts a completely fresh acquisition.
    s.controls.range_index = 3;
    run(&mut s, &o, &targets, ACQUIRE - 1);
    assert_eq!(s.acquired(), None);
    s.step(&o, &targets, &air);
    assert_eq!(s.acquired(), Some(1));
}

#[test]
fn only_one_contact_at_a_time_holds_a_fire_control_track() {
    let air = clear_air();
    let o = observer();
    let targets = [ahead(1, 20.), bearing(2, 14., 25.), bearing(3, -20., 30.)];

    let mut s = hornet();
    s.step(&o, &targets, &air);
    assert_eq!(s.contacts().len(), 3);
    assert!(s.designate(1));
    run(&mut s, &o, &targets, ACQUIRE);
    assert_eq!(s.acquired(), Some(1));
    assert_eq!(s.support(2), Support::NotSelected);
    assert_eq!(s.support(3), Support::NotSelected);

    assert!(s.designate(2));
    for _ in 0..(ACQUIRE * 3) {
        s.step(&o, &targets, &air);
        // The acquired track is an option, and it is only ever the selection.
        assert!(s.acquired().is_none() || s.acquired() == s.selected());
        assert_ne!(s.acquired(), Some(1));
        assert_ne!(s.acquired(), Some(3));
    }
    assert_eq!(s.acquired(), Some(2));

    // Removing the selected object never promotes another contact.
    let remaining = [targets[0].clone(), targets[2].clone()];
    let events = s.step(&o, &remaining, &air);
    assert_eq!(s.selected(), None);
    assert_eq!(s.acquired(), None);
    assert_eq!(s.contacts().len(), 2);
    assert!(events.contains(&Event::SelectionCleared(2)));
    assert!(events.contains(&Event::TrackReleased(2)));
    run(&mut s, &o, &remaining, 240);
    assert_eq!(s.selected(), None);
    assert_eq!(s.acquired(), None);
}

#[test]
fn losing_the_observation_clears_selection_and_weapon_support_on_the_same_step() {
    let air = clear_air();
    let o = observer();
    let near = [ahead(1, 30.)];

    // A tracked contact, ready to be lost four different ways.
    let tracking = || {
        let mut s = hornet();
        s.step(&observer(), &near, &clear_air());
        assert!(s.designate(1));
        run(&mut s, &observer(), &near, ACQUIRE);
        assert_eq!(s.acquired(), Some(1));
        s
    };
    let lost = |s: &Sensors, events: &[Event]| {
        assert_eq!(s.selected(), None);
        assert_eq!(s.acquired(), None);
        assert!(s.contacts().is_empty());
        assert!(events.contains(&Event::SelectionCleared(1)));
        assert!(events.contains(&Event::TrackReleased(1)));
        assert!(events.contains(&Event::ContactLost(1)));
    };

    // Outside the search volume's distance.
    let mut s = tracking();
    let far = [ahead(1, 100.)];
    let events = s.step(&o, &far, &air);
    lost(&s, &events);
    assert_eq!(s.support(1), Support::NotSelected);

    // Outside the search volume's azimuth.
    let mut s = tracking();
    let wide = [bearing(1, 80., 30.)];
    let events = s.step(&o, &wide, &air);
    lost(&s, &events);

    // Masked by terrain.
    let mut s = tracking();
    let events = s.step(&o, &near, &masked_air());
    lost(&s, &events);
    assert_eq!(s.support(1), Support::NotSelected);

    // Radar powered down.
    let mut s = tracking();
    let mut off = observer();
    off.radar_powered = false;
    let events = s.step(&off, &near, &air);
    lost(&s, &events);
    assert_eq!(s.support(1), Support::RadarOff);

    // Radar failed.
    let mut s = tracking();
    let mut failed = observer();
    failed.radar_failed = true;
    let events = s.step(&failed, &near, &air);
    lost(&s, &events);

    // Reappearance is an unselected contact, with no sticky identity.
    let mut s = tracking();
    s.step(&o, &far, &air);
    s.step(&o, &near, &air);
    assert_eq!(s.contacts().len(), 1);
    assert_eq!(s.selected(), None);
    assert_eq!(s.acquired(), None);
    assert_eq!(s.support(1), Support::NotSelected);

    // A display range change alone loses nothing that is still observed.
    let mut s = tracking();
    s.controls.range_index = 5;
    s.step(&o, &near, &air);
    assert_eq!(s.selected(), Some(1));
    assert_eq!(s.contacts().len(), 1);
    s.controls.range_index = 0;
    s.step(&o, &near, &air);
    assert_eq!(s.selected(), Some(1));
    assert_eq!(s.contacts().len(), 1);
}

#[test]
fn a_failed_radar_reports_its_own_inhibit_reason_rather_than_radar_off() {
    // docs/radar.md, "Mouse designation and missiles": the shared support status
    // owes the player an actionable inhibit reason, and lists "radar off" and
    // "damaged" as separate ones. Support::RadarFailed exists for the second.
    let air = clear_air();
    let targets = [ahead(1, 30.)];
    let mut s = hornet();
    s.step(&observer(), &targets, &air);
    assert!(s.designate(1));

    let mut failed = observer();
    failed.radar_failed = true;
    s.step(&failed, &targets, &air);
    assert_eq!(
        s.support(1),
        Support::RadarFailed,
        "a failed radar must be distinguishable from a radar the player switched off"
    );
}

#[test]
fn a_lost_contact_leaves_one_stale_plot_that_expires_after_one_second() {
    let air = clear_air();
    let o = observer();
    let near = [ahead(1, 30.)];
    let far = [ahead(1, 100.)];

    let mut s = hornet();
    s.step(&o, &near, &air);
    assert!(s.designate(1));
    run(&mut s, &o, &near, ACQUIRE);
    assert!(s.plots().is_empty());

    s.step(&o, &far, &air);
    assert_eq!(s.plots().len(), 1);
    let plot = s.plots()[0];
    assert_eq!(plot.id, 1);
    assert_eq!(plot.channel, Channel::Radar);
    assert_eq!(plot.position, near[0].position);
    assert!((plot.distance_ft - 30. * NMI).abs() < 1e-6);
    assert!(plot.bearing_rad.abs() < 1e-12);
    assert_eq!(plot.age, 1);

    // It is never a current contact and can never be designated.
    assert!(s.contacts().is_empty());
    assert!(s.contact(1).is_none());
    assert!(!s.designate(1));
    assert_eq!(s.selected(), None);

    run(&mut s, &o, &far, STALE - 1);
    assert_eq!(s.plots().len(), 1);
    assert_eq!(s.plots()[0].age, STALE_STEPS);
    assert_eq!(s.plots()[0].position, near[0].position);
    s.step(&o, &far, &air);
    assert!(s.plots().is_empty());

    // A contact that comes back replaces its own stale plot.
    let mut s = hornet();
    s.step(&o, &near, &air);
    s.step(&o, &far, &air);
    run(&mut s, &o, &far, 30);
    assert_eq!(s.plots().len(), 1);
    s.step(&o, &near, &air);
    assert!(s.plots().is_empty());
    assert_eq!(s.contacts().len(), 1);
}

#[test]
fn history_samples_every_half_second_and_expires_without_bridging_gaps() {
    let air = clear_air();
    let o = observer();
    let targets = [ahead(1, 30.)];
    let empty: [Observable; 0] = [];

    // Samples are collected with the display preference switched off.
    let mut s = hornet();
    assert!(!s.controls.history);
    s.step(&o, &targets, &air);
    assert_eq!(ticks(s.trail(1)), [0]);
    run(&mut s, &o, &targets, HISTORY_INTERVAL_STEPS - 1);
    assert_eq!(ticks(s.trail(1)), [0]);
    s.step(&o, &targets, &air);
    assert_eq!(ticks(s.trail(1)), [0, 60]);
    assert_eq!(s.trail(1)[1].position, targets[0].position);

    // At most eight samples per target per channel, oldest first out.
    run(&mut s, &o, &targets, 360);
    assert_eq!(s.tick(), 421);
    assert_eq!(ticks(s.trail(1)), [0, 60, 120, 180, 240, 300, 360, 420]);
    assert_eq!(s.trail(1).len(), HISTORY_SAMPLES);
    run(&mut s, &o, &targets, HISTORY_INTERVAL_STEPS);
    assert_eq!(ticks(s.trail(1)), [60, 120, 180, 240, 300, 360, 420, 480]);

    // With the target gone nothing is added, and samples age out at 480 ticks.
    run(&mut s, &o, &empty, 60);
    assert_eq!(s.tick(), 541);
    assert_eq!(s.trail(1)[0].tick, 60);
    assert_eq!(s.tick() - 1 - s.trail(1)[0].tick, HISTORY_AGE_STEPS);
    s.step(&o, &empty, &air);
    assert_eq!(s.trail(1)[0].tick, 120);

    // The inactive channel collects nothing and trails are never fused.
    let close = [ahead(1, 5.)];
    let mut both = Sensors::new(profiles(
        Some(radar(90., 50.)),
        Some(passive("TESTI.SEE", 9., 10.)),
    ));
    run(&mut both, &o, &close, HISTORY_INTERVAL_STEPS + 1);
    assert_eq!(ticks(both.trail(1)), [0, 60]);
    both.controls.channel = Channel::Infrared;
    run(&mut both, &o, &close, HISTORY_INTERVAL_STEPS + 1);
    assert_eq!(both.tick(), 122);
    assert_eq!(ticks(both.trail(1)), [120]);
    both.controls.channel = Channel::Radar;
    both.step(&o, &close, &air);
    assert_eq!(ticks(both.trail(1)), [0, 60]);

    // No samples after loss, and no interpolation across the gap.
    let mut gapped = hornet();
    run(&mut gapped, &o, &targets, HISTORY_INTERVAL_STEPS + 1);
    assert_eq!(ticks(gapped.trail(1)), [0, 60]);
    run(&mut gapped, &o, &empty, 120);
    assert_eq!(ticks(gapped.trail(1)), [0, 60]);
    run(&mut gapped, &o, &targets, 60);
    assert_eq!(ticks(gapped.trail(1)), [0, 60, 240]);
}

#[test]
fn infrared_is_a_separate_passive_channel_with_its_own_signature_and_envelopes() {
    let air = clear_air();
    let o = observer();

    // Without an installed infrared sensor the request falls back to radar.
    let mut radar_only = hornet();
    radar_only.controls.channel = Channel::Infrared;
    let close = [ahead(1, 8.)];
    radar_only.step(&o, &close, &air);
    assert!(!radar_only.available(Channel::Infrared));
    assert!(radar_only.available(Channel::Radar));
    assert_eq!(radar_only.contacts()[0].channel, Channel::Radar);
    assert_eq!(radar_only.mode(), Some(Mode::Tws));

    // A 9 nmi search and 10 nmi track sensor: new contacts need the search
    // envelope, and the unseen target at 10 nmi is never acquired.
    let unseen = bearing(2, 20., 10.);
    let inside = [ahead(1, 8.), unseen.clone()];
    let mut s = Sensors::new(profiles(
        Some(radar(90., 50.)),
        Some(passive("TESTI.SEE", 9., 10.)),
    ));
    s.controls.channel = Channel::Infrared;
    s.step(&o, &inside, &air);
    assert_eq!(s.mode(), Some(Mode::Infrared));
    assert_eq!(s.contacts().len(), 1);
    assert_eq!(s.contacts()[0].channel, Channel::Infrared);
    assert!(s.contact(2).is_none());
    assert!(!s.designate(2));
    assert!(s.designate(1));
    run(&mut s, &o, &inside, ACQUIRE);
    assert_eq!(s.acquired(), Some(1));
    // An infrared track is not radar illumination.
    assert!(!s.supports(1));

    // The acquired track stays current out to its own 10 nmi envelope.
    let retained = [ahead(1, 10.), unseen.clone()];
    s.step(&o, &retained, &air);
    assert_eq!(s.selected(), Some(1));
    assert_eq!(s.acquired(), Some(1));
    let contact = *s.contact(1).expect("the acquired track stays current");
    assert!((contact.distance_ft - 10. * NMI).abs() < 1e-9);
    assert!(contact.track_eligible);
    assert!(s.contact(2).is_none());

    // Just outside the tracking envelope even the retained track is gone.
    let beyond = [ahead(1, 10.5), unseen];
    let events = s.step(&o, &beyond, &air);
    assert!(s.contact(1).is_none());
    assert_eq!(s.selected(), None);
    assert_eq!(s.acquired(), None);
    assert!(events.contains(&Event::SelectionCleared(1)));

    // The channel scales by the target's infrared signature, halving the
    // 9 nmi search envelope to 4.5 at signature 25.
    let mut cool = ahead(1, 4.5);
    cool.signature.infrared = 25.;
    let mut dim = Sensors::new(profiles(
        Some(radar(90., 50.)),
        Some(passive("TESTI.SEE", 9., 10.)),
    ));
    dim.controls.channel = Channel::Infrared;
    dim.step(&o, &[cool], &air);
    assert!(dim.contact(1).is_some());
    let mut cooler = ahead(1, 4.6);
    cooler.signature.infrared = 25.;
    cooler.signature.radar = 10_000.;
    dim.step(&o, &[cooler], &air);
    assert!(dim.contact(1).is_none());

    // The RF jammer, the notch and look-down never reach this channel.
    let quiet = low(1, 5.);
    let mut loud = low(1, 5.);
    loud.velocity = [800., 0., 0.];
    loud.jammer = Some(jammer(Generation::LateColdWar));
    loud.jammer_active = true;
    let mut sensitive = Sensors::new(profiles(
        Some(tuned_radar(90., 50., Preset::Advanced, 0.)),
        Some(passive("TESTI.SEE", 9., 10.)),
    ));
    let mut blinded = Sensors::new(profiles(
        Some(tuned_radar(90., 50., Preset::Advanced, 100.)),
        Some(passive("TESTI.SEE", 9., 10.)),
    ));
    sensitive.controls.channel = Channel::Infrared;
    blinded.controls.channel = Channel::Infrared;
    sensitive.step(&o, &[quiet], &air);
    let baseline = observed(&sensitive);
    assert_eq!(baseline.len(), 1);
    sensitive.step(&o, &[loud.clone()], &air);
    assert_eq!(observed(&sensitive), baseline);
    assert!(sensitive.strobes().is_empty());
    blinded.step(&o, &[loud.clone()], &air);
    assert_eq!(observed(&blinded), baseline);

    // The same geometry on the radar channel does depend on look-down.
    sensitive.controls.channel = Channel::Radar;
    blinded.controls.channel = Channel::Radar;
    sensitive.step(&o, &[loud.clone()], &air);
    blinded.step(&o, &[loud], &air);
    assert!(sensitive.contact(1).is_some());
    assert!(blinded.contact(1).is_none());
    assert_eq!(sensitive.strobes().len(), 1);
}

#[test]
fn a_destroyed_aircraft_stays_a_contact_until_it_stops_being_airborne() {
    let air = clear_air();
    let o = observer();
    let mut wreck = ahead(1, 30.);
    wreck.destroyed = true;
    let flying = [wreck.clone()];

    let mut s = hornet();
    s.step(&o, &flying, &air);
    assert_eq!(s.contacts().len(), 1);
    assert!(s.contacts()[0].destroyed);
    assert!(s.designate(1));
    run(&mut s, &o, &flying, ACQUIRE);
    assert_eq!(s.acquired(), Some(1));
    assert_eq!(s.support(1), Support::Tracked);
    assert_eq!(s.tick(), 61);
    assert_eq!(ticks(s.trail(1)), [0, 60]);

    // A grounded wreck ends the air-to-air observation on that step.
    let mut grounded = wreck;
    grounded.airborne = false;
    let events = s.step(&o, &[grounded], &air);
    assert!(s.contacts().is_empty());
    assert_eq!(s.selected(), None);
    assert_eq!(s.acquired(), None);
    assert!(events.contains(&Event::SelectionCleared(1)));
    assert!(events.contains(&Event::TrackReleased(1)));
    // Its bounded history ages out normally rather than being erased.
    assert_eq!(ticks(s.trail(1)), [0, 60]);
}

#[test]
fn a_jamming_target_denies_its_own_return_until_ownship_closes() {
    let air = clear_air();
    let o = observer();
    let emitter = |distance_nmi: f64| {
        let mut t = ahead(1, distance_nmi);
        t.jammer = Some(jammer(Generation::LateColdWar));
        t.jammer_active = true;
        [t]
    };

    // Against this advanced receiver the interference caps the 90 nmi search
    // volume at 0.40 of nominal, so the contact returns at 36 nmi.
    let mut s = hornet();
    s.step(&o, &emitter(50.), &air);
    assert!(s.contact(1).is_none());
    s.step(&o, &[ahead(1, 50.)], &air);
    assert!(s.contact(1).is_some());
    s.step(&o, &emitter(36.1), &air);
    assert!(s.contact(1).is_none());
    s.step(&o, &emitter(35.9), &air);
    assert!(s.contact(1).is_some());

    // The 50 nmi tracking volume follows the same factor, so the jammer is
    // still denying a weapon track at 30 nmi and has burned through at 10.
    s.step(&o, &emitter(30.), &air);
    let searching = *s.contact(1).expect("a search return at 30 nmi");
    assert!(!searching.track_eligible);
    assert!(s.designate(1));
    s.step(&o, &emitter(30.), &air);
    assert_eq!(s.support(1), Support::TrackCoverage);
    s.step(&o, &emitter(10.), &air);
    let tracking = *s.contact(1).expect("a track return at 10 nmi");
    assert!(tracking.track_eligible);
    assert_eq!(s.support(1), Support::Acquiring);

    // Received noise is never clipped by the selected display range.
    let mut scoped = hornet();
    scoped.controls.range_index = 0;
    scoped.step(&o, &emitter(50.), &air);
    assert_eq!(scoped.strobes().len(), 1);
    assert!(scoped.strobes()[0].bearing_rad.abs() < 1e-12);
    assert!(scoped.contacts().is_empty());
    scoped.step(&o, &[ahead(1, 50.)], &air);
    assert!(scoped.strobes().is_empty());
    assert_eq!(scoped.contacts().len(), 1);

    // Halving the emitter distance quadruples the received interference.
    let mut receiver = hornet();
    receiver.step(&o, &emitter(40.), &air);
    let far = receiver.strobes()[0].received;
    receiver.step(&o, &emitter(20.), &air);
    let near = receiver.strobes()[0].received;
    assert!((far - 0.25).abs() < 1e-12);
    assert!((near - 1.).abs() < 1e-12);
    assert!((near - 4. * far).abs() < 1e-12);

    // A radar that is off or failed shows no live noise at all.
    let mut off = observer();
    off.radar_powered = false;
    receiver.step(&off, &emitter(20.), &air);
    assert!(receiver.strobes().is_empty());
    let mut failed = observer();
    failed.radar_failed = true;
    receiver.step(&failed, &emitter(20.), &air);
    assert!(receiver.strobes().is_empty());
}

/// One recorded step of the scripted scenario: contacts, selection, acquired
/// track and the first target's history trail.
type Frame = (Vec<Contact>, Option<u32>, Option<u32>, Vec<Sample>);

/// One scripted 600-step scenario: two moving targets, scheduled range and
/// channel changes and two designations, all driven by the step index alone.
fn scripted_trace() -> Vec<Frame> {
    let air = clear_air();
    let o = observer();
    let mut s = Sensors::new(profiles(
        Some(radar(90., 50.)),
        Some(passive("TESTI.SEE", 9., 10.)),
    ));
    let mut trace = Vec::new();
    for step in 0..600u32 {
        let elapsed = f64::from(step);
        let targets = [
            ahead(1, 60. - elapsed * 0.08),
            bearing(2, 25., 8. + elapsed * 0.02),
        ];
        s.controls.range_index = match step {
            0..=199 => 3,
            200..=399 => 4,
            _ => 1,
        };
        s.controls.channel = if (250..350).contains(&step) {
            Channel::Infrared
        } else {
            Channel::Radar
        };
        s.step(&o, &targets, &air);
        match step {
            60 => {
                s.designate(1);
            }
            420 => {
                s.designate(2);
            }
            _ => {}
        }
        trace.push((
            s.contacts().to_vec(),
            s.selected(),
            s.acquired(),
            s.trail(1).to_vec(),
        ));
    }
    trace
}

#[test]
fn the_same_scripted_scenario_produces_the_same_trace_every_time() {
    let first = scripted_trace();
    let second = scripted_trace();
    assert_eq!(first.len(), 600);
    assert_eq!(first, second);

    // The scenario has to exercise what it claims to reproduce.
    assert!(first.iter().any(|(contacts, _, _, _)| !contacts.is_empty()));
    assert!(first.iter().any(|(_, selected, _, _)| selected.is_some()));
    assert!(first.iter().any(|(_, _, acquired, _)| acquired.is_some()));
    assert!(first.iter().any(|(_, _, _, trail)| !trail.is_empty()));
    assert!(first.iter().any(|(_, selected, _, _)| selected.is_none()));
}

#[test]
fn a_visual_contact_keeps_a_close_target_selected_when_the_radar_is_off() {
    let air = clear_air();
    let o = observer();
    let mut off = observer();
    off.radar_powered = false;
    let targets = [ahead(1, 3.)];

    let mut s = Sensors::new(with_visual(profiles(Some(radar(90., 50.)), None), 5.));
    s.step(&o, &targets, &air);
    assert!(s.designate(1));
    run(&mut s, &o, &targets, ACQUIRE);
    assert_eq!(s.acquired(), Some(1));

    let events = s.step(&off, &targets, &air);
    assert_eq!(s.selected(), Some(1));
    assert!(s.contacts().is_empty());
    assert_eq!(s.visual().len(), 1);
    assert_eq!(s.visual()[0].channel, Channel::Visual);
    assert_eq!(s.acquired(), None);
    assert_eq!(s.support(1), Support::RadarOff);
    assert!(events.contains(&Event::TrackReleased(1)));
    assert!(!selection_cleared(&events));

    run(&mut s, &off, &targets, 240);
    assert_eq!(s.selected(), Some(1));
    assert_eq!(s.acquired(), None);
}

#[test]
fn visual_contacts_stay_out_of_the_scope_channel_history_and_weapon_tracks() {
    let air = clear_air();
    let o = observer();
    // No ordinary radar return at any distance, so only the eye sees it.
    let mut invisible = ahead(1, 3.);
    invisible.signature.radar = 0.;
    let targets = [invisible];
    let empty: [Observable; 0] = [];

    let mut s = Sensors::new(with_visual(profiles(Some(radar(90., 50.)), None), 5.));
    s.step(&o, &targets, &air);
    assert!(s.contacts().is_empty());
    assert_eq!(s.visual().len(), 1);
    assert!(s.observation(1).is_some());

    assert!(s.designate(1));
    run(&mut s, &o, &targets, ACQUIRE * 5);
    assert_eq!(s.selected(), Some(1));
    assert_eq!(s.acquired(), None);
    assert_eq!(s.support(1), Support::NoObservation);
    assert!(s.contacts().is_empty());
    assert!(s.trail(1).is_empty());
    assert!(s.plots().is_empty());

    // Losing a visual-only observation leaves no stale scope plot behind.
    s.step(&o, &empty, &air);
    assert_eq!(s.selected(), None);
    assert!(s.visual().is_empty());
    assert!(s.plots().is_empty());
}

#[test]
fn received_noise_fades_over_a_quarter_second_and_stops_with_the_radar() {
    use super::track::NOISE_FADE_STEPS;
    let mut sensors = hornet();
    let mut emitter = ahead(1, 30.);
    emitter.jammer = Some(jammer(Generation::LateColdWar));
    emitter.jammer_active = true;
    let observer = observer();
    run(&mut sensors, &observer, &[emitter.clone()], 1);
    let live = sensors.strobes().to_vec();
    assert_eq!(live.len(), 1);
    assert_eq!(sensors.display_strobes().len(), 1);
    assert_eq!(sensors.display_strobes()[0].received, live[0].received);
    // Powering the emitter off fades its indication instead of cutting it.
    emitter.jammer_active = false;
    run(&mut sensors, &observer, &[emitter.clone()], 1);
    assert!(sensors.strobes().is_empty());
    let faded = sensors.display_strobes();
    assert_eq!(faded.len(), 1);
    assert!(faded[0].received < live[0].received && faded[0].received > 0.);
    run(
        &mut sensors,
        &observer,
        &[emitter.clone()],
        u64::from(NOISE_FADE_STEPS),
    );
    assert!(sensors.display_strobes().is_empty());
    // Losing the radar hides its noise at once, with no cosmetic tail.
    emitter.jammer_active = true;
    run(&mut sensors, &observer, &[emitter.clone()], 1);
    assert_eq!(sensors.display_strobes().len(), 1);
    let off = Observer {
        radar_powered: false,
        ..observer
    };
    run(&mut sensors, &off, &[emitter], 1);
    assert!(sensors.strobes().is_empty());
    assert!(sensors.display_strobes().is_empty());
}

#[test]
fn map_surface_returns_do_not_grant_air_to_air_selection() {
    let mut sensors = Sensors::new(with_visual(profiles(Some(radar(90., 50.)), None), 10.));
    let mut surface = ahead(7, 5.);
    surface.airborne = false;
    sensors.step(&observer(), &[surface.clone()], &clear_air());
    assert_eq!(sensors.map_contacts().len(), 1);
    assert!(sensors.map_contacts()[0].identified);
    assert!(!sensors.map_contacts()[0].airborne);
    assert!(sensors.contacts().is_empty());
    assert!(sensors.visual().is_empty());
    assert!(!sensors.designate(7));
    surface.position[2] = 20. * NMI;
    sensors.step(&observer(), &[surface.clone()], &clear_air());
    assert_eq!(sensors.map_contacts().len(), 1);
    assert!(!sensors.map_contacts()[0].identified);
    let mut off = observer();
    off.radar_powered = false;
    sensors.step(&off, &[surface.clone()], &clear_air());
    assert!(sensors.map_contacts().is_empty());
    sensors.step(&observer(), &[surface], &masked_air());
    assert!(sensors.map_contacts().is_empty());
}

#[test]
fn map_identity_requires_visual_and_lost_contacts_disappear() {
    let mut sensors = Sensors::new(with_visual(profiles(Some(radar(90., 50.)), None), 10.));
    let mut target = ahead(3, 20.);
    sensors.step(&observer(), &[target.clone()], &clear_air());
    assert!(!sensors.map_contacts()[0].identified);
    target.position[2] = 5. * NMI;
    target.destroyed = true;
    sensors.step(&observer(), &[target.clone()], &clear_air());
    assert!(sensors.map_contacts()[0].identified);
    assert!(sensors.map_contacts()[0].contact.destroyed);
    assert_eq!(sensors.map_contacts()[0].contact.position, target.position);
    let mut failed = observer();
    failed.visual_failed = true;
    sensors.step(&failed, &[target], &clear_air());
    assert!(!sensors.map_contacts()[0].identified);
    sensors.step(&observer(), &[], &clear_air());
    assert!(sensors.map_contacts().is_empty());
}
