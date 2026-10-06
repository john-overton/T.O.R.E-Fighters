//! What each human costs the host, on a 15 against 15 mission with real data:
//! the host's elapsed time per tick and per human, and the bytes each way
//! per player and in total, with 0, 2, 8, 15 or 30 bots flying (slice D10,
//! docs/baselines/net-2026-09-30.md).
//!
//! The host and the bots run in one process on the network simulator with a
//! perfect link, stepped a millisecond at a time with no waiting. Elapsed time
//! includes worker completion and scheduler delays: measure on a quiet machine.
//! Linux also reports the host thread's CPU time, which excludes worker CPU
//! and must not be used as the threading speed-up. The bots' work is outside
//! the measured calls. Run it
//! in a release build, one count at a time:
//!
//! ```sh
//! TORE_DATA_DIR=$PWD/.local/mpb-data-matrix TORE_MEASURE_BOTS=15 \
//!     cargo test --release --locked -p tore-session --test host_players -- --ignored --nocapture
//! ```
//!
//! `TORE_MEASURE_BOTS` is the number of bots (default 2), `TORE_MEASURE_OPEN`
//! is `friendly` (default) or `all` (needed above 15),
//! `TORE_MEASURE_SECONDS` is the simulated flying time (default 300) and
//! `TORE_MEASURE_RATE` the snapshot rate (default the host's, 60 since slice
//! D12).

use std::sync::Arc;
use std::time::{Duration, Instant};
use tore_formats::aircraft::AircraftId;
use tore_net::Entropy;
use tore_net::sim::{LinkConfig, SimNetwork};
use tore_session::bot::Bot;
use tore_session::{
    BuildId, Client, ClientConfig, ClientPhase, Host, HostConfig, HostLog, OpenPlanes, StartMode,
};
use tore_world::mission::{MissionSpec, Skill, Start};

fn build() -> BuildId {
    BuildId {
        version: "test".into(),
        commit: "test".into(),
        release: false,
    }
}

/// Five aircraft in each of the six wings: F/A-18Ds against MiG-29s, 10 nm
/// apart at 10,000 feet.
fn spec() -> MissionSpec {
    let mut spec = MissionSpec::new("UKR", AircraftId::F18);
    for (index, wing) in spec.wings.iter_mut().enumerate() {
        wing.count = 5;
        wing.skill = Skill::Average;
        if index >= 3 {
            wing.aircraft = AircraftId::Mig29;
        }
    }
    spec.separation_nm = 10;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

/// This thread's processor time, nanoseconds.
struct CpuClock {
    #[cfg(target_os = "linux")]
    file: Option<std::fs::File>,
    started: Instant,
}

impl CpuClock {
    fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            file: std::fs::File::open("/proc/thread-self/schedstat").ok(),
            started: Instant::now(),
        }
    }

    /// Whether this is processor time (else wall time).
    fn is_cpu(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.file.is_some()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    fn now(&self) -> u64 {
        #[cfg(target_os = "linux")]
        if let Some(file) = &self.file {
            use std::os::unix::fs::FileExt;
            let mut buffer = [0u8; 128];
            if let Ok(read) = file.read_at(&mut buffer, 0)
                && let Some(ns) = std::str::from_utf8(&buffer[..read])
                    .ok()
                    .and_then(|text| text.split_whitespace().next())
                    .and_then(|first| first.parse::<u64>().ok())
            {
                return ns;
            }
        }
        self.started.elapsed().as_nanos() as u64
    }
}

fn percentile(sorted: &[u64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.;
    }
    sorted[((sorted.len() - 1) as f64 * p) as usize] as f64 / 1e6
}

#[test]
#[ignore = "reads a real import through TORE_DATA_DIR; run by hand in release"]
fn host_cost_and_bandwidth_with_bots_on_a_15_against_15_mission() {
    let bots: usize = std::env::var("TORE_MEASURE_BOTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let seconds: u64 = std::env::var("TORE_MEASURE_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let rate: Option<u32> = std::env::var("TORE_MEASURE_RATE")
        .ok()
        .and_then(|v| v.parse().ok());
    let open = match std::env::var("TORE_MEASURE_OPEN").as_deref() {
        Ok("all") => OpenPlanes::All,
        _ => OpenPlanes::Friendly,
    };
    let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
    let resources = Arc::new(tore_import::load(&directory).expect("an imported pack"));

    let net = SimNetwork::new(7);
    net.set_default_link(LinkConfig::PERFECT);
    let address = "10.0.0.1:26900".parse().unwrap();
    let mut host_socket = net.bind(address).unwrap();
    let mut host = Host::new(
        spec(),
        Arc::clone(&resources),
        HostConfig {
            open_planes: open.clone(),
            start: StartMode::Now,
            entropy: Entropy::Seeded(11),
            snapshot_rate: rate.unwrap_or(HostConfig::new(build()).snapshot_rate),
            ..HostConfig::new(build())
        },
    )
    .expect("the host starts");
    let snapshot_rate = host.config().snapshot_rate;
    let planes = host.world().roster.planes().len();
    let mut players: Vec<(tore_net::sim::SimSocket, Bot)> = (0..bots)
        .map(|i| {
            let socket = net
                .bind(format!("10.0.1.{}:40000", i + 1).parse().unwrap())
                .unwrap();
            let config = ClientConfig {
                entropy: Entropy::Seeded(100 + i as u64),
                ..ClientConfig::new(address, &format!("Bot{}", i + 1), build())
            };
            let client = Client::connect(config, Arc::clone(&resources), net.now()).unwrap();
            (socket, Bot::new(client))
        })
        .collect();

    let cpu = CpuClock::new();
    let step = Duration::from_millis(1);
    // Join first: the mission flies from the start (`start now`), so the
    // clock below starts when the last bot is seated and the run counts
    // `seconds` of flying from there.
    let mut calls: Vec<u64> = Vec::new();
    let mut tick_calls: Vec<u64> = Vec::new();
    let mut host_ns: u64 = 0;
    let mut host_cpu_ns: u64 = 0;
    let mut seated_at: Option<Duration> = None;
    let mut ticks_before = 0u64;
    let mut per_minute: Vec<(u64, u64)> = Vec::new();
    let mut minute = (0u64, 0u64);
    let mut next_second = Duration::ZERO;
    let mut up: Vec<f64> = Vec::new();
    let mut down: Vec<f64> = Vec::new();
    let mut up_total: Vec<f64> = Vec::new();
    let mut down_total: Vec<f64> = Vec::new();
    let mut peak_up_total = 0f64;
    let wall = Instant::now();
    loop {
        net.advance(step);
        let now = net.now();
        let before = host.world().tick();
        let t0 = cpu.now();
        let elapsed_start = Instant::now();
        host.receive_from(now, &mut host_socket).unwrap();
        host.update(now);
        host.transmit(&mut host_socket).unwrap();
        let spent = elapsed_start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let cpu_spent = cpu.now().saturating_sub(t0);
        let ticked = host.world().tick() - before;
        for (socket, bot) in &mut players {
            bot.client.receive_from(now, socket).unwrap();
            bot.update(now);
            bot.client.transmit(socket).unwrap();
            while bot.client.poll_event().is_some() {}
        }
        if seated_at.is_none() {
            if players
                .iter()
                .all(|(_, bot)| bot.client.phase() == ClientPhase::Flying)
            {
                seated_at = Some(now);
                next_second = now + Duration::from_secs(1);
                ticks_before = host.world().tick();
                println!(
                    "{} bots seated at {:.1} s, tick {}",
                    bots,
                    now.as_secs_f64(),
                    ticks_before
                );
            }
            assert!(
                now < Duration::from_secs(60 + 2 * bots as u64),
                "the bots were not all seated in time"
            );
            continue;
        }
        let flying = now - seated_at.unwrap();
        host_ns += spent;
        host_cpu_ns += cpu_spent;
        calls.push(spent);
        if ticked > 0 {
            tick_calls.push(spent);
        }
        minute.0 += spent;
        minute.1 += ticked;
        if minute.1 >= 60 * 120 {
            per_minute.push(minute);
            minute = (0, 0);
        }
        if now >= next_second {
            next_second += Duration::from_secs(1);
            let status = host.players();
            let (mut u, mut d) = (0f64, 0f64);
            for p in &status {
                up.push(p.bytes_up_per_second as f64);
                down.push(p.bytes_down_per_second as f64);
                u += p.bytes_up_per_second as f64;
                d += p.bytes_down_per_second as f64;
            }
            up_total.push(u);
            down_total.push(d);
            peak_up_total = peak_up_total.max(u);
        }
        if flying >= Duration::from_secs(seconds) {
            break;
        }
    }
    let ticks = host.world().tick() - ticks_before;
    let logs: Vec<HostLog> = std::iter::from_fn(|| host.poll_log()).collect();
    let faults = logs
        .iter()
        .filter(|l| matches!(l, HostLog::Fault { .. }))
        .count();
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len().max(1) as f64;
    let max = |v: &[f64]| v.iter().copied().fold(0., f64::max);
    tick_calls.sort_unstable();
    let ms_per_tick = host_ns as f64 / ticks.max(1) as f64 / 1e6;
    println!(
        "{planes} planes, {bots} bots ({}), {snapshot_rate} snapshots a second, {seconds} s \
         flown, {ticks} ticks, elapsed clock, wall {:.0} s",
        if matches!(open, OpenPlanes::All) {
            "all planes open"
        } else {
            "friendly planes open"
        },
        wall.elapsed().as_secs_f64()
    );
    println!(
        "host elapsed: {:.3} ms a tick on average ({:.1}% of the 120 Hz wall-time budget); \
         ticking calls p50 {:.3} ms, p95 {:.3} ms, p99 {:.3} ms, p99.9 {:.3} ms, longest {:.3} ms",
        ms_per_tick,
        ms_per_tick * 120. / 10.,
        percentile(&tick_calls, 0.5),
        percentile(&tick_calls, 0.95),
        percentile(&tick_calls, 0.99),
        percentile(&tick_calls, 0.999),
        tick_calls.last().copied().unwrap_or(0) as f64 / 1e6
    );
    if cpu.is_cpu() {
        println!(
            "host thread CPU (excludes workers): {:.3} ms a tick",
            host_cpu_ns as f64 / ticks.max(1) as f64 / 1e6
        );
    }
    for (index, (ns, t)) in per_minute.iter().enumerate() {
        println!(
            "  minute {}: {:.3} ms a tick",
            index + 1,
            *ns as f64 / (*t).max(1) as f64 / 1e6
        );
    }
    println!(
        "host to players (one second windows): per player mean {:.0} B/s max {:.0}; in total \
         mean {:.0} B/s ({:.2} Mbit/s) peak {:.0} B/s",
        mean(&up),
        max(&up),
        mean(&up_total),
        mean(&up_total) * 8. / 1e6,
        peak_up_total
    );
    println!(
        "players to host: per player mean {:.0} B/s max {:.0}; in total mean {:.0} B/s",
        mean(&down),
        max(&down),
        mean(&down_total)
    );
    println!(
        "host log: {} entries, {} faults, {} overloads",
        logs.len(),
        faults,
        logs.iter()
            .filter(|l| matches!(l, HostLog::Overloaded { .. }))
            .count()
    );
    assert_eq!(faults, 0, "{logs:#?}");
}
