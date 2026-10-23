//! Opt-in, bounded desktop frame measurements. No GPU timestamp/FPS claim.
use std::time::Instant;

#[derive(Default)]
pub struct Performance {
    limit: usize,
    tick_limit: usize,
    total_ticks: usize,
    network_tick: Option<u64>,
    locked_frames: usize,
    locked_weapon: Option<String>,
    cycle: bool,
    active: bool,
    frames: usize,
    previous: Option<Instant>,
    samples: Vec<[f64; 4]>,
    /// Whether the G vignette was forced on for each sample (`TORE_PERF_VEIL`).
    veiled: Vec<bool>,
    veil: bool,
    pub completed_previews: usize,
    paused_frames: usize,
    /// Simulation ticks the current frame ran, and the time compression asked
    /// for, noted by [`Performance::ticks`] before the frame is recorded.
    pending_ticks: usize,
    requested_scale: f64,
    /// Ticks run in each recorded sample's frame and the time compression
    /// asked for then, beside `samples`.
    ticks_run: Vec<(usize, f64)>,
}
impl Performance {
    pub fn from_env() -> crate::AppResult<Self> {
        let tick_limit = match std::env::var("TORE_PERF_TICKS") {
            Ok(s) => s.parse::<usize>()?,
            Err(_) => 0,
        };
        if tick_limit != 0 && !(120..=1_000_000).contains(&tick_limit) {
            return Err("TORE_PERF_TICKS must be 120..1000000".into());
        }
        let limit = match std::env::var("TORE_PERF_FRAMES") {
            Ok(s) => s.parse::<usize>()?,
            Err(_) => {
                if tick_limit > 0 {
                    100_000
                } else {
                    0
                }
            }
        };
        if limit != 0 && !(60..=100_000).contains(&limit) {
            return Err("TORE_PERF_FRAMES must be 60..100000".into());
        }
        Ok(Self {
            limit,
            tick_limit,
            cycle: std::env::var_os("TORE_PERF_VIEWS").is_some(),
            active: std::env::var_os("TORE_PERF_ACTIVE").is_some(),
            veil: std::env::var_os("TORE_PERF_VEIL").is_some(),
            ..Default::default()
        })
    }
    pub fn active(&self) -> bool {
        self.limit > 0 && self.active
    }
    pub fn measuring(&self) -> bool {
        self.limit > 0
    }
    /// Evidence that a timed lock case reached a designated weapon estimate.
    pub fn weapon(&mut self, source: &str, designated: bool, estimate: bool) {
        if self.measuring() && designated && estimate {
            self.locked_frames += 1;
            self.locked_weapon.get_or_insert_with(|| source.to_owned());
        }
    }
    pub fn view(&self) -> Option<u8> {
        (self.limit > 0 && self.cycle).then(|| [0, 3, 4, 1, 2][(self.frames / 30) % 5])
    }
    /// Blackout level to force on this frame: 30-frame blocks alternate off
    /// and on, so the report can compare the same run with and without the veil.
    pub fn veil_level(&self) -> Option<f64> {
        (self.limit > 0 && self.veil).then_some(if (self.frames / 30) % 2 == 1 { 0.6 } else { 0. })
    }
    /// Notes how many 120 Hz ticks this frame ran, and the time compression
    /// the player asked for, so the report can say what rate was reached.
    pub fn ticks(&mut self, run: usize, requested_scale: f64) {
        if self.requested_scale != requested_scale {
            self.locked_frames = 0;
            self.locked_weapon = None;
        }
        self.pending_ticks = run;
        self.total_ticks = self.total_ticks.saturating_add(run);
        self.requested_scale = requested_scale;
    }
    /// Network flights draw the client's advancing tick, not the local world's
    /// inactive clock. This is measurement only and does not schedule any tick.
    pub fn network_tick(&mut self, tick: u64) {
        let previous = self.network_tick.replace(tick).unwrap_or(tick);
        let advanced = usize::try_from(tick.saturating_sub(previous)).unwrap_or(usize::MAX);
        self.ticks(advanced, 1.);
    }
    /// What the recorded frames simulated: the ticks they ran, the wall time
    /// they covered and the rate of mission time that reached, beside the one
    /// asked for. It covers the frames since the asked-for rate last changed,
    /// so a run that starts at 1x and steps up to 8x reports the 8x. A frame's
    /// ticks cover the interval since the previous frame began, which is what
    /// each sample's interval measures. `None` when no tick ran.
    fn simulated_line(&self) -> Option<String> {
        let asked = self.ticks_run.last()?.1;
        let from = self.last_scale_from();
        let frames = &self.ticks_run[from..];
        let wall: f64 = self.samples[from..].iter().map(|s| s[0]).sum::<f64>() / 1000.;
        let ticks: usize = frames.iter().map(|t| t.0).sum();
        (ticks > 0 && wall > 0.).then(|| {
            format!(
                "simulated: {ticks} ticks in {wall:.2} s, {:.2}x real time (asked for {asked}x); ticks per frame mean {:.1}, max {}",
                ticks as f64 / 120. / wall,
                ticks as f64 / frames.len() as f64,
                frames.iter().map(|t| t.0).max().unwrap_or(0)
            )
        })
    }
    /// Frame statistics and achieved rate describe the same final-speed window.
    fn last_scale_from(&self) -> usize {
        let Some((_, asked)) = self.ticks_run.last() else {
            return 0;
        };
        self.ticks_run
            .iter()
            .rposition(|t| t.1 != *asked)
            .map_or(0, |i| i + 1)
    }
    pub fn record(
        &mut self,
        start: Instant,
        simulation: f64,
        compose: f64,
        present: f64,
        paused: bool,
    ) -> bool {
        if self.limit == 0 {
            return false;
        }
        let warmup = if self.tick_limit > 0 { 0 } else { 30 };
        if let Some(previous) = self.previous
            && self.frames >= warmup
        {
            self.samples.push([
                (start - previous).as_secs_f64() * 1000.,
                simulation,
                compose,
                present,
            ]);
            self.veiled.push(self.veil_level().is_some_and(|l| l > 0.));
            self.ticks_run
                .push((self.pending_ticks, self.requested_scale));
        }
        self.pending_ticks = 0;
        self.paused_frames += usize::from(paused);
        self.previous = Some(start);
        self.frames += 1;
        if self.frames < self.limit && (self.tick_limit == 0 || self.total_ticks < self.tick_limit)
        {
            return false;
        }
        println!(
            "Performance: {} frames, first {warmup} excluded; CPU wall times (presentation includes backpressure)",
            self.frames,
        );
        println!(
            "  paused frames: {}; completed camera readbacks: {}",
            self.paused_frames, self.completed_previews
        );
        println!(
            "  workload: {} ticks (target {}); designated estimate frames: {}; weapon: {}",
            self.total_ticks,
            self.tick_limit,
            self.locked_frames,
            self.locked_weapon.as_deref().unwrap_or("none")
        );
        if let Some(line) = self.simulated_line() {
            println!("  {line}");
        }
        let groups: &[(&str, Option<bool>)] = if self.veil {
            &[("veil off", Some(false)), ("veil on", Some(true))]
        } else {
            &[("", None)]
        };
        for (group, want) in groups {
            let rows: Vec<&[f64; 4]> = self
                .samples
                .iter()
                .zip(&self.veiled)
                .skip(self.last_scale_from())
                .filter(|(_, v)| want.is_none_or(|w| **v == w))
                .map(|(s, _)| s)
                .collect();
            if rows.is_empty() {
                continue;
            }
            println!("  [{group}] {} samples", rows.len());
            for (column, name) in [
                "frame interval",
                "simulation/cameras",
                "UI composition",
                "submit/present",
            ]
            .iter()
            .enumerate()
            {
                let mut values: Vec<_> = rows.iter().map(|s| s[column]).collect();
                values.sort_by(f64::total_cmp);
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                println!(
                    "  {name}: mean {mean:.2} ms, p50 {:.2}, p95 {:.2}, p99 {:.2}, max {:.2}",
                    values[values.len() / 2],
                    values[(values.len() - 1) * 95 / 100],
                    values[(values.len() - 1) * 99 / 100],
                    values[values.len() - 1]
                );
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn network_workloads_count_client_progress_and_tolerate_restart() {
        let mut perf = Performance::default();
        for tick in [100, 101, 101, 108, 0, 2] {
            perf.network_tick(tick);
        }
        assert_eq!(perf.total_ticks, 10);
        assert_eq!(perf.pending_ticks, 2);
        assert_eq!(perf.requested_scale, 1.);
    }
    #[test]
    fn a_fixed_tick_workload_finishes_even_when_frame_rates_differ() {
        for ticks_per_frame in [1, 3, 16, 240] {
            let mut perf = Performance {
                limit: 100_000,
                tick_limit: 1200,
                ..Default::default()
            };
            let start = Instant::now();
            let frames = 1200_usize.div_ceil(ticks_per_frame);
            for frame in 0..frames {
                perf.ticks(ticks_per_frame, 8.);
                assert_eq!(
                    perf.record(
                        start + std::time::Duration::from_millis(frame as u64),
                        1.,
                        2.,
                        3.,
                        false
                    ),
                    frame + 1 == frames
                );
            }
            assert!(perf.total_ticks >= 1200 && perf.total_ticks < 1200 + ticks_per_frame);
        }
        let mut perf = Performance {
            limit: 60,
            ..Default::default()
        };
        perf.weapon("SYNTHETIC.JT", false, true);
        perf.weapon("SYNTHETIC.JT", true, false);
        assert_eq!(perf.locked_frames, 0);
        perf.weapon("SYNTHETIC.JT", true, true);
        assert_eq!(perf.locked_frames, 1);
    }
    #[test]
    fn bounded_measurement_excludes_warmup_and_cycles_views() {
        let mut perf = Performance {
            limit: 60,
            cycle: true,
            ..Default::default()
        };
        let start = Instant::now();
        for frame in 0..60 {
            assert_eq!(perf.view(), Some(if frame < 30 { 0 } else { 3 }));
            perf.ticks(2, 1.);
            assert_eq!(
                perf.record(
                    start + std::time::Duration::from_millis(frame * 16),
                    1.,
                    2.,
                    3.,
                    false
                ),
                frame == 59
            );
        }
        assert_eq!(perf.samples.len(), 30);
        // The warm-up frames' ticks are left out with their samples.
        assert_eq!(perf.ticks_run, vec![(2, 1.); 30]);
        // 30 frames of 16 ms ran 60 ticks: 0.5 s of mission in 0.48 s.
        assert_eq!(
            perf.simulated_line().as_deref(),
            Some(
                "simulated: 60 ticks in 0.48 s, 1.04x real time (asked for 1x); ticks per frame mean 2.0, max 2"
            )
        );
        assert!(perf.samples.iter().all(|s| *s == [16., 1., 2., 3.]));
        assert_eq!(perf.paused_frames, 0);
    }

    #[test]
    fn the_simulated_line_covers_the_frames_since_the_asked_for_rate_changed() {
        let mut perf = Performance {
            limit: 100,
            ..Default::default()
        };
        let start = Instant::now();
        for frame in 0..70 {
            let (ticks, asked) = if frame < 50 { (2, 1.) } else { (16, 8.) };
            perf.ticks(ticks, asked);
            perf.record(
                start + std::time::Duration::from_millis(frame * 16),
                1.,
                2.,
                3.,
                false,
            );
        }
        // The last 20 recorded frames ran at 8x: 320 ticks, 2.67 s in 0.32 s.
        assert_eq!(perf.last_scale_from(), 20);
        assert_eq!(
            perf.simulated_line().as_deref(),
            Some(
                "simulated: 320 ticks in 0.32 s, 8.33x real time (asked for 8x); ticks per frame mean 16.0, max 16"
            )
        );
        // A run that ran no tick has nothing to report.
        assert_eq!(Performance::default().simulated_line(), None);
    }
}
