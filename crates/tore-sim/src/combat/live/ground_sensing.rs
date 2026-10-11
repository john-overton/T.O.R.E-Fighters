//! What the player's ground pages read of the targets (docs/spec/air-to-ground.md,
//! "Ground contacts and channels"): each row's class word, its type's listing
//! flag and visibility distance, its signatures, whether it emits, whether an
//! aircraft is in an airfield sequence, and the weather's sensor effects. The
//! host registers the type facts and the weather; the sensors decide what a
//! page lists ([`sensors::Sensors::step_ground`]).
use super::{Ownship, ParkedSite, State, Target, TargetRole};
use crate::sensors::{self, GroundObject, Volume, track::LISTED_FLAG, track::class};
use std::collections::{BTreeMap, BTreeSet};

/// The type facts a ground page reads of one ground object: its OBJECT
/// `flags` and `maxVisDist` (docs/formats/radar.md, "A-G page").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroundListing {
    pub flags: u32,
    /// Raw, in units of 256 ft.
    pub max_visible: u16,
}

impl State {
    /// Registers the type facts of ground object `id`. Fails for an id that is
    /// not a ground object.
    pub fn set_ground_listing(&mut self, id: u32, listing: GroundListing) -> bool {
        if !self.ground_bounds.contains_key(&id) {
            return false;
        }
        self.ground_listing.insert(id, listing);
        true
    }
    /// The type facts of ground object `id`, if registered.
    pub fn ground_listing(&self, id: u32) -> Option<GroundListing> {
        self.ground_listing.get(&id).copied()
    }
    /// Whether aircraft `id` is in an airfield's takeoff or landing sequence:
    /// the A-G page lists aircraft only then. The host says so each tick.
    pub fn set_airfield_attached(&mut self, id: u32, attached: bool) {
        if attached {
            self.airfield.insert(id);
        } else {
            self.airfield.remove(&id);
        }
    }
    pub fn airfield_attached(&self, id: u32) -> bool {
        self.airfield.contains(&id)
    }
    /// The active weather layers' sensor effects, from the host's weather.
    pub fn set_sensor_weather(&mut self, weather: sensors::Weather) {
        if self.sensor_weather != weather {
            self.sensor_weather = weather;
        }
    }
    pub fn sensor_weather(&self) -> &sensors::Weather {
        &self.sensor_weather
    }
    /// Ctrl+X: the nearest standing ground object (a surface row: a unit, a
    /// structure, a ship or a parked aircraft) of another side than `own`'s,
    /// neutral scenery excluded.
    pub(super) fn nearest_ground(
        &self,
        own: &Ownship,
        from: crate::attitude::Vector,
    ) -> Option<u32> {
        self.targets
            .iter()
            .filter(|t| {
                t.role == TargetRole::Surface
                    && t.hp > 0
                    && t.side != super::NO_SIDE
                    && t.side != own.side
                    && !own.friendlies.contains(&t.id)
            })
            .map(|t| {
                let d: f64 = (0..3).map(|i| (t.position[i] - from[i]).powi(2)).sum();
                (d, t.id)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, id)| id)
    }
    /// What a ground page reads of target row `t`, for a reader outside the
    /// player's pages (the AI's ground picture, docs/spec/ai-strike.md).
    pub fn ground_object(&self, t: &Target) -> GroundObject {
        self.ground_facts().ground_object(t)
    }
    /// The facts the ground pages read, borrowed for one observation pass.
    pub(super) fn ground_facts(&self) -> GroundFacts<'_> {
        GroundFacts {
            listing: &self.ground_listing,
            parked: &self.parked,
            airfield: &self.airfield,
            weather: &self.sensor_weather,
        }
    }
}

/// The state's read-only facts for the ground pages, without its
/// projectiles, ownships or random stream, so observations on workers can
/// read them.
pub(super) struct GroundFacts<'a> {
    listing: &'a BTreeMap<u32, GroundListing>,
    parked: &'a BTreeMap<u32, ParkedSite>,
    airfield: &'a BTreeSet<u32>,
    pub weather: &'a sensors::Weather,
}
impl GroundFacts<'_> {
    /// What a ground page reads of target row `t`.
    pub(super) fn ground_object(&self, t: &Target) -> GroundObject {
        let listing = self.listing.get(&t.id);
        let parked = self.parked.contains_key(&t.id);
        GroundObject {
            id: t.id,
            position: t.position,
            velocity: t.velocity,
            class: t.category,
            // A parked aircraft has no OT record: listed (fitted, retail
            // aircraft types carry the flag).
            listed: parked || listing.is_some_and(|l| l.flags & LISTED_FLAG != 0),
            max_visible: listing.map_or(0, |l| l.max_visible),
            radar: t.signature.radar,
            infrared: t.signature.infrared,
            emitting: t.radar_emitting || t.jammer_active,
            aircraft: t.role == TargetRole::Aircraft
                || t.aircraft.is_some()
                || t.category & class::AIRCRAFT != 0,
            airborne: t.airborne,
            airfield: self.airfield.contains(&t.id),
            destroyed: t.hp <= 0,
        }
    }
}

/// The seeker the HARM page uses: of the anti-radiation (signature 4)
/// weapons aboard with rounds left on a working station, the one with the
/// shortest search range (retail `HARDBestSeekers`: with an AGM-45 and an
/// AGM-88 aboard, the AGM-45's).
pub(super) fn harm_seeker(own: &Ownship) -> Option<Volume> {
    own.config
        .stations
        .iter()
        .enumerate()
        .filter(|(index, station)| {
            station.weapon.seeker.signature == 4
                && own.rounds(*index) > 0
                && own.ammo.get(*index).is_some_and(|ammo| ammo & 0x8000 == 0)
        })
        .filter_map(|(_, station)| Volume::from_zone(&station.weapon.seeker.zones[0]).ok())
        .min_by(|a, b| a.maximum_ft.total_cmp(&b.maximum_ft))
}
