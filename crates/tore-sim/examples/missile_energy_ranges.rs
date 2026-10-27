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
        if dot(gap, gap) <= radius * radius
            && age >= q.arming_age
            && !m.energy.unwrap().flight.defeated
        {
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
    if body.loft_range > 0. {
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
    candidate_maximum_precision(w, body, anchor, 12)
}
fn candidate_maximum_precision(
    w: &Weapon,
    body: missiles::energy::Body,
    anchor: [f64; 4],
    precision: usize,
) -> f64 {
    if precision < 12 {
        // Local bracketing for optimizer iterations. Every emitted candidate
        // is remeasured with the complete loft-aware scan below.
        let mut high = (anchor[2] * 1000.).clamp(100., 1_000_000.);
        let mut low = high;
        if candidate_hit(w, body, anchor, high) {
            while high < 1_000_000. {
                high = (high * 1.5).min(1_000_000.);
                if !candidate_hit(w, body, anchor, high) {
                    break;
                }
                low = high;
            }
        } else {
            loop {
                low /= 1.5;
                if low < 100. {
                    return 0.;
                }
                if candidate_hit(w, body, anchor, low) {
                    break;
                }
                high = low;
            }
        }
        for _ in 0..precision {
            let mid = (low + high) * 0.5;
            if candidate_hit(w, body, anchor, mid) {
                low = mid;
            } else {
                high = mid;
            }
        }
        return low / 1000.;
    }
    let mut low = 0.;
    let mut high: f64 = 1_000_000.;
    let mut distances = Vec::new();
    let mut d = 100.;
    while d < 1_000_000. {
        distances.push(d);
        d *= 1.5;
    }
    if body.loft_range > 0. {
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
    for _ in 0..precision {
        let mid = (low + high) * 0.5;
        if candidate_hit(w, body, anchor, mid) {
            low = mid;
        } else {
            high = mid;
        }
    }
    low / 1000.
}
/// Fixed-budget balance fitting. The only adjustable parameters are fuel and
/// drag. Explicit thrust overrides, burn durations and control budgets stay fixed.
fn fit_class(
    dir: &Path,
    filter: Option<&str>,
    ground_roles: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if ground_roles {
        println!("source,class,fraction,drag,control_s,anchor_km,primary_km,score");
    } else {
        println!(
            "source,class,fraction,drag,control_s,anchor_km,tail_target_km,score,min_launch_km,floor_km,r500h,r500b,r500t,r5h,r5b,r5t,r15h,r15b,r15t,r35h,r35b,r35t"
        );
    }
    for m in tore_formats::missile_motors::MOTORS {
        if filter.is_some_and(|s| !s.split(',').any(|name| name == m.source))
            || m.control_class.tail_floor_km().is_none() != ground_roles
        {
            continue;
        }
        let w = Weapon::parse(m.source, &fs::read(dir.join(m.source))?)?;
        let original = missiles::energy_profile::body(&w);
        let minimum = w
            .seeker
            .zones
            .iter()
            .map(|z| f64::from(z.minimum_range) * FT / 1000.)
            .fold(0f64, f64::max);
        let floor = m
            .control_class
            .tail_floor_km()
            .map_or(0., |v| v.max(1.2 * minimum + 0.01));
        let scaled = |fuel: f64, drag: f64| {
            let mut b = original;
            let ratio = fuel / m.propellant_fraction;
            b.boost.propellant *= ratio;
            b.sustain.propellant *= ratio;
            if m.thrust.is_none() {
                b.boost.thrust *= ratio;
                b.sustain.thrust *= ratio;
            }
            b.drag_scale = drag;
            b
        };
        for drag_step in 2..=12 {
            let drag = f64::from(drag_step) * 0.25;
            let threshold = |anchor: [f64; 4]| {
                let mut lo = 0.12;
                let mut hi = 0.5;
                if candidate_hit(&w, scaled(lo, drag), anchor, anchor[2] * 1000.) {
                    return lo;
                }
                if candidate_hit(&w, scaled(hi, drag), anchor, anchor[2] * 1000.) {
                    for _ in 0..12 {
                        let mid = (lo + hi) * 0.5;
                        if candidate_hit(&w, scaled(mid, drag), anchor, anchor[2] * 1000.) {
                            hi = mid;
                        } else {
                            lo = mid;
                        }
                    }
                }
                hi
            };
            let primary_fuel = threshold(m.primary);
            let mut fuels = vec![0.12, primary_fuel, 0.5];
            if !ground_roles {
                let tail_fuel = threshold(m.secondary);
                fuels.extend([
                    tail_fuel,
                    0.25 * primary_fuel + 0.75 * tail_fuel,
                    0.5 * (primary_fuel + tail_fuel),
                    0.75 * primary_fuel + 0.25 * tail_fuel,
                ]);
            }
            fuels.sort_by(f64::total_cmp);
            fuels.dedup_by(|a, b| (*a - *b).abs() < 0.00001);
            for fuel in fuels {
                let b = scaled(fuel, drag);
                let primary = candidate_maximum(&w, b, m.primary);
                let mut score = 4. * (primary / m.primary[2] - 1.).abs() + 0.02 * drag.ln().powi(2);
                if ground_roles {
                    println!(
                        "{},{:?},{fuel:.9},{drag},{},{},{primary:.6},{score:.8}",
                        m.source,
                        m.control_class,
                        m.control_class.seconds(),
                        m.primary[2]
                    );
                    continue;
                }
                let tail = candidate_maximum(&w, b, m.secondary);
                if tail < floor {
                    continue;
                }
                score += 4. * (tail / m.secondary[2] - 1.).abs();
                let mut ranges = Vec::new();
                for alt in [500., 5000., 15000., 35000.] {
                    for aspect in 0..3 {
                        ranges.push(if alt == 35000. && aspect == 0 {
                            primary
                        } else if alt == 5000. && aspect == 2 {
                            tail
                        } else {
                            candidate_maximum(&w, b, [alt, f64::from(aspect), 0., 250.])
                        });
                    }
                }
                if ranges.contains(&0.) {
                    continue;
                }
                print!(
                    "{},{:?},{fuel:.9},{drag},{},{},{},{score:.8},{minimum},{floor}",
                    m.source,
                    m.control_class,
                    m.control_class.seconds(),
                    m.primary[2],
                    m.secondary[2]
                );
                for value in ranges {
                    print!(",{value:.6}");
                }
                println!();
            }
        }
        eprintln!("balance candidates {}", m.source);
    }
    Ok(())
}

/// Authored game balance, never a reconstruction of a real motor.
/// Fuel .12..50%, drag .5..3, phase duration .25..16 times nominal and <=100%
/// of the fixed control budget, impulse scale .25..4, dry-mass axial load <=60 G.
fn fit_motors<const N: usize>(
    dir: &Path,
    filter: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "source,class,fraction,drag,control_s,boost_s,sustain_s,exhaust_mps,boost_n,sustain_n,boost_share,anchor_km,tail_target_km,primary_km,score,min_launch_km,floor_km,r500h,r500b,r500t,r5h,r5b,r5t,r15h,r15b,r15t,r35h,r35b,r35t"
    );
    let sources: Vec<_> = tore_formats::missile_motors::MOTORS
        .iter()
        .filter(|m| {
            let eligible = m.control_class.tail_floor_km().is_some()
                || matches!(
                    m.source,
                    "AGM84E.JT" | "AS7.JT" | "AS16.JT" | "AS14.JT" | "AS30.JT" | "AT12.JT"
                );
            eligible && filter.is_none_or(|f| f.split(',').any(|n| n == m.source))
        })
        .collect();
    std::thread::scope(|scope| {
        let mut jobs = Vec::new();
        for worker in 0..4 {
            let sources = &sources;
            jobs.push(scope.spawn(move || -> Result<(), String> {
                for m in sources.iter().skip(worker).step_by(4) {
                    let text = fit_motor_store::<N>(dir, m).map_err(|e| e.to_string())?;
                    std::io::Write::write_all(&mut std::io::stdout().lock(), text.as_bytes())
                        .map_err(|e| e.to_string())?;
                    eprintln!("motor fits {}", m.source);
                }
                Ok(())
            }));
        }
        for job in jobs {
            job.join()
                .map_err(|_| "fit worker panicked".to_string())??;
        }
        Ok::<_, String>(())
    })?;
    Ok(())
}
fn fit_motor_store<const N: usize>(
    dir: &Path,
    m: &tore_formats::missile_motors::MotorData,
) -> Result<String, Box<dyn std::error::Error>> {
    use missiles::energy::{Body, DT, G};
    use std::fmt::Write;
    let mut output = String::new();
    let air = m.control_class.tail_floor_km().is_some();
    let w = Weapon::parse(m.source, &fs::read(dir.join(m.source))?)?;
    let base = missiles::energy_profile::body(&w);
    let minimum = w
        .seeker
        .zones
        .iter()
        .map(|z| f64::from(z.minimum_range) * FT / 1000.)
        .fold(0f64, f64::max);
    let floor = m
        .control_class
        .tail_floor_km()
        .map_or(0., |f| f.max(1.2 * minimum + 0.02));
    let max_burn_scale = (f64::from(base.guidance_ticks)
        / f64::from(base.boost.ticks + base.sustain.ticks))
    .min(16.);
    let build = |x: [f64; N]| -> (Body, f64, f64) {
        let fuel = 0.12 + 0.38 * x[0];
        let duration = (0.25f64.ln() + x[2] * (max_burn_scale / 0.25).ln()).exp();
        let impulse = (0.25f64.ln() + x[3] * 16f64.ln()).exp();
        let mut body = base;
        body.drag_scale = 0.5 + 2.5 * x[1];
        for (phase, nominal) in [
            (&mut body.boost, base.boost),
            (&mut body.sustain, base.sustain),
        ] {
            phase.propellant = nominal.propellant * fuel / m.propellant_fraction;
            if nominal.ticks > 0 {
                phase.ticks = (f64::from(nominal.ticks) * duration).round().max(1.) as u32;
                phase.thrust = nominal.thrust * fuel / m.propellant_fraction
                    * impulse
                    * f64::from(nominal.ticks)
                    / f64::from(phase.ticks);
            }
        }
        if N == 6 {
            let total = ((f64::from(base.boost.ticks + base.sustain.ticks) * duration).round()
                as u32)
                .max(2);
            let share = 0.05 + 0.9 * x[5];
            let boost = (f64::from(total) * (0.05 + 0.9 * x[4]))
                .round()
                .clamp(1., f64::from(total - 1)) as u32;
            let total_impulse = (base.boost.thrust * f64::from(base.boost.ticks)
                + base.sustain.thrust * f64::from(base.sustain.ticks))
                * DT
                * fuel
                / m.propellant_fraction
                * impulse;
            body.boost = missiles::energy::Motor {
                ticks: boost,
                propellant: base.mass * fuel * share,
                thrust: total_impulse * share / (f64::from(boost) * DT),
            };
            body.sustain = missiles::energy::Motor {
                ticks: total - boost,
                propellant: base.mass * fuel * (1. - share),
                thrust: total_impulse * (1. - share) / (f64::from(total - boost) * DT),
            };
        }
        (body, fuel, impulse)
    };
    let mut effective_tail = if air { m.secondary[2].max(floor) } else { 0. };
    if matches!(m.source, "AIM7.JT" | "AIM7E.JT") {
        let a = tore_formats::missile_motors::motor("AIM7.JT")
            .unwrap()
            .secondary[2];
        let b = tore_formats::missile_motors::motor("AIM7E.JT")
            .unwrap()
            .secondary[2];
        // Minimum equal-relative-error compromise for contradictory tail targets.
        effective_tail = 2. * a * b / (a + b) + if m.source == "AIM7.JT" { 0.02 } else { -0.02 };
    }
    let goals: &[(f64, f64)] = if air {
        &[(1., 1.), (0.92, 1.), (1.08, 1.), (1., 0.92), (1., 1.08)]
    } else {
        &[(1., 1.)]
    };
    let mut outputs: Vec<([f64; N], f64)> = Vec::new();
    for &(head_factor, tail_factor) in goals {
        for start in 0..4 {
            let seed = if start == 0 {
                [
                    ((m.propellant_fraction - 0.12) / 0.38).clamp(0., 1.),
                    ((base.drag_scale - 0.5) / 2.5).clamp(0., 1.),
                    (4f64.ln() / (max_burn_scale / 0.25).ln()).clamp(0., 1.),
                    0.5,
                ]
            } else if start == 1 {
                [0.55, 0.8, 0.25, 0.55]
            } else if start == 2 {
                [0.55, 1., 0., 0.2]
            } else {
                [0.3, 0.3, 0.8, 0.65]
            };
            let mut initial = [0.; N];
            initial[..4].copy_from_slice(&seed);
            if N == 6 {
                let (time, share) = match start {
                    0 => (0.1, 0.5),
                    1 => (0.05, 0.35),
                    2 => (0.2, 0.7),
                    _ => (0.4, 0.8),
                };
                initial[4] = (time - 0.05) / 0.9;
                initial[5] = (share - 0.05) / 0.9;
            }
            while initial[3] > 0. {
                let (body, _, _) = build(initial);
                if [body.boost, body.sustain]
                    .iter()
                    .all(|p| p.thrust <= body.dry_mass() * 60. * G)
                {
                    break;
                }
                initial[3] = (initial[3] - 0.1).max(0.);
            }
            let evaluate = |x: [f64; N]| {
                let (b, _, _) = build(x);
                if [b.boost, b.sustain]
                    .iter()
                    .any(|p| p.thrust > b.dry_mass() * 60. * G)
                {
                    return 1e6;
                }
                let head = candidate_maximum_precision(&w, b, m.primary, 12);
                let head_error = (head / (m.primary[2] * head_factor) - 1.).powi(2);
                let mut value = 25. * head_error;
                if !air {
                    for fraction in [0.75, 0.9] {
                        if !candidate_hit(&w, b, m.primary, m.primary[2] * 1000. * fraction) {
                            value += 4.;
                        }
                    }
                }
                if air {
                    let tail = candidate_maximum_precision(&w, b, m.secondary, 12);
                    let tail_error = (tail / (effective_tail * tail_factor) - 1.).powi(2);
                    value = 30. * head_error.max(tail_error) + 5. * (head_error + tail_error);
                    value += 100. * (1. - tail / floor).max(0.).powi(2);
                }
                value
                    + 0.001
                        * (b.drag_scale.ln().powi(2)
                            + (x[2] - initial[2]).powi(2)
                            + (x[3] - 0.5).powi(2))
            };
            let clamp = |x: [f64; N]| x.map(|v| v.clamp(0., 1.));
            let mut simplex: Vec<_> = std::iter::once(initial)
                .chain((0..N).map(|i| {
                    let mut x = initial;
                    x[i] = if x[i] > 0.8 { x[i] - 0.18 } else { x[i] + 0.18 };
                    clamp(x)
                }))
                .map(|x| (x, evaluate(x)))
                .collect();
            for iteration in 0..120 {
                simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
                if iteration > 30 && simplex[0].1 < 0.0003 {
                    break;
                }
                let centroid: [f64; N] = std::array::from_fn(|i| {
                    simplex[..N].iter().map(|p| p.0[i]).sum::<f64>() / N as f64
                });
                let point = |factor: f64| {
                    clamp(std::array::from_fn(|i| {
                        centroid[i] + factor * (centroid[i] - simplex[N].0[i])
                    }))
                };
                let reflected = point(1.);
                let score = evaluate(reflected);
                if score < simplex[0].1 {
                    let expanded = point(2.);
                    let e = evaluate(expanded);
                    simplex[N] = if e < score {
                        (expanded, e)
                    } else {
                        (reflected, score)
                    };
                } else if score < simplex[N - 1].1 {
                    simplex[N] = (reflected, score);
                } else {
                    let contracted = point(if score < simplex[N].1 { 0.5 } else { -0.5 });
                    let c = evaluate(contracted);
                    if c < score.min(simplex[N].1) {
                        simplex[N] = (contracted, c);
                    } else {
                        let best = simplex[0].0;
                        for p in &mut simplex[1..] {
                            p.0 = clamp(std::array::from_fn(|i| 0.5 * (p.0[i] + best[i])));
                            p.1 = evaluate(p.0);
                        }
                    }
                }
            }
            simplex.sort_by(|a, b| a.1.total_cmp(&b.1));
            outputs.extend(simplex.into_iter().take(2));
        }
    }
    // Keep bounded low-energy endpoints too, rather than losing a feasible
    // endpoint when a local search follows a disconnected hit interval.
    if !air && N == 4 {
        let nominal_duration = (4f64.ln() / (max_burn_scale / 0.25).ln()).clamp(0., 1.);
        for seed in [
            [0., 1., nominal_duration, 0.5],
            [0., 1., 0., 0.],
            [1., 1., 0., 0.],
        ] {
            let mut x = [0.; N];
            x[..4].copy_from_slice(&seed);
            outputs.push((x, 0.));
        }
    }
    for (x, _) in outputs {
        let (b, fuel, impulse) = build(x);
        if [b.boost, b.sustain]
            .iter()
            .any(|p| p.thrust > b.dry_mass() * 60. * G)
        {
            continue;
        }
        let primary = candidate_maximum(&w, b, m.primary);
        let tail = if air {
            candidate_maximum(&w, b, m.secondary)
        } else {
            0.
        };
        let score = if air {
            let head = (primary / m.primary[2] - 1.).abs();
            let tail = (tail / m.secondary[2] - 1.).abs();
            8. * head.max(tail) + head + tail
        } else {
            4. * (primary / m.primary[2] - 1.).abs()
                + [0.75, 0.9]
                    .into_iter()
                    .filter(|fraction| {
                        !candidate_hit(&w, b, m.primary, m.primary[2] * 1000. * fraction)
                    })
                    .count() as f64
                    * 4.
        };
        let mut ranges = vec![0.; 12];
        if air {
            for (i, (alt, aspect)) in [500., 5000., 15000., 35000.]
                .into_iter()
                .flat_map(|alt| (0..3).map(move |aspect| (alt, aspect)))
                .enumerate()
            {
                ranges[i] = if i == 9 {
                    primary
                } else if i == 5 {
                    tail
                } else {
                    candidate_maximum(&w, b, [alt, f64::from(aspect), 0., 250.])
                };
            }
            if tail < floor || ranges.contains(&0.) {
                continue;
            }
        }
        write!(&mut output,
                "{},{:?},{fuel:.9},{:.9},{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{},{},{primary:.6},{score:.8},{minimum},{floor}",
                m.source,
                m.control_class,
                b.drag_scale,
                m.control_class.seconds(),
                f64::from(b.boost.ticks) * DT,
                f64::from(b.sustain.ticks) * DT,
                m.exhaust_mps * impulse,
                b.boost.thrust,
                b.sustain.thrust,
                b.boost.propellant / (b.boost.propellant + b.sustain.propellant),
                m.primary[2],
                if air { m.secondary[2] } else { 0. }
            ).unwrap();
        for r in ranges {
            write!(&mut output, ",{r:.6}").unwrap();
        }
        writeln!(&mut output).unwrap();
    }
    Ok(output)
}

/// Edge and ten-percent-beyond-edge diagnostics, using the same force kernel.
fn diagnostics(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "source,altitude_ft,aspect,range_km,edge_fraction,hit,seconds,defeated,control_expired,mach,available_g,required_g,g_deficit,closing_mps,max_g_deficit,ground_impact,life_expired"
    );
    for m in tore_formats::missile_motors::MOTORS {
        let w = Weapon::parse(m.source, &fs::read(dir.join(m.source))?)?;
        let b = missiles::energy_profile::body(&w);
        let air = m.control_class.tail_floor_km().is_some();
        let conditions: Vec<_> = if air {
            [500., 5000., 15000., 35000.]
                .into_iter()
                .flat_map(|alt| (0..3).map(move |aspect| [alt, f64::from(aspect), 0., 250.]))
                .collect()
        } else {
            vec![m.primary]
        };
        for condition in conditions {
            let edge = candidate_maximum(&w, b, condition);
            for ratio in [1., 1.1] {
                let mut q = question(
                    &w,
                    condition[0] * FT,
                    condition[1] as usize,
                    edge * ratio * 1000.,
                );
                let mut last = missiles::energy::Forces::default();
                let mut deficit: f64 = 0.;
                while q.answer.is_none() {
                    let mut sample = q.flight;
                    last = sample.step_diagnostics(q.body, q.wind, Some(q.target));
                    deficit = deficit.max(last.g_deficit);
                    q.step(&|_, _| 0.);
                }
                let a = q.answer.unwrap();
                println!(
                    "{},{},{},{:.6},{ratio},{},{:.6},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{deficit:.6},{},{}",
                    m.source,
                    condition[0],
                    condition[1],
                    edge * ratio,
                    a.hit,
                    a.seconds,
                    q.flight.defeated,
                    q.flight.age > q.body.guidance_ticks,
                    last.mach,
                    last.available_g,
                    last.required_g,
                    last.g_deficit,
                    last.closing_mps,
                    q.flight.position[1] <= 0.,
                    q.flight.age >= q.body.life
                );
            }
        }
    }
    Ok(())
}
fn fit_pairs(
    dir: &Path,
    filter: Option<&str>,
    measure_only: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
        let fractions: Vec<_> = if measure_only {
            vec![]
        } else if motor.thrust.is_some() || !motor.fit_propellant {
            vec![motor.propellant_fraction]
        } else {
            (6..=25).map(|n| n as f64 * 0.02).collect()
        };
        let tail_anchor = [5000., 2., 0., 250.];
        let tail_floor = (motor.primary[2] * 0.08).clamp(2., 6.);
        let mut best = if measure_only {
            Some((
                motor.propellant_fraction,
                original,
                candidate_maximum(&w, original, motor.secondary),
                if air {
                    candidate_maximum(&w, original, tail_anchor)
                } else {
                    0.
                },
                0.,
            ))
        } else {
            None
        };
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
                // Controlled life fitted per missile to its head-on reference:
                // the documented anchor, with the selected physical motor.
                // Ground/ship cases use their stationary or receding reference.
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

// Named, documented balance residuals. New profiles get no exemption.
// The normal band is approximately 10%, with 0.5 percentage point numerical
// allowance. Exceptions are bounded on the side of the actual compromise;
// a shortfall does not authorize a new overshoot.
fn accepted_fit_bands(source: &str) -> ([f64; 2], [f64; 2]) {
    match source {
        "AA10.JT" => ([0.86, 1.105], [0.895, 1.13]),
        "AA2.JT" => ([0.82, 1.105], [0.895, 1.18]),
        "AA8.JT" => ([0.895, 1.3], [0.895, 1.105]),
        "AIM54C.JT" => ([0.895, 1.105], [0.895, 1.14]),
        "AIM7.JT" => ([0.82, 1.105], [0.895, 1.21]),
        "AIM7E.JT" => ([0.895, 1.105], [0.79, 1.105]),
        // These former overshoots now fit the normal band.
        "AIM9M.JT" | "AIM9X.JT" => ([0.895, 1.105], [0.895, 1.105]),
        _ => ([0.895, 1.105], [0.895, 1.105]),
    }
}
fn in_fit_band(actual: f64, target: f64, band: [f64; 2]) -> bool {
    actual.is_finite() && target > 0. && actual >= target * band[0] && actual <= target * band[1]
}
#[cfg(test)]
mod fit_gate_tests {
    use super::*;
    #[test]
    fn an_unlisted_overshoot_is_rejected_even_when_it_is_finite() {
        assert!(!in_fit_band(120., 100., accepted_fit_bands("NEW.JT").0));
        assert!(in_fit_band(100., 100., accepted_fit_bands("NEW.JT").0));
        assert!(in_fit_band(12.8, 10., accepted_fit_bands("AA8.JT").0));
        assert!(!in_fit_band(16., 10., accepted_fit_bands("AA8.JT").0));
    }
}
fn verify(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("source,guidance_seconds,drag_scale,tail5_km,tail15_km,tail35_km,smooth_relative");
    let mut failures = Vec::new();
    let mut tails = std::collections::BTreeMap::new();
    for m in tore_formats::missile_motors::MOTORS {
        let w = Weapon::parse(m.source, &fs::read(dir.join(m.source))?)?;
        let body = missiles::energy_profile::body(&w);
        if !(0.5..=3.).contains(&body.drag_scale) || !(0.12..=0.5).contains(&m.propellant_fraction)
        {
            failures.push(format!("{}: fuel/drag outside approved bounds", m.source));
        }
        let primary = candidate_maximum(&w, body, m.primary);
        if !in_fit_band(primary, m.primary[2], accepted_fit_bands(m.source).0) {
            failures.push(format!(
                "{}: unapproved primary residual: {primary:.3} vs {}",
                m.source, m.primary[2]
            ));
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
        let minimum = w
            .seeker
            .zones
            .iter()
            .map(|z| f64::from(z.minimum_range) * FT / 1000.)
            .fold(0f64, f64::max);
        let floor = m.control_class.tail_floor_km().expect("explicit A2A class");
        if reach[0] < floor
            || reach[0] <= 1.2 * minimum
            || reach[1] < reach[0] * 0.9
            || reach[2] < reach[1] * 0.9
        {
            failures.push(format!(
                "{}: inadequate or reversed tail reach {reach:?}",
                m.source
            ));
        }
        tails.insert(m.source, reach);
        if !in_fit_band(reach[0], m.secondary[2], accepted_fit_bands(m.source).1) {
            failures.push(format!(
                "{}: unapproved round-2 tail residual: {:.3} vs {}",
                m.source, reach[0], m.secondary[2]
            ));
        }
        if candidate_hit(&w, body, m.primary, primary * 1500.) {
            failures.push(format!(
                "{}: 150% measured head-on envelope still hits",
                m.source
            ));
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
    // Within-family upgrades must win every measured altitude/aspect cell.
    for (new, old) in [
        ("AIM7.JT", "AIM7E.JT"),
        ("R27ER.JT", "AA10.JT"),
        ("R27ET.JT", "R27T.JT"),
    ] {
        let newer = Weapon::parse(new, &fs::read(dir.join(new))?)?;
        let older = Weapon::parse(old, &fs::read(dir.join(old))?)?;
        for altitude in [500., 5000., 15000., 35000.] {
            for aspect in [0., 1., 2.] {
                let condition = [altitude, aspect, 0., 250.];
                let a =
                    candidate_maximum(&newer, missiles::energy_profile::body(&newer), condition);
                let b =
                    candidate_maximum(&older, missiles::energy_profile::body(&older), condition);
                if a <= b {
                    failures.push(format!(
                        "{new} <= {old} at {altitude} ft aspect {aspect}: {a:.3}/{b:.3}"
                    ));
                }
            }
        }
    }
    // John's Phase 2.6 low-altitude role comparison, separate from the
    // high-head references and the within-family upgrade checks.
    for new in ["AIM120.JT", "AA12.JT", "MICA.JT"] {
        let newer = Weapon::parse(new, &fs::read(dir.join(new))?)?;
        for old in ["AIM7E.JT", "AIM7.JT", "AA10.JT", "R530.JT"] {
            let older = Weapon::parse(old, &fs::read(dir.join(old))?)?;
            for aspect in [0., 2.] {
                let condition = [5000., aspect, 0., 250.];
                let a =
                    candidate_maximum(&newer, missiles::energy_profile::body(&newer), condition);
                let b =
                    candidate_maximum(&older, missiles::energy_profile::body(&older), condition);
                if a < b {
                    failures.push(format!(
                        "5k role reversal {new}/{old} aspect {aspect}: {a}/{b}"
                    ));
                }
            }
        }
    }
    // Cross-family ordering is justified by the high-head reference, not by
    // generation alone: burn duration and drag can reverse other geometries.
    for new in ["AIM120.JT", "AA12.JT", "MICA.JT"] {
        let newer = Weapon::parse(new, &fs::read(dir.join(new))?)?;
        for old in ["AIM7E.JT", "AA6.JT", "R530.JT"] {
            let older = Weapon::parse(old, &fs::read(dir.join(old))?)?;
            let condition = [35000., 0., 0., 250.];
            if candidate_maximum(&newer, missiles::energy_profile::body(&newer), condition)
                <= candidate_maximum(&older, missiles::energy_profile::body(&older), condition)
            {
                failures.push(format!("high-head generation reversal {new}/{old}"));
            }
        }
    }
    for name in ["AIM9M.JT", "AIM9X.JT", "AA11.JT"] {
        let w = Weapon::parse(name, &fs::read(dir.join(name))?)?;
        if candidate_hit(
            &w,
            missiles::energy_profile::body(&w),
            [500., 2., 12., 250.],
            12000.,
        ) {
            failures.push(format!("{name}: 12 km deck drag did not escape"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

fn fit_sparrow(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let w = Weapon::parse("AIM7.JT", &fs::read(dir.join("AIM7.JT"))?)?;
    let old = Weapon::parse("AIM7E.JT", &fs::read(dir.join("AIM7E.JT"))?)?;
    let original = missiles::energy_profile::body(&w);
    let mut cases = Vec::new();
    let older = missiles::energy_profile::body(&old);
    for altitude in [500., 5000., 15000., 35000.] {
        for aspect in 0..3 {
            let condition = [altitude, aspect as f64, 0., 250.];
            cases.push((condition, candidate_maximum(&old, older, condition)));
        }
    }
    println!("fraction,drag,ignition_seconds,guidance_seconds,min_advantage,score");
    for fraction in [0.35, 0.4, 0.45, 0.5] {
        for drag in [0.5, 0.75, 1., 1.5, 2., 2.5, 3.] {
            for ignition in [0f64, 0.25, 0.5, 1.] {
                let mut b = original;
                let scale = fraction
                    / tore_formats::missile_motors::motor("AIM7.JT")
                        .unwrap()
                        .propellant_fraction;
                b.boost.propellant *= scale;
                b.boost.thrust *= scale;
                b.sustain.propellant *= scale;
                b.sustain.thrust *= scale;
                b.ignition = (ignition * 120.) as u32;
                b.drag_scale = drag;
                b.guidance_ticks = b.life;
                let mut q = question(&w, 35000. * FT, 0, 70000.);
                q.body = b;
                while q.answer.is_none() {
                    q.step_over(&|_, _| 0., &|a, b| a[1] > 0. && b[1] > 0.);
                }
                let a = q.answer.unwrap();
                if !a.hit {
                    continue;
                }
                b.guidance_ticks = (a.seconds * 120.).ceil() as u32 + 1;
                let mut minimum = f64::INFINITY;
                for &(condition, reference) in &cases {
                    let reach = candidate_maximum(&w, b, condition);
                    // A zero/zero beam cell is a failure, never a NaN that
                    // f64::min silently ignores. Require an actual intercept.
                    let advantage = if reach == 0. {
                        0.
                    } else if reference == 0. {
                        f64::INFINITY
                    } else {
                        reach / reference
                    };
                    minimum = minimum.min(advantage);
                    if minimum < 1.05 {
                        break;
                    }
                }
                if minimum >= 1.05 {
                    let score = (fraction - 0.35).powi(2)
                        + 0.02 * drag.ln().powi(2)
                        + 0.01 * (1. - ignition).powi(2);
                    println!(
                        "{fraction},{drag},{ignition},{:.6},{minimum:.4},{score:.6}",
                        f64::from(b.guidance_ticks) / 120.
                    );
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let dir = Path::new(args.get(1).ok_or("JT directory required")?);
    if args.get(2).is_some_and(|s| s == "--diagnostics") {
        return diagnostics(dir);
    }
    if args.get(2).is_some_and(|s| s == "--fit-staged") {
        return fit_motors::<6>(dir, args.get(3).map(String::as_str));
    }
    if args.get(2).is_some_and(|s| s == "--fit-motors") {
        return fit_motors::<4>(dir, args.get(3).map(String::as_str));
    }
    if args.get(2).is_some_and(|s| s == "--fit-ground") {
        return fit_class(dir, args.get(3).map(String::as_str), true);
    }
    if args.get(2).is_some_and(|s| s == "--fit-class") {
        return fit_class(dir, args.get(3).map(String::as_str), false);
    }
    if args
        .get(2)
        .is_some_and(|s| s == "--fit-pair" || s == "--fit-sparrow")
    {
        return Err("Timer fitting retired. Use --fit-class; historical reproduction only: --legacy-fit-pair or --legacy-fit-sparrow".into());
    }
    if args.get(2).is_some_and(|s| s.starts_with("--legacy-")) {
        eprintln!("LEGACY TIMER-FITTED CALIBRATION, NOT THE CURRENT MODEL");
    }
    if args.get(2).is_some_and(|s| s == "--legacy-fit-sparrow") {
        return fit_sparrow(dir);
    }
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
    if args
        .iter()
        .any(|s| s == "--legacy-fit-pair" || s == "--measure-pairs")
    {
        fit_pairs(
            dir,
            args.get(3).map(String::as_str),
            args[2] == "--measure-pairs",
        )?;
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
