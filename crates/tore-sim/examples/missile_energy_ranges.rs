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
    // Search every increasing range, including minimum-range failures.
    while high < 1_000_000. {
        if hit(w, alt, aspect, high, drag) {
            low = high;
            upper = 1_000_000.;
        } else if high > low {
            upper = upper.min(high);
        }
        high *= 1.5;
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let dir = Path::new(args.get(1).ok_or("JT directory required")?);
    let fit = args.iter().any(|a| a == "--fit");
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
                motor_exhaust_mps: 2200.,
                imaging: false,
                structural_g: None,
                tracking_deg_s: None,
                fov_half_deg: None,
                motor: None,
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
        let mut drag = None;
        if fit {
            let alt = if surface {
                (row.range_km * 200.).clamp(300., 3000.)
            } else if tore_sim::combat::guided::kind(&w) == Some(missiles::Guidance::Command) {
                100.
            } else if strike {
                1500.
            } else {
                35000. * FT
            };
            let aspect = if surface && !strike {
                3
            } else if row.range_evidence.ends_with("TAIL") {
                2
            } else {
                0
            };
            let target = row.range_km * 1000.;
            let mut low: f64 = 0.05;
            let mut high: f64 = 30.;
            while high < 1024. && hit(&w, alt, aspect, target, Some(high)) {
                high *= 2.;
            }
            for _ in 0..16 {
                let mid: f64 = (low * high).sqrt();
                if hit(&w, alt, aspect, target, Some(mid)) {
                    low = mid
                } else {
                    high = mid;
                }
            }
            drag = Some(low);
        }
        for altitude in if surface || strike {
            vec![if surface {
                (row.range_km * 200.).clamp(300., 3000.) / FT
            } else if tore_sim::combat::guided::kind(&w) == Some(missiles::Guidance::Command) {
                100. / FT
            } else {
                1500. / FT
            }]
        } else {
            vec![5000., 15000., 35000.]
        } {
            for aspect in 0..if strike {
                1
            } else if surface {
                4
            } else {
                3
            } {
                let max = maximum(&w, altitude * FT, aspect, drag, false);
                let valid = if !fit && missiles::Profile::for_weapon(&w).is_some() {
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
                    drag.unwrap_or(question(&w, altitude * FT, aspect, 1000.).body.drag_scale),
                    w.weight,
                    row.mass_kg / 0.45359237,
                    row.range_evidence
                );
            }
        }
    }
    Ok(())
}
