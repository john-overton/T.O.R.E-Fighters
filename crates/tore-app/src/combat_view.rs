//! Combat's presentation: what the app draws and reads out of a [`Combat`].
//!
//! `Combat` holds the simulation of the range, the targets and the weapons.
//! [`CombatView`] holds what only the screen needs beside it: the effect,
//! smoke, weapon and ejection art, and the drawn model of every other aircraft
//! type the mission loads (combat itself keeps their [`AircraftType`]s). The
//! functions here read `&Combat`, as `scenery.rs` reads `&Terrain`: vertex
//! building, the afterburner lights, the target window's camera, and the
//! readouts and status text the instruments and messages show.
use crate::{
    AppResult,
    aircraft::Airframe,
    aircraft_type::AircraftType,
    camera::Camera,
    combat::{Combat, launcher, target_pose},
    flight,
    render_snapshot::CombatArt,
    scenery::Scenery,
    sim_renderer::Contact,
    snapshot::{AircraftPose, RenderSnapshot, blend, interpolate},
    terrain::Terrain,
};
use std::{collections::BTreeMap, sync::Arc};
use tore_formats::aircraft::AircraftId;
use tore_sim::{
    attitude::{Basis, Vector},
    combat::live,
};

/// The art and models drawn for one [`Combat`].
pub struct CombatView {
    /// Effect, smoke, weapon and ejection art drawn from render snapshots.
    pub art: CombatArt,
    /// The drawn models of the other aircraft types, in draw order: one
    /// for each of combat's [`Combat::dummy_types`], in the same order.
    pub models: Vec<Airframe>,
    /// Where each model's engines exhaust, parallel to `models`.
    outlets: Vec<Vec<Vector>>,
    /// How many of combat's other-aircraft configurations have had their
    /// weapon shapes loaded into `art`.
    shaped: usize,
    /// The frame's fraction of the way from the previous tick to the current
    /// one. Combat keeps the last two snapshots; this is what blends them.
    alpha: f64,
    /// The render history `alpha` was set for; a restart replaces it, and the
    /// new history shows its newest snapshot until the frame sets a fraction.
    restarts: u64,
}

impl CombatView {
    /// The art for `combat`: the effect sheets and every weapon shape the
    /// player's stations name.
    pub fn new(combat: &Combat, data: &BTreeMap<String, Vec<u8>>) -> AppResult<Self> {
        let mut view = Self::for_configuration(combat.state.configuration(), data)?;
        view.restarts = combat.render_restarts();
        Ok(view)
    }

    /// The art for a player with this configuration's stations.
    pub fn for_configuration(
        config: &live::Configuration,
        data: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<Self> {
        let mut art = CombatArt::load(data)?;
        art.add_weapon_shapes(config, data);
        Ok(Self {
            art,
            models: Vec::new(),
            outlets: Vec::new(),
            shaped: 0,
            alpha: 1.,
            restarts: 0,
        })
    }

    /// Sets the frame's fraction of the way from the previous tick to the
    /// current one, for everything drawn this frame.
    pub fn present(&mut self, combat: &Combat, alpha: f64) {
        self.alpha = alpha;
        self.restarts = combat.render_restarts();
    }

    /// The tick fraction in force for `combat`'s current render history.
    fn alpha(&self, combat: &Combat) -> f64 {
        if self.restarts == combat.render_restarts() {
            self.alpha
        } else {
            1.
        }
    }

    /// The picture for this frame: the last two snapshots at the frame's
    /// tick fraction.
    pub fn presented(&self, combat: &Combat) -> RenderSnapshot {
        interpolate(
            combat.previous_snapshot(),
            combat.render_snapshot(),
            self.alpha(combat),
        )
    }

    /// One target at the frame's tick fraction; none when it is not in the
    /// snapshots.
    pub fn presented_target(&self, combat: &Combat, id: u32) -> Option<AircraftPose> {
        let current = combat.current_target(id)?;
        Some(blend(
            combat.previous_target(id),
            current,
            self.alpha(combat).clamp(0., 1.),
        ))
    }

    /// Where a target is drawn this frame; one missing from the snapshots is
    /// drawn where it is.
    pub fn pose(&self, combat: &Combat, target: &live::Target) -> ([f64; 3], [f64; 3]) {
        self.presented_target(combat, target.id).map_or(
            (target.position, target_pose(target, combat.ai_poses)),
            |pose| (pose.position, pose.attitude),
        )
    }

    /// The target window's camera on the displayed target, at its presented
    /// pose.
    pub fn target_camera(&self, combat: &Combat, player: &flight::State) -> Option<Camera> {
        let target = combat.state.display_target()?;
        let (position, _) = self.pose(combat, target);
        Some(crate::target_preview::camera(player.position, position))
    }

    /// Populate all six creator wings in `combat`, retaining their sides for
    /// placement, and load what draws them.
    pub fn mission_aircraft(
        &mut self,
        combat: &mut Combat,
        wings: &[tore_sim::ai::launch::WingLaunch],
        layout: &crate::mission_layout::MissionLayout,
        data: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<()> {
        combat.mission_aircraft(wings, layout, data, &mut |id| self.load_model(id, data))?;
        self.load_weapon_shapes(combat, data);
        Ok(())
    }

    /// Adds straight-flight fixtures of the given aircraft to `combat`, and
    /// loads what draws them.
    pub fn mission_dummies(
        &mut self,
        combat: &mut Combat,
        wings: &[(AircraftId, usize)],
        separation: f64,
        data: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<()> {
        combat.mission_dummies(wings, separation, data, &mut |id| self.load_model(id, data))?;
        self.load_weapon_shapes(combat, data);
        Ok(())
    }

    /// Loads the drawn model of an aircraft type combat has not seen yet and
    /// hands combat its simulation half.
    fn load_model(
        &mut self,
        id: AircraftId,
        data: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<Arc<AircraftType>> {
        let model = Airframe::load(data, id)?;
        let kind = Arc::clone(&model.kind);
        self.outlets.push(kind.contrail_offsets.clone());
        self.models.push(model);
        Ok(kind)
    }

    /// Loads the weapon shapes of the other aircraft combat gained since the
    /// last call.
    fn load_weapon_shapes(&mut self, combat: &Combat, data: &BTreeMap<String, Vec<u8>>) {
        for config in &combat.dummy_configurations()[self.shaped..] {
            self.art.add_weapon_shapes(config, data);
        }
        self.shaped = combat.dummy_configurations().len();
    }

    /// Per-model vertices for the dummy formation, with each airborne
    /// aircraft's vertex range for the spotting aid.
    pub fn dummy_geometry(
        &self,
        combat: &Combat,
        camera: &Camera,
        world: &Terrain,
        scenery: &Scenery,
    ) -> Vec<(&Airframe, Vec<f32>, Vec<Contact>)> {
        crate::render_snapshot::aircraft_batches(
            &self.presented(combat),
            &self.models,
            camera,
            world,
            scenery,
        )
    }
    /// Combat geometry drawn with the ownship airframe over its presented state.
    pub fn vertices(
        &self,
        combat: &Combat,
        h: &Airframe,
        s: &flight::State,
        camera: &Camera,
        world: &Terrain,
        scenery: &Scenery,
    ) -> crate::sim_renderer::CombatGeometry {
        crate::render_snapshot::combat_geometry(
            &self.presented(combat),
            &self.art,
            h,
            s,
            camera,
            world,
            scenery,
        )
    }
    /// The flame of every lit afterburner as a light source, each engine
    /// sharing its aircraft's strength, at this frame's presented poses
    /// (docs/spec/engine-material.md#afterburner-glow): the player's from
    /// its presented flight state, every other aircraft's from the presented
    /// snapshot, as a replay draws them.
    pub fn afterburner_glows(
        &self,
        combat: &Combat,
        player: &flight::State,
    ) -> Vec<crate::countermeasure_renderer::Afterburner> {
        let mut glows = Vec::new();
        if player.afterburner_active() && player.escape.is_none() && combat.state.player_hp > 0 {
            glows.extend(crate::render_snapshot::afterburner_glow(
                player.position,
                [player.yaw, player.pitch, player.bank],
                combat.contrail_offsets(),
            ));
        }
        let player_type = combat.state.configuration().aircraft;
        // Only the lit aircraft, blended as the picture blends them, so the
        // rest of the picture is not built again for the lights.
        let lit = RenderSnapshot {
            targets: combat
                .render_snapshot()
                .targets
                .iter()
                .filter(|pose| pose.engine.flame)
                .filter_map(|pose| self.presented_target(combat, pose.id))
                .collect(),
            ..RenderSnapshot::default()
        };
        glows.extend(crate::render_snapshot::target_glows(&lit, |pose| {
            crate::render_snapshot::engine_outlets(
                pose.aircraft,
                player_type,
                &self.models,
                &self.outlets,
                combat.contrail_offsets(),
            )
        }));
        glows
    }

    /// The target window's camera, framed to the target's model or bounds.
    pub fn framed_target_camera(
        &self,
        combat: &Combat,
        player: &flight::State,
        aircraft: &Airframe,
        world: &Terrain,
        scenery: &Scenery,
    ) -> Option<Camera> {
        let target = combat.state.display_target()?;
        let mut camera = self.target_camera(combat, player)?;
        if let Some(object) = combat.ground_object(target.id) {
            let bounds = object.bounds;
            let basis = Basis::new(bounds.heading, bounds.pitch, bounds.bank);
            let corners = (0..8).map(|corner| {
                let local: [f64; 3] = std::array::from_fn(|i| {
                    bounds.half[i] * if corner & (1 << i) == 0 { -1. } else { 1. }
                });
                std::array::from_fn(|i| {
                    bounds.center[i]
                        + basis.right[i] * local[0]
                        + basis.up[i] * local[1]
                        + basis.forward[i] * local[2]
                })
            });
            crate::target_preview::fit(&mut camera, corners);
        } else {
            let model = self
                .models
                .iter()
                .find(|h| Some(h.profile.id) == target.aircraft)
                .unwrap_or(aircraft);
            let mut pose = player.clone();
            pose.wreck = target.wreck.clone();
            pose.crashed = target.hp <= 0;
            let presented = self.presented_target(combat, target.id);
            let (position, angles) = presented.as_ref().map_or(
                (target.position, target_pose(target, combat.ai_poses)),
                |p| (p.position, p.attitude),
            );
            pose.position = position;
            [pose.yaw, pose.pitch, pose.bank] = angles;
            pose.damage_fraction = target.damage_fraction();
            pose.damage_variant = target
                .localized_damage
                .structural_section
                .map(|s| s as usize);
            pose.damage_regions = target.localized_damage.fractions(target.initial_hp);
            pose.gear = 0.;
            pose.flaps = 0.;
            pose.exhaust = 0.;
            pose.bay = 0.;
            pose.elevator = 0.;
            pose.aileron = 0.;
            pose.rudder = 0.;
            pose.brake = 0.;
            pose.hook = 0.;
            if let Some(devices) = presented.and_then(|p| p.devices) {
                crate::snapshot::set_devices(&mut pose, devices);
            }
            let vertices = model.vertices(&pose, &camera, world, scenery);
            crate::target_preview::fit(
                &mut camera,
                vertices
                    .chunks_exact(10)
                    .map(|v| [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]),
            );
        }
        Some(camera)
    }
}

pub fn readout(
    combat: &Combat,
    s: &flight::State,
    rcs_scale: f64,
) -> crate::instruments::CombatReadout {
    // (source, name, rounds, selected, loaded at the start)
    let mut groups: Vec<(String, String, u32, bool, bool)> = Vec::new();
    for (index, station) in combat.state.configuration().stations.iter().enumerate() {
        let selected = combat.state.armed && index == combat.state.selected;
        let loaded = combat.state.was_loaded(index);
        if let Some(group) = groups.iter_mut().find(|g| g.0 == station.weapon.source) {
            group.2 += u32::from(combat.state.rounds(index));
            group.3 |= selected;
            group.4 |= loaded;
        } else {
            groups.push((
                station.weapon.source.clone(),
                station.weapon.hud_name.clone(),
                u32::from(combat.state.rounds(index)),
                selected,
                loaded,
            ));
        }
    }
    crate::instruments::CombatReadout {
        // The window lists what the aircraft was loaded with: a weapon
        // that ran dry keeps its row at zero, but one that was never
        // loaded (taken off on the Load Ordnance page) has none.
        weapons: groups
            .into_iter()
            .filter(|(_, _, count, selected, loaded)| *count > 0 || *selected || *loaded)
            .map(|(_, name, count, selected, _)| (name, count, selected))
            .collect(),
        chaff: combat.state.chaff,
        flares: combat.state.flares,
        target: combat.state.display_target().map(|target| {
            let name = combat
                .ground_name(target.id)
                .map(str::to_owned)
                .or_else(|| target.aircraft.map(|id| id.label().to_owned()))
                .unwrap_or_else(|| format!("CONTACT {}", target.id));
            crate::target_window::Readout::new(target, s, name)
        }),
        envelope_target: combat.state.display_target().and_then(|target| {
            combat
                .dummy_types()
                .iter()
                .find(|h| Some(h.profile.id) == target.aircraft)
                .map(|h| h.profile.envelopes.clone())
        }),
        scope: crate::scope::scope(&combat.state, s),
        rcs: crate::scope::rcs(&combat.state, s, rcs_scale),
        rwr_failed: combat.state.rwr_failed,
        rwr: rwr_readout(combat, s),
    }
}
fn rwr_readout(combat: &Combat, own: &flight::State) -> crate::scope::Rwr {
    use crate::scope::{EmitterKind, EmitterState, Indicator, Rwr, RwrEmitter, RwrMissile};
    use tore_sim::combat::threats::GuidanceClass;
    let operating = !combat.state.rwr_failed && own.systems.counts[32] <= 1;
    let emitters = if operating {
        combat
            .state
            .emitters
            .iter()
            .map(|emitter| RwrEmitter {
                id: emitter.id,
                bearing_rad: emitter.bearing_rad,
                distance_nmi: emitter.distance_nmi,
                // Allegiance is supplied by the mission bridge only for known
                // aircraft. Passive reception alone remains unidentified.
                kind: match emitter.symbol {
                    tore_sim::sensors::passive::Symbol::Ground => EmitterKind::Ground,
                    tore_sim::sensors::passive::Symbol::Aircraft => EmitterKind::EnemyAircraft,
                    tore_sim::sensors::passive::Symbol::Unknown => EmitterKind::Unknown,
                },
                state: EmitterState::Detected,
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut radar_indicator = if combat.state.emitters.is_empty() || !operating {
        Indicator::Off
    } else {
        Indicator::Detected
    };
    let mut infrared_indicator = Indicator::Off;
    let missiles = combat
        .state
        .missile_threats
        .records()
        .map(|record| {
            if !record.stale && record.targeting_receiver {
                match record.guidance_class {
                    Some(GuidanceClass::Radar) => radar_indicator = Indicator::Incoming,
                    Some(GuidanceClass::Infrared) => infrared_indicator = Indicator::Incoming,
                    _ => {}
                }
            }
            RwrMissile {
                id: record.missile_id,
                bearing_rad: record.bearing_deg.to_radians(),
                distance_nmi: record.position.map(|position| {
                    position
                        .iter()
                        .zip(own.position)
                        .map(|(a, b)| (a - b).powi(2))
                        .sum::<f64>()
                        .sqrt()
                        / tore_sim::sensors::FEET_PER_NAUTICAL_MILE
                }),
                known_targeting_receiver: record.targeting_receiver,
                stale: record.stale,
            }
        })
        .collect();
    let mut readout = Rwr {
        tick: combat.state.sensors.tick(),
        operating,
        emitters,
        missiles,
        radar_indicator,
        infrared_indicator,
    };
    readout.mark_supported_sources(
        combat
            .state
            .missile_threats
            .records()
            .filter(|r| {
                !r.stale
                    && r.targeting_receiver
                    && r.source == tore_sim::combat::threats::EvidenceSource::ElectronicSupported
            })
            .filter_map(|r| r.radar_bearing_deg),
    );
    readout
}

pub fn equipment_damage_report(combat: &Combat) -> Vec<String> {
    let config = combat.state.configuration();
    (36..45)
        .filter(|i| combat.state.subsystem_counts[*i] > 0)
        .map(|i| {
            let hardpoint = i - 36;
            if config.external_fuel_lbs[hardpoint] > 0. {
                format!("External tank {} damaged", hardpoint + 1)
            } else if let Some(Some(slot)) = config.hardpoint_slots.get(hardpoint) {
                format!("{} station failed", config.stations[*slot].weapon.hud_name)
            } else if hardpoint == config.radar_hardpoint {
                "Radar failed".into()
            } else if hardpoint == config.visual_hardpoint {
                "Visual sensor failed".into()
            } else if Some(hardpoint) == config.infrared_hardpoint {
                "Infrared sensor failed".into()
            } else if Some(hardpoint) == config.rwr_hardpoint {
                "RWR failed".into()
            } else if hardpoint == config.ecm_hardpoint {
                format!(
                    "Countermeasures: jammer {}, chaff {}, flares {}",
                    if combat.state.ecm_failed {
                        "failed"
                    } else {
                        "available"
                    },
                    combat.state.chaff,
                    combat.state.flares
                )
            } else {
                format!("Hardpoint {} equipment damaged", hardpoint + 1)
            }
        })
        .collect()
}
pub fn status(combat: &Combat, s: &flight::State) -> String {
    let i = combat.state.selected;
    let target = combat.state.designated().map_or("NO TARGET".into(), |id| {
        combat
            .state
            .targets
            .iter()
            .find(|t| t.id == id)
            .map_or("NO TARGET".into(), |t| {
                if t.hp == 0 {
                    "DESTROYED".into()
                } else {
                    format!(
                        "{} HP {} {}",
                        combat
                            .ground_name(id)
                            .map_or_else(|| format!("T{id}"), str::to_owned),
                        t.hp,
                        if combat.state.configuration().stations[i]
                            .weapon
                            .seeker
                            .signature
                            == 0
                        {
                            "VISUAL"
                        } else if combat.state.can_lock(launcher(s)) {
                            "LOCK"
                        } else {
                            "NO LOCK"
                        }
                    )
                }
            })
    });
    let scope = crate::scope::scope(&combat.state, s);
    format!(
        "{} {} {}  {} C{} HIT {} | HP {} SYS {} ECM {} T-JAM {} IN {} | {} {} {:.0}NM {} CONTACTS{}{}",
        combat.state.configuration().stations[i].weapon.name,
        combat.state.rounds(i),
        combat.state.readiness(launcher(s)).label(),
        target,
        live::damage_class(combat.state.range_category),
        combat.state.history.last().map_or(0, |hit| hit.applied),
        combat.state.player_hp,
        combat
            .state
            .last_subsystem
            .map_or("--".into(), |i| i.to_string()),
        if combat.state.ecm_failed {
            "FAIL"
        } else if launcher(s).jammer {
            "ON"
        } else {
            "OFF"
        },
        if combat.state.target_jammer {
            "ON"
        } else {
            "OFF"
        },
        combat
            .state
            .projectiles
            .iter()
            .filter(|p| p.incoming)
            .count(),
        scope.channel,
        scope.mode.unwrap_or("OFF"),
        scope.range_nmi,
        scope.contacts.iter().filter(|c| !c.stale).count(),
        if scope.history { " HIST" } else { "" },
        scope
            .status
            .map(|status| format!(" {status}"))
            .unwrap_or_default()
    )
}

/// Exact drawn output of one synthetic combat scene. The hashes pin every
/// vertex live flight uploads for other aircraft, debris, weapons, effects
/// and ejected pilots, so a presentation refactor can prove it changed nothing.
/// Mission recordings reuse the scene to prove a replay draws the same.
#[cfg(test)]
pub(crate) mod render_hash_tests {
    use super::*;
    use crate::{
        combat::fixtures, damage_art::DamageArt, render_snapshot::combat_geometry,
        snapshot::pose_state,
    };
    use fixtures::load;
    pub(crate) use fixtures::{pilots, player, scene, snapshots};
    use tore_formats::{
        aircraft::AircraftId,
        shape::{Face, FogMode, Line, Shape},
    };
    use tore_sim::combat::live::DamageSection;

    /// Batches per model, fixture targets with the ownship, combat geometry
    /// beside loaded models, camera poses and ejected pilots.
    const HASHES: [u64; 5] = [
        0xd383_5c36_01f2_ee53,
        0xdd1a_67a0_c439_7ced,
        0x2f6c_0a1c_b3b7_f4e5,
        0x7ddb_79bb_1315_bf69,
        0x7e3f_dcdd_a149_201d,
    ];

    /// Where `HASHES` was recorded. The camera poses (`HASHES[3]`) are f64
    /// angles from trigonometry, whose last bit differs between maths
    /// libraries, so, as in tore-sim's golden tests, that hash is compared
    /// only here. The drawn vertices are f32 and match on every CI platform.
    const RECORDED_PLATFORM: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

    /// FNV-1a over exact bit patterns.
    struct Fnv(u64);
    impl Fnv {
        fn new() -> Self {
            Self(0xcbf2_9ce4_8422_2325)
        }
        fn bytes(&mut self, bytes: &[u8]) {
            for byte in bytes {
                self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
        fn count(&mut self, count: usize) {
            self.bytes(&(count as u64).to_le_bytes());
        }
        fn floats(&mut self, values: &[f32]) {
            self.count(values.len());
            for value in values {
                self.bytes(&value.to_bits().to_le_bytes());
            }
        }
        fn doubles(&mut self, values: &[f64]) {
            self.count(values.len());
            for value in values {
                self.bytes(&value.to_bits().to_le_bytes());
            }
        }
        fn contacts(&mut self, contacts: &[Contact]) {
            self.count(contacts.len());
            for contact in contacts {
                self.bytes(&contact.first.to_le_bytes());
                self.bytes(&contact.count.to_le_bytes());
                self.floats(&contact.center);
                self.floats(&[contact.extent]);
            }
        }
    }

    /// A small source-space polygon (X right, Y forward, Z up) around `at`.
    fn face(address: usize, at: [f32; 3], corners: usize, texture: &str, subtype: u8) -> Face {
        Face {
            positions: (0..corners)
                .map(|i| {
                    let angle = i as f32 * std::f32::consts::TAU / corners as f32 + 0.3;
                    [
                        at[0] + 4. * angle.cos(),
                        at[1] + 3. * angle.sin(),
                        at[2] + angle.sin(),
                    ]
                })
                .collect(),
            colors: (0..corners)
                .map(|i| (address % 97 + i * 31) as u8)
                .collect(),
            fog: if address.is_multiple_of(2) {
                FogMode::Enabled
            } else {
                FogMode::Disabled
            },
            uv: if texture.is_empty() {
                Vec::new()
            } else {
                (0..corners)
                    .map(|i| [1. + i as f32, 2. + (i % 2) as f32])
                    .collect()
            },
            texture: texture.into(),
            subtype,
            normal: Some([0.3, 0.8, -0.5]),
            address,
        }
    }
    fn shape(faces: Vec<Face>) -> Shape {
        Shape {
            lines: Vec::new(),
            faces,
            state_words: Default::default(),
        }
    }
    /// Reviewed F/A-18D part addresses: body, flame, both nozzles, brake,
    /// hook, gear and doors, flap, both tailplanes and a split rudder.
    fn hornet() -> Shape {
        let mut rudder = face(0x5467, [0.; 3], 4, "", 0x20);
        rudder.positions = vec![
            [8., -40., 4.],
            [8., -20., 4.],
            [18., -30., 24.],
            [18., -45., 24.],
        ];
        shape(vec![
            face(0x1000, [0., 10., 0.], 4, "_SYN.PIC", 0x60),
            face(0x1001, [-12., -5., 1.], 3, "", 0x21),
            face(0x5310, [0., -58., 0.], 4, "_SYN.PIC", 0x4c),
            face(0x3ff7, [-3., -50., 0.], 4, "", 0x20),
            face(0x4026, [3., -50., 0.], 4, "", 0x20),
            face(0x5059, [0., -28., 5.], 3, "", 0x20),
            face(0x4a03, [0., -37., -3.], 3, "", 0x20),
            face(0x4b69, [-3., 0., -6.], 3, "", 0x20),
            face(0x4bfa, [8., -7., -6.], 3, "", 0x20),
            face(0x4ee1, [0., 55., -6.], 3, "", 0x20),
            face(0x525b, [-13., -8., 4.], 4, "", 0x20),
            face(0x449f, [-11., -43., 0.], 3, "", 0x20),
            face(0x486b, [11., -43., 0.], 3, "", 0x20),
            rudder,
        ])
    }
    /// Rafale C parts; the gear-down pose adds a main gear leg.
    fn rafale(gear: bool) -> Shape {
        let mut faces = vec![
            face(0x1000, [0., 12., 0.], 4, "_SYN.PIC", 0x60),
            face(0x4265, [0., -49., 0.], 4, "_SYN.PIC", 0x4c),
            face(0x3032, [-2., -45., 0.], 4, "", 0x20),
            face(0x3e1e, [2., -3., 4.], 3, "", 0x20),
            face(0x3b2f, [-2., 7., -7.], 3, "", 0x20),
            face(0x4190, [-10., -23., -1.], 4, "", 0x20),
            face(0x3d81, [-5., 30., 0.], 3, "", 0x20),
            face(0x3f08, [0., -40., 10.], 3, "", 0x20),
        ];
        if gear {
            faces.push(face(0x3c50, [-4., 10., -7.], 3, "", 0x20));
        }
        shape(faces)
    }
    /// F-14 parts over a synthetic rig: swept wing, flap, tailplane, split
    /// rudder, cold nozzle, and rig flame, gear, hook and brake groups.
    fn tomcat() -> Shape {
        shape(vec![
            face(0x1000, [0., 8., 0.], 4, "_SYN.PIC", 0x60),
            face(0x3000, [0., -20., 0.], 4, "", 0x4c),
            face(0x3100, [0., 17., -3.], 3, "", 0x20),
            face(0x3101, [5., 1., -2.], 3, "", 0x20),
            face(0x3200, [0., -5., -2.], 3, "", 0x20),
            face(0x3300, [2., -11., 1.], 3, "", 0x20),
            face(0x4e00, [-12., -2., 1.], 4, "", 0x20),
            face(0x540d, [10., -6., 1.], 3, "", 0x20),
            face(0x4828, [-6., -12., 0.], 3, "", 0x20),
            face(0x4a7b, [4., -14., 6.], 4, "", 0x20),
            face(0x48a6, [-2., -16., 0.], 4, "", 0x20),
        ])
    }
    fn damage() -> DamageArt {
        let piece = |address: usize, texture: &str| {
            shape(vec![
                face(address, [2., 5., 1.], 4, texture, 0x60),
                face(address + 1, [-6., -9., 0.], 3, "", 0x20),
            ])
        };
        DamageArt::synthetic(
            [20., 60., 12.],
            [piece(0x2000, "_DMG.PIC"), piece(0x2100, "_SYN.PIC")],
            [piece(0x2200, "_DMG.PIC"), piece(0x2300, "")],
            BTreeMap::from([
                ("_SYN.PIC".to_string(), [16, 8, 0]),
                ("_DMG.PIC".to_string(), [16, 4, 8]),
                ("_F18_A.PIC".to_string(), [16, 4, 12]),
            ]),
        )
    }
    pub(crate) fn hornet_airframe(material: bool) -> Airframe {
        Airframe::synthetic(
            AircraftId::F18,
            (0..16).map(|_| hornet()).collect(),
            None,
            damage(),
            material.then(|| crate::engine_material::Image {
                width: 2,
                height: 2,
                pixels: vec![200; 16],
            }),
        )
    }
    pub(crate) fn models() -> Vec<Airframe> {
        vec![
            hornet_airframe(false),
            Airframe::synthetic(
                AircraftId::Rafale,
                vec![rafale(false), rafale(true)],
                None,
                damage(),
                None,
            ),
            Airframe::synthetic(
                AircraftId::F14,
                vec![tomcat()],
                Some(crate::additional_animation::Rig::synthetic(
                    AircraftId::F14,
                    &[0x3000],
                    &[0x3300],
                    &[0x3100, 0x3101],
                    &[0x3200],
                )),
                damage(),
                None,
            ),
        ]
    }
    fn missile_shape(seed: usize) -> Shape {
        shape(vec![
            face(0x10 + seed, [0., 10., 0.], 4, "", 0x61),
            face(0x20 + seed, [0., -10., 1.], 3, "", 0x4c),
            face(0x30 + seed, [2., 0., 0.], 3, "", 0x20),
        ])
    }

    /// The weapon art the scene's missiles are drawn with.
    fn art() -> CombatArt {
        CombatArt::synthetic(BTreeMap::from([
            ("SYNMSL.SH".to_string(), missile_shape(0)),
            ("AIMSL.SH".to_string(), missile_shape(1)),
        ]))
    }
    /// The view of a scene with these models drawing the other aircraft.
    fn view(models: Vec<Airframe>) -> CombatView {
        CombatView {
            art: art(),
            outlets: models
                .iter()
                .map(|model| model.contrail_offsets.clone())
                .collect(),
            models,
            shaped: 0,
            alpha: 1.,
            restarts: 0,
        }
    }
    /// A combat scene whose other aircraft are the types of `models`.
    pub(crate) fn combat(models: Vec<Airframe>, dummies: Vec<(usize, Vector)>) -> Combat {
        pair(models, dummies).0
    }
    /// A combat scene and its view, the other aircraft drawn by `models`.
    pub(crate) fn pair(
        models: Vec<Airframe>,
        dummies: Vec<(usize, Vector)>,
    ) -> (Combat, CombatView) {
        let types = models.iter().map(|model| Arc::clone(&model.kind)).collect();
        (fixtures::combat(types, dummies), view(models))
    }

    fn camera(position: [f64; 3], [yaw, pitch, roll]: [f32; 3]) -> Camera {
        let mut camera = Camera::new();
        camera.position = position;
        camera.yaw = yaw;
        camera.pitch = pitch;
        camera.roll = roll;
        camera
    }
    /// A general view, one hiding target 7 and missile 17, and one looking
    /// straight down the player's first tracer.
    pub(crate) fn cameras() -> Vec<Camera> {
        let mut hiding = camera([500., 5100., 1500.], [-0.3, 0.05, -0.2]);
        hiding.hidden_target = Some(7);
        hiding.hidden_projectile = Some(17);
        vec![
            camera([-400., 5400., 600.], [0.2, -0.15, 0.1]),
            hiding,
            camera([33., 5011., 430.], [3.24, -0.03, 0.]),
        ]
    }
    pub(crate) fn escape_art() -> crate::ejection_art::Art {
        let pose = |n: usize| Shape {
            lines: vec![
                Line {
                    positions: [[0., 0., 3. + n as f32], [2., 1., 9.]],
                    color: 40 + n as u8,
                    fog: FogMode::Enabled,
                },
                Line {
                    positions: [[1.; 3], [1.; 3]],
                    color: 1,
                    fog: FogMode::Disabled,
                },
            ],
            faces: vec![
                face(0x10 + n, [0., 0., 2.], 4, "_EJECTA.PIC", 0x60),
                face(0x20 + n, [1., 1., 1.], 3, "", 0x20),
            ],
            state_words: Default::default(),
        };
        crate::ejection_art::Art::synthetic(
            (0..5).map(pose).collect(),
            &[("_EJECTA.PIC", 8, 8), ("_EJECTB.PIC", 8, 4)],
        )
    }

    #[test]
    fn drawn_combat_scene_is_unchanged() {
        let ownship = hornet_airframe(true);
        let player = player();
        let mut stepped = crate::scenery::tests::scenery();
        stepped.smooth_weather = false;
        let worlds = [
            (
                crate::terrain::tests::world(),
                crate::scenery::tests::scenery(),
            ),
            (crate::terrain::tests::world(), stepped),
        ];
        let (mut with_models, mut with_models_view) =
            pair(models(), (0..7).map(|i| (i % 3, [0.; 3])).collect());
        let (mut fixture, mut fixture_view) = pair(Vec::new(), Vec::new());
        let scene = scene(with_models.state.configuration());
        let mut hashes = [(); 5].map(|()| Fnv::new());
        let mut drawn = [0; 3];
        for ai_poses in [true, false] {
            for alpha in [0., 0.37, 1.] {
                load(&mut with_models, &scene, ai_poses, &player);
                load(&mut fixture, &scene, ai_poses, &player);
                with_models_view.present(&with_models, alpha);
                fixture_view.present(&fixture, alpha);
                for target in &scene.current {
                    let (position, angles) = with_models_view.pose(&with_models, target);
                    hashes[3].doubles(&position);
                    hashes[3].doubles(&angles);
                }
                let [modelled, fixtures] =
                    [(&with_models, &with_models_view), (&fixture, &fixture_view)]
                        .map(|(combat, view)| view.presented(combat));
                for (world, scenery) in &worlds {
                    for camera in cameras() {
                        for (model, vertices, contacts) in
                            with_models_view.dummy_geometry(&with_models, &camera, world, scenery)
                        {
                            hashes[0].bytes(format!("{:?}", model.profile.id).as_bytes());
                            hashes[0].floats(&vertices);
                            hashes[0].contacts(&contacts);
                            drawn[0] += vertices.len();
                        }
                        let geometry = combat_geometry(
                            &fixtures,
                            &fixture_view.art,
                            &ownship,
                            &player,
                            &camera,
                            world,
                            scenery,
                        );
                        hashes[1].floats(&geometry.vertices);
                        hashes[1].contacts(&geometry.contacts);
                        drawn[1] += geometry.vertices.len();
                        let geometry = combat_geometry(
                            &modelled,
                            &with_models_view.art,
                            &ownship,
                            &player,
                            &camera,
                            world,
                            scenery,
                        );
                        hashes[2].floats(&geometry.vertices);
                        hashes[2].contacts(&geometry.contacts);
                        drawn[2] += geometry.vertices.len();
                    }
                }
            }
        }
        let art = escape_art();
        let pilots = pilots();
        for camera in cameras() {
            hashes[4].floats(&art.vertices_for(
                pilots.iter().map(|p| (p.position, p.heading, p.phase)),
                &ownship.palette,
                camera.position,
                [0.; 3],
            ));
        }
        assert!(drawn.iter().all(|&floats| floats > 10_000), "{drawn:?}");
        let mut hashes = hashes.map(|hash| hash.0);
        if !RECORDED_PLATFORM {
            hashes[3] = HASHES[3];
        }
        assert_eq!(hashes, HASHES, "drawn output changed: {hashes:#018x?}");
    }

    /// Fixture targets drawn from a snapshot match the rule they were drawn
    /// with before snapshots existed: a copy of the presented player state
    /// with the target's pose, damage and crash flag, devices stowed. The
    /// player states cover afterburner, dry, engine off, no fuel and a
    /// crashed player whose afterburner switch is still on.
    /// Far from the map origin, 32-bit world coordinates step 1/8 foot, which
    /// made aircraft shimmer. Built relative to the render origin, the model
    /// keeps its exact shape there.
    #[test]
    fn aircraft_keep_their_shape_far_from_the_map_origin() {
        let ownship = hornet_airframe(true);
        let world = crate::terrain::tests::world();
        let mut scenery = crate::scenery::tests::scenery();
        let near = player();
        let camera = cameras().remove(0);
        let reference = ownship.vertices(&near, &camera, &world, &scenery);
        let shift = [1_070_000.37, 0., 590_000.61];
        let mut far = near.clone();
        far.position = std::array::from_fn(|i| near.position[i] + shift[i]);
        let mut far_camera = cameras().remove(0);
        far_camera.position = std::array::from_fn(|i| camera.position[i] + shift[i]);
        let error = |scenery: &crate::scenery::Scenery| {
            let vertices = ownship.vertices(&far, &far_camera, &world, scenery);
            assert_eq!(vertices.len(), reference.len());
            vertices
                .chunks_exact(10)
                .zip(reference.chunks_exact(10))
                .flat_map(|(a, b)| {
                    (0..3).map(move |i| {
                        (f64::from(a[i]) + scenery.origin[i] - f64::from(b[i]) - shift[i]).abs()
                    })
                })
                .fold(0., f64::max)
        };
        assert!(error(&scenery) > 0.03, "world coordinates snap");
        scenery.set_origin(far_camera.position);
        assert_eq!(scenery.origin, [1_070_080., 5120., 590_848.]);
        assert!(error(&scenery) < 0.002, "{}", error(&scenery));
    }
    #[test]
    fn fixture_targets_keep_the_player_copy_rule() {
        let ownship = hornet_airframe(true);
        let world = crate::terrain::tests::world();
        let scenery = crate::scenery::tests::scenery();
        let (mut fixture, fixture_view) = pair(Vec::new(), Vec::new());
        let scene = scene(fixture.state.configuration());
        fixture.state.targets.clone_from(&scene.current);
        let mut states = Vec::new();
        for case in 0..6 {
            let mut s = player();
            s.throttle = 1.;
            match case {
                1 => s.burner = false,
                2 => s.engine = false,
                3 => s.fuel = 0.,
                4 => s.crashed = true,
                5 => s.throttle = 0.5,
                _ => {}
            }
            states.push(s);
        }
        assert!(states[0].afterburner_active() && !states[4].afterburner_active());
        for s in &states {
            fixture.restart_render(s, None);
            let snapshot = fixture_view.presented(&fixture);
            for camera in cameras() {
                let mut expected = Vec::new();
                for t in scene
                    .current
                    .iter()
                    .filter(|t| t.airborne && Some(t.id) != camera.hidden_target)
                {
                    let mut pose = s.clone();
                    pose.wreck = t.wreck.clone();
                    pose.crashed = t.hp <= 0;
                    pose.position = t.position;
                    pose.damage_fraction = t.damage_fraction();
                    pose.damage_variant = t
                        .localized_damage
                        .structural_section
                        .map(|section| section as usize);
                    pose.damage_regions = t.localized_damage.fractions(t.initial_hp);
                    [pose.yaw, pose.pitch, pose.bank] =
                        crate::combat::target_pose(t, fixture.ai_poses);
                    pose.exhaust = 0.;
                    pose.gear = 0.;
                    pose.flaps = 0.;
                    pose.elevator = 0.;
                    pose.aileron = 0.;
                    pose.rudder = 0.;
                    pose.brake = 0.;
                    pose.hook = 0.;
                    expected.extend(ownship.vertices(&pose, &camera, &world, &scenery));
                }
                let drawn = combat_geometry(
                    &snapshot,
                    &fixture_view.art,
                    &ownship,
                    s,
                    &camera,
                    &world,
                    &scenery,
                )
                .vertices;
                assert!(!expected.is_empty());
                assert_eq!(drawn, expected);
            }
        }
    }

    #[test]
    fn afterburner_lights_sit_in_the_flames_of_the_presented_poses() {
        use crate::countermeasure_renderer::{
            AFTERBURNER_BEHIND_FEET, AFTERBURNER_SHARE, Afterburner,
        };
        let player = player();
        let mut models = models();
        let outlets = [vec![[0., 1., -18.]], vec![], vec![[1., 0., -15.]]];
        for (model, offsets) in models.iter_mut().zip(&outlets) {
            Arc::get_mut(&mut model.kind)
                .expect("a model nothing else holds")
                .contrail_offsets
                .clone_from(offsets);
        }
        let (mut combat, mut view) = pair(models, (0..7).map(|i| (i % 3, [0.; 3])).collect());
        let own = vec![[-2., 0.5, -20.], [2., 0.5, -20.]];
        fixtures::set_contrail_offsets(&mut combat, own.clone());
        let scene = scene(combat.state.configuration());
        let [previous, mut current] = snapshots(&mut combat, &scene, true, &player);
        // Aircraft 1 (the player's type), 2 (Rafale) and 3 (F-14) burn.
        for pose in &mut current.targets {
            pose.engine.flame = matches!(pose.id, 1..=3);
        }
        fixtures::set_history(&mut combat, previous, current);
        view.present(&combat, 0.37);
        // The lights as they were computed before the snapshots carried
        // them: each lit aircraft's presented pose and its own outlets.
        let glow = |position: Vector, [yaw, pitch, bank]: [f64; 3], offsets: &[Vector]| {
            let basis = Basis::new(yaw, pitch, bank);
            let share = AFTERBURNER_SHARE / offsets.len().max(1) as f64;
            offsets
                .iter()
                .map(|o| Afterburner {
                    position: std::array::from_fn(|i| {
                        position[i]
                            + basis.right[i] * o[0]
                            + basis.up[i] * o[1]
                            + basis.forward[i] * (o[2] - AFTERBURNER_BEHIND_FEET)
                    }),
                    share,
                })
                .collect::<Vec<_>>()
        };
        let mut expected = Vec::new();
        if player.afterburner_active() && player.escape.is_none() {
            expected.extend(glow(
                player.position,
                [player.yaw, player.pitch, player.bank],
                &own,
            ));
        }
        for target in combat
            .state
            .targets
            .iter()
            .filter(|t| matches!(t.id, 1..=3))
        {
            let offsets = match target.aircraft {
                Some(AircraftId::Rafale) => &outlets[1],
                Some(AircraftId::F14) => &outlets[2],
                _ => &own,
            };
            let (position, angles) = view.pose(&combat, target);
            expected.extend(glow(position, angles, offsets));
        }
        let glows = view.afterburner_glows(&combat, &player);
        assert_eq!(glows, expected);
        // Both of the player's type's engines, the Rafale's none and the
        // F-14's one light the scene.
        assert_eq!(
            glows.len(),
            3 + 2 * usize::from(player.afterburner_active())
        );
    }

    /// A replay rebuilds the player's aircraft from snapshots alone and draws
    /// exactly what live flight draws from the presented flight state.
    #[test]
    fn the_player_round_trips_through_snapshots() {
        let ownship = hornet_airframe(true);
        let world = crate::terrain::tests::world();
        let scenery = crate::scenery::tests::scenery();
        let template =
            flight::State::new(&flight::animation_tests::profile(), [0., 5000., 0.]).unwrap();
        let (mut combat, view) = pair(Vec::new(), Vec::new());
        let scene = scene(combat.state.configuration());
        combat.state.targets.clone_from(&scene.current);
        combat.state.projectiles.clone_from(&scene.projectiles);
        combat.state.effects.clone_from(&scene.effects);
        combat.state.debris.clone_from(&scene.debris);
        let capacity = combat.state.configuration().damage_capacity;
        for case in 0..8 {
            let mut previous = player();
            previous.position = [0., 5000., 2000.];
            [previous.yaw, previous.pitch, previous.bank] = [0.1, 0., 0.2];
            previous.velocity = [10., 0., 700.];
            let mut current = previous.clone();
            current.position = [30., 5010., 2100.];
            [current.yaw, current.pitch, current.bank] = [0.2, 0.05, 0.4];
            current.velocity = [20., 5., 710.];
            [current.gear, current.flaps, current.exhaust] = [0.4, 0.2, 0.6];
            [current.elevator, current.aileron, current.rudder] = [-0.1, 0.3, -0.2];
            let mut hp = capacity / 5;
            match case {
                1 => {
                    current.burner = false;
                    [previous.throttle, current.throttle] = [0.5, 0.6];
                }
                2 => current.engine = false,
                3 => hp = 0,
                4 | 5 | 7 => {
                    current.crashed = true;
                    let mut wreck = tore_sim::wreck::Wreck::new(0, 9, [0.; 3]);
                    if case == 5 {
                        wreck.phase = tore_sim::wreck::Phase::Grounded;
                    }
                    current.wreck = Some(wreck);
                }
                _ => {}
            }
            // Afterburner lit, and still switched on after a crash.
            if matches!(case, 6 | 7) {
                [previous.throttle, current.throttle] = [0.99, 1.];
            }
            assert_eq!(current.afterburner_active(), case == 6);
            // Damage as the combat step leaves it on the flight state.
            combat
                .state
                .preview_localized_damage(DamageSection::LeftWing, 0.8);
            combat.state.player_hp = hp;
            for s in [&mut previous, &mut current] {
                s.damage_fraction = (1. - f64::from(hp) / f64::from(capacity)).clamp(0., 1.);
                s.damage_variant = combat
                    .state
                    .player_damage_section()
                    .map(|section| section as usize);
                s.damage_regions = combat.state.player_damage_regions();
            }
            let snapshots = [&previous, &current].map(|s| combat.snapshot(s, None));
            for alpha in [0., 0.37, 1.] {
                let presented = current.presented(&previous, alpha);
                let frame = interpolate(Some(&snapshots[0]), &snapshots[1], alpha);
                let rebuilt = pose_state(&template, &frame.player);
                for camera in cameras() {
                    assert_eq!(
                        ownship.vertices(&rebuilt, &camera, &world, &scenery),
                        ownship.vertices(&presented, &camera, &world, &scenery),
                        "case {case} at {alpha}"
                    );
                    // Fixtures and the player's debris copy the player state.
                    let [replayed, live] = [&rebuilt, &presented].map(|s| {
                        combat_geometry(&frame, &view.art, &ownship, s, &camera, &world, &scenery)
                            .vertices
                    });
                    assert_eq!(replayed, live, "fixtures, case {case} at {alpha}");
                }
            }
        }
    }
}
