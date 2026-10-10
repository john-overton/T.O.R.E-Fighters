//! `--validate-ils`: the vertical and lateral alignment of the ILS at every
//! airport of the chosen theaters, measured against the runway surface plane
//! and the terrain, and a closed-loop check that an aircraft flown down the
//! ideal path reads zero on the glide and localizer bars with the right signs.
//! A developer probe; see `docs/testing/ils.md`.

use std::collections::BTreeMap;

use tore_sim::airport::{
    Aircraft, ApproachEnd, Command, GLIDE_SLOPE_DEGREES, Service, glide_path_height_ft,
};

use crate::{AppResult, terrain::Terrain};

/// Distances from the threshold (feet) at which the ideal path is checked.
const CHECK_RANGES_FT: [f64; 6] = [28_000., 15_000., 6_000., 2_500., 800., 300.];

/// One runway end's measured alignment.
#[derive(Debug, Clone, PartialEq)]
pub struct EndReport {
    pub theater: String,
    pub airport: u32,
    pub runway: String,
    pub end: ApproachEnd,
    /// ILS datum height at the threshold (the authored airport ground), feet.
    pub datum_ft: f64,
    /// Height of the runway support plane at the threshold, feet.
    pub support_ft: f64,
    /// Terrain height at the threshold, feet.
    pub terrain_ft: f64,
    /// Support plane height at the touchdown zone (1,000 ft in), feet.
    pub tdz_support_ft: f64,
    /// Height of the glide path above the runway surface over the threshold,
    /// feet (the aim point is in the touchdown zone, so this is about 52).
    pub crossing_height_ft: f64,
    /// Runway surface pitch and bank, degrees.
    pub pitch_deg: f64,
    pub bank_deg: f64,
    pub length_ft: f64,
}

impl EndReport {
    /// The ILS datum above (positive) or below (negative) the runway surface
    /// where the path meets the threshold, feet.
    pub fn datum_offset_ft(&self) -> f64 {
        self.datum_ft - self.support_ft
    }
}

/// Every runway end of a scene, measured for a glide path of `glide_deg`.
pub fn survey_world(theater: &str, world: &Terrain) -> Vec<EndReport> {
    let scene = &world.airport_scene;
    let mut out = Vec::new();
    for runway in &scene.runways {
        if scene.vertical_pad(runway.object) {
            continue;
        }
        let airport = runway.airport;
        for end in [ApproachEnd::Near, ApproachEnd::Far] {
            let t = runway.threshold(end);
            let heading = runway.approach_heading(end);
            let support = runway.support_height(t[0], t[2]).unwrap_or(f64::NAN);
            let tdz = [t[0] + heading.sin() * 1_000., t[2] + heading.cos() * 1_000.];
            let tdz_support = runway.support_height(tdz[0], tdz[1]).unwrap_or(f64::NAN);
            let aim = runway.aim_point(end);
            out.push(EndReport {
                theater: theater.to_owned(),
                airport,
                runway: runway.name.clone(),
                end,
                datum_ft: t[1],
                support_ft: support,
                terrain_ft: f64::from(world.height(t[0] as f32, t[2] as f32)),
                tdz_support_ft: tdz_support,
                crossing_height_ft: aim[1] + glide_path_height_ft(0.) - support,
                pitch_deg: runway.surface.pitch.to_degrees(),
                bank_deg: runway.surface.bank.to_degrees(),
                length_ft: runway.length_ft,
            });
        }
    }
    out
}

/// Fly the ideal path to every runway end and check the bars. Returns the
/// problems found.
pub fn check_path(theater: &str, world: &Terrain, clearance_ft: f64) -> Vec<String> {
    let scene = &world.airport_scene;
    let mut problems = Vec::new();
    for runway in &scene.runways {
        // A pad has no conventional approach, and the tower gives no ILS for
        // a short strip (John, 2026-09-30).
        if scene.vertical_pad(runway.object) || runway.short_strip() {
            continue;
        }
        for end in [ApproachEnd::Near, ApproachEnd::Far] {
            let t = runway.threshold(end);
            let aim = runway.aim_point(end);
            let heading = runway.approach_heading(end);
            let (s, c) = heading.sin_cos();
            let forward = [s, 0., c];
            let name = format!(
                "{theater} airport {} {} {end:?}",
                runway.airport, runway.name
            );
            for range in CHECK_RANGES_FT {
                let probe = |lateral: f64, height_offset: f64| {
                    let position = [
                        t[0] - s * range + c * lateral,
                        aim[1] + glide_path_height_ft(range) + clearance_ft + height_offset,
                        t[2] - c * range - s * lateral,
                    ];
                    let aircraft = Aircraft {
                        position,
                        forward,
                        nav_mode: true,
                        gear_down: true,
                        supported: false,
                        alive: true,
                        speed_fps: 250.,
                        ground_clearance_ft: clearance_ft,
                        redfor: false,
                    };
                    let mut service = Service::new(scene).ok()?;
                    service.command(scene, aircraft, Command::SelectAirport(runway.airport));
                    service.guidance(scene, aircraft)
                };
                let Some(on) = probe(0., 0.) else {
                    problems.push(format!("{name}: no guidance on the path at {range:.0} ft"));
                    continue;
                };
                if on.end != end || on.runway != runway.object {
                    problems.push(format!(
                        "{name}: guidance chose another runway end at {range:.0} ft"
                    ));
                    continue;
                }
                if on.glide_degrees.abs() > 0.01 || on.localizer_degrees.abs() > 0.01 {
                    problems.push(format!(
                        "{name}: on the ideal path at {range:.0} ft the bars read glide {:.3} localizer {:.3} degrees",
                        on.glide_degrees, on.localizer_degrees
                    ));
                }
                // 100 ft high reads positive; 100 ft low negative; 100 ft to
                // the right reads positive (the HUD bar moves left).
                // A displacement small enough to stay inside the 90 degree cone.
                let d = if range > 500. { 100. } else { 10. };
                let high = probe(0., d).map_or(0., |g| g.glide_degrees);
                let low = probe(0., -d).map_or(0., |g| g.glide_degrees);
                let right = probe(d, 0.).map_or(0., |g| g.localizer_degrees);
                let left = probe(-d, 0.).map_or(0., |g| g.localizer_degrees);
                if !(high > on.glide_degrees && low < on.glide_degrees) {
                    problems.push(format!(
                        "{name}: glide sign wrong at {range:.0} ft (high {high:.3}, low {low:.3})"
                    ));
                }
                if !(right > on.localizer_degrees && left < on.localizer_degrees) {
                    problems.push(format!("{name}: localizer sign wrong at {range:.0} ft (right {right:.3}, left {left:.3})"));
                }
            }
        }
    }
    problems
}

/// Where the ideal glide path (wheel height) meets terrain on its final 5 nm:
/// the largest depth of terrain above the path, feet, and its range. Scenery
/// fact, not an ILS fault, so it is reported and not failed.
pub fn terrain_under_path(
    world: &Terrain,
    runway: &tore_sim::airport::Runway,
    end: ApproachEnd,
) -> Option<(f64, f64)> {
    let aim = runway.aim_point(end);
    let heading = runway.approach_heading(end);
    let t = runway.threshold(end);
    let (s, c) = heading.sin_cos();
    let mut worst: Option<(f64, f64)> = None;
    let mut range = 0.;
    while range <= tore_sim::airport::ILS_RANGE_FT {
        let (x, z) = (t[0] - s * range, t[2] - c * range);
        let path = aim[1] + glide_path_height_ft(range);
        let cell = f64::from(tore_formats::theater::CELL_FEET);
        // Past the map edge the height is clamped, so there is nothing to hit.
        if x < 0.
            || z < 0.
            || x > (world.theater.cols - 1) as f64 * cell
            || z > (world.theater.rows - 1) as f64 * cell
        {
            range += 250.;
            continue;
        }
        let ground = f64::from(world.height(x as f32, z as f32));
        let depth = ground - path;
        if depth > 0. && worst.is_none_or(|(d, _)| depth > d) {
            worst = Some((depth, range));
        }
        range += 250.;
    }
    worst
}

/// Print the survey for `codes` and fail on any problem. `clearance_ft` is the
/// height of the aircraft origin above its wheels.
pub fn run(
    resources: &BTreeMap<String, Vec<u8>>,
    codes: &[String],
    clearance_ft: f64,
    max_offset_ft: f64,
) -> AppResult<()> {
    let mut problems = Vec::new();
    let mut ends = 0;
    for code in codes {
        let world = crate::scenery::launch_terrain(resources, code, None)?;
        let reports = survey_world(code, &world);
        let mut worst = 0_f64;
        for r in &reports {
            ends += 1;
            let offset = r.datum_offset_ft();
            worst = worst.max(offset.abs());
            println!(
                "ils: theater={} airport={} runway={:?} end={:?} length_ft={:.0} datum_ft={:.1} support_ft={:.1} terrain_ft={:.1} aim_offset_ft={:+.1} tdz_support_ft={:.1} tdz_offset_ft={:+.1} crossing_ft={:.1} pitch_deg={:.2} bank_deg={:.2}",
                r.theater,
                r.airport,
                r.runway,
                r.end,
                r.length_ft,
                r.datum_ft,
                r.support_ft,
                r.terrain_ft,
                offset,
                r.tdz_support_ft,
                r.datum_ft - r.tdz_support_ft,
                r.crossing_height_ft,
                r.pitch_deg,
                r.bank_deg
            );
            if !offset.is_finite() || offset.abs() > max_offset_ft {
                problems.push(format!(
                    "{} airport {} {} {:?}: ILS datum is {offset:+.1} ft from the runway surface at the threshold",
                    r.theater, r.airport, r.runway, r.end
                ));
            }
        }
        println!(
            "ils-theater: {code} runway_ends={} worst_aim_offset_ft={worst:.1}",
            reports.len()
        );
        problems.extend(check_path(code, &world, clearance_ft));
        for runway in &world.airport_scene.runways {
            for end in [ApproachEnd::Near, ApproachEnd::Far] {
                if let Some((depth, range)) = terrain_under_path(&world, runway, end) {
                    println!(
                        "ils-terrain: theater={code} airport={} runway={:?} end={end:?} terrain_above_path_ft={depth:.0} at_range_ft={range:.0}",
                        runway.airport, runway.name
                    );
                }
            }
        }
    }
    println!(
        "ils-survey: {} theaters, {ends} runway ends, glide {GLIDE_SLOPE_DEGREES} degrees, {} problems",
        codes.len(),
        problems.len()
    );
    for p in &problems {
        println!("  PROBLEM {p}");
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err("ILS alignment problems".into())
    }
}

/// What the ILS read along a flown approach: how well the flown path followed
/// the bars, and the wheels' height over the last guidance sample.
#[derive(Debug, Default)]
pub struct PathRecord {
    samples: u64,
    active: u64,
    max_abs_glide: f64,
    max_abs_localizer: f64,
    last: Option<(f64, f64, f64, f64)>,
}

impl PathRecord {
    /// One tick: the guidance (if any) and the wheels' height above the
    /// surface underneath.
    pub fn observe(&mut self, guidance: Option<tore_sim::airport::Guidance>, wheel_height_ft: f64) {
        self.samples += 1;
        let Some(g) = guidance else { return };
        self.active += 1;
        // The last 1,500 ft before the threshold are the flare: only the
        // approach proper counts toward the worst deviation.
        if g.range_ft > 1_500. {
            self.max_abs_glide = self.max_abs_glide.max(g.glide_degrees.abs());
            self.max_abs_localizer = self.max_abs_localizer.max(g.localizer_degrees.abs());
        }
        self.last = Some((
            g.range_ft,
            g.glide_degrees,
            g.localizer_degrees,
            wheel_height_ft,
        ));
    }
    pub fn report(&self) -> String {
        match self.last {
            None => format!("ils_probe: samples={} active=0", self.samples),
            Some((range, glide, loc, height)) => format!(
                "ils_probe: samples={} active={} max_abs_glide_deg={:.3} max_abs_localizer_deg={:.3} last_range_ft={range:.0} last_glide_deg={glide:.3} last_localizer_deg={loc:.3} last_wheel_height_ft={height:.1}",
                self.samples, self.active, self.max_abs_glide, self.max_abs_localizer
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_path_record_ignores_the_flare_and_reports_the_last_sample() {
        let mut record = PathRecord::default();
        assert_eq!(record.report(), "ils_probe: samples=0 active=0");
        let guidance = |range_ft, glide_degrees| {
            Some(tore_sim::airport::Guidance {
                airport: 1,
                runway: 1,
                end: ApproachEnd::Near,
                threshold: [0.; 3],
                range_ft,
                bearing: 0.,
                localizer_degrees: 0.1,
                glide_degrees,
                localizer_normalized: 0.,
                glide_normalized: 0.,
                active: true,
            })
        };
        record.observe(guidance(5_000., -0.3), 250.);
        record.observe(guidance(200., 3.0), 40.);
        record.observe(None, 0.);
        let text = record.report();
        assert!(
            text.contains("samples=3 active=2 max_abs_glide_deg=0.300"),
            "{text}"
        );
        assert!(
            text.contains("last_range_ft=200 last_glide_deg=3.000"),
            "{text}"
        );
    }
}
