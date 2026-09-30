//! What the simulation knows about one aircraft type: the imported profile,
//! its flight model, its sensors and where its engines exhaust. Nothing here
//! draws, so the mission core can hold it without art, menus or animation rigs.
//! The app's drawn half, `Airframe`, wraps one of these.
use crate::{WorldResult, resources::ResourceSource, terrain::Terrain};
use std::sync::Arc;
use tore_formats::aircraft::{Aircraft, AircraftId};
use tore_sim::flight;
use tore_sim::{attitude::Vector, models::AircraftModel, sensors::SensorProfiles};

/// One aircraft type as the simulation needs it. The player's flight, combat
/// and the AI take a reference to one; `World::restart` starts the player's
/// flight from it.
pub struct AircraftType {
    /// The imported aircraft record: identity, envelopes, hardpoints and name.
    pub profile: Aircraft,
    /// The flight model a flight of this type starts with.
    model: AircraftModel,
    /// Imported sensor capability for this identity, resolved by record
    /// channel. Used for capability reporting and scope labels.
    pub sensors: SensorProfiles,
    /// Where each engine's exhaust leaves the aircraft, in feet right, up and
    /// forward of its reference point. Contrails and afterburner lights
    /// attach here. They come from the drawn model's nozzle faces, so the
    /// loader that reads the shape works them out.
    pub contrail_offsets: Vec<Vector>,
}

/// Loads the simulation half of an aircraft type from the imported resources:
/// the profile, the flight model and the sensors, with no engine outlets.
/// A headless host has no drawn model to take them from; see
/// [`AircraftType::load`].
pub fn load_type(resources: &dyn ResourceSource, id: AircraftId) -> WorldResult<Arc<AircraftType>> {
    Ok(Arc::new(AircraftType::load(resources, id)?))
}

impl AircraftType {
    /// Loads the simulation half of an aircraft type: parses the profile,
    /// checks that it is the identity asked for, and builds the flight model
    /// and the sensor profiles. The engine outlets are empty: they come from
    /// the drawn model's nozzle faces, so the app, which loads the shape, sets
    /// `contrail_offsets` on what this returns; a host with no drawing leaves
    /// them empty and a client works them out from its own art.
    pub fn load(resources: &dyn ResourceSource, id: AircraftId) -> WorldResult<Self> {
        let get = |name: &str| {
            resources
                .get(name)
                .ok_or_else(|| format!("aircraft cache missing {name}; re-import media"))
        };
        let mut profile = Aircraft::parse(get(id.pt())?)?;
        if profile.id != id.source() || profile.shape != format!("{}.SH", id.stem()) {
            return Err(
                "aircraft identity/shape does not match the selected retail profile".into(),
            );
        }
        profile.id = id;
        let sensors = SensorProfiles::from_source(&profile, |name| {
            get(name).cloned().map_err(std::io::Error::other)
        })?;
        let model = AircraftModel::for_aircraft(&profile)?;
        Ok(Self::new(profile, model, sensors, Vec::new()))
    }

    pub fn new(
        profile: Aircraft,
        model: AircraftModel,
        sensors: SensorProfiles,
        contrail_offsets: Vec<Vector>,
    ) -> Self {
        Self {
            profile,
            model,
            sensors,
            contrail_offsets,
        }
    }

    /// The player's flight state at the start of a free flight: over the
    /// theater's camera start, 2,000 feet above the ground or at 5,000 feet,
    /// whichever is higher.
    pub fn start(&self, world: &Terrain) -> flight::State {
        let mut p = world.free_flight_start();
        p[1] = 5000f64.max(world.height(p[0] as f32, p[2] as f32) as f64 + 2000.);
        let mut state = flight::State::from_model(self.model.clone(), p);
        // State velocity is ground-relative; initialize the requested airspeed
        // with advection already present so the first tick does not subtract it twice.
        for (v, wind) in state.velocity.iter_mut().zip(world.wind()) {
            *v += wind;
        }
        state
    }
}

#[cfg(any(test, feature = "test-support"))]
impl AircraftType {
    /// A type built from the synthetic F/A-18D record and flight model, so
    /// tests run without retail media. `id` selects the identity the drawing
    /// and combat rules see.
    pub fn synthetic(
        id: tore_formats::aircraft::AircraftId,
        contrail_offsets: Vec<Vector>,
    ) -> Self {
        let source = crate::test_support::profile();
        let model = AircraftModel::for_aircraft(&source).expect("synthetic flight model");
        let mut profile = source;
        profile.id = id;
        let sensors = SensorProfiles {
            aircraft: id,
            radar: None,
            infrared: None,
            visual: None,
            jammer: None,
            signature: Default::default(),
        };
        Self::new(profile, model, sensors, contrail_offsets)
    }
}
