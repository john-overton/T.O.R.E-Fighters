//! Promo camera and surface-free presentation. Authoritative state stays recorded.
use super::*;
use crate::reel::{Frame, Gpu};
impl Viewer {
    pub fn director_guns(
        &self,
        gpu: &mut Gpu,
        resources: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<()> {
        for model in std::iter::once(&self.ownship).chain(&self.models) {
            if gpu
                .gun_models
                .iter()
                .any(|(id, ..)| *id == model.profile.id)
            {
                continue;
            }
            let combat = crate::combat::Combat::new(model, resources, false)?;
            if let Some(station) = combat
                .state
                .configuration()
                .stations
                .iter()
                .find(|s| tore_sim::combat::live::is_gun(&s.weapon))
            {
                gpu.gun_models
                    .push((model.profile.id, station.weapon.clone(), station.mount));
            }
        }
        Ok(())
    }
    pub fn director_frame(
        &mut self,
        gpu: &mut Gpu,
        frame: &Frame,
        previous: Option<&Frame>,
        audio: &crate::audio::Audio,
    ) -> AppResult<Vec<u8>> {
        let cockpit_view = matches!(frame.view, 1 | 2);
        self.clock.seek(frame.tick);
        let tick = self.clock.tick();
        let picture = self.playback.picture(tick, self.clock.alpha());
        self.weather
            .build(&mut self.world, &self.tracks, self.clock.last());
        self.weather.seek(&mut self.world, &self.tracks, tick);
        if let Some(minutes) = frame.time_minutes {
            let config = self.world.weather.configuration();
            let resources = crate::reel::visual_weather_module(&self.world)?;
            self.world.weather =
                tore_sim::environment::Environment::new(tore_sim::environment::Configuration::new(
                    resources,
                    minutes / 60,
                    minutes % 60,
                    config.parameter(),
                    crate::terrain::Recorded::from_identity(&self.recording.header().world)?.wind,
                )?);
        }
        let player = self.player_state(&picture, tick);
        // u32::MAX is the recipe's static first-runway camera anchor.
        let (position, heading) = if frame.anchor == u32::MAX {
            let runway = self
                .world
                .airport_scene
                .runways
                .first()
                .ok_or("theater has no runway camera anchor")?;
            (runway.surface.center, runway.heading)
        } else {
            let pose = if frame.anchor == 0 {
                &picture.player
            } else {
                picture
                    .target(frame.anchor)
                    .ok_or("director anchor absent")?
            };
            (pose.position, pose.attitude[0])
        };
        let frame_of = |position: [f64; 3], heading: f64| {
            let (sin, cos) = heading.sin_cos();
            move |p: [f64; 3]| {
                [
                    position[0] + p[0] * cos + p[2] * sin,
                    position[1] + p[1],
                    position[2] - p[0] * sin + p[2] * cos,
                ]
            }
        };
        let place = frame_of(position, heading);
        // A fixed eye stays where the anchor's recorded frame put it at
        // eye_tick; the look target still follows the anchor.
        let place_eye = match frame.eye_tick {
            Some(at) if frame.anchor != u32::MAX => {
                let state = self
                    .playback
                    .aircraft(at, frame.anchor)
                    .ok_or("fixed-eye anchor absent at its tick")?;
                frame_of(state.position, state.attitude[0])
            }
            _ => place,
        };
        let watched_pose = if frame.anchor == 0 {
            Some(&picture.player)
        } else {
            picture.target(frame.anchor)
        };
        let watched_now = if cockpit_view {
            self.playback.aircraft(tick, frame.anchor)
        } else {
            None
        };
        let watched_model = watched_pose
            .and_then(|p| p.aircraft)
            .and_then(|id| {
                if self.ownship.profile.id == id {
                    Some(&self.ownship)
                } else {
                    self.models.iter().find(|m| m.profile.id == id)
                }
            })
            .unwrap_or(&self.ownship);
        let mut watched =
            watched_pose.map(|p| render_snapshot::pose_state(&watched_model.start(&self.world), p));
        if let (Some(state), Some(now)) = (&mut watched, &watched_now) {
            state.speed = now.airspeed;
            state.g = now.g;
            state.fuel = now.fuel_lb;
            state.vertical_speed = now.velocity[1];
            state.ticks = tick;
            state.throttle = now.controls[3];
        }
        let mut camera = Camera::new();
        camera.position = place_eye(frame.eye);
        let target = place(frame.target);
        let d: [f64; 3] = std::array::from_fn(|i| target[i] - camera.position[i]);
        camera.yaw = d[0].atan2(d[2]) as f32;
        camera.pitch = d[1].atan2(d[0].hypot(d[2])) as f32;
        camera.zoom = ((30f64).to_radians().tan() / (frame.fov / 2.).to_radians().tan()) as f32;
        if cockpit_view {
            let state = watched
                .as_ref()
                .ok_or("cockpit requires a recorded aircraft anchor")?;
            camera.position = crate::mirrors::pilot_eye(
                state.position,
                Basis::new(state.yaw, state.pitch, state.bank),
            );
            camera.yaw = state.yaw as f32;
            camera.pitch = state.pitch as f32;
            camera.roll = -state.bank as f32;
            camera.hidden_target = Some(frame.anchor);
        }
        // HUD/cockpit sound is heard inside the watched aircraft.
        let mut listener = copy(&camera);
        if cockpit_view {
            listener.hidden_target = Some(0);
        }
        let forward = previous.is_some_and(|p| (frame.tick - p.tick - 2.).abs() < 0.001);
        if frame.view == 3 {
            if frame.rate == 0. {
                self.clock.pause();
            } else if !self.clock.set_speed(frame.rate) {
                return Err("replay UI requires a supported playback speed".into());
            }
        } else if forward {
            self.clock.play();
        } else {
            self.clock.pause();
        }
        self.sound.frame(
            Some(audio),
            &sound::Moment {
                from: previous.map_or(frame.tick, |p| p.tick),
                clock: &self.clock,
                scrubbing: !forward,
                camera: &listener,
                view: (cockpit_view).then_some(0),
                selected: if frame.anchor == u32::MAX {
                    0
                } else {
                    frame.anchor
                },
            },
        );
        if cockpit_view && frame.anchor != 0 && forward {
            let from = previous.map_or(tick, |p| p.tick.ceil() as u64);
            for entry in self.recording.events_between(from + 1, tick) {
                if entry.event.kind == "weapon.launch"
                    && entry.event.subject == Some(frame.anchor)
                    && entry.event.string("class") == Some("gun")
                {
                    if gpu.last_gun_tick.is_none_or(|last| entry.tick > last + 12)
                        && let Some(weapon) = entry.event.get("weapon").and_then(|v| v.as_i64())
                        && let Some(sound) = gpu.gun_sounds.get(&(weapon as u32))
                    {
                        audio.effect(sound);
                    }
                    gpu.last_gun_tick = Some(entry.tick);
                }
            }
        }
        self.world.resolve_palette(camera.position[1]);
        self.world.set_origin(camera.position);
        let device = &gpu.device;
        let queue = &gpu.queue;
        let sim = &mut gpu.sim;
        let vapor = match self.playback.vapor(tick, &self.ownship, &mut self.scratch) {
            Some(vapor) => crate::vapor_vertices(
                &vapor,
                &self.world,
                &player,
                self.ownship.streamer_points(&player),
            ),
            None => Vec::new(),
        };
        sim.vapor(device, queue, &vapor);
        let [smoke, contrails] = self.playback.smoke(tick);
        let devices = self.devices.at(tick, &self.world);
        sim.smoke(device, queue, &self.art.smoke, [smoke, contrails], devices);
        sim.effects(
            device,
            queue,
            &self.art.effects,
            &picture.effects,
            &picture.marks,
        );
        let (player_outlets, model_outlets) = &self.outlets;
        let mut glows = Vec::new();
        if picture.player.engine.flame {
            glows.extend(render_snapshot::afterburner_glow(
                picture.player.position,
                picture.player.attitude,
                player_outlets,
            ));
        }
        glows.extend(render_snapshot::target_glows(&picture, |pose| {
            render_snapshot::engine_outlets(
                pose.aircraft,
                self.ownship.profile.id,
                &self.models,
                model_outlets,
                player_outlets,
            )
        }));
        sim.emitters(queue, devices, &glows);
        let destroyed = self.tracks.destroyed(tick);
        if self
            .airports
            .as_ref()
            .is_none_or(|(set, ..)| *set != destroyed)
        {
            self.airports = Some((
                destroyed.clone(),
                self.world.visible_static_vertices_where(&destroyed),
                self.world.visible_static_lines_where(&destroyed),
            ));
        }
        if let Some((_, vertices, lines)) = &self.airports {
            sim.airports(device, queue, vertices);
            sim.airport_lines(device, queue, lines);
        }
        if let Some(art) = &self.art.escape {
            sim.escapees(
                device,
                queue,
                art,
                &art.vertices_for(
                    picture
                        .pilots
                        .iter()
                        .map(|p| (p.position, p.heading, p.phase)),
                    &self.ownship.palette,
                    camera.position,
                    self.world.origin,
                ),
            );
        }
        sim.dummies(
            device,
            queue,
            render_snapshot::aircraft_batches(&picture, &self.models, &camera, &self.world),
        );
        sim.combat(
            device,
            queue,
            &render_snapshot::combat_geometry(
                &picture,
                &self.art,
                &self.ownship,
                &player,
                &camera,
                &self.world,
            ),
        );
        sim.aircraft(
            device,
            queue,
            &self.ownship,
            &self.ownship.vertices(&player, &camera, &self.world),
        );
        if cockpit_view {
            let state = watched.as_ref().ok_or("missing cockpit state")?;
            if gpu.cockpit_aircraft != Some(watched_model.profile.id) {
                gpu.cockpit.prepare(
                    device,
                    queue,
                    &watched_model.sprites[watched_model.profile.id.cockpit()],
                    &watched_model.cockpit_pic,
                    watched_model.profile.id,
                );
                gpu.cockpit_aircraft = Some(watched_model.profile.id);
            }
            let palette = watched_model.cockpit_palette(&self.world, camera.position[1], 0);
            let color = palette[usize::from(watched_model.hud.primary_color)];
            let mut pixels = vec![0; 640 * 480 * 4];
            let zoom = (1. / crate::flight_canvas::HUD_SCALE) as f32;
            crate::hud::draw(
                &mut pixels,
                state,
                &watched_model.hud_font,
                f64::from(
                    self.world
                        .height(state.position[0] as f32, state.position[2] as f32),
                ),
                self.world.air_data(state).ok().as_ref(),
                true,
                false,
                color,
                zoom,
                None,
                None,
                (state.bank, 1.),
            );
            let selected = self
                .recording
                .events()
                .iter()
                .rev()
                .find(|e| {
                    e.tick <= tick
                        && e.event.kind == "ai.target"
                        && e.event.subject == Some(frame.anchor)
                })
                .and_then(|e| e.event.object);
            if let Some(target) = selected.and_then(|id| picture.target(id)) {
                crate::weapon_hud::replay_target(
                    &mut pixels,
                    state,
                    target.position,
                    color,
                    f64::from(zoom),
                    false,
                );
                if let Some((_, gun, mount)) = gpu
                    .gun_models
                    .iter()
                    .find(|(id, ..)| *id == watched_model.profile.id)
                {
                    crate::weapon_hud::replay_gunsight(
                        &mut pixels,
                        state,
                        color,
                        f64::from(zoom),
                        gun,
                        *mount,
                        tore_sim::combat::gunsight::TargetObservation {
                            position: target.position,
                            velocity: target.velocity,
                        },
                    );
                }
            }
            gpu.cockpit.weather(queue, &palette);
            gpu.cockpit.update(
                queue,
                [crate::reel::WIDTH, crate::reel::HEIGHT],
                state,
                &camera,
                frame.view == 2,
                true,
                &pixels,
            );
            if frame.view == 2 && gpu.cockpit.mirrors_visible {
                let mirror = crate::mirrors::camera(state);
                sim.aircraft(
                    device,
                    queue,
                    watched_model,
                    &watched_model.vertices(state, &mirror, &self.world),
                );
                gpu.mirror(&mirror, &self.world);
                gpu.sim.aircraft(
                    &gpu.device,
                    &gpu.queue,
                    &self.ownship,
                    &self.ownship.vertices(&player, &camera, &self.world),
                );
            }
            if frame.anchor == 0 {
                gpu.sim.hide_aircraft();
            }
        } else {
            gpu.cockpit.update(
                queue,
                [crate::reel::WIDTH, crate::reel::HEIGHT],
                &player,
                &camera,
                false,
                false,
                &[],
            );
        }
        let mut pixels = gpu.pixels(&camera, &self.world)?;
        if gpu.symbols.is_some() {
            gpu.symbols = Some(if cockpit_view {
                gpu.hud_symbols()?
            } else {
                Vec::new()
            });
        }
        if frame.view == 3 {
            self.selected = frame.anchor;
            self.view = 1;
            self.ui = Ui::default();
            let mut canvas = FlightCanvas::default();
            self.overlay(
                &picture,
                tick,
                &camera,
                [crate::reel::WIDTH, crate::reel::HEIGHT],
                &mut canvas,
            );
            // The real replay UI, composited in linear light like the GPU blit.
            let linear: [f32; 256] = std::array::from_fn(|i| {
                let c = i as f32 / 255.;
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            });
            for (dst, src) in pixels
                .chunks_exact_mut(4)
                .zip(canvas.pixels.chunks_exact(4))
            {
                match src[3] {
                    0 => {}
                    255 => dst.copy_from_slice(src),
                    alpha => {
                        let a = f32::from(alpha) / 255.;
                        for c in 0..3 {
                            let v = linear[usize::from(src[c])] * a
                                + linear[usize::from(dst[c])] * (1. - a);
                            let v = if v <= 0.0031308 {
                                12.92 * v
                            } else {
                                1.055 * v.powf(1. / 2.4) - 0.055
                            };
                            dst[c] = (v * 255.).round().clamp(0., 255.) as u8;
                        }
                    }
                }
            }
        }
        Ok(pixels)
    }
}
