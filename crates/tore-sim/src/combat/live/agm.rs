//! The player's air-to-ground missiles (slice W2): the A-G track that
//! supports the surface radar weapons, the HARM boresight candidate, the
//! emitters the HARM's HUD cue shows, and the in-flight retarget of the
//! supported surface missiles. docs/spec/air-to-ground.md, "Missiles and the
//! HARM boresight cue" and "Weapon classes" 8 to 11; retail facts in
//! docs/formats/radar.md ("A-G page", "HARM page") and docs/formats/missiles.md
//! ("Target switch").
use super::{Launcher, Ownship, Projectile, State, Target};
use crate::attitude::{Basis, Vector, dot, unit};
use crate::combat::missiles::{
    self, Profile,
    seeker::{self, Seeker, Status},
};
use crate::sensors::Mode;
use tore_formats::weapons::{Weapon, Zone};

/// Emitters the HARM cue lists at most (opinionated, John, 2026-10-10; the
/// count is the agent's).
pub const MAX_HARM_CUE: usize = 8;

/// One emitter of the HARM boresight cue: an object the anti-radiation
/// seeker receives inside its cone, or the cued target after its radar went
/// off (dim).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CueEmitter {
    pub observation: seeker::Observation,
    /// Its radar paints this aircraft (the RWR's bright square).
    pub painting: bool,
    /// It emits now. A cued or acquired target that stopped emitting stays
    /// listed, dim, while it is inside the seeker's cone.
    pub emitting: bool,
}

impl Ownship {
    /// Whether the A-G page holds `id` now: the page is up and lists it.
    /// This is the track a surface radar weapon is released and supported on
    /// (retail data: A-G on at release; for AS-7 and AS-15 for the whole
    /// flight).
    pub fn ground_track(&self, id: u32) -> bool {
        !self.radar_failed
            && self.sensors.ground_page() == Some(Mode::AirGround)
            && self.sensors.ground_contact(id).is_some()
    }
    /// The radars the host says paint this aircraft (the surface units in
    /// Track or Fire on it): the HARM boresight candidate prefers them.
    pub fn painted_by(&self) -> &std::collections::BTreeSet<u32> {
        &self.painted_by
    }
    /// The HARM boresight cue's emitters this step (see [`harm_emitters`]):
    /// empty unless an anti-radiation missile is armed. For the HUD only.
    pub fn harm_cue(&self) -> &[CueEmitter] {
        &self.harm_cue
    }
}

impl State {
    /// The radars that paint `aircraft`, set by the host every tick from the
    /// surface units' fire control (the RWR's painting list).
    pub fn set_painted_by(&mut self, aircraft: u32, emitters: impl IntoIterator<Item = u32>) {
        if let Some(own) = self.ownship_mut(aircraft) {
            own.painted_by = emitters.into_iter().collect();
        }
    }
}

/// Whether the launcher's fire control supports a missile of `profile` at
/// `id` this tick. Surface radar weapons ride the A-G track, a supported one
/// (AS-7, AS-15) with the target inside its zone 1, range included, seen
/// from the launcher (retail data); every other weapon keeps the air radar's
/// answer.
pub(super) fn ownship_supports(
    own: &Ownship,
    profile: Option<Profile>,
    w: &Weapon,
    launcher: &Launcher,
    id: u32,
    target: Option<&Target>,
) -> bool {
    match profile {
        Some(p) if p.needs_air_ground() => {
            own.ground_track(id)
                && (!p.rides_air_ground()
                    || target.is_some_and(|t| {
                        missiles::geometry(
                            &w.seeker.zones[1],
                            launcher.position,
                            launcher.basis,
                            t.position,
                            None,
                        )
                    }))
        }
        _ => own.sensors.supports(id),
    }
}

/// Whether `target` lies inside `zone`'s heading and pitch limits about
/// `basis`, angles only: the retail in-flight lock (`PROJInFOV` about the
/// weapon's nose, no range or altitude).
pub(super) fn inside_angles(zone: &Zone, position: Vector, basis: Basis, target: Vector) -> bool {
    missiles::geometry(
        &Zone {
            minimum_range: 0,
            maximum_range: i32::MAX,
            minimum_altitude: i32::MIN,
            maximum_altitude: i32::MAX,
            ..*zone
        },
        position,
        basis,
        target,
        None,
    )
}

/// A projectile's seeker basis: its nose, level.
pub(super) fn nose(direction: Vector) -> Basis {
    Basis::new(
        direction[0].atan2(direction[2]),
        direction[1].atan2(direction[0].hypot(direction[2])),
        0.,
    )
}

/// The HARM boresight candidate among the seeker's returns: inside the
/// launch zone's angles, painting first, then nearest the bore, then nearest,
/// then by identity (docs/spec/air-to-ground.md; the order is fitted).
pub(super) fn harm_candidate(
    returns: &[seeker::Observation],
    painted_by: &std::collections::BTreeSet<u32>,
    w: &Weapon,
    launcher: &Launcher,
) -> Option<seeker::Observation> {
    returns
        .iter()
        .filter(|o| {
            missiles::geometry(
                &missiles::launch_geometry(w),
                launcher.position,
                launcher.basis,
                o.position,
                None,
            )
        })
        .min_by(|a, b| {
            painted_by
                .contains(&b.id)
                .cmp(&painted_by.contains(&a.id))
                .then(a.off_axis.total_cmp(&b.off_axis))
                .then(a.range.total_cmp(&b.range))
                .then(a.id.cmp(&b.id))
        })
        .copied()
}

/// The emitters the HARM cue shows for the selected anti-radiation weapon:
/// every object its seeker receives inside its cone (zone 0, 45 degrees on
/// the AGM-88 and AGM-45), nearest the bore first, at most [`MAX_HARM_CUE`];
/// and the cued or acquired target, dim, after its radar stops while it is
/// inside the cone. Empty for any other weapon.
pub(super) fn harm_emitters<'a>(
    own: &Ownship,
    launcher: &Launcher,
    targets: impl Iterator<Item = &'a Target>,
    obscured: &dyn Fn(Vector, Vector) -> bool,
) -> Vec<CueEmitter> {
    let Some(station) = own.config.stations.get(own.selected) else {
        return Vec::new();
    };
    let w = &station.weapon;
    let Some(profile) = Profile::for_weapon(w).filter(|p| p.holds_on_angles()) else {
        return Vec::new();
    };
    if !own.armed || !launcher.alive || !profile.guidance_available(launcher.radar_power) {
        return Vec::new();
    }
    let view = seeker::View {
        position: launcher.position,
        basis: launcher.basis,
        cap: None,
        obscured,
    };
    let held = own.designated().or(own.mounted.target);
    let mut cue: Vec<CueEmitter> = Vec::new();
    for t in targets.filter(|t| t.body_present()) {
        if let Some(observation) = seeker::observe(w, profile, &view, t) {
            cue.push(CueEmitter {
                observation,
                painting: own.painted_by.contains(&t.id),
                emitting: true,
            });
        } else if held == Some(t.id)
            && !obscured(view.position, t.position)
            && missiles::geometry(
                &w.seeker.zones[0],
                view.position,
                view.basis,
                t.position,
                None,
            )
        {
            let delta = missiles::sub(t.position, launcher.position);
            cue.push(CueEmitter {
                observation: seeker::Observation {
                    id: t.id,
                    position: t.position,
                    velocity: t.velocity,
                    quality: 0.,
                    off_axis: dot(unit(delta), launcher.basis.forward)
                        .clamp(-1., 1.)
                        .acos(),
                    range: missiles::length(delta),
                },
                painting: false,
                emitting: false,
            });
        }
    }
    cue.sort_by(|a, b| {
        a.observation
            .off_axis
            .total_cmp(&b.observation.off_axis)
            .then(a.observation.id.cmp(&b.observation.id))
    });
    cue.truncate(MAX_HARM_CUE);
    cue
}

/// The player's new designation `id` retargets the supported surface
/// missiles `own` has in flight (AS-7, AS-15): one whose zone-1 cone about
/// its nose holds the new target, with the A-G track on it and the target
/// inside the weapon's zone 1 from the launcher, now homes on it; any other
/// is blinded and flies on unguided (retail `PROJRetargetMissiles`,
/// docs/formats/missiles.md, "Target switch"). A missile that already homes
/// on `id` is left alone. Clearing the designation does not come here.
pub(super) fn retarget(state: &mut State, own: &Ownship, id: u32, launcher: &Launcher) {
    let target = state
        .targets
        .iter()
        .chain(state.ownship_rows.iter())
        .find(|t| t.id == id && t.body_present())
        .cloned();
    for p in state
        .projectiles
        .iter_mut()
        .filter(|p| p.owner == own.aircraft)
    {
        let w = super::projectile_weapon(std::slice::from_ref(own), p).clone();
        let Some(flight) = p.guidance.as_mut() else {
            continue;
        };
        if !flight.profile.rides_air_ground() || flight.unguided || flight.seeker.target == Some(id)
        {
            continue;
        }
        let locks = target.as_ref().is_some_and(|t| {
            flight.profile.accepts(t)
                && inside_angles(
                    &w.seeker.zones[1],
                    p.position,
                    nose(p.direction),
                    t.position,
                )
                && ownship_supports(own, Some(flight.profile), &w, launcher, id, Some(t))
        });
        if locks {
            flight.seeker = Seeker::new(Some(id));
            flight.qualified_target = Some(id);
            flight.last_intercept = None;
            flight.solution = None;
            p.target = Some(id);
        } else {
            blind(p);
        }
    }
}

/// The missile loses its target for good and flies on along its nose.
pub(super) fn blind(p: &mut Projectile) {
    if let Some(flight) = p.guidance.as_mut() {
        flight.unguided = true;
        flight.enabled = false;
        flight.seeker = Seeker {
            status: Status::Lost,
            ..Seeker::default()
        };
        flight.last_intercept = None;
        flight.solution = None;
    }
    p.target = None;
}

crate::checkpoint_struct!(CueEmitter {
    observation,
    painting,
    emitting,
});
