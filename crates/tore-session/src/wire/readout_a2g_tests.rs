//! The air-to-ground round's share of the readout (protocol 23, slice NA):
//! its groups stand for the cockpit's sight, laser, bomb sight, guided sight,
//! HARM cue, weapons in flight and ground pages; a group that goes away
//! reads as none on the client; and a synthetic F/A-18D's real readout,
//! sight pinned and laser on by hand with a laser bomb in flight, comes
//! through a connection as the host built it.

use super::connection::{ClientConnection, HostConnection};
use super::readout::QReadout;
use super::samples;
use tore_sim::combat::delivery::Warning;
use tore_sim::combat::live::Command as Combat;
use tore_world::combat::launcher;
use tore_world::mission::{MissionSpec, Start};
use tore_world::readout::CockpitReadout;
use tore_world::seats::{SeatCommand, SeatId, SeatInput};
use tore_world::test_support::resources::{a2g_loadout, add_a2g_stores};
use tore_world::test_support::surface::{routed_resources, spec_with_target, target};
use tore_world::world::{Seating, TickOutput, World};

fn round_trip(readout: &CockpitReadout) -> CockpitReadout {
    QReadout::of(readout, 400).readout(400, None, None).unwrap()
}

#[test]
fn the_air_to_ground_groups_stand_for_the_readout() {
    // The sample is on the wire's grid: every group comes back exactly.
    let readout = samples::readout();
    let back = round_trip(&readout);
    assert_eq!(back.sight, readout.sight);
    assert_eq!(back.laser, readout.laser);
    assert_eq!(back.bomb_sight, readout.bomb_sight);
    assert_eq!(back.guided, readout.guided);
    assert_eq!(back.estimates.harm, readout.estimates.harm);
    assert_eq!(back.weapon_tti, readout.weapon_tti);
    assert_eq!(back.guided_impact_tick, Some(380));
    assert!(back.sensors.air_ground && back.sensors.harm);
    // The ground page's contacts: every field but the bearings, which the
    // client works out around its own plane.
    let mut sent = readout.sensors.ground.clone();
    sent.sort_by_key(|g| g.contact.id);
    let mut got = back.sensors.ground.clone();
    got.sort_by_key(|g| g.contact.id);
    assert_eq!(got, sent);
    // The warnings keep their order.
    assert_eq!(
        back.bomb_sight.as_ref().unwrap().warnings,
        [Warning::Low, Warning::Overfly]
    );
    // Rounding twice changes nothing.
    assert_eq!(QReadout::of(&back, 400), QReadout::of(&readout, 400));
}

#[test]
fn the_guided_level_t_rides_the_guided_group_and_may_be_absent() {
    let mut readout = samples::readout();
    let sent = readout.guided.unwrap().level.unwrap();
    let back = round_trip(&readout).guided.unwrap().level.unwrap();
    assert_eq!(back, sent);
    // The client's T numbers are the host's.
    let sight = round_trip(&readout).guided.unwrap();
    assert_eq!(
        sight.release_seconds(),
        readout.guided.unwrap().release_seconds()
    );
    readout.guided.as_mut().unwrap().level = None;
    assert_eq!(round_trip(&readout).guided.unwrap().level, None);
}

#[test]
fn a_cockpit_without_the_air_to_ground_parts_reads_none_of_them() {
    let mut plain = samples::readout();
    plain.sight = None;
    plain.laser = None;
    plain.bomb_sight = None;
    plain.guided = None;
    plain.estimates.harm = None;
    plain.weapon_tti.clear();
    plain.guided_impact_tick = None;
    plain.sensors.air_ground = false;
    plain.sensors.harm = false;
    plain.sensors.ground.clear();
    let back = round_trip(&plain);
    assert_eq!(back.sight, None);
    assert_eq!(back.laser, None);
    assert_eq!(back.bomb_sight, None);
    assert_eq!(back.guided, None);
    assert_eq!(back.estimates.harm, None);
    assert!(back.weapon_tti.is_empty());
    assert_eq!(back.guided_impact_tick, None);
    assert!(!back.sensors.air_ground && !back.sensors.harm);
    assert!(back.sensors.ground.is_empty());
    // Each part alone, and the CCIP or the level release alone.
    let full = samples::readout();
    let mut one = plain.clone();
    one.laser = full.laser;
    let back = round_trip(&one);
    assert_eq!((back.laser, back.sight), (full.laser, None));
    let mut bomb = full.bomb_sight.clone().unwrap();
    bomb.level = None;
    bomb.warnings.clear();
    bomb.falling = None;
    one.bomb_sight = Some(bomb.clone());
    assert_eq!(round_trip(&one).bomb_sight, Some(bomb.clone()));
    bomb.ccip = None;
    bomb.rocket = true;
    one.bomb_sight = Some(bomb.clone());
    assert_eq!(round_trip(&one).bomb_sight, Some(bomb));
    let mut laser = full.laser.unwrap();
    laser.buddy = None;
    laser.spot = None;
    laser.manual = Some(false);
    laser.state = tore_sim::combat::laser::LaserState::Masked;
    laser.mask = Some(tore_sim::combat::laser::Mask::Terrain);
    one.laser = Some(laser);
    assert_eq!(round_trip(&one).laser, Some(laser));
}

#[test]
fn values_off_the_grid_come_back_within_half_a_step() {
    let mut readout = samples::readout();
    let mut sight = readout.sight.unwrap();
    sight.aim = sight.aim.map(|a| a.map(|x| x + 0.051));
    sight.look[0] += 1e-7;
    readout.sight = Some(sight);
    let mut bomb = readout.bomb_sight.clone().unwrap();
    let ccip = bomb.ccip.as_mut().unwrap();
    ccip.impact[0] += 0.03;
    ccip.seconds += 0.004;
    readout.bomb_sight = Some(bomb.clone());
    let back = round_trip(&readout);
    let got = back.sight.unwrap();
    assert!((0..3).all(|i| (got.aim.unwrap()[i] - sight.aim.unwrap()[i]).abs() <= 1. / 16.));
    assert!((got.look[0] - sight.look[0]).abs() <= std::f64::consts::TAU / 2_097_152.);
    let c = back.bomb_sight.unwrap().ccip.unwrap();
    assert!((c.impact[0] - bomb.ccip.unwrap().impact[0]).abs() <= 1. / 16.);
    assert!((c.seconds - bomb.ccip.unwrap().seconds).abs() <= 1. / 128.);
}

#[test]
fn a_part_that_went_away_reads_none_through_a_connection() {
    // A bomb sight that was there and is not (the bomb is gone): the client
    // shows none, not the last values that arrived.
    let mut host = HostConnection::new(4);
    let mut client = ClientConnection::for_flight(4, 0);
    let with = samples::readout();
    let mut without = with.clone();
    without.tick += 4;
    without.bomb_sight = None;
    without.laser = None;
    without.weapon_tti.clear();
    for (sequence, (tick, readout)) in [(400u32, &with), (404, &without)].into_iter().enumerate() {
        let packet = host
            .snapshot_with_readout(&samples::header(tick), &[], Some(readout), 0)
            .unwrap();
        host.sent(sequence as u16);
        let (_, received) = client.snapshot(&packet.snapshot).unwrap();
        host.delivered(sequence as u16);
        assert!(received.readout.is_some());
    }
    let shown = client
        .cockpit_readout(
            &tore_sim::flight::State::new(&tore_world::test_support::aircraft(), [0.; 3]).unwrap(),
            None,
        )
        .expect("a readout")
        .unwrap();
    assert_eq!(shown.bomb_sight, None);
    assert_eq!(shown.laser, None);
    assert!(shown.weapon_tti.is_empty());
    assert_eq!(shown.sight, with.sight, "what did not change stays");
}

/// The synthetic F/A-18D with a laser pod and two laser bombs, flying at the
/// routed column.
pub(crate) fn a2g_world() -> World {
    let mut resources = routed_resources();
    add_a2g_stores(&mut resources);
    World::new(&a2g_spec(), &resources, Seating::SinglePlayer).unwrap()
}

/// The spec of [`a2g_world`].
pub(crate) fn a2g_spec() -> MissionSpec {
    let mut spec = spec_with_target(&target("QUCOL", 0, 0, 7));
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec.loadout = Some(a2g_loadout());
    spec
}

/// The commands the seat gives on `tick` of an air-to-ground run: the A-G
/// radar on and the laser bomb selected at once, the sight pinned on the
/// ground ahead after one second, Ctrl+L (the laser, on by the automatic
/// rule with the bomb selected, off by hand), and a bomb released at three
/// seconds (it waits for a spot; the release ends the hand setting).
pub(crate) fn a2g_commands(tick: u64) -> (Vec<SeatCommand>, bool) {
    let commands = match tick {
        1 => vec![
            SeatCommand::Combat(Combat::ToggleAirGround),
            SeatCommand::CycleWeapon { forward: true },
        ],
        120 => vec![SeatCommand::Combat(Combat::SightPinGround)],
        180 => vec![SeatCommand::Combat(Combat::ToggleLaser)],
        _ => Vec::new(),
    };
    (commands, (360..364).contains(&tick))
}

/// A player's game flying [`a2g_commands`]'s run by its clock: the A-G
/// radar and the laser bomb a second after it first flies, the pin at two,
/// Ctrl+L at three, the trigger from four seconds for a quarter second. For
/// the session tests (standbys, takeovers) that carry its state.
#[derive(Clone, Debug, Default)]
pub(crate) struct A2gPilot {
    flying_since: Option<std::time::Duration>,
    given: usize,
}

impl A2gPilot {
    /// Adds the run's commands and trigger due at `now` to `controls`.
    pub(crate) fn drive(
        &mut self,
        now: std::time::Duration,
        controls: &mut crate::client::Controls,
    ) {
        use std::time::Duration;
        let since = now.saturating_sub(*self.flying_since.get_or_insert(now));
        let schedule: [(Duration, &[SeatCommand]); 3] = [
            (
                Duration::from_secs(1),
                &[
                    SeatCommand::Combat(Combat::ToggleAirGround),
                    SeatCommand::CycleWeapon { forward: true },
                ],
            ),
            (
                Duration::from_secs(2),
                &[SeatCommand::Combat(Combat::SightPinGround)],
            ),
            (
                Duration::from_secs(3),
                &[SeatCommand::Combat(Combat::ToggleLaser)],
            ),
        ];
        while let Some((at, commands)) = schedule.get(self.given) {
            if since < *at {
                break;
            }
            controls.commands.extend_from_slice(commands);
            self.given += 1;
        }
        controls.trigger = (Duration::from_secs(4)..Duration::from_millis(4_250)).contains(&since);
    }
}

#[test]
fn a_laser_bombers_readout_comes_through_a_connection_as_the_host_built_it() {
    let mut world = a2g_world();
    let mut out = TickOutput::default();
    let mut host = HostConnection::new(4);
    let mut client = ClientConnection::for_flight(4, 0);
    let mut sequence = 0u16;
    let mut seen = (false, false, false);
    for tick in 0..720u64 {
        let (commands, trigger) = a2g_commands(tick);
        world
            .step(
                &[SeatInput {
                    tick,
                    commands,
                    trigger,
                    ..SeatInput::default()
                }],
                &mut out,
            )
            .unwrap();
        if tick % 4 != 3 {
            continue;
        }
        let readout = world
            .cockpit_readout(SeatId(0), launcher(&world.cockpits[0].flight))
            .unwrap();
        let packet = host
            .snapshot_with_readout(
                &samples::header(world.tick() as u32),
                &[],
                Some(&readout),
                0,
            )
            .unwrap();
        host.sent(sequence);
        let (_, received) = client.snapshot(&packet.snapshot).unwrap();
        host.delivered(sequence);
        sequence = sequence.wrapping_add(1);
        let got = received
            .readout
            .unwrap()
            .readout(world.tick() as u32, None, None)
            .unwrap();
        // What the host built, within the wire's steps: the same numbers
        // when coded again (the tower's service aside: it is rebuilt on the
        // airport scene, which this test does not pass).
        let mut built = readout.clone();
        built.airport.service = None;
        assert_eq!(
            QReadout::of(&got, world.tick() as u32),
            QReadout::of(&built, world.tick() as u32),
            "tick {tick}"
        );
        let sight = got.sight.expect("the pod's sight");
        assert!(sight.sensor.laser);
        if matches!(sight.mode, tore_sim::combat::sight::SightMode::Pinned(_)) {
            seen.0 = true;
        }
        let laser = got.laser.expect("the pod's laser");
        if laser.manual.is_some() {
            seen.1 = true;
        }
        if !got.weapon_tti.is_empty() {
            assert_eq!(
                got.weapon_tti[0].kind,
                tore_sim::combat::missiles::Guidance::Laser
            );
            seen.2 = true;
        }
        assert!(got.sensors.air_ground || tick < 4);
    }
    assert_eq!(
        seen,
        (true, true, true),
        "pinned, the laser set by hand, a bomb in flight"
    );
    assert!(
        world.combat.state.guided_flights().next().is_some(),
        "the bomb flies a guided flight on the host"
    );
}

#[test]
fn quantized_bomb_body_keeps_client_trail_dots_within_a_tenth_of_a_foot() {
    let mut readout = samples::readout();
    let ccip = readout.bomb_sight.as_mut().unwrap().ccip.as_mut().unwrap();
    ccip.release.body = tore_sim::combat::bomb_flight::Body::default();
    ccip.seconds = 40.;
    let back = round_trip(&readout);
    let host = readout.bomb_sight.as_ref().unwrap().ccip.as_ref().unwrap();
    let client = back.bomb_sight.as_ref().unwrap().ccip.as_ref().unwrap();
    for (a, b) in host.trail().zip(client.trail()) {
        let distance = a
            .iter()
            .zip(b)
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            .sqrt();
        assert!(distance < 0.1, "wire body moved trail {distance} ft");
    }
}
