//! Parked aircraft (docs/spec/surface-defenses.md, "Parked aircraft"): the
//! aircraft a ground target template stands on an apron, in a shelter or on a
//! carrier deck become simulated aircraft that never take off (John,
//! 2026-10-10), not scenery.
//!
//! Resolution lists them ([`super::ParkedAircraft`], ids and owners in the
//! digest). This module decides which fleet aircraft stand on a carrier deck
//! ([`deck_spot`]) and, once the terrain is known, where each one stands and
//! what it is ([`ParkedPose`]): its record (`tore_formats::parked_aircraft`),
//! its gear-down shape, its aircraft-convention size, its pose on the ground
//! or deck, its fragment offsets. Combat registers the poses as targets
//! (`tore_sim::combat::live::State::add_parked_aircraft`) and the scenery
//! draws them.
//!
//! - **Size**: a parked aircraft is drawn and measured at the aircraft
//!   convention, one third of the shape's scenery scale (`2^(e-8) / 3` feet
//!   per unit), so it matches the same type in flight
//!   ([`aircraft_shape_scale`]).
//! - **Height**: it stands on its wheels: the lowest point of its gear-down
//!   shape meets the ground or the deck (fitted; the shape's recorded ground
//!   offset agrees within two units for every parked type).
//! - **Deck**: a fleet template's aircraft stands on a carrier when its spot,
//!   in the hull's own units, lies on the hull's flight deck
//!   (`tore_formats::carrier::flight_deck`); it keeps that spot in hull units,
//!   so deck and aircraft scale together through
//!   [`crate::terrain::placed_shape_scale`], and stands at the deck's height
//!   scaled the same way. An aircraft on no deck stays out (listed in the
//!   site's left-out list). Agent decision: the template positions were
//!   authored around the hull drawn at its scenery scale, where every
//!   `~QFFLT` aircraft lies on the Clemenceau's deck.
use super::{Surface, Unit, UnitId};
use crate::resources::ResourceSource;
use std::collections::BTreeMap;
use tore_formats::{
    aircraft::AircraftId,
    carrier::{Deck, flight_deck},
    parked_aircraft::{ParkedType, gear},
    shape::{Shape, object_scale},
};
use tore_sim::attitude::{Basis, Vector};
use tore_sim::combat::live::{self, Side};

/// Feet per shape unit of an aircraft drawn and measured at the aircraft
/// convention: one third of the scenery scale (`2^(e-8) / 3`), the scale the
/// game's aircraft renderer and fragments use
/// (`tore_sim::combat::debris::scale`).
pub fn aircraft_shape_scale(shape_bytes: &[u8]) -> tore_formats::Result<f64> {
    Ok(object_scale(shape_bytes)? / 3.)
}

/// Where a fleet aircraft stands on a carrier, in the hull's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeckSpot {
    /// Right and forward of the hull's origin, in hull shape units.
    pub units: [f32; 2],
    /// Right, up (the deck) and forward of the carrier's origin, in feet at
    /// the placed scale.
    pub feet: [f64; 3],
}

/// The spot of an aircraft at template position `aircraft` (x east, z north,
/// feet) on the deck of a carrier standing at `carrier` with heading
/// `heading_deg`, whose hull was authored at `authored_scale` feet per unit
/// (its scenery scale) and is placed at `placed_scale`. `None` off the deck.
pub fn deck_spot(
    deck: &Deck,
    authored_scale: f64,
    placed_scale: f64,
    carrier: [i32; 3],
    heading_deg: i32,
    aircraft: [i32; 3],
) -> Option<DeckSpot> {
    let basis = Basis::new(f64::from(heading_deg).to_radians(), 0., 0.);
    let offset = [
        f64::from(aircraft[0] - carrier[0]),
        0.,
        f64::from(aircraft[2] - carrier[2]),
    ];
    let right = tore_sim::attitude::dot(offset, basis.right) / authored_scale;
    let forward = tore_sim::attitude::dot(offset, basis.forward) / authored_scale;
    let units = [right as f32, forward as f32];
    deck.contains(units).then(|| DeckSpot {
        units,
        feet: [
            right * placed_scale,
            f64::from(deck.height) * placed_scale,
            forward * placed_scale,
        ],
    })
}

/// A carrier's hull as the deck rule needs it.
pub struct Hull {
    pub deck: Deck,
    /// Feet per unit at the scenery scale, where the templates were authored.
    pub authored_scale: f64,
    /// Feet per unit as placed ([`crate::terrain::placed_shape_scale`]).
    pub placed_scale: f64,
}

impl Hull {
    /// The hull of `unit`'s NT, `None` when it names no readable shape with a
    /// level deck.
    pub fn of(resources: &dyn ResourceSource, unit: &Unit, shape: &str) -> Option<Self> {
        let bytes = resources.get(shape)?;
        // A deck stands above the waterline (the hull's origin): the Kiev's
        // only level faces lie 22 units below it, and it has no deck the
        // rule can find (fitted).
        let deck = flight_deck(&Shape::scenery(bytes).ok()?).filter(|deck| deck.height > 0.)?;
        Some(Self {
            deck,
            authored_scale: object_scale(bytes).ok()?,
            placed_scale: crate::terrain::placed_shape_scale(&unit.resource, bytes).ok()?,
        })
    }
}

/// One parked aircraft placed in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct ParkedPose {
    pub id: UnitId,
    /// The PT, by exact name.
    pub resource: String,
    /// The record's short name, for the target window.
    pub name: String,
    /// The main shape and the state word that puts its gear down.
    pub shape: String,
    pub gear_word: Option<usize>,
    /// Feet per shape unit ([`aircraft_shape_scale`]).
    pub scale: f64,
    /// The shape's origin in the world, and its orientation.
    pub origin: Vector,
    pub heading: f64,
    pub basis: Basis,
    /// The ground or deck point under it, where its crash site lies.
    pub ground: Vector,
    /// It stands on a carrier deck: no crater when it burns.
    pub water: bool,
    /// The carrier it stands on.
    pub deck: Option<UnitId>,
    pub hit_points: i32,
    /// PT class word: 0x8000 fighter, 0x4000 bomber.
    pub class: u16,
    pub side: Side,
    pub target: bool,
    pub radar_signature: i32,
    pub infrared_signature: i32,
    /// Fragment offsets, right, up, forward feet, for pieces 0 and 1.
    pub fragment_offsets: [Vector; 2],
    /// The type, when the game flies it.
    pub aircraft: Option<AircraftId>,
}

impl ParkedPose {
    /// The target row combat registers.
    pub fn target(&self) -> live::ParkedAircraft {
        live::ParkedAircraft {
            id: self.id.0,
            position: self.origin,
            basis: self.basis,
            ground: self.ground,
            water: self.water,
            hit_points: self.hit_points,
            category: self.class,
            side: self.side,
            radar_signature: f64::from(self.radar_signature),
            infrared_signature: f64::from(self.infrared_signature),
            fragment_offsets: self.fragment_offsets,
            aircraft: self.aircraft,
        }
    }
}

/// The game's own type for a PT, by exact name (AGENTS.md: no aliasing).
pub fn flown(resource: &str) -> Option<AircraftId> {
    AircraftId::ALL
        .into_iter()
        .find(|id| id.pt().eq_ignore_ascii_case(resource))
}

/// Places every parked aircraft of `surface`. `height` is the terrain under a
/// point. A type whose record or shape does not read is left out of the
/// world and reported with the reason.
pub fn place(
    resources: &dyn ResourceSource,
    surface: &Surface,
    height: &dyn Fn(f64, f64) -> f64,
) -> (Vec<ParkedPose>, Vec<(String, String)>) {
    let mut poses = Vec::new();
    let mut unreadable = Vec::new();
    let mut hulls: BTreeMap<UnitId, Option<Hull>> = BTreeMap::new();
    for parked in &surface.parked {
        let placed = (|| -> Result<ParkedPose, String> {
            let get = |name: &str| {
                resources
                    .get(name)
                    .ok_or_else(|| format!("missing {name}; re-import media"))
            };
            let record = ParkedType::parse(get(&parked.resource)?).map_err(|e| e.to_string())?;
            let shape_bytes = get(&record.shape)?;
            let gear = gear(shape_bytes).map_err(|e| format!("{}: {e}", record.shape))?;
            let scale = aircraft_shape_scale(shape_bytes).map_err(|e| e.to_string())?;
            let low = gear
                .down
                .faces
                .iter()
                .flat_map(|face| face.positions.iter().map(|p| f64::from(p[2])))
                .fold(f64::INFINITY, f64::min);
            if !low.is_finite() {
                return Err(format!("{}: no geometry", record.shape));
            }
            let x = f64::from(parked.position[0]);
            let z = f64::from(parked.position[2]);
            let (ground, water) = match parked.deck {
                Some(carrier_id) => {
                    let carrier = surface
                        .unit(carrier_id)
                        .ok_or("the carrier is not a unit")?;
                    let hull = hulls.entry(carrier_id).or_insert_with(|| {
                        let shape = super::catalog::unit_shape(resources, &carrier.resource)?;
                        Hull::of(resources, carrier, &shape)
                    });
                    let hull = hull.as_ref().ok_or("the carrier's hull has no deck")?;
                    let spot = deck_spot(
                        &hull.deck,
                        hull.authored_scale,
                        hull.placed_scale,
                        carrier.position,
                        carrier.angles[0],
                        parked.position,
                    )
                    .ok_or("off the carrier's deck")?;
                    let [cx, cy, cz] = carrier.position.map(f64::from);
                    let base = height(cx, cz) + cy;
                    let turn = Basis::new(f64::from(carrier.angles[0]).to_radians(), 0., 0.);
                    let at = std::array::from_fn(|i| {
                        [cx, base, cz][i]
                            + turn.right[i] * spot.feet[0]
                            + turn.up[i] * spot.feet[1]
                            + turn.forward[i] * spot.feet[2]
                    });
                    (at, true)
                }
                None => ([x, height(x, z) + f64::from(parked.position[1]), z], false),
            };
            let heading = f64::from(parked.angles[0]).to_radians();
            let basis = Basis::new(heading, 0., 0.);
            let origin = std::array::from_fn(|i| ground[i] - basis.up[i] * low * scale);
            let aircraft = flown(&record.resource);
            let fragment_offsets = match aircraft {
                Some(id) => {
                    let read = |name: &str| {
                        resources.get(name).cloned().ok_or_else(|| {
                            std::io::Error::other(format!("missing {name}; re-import media"))
                        })
                    };
                    [0, 1].map(|variant| {
                        tore_sim::combat::debris::attachment(id, variant, read).unwrap_or([0.; 3])
                    })
                }
                // The OBJECT block's debris positions, shape units as the
                // record writes them (x right, y up, z forward): the
                // destroyed position for piece 0, the damaged one for piece
                // 1 (fitted).
                None => [record.debris_destroyed, record.debris_damaged]
                    .map(|p| p.map(|v| f64::from(v) * scale)),
            };
            Ok(ParkedPose {
                id: parked.id,
                resource: record.resource.clone(),
                name: record.short_name.clone(),
                shape: record.shape.clone(),
                gear_word: gear.word,
                scale,
                origin,
                heading,
                basis,
                ground,
                water,
                deck: parked.deck,
                hit_points: record.hit_points,
                class: record.class,
                side: parked.side,
                target: parked.target,
                radar_signature: record.radar_signature(),
                infrared_signature: record.infrared_signature(),
                fragment_offsets,
                aircraft,
            })
        })();
        match placed {
            Ok(pose) => poses.push(pose),
            Err(why) => unreadable.push((parked.resource.clone(), why)),
        }
    }
    (poses, unreadable)
}
