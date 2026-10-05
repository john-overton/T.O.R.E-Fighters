//! Independent ownship observation work. No shared projectile, event, random
//! stream or live peer state is available through this context.
use super::*;

pub(super) struct Context<'a, G> {
    pub targets: &'a [Target],
    pub rows: &'a [OwnRow],
    pub weapon_rules: Rules,
    pub easy_targeting: bool,
    pub tick: u64,
    pub ground: &'a G,
}

pub(super) fn observe_ownship(
    own: &mut Ownship,
    own_index: usize,
    launcher: Launcher,
    context: &Context<'_, impl Fn(f64, f64) -> f64 + Sync>,
) {
    let obscured = |from: Vector, to: Vector| terrain_hit(from, to, context.ground).is_some();
    // Shared observations are produced before this tick's firing decision,
    // so the scope, the target view and weapon support all agree.
    own.sensors.controls = launcher.controls;
    let observables: Vec<Observable> = context
        .targets
        .iter()
        .chain(peers(context.rows, own_index))
        .map(observable_of)
        .collect();
    let observer = Observer {
        position: launcher.position,
        basis: launcher.basis,
        radar_powered: launcher.radar && launcher.alive,
        radar_failed: own.radar_failed,
        infrared_failed: own.infrared_failed || !launcher.alive,
        visual_failed: own.visual_failed || !launcher.alive,
    };
    let height = |x: f64, z: f64| (context.ground)(x, z);
    let environment = sensors::Environment {
        ground: &height,
        obscured: &obscured,
    };
    own.sensors.keep_selection = context.easy_targeting;
    let in_view = own.designated().or(own.sight_hold);
    own.sensors.step(&observer, &observables, &environment);
    if let Some(id) = own.designated() {
        own.hud_selection = Some(id);
    } else if !context.easy_targeting {
        // A dropped target is gone; nothing is remembered for later.
        own.hud_selection = None;
    }
    // The views keep a dropped target while it is within visual range, in
    // any direction (John, 2026-09-28); beyond it the target is gone for good.
    let visual_range = own
        .sensors
        .profiles
        .visual
        .as_ref()
        .filter(|_| !observer.visual_failed)
        .map_or(0., |visual| visual.search.maximum_ft);
    own.sight_hold = own.designated().or(in_view.filter(|id| {
        context.targets.iter().any(|t| {
            t.id == *id
                && t.body_present()
                && (0..3)
                    .map(|i| (t.position[i] - launcher.position[i]).powi(2))
                    .sum::<f64>()
                    .sqrt()
                    <= visual_range
        })
    }));
    own.emitters = passive::emitters(
        &observer,
        &observables,
        own.sensors.contacts(),
        &environment,
    );
    own.bore_observation = None;
    let index = own.selected;
    let Some(station) = own.config.stations.get(index) else {
        own.mounted = Seeker::default();
        own.range_estimate = None;
        return;
    };
    let w = &station.weapon;
    if let Some(profile) =
        missiles::Profile::for_weapon(w).filter(|_| context.weapon_rules == Rules::Spec)
    {
        if !profile.guidance_available(launcher.radar_power) || !profile.supports_boresight() {
            own.launch_mode = LaunchMode::Cued;
        }
        if profile.guidance_available(launcher.radar_power)
            && own.armed
            && own.designated().is_none()
            && profile.supports_boresight()
        {
            own.launch_mode = LaunchMode::Boresight;
        }
        if profile.guidance == Guidance::Infrared && own.designated().is_some() {
            own.launch_mode = LaunchMode::Cued;
        }
        let assigned = if own.launch_mode == LaunchMode::Cued {
            own.designated()
        } else {
            None
        };
        let key = (index, own.launch_mode, assigned);
        if own.mounted_key != Some(key) {
            own.mounted = Seeker::new(assigned);
            own.mounted_key = Some(key);
        }
        if !profile.guidance_available(launcher.radar_power) {
            own.mounted = Seeker {
                status: Status::Unguided,
                ..Default::default()
            };
            own.mounted_key = None;
        }
        if own.armed
            && profile.guidance_available(launcher.radar_power)
            && launcher.alive
            && own.hp > 0
            && own.rounds(index) > 0
            && own.ammo[index] & 0x8000 == 0
        {
            let bore = own.launch_mode == LaunchMode::Boresight;
            let cap = bore.then(|| profile.search_cap());
            // Mounted IR may choose a stronger return. Released missiles keep identity.
            if bore && profile.guidance == Guidance::Infrared {
                own.mounted.target = None;
                own.mounted.acquired = false;
                own.mounted.missing = 0;
            }
            let view = seeker::View {
                position: launcher.position,
                basis: launcher.basis,
                cap,
                obscured: &obscured,
            };
            let observations: Vec<_> = context
                .targets
                .iter()
                .chain(peers(context.rows, own_index))
                .filter(|t| t.body_present())
                .filter(|t| bore || assigned == Some(t.id))
                .filter_map(|t| seeker::observe(w, profile, &view, t))
                .filter(|o| {
                    !bore
                        || profile.guidance != Guidance::Active
                        || own.config.sensors.radar.as_ref().is_some_and(|r| {
                            o.range <= launcher.controls.range_nmi() * missiles::NMI
                                && o.range <= r.track.maximum_ft
                        })
                })
                .collect();
            if bore {
                own.bore_observation = observations
                    .iter()
                    .min_by(|a, b| seeker::compare_returns(a, b, profile))
                    .copied();
            }
            if bore && profile.guidance == Guidance::Active {
                // The HUD estimate never pre-locks or assigns an active-radar shot.
                own.mounted = Seeker::default();
            } else if own.launch_mode == LaunchMode::Cued
                && matches!(profile.guidance, Guidance::Active | Guidance::Supported)
            {
                let supported: Vec<_> = observations
                    .into_iter()
                    .filter(|o| own.sensors.supports(o.id))
                    .collect();
                own.mounted.step(
                    missiles::Profile {
                        guidance: Guidance::Supported,
                        ..profile
                    },
                    &supported,
                );
            } else {
                own.mounted.step(profile, &observations);
            }
        } else {
            own.mounted = Seeker::new(assigned);
        }
    } else {
        own.bore_observation = None;
        own.mounted = Seeker::default();
        own.mounted_key = None;
    }
    if let Some(o) = weapon_observation(own, launcher, context.weapon_rules, |id| {
        context
            .targets
            .iter()
            .chain(peers(context.rows, own_index))
            .find(|target| target.id == id)
    }) {
        if context.tick.is_multiple_of(60)
            || own
                .range_estimate
                .is_none_or(|e| e.station != index || e.target != o.id || e.mode != own.launch_mode)
        {
            own.range_estimate = missiles::Profile::for_weapon(w).map(|profile| {
                let maximum = missiles::maximum_range(
                    w,
                    launcher.position,
                    launcher.basis.forward,
                    launcher.velocity,
                    o.position,
                    o.velocity,
                    profile.guidance_ticks,
                );
                RangeEstimate {
                    station: index,
                    target: o.id,
                    mode: own.launch_mode,
                    maximum,
                    favorable: missiles::firing_band(
                        w,
                        launcher.position,
                        launcher.basis,
                        launcher.velocity,
                        o,
                        maximum,
                        profile.guidance_ticks,
                        own.launch_mode == LaunchMode::Boresight,
                    ),
                }
            });
        }
    } else {
        own.range_estimate = None;
    }
}
