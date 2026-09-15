//! Transactional preflight loadout. Compatibility is translated; selectable live
//! weapons remain explicitly limited to the reviewed manual-weapons service.
use super::{live, loading};
use tore_formats::{
    Result,
    aircraft::{Aircraft, AircraftId, Hardpoint},
    weapons::Weapon,
};
#[derive(Clone, Debug)]
pub struct Loadout {
    pub aircraft: AircraftId,
    pub configuration: live::Configuration,
    pub quantities: Vec<u16>,
    pub fuel_lbs: f64,
    pub internal_capacity_lbs: f64,
    pub empty_lbs: f64,
    pub maximum_lbs: f64,
    pub hardpoints: Vec<Hardpoint>,
}
impl Loadout {
    pub fn new(a: &Aircraft, read: impl FnMut(&str) -> Result<Vec<u8>>) -> Result<Self> {
        let configuration = live::Configuration::from_source(a, read)?;
        Ok(Self {
            aircraft: a.id,
            quantities: configuration.stations.iter().map(|s| s.count).collect(),
            configuration,
            fuel_lbs: a.fields["internalFuel"].number()? as f64,
            internal_capacity_lbs: a.fields["internalFuel"].number()? as f64,
            empty_lbs: a.object["weight"].number()? as f64,
            maximum_lbs: a.fields["maxTakeoffWeight"].number()? as f64,
            hardpoints: a
                .hardpoints
                .iter()
                .filter(|h| h.store.as_deref().is_some_and(|n| n.ends_with(".JT")))
                .cloned()
                .collect(),
        })
    }
    pub fn capacity(&self, slot: usize, w: &Weapon) -> i32 {
        self.hardpoints
            .get(slot)
            .and_then(|h| {
                loading::Station::from_source(h).ok().map(|s| {
                    s.allowed_count(
                        loading::Store::weapon(w),
                        h.store.as_deref() == Some(&w.source),
                    )
                })
            })
            .unwrap_or(0)
            .max(0)
    }
    pub fn select(&mut self, slot: usize, w: Weapon) -> Result<()> {
        let capacity = self.capacity(slot, &w);
        if capacity <= 0 || capacity >= 32767 {
            return Err(super::invalid(
                "This store cannot be loaded at this station.",
            ));
        }
        self.configuration.stations[slot].weapon = w;
        self.configuration.stations[slot].count = capacity as u16;
        self.quantities[slot] = capacity as u16;
        Ok(())
    }
    pub fn change(&mut self, slot: usize, direction: i32) {
        if let Some(s) = self.configuration.stations.get(slot) {
            let cap = self.capacity(slot, &s.weapon);
            let step = if cap > 300 {
                100
            } else if cap > 100 {
                10
            } else {
                1
            };
            self.quantities[slot] =
                (i32::from(self.quantities[slot]) + direction.signum() * step).clamp(0, cap) as u16;
        }
    }
    pub fn fuel(&mut self, up: bool) {
        self.fuel_lbs =
            (self.fuel_lbs + if up { 500. } else { -500. }).clamp(0., self.internal_capacity_lbs);
    }
    pub fn total_lbs(&self) -> f64 {
        self.empty_lbs
            + self.fuel_lbs
            + f64::from(self.configuration.external_equipment_lbs)
            + self
                .configuration
                .stations
                .iter()
                .zip(&self.quantities)
                .filter(|(s, _)| !s.internal)
                .map(|(s, n)| f64::from(s.weapon.weight) * f64::from(*n))
                .sum::<f64>()
    }
    pub fn validate(&self) -> Result<()> {
        if self.quantities.len() != self.configuration.stations.len()
            || !self.fuel_lbs.is_finite()
            || !(0. ..=self.internal_capacity_lbs).contains(&self.fuel_lbs)
        {
            return Err(super::invalid("Invalid loadout fuel or station state."));
        }
        for (i, (s, n)) in self
            .configuration
            .stations
            .iter()
            .zip(&self.quantities)
            .enumerate()
        {
            if i32::from(*n) > self.capacity(i, &s.weapon) {
                return Err(super::invalid("Station quantity exceeds capacity."));
            }
            if *n > 0 && !supported(&s.weapon.source) {
                return Err(super::invalid(
                    "This store is available for setup only; its flight behavior is not implemented yet.",
                ));
            }
        }
        if self.total_lbs() > self.maximum_lbs {
            return Err(super::invalid(
                "This plane is too heavy to take off. Remove stores or fuel.",
            ));
        }
        Ok(())
    }
}
pub fn supported(name: &str) -> bool {
    [
        "M61.JT",
        "DEFA.JT",
        "AIM120.JT",
        "AIM9M.JT",
        "AGM65G.JT",
        "MICA.JT",
        "R530.JT",
        "R550.JT",
    ]
    .contains(&name)
}
