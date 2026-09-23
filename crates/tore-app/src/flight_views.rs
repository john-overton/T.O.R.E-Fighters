//! Manual-derived flight views, with fitted camera geometry. Never changes simulation.
use crate::{attitude::Basis, combat::Combat, flight, terrain::Camera};
use tore_sim::attitude::{Vector, dot};

// Preserve the existing capture interface: 0 front, 1 external, 2 oblique, 3 back, 4 up.
pub const TRACK: u8 = 5;
pub const THREAT: u8 = 6;
pub const WING: u8 = 7;
pub const TARGET: u8 = 8;
pub const TARGET_PLAYER: u8 = 9;
pub const FLY_BY: u8 = 10;
pub const MISSILE: u8 = 11;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reference {
    #[default]
    Player,
    Target,
    Missile,
}

pub fn key(key: &str) -> Option<u8> {
    Some(match key {
        "F1" => 0,
        "F2" => 3,
        "F3" => 4,
        "F4" => TRACK,
        "F5" => THREAT,
        "F6" => WING,
        "F7" => TARGET,
        "F8" => TARGET_PLAYER,
        "F9" => FLY_BY,
        "F10" => 1,
        "F12" => MISSILE,
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug)]
struct Body {
    id: u32,
    position: Vector,
    velocity: Vector,
    basis: Basis,
    missile: bool,
}
#[derive(Clone, Copy)]
struct Shot {
    body: Body,
    owner: u32,
    target: Option<u32>,
    incoming: bool,
}

pub struct Scene {
    player: Body,
    target: Option<u32>,
    bodies: Vec<Body>,
    wings: Vec<(u32, bool, u8, u8)>,
    missiles: Vec<Shot>,
}
impl Scene {
    pub fn new(
        player: &flight::State,
        combat: &Combat,
        wings: Option<&crate::ai_wings::AiWings>,
        presented: bool,
    ) -> Self {
        Self {
            player: Body {
                id: 0,
                position: player.view_position(),
                velocity: player.velocity,
                basis: Basis::new(player.yaw, player.pitch, player.bank),
                missile: false,
            },
            target: combat.state.display_target().map(|t| t.id),
            bodies: combat
                .state
                .targets
                .iter()
                .filter(|t| t.airborne || t.hp > 0)
                .map(|t| {
                    let (position, angles) = combat.view_pose(t, presented);
                    Body {
                        id: t.id,
                        position,
                        velocity: t.velocity,
                        basis: Basis::new(angles[0], angles[1], angles[2]),
                        missile: false,
                    }
                })
                .collect(),
            wings: wings.map_or_else(Vec::new, |w| {
                w.slots()
                    .iter()
                    .filter(|slot| {
                        combat
                            .state
                            .targets
                            .iter()
                            .any(|t| t.id == slot.id && t.airborne && t.hp > 0)
                    })
                    .map(|slot| {
                        (
                            slot.id,
                            slot.side == tore_sim::ai::launch::Side::Friendly,
                            slot.wing_number,
                            slot.member_number,
                        )
                    })
                    .collect()
            }),
            missiles: combat
                .state
                .projectiles
                .iter()
                .filter(|p| !tore_sim::combat::live::is_gun(p.weapon(combat.state.configuration())))
                .map(|p| {
                    let direction = unit(p.direction, [0., 0., 1.]);
                    Shot {
                        body: Body {
                            id: p.id,
                            position: p.position,
                            velocity: direction.map(|v| v * f64::from(p.speed_f8) / 256.),
                            basis: Basis::new(
                                direction[0].atan2(direction[2]),
                                direction[1].atan2(direction[0].hypot(direction[2])),
                                0.,
                            ),
                            missile: true,
                        },
                        owner: p.owner,
                        target: p.target,
                        incoming: p.incoming,
                    }
                })
                .collect(),
        }
    }
    fn body(&self, id: u32) -> Option<Body> {
        if id == 0 {
            Some(self.player)
        } else {
            self.bodies.iter().find(|b| b.id == id).copied()
        }
    }
    fn target(&self) -> Result<Body, &'static str> {
        self.target
            .and_then(|id| self.body(id))
            .ok_or("No current target for this view")
    }
    fn wing(&self, reference: Body) -> Result<Body, &'static str> {
        let id = if reference.missile {
            self.missiles
                .iter()
                .find(|p| p.body.id == reference.id)
                .map_or(0, |p| p.owner)
        } else {
            reference.id
        };
        let group = if id == 0 {
            Some((true, 1))
        } else {
            self.wings.iter().find(|s| s.0 == id).map(|s| (s.1, s.2))
        };
        self.wings
            .iter()
            .filter(|s| s.0 != id && Some((s.1, s.2)) == group)
            .min_by_key(|s| (s.2, s.3, s.0))
            .and_then(|s| self.body(s.0))
            .ok_or("No wingman for this view")
    }
}

#[derive(Clone)]
struct Saved {
    view: u8,
    reference: Reference,
    look: [f32; 2],
    zoom: f32,
    fly_by: Option<Vector>,
}
#[derive(Clone, Default)]
pub struct Rig {
    pub reference: Reference,
    last_missile: Option<u32>,
    fly_by: Option<Vector>,
    other: Option<Saved>,
    /// Reject a pending GPU image after V changes the saved camera.
    pub other_pending: bool,
}
impl Rig {
    pub fn observe(&mut self, scene: &Scene) {
        if let Some(id) = scene
            .missiles
            .iter()
            .filter(|p| p.owner == 0 && !p.incoming)
            .map(|p| p.body.id)
            .max()
        {
            self.last_missile = Some(self.last_missile.map_or(id, |old| old.max(id)));
        }
    }
    pub fn select(&mut self, reference: Reference) {
        self.reference = reference;
        self.fly_by = None;
    }
    pub fn cockpit(&self, view: u8) -> bool {
        self.reference == Reference::Player && matches!(view, 0 | 3 | 4 | TRACK)
    }
    pub fn save(&mut self, view: u8, look: [f32; 2], zoom: f32) {
        self.other_pending = false;
        self.other = Some(Saved {
            view,
            reference: self.reference,
            look,
            zoom,
            fly_by: self.fly_by,
        });
    }
    pub fn other_shows_player(&self) -> bool {
        self.other.as_ref().is_some_and(|s| {
            s.reference != Reference::Player || !matches!(s.view, 0 | 3 | 4 | TRACK)
        })
    }
    pub fn other_view(&self) -> u8 {
        self.other.as_ref().map_or(3, |s| s.view)
    }
    pub fn other_camera(&self, scene: &Scene, base: Camera) -> Result<Camera, &'static str> {
        let saved = self.other.clone().unwrap_or(Saved {
            view: 3,
            reference: Reference::Player,
            look: [0.; 2],
            zoom: 1.,
            fly_by: None,
        });
        let mut rig = self.clone();
        rig.reference = saved.reference;
        rig.fly_by = saved.fly_by;
        let mut camera = rig.camera(saved.view, scene, base, saved.look, saved.zoom)?;
        camera.weather_slot = 3;
        Ok(camera)
    }
    fn missile<'a>(&self, scene: &'a Scene) -> Result<&'a Shot, &'static str> {
        scene
            .missiles
            .iter()
            .find(|p| p.owner == 0 && !p.incoming && Some(p.body.id) == self.last_missile)
            .ok_or("No last-launched missile for this view")
    }
    pub fn camera(
        &mut self,
        view: u8,
        scene: &Scene,
        mut camera: Camera,
        look: [f32; 2],
        zoom: f32,
    ) -> Result<Camera, &'static str> {
        self.observe(scene);
        let keys = std::mem::take(&mut camera.keys);
        let subject = match self.reference {
            Reference::Player => scene.player,
            Reference::Target => scene.target()?,
            Reference::Missile => self.missile(scene)?.body,
        };
        if matches!(view, 0..=4) {
            if self.reference != Reference::Player {
                camera = body_camera(subject, view);
            }
            crate::look::apply(
                &mut camera,
                subject.position.map(|v| v as f32),
                look,
                matches!(view, 1 | 2),
            );
        } else {
            match view {
                TRACK => {
                    let target = scene.target()?;
                    let delta = direction(subject.position, target.position, subject.basis.forward);
                    let yaw =
                        dot(delta, subject.basis.right).atan2(dot(delta, subject.basis.forward));
                    let pitch = dot(delta, subject.basis.up)
                        .clamp(-1., 1.)
                        .asin()
                        .clamp(0., std::f64::consts::FRAC_PI_2);
                    camera = body_camera(subject, 0);
                    crate::look::apply(
                        &mut camera,
                        subject.position.map(|v| v as f32),
                        [yaw as f32, pitch as f32],
                        false,
                    );
                }
                THREAT => {
                    let threat = scene
                        .missiles
                        .iter()
                        .filter(|p| {
                            !subject.missile
                                && (p.target == Some(subject.id) || (subject.id == 0 && p.incoming))
                        })
                        .min_by(|a, b| {
                            distance(a.body.position, subject.position)
                                .total_cmp(&distance(b.body.position, subject.position))
                        })
                        .ok_or("No inbound missile for this view")?;
                    camera = relation(subject, threat.body.position);
                }
                WING => camera = relation(subject, scene.wing(subject)?.position),
                TARGET => camera = relation(subject, scene.target()?.position),
                TARGET_PLAYER => camera = relation(scene.target()?, subject.position),
                FLY_BY => {
                    let eye = *self.fly_by.get_or_insert_with(|| {
                        std::array::from_fn(|i| {
                            subject.position[i]
                                + subject.velocity[i] * 3.
                                + subject.basis.right[i] * 300.
                                + if i == 1 { 100. } else { 0. }
                        })
                    });
                    camera = facing(eye, subject.position, subject.basis.forward);
                }
                MISSILE => {
                    let missile = if self.reference == Reference::Target {
                        scene
                            .missiles
                            .iter()
                            .filter(|p| p.owner == subject.id)
                            .max_by_key(|p| p.body.id)
                            .ok_or("Target has no live missile")?
                    } else {
                        self.missile(scene)?
                    };
                    let target = missile.target.and_then(|id| scene.body(id)).map_or_else(
                        || {
                            std::array::from_fn(|i| {
                                missile.body.position[i] + missile.body.basis.forward[i] * 1000.
                            })
                        },
                        |b| b.position,
                    );
                    camera = relation(missile.body, target);
                }
                _ => return Err("Unknown flight view"),
            }
        }
        if matches!(view, 0 | 3 | 4 | TRACK) && self.reference != Reference::Player {
            if subject.missile {
                camera.hidden_projectile = Some(subject.id);
            } else {
                camera.hidden_target = Some(subject.id);
            }
        }
        camera.keys = keys;
        camera.zoom = zoom;
        Ok(camera)
    }
}
fn body_camera(body: Body, view: u8) -> Camera {
    let mut camera = Camera::new();
    camera.position = body.position.map(|v| v as f32);
    let [yaw, pitch, bank] = body.basis.angles();
    camera.yaw = yaw as f32;
    camera.pitch = pitch as f32;
    camera.roll = -bank as f32;
    match view {
        3 => camera.yaw += std::f32::consts::PI,
        4 => camera.pitch += 0.8,
        1 | 2 => {
            let angle = yaw + if view == 2 { 0.8 } else { 0. };
            let distance = if view == 2 { 130. } else { 180. };
            camera.position[0] -= (angle.sin() * distance) as f32;
            camera.position[2] -= (angle.cos() * distance) as f32;
            camera.position[1] += 60.;
            camera.yaw = angle as f32;
            camera.pitch = -0.3;
            camera.roll = 0.;
        }
        _ => {}
    }
    camera
}
fn distance(a: Vector, b: Vector) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1]).hypot(a[2] - b[2])
}
fn unit(v: Vector, fallback: Vector) -> Vector {
    let length = v[0].hypot(v[1]).hypot(v[2]);
    if length < 1e-6 {
        fallback
    } else {
        v.map(|n| n / length)
    }
}
fn direction(from: Vector, to: Vector, fallback: Vector) -> Vector {
    unit(std::array::from_fn(|i| to[i] - from[i]), fallback)
}
fn facing(eye: Vector, target: Vector, fallback: Vector) -> Camera {
    let d = direction(eye, target, fallback);
    let mut c = Camera::new();
    c.position = eye.map(|v| v as f32);
    c.yaw = d[0].atan2(d[2]) as f32;
    c.pitch = d[1].atan2(d[0].hypot(d[2])) as f32;
    c
}
fn relation(subject: Body, target: Vector) -> Camera {
    let d = direction(subject.position, target, subject.basis.forward);
    let (back, up) = if subject.missile {
        (30., 10.)
    } else {
        (180., 60.)
    };
    let eye =
        std::array::from_fn(|i| subject.position[i] - d[i] * back + if i == 1 { up } else { 0. });
    // Aim through the subject so both endpoints share the center sightline.
    facing(eye, target, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body(id: u32, position: Vector) -> Body {
        Body {
            id,
            position,
            velocity: [0., 0., 200.],
            basis: Basis::new(0., 0., 0.),
            missile: false,
        }
    }
    fn scene() -> Scene {
        Scene {
            player: body(0, [0., 1000., 0.]),
            target: Some(1),
            bodies: vec![body(1, [1000., 1000., 0.]), body(2, [0., 1000., 1000.])],
            wings: vec![(1, false, 1, 1), (2, true, 1, 1)],
            missiles: vec![],
        }
    }
    fn camera(rig: &mut Rig, mode: u8, scene: &Scene) -> Camera {
        rig.camera(mode, scene, body_camera(scene.player, mode), [0.; 2], 1.)
            .unwrap()
    }
    fn shot(id: u32, owner: u32, target: Option<u32>, position: Vector) -> Shot {
        Shot {
            body: Body {
                missile: true,
                ..body(id, position)
            },
            owner,
            target,
            incoming: owner != 0,
        }
    }
    fn near(a: Vector, b: Vector) {
        assert!(distance(a, b) < 0.001, "{a:?} != {b:?}");
    }
    #[test]
    fn relations_reverse_endpoints_and_show_the_reference_subject() {
        let scene = scene();
        let mut rig = Rig::default();
        let c = camera(&mut rig, TARGET, &scene);
        near(c.position.map(f64::from), [-180., 1060., 0.]);
        assert!((c.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        let c = camera(&mut rig, TARGET_PLAYER, &scene);
        near(c.position.map(f64::from), [1180., 1060., 0.]);
        assert!((c.yaw + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        near(
            camera(&mut rig, WING, &scene).position.map(f64::from),
            [0., 1060., -180.],
        );
        assert!(!rig.cockpit(TARGET));
    }
    #[test]
    fn tracking_respects_eye_line_and_banked_aircraft_axes() {
        let mut scene = scene();
        let mut rig = Rig::default();
        let c = camera(&mut rig, TRACK, &scene);
        assert!((c.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!(rig.cockpit(TRACK));
        scene.bodies[0].position = [0., 0., 1000.];
        assert_eq!(camera(&mut rig, TRACK, &scene).pitch, 0.);
        scene.player.basis = Basis::new(0., 0., std::f64::consts::FRAC_PI_2);
        let up = scene.player.basis.up;
        scene.bodies[0].position =
            std::array::from_fn(|i| scene.player.position[i] + up[i] * 1000.);
        let c = camera(&mut rig, TRACK, &scene);
        let forward = Basis::new(c.yaw.into(), c.pitch.into(), -f64::from(c.roll)).forward;
        near(forward, up);
    }
    #[test]
    fn fly_by_stays_fixed_until_reselected_and_saved_copy_stays_independent() {
        let mut scene = scene();
        let mut rig = Rig::default();
        let first = camera(&mut rig, FLY_BY, &scene);
        near(first.position.map(f64::from), [300., 1100., 600.]);
        rig.other_pending = true;
        rig.save(FLY_BY, [0.; 2], 2.);
        assert!(!rig.other_pending);
        scene.player.position[2] = 500.;
        let later = camera(&mut rig, FLY_BY, &scene);
        assert_eq!(first.position, later.position);
        assert_ne!(first.yaw, later.yaw);
        rig.select(Reference::Player);
        let next = camera(&mut rig, FLY_BY, &scene);
        assert_eq!(next.position[2], 1100.);
        let stored = rig.other_camera(&scene, Camera::new()).unwrap();
        assert_eq!(stored.position, first.position);
        assert_eq!(stored.zoom, 2.);
        assert_eq!(stored.weather_slot, 3);
    }
    #[test]
    fn threat_chooses_nearest_inbound_not_nearest_outgoing_round() {
        let mut scene = scene();
        let mut rig = Rig::default();
        scene.missiles = vec![
            shot(10, 5, Some(0), [100., 1000., 0.]),
            shot(11, 6, Some(0), [0., 1000., 50.]),
            shot(12, 0, Some(1), [0., 1000., 1.]),
        ];
        let c = camera(&mut rig, THREAT, &scene);
        assert_eq!(c.yaw, 0.);
        near(c.position.map(f64::from), [0., 1060., -180.]);
    }
    #[test]
    fn last_missile_keeps_its_target_and_does_not_revert_to_older_shots() {
        let mut scene = scene();
        let mut rig = Rig::default();
        scene.missiles = vec![
            shot(10, 0, Some(1), [0., 1000., 50.]),
            shot(11, 0, Some(2), [0., 1000., 100.]),
        ];
        let c = camera(&mut rig, MISSILE, &scene);
        near(c.position.map(f64::from), [0., 1010., 70.]);
        scene.target = Some(1);
        assert_eq!(camera(&mut rig, MISSILE, &scene).yaw, 0.);
        scene.missiles.pop();
        // A diagnostic incoming round can share the player shot counter.
        scene.missiles.push(shot(11, 0, Some(0), [0., 1000., 200.]));
        scene.missiles.last_mut().unwrap().incoming = true;
        assert!(
            rig.camera(MISSILE, &scene, Camera::new(), [0.; 2], 1.)
                .is_err()
        );
        assert_eq!(rig.last_missile, Some(11));
    }
    #[test]
    fn relative_cameras_hide_only_the_reference_body_and_unmodified_restores_cockpit() {
        let scene = scene();
        let mut rig = Rig::default();
        rig.select(Reference::Target);
        let c = camera(&mut rig, 0, &scene);
        assert_eq!(c.hidden_target, Some(1));
        assert_eq!(c.position, [1000., 1000., 0.]);
        assert!(!rig.cockpit(0));
        rig.select(Reference::Player);
        assert!(rig.cockpit(0));
        assert_eq!(camera(&mut rig, 0, &scene).hidden_target, None);
    }
    #[test]
    fn absent_and_coincident_subjects_are_bounded_and_saved_view_is_live() {
        let mut scene = scene();
        let mut rig = Rig::default();
        scene.bodies[0].position = scene.player.position;
        let c = camera(&mut rig, TARGET, &scene);
        assert!(
            c.position
                .iter()
                .chain([&c.yaw, &c.pitch])
                .all(|v| v.is_finite())
        );
        rig.save(TARGET, [0.; 2], 1.5);
        scene.bodies[0].position = [1000., 1000., 0.];
        assert!(
            (rig.other_camera(&scene, Camera::new()).unwrap().yaw - std::f32::consts::FRAC_PI_2)
                .abs()
                < 1e-6
        );
        scene.target = None;
        assert!(rig.other_camera(&scene, Camera::new()).is_err());
        for mode in [TRACK, THREAT, TARGET, TARGET_PLAYER, MISSILE] {
            assert!(
                rig.camera(mode, &scene, Camera::new(), [0.; 2], 1.)
                    .is_err()
            );
        }
        scene.wings.clear();
        assert!(
            rig.camera(WING, &scene, Camera::new(), [0.; 2], 1.)
                .is_err()
        );
    }
    #[test]
    fn default_other_view_is_back_and_retains_flight_keys() {
        let scene = scene();
        let mut rig = Rig::default();
        let mut base = body_camera(scene.player, 3);
        base.keys.insert("ArrowDown".into());
        let c = rig.other_camera(&scene, base).unwrap();
        assert!((c.yaw - std::f32::consts::PI).abs() < 1e-6);
        assert!(!rig.other_shows_player());
        assert!(c.keys.contains("ArrowDown"));
        rig.save(1, [0.; 2], 1.);
        assert!(rig.other_shows_player());
    }
}
