//! Opt-in, bounded desktop frame measurements. No GPU timestamp/FPS claim.
use std::time::Instant;

#[derive(Default)]
pub struct Performance {
    limit: usize,
    cycle: bool,
    active: bool,
    frames: usize,
    previous: Option<Instant>,
    samples: Vec<[f64; 4]>,
    pub completed_previews: usize,
    paused_frames: usize,
}
impl Performance {
    pub fn from_env() -> crate::AppResult<Self> {
        let limit = match std::env::var("TORE_PERF_FRAMES") {
            Ok(s) => s.parse::<usize>()?,
            Err(_) => 0,
        };
        if limit != 0 && !(60..=100_000).contains(&limit) {
            return Err("TORE_PERF_FRAMES must be 60..100000".into());
        }
        Ok(Self {
            limit,
            cycle: std::env::var_os("TORE_PERF_VIEWS").is_some(),
            active: std::env::var_os("TORE_PERF_ACTIVE").is_some(),
            ..Default::default()
        })
    }
    pub fn active(&self) -> bool {
        self.limit > 0 && self.active
    }
    pub fn view(&self) -> Option<u8> {
        (self.limit > 0 && self.cycle).then(|| [0, 3, 4, 1, 2][(self.frames / 30) % 5])
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
        if let Some(previous) = self.previous
            && self.frames >= 30
        {
            self.samples.push([
                (start - previous).as_secs_f64() * 1000.,
                simulation,
                compose,
                present,
            ]);
        }
        self.paused_frames += usize::from(paused);
        self.previous = Some(start);
        self.frames += 1;
        if self.frames < self.limit {
            return false;
        }
        println!(
            "Performance: {} frames, first 30 excluded; CPU wall times (presentation includes backpressure)",
            self.frames
        );
        println!(
            "  paused frames: {}; completed camera readbacks: {}",
            self.paused_frames, self.completed_previews
        );
        for (column, name) in [
            "frame interval",
            "simulation/cameras",
            "UI composition",
            "submit/present",
        ]
        .iter()
        .enumerate()
        {
            let mut values: Vec<_> = self.samples.iter().map(|s| s[column]).collect();
            values.sort_by(f64::total_cmp);
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            println!(
                "  {name}: mean {mean:.2} ms, p50 {:.2}, p95 {:.2}, max {:.2}",
                values[values.len() / 2],
                values[(values.len() - 1) * 95 / 100],
                values[values.len() - 1]
            );
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(perf.samples.iter().all(|s| *s == [16., 1., 2., 3.]));
        assert_eq!(perf.paused_frames, 0);
    }
}
