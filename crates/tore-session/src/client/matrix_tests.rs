//! The network matrix (slice D10): a host and two bots fly a scripted fight
//! on the network simulator, once for each round trip (50, 150 and 300 ms,
//! arrivals spread by plus or minus 10 percent of the one-way delay) and each
//! loss (0, 2 and 5 percent each way, with 1 percent of packets duplicated),
//! and every limit of the stage D acceptance table
//! (docs/MULTIPLAYER.md, "Netcode numbers") is measured and asserted.
//!
//! Two forms of each cell: a short one of [`SHORT_SECONDS`] simulated seconds
//! in the normal suite, and the acceptance's five minutes as an ignored test:
//!
//! ```sh
//! cargo test --release -p tore-session --lib matrix_tests -- --ignored --nocapture
//! ```
//!
//! The mission is built from the synthetic fixtures: four friendly aircraft
//! (the two bots and two AI) against four enemy AI, one nautical mile apart
//! so the fight starts at once. Every figure is judged for each bot on its
//! own; the worst of the two has to pass.

use super::tests::{Rig, bot_script, spec};
use super::*;
use crate::host::Unforeseen;
use tore_net::sim::LinkConfig;

/// The short form of a cell, simulated seconds.
const SHORT_SECONDS: u64 = 60;
/// The acceptance's form.
const FULL_SECONDS: u64 = 300;
/// The inputs the host repeats are judged after this long.
const SETTLE: Duration = Duration::from_secs(5);
/// A hit, blast or release explains a correction made within this long.
const EXPLAINED_TICKS: u64 = 120;
/// A correction under this is too small to show (feet, degrees).
const TOO_SMALL: f64 = 0.01;

/// The plan's bandwidth budget per player, bytes a second
/// (docs/multiplayer-plan.md): 22 KB/s down, 4 KB/s up. The table's limit is
/// that the figures are measured and recorded against it, so the test only
/// asserts twice the budget (agent decision) and prints the rest.
const BUDGET_DOWN: f64 = 22_000.;
const BUDGET_UP: f64 = 4_000.;
const BANDWIDTH_SLACK: f64 = 2.;

/// One cell of the matrix.
#[derive(Clone, Copy, Debug)]
struct Cell {
    round_trip_ms: u64,
    /// Percent lost each way.
    loss_percent: u64,
}

impl Cell {
    fn loss(self) -> f64 {
        self.loss_percent as f64 / 100.
    }
}

/// The limits of the acceptance table for a loss.
#[derive(Clone, Copy, Debug)]
struct Limits {
    /// Share of snapshots that may need a visible correction.
    visible: f64,
    /// The distance within which 99 percent of other-aircraft frames must
    /// be, feet.
    others_within_ft: f64,
    /// Share of entity frames drawn past their newest state.
    extrapolated: f64,
    /// Share of ticks the host repeated an input for.
    repeated: f64,
}

fn limits(loss_percent: u64) -> Limits {
    match loss_percent {
        0 => Limits {
            visible: 0.01,
            others_within_ft: 1.,
            extrapolated: 0.01,
            repeated: 0.005,
        },
        2 => Limits {
            visible: 0.01,
            others_within_ft: 3.,
            extrapolated: 0.01,
            repeated: 0.005,
        },
        _ => Limits {
            visible: 0.03,
            others_within_ft: 10.,
            extrapolated: 0.03,
            repeated: 0.02,
        },
    }
}

/// What one bot measured.
#[derive(Debug, Default)]
struct Figures {
    // Own aircraft.
    snapshots: u64,
    hashes: u64,
    corrections: u64,
    /// Every exact state that changed the prediction, seating's included.
    corrections_all: u64,
    /// Corrections with no late input, hit, blast or release in the second
    /// before their tick: the limit says none.
    unexplained: u64,
    /// Corrections big enough to show (blended or snapped), seating's
    /// second apart.
    visible: u64,
    /// Corrections outside the second after a hit, blast or release, and of
    /// them the ones under a foot.
    outside_events: u64,
    outside_small: u64,
    worst_correction_ft: f64,
    /// Snapshots whose second before had a hit, blast or release, as a share
    /// of the seat's ticks (how much of the run the event rule exempts).
    event_ticks: u64,
    late_input_ticks: u64,
    // Other aircraft.
    drawn: u64,
    within_limit: u64,
    within_1: u64,
    worst_other_ft: f64,
    // Extrapolation and repeats.
    entity_frames: u64,
    extrapolated: u64,
    /// Of the entity frames and the extrapolated ones, those drawn with an
    /// extra delay (sent twice a second).
    far_frames: u64,
    far_extrapolated: u64,
    ticks_judged: u64,
    repeated_judged: u64,
    repeated_all: u64,
    // Bandwidth, host to player and player to host, bytes a second, from the
    // host's one-second windows.
    down_mean: f64,
    up_mean: f64,
    down_max: u64,
    up_max: u64,
    round_trip: Duration,
}

fn pct(n: u64, of: u64) -> f64 {
    100. * n as f64 / of.max(1) as f64
}

impl Figures {
    fn line(&self, name: &str) -> String {
        format!(
            "{name}: own: {} hashes, {} snapshots, {} corrections ({} after seating, {} unexplained, {} visible = {:.2}%), \
             outside events {}/{} under 1 ft (worst {:.2} ft); others {} frames: within 1 ft \
             {:.2}%, limit {:.2}%, worst {:.1} ft; extrapolated {:.3}% ({} of {} far frames); repeated {:.3}% \
             (after settle {:.3}%); host to player {:.0} B/s (max {}), player to host \
             {:.0} B/s (max {}); rtt {:?}",
            self.hashes,
            self.snapshots,
            self.corrections_all,
            self.corrections,
            self.unexplained,
            self.visible,
            pct(self.visible, self.snapshots),
            self.outside_small,
            self.outside_events,
            self.worst_correction_ft,
            self.drawn,
            pct(self.within_1, self.drawn),
            pct(self.within_limit, self.drawn),
            self.worst_other_ft,
            pct(self.extrapolated, self.entity_frames),
            self.far_extrapolated,
            self.far_frames,
            pct(self.repeated_all, self.ticks_judged.max(1)),
            pct(self.repeated_judged, self.ticks_judged),
            self.down_mean,
            self.down_max,
            self.up_mean,
            self.up_max,
            self.round_trip,
        )
    }
}

/// Runs one cell and measures both bots.
fn run(cell: Cell, seconds: u64) -> Vec<Figures> {
    let link = LinkConfig::for_round_trip(
        Duration::from_millis(cell.round_trip_ms),
        0.1,
        cell.loss(),
        0.01,
    );
    let seed = 1000 + cell.round_trip_ms + cell.loss_percent;
    let mut rig = Rig::new(spec(4, 4, 1), link, seed);
    rig.watch = true;
    let a = rig.join(|c| c.callsign = "Alpha".into(), bot_script());
    let b = rig.join(|c| c.callsign = "Bravo".into(), bot_script());
    let bots = [a, b];
    assert!(
        rig.run_until(Duration::from_secs(12), |r| r.seated(a) && r.seated(b)),
        "{cell:?}: both bots were seated: {:?} {:?}",
        rig.players[a].events,
        rig.players[b].events
    );
    let flying_from = rig.net.now();
    let end = flying_from + Duration::from_secs(seconds);
    let mut figures: Vec<Figures> = bots.iter().map(|_| Figures::default()).collect();
    let mut counted = [0usize; 2];
    let mut next_second = flying_from + Duration::from_secs(1);
    let mut seconds_sampled = 0u64;
    let mut after_settle: Option<[u64; 2]> = None;
    let mut sums = [(0u64, 0u64); 2];
    while rig.net.now() < end {
        rig.step();
        let now = rig.net.now();
        // Each seat's bytes in the host's one-second windows.
        if now >= next_second {
            next_second += Duration::from_secs(1);
            seconds_sampled += 1;
            let players = rig.host.players();
            for (i, f) in figures.iter_mut().enumerate() {
                let seat = rig.players[bots[i]].client.seat().map(|(s, _)| s.0);
                if let Some(p) = players.iter().find(|p| p.seat == seat) {
                    sums[i].0 += p.bytes_up_per_second;
                    sums[i].1 += p.bytes_down_per_second;
                    f.down_max = f.down_max.max(p.bytes_up_per_second);
                    f.up_max = f.up_max.max(p.bytes_down_per_second);
                }
            }
        }
        if after_settle.is_none() && now >= flying_from + SETTLE {
            after_settle = Some(bots.map(|i| rig.players[i].client.clone_stats().inputs_repeated));
        }
        for (i, f) in figures.iter_mut().enumerate() {
            let player = &rig.players[bots[i]];
            if player.digests.len() == counted[i] {
                continue;
            }
            counted[i] = player.digests.len();
            let (Some(picture), Some(render)) = (&player.picture, player.client.render_tick())
            else {
                continue;
            };
            for pose in picture.targets.iter().filter(|p| p.aircraft.is_some()) {
                let key = EntityKey {
                    kind: crate::wire::entity::EntityKind::Aircraft,
                    id: pose.id,
                };
                let extra = player.client.interpolator().extra(key).unwrap_or(0.);
                let Some(truth) = rig.truth_at(pose.id, render - extra) else {
                    continue;
                };
                let error = (0..3)
                    .map(|k| (pose.position[k] - truth[k]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                f.drawn += 1;
                f.within_1 += u64::from(error <= 1.);
                f.within_limit += u64::from(error <= limits(cell.loss_percent).others_within_ft);
                f.worst_other_ft = f.worst_other_ft.max(error);
            }
        }
    }
    let ticks = seconds * 120;
    let settled_ticks = ticks.saturating_sub(SETTLE.as_secs() * 120);
    let at_end: Vec<ClientStats> = bots
        .iter()
        .map(|&i| rig.players[i].client.clone_stats())
        .collect();
    let seats: Vec<_> = bots
        .iter()
        .map(|&i| rig.players[i].client.seat().expect("seated").0)
        .collect();
    // The network's delay, spread and loss never pass for a stalled game:
    // no seat was flown neutral (EF-K follow-up).
    let neutral: u64 = rig.host.players().iter().map(|p| p.inputs_neutral).sum();
    assert_eq!(neutral, 0, "{cell:?}: ticks flown neutral as stalled");
    // Leave cleanly: a refusal or a drop would show here.
    for &i in &bots {
        let now = rig.net.now();
        rig.players[i].client.leave_game(now);
    }
    assert!(
        rig.run_until(Duration::from_secs(12), |r| r.closed(a) && r.closed(b)),
        "{cell:?}: both bots left"
    );
    for (n, &i) in bots.iter().enumerate() {
        let p = &rig.players[i];
        assert!(
            p.events
                .iter()
                .any(|e| matches!(e, ClientEvent::Debrief(_))),
            "{cell:?}: bot {n} got its debrief: {:?}",
            p.events
        );
        assert!(
            p.events.iter().any(|e| matches!(
                e,
                ClientEvent::Closed(CloseReason::Disconnected {
                    reason: DisconnectReason::Left,
                    by_peer: false
                })
            )),
            "{cell:?}: bot {n} left cleanly: {:?}",
            p.events
        );
    }
    let log = rig.host.unforeseen_log().to_vec();
    for (n, &i) in bots.iter().enumerate() {
        let player = &rig.players[i];
        let f = &mut figures[n];
        let stats = &at_end[n];
        let seat = seats[n];
        let seated_tick = player
            .events
            .iter()
            .find_map(|e| match e {
                ClientEvent::Seated { tick, .. } => Some(u64::from(*tick)),
                _ => None,
            })
            .expect("a Seated event");
        let causes: Vec<(u64, Unforeseen)> = log
            .iter()
            .filter(|(_, s, _)| *s == seat)
            .map(|&(tick, _, why)| (tick, why))
            .collect();
        let within = |c: u64, wanted: Option<Unforeseen>| {
            causes.iter().any(|&(t, why)| {
                t <= c && c - t < EXPLAINED_TICKS && wanted.is_none_or(|w| w == why)
            })
        };
        f.snapshots = stats.snapshots;
        f.hashes = stats.hashes_compared;
        f.corrections_all = stats.corrections;
        // Only the flying: leaving hands the plane back, and the corrections
        // that follow are not flight.
        for c in &player.client.corrections()[..stats.corrections as usize] {
            // Seating's own second: the plane is snapped into place.
            if c.tick < seated_tick + EXPLAINED_TICKS {
                continue;
            }
            f.corrections += 1;
            f.worst_correction_ft = f.worst_correction_ft.max(c.feet);
            f.unexplained += u64::from(!within(c.tick, None));
            f.visible += u64::from(c.feet >= TOO_SMALL || c.degrees >= TOO_SMALL);
            if !within(c.tick, Some(Unforeseen::Event)) {
                f.outside_events += 1;
                f.outside_small += u64::from(c.feet < 1.);
            }
        }
        let host_ticks = ticks;
        f.event_ticks = (0..host_ticks)
            .filter(|k| within(seated_tick + k, Some(Unforeseen::Event)))
            .count() as u64;
        f.late_input_ticks = (0..host_ticks)
            .filter(|k| within(seated_tick + k, Some(Unforeseen::LateInput)))
            .count() as u64;
        f.entity_frames = stats.entity_frames;
        f.extrapolated = stats.extrapolated;
        f.far_frames = stats.far_frames;
        f.far_extrapolated = stats.far_extrapolated;
        f.ticks_judged = settled_ticks;
        f.repeated_all = stats.inputs_repeated;
        f.repeated_judged = stats
            .inputs_repeated
            .saturating_sub(after_settle.expect("settled")[n]);
        f.round_trip = stats.round_trip;
        f.down_mean = sums[n].0 as f64 / seconds_sampled.max(1) as f64;
        f.up_mean = sums[n].1 as f64 / seconds_sampled.max(1) as f64;
        let name = format!(
            "[{} ms, {}%] {} bot {n}",
            cell.round_trip_ms,
            cell.loss_percent,
            if seconds >= FULL_SECONDS {
                "full"
            } else {
                "short"
            }
        );
        eprintln!("{}", f.line(&name));
        eprintln!(
            "    exempt ticks: {:.1}% after a hit, blast or release, {:.1}% after a late input",
            pct(f.event_ticks, host_ticks),
            pct(f.late_input_ticks, host_ticks)
        );
    }
    figures
}

/// Asserts every limit of the acceptance table for both bots.
fn check(cell: Cell, figures: &[Figures]) {
    let limit = limits(cell.loss_percent);
    for (n, f) in figures.iter().enumerate() {
        let who = format!("{cell:?} bot {n}");
        // Own aircraft: no correction with no late input, hit, blast or
        // release in the second before.
        assert_eq!(
            f.unexplained,
            0,
            "{who}: corrections with nothing to explain them\n{}",
            f.line("figures")
        );
        // At most 1 or 3 percent of snapshots need a visible correction.
        assert!(
            (f.visible as f64) <= limit.visible * f.snapshots as f64,
            "{who}: {} of {} snapshots needed a visible correction (limit {})",
            f.visible,
            f.snapshots,
            limit.visible
        );
        // 99 percent of the corrections outside the second after an event are
        // under a foot.
        assert!(
            f.outside_small as f64 >= 0.99 * f.outside_events as f64,
            "{who}: {} of {} corrections outside events were under a foot",
            f.outside_small,
            f.outside_events
        );
        // The fight happened: the bot fired, so the release rule was tested.
        assert!(f.event_ticks > 0, "{who}: the bot fired no gun");
        // Other aircraft: 99 percent within the distance for the loss.
        assert!(f.drawn > 0, "{who}: other aircraft were drawn");
        assert!(
            f.within_limit as f64 >= 0.99 * f.drawn as f64,
            "{who}: {:.2}% of other-aircraft frames within {} ft (limit 99%)",
            pct(f.within_limit, f.drawn),
            limit.others_within_ft
        );
        assert!(
            (f.extrapolated as f64) < limit.extrapolated * f.entity_frames as f64,
            "{who}: {:.3}% of entity frames extrapolated (limit {}%)",
            pct(f.extrapolated, f.entity_frames),
            100. * limit.extrapolated
        );
        assert!(
            (f.repeated_judged as f64) < limit.repeated * f.ticks_judged as f64,
            "{who}: {:.3}% of ticks repeated after the first 5 s (limit {}%)",
            pct(f.repeated_judged, f.ticks_judged),
            100. * limit.repeated
        );
        // Bandwidth against the plan's budget.
        assert!(
            f.down_mean < BANDWIDTH_SLACK * BUDGET_DOWN,
            "{who}: download {:.0} B/s is over twice the budget",
            f.down_mean
        );
        assert!(
            f.up_mean < BANDWIDTH_SLACK * BUDGET_UP,
            "{who}: upload {:.0} B/s is over twice the budget",
            f.up_mean
        );
    }
}

macro_rules! cells {
    ($($short:ident, $full:ident: $rtt:expr, $loss:expr;)*) => {
        $(
            #[test]
            fn $short() {
                let cell = Cell { round_trip_ms: $rtt, loss_percent: $loss };
                check(cell, &run(cell, SHORT_SECONDS));
            }

            #[test]
            #[ignore = "five minutes of simulated flight; run before a release"]
            fn $full() {
                let cell = Cell { round_trip_ms: $rtt, loss_percent: $loss };
                check(cell, &run(cell, FULL_SECONDS));
            }
        )*
    };
}

cells! {
    rtt_50_loss_0, full_rtt_50_loss_0: 50, 0;
    rtt_50_loss_2, full_rtt_50_loss_2: 50, 2;
    rtt_50_loss_5, full_rtt_50_loss_5: 50, 5;
    rtt_150_loss_0, full_rtt_150_loss_0: 150, 0;
    rtt_150_loss_2, full_rtt_150_loss_2: 150, 2;
    rtt_150_loss_5, full_rtt_150_loss_5: 150, 5;
    rtt_300_loss_0, full_rtt_300_loss_0: 300, 0;
    rtt_300_loss_2, full_rtt_300_loss_2: 300, 2;
    rtt_300_loss_5, full_rtt_300_loss_5: 300, 5;
}
