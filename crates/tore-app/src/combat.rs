//! Live range host, original geometry and sampled original effect art.
use crate::{
    AppResult,
    aircraft::Airframe,
    flight,
    terrain::{Camera, World},
};
use std::collections::BTreeMap;
use tore_formats::{Pic, shape::Shape};
use tore_sim::{
    attitude::{Basis, Vector, unit},
    combat::live::{self, EffectKind, Event, Launcher},
};

#[derive(Default)]
pub struct FireInput {
    pub held: bool,
    inhibited: bool,
}
impl FireInput {
    pub fn space(&mut self, down: bool, repeat: bool, blocked: bool) {
        if !down {
            self.held = false;
            self.inhibited = false;
        } else if blocked {
            self.cancel();
            self.inhibited = true;
        } else if !repeat && !self.inhibited {
            self.held = true;
        }
    }
    pub fn cancel(&mut self) {
        self.inhibited |= self.held;
        self.held = false;
    }
}
pub struct Combat {
    pub state: live::State,
    pub input: FireInput,
    pub range: bool,
    shapes: Vec<Option<Shape>>,
    explosions: Vec<Vec<([f32; 2], [f32; 3])>>,
}
pub fn launcher(s: &flight::State) -> Launcher {
    Launcher {
        position: s.position,
        basis: Basis::new(s.yaw, s.pitch, s.bank),
        speed_fps: s.speed,
        radar: s.radar,
        alive: !s.crashed,
    }
}
impl Combat {
    pub fn new(h: &Airframe, data: &BTreeMap<String, Vec<u8>>, range: bool) -> AppResult<Self> {
        let config = live::Configuration::from_source(&h.profile, |name| {
            data.get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing live-fire resource {name}")))
        })?;
        let shapes = config.stations.iter().map(|s| s.weapon.shape.as_ref().and_then(|name| {
            match data.get(name).ok_or("missing shape".to_string()).and_then(|b| Shape::parse(b).map_err(|e| e.to_string())) {
                Ok(shape) if !shape.faces.is_empty() => Some(shape),
                _ => {eprintln!("Combat: {name} uses a tracer marker; native line/point drawing remains open");None}
            }
        })).collect();
        let pic = Pic::parse(
            data.get("AIRLRG.PIC")
                .ok_or("missing AIRLRG.PIC combat art")?,
        )?;
        if pic.width != 256 || pic.height != 232 {
            return Err("unreviewed AIRLRG frame sheet".into());
        }
        let mut palette = h.palette;
        palette[..pic.palette.len()].copy_from_slice(&pic.palette);
        // Visually reviewed 3x4 cells of the original explosion sheet. Cell
        // cropping, scheduling and 20x20 GPU sampling are fitted presentation.
        let mut explosions = Vec::new();
        for frame in 0..12 {
            let mut cells = Vec::new();
            for y in 0..20 {
                for x in 0..20 {
                    let index = pic.pixels
                        [(frame / 3 * 58 + y * 58 / 20) * pic.width + frame % 3 * 80 + x * 80 / 20];
                    if index != 255 {
                        cells.push((
                            [x as f32 / 20. - 0.5, 0.5 - y as f32 / 20.],
                            palette[index as usize].map(|v| f32::from(v) / 255.),
                        ));
                    }
                }
            }
            explosions.push(cells);
        }
        Ok(Self {
            state: live::State::new(config, range)?,
            input: FireInput::default(),
            range,
            shapes,
            explosions,
        })
    }
    pub fn cancel(&mut self) {
        self.input.cancel();
        self.state.release();
    }
    pub fn reset(&mut self, s: &mut flight::State) -> AppResult<()> {
        self.state = live::State::new(self.state.configuration().clone(), self.range)?;
        self.input = FireInput::default();
        s.set_payload(self.state.payload_lbs())?;
        if self.range {
            self.state.range_target(launcher(s));
        }
        Ok(())
    }
    pub fn step(&mut self, s: &mut flight::State, world: &World) -> AppResult<Vec<Event>> {
        let events = self.state.step(self.input.held, launcher(s), |x, z| {
            f64::from(world.height(x as f32, z as f32))
        });
        s.set_payload(self.state.payload_lbs())?;
        Ok(events)
    }
    pub fn readout(&self, s: &flight::State) -> crate::instruments::CombatReadout {
        let i = self.state.selected;
        crate::instruments::CombatReadout {
            weapon: self.state.configuration().stations[i].weapon.name.clone(),
            guided: self.state.configuration().stations[i]
                .weapon
                .seeker
                .signature
                != 0,
            ammo: self.state.ammo[i],
            loaded: self.range,
            target: self
                .state
                .designated
                .and_then(|id| self.state.targets.iter().find(|t| t.id == id))
                .map(|t| (t.id, t.hp, self.state.can_lock(launcher(s)))),
            contacts: self
                .state
                .targets
                .iter()
                .filter(|t| t.hp > 0 && self.state.radar_detects(launcher(s), t.position))
                .map(|t| {
                    let delta: Vector = std::array::from_fn(|i| t.position[i] - s.position[i]);
                    let b = launcher(s).basis;
                    (
                        tore_sim::attitude::dot(delta, b.right)
                            .atan2(tore_sim::attitude::dot(delta, b.forward)),
                        tore_sim::attitude::dot(delta, delta).sqrt(),
                    )
                })
                .collect(),
        }
    }
    pub fn status(&self, s: &flight::State) -> String {
        let i = self.state.selected;
        let target = self.state.designated.map_or("NO TARGET".into(), |id| {
            self.state
                .targets
                .iter()
                .find(|t| t.id == id)
                .map_or("NO TARGET".into(), |t| {
                    if t.hp == 0 {
                        "DESTROYED".into()
                    } else {
                        format!(
                            "T{id} HP {} {}",
                            t.hp,
                            if self.state.configuration().stations[i]
                                .weapon
                                .seeker
                                .signature
                                == 0
                            {
                                "VISUAL"
                            } else if self.state.can_lock(launcher(s)) {
                                "LOCK"
                            } else {
                                "NO LOCK"
                            }
                        )
                    }
                })
        });
        format!(
            "{} {}  {}  HITS {}",
            self.state.configuration().stations[i].weapon.name,
            self.state.ammo[i],
            target,
            self.state.hits
        )
    }
    pub fn vertices(&self, h: &Airframe, s: &flight::State, camera: &Camera) -> Vec<f32> {
        let mut v = Vec::new();
        for t in self.state.targets.iter().filter(|t| t.hp > 0) {
            let mut pose = s.clone();
            pose.position = t.position;
            pose.pitch = 0.;
            pose.bank = 0.;
            pose.yaw = t.velocity[0].atan2(t.velocity[2]);
            pose.exhaust = 0.;
            pose.gear = 0.;
            pose.flaps = 0.;
            pose.elevator = 0.;
            pose.aileron = 0.;
            pose.rudder = 0.;
            v.extend(h.vertices(&pose, camera));
        }
        for p in &self.state.projectiles {
            if let Some(shape) = &self.shapes[p.station] {
                let forward = p.direction;
                let right = unit([forward[2], 0., -forward[0]]);
                let up = tore_sim::attitude::cross(forward, right);
                for face in &shape.faces {
                    for i in 1..face.positions.len() - 1 {
                        for j in [0, i, i + 1] {
                            let q = face.positions[j];
                            let pos: Vector = std::array::from_fn(|k| {
                                p.position[k]
                                    + (right[k] * f64::from(q[0])
                                        + up[k] * f64::from(q[2])
                                        + forward[k] * f64::from(q[1]))
                                        / 3.
                            });
                            vertex(
                                &mut v,
                                pos,
                                h.palette[face.colors[j] as usize].map(|c| f32::from(c) / 255.),
                            );
                        }
                    }
                }
            }
            // A visible thin strip marks the actual swept projectile segment.
            let right = Basis::new(f64::from(camera.yaw), f64::from(camera.pitch), 0.).right;
            let a: Vector = std::array::from_fn(|i| p.previous[i] + right[i] * 0.4);
            let b: Vector = std::array::from_fn(|i| p.previous[i] - right[i] * 0.4);
            for pos in [a, b, p.position] {
                vertex(&mut v, pos, [1., 0.8, 0.3]);
            }
        }
        let basis = Basis::new(
            f64::from(camera.yaw),
            f64::from(camera.pitch),
            -f64::from(camera.roll),
        );
        for e in &self.state.effects {
            if e.kind == EffectKind::Launch {
                continue;
            }
            let scale = if e.kind == EffectKind::Destroyed {
                75.
            } else {
                15.
            };
            let duration = if e.kind == EffectKind::Destroyed {
                240
            } else {
                45
            };
            let frame = (usize::from(duration - e.ticks) * 12 / usize::from(duration)).min(11);
            for (xy, color) in &self.explosions[frame] {
                for d in [
                    [0., 0.],
                    [1. / 20., 0.],
                    [0., 1. / 20.],
                    [0., 1. / 20.],
                    [1. / 20., 0.],
                    [1. / 20., 1. / 20.],
                ] {
                    let pos: Vector = std::array::from_fn(|i| {
                        e.position[i]
                            + basis.right[i] * f64::from(xy[0] + d[0]) * scale
                            + basis.up[i] * f64::from(xy[1] + d[1]) * scale
                    });
                    vertex(&mut v, pos, *color);
                }
            }
        }
        v
    }
}
fn vertex(out: &mut Vec<f32>, pos: Vector, color: [f32; 3]) {
    out.extend([
        pos[0] as f32,
        pos[1] as f32,
        pos[2] as f32,
        0.,
        0.,
        -1.,
        color[0],
        color[1],
        color[2],
    ]);
}

/// Uses the same imported configuration, flight state, trigger host, movement and
/// hit/effect path as desktop flight. Explicit scripted range, not a retail replay.
pub fn smoke(h: &Airframe, data: &BTreeMap<String, Vec<u8>>) -> AppResult<()> {
    let world = World::for_theater(data, "UKR")?;
    let mut combat = Combat::new(h, data, true)?;
    for index in 0..combat.state.ammo.len() {
        let mut flight = h.start(&world);
        combat.reset(&mut flight)?;
        combat.state.selected = index;
        combat.state.range_target(launcher(&flight));
        combat.state.designate_next();
        let initial = combat.state.ammo[index];
        if let Some(name) = combat.state.configuration().stations[index]
            .weapon
            .fire_sound
            .as_deref()
        {
            let pcm =
                tore_formats::pcm::Pcm::parse(name, data.get(name).ok_or("missing firing PCM")?)?;
            if !pcm.samples.windows(2).any(|s| s[0] != s[1]) {
                return Err("firing PCM contains no signal".into());
            }
        }
        for name in ["&EXPL3.5K", "&EXPL12.5K"] {
            tore_formats::pcm::Pcm::parse(name, data.get(name).ok_or("missing impact PCM")?)?;
        }

        let mut fired = 0;
        let mut impacts = 0;
        let mut destroyed = 0;
        // Pulse for missiles, hold for gun. Two shots are available in the
        // smallest source station; damage remains source class-0 per hit.
        for tick in 0..6000 {
            if tick % 240 == 0 {
                combat.input.space(true, false, false);
            }
            if index != 0 && tick % 240 == 1 {
                combat.input.space(false, false, false);
            }
            flight.step(&flight::PilotInput::default(), |x, z| {
                f64::from(world.height(x as f32, z as f32))
            });
            let events = combat.step(&mut flight, &world)?;
            for event in events {
                match event {
                    Event::Fired(_) => fired += 1,
                    Event::Hit(_) => impacts += 1,
                    Event::Destroyed(_) => destroyed += 1,
                    _ => {}
                }
            }
            if destroyed > 0 {
                break;
            }
        }
        if fired == 0
            || impacts == 0
            || destroyed != 1
            || combat.state.ammo[index] >= initial
            || !combat
                .state
                .effects
                .iter()
                .any(|e| e.kind == EffectKind::Destroyed)
        {
            return Err(format!(
                "combat smoke {} {} failed: fired={fired} hits={impacts} destroyed={destroyed}",
                h.profile.name,
                combat.state.configuration().stations[index].weapon.source
            )
            .into());
        }
        println!(
            "combat smoke {} slot={} {}: shots={fired} hits={impacts} destroyed={destroyed} ammo={}->{} effects={} PASS",
            h.profile.name,
            index + 1,
            combat.state.configuration().stations[index].weapon.source,
            initial,
            combat.state.ammo[index],
            combat.state.effects.len()
        );
        combat.cancel();
        let ammo = combat.state.ammo.clone();
        for _ in 0..120 {
            combat.step(&mut flight, &world)?;
        }
        if combat.state.ammo != ammo {
            return Err("firing continued after release".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fire_requires_unmodified_press_and_release_after_interruption() {
        let mut f = FireInput::default();
        f.space(true, false, true);
        assert!(!f.held);
        f.space(true, true, false);
        assert!(!f.held);
        f.space(false, false, false);
        f.space(true, false, false);
        assert!(f.held);
        f.cancel();
        assert!(!f.held);
        f.space(true, true, false);
        assert!(!f.held);
        f.space(false, false, true);
        f.space(true, false, false);
        assert!(f.held);
    }
}
