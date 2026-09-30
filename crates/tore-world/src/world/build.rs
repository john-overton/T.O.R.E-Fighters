//! Building a mission: [`World::new`] turns a [`MissionSpec`] and the imported
//! resources into a running [`World`], with no window, screen or environment
//! variable in the way. The game's creator builds through it, and so will a
//! dedicated server. See docs/ARCHITECTURE.md, "A mission with no window".

use super::{AiSetup, Cockpit, Restarted, Setup, World};
use crate::{
    WorldResult, ai_wings,
    aircraft_type::{AircraftType, load_type},
    combat::Combat,
    comms, crew_voice,
    mission::MissionSpec,
    mission_layout,
    resources::ResourceSource,
    seats::{PlaneId, Roster},
};
use std::sync::Arc;
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::combat::loadout::Loadout;

/// Who flies what when the mission is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seating {
    /// Single player: seat 0 flies plane 0, the lead of friendly wing 1, with
    /// the loadout the spec gives it; the AI flies every other plane.
    SinglePlayer,
    // `Open`, every plane on the AI and humans taking planes by handoff,
    // is slice D3c.
}

/// What a caller can supply while a mission is built. The default supplies
/// nothing: every aircraft type loads through [`load_type`] and weapons keep
/// the names they are read with.
#[derive(Default)]
pub struct Hooks<'a> {
    /// The player's aircraft type, when the caller has loaded it already (the
    /// game loads it with its drawn half).
    pub player: Option<Arc<AircraftType>>,
    /// Loads each other aircraft type the mission flies, the first time it is
    /// needed. The game loads the drawn model beside the type and keeps it.
    pub load: Option<&'a mut dyn FnMut(AircraftId) -> WorldResult<Arc<AircraftType>>>,
    /// Tidies a weapon's display name as the game's loadout screen does, for
    /// every weapon of the player's load.
    pub weapon_label: Option<&'a dyn Fn(&mut Weapon)>,
}

/// A mission just built: the world, and what starting it reported.
pub struct Built {
    pub world: World,
    pub restarted: Restarted,
}

impl World {
    /// Builds the mission `spec` describes from the imported `resources`: the
    /// terrain for the theater and weather, the aircraft types, the layout,
    /// combat with the player's loadout and the other aircraft, the AI wings,
    /// and the start. The world is ready to step.
    pub fn new(
        spec: &MissionSpec,
        resources: &dyn ResourceSource,
        seating: Seating,
    ) -> WorldResult<World> {
        Ok(Self::build(spec, resources, seating, &mut Hooks::default())?.world)
    }

    /// [`World::new`], with the caller's [`Hooks`], returning what the start
    /// reported too. A start runs once, here: a caller that goes on to call
    /// [`World::restart`] starts the mission again.
    pub fn build(
        spec: &MissionSpec,
        resources: &dyn ResourceSource,
        seating: Seating,
        hooks: &mut Hooks<'_>,
    ) -> WorldResult<Built> {
        let Seating::SinglePlayer = seating;
        spec.validate()?;
        let terrain = crate::terrain::Terrain::for_mission(
            resources,
            &spec.theater,
            Some(spec.condition.index()),
            &spec.weather,
        )?;
        let player = match hooks.player.clone() {
            Some(player) => player,
            None => load_type(resources, spec.player())?,
        };

        // The player's load: the spec's, or the aircraft's standard one.
        let label = hooks.weapon_label;
        let standard = Loadout::new(&player.profile, |name| {
            resources
                .get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing loadout resource {name}")))
        })?;
        let load = match &spec.loadout {
            Some(load) => load.apply(standard, resources, label)?,
            None => {
                let mut load = standard;
                if let Some(label) = label {
                    for station in &mut load.configuration.stations {
                        label(&mut station.weapon);
                    }
                }
                if spec.guns_only {
                    load.restrict_to_guns();
                }
                load
            }
        };
        if spec.guns_only
            && load
                .configuration
                .stations
                .iter()
                .zip(&load.quantities)
                .any(|(s, n)| s.weapon.source != load.aircraft.gun() && *n > 0)
        {
            return Err("Guns only is selected. Unload other weapons or return to setup and change the restriction.".into());
        }

        let altitude = f64::from(spec.start.altitude_ft());
        let selected_ground = spec.ground_runway();
        if selected_ground.is_some() && !spec.researched_flight {
            return Err("Ground start requires the researched flight model. Choose Airborne for this adapter.".into());
        }
        let wings = spec.wing_launches().map_err(|error| error.to_string())?;
        // A ground start parks the player's whole wing on the runway; the
        // straight-flight fixtures keep only the player there.
        let parked = if spec.fixture_wings {
            1
        } else {
            spec.player_wing_size()
        };
        let mut start = player.start(&terrain);
        let ground_layout = match selected_ground {
            Some(object) => {
                start.enable_research(1)?;
                let layout = mission_layout::ground_layout(&terrain, object, parked)?;
                mission_layout::place_on_runway(&terrain, &mut start, &layout, 0)?;
                Some(layout)
            }
            None => None,
        };
        let layout = mission_layout::MissionLayout::plan(
            &terrain,
            &start,
            ground_layout,
            &ai_wings::enemy_group_offsets(&wings),
            spec.separation_feet(),
        );
        let ground = f64::from(terrain.height(start.position[0] as f32, start.position[2] as f32));
        // Only aircraft that start in the air need the altitude to clear the
        // ground; parked wingmen do not.
        let airborne_wings = if spec.fixture_wings {
            spec.fixture_pairs().iter().any(|(_, count)| *count > 0)
        } else {
            wings.iter().any(|wing| {
                !wing.is_empty()
                    && (layout.ground.is_none()
                        || wing.wing.side.is_enemy()
                        || wing.wing.index != 0)
            })
        };
        if (selected_ground.is_none() || airborne_wings) && altitude < ground + 100. {
            return Err(format!(
                "Airborne altitude must exceed {:.0} feet here. Choose a higher altitude.",
                ground + 100.
            )
            .into());
        }

        // Combat, with the other aircraft of the mission.
        let fuel = load.fuel_lbs;
        let mut combat = Combat::with_loadout(&player, &load)?;
        combat.add_airport_targets(&terrain.airport_scene)?;
        let mut default_load = |id| load_type(resources, id);
        let loader: &mut dyn FnMut(AircraftId) -> WorldResult<Arc<AircraftType>> =
            match hooks.load.as_deref_mut() {
                Some(load) => load,
                None => &mut default_load,
            };
        if spec.fixture_wings {
            combat.mission_layout = Some(layout.clone());
            combat.mission_dummies(
                &spec.fixture_pairs(),
                layout.enemy.distance_ft,
                resources,
                loader,
            )?;
        } else {
            combat.mission_aircraft(&wings, &layout, resources, loader)?;
        }

        let setup = Setup {
            mission: Some((altitude, fuel)),
            ground_start: selected_ground,
            researched_flight: spec.researched_flight,
            native_tables: None,
            ai: (!spec.fixture_wings).then_some(AiSetup {
                wings,
                guns_only: spec.guns_only,
                preset: spec.preset,
                flight_model: spec.ai_flight_model,
                group_objectives: spec.objectives,
                group_must_survive: spec.must_survive,
            }),
        };

        // The first cockpit is a free start that `restart` places.
        let flight = player.start(&terrain);
        let airport_service = tore_sim::airport::Service::new(&terrain.airport_scene)
            .map_err(std::io::Error::other)?;
        let mut world = World {
            setup,
            roster: Roster::single_player(comms::crew(&player.profile), []),
            cockpits: vec![Cockpit {
                plane: PlaneId(0),
                previous_flight: flight.clone(),
                flight,
                turbulence: Default::default(),
                turbulence_rng: tore_formats::flight_model::clock_rng::NativeRng::seeded(1)?,
                airport_service,
                airport_nav_mode: false,
                airfield_radio: Default::default(),
                crew_voice: crew_voice::CrewVoice::new(&player.profile),
                result: Default::default(),
                overspeed_message_at: None,
                edge_message_at: None,
            }],
            terrain,
            combat,
            ai_wings: None,
            comms: comms::Comms::new(1),
            wing_status: Default::default(),
            radio: Default::default(),
            phrases: comms::phrases(resources),
        };
        let restarted = world.restart(&player, resources)?;
        // Cheats reach the mission as the Settings command does. The default
        // is not sent, so a setting the build made (the AI's guns only) stands.
        if spec.cheats != tore_sim::cheats::Cheats::default() {
            world.apply_mission_command(&super::MissionCommand::Settings(super::Settings {
                cheats: spec.cheats,
            }))?;
        }
        Ok(Built { world, restarted })
    }
}
