//! Selected-object facts at the playhead. Opinionated UI requested by John,
//! 2026-10-15. Read recorded state only; never infer an AI decision from motion.
use super::{context_menu::Target, panels::RecordedTrees, trees};
use std::collections::BTreeMap;
use tore_replay::{
    Frame, Recording, TreeSample, Value, WeaponClass,
    vocab::{channel, field, kind, node},
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Banner {
    pub name: String,
    pub motion: Vec<String>,
    pub action: Option<String>,
    pub target: Option<String>,
}

impl Banner {
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![self.name.clone()];
        if !self.motion.is_empty() {
            lines.push(self.motion.join("  |  "));
        }
        if let Some(action) = &self.action {
            lines.push(format!("Action: {action}"));
        }
        if let Some(target) = &self.target {
            lines.push(format!("Target: {target}"));
        }
        lines
    }
}

/// World scenery has selectable objects that never enter the recorder's registry.
#[derive(Clone, Debug)]
pub struct Ground {
    pub active: bool,
    pub name: String,
    pub position: [f64; 3],
}

/// Index only change events, so rendering never scans a mission's entire journal.
#[derive(Default)]
pub struct Inspector {
    indexed: usize,
    launches: BTreeMap<u32, usize>,
    stations: BTreeMap<u32, BTreeMap<usize, Vec<usize>>>,
    events: BTreeMap<(u32, String), Vec<usize>>,
    trees: RecordedTrees,
}

impl Inspector {
    fn index(&mut self, recording: &Recording) {
        for (i, timed) in recording.events().iter().enumerate().skip(self.indexed) {
            if timed.event.kind == kind::WEAPON_LAUNCH
                && let Some(id) = timed.event.id(field::PROJECTILE)
            {
                self.launches.insert(id, i);
            }
            if timed.event.kind == kind::WEAPON_GUIDED
                && let Some(id) = timed.event.id(field::PROJECTILE)
            {
                self.events
                    .entry((id, kind::WEAPON_GUIDED.into()))
                    .or_default()
                    .push(i);
            }
            if (timed.event.kind == kind::SURFACE_PHASE
                || (timed.event.kind == kind::WEAPON_GUN_BURST
                    && timed.event.string(field::SOURCE) == Some("net.surface")))
                && let Some(id) = timed.event.subject
            {
                let station = timed.event.num(field::STATION).unwrap_or(0.) as usize;
                self.stations
                    .entry(id)
                    .or_default()
                    .entry(station)
                    .or_default()
                    .push(i);
            }
            if let Some(id) = timed.event.subject
                && matches!(
                    timed.event.kind.as_str(),
                    kind::AI_ACTIVITY
                        | kind::AI_TARGET
                        | kind::PLAYER_VIEW_TARGET
                        | kind::SURFACE_PHASE
                        | kind::PLAYER_DESIGNATE
                )
            {
                self.events
                    .entry((id, timed.event.kind.clone()))
                    .or_default()
                    .push(i);
            }
        }
        self.indexed = recording.events().len();
    }

    fn latest<'a>(
        &self,
        recording: &'a Recording,
        id: u32,
        kind: &str,
        tick: u64,
    ) -> Option<&'a tore_replay::TimedEvent> {
        let list = self.events.get(&(id, kind.to_owned()))?;
        let end = list.partition_point(|i| recording.events()[*i].tick <= tick);
        Some(&recording.events()[*list.get(end.checked_sub(1)?)?])
    }

    /// A site's most engaged mount, with deterministic station-order ties.
    fn engagement<'a>(
        &self,
        recording: &'a Recording,
        id: u32,
        tick: u64,
    ) -> Option<&'a tore_replay::TimedEvent> {
        self.stations
            .get(&id)?
            .values()
            .filter_map(|list| {
                let end = list.partition_point(|i| recording.events()[*i].tick <= tick);
                list.get(end.checked_sub(1)?)
                    .map(|i| &recording.events()[*i])
            })
            .filter(|e| {
                e.event.kind != kind::WEAPON_GUN_BURST
                    || e.event
                        .num("last_tick")
                        .is_some_and(|last| tick as f64 <= last)
            })
            .max_by_key(|e| {
                if e.event.kind == kind::WEAPON_GUN_BURST {
                    5
                } else {
                    match e.event.string(field::TO) {
                        Some("fire") => 5,
                        Some("track") => 4,
                        Some("pause") => 3,
                        Some("prepare") => 2,
                        _ => 1,
                    }
                }
            })
    }

    /// Returns None when the selected moving object is absent at this tick.
    /// `previous` is the preceding recorded tick, never a previous UI frame.
    pub fn inspect(
        &mut self,
        recording: &Recording,
        frame: &Frame,
        previous: Option<&Frame>,
        selected: Target,
        ground: Option<Ground>,
    ) -> Option<Banner> {
        self.index(recording);
        let tick = frame.tick;
        let mut out = Banner::default();
        let (position, target) = match selected {
            Target::Aircraft(id) => {
                let state = frame.aircraft.iter().find(|a| a.id == id)?;
                let info = recording.aircraft_info(id)?;
                out.name = format!("{}  |  {}  |  {}", info.label, info.name, info.side.name());
                let thought = self.trees.tree(recording, id, channel::AI_THOUGHT, tick);
                let telemetry = self
                    .trees
                    .tree(recording, id, channel::FLIGHT_TELEMETRY, tick);
                let tree = thought.as_ref().map(|(_, tree)| tree);
                out.motion
                    .push(format!("{:.0} kt", state.airspeed / trees::FPS_PER_KT));
                if let Some(mach) =
                    value(telemetry.as_ref().map(|(_, t)| t), node::MACH).and_then(Value::as_f64)
                {
                    out.motion.push(format!("M {mach:.2}"));
                }
                altitude(&mut out, state.position[1]);
                heading(&mut out, state.attitude[0]);
                let activity = self
                    .latest(recording, id, kind::AI_ACTIVITY, tick)
                    .filter(|e| thought.as_ref().is_none_or(|(t, _)| e.tick >= *t))
                    .and_then(|e| e.event.string(field::TO))
                    .or_else(|| value(tree, node::ACTIVITY).and_then(Value::as_str));
                let activity = if state.flags.crashed {
                    "crashed"
                } else if state.flags.ejected {
                    "ejected"
                } else if !state.flags.alive {
                    "destroyed"
                } else if info.human {
                    "player"
                } else {
                    activity.unwrap_or("-")
                };
                let mission = (!info.human
                    && state.flags.alive
                    && !state.flags.crashed
                    && !state.flags.ejected)
                    .then(|| value(tree, "Mission").and_then(Value::as_str))
                    .flatten();
                out.action = Some(mission.map_or_else(
                    || activity.into(),
                    |mission| format!("{activity} ({mission})"),
                ));
                let event = self.latest(
                    recording,
                    id,
                    if info.human {
                        kind::PLAYER_VIEW_TARGET
                    } else {
                        kind::AI_TARGET
                    },
                    tick,
                );
                let target = event
                    .filter(|e| info.human || thought.as_ref().is_none_or(|(t, _)| e.tick >= *t))
                    .map(|e| e.event.object)
                    .unwrap_or_else(|| value(tree, node::TARGET).and_then(Value::as_id));
                let target = if info.human {
                    self.latest(recording, id, kind::PLAYER_DESIGNATE, tick)
                        .and_then(|e| e.event.object)
                        .or(target)
                } else {
                    target
                };
                (state.position, target)
            }
            Target::Missile(id) => {
                let state = frame.projectiles.iter().find(|p| p.id == id)?;
                let info = recording.weapon_info(state.weapon)?;
                if info.class == WeaponClass::Gun {
                    return None;
                }
                out.name = format!(
                    "{}  |  {} from {}",
                    info.name,
                    info.class.name(),
                    name(recording, state.owner)
                );
                out.motion
                    .push(format!("{:.0} kt", state.speed / trees::FPS_PER_KT));
                altitude(&mut out, state.position[1]);
                if state.direction[0].hypot(state.direction[2]) > 1e-6 {
                    heading(&mut out, state.direction[0].atan2(state.direction[2]));
                }
                let guidance = self
                    .trees
                    .tree(recording, id, channel::WEAPON_GUIDANCE, tick);
                let seeker =
                    value(guidance.as_ref().map(|(_, t)| t), "Seeker").and_then(Value::as_str);
                let guided = self.latest(recording, id, kind::WEAPON_GUIDED, tick);
                out.action = Some(
                    if let Some(mode) = guided.and_then(|e| e.event.string(field::MODE)) {
                        mode.into()
                    } else if let Some(seeker) = seeker {
                        format!("seeker {seeker}")
                    } else if info.class == WeaponClass::Bomb {
                        "falling".into()
                    } else {
                        "-".into()
                    },
                );
                if let Some(side) = recording
                    .aircraft_info(state.owner)
                    .map(|i| i.side)
                    .or_else(|| recording.surface_info(state.owner).map(|i| i.side))
                {
                    out.name.push_str(&format!("  |  {}", side.name()));
                }
                // Unguided bombs/rockets retain their release aim, not a live lock.
                let target = state
                    .target
                    .or_else(|| guided.and_then(|e| e.event.object))
                    .or_else(|| {
                        matches!(info.class, WeaponClass::Bomb | WeaponClass::Rocket)
                            .then(|| self.launches.get(&id).map(|i| &recording.events()[*i]))
                            .flatten()
                            .filter(|e| e.tick <= tick)
                            .and_then(|e| e.event.object)
                    });
                (state.position, target)
            }
            Target::Ground(id) => {
                let info = recording.surface_info(id);
                let pose = frame.surface.iter().find(|s| s.id == id);
                let position = pose
                    .map(|p| p.position)
                    .or_else(|| ground.as_ref().map(|g| g.position))
                    .or_else(|| info.map(|i| i.position))?;
                out.name = ground
                    .as_ref()
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|| name(recording, id));
                if let Some(info) = info {
                    out.name.push_str(&format!("  |  {}", info.side.name()));
                }
                if let Some(pose) = pose {
                    if let Some(old) = previous
                        .filter(|p| p.tick + 1 == tick)
                        .and_then(|f| f.surface.iter().find(|s| s.id == id))
                    {
                        out.motion.push(format!(
                            "{:.0} kt",
                            distance(old.position, position) * 120. / trees::FPS_PER_KT
                        ));
                    }
                    heading(&mut out, pose.attitude[0]);
                }
                // Elevation is useful for all ground objects; AGL and climb are not.
                altitude(&mut out, position[1]);
                let phase = self.engagement(recording, id, tick);
                if phase.is_some()
                    || pose.is_some()
                    || ground.as_ref().map_or(info.is_some(), |g| g.active)
                {
                    out.action = Some(
                        if pose.is_some_and(|p| p.wrecked) {
                            "destroyed"
                        } else if phase.is_some_and(|e| e.event.kind == kind::WEAPON_GUN_BURST) {
                            "firing"
                        } else {
                            phase.and_then(|e| e.event.string(field::TO)).unwrap_or("-")
                        }
                        .into(),
                    );
                }
                (
                    position,
                    phase
                        .filter(|e| {
                            e.event.kind == kind::WEAPON_GUN_BURST
                                || matches!(
                                    e.event.string(field::TO),
                                    Some("prepare" | "track" | "fire" | "pause")
                                )
                        })
                        .and_then(|e| e.event.object),
                )
            }
            Target::Nothing => return None,
        };
        out.target = Some(target.map_or_else(
            || "-".into(),
            |id| {
                let label = name(recording, id);
                let at = frame
                    .aircraft
                    .iter()
                    .find(|a| a.id == id)
                    .map(|a| a.position)
                    .or_else(|| {
                        frame
                            .surface
                            .iter()
                            .find(|s| s.id == id)
                            .map(|s| s.position)
                    })
                    .or_else(|| recording.surface_info(id).map(|s| s.position));
                at.map_or(label.clone(), |at| {
                    format!("{label} ({})", trees::distance(distance(position, at)))
                })
            },
        ));
        // Inert scenery has no action or targeting system.
        if matches!(selected, Target::Ground(_)) && out.action.is_none() {
            out.target = None;
        }
        Some(out)
    }
}

fn value<'a>(tree: Option<&'a TreeSample>, label: &str) -> Option<&'a Value> {
    tree?.node(label).map(|n| &n.value)
}
fn altitude(out: &mut Banner, ft: f64) {
    out.motion.push(format!("{} ft MSL", trees::thousands(ft)));
}
fn heading(out: &mut Banner, radians: f64) {
    let degrees = radians.to_degrees().rem_euclid(360.);
    if degrees.is_finite() {
        let degrees = degrees.round() as u32 % 360;
        out.motion.push(format!("HDG {degrees:03} T"));
    } else {
        out.motion.push("HDG -".into());
    }
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2])
}
fn name(recording: &Recording, id: u32) -> String {
    recording
        .aircraft_info(id)
        .map(|i| i.label.clone())
        .or_else(|| recording.surface_info(id).map(|i| i.label.clone()))
        .unwrap_or_else(|| format!("Object {id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::{fixture, playback::Playback, tests::TempDir};
    use std::sync::Arc;

    #[test]
    fn aircraft_player_missile_bomb_and_old_missing_fields() {
        let dir = TempDir::new("banner-kinds");
        let recording = Arc::new(fixture::recording_with_weapons(dir.path(), "kinds", true));
        let mut playback = Playback::new(Arc::clone(&recording));
        let mut inspector = Inspector::default();
        let (frames, at) = playback.frame(180).unwrap();
        let frame = &frames[at];
        let mut inspect = |target| {
            inspector
                .inspect(&recording, frame, None, target, None)
                .unwrap()
        };
        let ai = inspect(Target::Aircraft(1));
        assert!(ai.name.contains("MiG-29") && ai.name.contains("enemy"));
        assert_eq!(ai.action.as_deref(), Some("ATTACKING"));
        assert!(ai.target.as_deref().unwrap().contains("You"));
        assert!(ai.motion.iter().any(|s| s.ends_with("ft MSL")));
        assert!(
            ai.motion
                .iter()
                .any(|s| s.starts_with("HDG ") && s.ends_with(" T"))
        );
        assert_eq!(
            inspect(Target::Aircraft(0)).action.as_deref(),
            Some("player")
        );
        let missile = inspect(Target::Missile(7));
        assert!(missile.name.contains("AA10") && missile.name.contains("missile"));
        assert!(missile.target.as_deref().unwrap().contains("You"));
        assert_eq!(
            inspect(Target::Missile(13)).action.as_deref(),
            Some("falling")
        );
        let old = inspect(Target::Aircraft(2));
        assert_eq!(old.action.as_deref(), Some("-"));
        assert_eq!(old.target.as_deref(), Some("-"));
        assert!(
            inspector
                .inspect(&recording, frame, None, Target::Missile(14), None)
                .is_none()
        );
        // A disappeared weapon never keeps its last speed, action or target.
        let (frames, at) = playback.frame(500).unwrap();
        assert!(
            inspector
                .inspect(&recording, &frames[at], None, Target::Missile(7), None)
                .is_none()
        );
    }

    #[test]
    fn seeking_backwards_uses_only_facts_at_or_before_the_playhead() {
        let dir = TempDir::new("banner-seek");
        let recording = Arc::new(fixture::recording(dir.path(), "seek"));
        let mut playback = Playback::new(Arc::clone(&recording));
        let mut inspector = Inspector::default();
        let mut sample = |tick| {
            let (frames, at) = playback.frame(tick).unwrap();
            inspector
                .inspect(&recording, &frames[at], None, Target::Aircraft(1), None)
                .unwrap()
        };
        let before = sample(60);
        let after = sample(180);
        assert_eq!(before.action.as_deref(), Some("FORMATION"));
        assert_eq!(after.action.as_deref(), Some("ATTACKING"));
        assert_eq!(sample(60), before);
        assert_eq!(sample(180), after);
        assert_eq!(sample(180), after, "pause is stable");
    }

    #[test]
    fn recorded_surface_mounts_guided_bombs_and_release_targets() {
        use tore_replay::{
            Event, Header, Node, ProjectileState, SurfaceInfo, SurfaceState, TreeSample,
            WeaponInfo, Writer,
        };
        let dir = TempDir::new("banner-surface");
        let path = dir.path().join("surface.tore-replay");
        let mut header = Header::default();
        header.world.surface = true;
        let mut writer = Writer::create(&path, &header).unwrap();
        let unit = 0x4000_0000;
        for (id, label) in [(unit, "Ship"), (unit + 1, "SAM site"), (unit + 2, "Truck")] {
            writer
                .register_surface_unit(&SurfaceInfo {
                    id,
                    name: label.into(),
                    label: label.into(),
                    side: tore_replay::Side::Enemy,
                    position: [0., 50., 6076.],
                    ..Default::default()
                })
                .unwrap();
        }
        for (id, class) in [
            (1, WeaponClass::Missile),
            (2, WeaponClass::Bomb),
            (3, WeaponClass::Rocket),
        ] {
            writer
                .register_weapon(&WeaponInfo {
                    id,
                    name: class.name().into(),
                    class,
                    ..Default::default()
                })
                .unwrap();
        }
        writer
            .register_aircraft(&tore_replay::AircraftInfo {
                id: 10,
                pt: "AH64.PT".into(),
                name: "AH-64".into(),
                label: "Helicopter".into(),
                ..Default::default()
            })
            .unwrap();
        let mut frames = Vec::new();
        for tick in 0..3 {
            let mut frame = Frame {
                tick,
                ..Default::default()
            };
            frame.aircraft.push(tore_replay::AircraftState {
                id: 10,
                airspeed: 168.7809857,
                position: [0., 500., 0.],
                flags: tore_replay::AircraftFlags {
                    alive: true,
                    airborne: true,
                    ..Default::default()
                },
                ..Default::default()
            });
            frame.surface.push(SurfaceState {
                id: unit,
                position: [tick as f64, 50., 0.],
                attitude: [std::f64::consts::FRAC_PI_2, 0., 0.],
                wrecked: false,
            });
            for weapon in 1..=3 {
                frame.projectiles.push(ProjectileState {
                    id: weapon,
                    owner: unit,
                    weapon,
                    position: [0., 2000., 0.],
                    direction: [0., -0.5, 0.5],
                    target: (weapon == 1).then_some(unit + 1),
                    speed: 1687.809857,
                    ..Default::default()
                });
            }
            if tick == 0 {
                frame.events = vec![
                    Event::new(kind::SURFACE_PHASE)
                        .with_subject(unit)
                        .with_object(unit + 1)
                        .with(field::STATION, 0_i64)
                        .with(field::TO, "fire"),
                    Event::new(kind::SURFACE_PHASE)
                        .with_subject(unit)
                        .with(field::STATION, 1_i64)
                        .with(field::TO, "search"),
                    Event::new(kind::WEAPON_LAUNCH)
                        .with_subject(unit)
                        .with_object(unit + 2)
                        .with(field::PROJECTILE, Value::Id(2)),
                    Event::new(kind::WEAPON_LAUNCH)
                        .with_subject(unit)
                        .with_object(unit + 2)
                        .with(field::PROJECTILE, Value::Id(3)),
                ];
                frame.events.push(
                    Event::new(kind::WEAPON_GUN_BURST)
                        .with_subject(unit)
                        .with_object(unit + 2)
                        .with(field::STATION, 9_i64)
                        .with(field::SOURCE, "net.surface")
                        .with("last_tick", 0_i64),
                );
                frame.trees.push(TreeSample {
                    subject: 1,
                    channel: channel::WEAPON_GUIDANCE.into(),
                    nodes: vec![Node::new(0, "Seeker", "tracking")],
                });
            }
            if tick == 1 {
                frame.events.push(
                    Event::new(kind::WEAPON_GUIDED)
                        .with_subject(unit)
                        .with(field::PROJECTILE, Value::Id(2))
                        .with(field::MODE, "guiding"),
                );
            }
            if tick == 2 {
                frame.events.push(
                    Event::new(kind::SURFACE_PHASE)
                        .with_subject(unit)
                        .with(field::STATION, 0_i64)
                        .with(field::TO, "search"),
                );
            }
            writer.push(&frame).unwrap();
            frames.push(frame);
        }
        writer.finish(&tore_replay::Footer::default()).unwrap();
        let recording = Recording::open(path).unwrap();
        let mut inspector = Inspector::default();
        let helicopter = inspector
            .inspect(&recording, &frames[1], None, Target::Aircraft(10), None)
            .unwrap();
        assert!(helicopter.name.contains("AH-64"));
        assert!(helicopter.motion.contains(&"100 kt".into()));
        let mut wrapped = Banner::default();
        heading(&mut wrapped, 359.8_f64.to_radians());
        assert_eq!(wrapped.motion, ["HDG 000 T"]);
        heading(&mut wrapped, -0.0);
        assert_eq!(wrapped.motion[1], "HDG 000 T");
        let firing = inspector
            .inspect(&recording, &frames[0], None, Target::Ground(unit), None)
            .unwrap();
        assert_eq!(firing.action.as_deref(), Some("firing"));
        assert!(firing.target.unwrap().starts_with("Truck"));
        let ship = inspector
            .inspect(
                &recording,
                &frames[1],
                Some(&frames[0]),
                Target::Ground(unit),
                None,
            )
            .unwrap();
        assert_eq!(
            ship.action.as_deref(),
            Some("fire"),
            "an idle second mount must not hide an engagement"
        );
        assert!(ship.target.unwrap().starts_with("SAM site ("));
        assert_eq!(ship.motion, ["71 kt", "HDG 090 T", "50 ft MSL"]);
        let searching = inspector
            .inspect(
                &recording,
                &frames[2],
                Some(&frames[1]),
                Target::Ground(unit),
                None,
            )
            .unwrap();
        assert_eq!(searching.target.as_deref(), Some("-"));
        assert_eq!(searching.action.as_deref(), Some("search"));
        for (id, action, target) in [
            (1, "seeker tracking", "SAM site"),
            (2, "guiding", "Truck"),
            (3, "-", "Truck"),
        ] {
            let banner = inspector
                .inspect(
                    &recording,
                    &frames[1],
                    Some(&frames[0]),
                    Target::Missile(id),
                    None,
                )
                .unwrap();
            assert_eq!(banner.action.as_deref(), Some(action));
            assert!(banner.target.unwrap().starts_with(target));
            assert!(banner.motion.contains(&"1000 kt".into()));
        }
        let before = inspector
            .inspect(&recording, &frames[0], None, Target::Missile(2), None)
            .unwrap();
        assert_eq!(
            before.action.as_deref(),
            Some("falling"),
            "rewind before guidance"
        );
    }

    #[test]
    fn scenery_omits_inapplicable_fields_and_heading_wraps() {
        let dir = TempDir::new("banner-ground");
        let recording = fixture::recording(dir.path(), "ground");
        let banner = Inspector::default()
            .inspect(
                &recording,
                &Frame {
                    tick: 10,
                    ..Default::default()
                },
                None,
                Target::Ground(9001),
                Some(Ground {
                    active: false,
                    name: "Radar installation".into(),
                    position: [0., 123., 0.],
                }),
            )
            .unwrap();
        assert_eq!(banner.lines(), ["Radar installation", "123 ft MSL"]);
        assert!(banner.action.is_none() && banner.target.is_none());
    }
}
