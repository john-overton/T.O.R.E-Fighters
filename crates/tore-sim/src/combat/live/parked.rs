//! Aircraft parked on the ground (docs/spec/surface-defenses.md, "Parked
//! aircraft"): the aircraft a Quick Mission ground target template stands on
//! an apron, in a shelter or on a carrier deck. John (2026-10-10): they are
//! simulated aircraft, not scenery, and they never take off.
//!
//! A parked aircraft is a target row with two natures:
//!
//! - **For weapons it is a surface target**: role [`TargetRole::Surface`], so
//!   a Maverick or a bomb's targeting takes it and an air-to-air missile does
//!   not; on the ground, so radar cannot see it; engines off. Surface units'
//!   rounds pass it by, as they pass every ground object.
//! - **For damage it is an aircraft**: the aircraft volume contact test for
//!   gun rounds and the aircraft sphere for everything else (it has no ground
//!   box), without the weapon's fuze radius (fitted: a bomb does not burst
//!   100 ft above it), hits recorded by damage section, its own PT class
//!   word for the debrief's kill rows. When destroyed it explodes as an aircraft (type
//!   30) and stays where it stood as a burnt hulk (the picture draws its own
//!   shape charred and broken, `tore-app` `scenery::rubble`): no crash site,
//!   no crater and no flying piece (John, 2026-10-13). It may burn: a fire
//!   with its smoke column for 15 minutes on a small scorch under it
//!   ([`State::burn_wrecks`]), none of it on a carrier deck's scorch. An
//!   aircraft killed by splash burns the same way.
//!
//! No critical (pilot) gun kills, no system faults and no jammer: nobody is
//! aboard and nothing is running (agent decision). The fragment offsets and
//! the type the host registers are no longer used by the hulk; they stay
//! so the registration and checkpoints keep their shape.
use super::{
    AIRCRAFT_RADIUS_FT, AircraftId, Basis, Heat, LocalizedDamage, Result, Side, State, Target,
    TargetRole, Vector, sensors,
};

/// One parked aircraft as the host registers it.
#[derive(Clone, Debug, PartialEq)]
pub struct ParkedAircraft {
    /// The surface id (`0x5000_0000` plus the template ordinal).
    pub id: u32,
    /// The aircraft's origin: the centre of its contact volume.
    pub position: Vector,
    pub basis: Basis,
    /// Where its crash site lies: the ground or deck under it.
    pub ground: Vector,
    /// It stands over water (a carrier deck): no crater.
    pub water: bool,
    /// From its PT's OBJECT block.
    pub hit_points: i32,
    /// Its PT class word: 0x8000 fighter, 0x4000 bomber.
    pub category: u16,
    pub side: Side,
    /// PT `sigs[3]` and `sigs[2]`.
    pub radar_signature: f64,
    pub infrared_signature: f64,
    /// Where each of its two fragment pieces leaves it (right, up, forward
    /// feet).
    pub fragment_offsets: [Vector; 2],
    /// The type, when it is one the game flies: its reviewed fragment pairs
    /// say which piece a broken section throws
    /// ([`crate::combat::debris::damage_variant`]). Any other section, and
    /// every type the game does not fly, throws piece 0 for a break forward
    /// and piece 1 aft or at a wing (fitted).
    pub aircraft: Option<AircraftId>,
}

/// What the state keeps of a parked aircraft beyond its target row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParkedSite {
    pub ground: Vector,
    pub water: bool,
    pub aircraft: Option<AircraftId>,
}

impl State {
    /// Registers a parked aircraft as a target row. Fails for an id already
    /// in use, an id of 0 or no hit points.
    pub fn add_parked_aircraft(&mut self, parked: &ParkedAircraft) -> Result<()> {
        let id = parked.id;
        if id == 0
            || parked.hit_points <= 0
            || self.targets.iter().any(|target| target.id == id)
            || self.ground_bounds.contains_key(&id)
        {
            return Err(super::super::invalid(
                "invalid or duplicate parked aircraft",
            ));
        }
        let signature = sensors::SignatureProfile {
            radar: parked.radar_signature,
            infrared: parked.infrared_signature,
            ..sensors::SignatureProfile::default()
        };
        self.targets.push(Target {
            aircraft: None,
            role: TargetRole::Surface,
            heat: Heat::Engine {
                on: false,
                throttle: 0.,
                afterburner: false,
            },
            radar_emitting: false,
            id,
            position: parked.position,
            velocity: [0.; 3],
            basis: parked.basis,
            configuration: sensors::Configuration::CLEAN,
            signature,
            jammer: None,
            jammer_active: false,
            airborne: false,
            on_ground: true,
            radius: AIRCRAFT_RADIUS_FT,
            hp: parked.hit_points,
            initial_hp: parked.hit_points,
            fragment_offsets: parked.fragment_offsets,
            wreck: None,
            wreck_power: crate::wreck::Power::default(),
            fragment_released: false,
            localized_damage: LocalizedDamage::default(),
            faults: Default::default(),
            category: parked.category,
            side: parked.side,
        });
        self.parked.insert(
            id,
            ParkedSite {
                ground: parked.ground,
                water: parked.water,
                aircraft: parked.aircraft,
            },
        );
        Ok(())
    }

    /// Whether `id` is a parked aircraft.
    pub fn is_parked(&self, id: u32) -> bool {
        self.parked.contains_key(&id)
    }

    /// The crash site of parked aircraft `id`.
    pub fn parked_site(&self, id: u32) -> Option<ParkedSite> {
        self.parked.get(&id).copied()
    }
}
