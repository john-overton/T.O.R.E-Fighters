//! The HUD camera and the Sensor View on the selected aircraft
//! (nr-replayviews, 2026-10-16; the rules are in
//! [`crate::replay::tape_views`]). Both draw the world from a camera of
//! their own and show it through the CRT pass ([`crate::crt`]) with an
//! overlay, the way the AC-130's sensor console does in flight: the window
//! viewer hands them to the renderer's console pass, the offscreen director
//! to its console pixels.
use super::*;
use crate::replay::tape_views::{self, HUD_CAMERA, SENSOR, Sight, Source};
use crate::sensor_console;
use tore_sim::combat::{gunship, sight::SightSensor};
use tore_sim::combat::{laser::LaserState, live::Readiness};

/// A taped view's frame: its camera, its overlay and how the CRT shows it.
pub(crate) struct Taped {
    pub camera: Camera,
    pub overlay: Vec<u8>,
    pub overlay_size: [u32; 2],
    pub frame: crate::crt::Frame,
}

/// The noise seed of the HUD camera's tape (any fixed number).
const TAPE_SEED: u32 = 0x007a_9e3d;

/// A sight record's own name and sensor: the first laser record of the
/// aircraft's own hardpoints, else the first FLIR (`sight::fixed_sight`'s
/// rule), with the record's display name.
pub(super) fn sight_record(
    profile: &tore_formats::aircraft::Aircraft,
    resources: &BTreeMap<String, Vec<u8>>,
) -> Option<(String, SightSensor)> {
    let mut found: Option<(String, SightSensor)> = None;
    for name in profile
        .hardpoints
        .iter()
        .filter_map(|h| h.store.as_deref())
        .filter(|n| n.to_ascii_uppercase().ends_with(".SEE"))
    {
        let Some(bytes) = resources
            .get(name)
            .or_else(|| resources.get(&name.to_ascii_uppercase()))
            .or_else(|| resources.get(&name.to_ascii_lowercase()))
        else {
            continue;
        };
        let Ok(seeker) = tore_formats::weapons::Seeker::parse(name, bytes) else {
            continue;
        };
        let equipment = tore_formats::aircraft::Equipment::parse(name, bytes).ok();
        let weight = equipment
            .as_ref()
            .and_then(|e| e.fields.get("weight"))
            .and_then(|w| w.number().ok())
            .unwrap_or(0);
        let Some(sensor) = SightSensor::from_record(&seeker, weight) else {
            continue;
        };
        let label = equipment
            .map(|e| e.name)
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| name.trim_end_matches(".SEE").to_owned());
        if found.as_ref().is_none_or(|(_, f)| sensor.laser && !f.laser) {
            found = Some((tore_formats::text::upper(&label), sensor));
        }
    }
    found
}

impl Viewer {
    /// The sights of the recording's aircraft types and their guns, read
    /// from the import once when the viewer opens.
    pub(super) fn load_sights(&mut self, resources: &BTreeMap<String, Vec<u8>>) {
        for model in std::iter::once(&self.ownship).chain(&self.models) {
            if !self.sights.iter().any(|(id, _)| *id == model.profile.id) {
                let sight = sight_record(&model.profile, resources);
                log::info!(
                    "Replay: {} sight {:?}",
                    model.profile.id.label(),
                    sight.as_ref().map(|(name, s)| (name, s.laser))
                );
                self.sights.push((model.profile.id, sight));
            }
            if self.guns.iter().any(|(id, ..)| *id == model.profile.id) {
                continue;
            }
            let configuration =
                tore_sim::combat::live::Configuration::from_source(&model.profile, |name| {
                    resources
                        .get(name)
                        .cloned()
                        .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
                });
            if let Some(station) = configuration.ok().and_then(|c| {
                c.stations
                    .iter()
                    .find(|s| tore_sim::combat::live::is_gun(&s.weapon))
                    .cloned()
            }) {
                self.guns
                    .push((model.profile.id, station.weapon.clone(), station.mount));
            }
        }
        // Types the recording names but draws with another model (an
        // air-to-ground run lists none): their sight from their own records.
        let kinds: Vec<AircraftId> = self.kinds.values().copied().collect();
        for kind in kinds {
            if self.sights.iter().any(|(id, _)| *id == kind) {
                continue;
            }
            if let Ok(loaded) = crate::aircraft_type::AircraftType::load(resources, kind) {
                self.sights
                    .push((kind, sight_record(&loaded.profile, resources)));
            }
        }
        for name in gunship::GUNS {
            self.store_weapon(name, resources);
        }
        let names: Vec<String> = self
            .recording
            .events()
            .iter()
            .filter(|event| event.event.kind == tore_replay::vocab::kind::AIRCRAFT_HUD)
            .filter_map(|event| event.event.string(tore_replay::vocab::field::WEAPON))
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect();
        for name in names {
            self.store_weapon(&name, resources);
        }
        for model in std::iter::once(&self.ownship).chain(&self.models) {
            if self
                .hud_configs
                .iter()
                .any(|(id, _)| *id == model.profile.id)
            {
                continue;
            }
            let Ok(config) =
                tore_sim::combat::live::Configuration::from_source(&model.profile, |name| {
                    resources
                        .get(name)
                        .cloned()
                        .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
                })
            else {
                continue;
            };
            self.hud_configs.push((model.profile.id, config));
        }
    }

    fn store_weapon(&mut self, name: &str, resources: &BTreeMap<String, Vec<u8>>) {
        if self.hud_weapons.contains_key(name) {
            return;
        }
        let Some(bytes) = resources
            .get(name)
            .or_else(|| resources.get(&name.to_ascii_uppercase()))
            .or_else(|| resources.get(&name.to_ascii_lowercase()))
        else {
            return;
        };
        if let Ok(weapon) = tore_formats::weapons::Weapon::parse(name, bytes) {
            self.hud_weapons.insert(weapon.source.clone(), weapon);
        }
    }

    /// The aircraft's configuration with the recorded weapon on its station.
    /// `None` when the recorded weapon cannot be loaded, so the HUD camera
    /// keeps the derived picture rather than naming another store.
    fn hud_config(
        &self,
        id: u32,
        sample: &tore_replay::display::HudSample,
    ) -> Option<tore_sim::combat::live::Configuration> {
        let kind = self.type_of(id).unwrap_or(self.ownship.profile.id);
        let mut config = self
            .hud_configs
            .iter()
            .find(|(stored, _)| *stored == kind)?
            .1
            .clone();
        let index = usize::from(sample.station);
        let weapon = match self.hud_weapons.get(&sample.weapon) {
            Some(weapon) => Some(weapon),
            None if sample.weapon.is_empty() => None,
            None => {
                let carried = config.stations.get(index)?;
                if !carried.weapon.source.eq_ignore_ascii_case(&sample.weapon) {
                    return None;
                }
                None
            }
        };
        if let Some(weapon) = weapon {
            if config.stations.len() <= index
                && let Some(first) = config.stations.first().cloned()
            {
                while config.stations.len() <= index {
                    config.stations.push(first.clone());
                }
            }
            if let Some(station) = config.stations.get_mut(index) {
                station.weapon = weapon.clone();
                if sample.mount != [0.; 3] {
                    station.mount = sample.mount;
                }
            }
        }
        config.stations.get(index).is_some().then_some(config)
    }

    /// The airframe drawn for aircraft type `id`.
    fn airframe(&self, id: Option<AircraftId>) -> &Airframe {
        id.and_then(|id| {
            std::iter::once(&self.ownship)
                .chain(&self.models)
                .find(|m| m.profile.id == id)
        })
        .unwrap_or(&self.ownship)
    }

    /// The right-hand target block for a recorded sight.
    fn sight_target(
        &self,
        picture: &RenderSnapshot,
        state: &flight::State,
        tick: u64,
        sample: &tore_replay::display::SightSample,
    ) -> Option<sensor_console::TargetLine> {
        let (name, at) = if let Some(id) = sample.tracked {
            let at = self
                .target_view(picture, id)
                .map(|target| target.position)
                .or_else(|| self.object_at(picture, tick, id))?;
            let name = self
                .info
                .get(&id)
                .map(|info| info.label.clone())
                .unwrap_or_else(|| self.object_name(Target::Ground(id)));
            (name, at)
        } else {
            ("POINT".into(), sample.point?)
        };
        Some(sensor_console::TargetLine {
            name: tore_formats::text::upper(&name),
            range_ft: feet(state.position, at),
            clock: String::new(),
            bearing_deg: (at[0] - state.position[0])
                .atan2(at[2] - state.position[2])
                .to_degrees()
                .rem_euclid(360.),
        })
    }

    /// A recorded target's pose, for the HUD box and the range line.
    fn target_view(
        &self,
        picture: &RenderSnapshot,
        id: u32,
    ) -> Option<crate::replay::hud_play::TargetView> {
        let pose = if picture.player.id == id {
            &picture.player
        } else {
            picture.target(id)?
        };
        Some(crate::replay::hud_play::TargetView {
            id,
            position: pose.position,
            velocity: pose.velocity,
            aircraft: pose.aircraft,
        })
    }

    /// The type of aircraft `id`, as the recording names it.
    fn type_of(&self, id: u32) -> Option<AircraftId> {
        self.kinds.get(&id).copied()
    }

    /// The Sensor View's sight on aircraft `id`, or `None` when it has none.
    pub(crate) fn sight_of(&self, id: u32) -> Option<Sight> {
        let kind = self.type_of(id)?;
        tape_views::sight_for(
            kind,
            self.sights
                .iter()
                .find(|(id, _)| *id == kind)
                .and_then(|(_, sight)| sight.as_ref()),
            self.marks.lased(id),
        )
    }

    /// V and Shift+V, and the menus: the HUD camera or the Sensor View on
    /// the selected aircraft. The Sensor View says so and stays put on an
    /// aircraft without a sight.
    pub(crate) fn taped_view_key(&mut self, view: u8) {
        if view == SENSOR && self.sight_of(self.selected).is_none() {
            let who = self.label(self.selected);
            self.toast(format!(
                "{who} has no targeting sight: no Sensor View (a laser pod, a laser or a FLIR)"
            ));
            return;
        }
        self.show_view(view, Reference::Aircraft(self.selected));
        if view == HUD_CAMERA {
            self.zoom = 1.;
        }
    }

    /// The picture keys of the Sensor View: 1 to 4, Shift+F5 to Shift+F8,
    /// and Backslash to step through them. True when taken.
    pub(crate) fn sensor_mode_key(&mut self, name: &str, shift: bool) -> bool {
        let mode = match (name, tape_views::mode_key(name, shift)) {
            (_, Some(mode)) => mode,
            ("\\", None) if self.view == SENSOR && self.drone.is_none() => {
                self.sensor_mode.cycled()
            }
            _ => return false,
        };
        // Digits only mean a picture inside the view; Shift+F5 to F8 open it.
        if self.view != SENSOR || self.drone.is_some() {
            if !shift {
                return false;
            }
            self.sensor_mode = mode;
            self.taped_view_key(SENSOR);
        } else {
            self.sensor_mode = mode;
        }
        self.toast(format!("Sensor picture: {}", mode.label()));
        true
    }

    /// The HUD camera's or the Sensor View's frame on the selected aircraft
    /// at `tick`, for a view of `size`; `None` for the other views, a drone,
    /// or an aircraft that is not in the recording now (the caller shows it
    /// from outside).
    pub(crate) fn taped_view(
        &mut self,
        picture: &RenderSnapshot,
        tick: u64,
        size: [u32; 2],
    ) -> Option<Taped> {
        if self.drone.is_some() || !matches!(self.view, HUD_CAMERA | SENSOR) {
            return None;
        }
        let id = self.selected;
        let pose = if id == self.player {
            Some(&picture.player)
        } else {
            picture.target(id)
        };
        let Some(pose) = pose.filter(|p| !p.crashed) else {
            self.taped_failed("That aircraft is not in the recording now");
            return None;
        };
        let model = self.airframe(pose.aircraft.or_else(|| self.type_of(id)));
        let mut state = snapshot::pose_state(&model.start(&self.world), pose);
        if let Some(now) = self.playback.aircraft(tick, id) {
            state.speed = now.airspeed;
            state.g = now.g;
            state.fuel = now.fuel_lb;
            state.vertical_speed = now.velocity[1];
            state.throttle = now.controls[3];
        }
        state.ticks = tick;
        let taped = if self.view == HUD_CAMERA {
            Some(self.hud_camera(picture, tick, size, id, &state))
        } else {
            match self.sight_of(id) {
                Some(sight) => Some(self.sensor_view(picture, tick, size, id, &state, &sight)),
                None => {
                    let who = self.label(id);
                    self.taped_failed(if who.is_empty() {
                        "No targeting sight on this aircraft"
                    } else {
                        "This aircraft has no targeting sight"
                    });
                    None
                }
            }
        };
        if taped.is_some() {
            self.camera_error = None;
        }
        taped
    }

    fn taped_failed(&mut self, reason: &'static str) {
        if self.camera_error != Some(reason) {
            self.camera_error = Some(reason);
            self.toast(reason);
        }
    }

    /// The HUD camera: the pilot's eye looking along the nose, the HUD drawn
    /// as the recorded aircraft's HUD shows it (the replay's HUD as the
    /// promo director builds it: flight data, the target box and the gun
    /// pipper on an air target, the laser rows and the time to impact),
    /// with the recorder's lines, on tape.
    fn hud_camera(
        &mut self,
        picture: &RenderSnapshot,
        tick: u64,
        size: [u32; 2],
        id: u32,
        state: &flight::State,
    ) -> Taped {
        let model = self.airframe(Some(state_kind(self, picture, id)));
        let mut camera = Camera::new();
        camera.position = crate::mirrors::pilot_eye(
            state.position,
            Basis::new(state.yaw, state.pitch, state.bank),
        );
        camera.yaw = state.yaw as f32;
        camera.pitch = state.pitch as f32;
        camera.roll = -state.bank as f32;
        camera.hidden_target = Some(id);
        camera.zoom = tape_views::hud_camera_zoom() * self.zoom;
        let palette = model.cockpit_palette(&self.world, &self.scenery, camera.position[1], 0);
        let color = palette[usize::from(model.hud.primary_color)];
        // The HUD's own projection at the cockpit's unmagnified scale; the
        // raster is then scaled with the camera, as the cockpit pass does.
        let [w, h] = size.map(f64::from);
        let hud_zoom =
            (1. / crate::flight_canvas::HUD_SCALE) * (h / (480. * (w / 640.).min(h / 480.)));
        let mut raster = vec![0u8; 640 * 480 * 4];
        let sample = self.display.hud(id, tick).cloned();
        crate::hud::draw(
            &mut raster,
            state,
            &model.hud_font,
            f64::from(
                self.world
                    .height(state.position[0] as f32, state.position[2] as f32),
            ),
            self.world.air_data(state).ok().as_ref(),
            true,
            sample.as_ref().is_some_and(|sample| !sample.nav),
            color,
            hud_zoom as f32,
            None,
            None,
            (state.bank, 1.),
        );
        let scene = self.marks.scene(tick);
        let laser = self.laser_readout(&scene, tick, id, state.position);
        let tti = tape_views::tti(&self.marks.timeline_of(id), tick);
        if let Some(sample) = sample.as_ref()
            && let Some(config) = self.hud_config(id, sample)
        {
            let target = sample
                .target
                .and_then(|target| self.target_view(picture, target));
            let aim = self
                .display
                .sight(id, tick)
                .filter(|sight| sight.gunship)
                .and_then(|sight| sight.point.or(sight.gun_lock));
            let falling = self.marks.timeline_of(id).iter().find_map(|weapon| {
                weapon
                    .landed
                    .filter(|landed| weapon.released <= tick && *landed > tick)
                    .map(|landed| (landed - tick) as f64 / 120.)
            });
            let terrain = |x: f64, z: f64| f64::from(self.world.height(x as f32, z as f32));
            crate::replay::hud_play::draw(
                &mut raster,
                state,
                &config,
                sample,
                target,
                aim,
                &model.hud_font,
                color,
                hud_zoom,
                tick,
                falling,
                laser,
                &terrain,
            );
        } else {
            // An older recording, or a weapon the import does not have: the
            // derived box, the gun pipper and the laser rows.
            if let Some(target) = self
                .target_of(id, tick)
                .flatten()
                .and_then(|t| picture.target(t))
            {
                crate::weapon_hud::replay_target(
                    &mut raster,
                    state,
                    target.position,
                    color,
                    hud_zoom,
                    false,
                );
                if let Some((_, gun, mount)) =
                    self.guns.iter().find(|(k, ..)| *k == model.profile.id)
                {
                    crate::weapon_hud::replay_gunsight(
                        &mut raster,
                        state,
                        color,
                        hud_zoom,
                        gun,
                        *mount,
                        tore_sim::combat::gunsight::TargetObservation {
                            position: target.position,
                            velocity: target.velocity,
                        },
                    );
                }
            } else if let Some(at) = self.ground_target(picture, tick, id) {
                crate::weapon_hud::replay_target(&mut raster, state, at, color, hud_zoom, false);
            }
            if laser.is_some() || tti.is_some() {
                crate::weapon_hud::laser::draw_rows(
                    &mut raster,
                    state,
                    laser.as_ref(),
                    tick,
                    tti.as_deref(),
                    &model.hud_font,
                    color,
                    hud_zoom,
                );
            }
        }
        let overlay_size = tape_views::overlay_size(size);
        let mut overlay = tape_views::hud_tape_layer(&raster, overlay_size, camera.zoom);
        let kind = model.profile.id.label();
        let (lines, scale) = tape_views::recorder_lines(tick, &self.label(id), kind, overlay_size);
        tape_views::over(
            &mut overlay,
            &sensor_console::captions(overlay_size, &lines, scale),
        );
        Taped {
            camera,
            overlay,
            overlay_size,
            frame: {
                let mut frame = crate::crt::Frame::tape(tick, TAPE_SEED);
                // A developer switch for the smoothing's before-and-after
                // renders.
                if std::env::var_os("TORE_REPLAY_TAPE_SMOOTHING").is_some_and(|v| v == "0")
                    && let Some((tape, _)) = &mut frame.tape
                {
                    *tape = crate::crt::VHS_SHARP;
                }
                frame
            },
        }
    }

    /// The laser as the aircraft's HUD would show it, from the recording:
    /// its own laser's state and spot, and a buddy's spot its weapon follows.
    fn laser_readout(
        &self,
        scene: &ground_marks::Scene,
        tick: u64,
        id: u32,
        from: [f64; 3],
    ) -> Option<tore_world::readout::LaserReadout> {
        use tore_world::readout::{LaserReadout, SpotReadout};
        let range = |at: [f64; 3]| feet(from, at);
        let spot_of = |owner: u32, own: bool| {
            scene
                .spots
                .iter()
                .find(|s| s.owner == owner && s.lasing)
                .map(|s| SpotReadout {
                    spot: tore_sim::combat::laser::LaserSpot {
                        owner,
                        side: tore_sim::combat::live::Side(0),
                        position: s.at,
                        target: None,
                    },
                    own,
                    range_ft: range(s.at),
                })
        };
        let state = self.marks.laser_state(id, tick);
        let own = spot_of(id, true);
        let buddy = scene
            .weapons
            .iter()
            .filter(|w| w.owner == id)
            .find_map(|w| w.spot_owner.filter(|o| *o != id))
            .and_then(|owner| spot_of(owner, false));
        if state.is_none() && buddy.is_none() {
            return None;
        }
        Some(LaserReadout {
            designator: state.is_some(),
            state: match state.map(|s| s.0) {
                Some("LASE") => LaserState::Lasing,
                Some("LSR RDY") => LaserState::Ready,
                Some("MASK") => LaserState::Masked,
                _ => LaserState::Off,
            },
            mask: None,
            manual: None,
            range_ft: own.as_ref().map_or(0., |s| s.range_ft),
            limit_ft: tape_views::pave_spike().range_ft,
            spot: own.or(buddy),
            buddy: None,
        })
    }

    /// Where an object is now for the Sensor View: an aircraft or a ground
    /// object.
    fn object_at(&self, picture: &RenderSnapshot, tick: u64, id: u32) -> Option<[f64; 3]> {
        self.body_of(picture, tick, Target::Aircraft(id))
            .or_else(|| self.body_of(picture, tick, Target::Ground(id)))
            .map(|b| b.position())
    }

    /// The ground target of aircraft `id` at `tick`: what it designated, else
    /// the ground object its recorded target names.
    fn ground_target(&self, picture: &RenderSnapshot, tick: u64, id: u32) -> Option<[f64; 3]> {
        let scene = self.marks.scene(tick);
        scene
            .designations
            .iter()
            .find(|d| d.aircraft == id)
            .and_then(|d| {
                d.object
                    .and_then(|o| self.object_at(picture, tick, o))
                    .or(d.at)
            })
            .or_else(|| self.recorded_ground_target(picture, tick, id))
    }

    /// The ground object the recording gives as aircraft `id`'s target at
    /// `tick` (an AI's `ai.target`, or the player's view target), where it
    /// is now.
    fn recorded_ground_target(
        &self,
        picture: &RenderSnapshot,
        tick: u64,
        id: u32,
    ) -> Option<[f64; 3]> {
        self.target_of(id, tick)
            .flatten()
            .and_then(|t| self.body_of(picture, tick, Target::Ground(t)))
            .map(|b| b.position())
    }

    /// The AC-130's orbit centre at `tick` while its console's orbit runs
    /// (`player.orbit`).
    fn orbit_of(&self, id: u32, tick: u64) -> Option<([f64; 3], f64, f64)> {
        let last = self
            .recording
            .events()
            .iter()
            .take_while(|e| e.tick <= tick)
            .filter(|e| e.event.kind == vocab::kind::PLAYER_ORBIT && e.event.subject == Some(id))
            .last()?;
        let e = &last.event;
        if e.flag("on") != Some(true) {
            return None;
        }
        Some((
            [e.num("x_ft")?, e.num("y_ft")?, e.num("z_ft")?],
            e.num("radius_ft").unwrap_or(0.),
            e.num("height_ft").unwrap_or(0.),
        ))
    }

    /// The Sensor View: the world from the sight's eye along the line the
    /// recording gives, in the chosen picture, with the AC-130's console
    /// symbology or the aircraft's own.
    fn sensor_view(
        &mut self,
        picture: &RenderSnapshot,
        tick: u64,
        size: [u32; 2],
        id: u32,
        state: &flight::State,
        sight: &Sight,
    ) -> Taped {
        let launcher = tore_world::combat::launcher(state);
        let scene = self.marks.scene(tick);
        let orbit = self.orbit_of(id, tick);
        let object = |object: u32| self.object_at(picture, tick, object);
        let line = tape_views::sight_line(&tape_views::Inputs {
            aircraft: id,
            scene: &scene,
            object: &object,
            orbit: orbit.map(|o| o.0),
            rounds: self
                .ground_impacts
                .get(&id)
                .and_then(|list| tape_views::rounds_point(list, tick)),
            target: self.recorded_ground_target(picture, tick, id),
        });
        let (eye, default_look, gimbal) = match sight {
            Sight::Console => (gunship::eye_position(launcher), gunship::DEFAULT_LOOK, None),
            Sight::Pod { sensor, .. } => (
                crate::gunsight_view::sensor_eye(&launcher, sensor.eye),
                sensor.default_look,
                Some(sensor.gimbal),
            ),
        };
        // A recorded sight (nr-replayhud) points the camera. Without one, the
        // line of sight stays the derived one, and the console says so.
        let recorded = self
            .display
            .sight(id, tick)
            .cloned()
            .filter(crate::replay::hud_play::use_look);
        let wanted = if let Some(sample) = &recorded {
            [sample.az, sample.el]
        } else {
            line.point.map_or(default_look, |p| {
                tore_sim::combat::sight::body_angles(
                    launcher,
                    std::array::from_fn(|i| p[i] - eye[i]),
                )
            })
        };
        let (look, gimbal_limit) = match gimbal {
            None => (gunship::clamp_look(wanted), gunship::beyond_gimbal(wanted)),
            Some(g) => (g.clamp(wanted), g.beyond(wanted)),
        };
        // Where the line meets the ground when nothing gives a point: the
        // range and the gimbal graphic read from it.
        let terrain = |x: f64, z: f64| f64::from(self.world.height(x as f32, z as f32));
        let aim = recorded
            .as_ref()
            .and_then(|sample| sample.point.or(sample.gun_lock))
            .or(line.point)
            .or_else(|| {
                let direction = match sight {
                    Sight::Console => gunship::direction(launcher, look[0], look[1]),
                    Sight::Pod { .. } => {
                        tore_sim::combat::sight::direction(launcher, look[0], look[1])
                    }
                };
                tore_sim::combat::sight::sight_ground(eye, direction, &terrain)
            });
        let slant = aim.map(|p| feet(eye, p));
        // No recorded zoom: the sight's default step, 7.5 degrees.
        let step = recorded
            .as_ref()
            .map(|sample| sample.zoom.max(1))
            .unwrap_or(tore_sim::combat::sight::DEFAULT_ZOOM);
        let view = match sight {
            Sight::Console => crate::gunsight_view::free_view(&launcher, look, step),
            Sight::Pod { sensor, .. } => {
                crate::gunsight_view::sensor_view(&launcher, sensor.eye, look, step)
            }
        };
        let mut camera = crate::gunsight_view::camera(&view);
        camera.zoom *= self.zoom;
        camera.weather_slot = 0;
        camera.thermal = self.sensor_mode.thermal();
        camera.sensor = true;
        camera.near_clip = 1.;
        camera.hidden_target = Some(id);
        let air = self.world.air_data(state).ok();
        let ground = f64::from(
            self.world
                .height(state.position[0] as f32, state.position[2] as f32),
        );
        let agl_ft = air
            .as_ref()
            .map_or(state.position[1] - ground, |a| a.altitude_agl_ft);
        let heading_deg = air
            .as_ref()
            .map_or(state.yaw.to_degrees(), |a| a.heading_true_deg);
        let airspeed_kt = air
            .as_ref()
            .map_or(state.speed / 1.68781, |a| a.true_airspeed_knots);
        let fov_deg = tape_views::field_deg(camera.zoom);
        let los_deg = f64::from(camera.yaw).to_degrees().rem_euclid(360.);
        let seconds = tick as f64 / 120.;
        let blink = tick % 60 < 30;
        let designated = scene
            .designations
            .iter()
            .find(|d| d.aircraft == id)
            .and_then(|d| {
                d.object
                    .and_then(|o| self.object_at(picture, tick, o))
                    .or(d.at)
            });
        let target = scene
            .designations
            .iter()
            .find(|d| d.aircraft == id)
            .and_then(|d| {
                let at = d
                    .object
                    .and_then(|o| self.object_at(picture, tick, o))
                    .or(d.at)?;
                Some(sensor_console::TargetLine {
                    name: tore_formats::text::upper(&match d.object {
                        Some(o) => self.object_name(Target::Ground(o)),
                        None => "POINT".into(),
                    }),
                    range_ft: feet(state.position, at),
                    clock: String::new(),
                    bearing_deg: (at[0] - state.position[0])
                        .atan2(at[2] - state.position[2])
                        .to_degrees()
                        .rem_euclid(360.),
                })
            });
        let overlay_size = tape_views::overlay_size(size);
        let overlay = match sight {
            Sight::Console => {
                let recorded_guns = recorded.as_ref().filter(|sample| sample.gunship);
                let mode = recorded_guns
                    .map(crate::replay::hud_play::console_mode)
                    .unwrap_or(match line.source {
                        Source::Designated => sensor_console::Mode::Track,
                        Source::Default => sensor_console::Mode::Manual,
                        _ => sensor_console::Mode::Point,
                    });
                let (guns, lead) = recorded_guns
                    .map(crate::replay::hud_play::gun_lines)
                    .unwrap_or_else(|| {
                        let gun = |slot: usize| sensor_console::GunLine {
                            label: gunship::NAMES[slot],
                            fitted: true,
                            linked: true,
                            candidate: false,
                            rounds: None,
                            status: Readiness::Ready,
                        };
                        (std::array::from_fn(gun), None)
                    });
                let weapons = std::array::from_fn(|slot| self.hud_weapons.get(gunship::GUNS[slot]));
                let pipper = recorded_guns.and_then(|sample| {
                    crate::replay::hud_play::pipper(sample, &weapons, &launcher, tick, &terrain)
                });
                let trains = recorded_guns
                    .and_then(|sample| sample.train)
                    .unwrap_or([look; 3]);
                let gun_lock = recorded_guns.and_then(|sample| sample.gun_lock);
                let notice = if recorded_guns.is_some() {
                    None
                } else {
                    Some(line.source.notice())
                };
                let radar = recorded_guns.map_or(
                    sensor_console::Radar {
                        on: false,
                        range_nm: sensor_console::radar_range_nm(0),
                        dots: Vec::new(),
                    },
                    |sample| sensor_console::Radar {
                        on: sample.radar,
                        range_nm: sensor_console::radar_range_nm(usize::from(sample.range)),
                        dots: Vec::new(),
                    },
                );
                let target = recorded_guns
                    .and_then(|sample| self.sight_target(picture, state, tick, sample))
                    .or(target);
                let orbit_status = orbit.map(|(centre, radius_ft, height_ft)| {
                    let across =
                        (state.position[0] - centre[0]).hypot(state.position[2] - centre[2]);
                    sensor_console::OrbitStatus {
                        settled: (across - radius_ft).abs() < radius_ft / 4.,
                        radius_ft,
                        height_ft,
                        bearing: true,
                    }
                });
                let sym = sensor_console::Symbology {
                    sensor: self.sensor_mode,
                    mode,
                    zoom: step,
                    fov_deg,
                    heading_deg,
                    agl_ft,
                    airspeed_kt,
                    look,
                    los_deg,
                    slant_ft: slant,
                    target,
                    guns,
                    lead,
                    trains,
                    gimbal_limit,
                    pipper,
                    others: Vec::new(),
                    mark: recorded_guns
                        .and_then(|sample| sample.point.or(sample.gun_lock))
                        .or(line.point)
                        .map(|p| {
                            (
                                p,
                                recorded_guns.is_some_and(|sample| sample.mode == "track")
                                    || line.source == Source::Designated,
                            )
                        }),
                    // No recorded sight: the status line says where the line
                    // of sight was derived from. A recorded sight shows the gun.
                    notice,
                    orbit: orbit_status,
                    designated,
                    radar,
                    plan: sensor_console::Plan::new(launcher, agl_ft, look, trains, aim, None),
                    autopilot: None,
                    gun_lock,
                    blink,
                };
                sensor_console::draw(&sym, &camera, overlay_size)
            }
            Sight::Pod { sensor, .. } => {
                let own_spot = scene.spots.iter().find(|s| s.owner == id);
                let laser = sensor.laser.then(|| {
                    let (state_word, mask) =
                        self.marks.laser_state(id, tick).unwrap_or(("OFF", None));
                    sensor_console::PodLaser {
                        state: state_word,
                        range_ft: own_spot.filter(|s| s.lasing).map(|s| feet(eye, s.at)),
                        mask,
                    }
                });
                let buddy = scene
                    .weapons
                    .iter()
                    .filter(|w| w.owner == id)
                    .find_map(|w| w.spot_owner.filter(|o| *o != id))
                    .map(|owner| {
                        let range = scene
                            .spots
                            .iter()
                            .find(|s| s.owner == owner)
                            .map(|s| {
                                format!(
                                    " {:.1}NM",
                                    feet(state.position, s.at)
                                        / tore_sim::sensors::FEET_PER_NAUTICAL_MILE
                                )
                            })
                            .unwrap_or_default();
                        tore_formats::text::upper(&format!("BUDDY {}{range}", self.label(owner)))
                    });
                let weapon = self.weapon_of(id, tick).map(|w| format!("WPN {w}"));
                let impact = scene
                    .impacts
                    .iter()
                    .filter(|i| i.owner == id)
                    .next_back()
                    .map(|i| i.label());
                let mut marks = Vec::new();
                for spot in &scene.spots {
                    let mine = spot.owner == id;
                    let followed = scene
                        .weapons
                        .iter()
                        .any(|w| w.owner == id && w.spot_owner == Some(spot.owner));
                    if mine || followed {
                        marks.push((spot.at, sensor_console::PodMark::Spot { own: mine }));
                    }
                }
                if let Some(at) = designated {
                    marks.push((at, sensor_console::PodMark::Designated));
                }
                for w in scene.weapons.iter().filter(|w| w.owner == id) {
                    if let Some(aim) = w.aim {
                        marks.push((aim, sensor_console::PodMark::Aim));
                    }
                }
                for i in scene.impacts.iter().filter(|i| i.owner == id) {
                    marks.push((i.at, sensor_console::PodMark::Impact));
                }
                let kind = self
                    .type_of(id)
                    .unwrap_or(self.airframe(None).profile.id)
                    .label();
                let sym = sensor_console::PodSymbology {
                    sensor: self.sensor_mode,
                    mode: recorded
                        .as_ref()
                        .map(crate::replay::hud_play::pod_mode)
                        .unwrap_or_else(|| line.source.mode()),
                    zoom: step,
                    fov_deg,
                    aircraft: tore_formats::text::upper(&format!("{}  {kind}", self.label(id))),
                    heading_deg,
                    agl_ft,
                    airspeed_kt,
                    look,
                    los_deg,
                    slant_ft: slant,
                    gimbal_limit,
                    laser,
                    buddy,
                    target: recorded
                        .as_ref()
                        .and_then(|sample| self.sight_target(picture, state, tick, sample))
                        .or(target),
                    weapon,
                    tti: tape_views::tti(&self.marks.timeline_of(id), tick),
                    impact,
                    notice: if recorded.is_some() {
                        None
                    } else {
                        Some(line.source.notice().into())
                    },
                    marks,
                    blink,
                };
                sensor_console::draw_pod(&sym, &camera, overlay_size)
            }
        };
        let night = sensor_console::night(&self.world, camera.position[1]);
        Taped {
            frame: sensor_console::crt_frame(seconds, 0., self.sensor_mode, night),
            camera,
            overlay,
            overlay_size,
        }
    }

    /// The weapon aircraft `id` last let go of at or before `tick`: the
    /// selected weapon as far as the recording shows it.
    fn weapon_of(&self, id: u32, tick: u64) -> Option<String> {
        self.recording
            .events()
            .iter()
            .take_while(|e| e.tick <= tick)
            .filter(|e| {
                e.event.kind == vocab::kind::WEAPON_LAUNCH
                    && e.event.subject == Some(id)
                    && e.event.string(vocab::field::CLASS) != Some("gun")
            })
            .last()
            .and_then(|e| e.event.id(vocab::field::PROJECTILE))
            .and_then(|p| self.missiles.get(&p))
            .map(|(_, name)| tore_formats::text::upper(name))
    }
}

/// The type of aircraft `id` in `picture`.
fn state_kind(viewer: &Viewer, picture: &RenderSnapshot, id: u32) -> AircraftId {
    let pose = if id == viewer.player {
        Some(&picture.player)
    } else {
        picture.target(id)
    };
    pose.and_then(|p| p.aircraft)
        .or_else(|| viewer.type_of(id))
        .unwrap_or(viewer.ownship.profile.id)
}
