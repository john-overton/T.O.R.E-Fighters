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
//!   box), hits recorded by damage section, its own PT class word for the
//!   debrief's kill rows. When destroyed it explodes as an aircraft (type
//!   30), throws its fragment, and leaves the ground-crash crater and a fire
//!   with its smoke column for 15 minutes ([`State::aircraft_crashed`]'s
//!   marks), without the crash explosion: it did not fall. On a carrier deck
//!   there is no crater.
//!
//! No critical (pilot) gun kills, no system faults and no jammer: nobody is
//! aboard and nothing is running (agent decision). Its fragment is the
//! aircraft's own piece shape, released at the attachment point the host
//! gives and tossed up and outward (fitted).
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

/// How fast a parked aircraft's fragment is thrown when it is destroyed:
/// upward and outward along its attachment offset, feet per second
/// (fitted).
const FRAGMENT_TOSS_UP_FPS: f64 = 45.;
const FRAGMENT_TOSS_OUT_FPS: f64 = 25.;

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

    /// Parked aircraft `id` was destroyed: its crash crater (on land) and
    /// fire, once, and its fragment when a section broke.
    pub(super) fn parked_destroyed(&mut self, id: u32) {
        use crate::combat::blast::{self, MarkKind};
        let Some(site) = self.parked.get(&id).copied() else {
            return;
        };
        if !self.crashed.insert(id) {
            return;
        }
        if !site.water {
            self.crater(site.ground, blast::CRASH_CRATER, blast::CRASH_TICKS);
        }
        if self
            .marks
            .iter()
            .filter(|m| m.kind == MarkKind::Fire)
            .count()
            == blast::MAX_FIRES
            && let Some(oldest) = self.marks.iter().position(|m| m.kind == MarkKind::Fire)
        {
            self.marks.remove(oldest);
        }
        self.mark(site.ground, MarkKind::Fire, blast::CRASH_TICKS);
        let Some(t) = self.targets.iter_mut().find(|t| t.id == id) else {
            return;
        };
        let reviewed = site
            .aircraft
            .zip(t.localized_damage.structural_section)
            .and_then(|(aircraft, section)| {
                crate::combat::debris::damage_variant(aircraft, section as usize)
            });
        let Some(variant) = reviewed.or(t.localized_damage.structural_variant) else {
            return;
        };
        if t.fragment_released || self.debris.len() >= crate::combat::debris::MAX_PIECES {
            return;
        }
        t.fragment_released = true;
        let offset = t.fragment_offsets[variant];
        let across = offset[0].hypot(offset[2]).max(1.);
        // Thrown up, and outward the way the piece sits from the centre.
        let local = [
            offset[0] / across * FRAGMENT_TOSS_OUT_FPS,
            FRAGMENT_TOSS_UP_FPS,
            offset[2] / across * FRAGMENT_TOSS_OUT_FPS,
        ];
        let velocity = std::array::from_fn(|i| {
            t.basis.right[i] * local[0] + t.basis.up[i] * local[1] + t.basis.forward[i] * local[2]
        });
        self.debris.push(crate::combat::debris::Piece::new(
            id, variant, t.position, velocity, t.basis, offset,
        ));
    }
}
