//! Shared recording conversion, with renderer integration tests here.
pub use tore_session::recording::convert::*;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use tore_formats::aircraft::AircraftId;
    use tore_replay as replay;
    use tore_sim::{
        combat::{
            blast,
            live::{self, DamageSection},
        },
        wreck,
    };
    use tore_world::snapshot::*;
    use tore_world::surface::{SurfacePose, UnitId};

    use super::*;

    #[test]
    fn an_aircraft_recorded_by_name_only_is_known_by_its_exact_label() {
        assert_eq!(by_label("A-10 Thunderbolt"), Some(AircraftId::A10));
        assert_eq!(by_label("F- 16C Falcon"), Some(AircraftId::F16C));
        assert_eq!(by_label(""), None);
        assert_eq!(by_label("A-10"), None, "only the exact label");
    }
    use crate::combat_view::render_hash_tests as fixture;
    use crate::replay::tests::TempDir;
    use crate::{
        render_snapshot::{aircraft_batches, combat_geometry},
        snapshot::{interpolate, pose_state},
    };
    use std::path::Path;

    /// A weapon record shaped like the fixture's first station, as the
    /// retail record it stands for would be flagged.
    fn record(source: &str, flags: u32, pod: i16, signature: u8) -> tore_formats::weapons::Weapon {
        let mut w = fixture::combat(Vec::new(), Vec::new())
            .state
            .own()
            .configuration()
            .stations[0]
            .weapon
            .clone();
        w.source = source.into();
        w.flags = flags;
        w.burst.projectiles_in_pod = pod;
        w.seeker.signature = signature;
        w.movement.ignite_t = 0;
        w.movement.fuel_t = 10;
        w
    }

    #[test]
    fn weapons_are_classed_with_bombs_and_rockets_apart_from_missiles() {
        use replay::WeaponClass::{Bomb, Gun, Missile, Other, Rocket};
        // The Mk 82, the LAU-61 and LAU-10 pods and the B-13L: bombs and
        // rockets, where rockets were recorded as other before.
        assert_eq!(weapon_class(&record("MK82.JT", 0x22012, 1, 0)), Bomb);
        for (source, rockets) in [
            ("LAU61.JT", 19),
            ("B8.JT", 19),
            ("B13.JT", 5),
            ("LAU10.JT", 4),
        ] {
            let w = record(source, 0x20846, rockets, 0);
            assert!(is_rocket(&w), "{source}");
            assert_eq!(weapon_class(&w), Rocket, "{source}");
        }
        // One rocket alone, a guided store and a gun are not rockets.
        assert_eq!(weapon_class(&record("ONE.JT", 0x20846, 1, 0)), Other);
        assert_eq!(weapon_class(&record("AGM65G.JT", 0x20001, 1, 2)), Missile);
        assert_eq!(weapon_class(&record("AA.JT", 0x10001, 1, 2)), Missile);
        assert_eq!(weapon_class(&record("M61.JT", 0x80, 1, 0)), Gun);
        // A guided glide bomb (no motor): the laser bombs and the JDAMs are
        // bombs, though the debrief counts them with the missiles; a laser
        // missile with a motor stays a missile.
        let mut gbu = record("GBU10.JT", 0x20401, 1, 1);
        gbu.movement.fuel_t = 0;
        assert_eq!(weapon_class(&gbu), Bomb);
        let mut jdam = record("GBU30.JT", 0x20001, 1, 0);
        jdam.movement.fuel_t = 0;
        assert_eq!(weapon_class(&jdam), Bomb);
        let mut as30 = record("AS30.JT", 0x20401, 1, 1);
        as30.movement.fuel_t = 40;
        assert_eq!(weapon_class(&as30), Missile);
        // A network capture's conversion classes them the same (slice NA).
        let rockets =
            ["LAU61.JT", "B8.JT", "B13.JT", "LAU10.JT"].map(|s| record(s, 0x20846, 19, 0));
        for w in [
            record("MK82.JT", 0x22012, 1, 0),
            record("ONE.JT", 0x20846, 1, 0),
            record("AGM65G.JT", 0x20001, 1, 2),
            record("AA.JT", 0x10001, 1, 2),
            record("M61.JT", 0x80, 1, 0),
            gbu,
            jdam,
            as30,
        ]
        .iter()
        .chain(&rockets)
        {
            assert_eq!(
                tore_session::client::convert::weapon_class(w),
                weapon_class(w),
                "{}",
                w.source
            );
        }
    }

    #[test]
    fn identities_round_trip_without_aliasing_a_variant() {
        for id in AircraftId::SELECTABLE {
            assert_eq!(identity(identity_key(id)), Some(id), "{id:?}");
        }
        // The F/A-XX borrows the F-22N's resources but is never recorded as one.
        assert_ne!(
            identity_key(AircraftId::Faxx),
            identity_key(AircraftId::F22n)
        );
        assert_eq!(identity_key(AircraftId::F18), "F18.PT");
        assert_eq!(identity_key(AircraftId::Rafale), "RAFALE.PT");
        assert_eq!(identity(""), None);
    }

    #[test]
    fn presentation_survives_the_header_and_routes_every_draw_mode() {
        let rules = Presentation {
            models: vec![AircraftId::Mig29, AircraftId::F18, AircraftId::Faxx],
            slots: 5,
            player: 0,
        };
        let header = replay::Header {
            extra: rules.extras(),
            ..Default::default()
        };
        assert_eq!(Presentation::from_header(&header), rules);
        assert_eq!(rules.draw(0, Some(AircraftId::F18)), Draw::Ownship);
        assert_eq!(
            rules.draw(3, Some(AircraftId::Mig29)),
            Draw::Model(AircraftId::Mig29)
        );
        assert_eq!(rules.draw(6, Some(AircraftId::Mig29)), Draw::Hidden);
        assert_eq!(rules.draw(2, Some(AircraftId::Su27)), Draw::Hidden);
        assert_eq!(rules.draw(2, None), Draw::Hidden);
        // No models: the range's fixtures draw with the player's airframe.
        let range = Presentation::from_header(&replay::Header::default());
        assert_eq!(range, Presentation::default());
        assert_eq!(range.draw(4, Some(AircraftId::F18)), Draw::Ownship);
    }

    #[test]
    fn a_recording_names_a_player_other_than_plane_0_in_its_header() {
        let rules = Presentation {
            models: vec![AircraftId::Mig29],
            slots: 4,
            player: 3,
        };
        let header = replay::Header {
            extra: rules.extras(),
            ..Default::default()
        };
        assert_eq!(header.extra(PLAYER_KEY), Some("3"));
        assert_eq!(Presentation::from_header(&header), rules);
        // Plane 3 draws with the player's airframe and plane 0 is one of the
        // others, drawn with its own model.
        assert_eq!(rules.draw(3, Some(AircraftId::Mig29)), Draw::Ownship);
        assert_eq!(
            rules.draw(0, Some(AircraftId::Mig29)),
            Draw::Model(AircraftId::Mig29)
        );
        // Plane 0's recordings carry no such entry.
        let single = Presentation {
            player: 0,
            ..rules.clone()
        };
        assert!(
            single
                .extras()
                .iter()
                .all(|(key, _)| key.as_str() != PLAYER_KEY)
        );
        // The frame's player is plane 3's state.
        let frame = replay::Frame {
            tick: 5,
            aircraft: [0, 3, 4]
                .into_iter()
                .map(|id| replay::AircraftState {
                    id,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let picture = snapshot(&frame, &[], &rules, &Identities::default());
        assert_eq!(picture.player.id, 3);
        assert_eq!(
            picture.targets.iter().map(|t| t.id).collect::<Vec<_>>(),
            [0, 4]
        );
        let single = snapshot(&frame, &[], &single, &Identities::default());
        assert_eq!(single.player.id, 0);
    }

    #[test]
    fn surface_units_debris_pieces_and_flak_rebuild_the_picture_they_were_recorded_from() {
        let parked = 0x5000_0007;
        let live = RenderSnapshot {
            tick: 500,
            models: vec![AircraftId::Mig29],
            surface: vec![
                SurfacePose {
                    id: UnitId(0x5000_0001),
                    position: [10_000.25, 12., 20_000.5],
                    attitude: [1.5, 0.02, -0.01],
                    shape: None,
                    wrecked: false,
                },
                SurfacePose {
                    id: UnitId(0x5000_0002),
                    position: [10_100., 11., 20_050.],
                    attitude: [0.5, 0., 0.],
                    shape: None,
                    wrecked: true,
                },
            ],
            // An aircraft's piece follows its section; a parked aircraft's
            // is the piece it threw.
            debris: vec![
                DebrisPose {
                    owner: parked,
                    draw: Draw::Hidden,
                    position: [5., 6., 7.],
                    attitude: [0.1, 0.2, 0.3],
                    variant: Some(1),
                },
                DebrisPose {
                    owner: parked,
                    draw: Draw::Hidden,
                    position: [8., 9., 10.],
                    attitude: [0.; 3],
                    variant: Some(0),
                },
            ],
            ..RenderSnapshot::default()
        };
        let frame = replay::Frame {
            tick: 500,
            surface: surface_states(&live.surface),
            debris: debris_states(&live.debris),
            debris_pieces: debris_pieces(&live.debris),
            ..Default::default()
        };
        assert_eq!(
            frame.debris_pieces,
            [(parked, 0, 1), (parked, 1, 0)],
            "each piece by its place among its owner's"
        );
        let rules = Presentation {
            models: vec![AircraftId::Mig29],
            slots: 4,
            player: 0,
        };
        let rebuilt = snapshot(&frame, &[], &rules, &Identities::default());
        assert_eq!(rebuilt.surface, live.surface);
        assert_eq!(difference(&live, &rebuilt), None);
        // A pose that is not where it was recorded is a difference.
        let mut moved = rebuilt.clone();
        moved.surface[0].position[0] += 1.;
        assert!(
            difference(&live, &moved)
                .unwrap()
                .starts_with("surface unit differs")
        );
        let mut hidden = rebuilt;
        hidden.surface.pop();
        assert!(difference(&live, &hidden).unwrap().contains("routed"));
        // The flak keeps its explosion type, the KS-19's 28 included.
        for explosion in [27, 28] {
            let kind = effect_kind(live::EffectKind::Flak, Some(explosion));
            assert_eq!(kind, replay::EffectKind::Flak { explosion });
            assert_eq!(
                live_effect_kind(kind),
                Some((live::EffectKind::Flak, Some(explosion)))
            );
        }
        // A flak burst with no type a recording can name is an ordinary hit.
        assert_eq!(
            effect_kind(live::EffectKind::Flak, None),
            replay::EffectKind::Hit
        );
    }

    fn effect(kind: live::EffectKind, x: f64, ticks: u16) -> EffectPose {
        EffectPose {
            kind,
            position: [x, 5000., 0.],
            ticks,
            blast: None,
            scale: tore_sim::combat::blast_scale::FULL,
        }
    }

    #[test]
    fn explosion_types_survive_recording_and_marks_rebuild_with_their_caps() {
        use live::EffectKind::{Destroyed, Ground, Launch};
        let pose = |kind, blast| EffectPose {
            kind,
            position: [1., 2., 3.],
            ticks: 100,
            blast,
            scale: tore_sim::combat::blast_scale::FULL,
        };
        for (kind, blast) in [
            (Destroyed, Some(33)),
            (Ground, Some(15)),
            (Destroyed, None),
            (Launch, None),
        ] {
            let recorded = replay::LiveEffect {
                spawn_tick: 0,
                age_ticks: 0,
                kind: effect_kind(kind, blast),
                position: [1., 2., 3.],
                duration_ticks: 100,
                scale: tore_replay::EffectSpawn::FULL_SCALE,
            };
            assert_eq!(effect_pose(&recorded), Some(pose(kind, blast)));
        }
        // A type outside the table is recorded as its plain kind.
        assert_eq!(
            effect_kind(Destroyed, Some(99)),
            replay::EffectKind::Destroyed
        );
        let fire = blast::Mark {
            position: [5., 0., 5.],
            kind: blast::MarkKind::Fire,
            ticks: 1000,
            born: 10,
            serial: 0,
            scale: 100,
        };
        let mut marks = vec![(11, mark_spawn(&fire))];
        for i in 0..=300_u64 {
            let crater = blast::Mark {
                position: [i as f64, 0., 0.],
                kind: blast::MarkKind::Crater(9),
                ticks: blast::FOREVER,
                born: i,
                serial: i,
                scale: 159,
            };
            marks.push((i, mark_spawn(&crater)));
        }
        let at = marks_at(&marks, 500);
        let fires: Vec<_> = at
            .iter()
            .filter(|m| m.kind == blast::MarkKind::Fire)
            .collect();
        assert_eq!(fires.len(), 1);
        assert_eq!(fires[0].age, 489);
        // Every crater stays, at its recorded scale (no cap since
        // 2026-10-12).
        assert_eq!(at.len(), 301 + 1);
        assert!(
            at.iter()
                .all(|m| m.kind == blast::MarkKind::Fire || m.scale == 159)
        );
        // The fire is out 1,000 ticks after it was recorded.
        assert!(
            marks_at(&marks, 1011)
                .iter()
                .all(|m| m.kind != blast::MarkKind::Fire)
        );
        assert!(marks_at(&marks, 5).len() == 6);
        // A building's fire burns for good and is outside the cap: 100 of
        // them and a crowd of crash fires past the 64.
        let mut crowd = Vec::new();
        for i in 0..100_u64 {
            let building = blast::Mark {
                position: [i as f64, 0., 50.],
                kind: blast::MarkKind::Fire,
                ticks: blast::FOREVER,
                born: i,
                serial: i,
                scale: 100,
            };
            crowd.push((i, mark_spawn(&building)));
            let crash = blast::Mark {
                position: [i as f64, 0., 90.],
                kind: blast::MarkKind::Fire,
                ticks: blast::CRASH_TICKS,
                born: i,
                serial: 1_000 + i,
                scale: 100,
            };
            crowd.push((i, mark_spawn(&crash)));
        }
        let at = marks_at(&crowd, 1_000);
        let forever = at.iter().filter(|m| m.position[2] == 50.).count();
        let crashes: Vec<_> = at.iter().filter(|m| m.position[2] == 90.).collect();
        assert_eq!(forever, 100);
        assert_eq!(crashes.len(), blast::MAX_FIRES);
        // The newest crash fires stay.
        assert_eq!(crashes[0].position[0], 36.);
        // A thousand ticks (and a quarter hour) on, the buildings burn alone.
        let late = marks_at(&crowd, u64::from(blast::CRASH_TICKS) + 1_000);
        assert_eq!(late.len(), 100);
    }

    #[test]
    fn an_effect_dropped_at_the_cap_stays_gone_after_others_run_out() {
        let spawn = |x: f64, duration_ticks: u32| replay::EffectSpawn {
            kind: replay::EffectKind::Destroyed,
            position: [x, 0., 0.],
            duration_ticks,
            scale: replay::EffectSpawn::FULL_SCALE,
        };
        // A full list on tick 0, its newest lasting 5 ticks; a new effect on
        // tick 1 drops the oldest, which still had 239 ticks to play.
        let mut spawns: Vec<(u64, replay::EffectSpawn)> = (0..live::MAX_EFFECTS - 1)
            .map(|i| (0, spawn(i as f64, 240)))
            .collect();
        spawns.push((0, spawn(100., 5)));
        spawns.push((1, spawn(200., 240)));
        // A crater recorded alongside is a mark: it drops nothing.
        spawns.push((
            1,
            replay::EffectSpawn {
                kind: replay::EffectKind::Crater(0),
                duration_ticks: u32::MAX,
                ..spawn(300., 0)
            },
        ));
        let at = |tick| -> Vec<f64> {
            cap_effects(&spawns, tick)
                .iter()
                .map(|e| e.position[0])
                .collect()
        };
        // Tick 0 shows the full list, not the start still to come.
        assert_eq!(at(0).len(), live::MAX_EFFECTS);
        assert_eq!(at(0)[0], 0.);
        let one = at(1);
        assert_eq!(one.len(), live::MAX_EFFECTS);
        assert_eq!((one[0], one[live::MAX_EFFECTS - 1]), (1., 200.));
        // The short one ran out; the dropped one does not come back.
        let ten = at(10);
        assert_eq!(ten.len(), live::MAX_EFFECTS - 1);
        assert!(!ten.contains(&0.) && !ten.contains(&100.));
        assert_eq!(ten.last(), Some(&200.));
    }

    #[test]
    fn effects_are_recorded_once_when_they_start() {
        use live::EffectKind::{Destroyed, Flare, Hit};
        let mut watch = EffectWatch::default();
        let first = vec![effect(Hit, 1., 45), effect(Flare, 2., 2)];
        let started = watch.started(&first);
        assert_eq!(started.len(), 2);
        assert_eq!(started[1].duration_ticks, 2);
        // One tick on: one effect was added between ticks (already a tick
        // old) and one during this tick.
        let second = vec![
            effect(Hit, 1., 44),
            effect(Flare, 2., 1),
            effect(Destroyed, 3., 239),
            effect(Hit, 4., 45),
        ];
        let started = watch.started(&second);
        assert_eq!(
            started
                .iter()
                .map(|s| (s.position[0], s.duration_ticks))
                .collect::<Vec<_>>(),
            [(3., 239), (4., 45)]
        );
        // The flare ran out and the list hit its cap: the oldest was dropped.
        let third = vec![
            effect(Destroyed, 3., 238),
            effect(Hit, 4., 44),
            effect(Hit, 5., 45),
        ];
        let started = watch.started(&third);
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].position[0], 5.);
        // Nothing new.
        let fourth: Vec<_> = third
            .iter()
            .map(|e| EffectPose {
                ticks: e.ticks - 1,
                ..*e
            })
            .collect();
        assert!(watch.started(&fourth).is_empty());
    }

    /// Records a pair of snapshots as two consecutive ticks, the way the
    /// recorder does, and rebuilds them from the file.
    fn round_trip(dir: &Path, name: &str, live: &[RenderSnapshot; 2]) -> [RenderSnapshot; 2] {
        let presentation = Presentation::of(&live[1]);
        let header = replay::Header {
            extra: presentation.extras(),
            ..Default::default()
        };
        let mut writer = replay::Writer::create(dir.join(name), &header).unwrap();
        let mut weapons: BTreeMap<String, u32> = BTreeMap::new();
        let mut effects = EffectWatch::default();
        for snapshot in live {
            let aircraft: Vec<&AircraftPose> = std::iter::once(&snapshot.player)
                .chain(&snapshot.targets)
                .filter(|pose| pose.aircraft.is_some())
                .collect();
            for pose in &aircraft {
                writer
                    .register_aircraft(&replay::AircraftInfo {
                        id: pose.id,
                        pt: identity_key(pose.aircraft.unwrap()).into(),
                        ..Default::default()
                    })
                    .unwrap();
            }
            let data = FlightData {
                airspeed: 700.,
                g: 1.5,
                fuel_lb: 5_000.,
                controls: [0.1, -0.2, 0., 0.8],
                alive: true,
                ..Default::default()
            };
            let mut projectiles = Vec::new();
            for p in &snapshot.projectiles {
                let next = weapons.len() as u32;
                let id = *weapons.entry(p.weapon.clone()).or_insert(next);
                writer
                    .register_weapon(&replay::WeaponInfo {
                        id,
                        source: p.weapon.clone(),
                        shape: p.shape.clone(),
                        name: p.weapon.clone(),
                        class: if p.gun {
                            replay::WeaponClass::Gun
                        } else {
                            replay::WeaponClass::Missile
                        },
                    })
                    .unwrap();
                projectiles.push(projectile_state(p, id, 3, None));
            }
            writer
                .push(&replay::Frame {
                    tick: snapshot.tick,
                    aircraft: aircraft
                        .iter()
                        .map(|pose| aircraft_state(pose, &data))
                        .collect(),
                    projectiles,
                    debris: debris_states(&snapshot.debris),
                    escapees: snapshot.pilots.iter().map(escapee_state).collect(),
                    new_effects: effects.started(&snapshot.effects),
                    ..Default::default()
                })
                .unwrap();
        }
        let path = writer.finish(&replay::Footer::default()).unwrap();
        let recording = replay::Recording::open(path).unwrap();
        assert!(recording.complete() && recording.problems().is_empty());
        let identities = Identities::of(&recording);
        let presentation = Presentation::from_header(recording.header());
        live.each_ref().map(|live| {
            let frame = recording.frame(live.tick).unwrap().unwrap();
            let effects = playing_effects(&recording, live.tick).unwrap();
            snapshot(&frame, &effects, &presentation, &identities)
        })
    }

    /// Vertex streams drawn from a live and a replayed picture agree: the
    /// same vertices in the same order, each value within the position
    /// precision of 1/64 ft (quantized angles and devices move a vertex far
    /// less than that).
    fn assert_same_vertices(what: &str, live: &[f32], replayed: &[f32]) {
        assert_eq!(live.len(), replayed.len(), "{what}: vertex counts differ");
        let worst = live
            .iter()
            .zip(replayed)
            .map(|(a, b)| (a - b).abs() / (1. + a.abs() * 1e-4))
            .fold(0f32, f32::max);
        let bound = (tore_replay::precision::POSITION_FT / 2.) as f32 + 1e-3;
        assert!(worst < bound, "{what}: vertices differ by {worst}");
    }

    #[test]
    fn recorded_frames_draw_the_live_picture() {
        let dir = TempDir::new("convert");
        let ownship = fixture::hornet_airframe(true);
        let player = fixture::player();
        let world = tore_world::test_support::terrain();
        let scenery = crate::scenery::tests::scenery();
        let art = fixture::escape_art();
        let mut drawn = 0;
        for with_models in [true, false] {
            let (mut combat, view) = if with_models {
                fixture::pair(
                    fixture::models(),
                    (0..7).map(|i| (i % 3, [0.; 3])).collect(),
                )
            } else {
                fixture::pair(Vec::new(), Vec::new())
            };
            let scene = fixture::scene(combat.state.own().configuration());
            for ai_poses in [true, false] {
                let mut live = fixture::snapshots(&mut combat, &scene, ai_poses, &player);
                for (tick, snapshot) in (40..).zip(&mut live) {
                    snapshot.tick = tick;
                }
                // The scene's effects start on its current tick. (On the
                // second pass the reused combat state would otherwise show
                // them un-aged on the previous tick too, which live flight
                // never does: every effect ages each tick.)
                live[0].effects.clear();
                // Every ejection phase, the player's first.
                live[1].pilots = fixture::pilots()
                    .iter()
                    .enumerate()
                    .map(|(owner, escape)| PilotPose {
                        owner: owner as u32,
                        position: escape.position,
                        heading: escape.heading,
                        phase: escape.phase,
                        crew: false,
                    })
                    .collect();
                // Live flight sets the player's damage variant from the combat
                // state's structural section every tick; the fixture's flight
                // state and combat state are separate, so align them.
                for snapshot in &mut live {
                    snapshot.player.damage.structural = player
                        .damage_variant
                        .and_then(|variant| section(variant as u8));
                }
                // A falling player wreck.
                let player_pose = &mut live[1].player;
                player_pose.wreck = Some(wreck::Phase::Falling);
                player_pose.crashed = true;
                player_pose.damage.hp = 0;
                let name = format!("scene-{with_models}-{ai_poses}.tore-replay");
                let mut replayed = round_trip(dir.path(), &name, &live);
                for (a, b) in live.iter().zip(&replayed) {
                    assert_eq!(difference(a, b), None);
                }
                // A recording keeps no motor state: the viewer dates each
                // round's motor from its recorded age and the import
                // (`Playback::add_motors`), which this test has no import
                // for, so it takes the live picture's.
                for (a, b) in live.iter().zip(replayed.iter_mut()) {
                    for (pa, pb) in a.projectiles.iter().zip(b.projectiles.iter_mut()) {
                        pb.powered = pa.powered;
                    }
                }
                // The drawable player state a replay rebuilds.
                let [a, b] = [&live[1].player, &replayed[1].player].map(|p| pose_state(&player, p));
                assert_eq!(
                    (
                        a.engine,
                        a.burner,
                        a.crashed,
                        a.damage_variant,
                        a.damage_regions
                    ),
                    (
                        b.engine,
                        b.burner,
                        b.crashed,
                        b.damage_variant,
                        b.damage_regions
                    )
                );
                assert!(!far(a.auxiliary_rates, b.auxiliary_rates, tolerance::RATE));
                for alpha in [0., 0.37, 1.] {
                    let [a, b] =
                        [&live, &replayed].map(|pair| interpolate(Some(&pair[0]), &pair[1], alpha));
                    for camera in fixture::cameras() {
                        if with_models {
                            let batches = [&a, &b].map(|s| {
                                aircraft_batches(
                                    s,
                                    &view.models,
                                    &view.art,
                                    &camera,
                                    &world,
                                    &scenery,
                                )
                            });
                            assert_eq!(batches[0].len(), batches[1].len());
                            for ((ma, va, ca), (mb, vb, cb)) in batches[0].iter().zip(&batches[1]) {
                                assert_eq!(ma.profile.id, mb.profile.id);
                                assert_same_vertices("model batch", va, vb);
                                assert_eq!(ca.len(), cb.len());
                                drawn += va.len();
                            }
                        }
                        let [ga, gb] = [&a, &b].map(|s| {
                            combat_geometry(
                                s, &view.art, &ownship, &player, &camera, &world, &scenery,
                            )
                        });
                        assert_same_vertices("combat geometry", &ga.vertices, &gb.vertices);
                        assert_eq!(ga.contacts.len(), gb.contacts.len());
                        drawn += ga.vertices.len();
                        let [pa, pb] = [&a, &b].map(|s| {
                            art.vertices_for(
                                s.pilots.iter().map(|p| (p.position, p.heading, p.phase)),
                                &ownship.palette,
                                camera.position,
                                [0.; 3],
                            )
                        });
                        assert_same_vertices("ejected pilots", &pa, &pb);
                    }
                }
            }
        }
        // The scene really drew models, fixtures, weapons and effects.
        assert!(drawn > 100_000, "{drawn}");
    }

    #[test]
    fn a_frame_rebuilds_every_state_it_can_hold() {
        use tore_sim::ejection::Phase;
        let pose = AircraftPose {
            id: 3,
            aircraft: Some(AircraftId::Mig29),
            draw: Draw::Model(AircraftId::Mig29),
            position: [1., 2., 3.],
            attitude: [0.5, -0.2, 3.],
            velocity: [100., 0., -200.],
            devices: Some([
                1., 0.5, 0.25, 0., 1., 0.75, -0.5, 0.5, 1., 812., 0.9, 0.8, -0.6, 0.4, 0.2, 0., 0.,
                0., 0., 0., 0., 0.,
            ]),
            engine: Engine {
                lit: true,
                afterburner: true,
                rates: [0.1, -0.4, 0.2],
                rotor: 1.043,
                rotor_turns: 0.,
                rotor_tilt: [[0.04, -0.0123], [-0.2, 0.6]],
                flame: true,
            },
            damage: Damage {
                hp: 12,
                initial_hp: 90,
                sections: [1, 2, 3, 4, 5, 6],
                structural: Some(DamageSection::RightWing),
            },
            airborne: true,
            wreck: Some(wreck::Phase::Grounded),
            crashed: true,
            stores: Vec::new(),
        };
        for phase in [
            None,
            Some(wreck::Phase::Falling),
            Some(wreck::Phase::Grounded),
            Some(wreck::Phase::Exploded),
        ] {
            assert_eq!(wreck_phase(wreck_code(phase)), phase);
        }
        for phase in [
            Phase::Seat,
            Phase::Freefall,
            Phase::Inflating,
            Phase::Parachute,
            Phase::Landed,
            Phase::Impact,
        ] {
            assert_eq!(escape_phase(escape_code(phase)), phase);
        }
        let data = FlightData {
            airspeed: 820.,
            g: 4.5,
            fuel_lb: 3_000.,
            controls: [0.5, -0.5, 0.1, 1.],
            on_ground: false,
            alive: false,
            ejected: true,
            wreck_gone: false,
        };
        let state = aircraft_state(&pose, &data);
        assert_eq!(state.heat, 1.);
        assert!(state.flags.animated && state.flags.ejected && !state.flags.alive);
        assert_eq!(aircraft_pose(&state, pose.aircraft, pose.draw), pose);
        // A fixture nothing animates keeps the neutral pose.
        let fixture = AircraftPose {
            devices: None,
            ..pose.clone()
        };
        let state = aircraft_state(&fixture, &data);
        assert!(!state.flags.animated);
        assert_eq!(state.devices, [0.; DEVICES]);
        assert_eq!(
            aircraft_pose(&state, pose.aircraft, pose.draw).devices,
            None
        );
    }
}
