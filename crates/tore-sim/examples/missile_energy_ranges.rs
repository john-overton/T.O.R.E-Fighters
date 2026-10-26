//! Offline physical envelopes and public-anchor calibration. Runtime never calls this search.
use std::{fs, path::Path};
use tore_formats::weapons::Weapon;
use tore_sim::{
    attitude::{Basis, dot},
    combat::{
        live::{self, ActorSupport, Launcher, Projectile, Target},
        missiles::{self, energy::FT, energy_estimate::Trial, prediction},
    },
};
fn question(w: &Weapon, alt: f64, aspect: usize, range: f64) -> Trial {
    let surface = missiles::surface_launched(&w.source);
    let strike = w.flags & 0x20000 != 0;
    let command = tore_sim::combat::guided::kind(w) == Some(missiles::Guidance::Command);
    let speed = if surface {
        0.
    } else if command {
        30.
    } else {
        300.
    };
    let launch_alt = if surface { 10. } else { alt };
    let target_alt = if strike { 10. } else { alt };
    let delta = [0., target_alt - launch_alt, range];
    let pitch = delta[1].atan2(range);
    let l = Launcher {
        position: [0., launch_alt / FT, 0.],
        basis: Basis::new(0., pitch, 0.),
        velocity: [0., 0., speed / FT],
        speed_fps: speed / FT,
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: Default::default(),
    };
    let velocity = if strike {
        [0.; 3]
    } else {
        match aspect {
            3 => [0.; 3],
            0 => [0., 0., -250.],
            1 => [250., 0., 0.],
            _ => [0., 0., 250.],
        }
    };
    let o = missiles::seeker::Observation {
        id: 1,
        position: [0., target_alt / FT, range / FT],
        velocity: velocity.map(|v| v / FT),
        range: range / FT,
        quality: 1.,
        off_axis: 0.,
    };
    prediction::launch_question(
        w,
        l,
        o,
        if tore_sim::combat::guided::kind(w).is_some() {
            tore_sim::combat::guided::HIT_FT
        } else {
            (f64::from(w.damage.fuze_radius.max(0))) + 30.
        },
        [0.; 3],
    )
}
fn hit(w: &Weapon, alt: f64, aspect: usize, range: f64, drag: Option<f64>) -> bool {
    let mut q = question(w, alt, aspect, range);
    if let Some(d) = drag {
        q.body.drag_scale = d;
    }
    while q.answer.is_none() {
        q.step_over(&|_, _| 0., &|a, b| a[1] > 0. && b[1] > 0.);
    }
    q.answer.unwrap().hit
}
fn seeker_hit(w: &Weapon, alt: f64, aspect: usize, range: f64, drag: Option<f64>) -> bool {
    let Some(profile) = missiles::Profile::for_weapon(w) else {
        return false;
    };
    let q = question(w, alt, aspect, range);
    let start = q.flight.position.map(|v| v / FT);
    let heading = if aspect == 0 {
        std::f64::consts::PI
    } else if aspect == 1 {
        std::f64::consts::FRAC_PI_2
    } else {
        0.
    };
    let mut target = Target {
        aircraft: Some(tore_formats::aircraft::AircraftId::Mig29),
        role: profile.role,
        heat: missiles::seeker::Heat::Engine {
            on: true,
            throttle: 1.,
            afterburner: false,
        },
        radar_emitting: true,
        id: 1,
        position: q.target.position.map(|v| v / FT),
        velocity: q.target.velocity.map(|v| v / FT),
        basis: Basis::new(heading, 0., 0.),
        configuration: Default::default(),
        signature: Default::default(),
        jammer: None,
        jammer_active: false,
        airborne: true,
        on_ground: false,
        wreck: None,
        wreck_power: Default::default(),
        radius: 30.,
        hp: 100,
        initial_hp: 100,
        fragment_offsets: [[0.; 3]; 2],
        fragment_released: false,
        localized_damage: Default::default(),
        faults: Default::default(),
        category: 0x80,
        side: live::Side(2),
    };
    let observation = |t: &Target, p: [f64; 3]| missiles::seeker::Observation {
        id: t.id,
        position: t.position,
        velocity: t.velocity,
        quality: 1.,
        range: dot(
            std::array::from_fn::<_, 3, _>(|i| t.position[i] - p[i]),
            std::array::from_fn::<_, 3, _>(|i| t.position[i] - p[i]),
        )
        .sqrt(),
        off_axis: 0.,
    };
    if profile.guidance == missiles::Guidance::Infrared {
        let view = missiles::seeker::View {
            position: start,
            basis: Basis::new(
                0.,
                (q.target.position[1] - q.flight.position[1]).atan2(range),
                0.,
            ),
            cap: None,
            obscured: &|_, _| false,
        };
        if missiles::seeker::observe_energy(w, profile, &view, &target)
            .is_none_or(|o| o.quality < 0.25)
        {
            return false;
        }
    }
    let mut motion = missiles::Motion::launch_rules(
        w,
        [
            0.,
            0.,
            if missiles::surface_launched(&w.source) {
                0.
            } else {
                300. / FT
            },
        ],
        start[1],
        missiles::Rules::Energy,
    );
    let energy = motion.energy.as_mut().unwrap();
    energy.body = q.body;
    energy.flight = q.flight;
    if let Some(d) = drag {
        energy.body.drag_scale = d;
    }
    let mut guidance = missiles::Flight::from_supported_launch(
        profile,
        missiles::LaunchMode::Cued,
        observation(&target, start),
        start,
    );
    if profile.guidance == missiles::Guidance::Infrared {
        guidance.seeker.acquired = true;
        guidance.seeker.dwell = missiles::DWELL;
    }
    let mut p = Projectile {
        id: 1,
        owner: 0,
        weapon: None,
        guidance: Some(guidance),
        motion: Some(motion),
        guidance_ticks: None,
        age: 0,
        incoming: None,
        station: 0,
        position: start,
        previous: start,
        direction: q.flight.axis,
        speed_f8: 0,
        launched_t: 0,
        target: Some(1),
        fall: Default::default(),
        gun_round: None,
        tracer: false,
    };
    let radius = q.radius / FT;
    for age in 0..q.body.life {
        p.age = u64::from(age);
        p.previous = p.position;
        p.motion
            .as_mut()
            .unwrap()
            .prepare(p.position, p.direction, p.age, [0.; 3]);
        let support = ActorSupport {
            owner: 0,
            observation: Some(observation(&target, start)),
            supported: true,
            radar_position: start,
            radar_emitting: true,
        };
        live::guide_owned(
            &mut p,
            w,
            std::slice::from_ref(&target),
            std::iter::empty(),
            Some(&support),
            &|_, _| false,
            &|_, _| 0.,
        );
        let before = std::array::from_fn::<_, 3, _>(|i| target.position[i] - p.position[i]);
        let m = p.motion.as_mut().unwrap();
        let delta = m.step_over_terrain(&w.movement, p.age, p.direction, &|_, _| 0.);
        for (i, d) in delta.into_iter().enumerate() {
            p.position[i] += d;
            target.position[i] += target.velocity[i] / 120.;
        }
        p.direction = m.energy.unwrap().flight.axis;
        let after = std::array::from_fn::<_, 3, _>(|i| target.position[i] - p.position[i]);
        let d = std::array::from_fn::<_, 3, _>(|i| after[i] - before[i]);
        let u = (-dot(before, d) / dot(d, d).max(1e-12)).clamp(0., 1.);
        let gap = std::array::from_fn::<_, 3, _>(|i| before[i] + d[i] * u);
        if dot(gap, gap) <= radius * radius && age >= q.arming_age {
            return true;
        }
        if p.position[1] <= 0. || m.energy.unwrap().flight.lost {
            return false;
        }
    }
    false
}
fn maximum(w: &Weapon, alt: f64, aspect: usize, drag: Option<f64>, seeker: bool) -> f64 {
    let hit = |w, alt, aspect, range, drag| {
        if seeker {
            seeker_hit(w, alt, aspect, range, drag)
        } else {
            hit(w, alt, aspect, range, drag)
        }
    };
    let mut low = 0.;
    let mut high = 100.;
    let mut upper: f64 = 1_000_000.;
    let mut distances = Vec::new();
    let body = missiles::energy_profile::body(w);
    if body.loft_height > 0. {
        distances.extend(
            prediction::envelope_probe_scales(body)
                .iter()
                .map(|scale| body.loft_range / 0.6 * scale)
                .filter(|d| *d < 1_000_000.),
        );
    }
    // Search every increasing range, including minimum-range failures.
    while high < 1_000_000. {
        distances.push(high);
        high *= 1.5;
    }
    distances.sort_by(f64::total_cmp);
    distances.dedup();
    for distance in distances {
        if hit(w, alt, aspect, distance, drag) {
            low = distance;
            upper = 1_000_000.;
        } else if distance > low {
            upper = upper.min(distance);
        }
    }
    if low == 0. {
        return 0.;
    }
    high = upper;
    for _ in 0..10 {
        let mid = (low + high) * 0.5;
        if hit(w, alt, aspect, mid, drag) {
            low = mid
        } else {
            high = mid
        }
    }
    low
}
fn candidate_hit(
    w: &Weapon,
    body: missiles::energy::Body,
    anchor: [f64; 4],
    distance: f64,
) -> bool {
    let mut q = question(w, anchor[0] * FT, anchor[1] as usize, distance);
    q.body = body;
    q.target.velocity = match anchor[1] as usize {
        0 => [0., 0., -anchor[3]],
        1 => [anchor[3], 0., 0.],
        2 => [0., 0., anchor[3]],
        _ => [0.; 3],
    };
    while q.answer.is_none() {
        q.step_over(&|_, _| 0., &|a, b| a[1] > 0. && b[1] > 0.);
    }
    q.answer.unwrap().hit
}
fn candidate_maximum(w: &Weapon, body: missiles::energy::Body, anchor: [f64; 4]) -> f64 {
    let mut low = 0.;
    let mut high: f64 = 1_000_000.;
    let mut distances = Vec::new();
    let mut d = 100.;
    while d < 1_000_000. {
        distances.push(d);
        d *= 1.5;
    }
    if body.loft_height > 0. {
        distances.extend(
            prediction::envelope_probe_scales(body)
                .iter()
                .map(|scale| body.loft_range / 0.6 * scale),
        );
    }
    distances.sort_by(f64::total_cmp);
    distances.dedup();
    for d in distances {
        if candidate_hit(w, body, anchor, d) {
            low = d;
            high = 1_000_000.;
        } else if d > low {
            high = high.min(d);
        }
    }
    if low == 0. {
        return 0.;
    }
    for _ in 0..12 {
        let mid = (low + high) * 0.5;
        if candidate_hit(w, body, anchor, mid) {
            low = mid;
        } else {
            high = mid;
        }
    }
    low / 1000.
}
fn fit_pairs(dir: &Path, filter: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "source,fraction,drag_scale,guidance_seconds,primary_target_km,primary_km,secondary_target_km,secondary_km,tail5_km,smooth_relative"
    );
    for motor in tore_formats::missile_motors::MOTORS {
        if filter.is_some_and(|s| !s.split(',').any(|name| name == motor.source)) {
            continue;
        }
        let w = Weapon::parse(motor.source, &fs::read(dir.join(motor.source))?)?;
        let original = missiles::energy_profile::body(&w);
        let air = !missiles::surface_launched(&w.source) && w.flags & 0x20000 == 0;
        let fractions: Vec<_> = if motor.thrust.is_some() || !motor.fit_propellant {
            vec![motor.propellant_fraction]
        } else {
            (6..=25).map(|n| n as f64 * 0.02).collect()
        };
        let tail_anchor = [5000., 2., 0., 250.];
        let tail_floor = (motor.primary[2] * 0.08).clamp(2., 6.);
        let mut best = None;
        for fraction in fractions {
            let drags: &[f64] = if matches!(motor.source, "SSN9.JT" | "AS16.JT") {
                &[0.25, 0.3, 0.4, 0.5, 0.75, 1., 1.5, 2., 2.5, 3.]
            } else {
                &[0.5, 0.75, 1., 1.5, 2., 2.5, 3.]
            };
            for &drag in drags {
                let mut body = original;
                if motor.thrust.is_none() {
                    let ratio = fraction / motor.propellant_fraction;
                    body.boost.propellant *= ratio;
                    body.boost.thrust *= ratio;
                    body.sustain.propellant *= ratio;
                    body.sustain.thrust *= ratio;
                }
                body.drag_scale = drag;
                body.guidance_ticks = body.life;
                let mut q = question(
                    &w,
                    motor.primary[0] * FT,
                    motor.primary[1] as usize,
                    motor.primary[2] * 1000.,
                );
                q.body = body;
                q.target.velocity = match motor.primary[1] as usize {
                    0 => [0., 0., -motor.primary[3]],
                    2 => [0., 0., motor.primary[3]],
                    _ => [0.; 3],
                };
                while q.answer.is_none() {
                    q.step_over(&|_, _| 0., &|a, b| a[1] > 0. && b[1] > 0.);
                }
                let answer = q.answer.unwrap();
                if !answer.hit {
                    continue;
                }
                // Class-derived controlled life: rounded-up intercept time at
                // the documented anchor, with the selected physical motor.
                body.guidance_ticks = if motor.guidance_evidence.starts_with("NAVWEPS") {
                    original.guidance_ticks
                } else {
                    (answer.seconds * 120.).ceil() as u32 + 1
                };
                let secondary = candidate_maximum(&w, body, motor.secondary);
                let tail = if air {
                    candidate_maximum(&w, body, tail_anchor)
                } else {
                    0.
                };
                let penalty = 0.15 * drag.ln().powi(2) + 0.03 * ((fraction - 0.35) / 0.15).powi(2);
                let primary_error = if motor.guidance_evidence.starts_with("NAVWEPS") {
                    (candidate_maximum(&w, body, motor.primary) / motor.primary[2] - 1.).abs()
                } else {
                    0.
                };
                let score = if drag < 0.5 {
                    1. + (0.5 / drag).ln().powi(2)
                } else {
                    0.
                } + 4. * primary_error
                    + 4. * (secondary / motor.secondary[2] - 1.).abs()
                    + penalty
                    + if air {
                        10. * (1. - tail / tail_floor).max(0.)
                    } else {
                        0.
                    };
                if best.is_none_or(|(_, _, _, _, old)| score < old) {
                    best = Some((fraction, body, secondary, tail, score));
                }
            }
        }
        let (fraction, body, secondary, tail, _) =
            best.ok_or_else(|| format!("{}: no feasible primary", motor.source))?;
        let primary = candidate_maximum(&w, body, motor.primary);
        let mut smooth: f64 = 0.;
        for ratio in [0.98, 1.02] {
            for field in 0..3 {
                let mut nearby = body;
                match field {
                    0 => {
                        nearby.boost.propellant *= ratio;
                        nearby.boost.thrust *= ratio;
                        nearby.sustain.propellant *= ratio;
                        nearby.sustain.thrust *= ratio;
                    }
                    1 => nearby.drag_scale *= ratio,
                    _ => {
                        nearby.guidance_ticks =
                            (f64::from(nearby.guidance_ticks) * ratio).round() as u32
                    }
                }
                for (anchor, base) in [(motor.primary, primary), (motor.secondary, secondary)] {
                    let reach = candidate_maximum(&w, nearby, anchor);
                    smooth = smooth.max((reach / base - 1.).abs());
                }
            }
        }
        println!(
            "{},{fraction:.9},{:.8},{:.6},{},{primary:.3},{},{secondary:.3},{tail:.3},{smooth:.5}",
            motor.source,
            body.drag_scale,
            f64::from(body.guidance_ticks) / 120.,
            motor.primary[2],
            motor.secondary[2]
        );
    }
    Ok(())
}

fn verify(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("source,guidance_seconds,drag_scale,tail5_km,tail15_km,tail35_km,smooth_relative");
    let mut failures = Vec::new();
    let mut tails = std::collections::BTreeMap::new();
    for m in tore_formats::missile_motors::MOTORS {
        let w = Weapon::parse(m.source, &fs::read(dir.join(m.source))?)?;
        let body = missiles::energy_profile::body(&w);
        if !(0.5..=3.).contains(&body.drag_scale) && !matches!(m.source, "SSN9.JT" | "AS16.JT") {
            failures.push(format!("{}: unapproved drag exception", m.source));
        }
        if missiles::surface_launched(&w.source) || w.flags & 0x20000 != 0 {
            continue;
        }
        let anchors = [
            [5000., 2., 0., 250.],
            [15000., 2., 0., 250.],
            [35000., 2., 0., 250.],
        ];
        let reach = anchors.map(|a| candidate_maximum(&w, body, a));
        if reach[0] < 2. || reach[0] >= reach[1] || reach[1] >= reach[2] {
            failures.push(format!(
                "{}: inadequate or reversed tail reach {reach:?}",
                m.source
            ));
        }
        tails.insert(m.source, reach);
        let primary = candidate_maximum(&w, body, m.primary);
        if candidate_hit(&w, body, m.primary, m.primary[2] * 1500.) {
            failures.push(format!("{}: 150% head-on anchor still hits", m.source));
        }
        let mut smooth: f64 = 0.;
        for ratio in [0.98, 1.02] {
            for field in 0..4 {
                let mut nearby = body;
                match field {
                    0 => {
                        nearby.boost.propellant *= ratio;
                        nearby.boost.thrust *= ratio;
                        nearby.sustain.propellant *= ratio;
                        nearby.sustain.thrust *= ratio;
                    }
                    1 => nearby.drag_scale *= ratio,
                    2 => {
                        nearby.guidance_ticks =
                            (f64::from(nearby.guidance_ticks) * ratio).round() as u32
                    }
                    _ => {
                        nearby.boost.ticks = (f64::from(nearby.boost.ticks) * ratio).round() as u32;
                        nearby.sustain.ticks =
                            (f64::from(nearby.sustain.ticks) * ratio).round() as u32;
                        nearby.boost.thrust /= ratio;
                        nearby.sustain.thrust /= ratio;
                    }
                }
                for (anchor, base) in [
                    (m.primary, primary),
                    (anchors[0], reach[0]),
                    (anchors[2], reach[2]),
                ] {
                    let value = candidate_maximum(&w, nearby, anchor);
                    smooth = smooth.max((value / base - 1.).abs());
                }
            }
        }
        if smooth > 0.10 {
            failures.push(format!("{}: discontinuous reach {smooth}", m.source));
        }
        println!(
            "{},{:.6},{:.4},{:.3},{:.3},{:.3},{smooth:.5}",
            m.source,
            f64::from(body.guidance_ticks) / 120.,
            body.drag_scale,
            reach[0],
            reach[1],
            reach[2]
        );
    }
    for modern in ["AIM9M.JT", "AIM9X.JT", "AA11.JT"] {
        for old in ["AIM9B.JT", "AA2.JT", "AA8.JT"] {
            if tails[modern].iter().zip(tails[old]).any(|(a, b)| *a <= b) {
                failures.push(format!(
                    "{modern} does not exceed {old} at every tail altitude"
                ));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let dir = Path::new(args.get(1).ok_or("JT directory required")?);
    if args.get(2).is_some_and(|s| s == "--verify") {
        return verify(dir);
    }
    if args.get(2).is_some_and(|s| s == "--point") {
        let source = args
            .get(3)
            .ok_or("--point SOURCE ALTITUDE_FT ASPECT RANGE_M")?;
        let alt = args.get(4).ok_or("altitude required")?.parse::<f64>()? * FT;
        let aspect = match args.get(5).map(String::as_str) {
            Some("head") => 0,
            Some("beam") => 1,
            Some("tail") => 2,
            Some("stationary") => 3,
            _ => return Err("head, beam, tail or stationary required".into()),
        };
        let range = args
            .get(6)
            .ok_or("range in meters required")?
            .parse::<f64>()?;
        if !alt.is_finite() || alt < 0. || !range.is_finite() || range <= 0. {
            return Err("finite nonnegative altitude and positive range required".into());
        }
        let w = Weapon::parse(source, &fs::read(dir.join(source))?)?;
        println!(
            "source={source} range_m={range} physical_hit={} seeker_hit={}",
            hit(&w, alt, aspect, range, None),
            seeker_hit(&w, alt, aspect, range, None)
        );
        return Ok(());
    }
    if args.iter().any(|s| s == "--fit-pair") {
        fit_pairs(dir, args.get(3).map(String::as_str))?;
        return Ok(());
    }
    println!(
        "source,altitude_ft,aspect,physical_km,seeker_valid_km,anchor_km,drag_scale,retail_lb,physical_lb,anchor_evidence"
    );
    for source in tore_formats::missile_data::REFERENCES
        .iter()
        .map(|r| r.source)
        .chain(
            tore_formats::missile_data::FALLBACKS
                .iter()
                .map(|r| r.source),
        )
    {
        let Ok(bytes) = fs::read(dir.join(source)) else {
            continue;
        };
        let w = Weapon::parse(source, &bytes)?;
        let body = missiles::energy_profile::body(&w);
        let row = tore_formats::missile_data::reference(source)
            .copied()
            .unwrap_or(tore_formats::missile_data::Reference {
                source,
                mass_kg: body.mass,
                diameter_m: body.diameter,
                range_km: f64::from(w.seeker.zones[1].maximum_range) * FT / 1000.,
                dimensions_evidence: "RETAIL-AMBIGUOUS",
                range_evidence: "RETAIL-AMBIGUOUS",
                drag_scale: body.drag_scale,
                imaging: false,
                structural_g: None,
                tracking_deg_s: None,
                fov_half_deg: None,
            });
        let surface = missiles::surface_launched(&w.source);
        let strike = w.flags & 0x20000 != 0;
        if w.movement.fuel_t <= w.movement.ignite_t {
            continue;
        }
        if args.iter().any(|a| a == "--data") {
            eprintln!(
                "{} flags={:x} rawfuel={} body={:?}",
                w.source,
                w.flags,
                w.movement.fuel_t,
                missiles::energy_profile::body(&w)
            );
            continue;
        }
        let drag = None;
        for altitude in if surface || strike {
            vec![if surface {
                (row.range_km * 200.).clamp(300., 3000.) / FT
            } else if tore_sim::combat::guided::kind(&w) == Some(missiles::Guidance::Command) {
                100. / FT
            } else {
                1500. / FT
            }]
        } else {
            vec![500., 5000., 15000., 35000.]
        } {
            for aspect in 0..if strike {
                1
            } else if surface {
                4
            } else {
                3
            } {
                let max = maximum(&w, altitude * FT, aspect, drag, false);
                let valid = if missiles::Profile::for_weapon(&w).is_some() {
                    format!(
                        "{:.3}",
                        maximum(&w, altitude * FT, aspect, drag, true) / 1000.
                    )
                } else {
                    String::new()
                };
                println!(
                    "{},{altitude:.0},{},{:.3},{valid},{},{:.8},{},{:.4},{}",
                    row.source,
                    ["head", "beam", "tail", "stationary"][aspect],
                    max / 1000.,
                    row.range_km,
                    body.drag_scale,
                    w.weight,
                    row.mass_kg / 0.45359237,
                    row.range_evidence
                );
            }
        }
    }
    Ok(())
}
