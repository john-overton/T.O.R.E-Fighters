//! Transactional preflight loadout. Compatibility is translated; selectable live
//! weapons remain explicitly limited to the reviewed manual-weapons service.
use super::{live, loading};
use tore_formats::{
    Result,
    aircraft::{Aircraft, AircraftId, Hardpoint},
    weapons::Weapon,
};
/// One editable aggregate source row, independent of catalog category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditableStation {
    pub hardpoint: usize,
    pub location: u8,
    pub weapon: Option<usize>,
    pub tank: Option<usize>,
}
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
    /// Original source hardpoints corresponding to the explicit tank station rows.
    pub tank_hardpoints: Vec<Hardpoint>,
    /// Loadout screen Cheat: any store on any station, up to its capacity.
    pub cheat: bool,
}
impl Loadout {
    pub fn editable_stations(&self) -> Vec<EditableStation> {
        let mut rows = std::collections::BTreeMap::new();
        for (hardpoint, slot) in self.configuration.hardpoint_slots.iter().enumerate() {
            if let Some(slot) = slot {
                rows.insert(
                    hardpoint,
                    EditableStation {
                        hardpoint,
                        location: self.hardpoints[*slot].location,
                        weapon: Some(*slot),
                        tank: None,
                    },
                );
            }
        }
        for (slot, tank) in self.configuration.tanks.iter().enumerate() {
            rows.entry(tank.hardpoint)
                .or_insert(EditableStation {
                    hardpoint: tank.hardpoint,
                    location: self.tank_hardpoints[slot].location,
                    weapon: None,
                    tank: None,
                })
                .tank = Some(slot);
        }
        rows.into_values().collect()
    }
    pub fn unload_tank(&mut self, slot: usize) -> Result<()> {
        let mut draft = self.clone();
        let tank = draft
            .configuration
            .tanks
            .get_mut(slot)
            .ok_or_else(|| super::invalid("Invalid tank station."))?;
        tank.quantity = 0;
        draft.refresh_equipment()?;
        *self = draft;
        Ok(())
    }
    /// Move one installed tank, leaving both loads unchanged if rejected.
    pub fn transfer_tank(&mut self, source: usize, target: usize) -> Result<()> {
        if source == target {
            return Ok(());
        }
        let from = self
            .configuration
            .tanks
            .get(source)
            .ok_or_else(|| super::invalid("Invalid source tank station."))?;
        if from.quantity == 0 {
            return Ok(());
        }
        let store = from
            .store
            .clone()
            .ok_or_else(|| super::invalid("Installed tank has no source type."))?;
        let to = self
            .configuration
            .tanks
            .get(target)
            .ok_or_else(|| super::invalid("Invalid destination tank station."))?;
        let capacity = self.tank_capacity(target, &store);
        if capacity <= 0 {
            return Err(super::invalid(
                "This tank cannot be loaded at this station.",
            ));
        }
        let existing = if to
            .store
            .as_ref()
            .is_some_and(|tank| tank.source == store.source)
        {
            to.quantity
        } else {
            0
        };
        if i32::from(existing) >= capacity {
            return Ok(());
        }
        let mut draft = self.clone();
        draft.configuration.tanks[target].store = Some(store);
        draft.configuration.tanks[target].quantity = existing + 1;
        draft.configuration.tanks[source].quantity -= 1;
        let hardpoint = draft.configuration.tanks[target].hardpoint;
        if let Some(Some(slot)) = draft.configuration.hardpoint_slots.get(hardpoint) {
            draft.quantities[*slot] = 0;
        }
        draft.refresh_equipment()?;
        *self = draft;
        Ok(())
    }
    pub fn ammunition(&self) -> Result<Vec<u16>> {
        self.configuration.ammunition(&self.quantities)
    }
    pub fn refresh_equipment(&mut self) -> Result<()> {
        for pod in &mut self.configuration.gun_pods {
            pod.quantity = if self.configuration.stations[pod.station].weapon.source == "SUU16.JT" {
                self.quantities[pod.station]
            } else {
                0
            };
        }
        self.configuration.refresh_tanks()
    }
    pub fn new(a: &Aircraft, read: impl FnMut(&str) -> Result<Vec<u8>>) -> Result<Self> {
        let configuration = live::Configuration::from_source(a, read)?;
        let hardpoints = a
            .hardpoints
            .iter()
            .enumerate()
            .filter_map(|(index, h)| {
                configuration.hardpoint_slots[index].map(|slot| (slot, h.clone()))
            })
            .collect::<std::collections::BTreeMap<_, _>>()
            .into_values()
            .collect();
        Ok(Self {
            aircraft: a.id,
            quantities: configuration.stations.iter().map(|s| s.count).collect(),
            tank_hardpoints: configuration
                .tanks
                .iter()
                .map(|s| a.hardpoints[s.hardpoint].clone())
                .collect(),
            configuration,
            fuel_lbs: a.fields["internalFuel"].number()? as f64,
            internal_capacity_lbs: a.fields["internalFuel"].number()? as f64,
            empty_lbs: a.object["weight"].number()? as f64,
            maximum_lbs: a.fields["maxTakeoffWeight"].number()? as f64,
            hardpoints,
            cheat: false,
        })
    }
    pub fn tank_capacity(&self, slot: usize, tank: &live::TankStore) -> i32 {
        self.tank_hardpoints
            .get(slot)
            .and_then(|h| {
                loading::Station::from_source(h).ok().map(|s| {
                    let matches_default = h.store.as_deref() == Some(tank.source.as_str());
                    // Preserve reviewed source-default quantities. The generic
                    // full-tank weight cap disagrees with A4E/F104 defaults;
                    // paired/weight consumer semantics remain unresolved.
                    if matches_default {
                        return h.count;
                    }
                    let store = loading::Store::tank(tank.tank);
                    if self.cheat {
                        s.cheat_count(store, matches_default, h.store.is_some())
                    } else {
                        s.allowed_count(store, matches_default)
                    }
                })
            })
            .unwrap_or(0)
            .clamp(0, 32766)
    }
    /// Tanks start full. Filling an installed empty tank is a later UI feature.
    pub fn select_tank(&mut self, slot: usize, tank: live::TankStore) -> Result<()> {
        let capacity = self.tank_capacity(slot, &tank);
        if capacity <= 0 {
            return Err(super::invalid(
                "This tank cannot be loaded at this station.",
            ));
        }
        let mut configuration = self.configuration.clone();
        let station = configuration
            .tanks
            .get_mut(slot)
            .ok_or_else(|| super::invalid("Invalid tank station."))?;
        let hardpoint = station.hardpoint;
        station.store = Some(tank);
        station.quantity = capacity as u16;
        configuration.refresh_tanks()?;
        self.configuration = configuration;
        if let Some(Some(weapon)) = self.configuration.hardpoint_slots.get(hardpoint) {
            self.quantities[*weapon] = 0;
        }
        self.refresh_equipment()
    }
    pub fn change_tank(&mut self, slot: usize, direction: i32) -> Result<()> {
        let Some(station) = self.configuration.tanks.get(slot) else {
            return Ok(());
        };
        let Some(store) = &station.store else {
            return Ok(());
        };
        let capacity = self.tank_capacity(slot, store);
        let quantity = (i32::from(station.quantity) + direction.signum()).clamp(0, capacity) as u16;
        let hardpoint = station.hardpoint;
        self.configuration.tanks[slot].quantity = quantity;
        if quantity > 0
            && let Some(Some(weapon)) = self.configuration.hardpoint_slots.get(hardpoint)
        {
            self.quantities[*weapon] = 0;
        }
        self.refresh_equipment()
    }
    pub fn clear_tanks(&mut self) -> Result<()> {
        for station in &mut self.configuration.tanks {
            station.quantity = 0;
        }
        self.refresh_equipment()
    }
    pub fn external_fuel_lbs(&self) -> f64 {
        self.configuration.external_fuel_lbs.iter().sum()
    }
    pub fn tank_shell_lbs(&self) -> f64 {
        self.configuration
            .tanks
            .iter()
            .filter_map(|s| {
                s.store
                    .as_ref()
                    .map(|store| f64::from(s.quantity) * f64::from(store.tank.empty_weight))
            })
            .sum()
    }
    fn clear_tank_on_weapon_slot(&mut self, slot: usize) -> Result<()> {
        for tank in &mut self.configuration.tanks {
            if self.configuration.hardpoint_slots.get(tank.hardpoint) == Some(&Some(slot)) {
                tank.quantity = 0;
            }
        }
        self.refresh_equipment()
    }
    pub fn capacity(&self, slot: usize, w: &Weapon) -> i32 {
        // The reviewed pod contract is limited to source-installed gun-pod
        // stations. Unrelated rocket launchers keep their existing semantics.
        if w.source == "SUU16.JT"
            && !self
                .configuration
                .gun_pods
                .iter()
                .any(|pod| pod.station == slot)
        {
            return 0;
        }
        self.hardpoints
            .get(slot)
            .and_then(|h| {
                loading::Station::from_source(h).ok().map(|s| {
                    let store = loading::Store::weapon(w);
                    let matches_default = h.store.as_deref() == Some(&w.source);
                    if self.cheat {
                        s.cheat_count(store, matches_default, h.store.is_some())
                    } else {
                        s.allowed_count(store, matches_default)
                    }
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
        self.clear_tank_on_weapon_slot(slot)
    }
    pub fn change(&mut self, slot: usize, direction: i32) {
        if let Some(s) = self.configuration.stations.get(slot) {
            let cap = self.capacity(slot, &s.weapon);
            let step = quantity_step(cap);
            self.quantities[slot] =
                (i32::from(self.quantities[slot]) + direction.signum() * step).clamp(0, cap) as u16;
            if self.quantities[slot] > 0 && self.configuration.stations[slot].count == 0 {
                self.configuration.stations[slot].count = cap as u16;
            }
            if self.quantities[slot] > 0 {
                let _ = self.clear_tank_on_weapon_slot(slot);
            } else {
                let _ = self.refresh_equipment();
            }
        }
    }
    /// Move one quantity step, preserving both loads if the destination rejects it.
    pub fn transfer(&mut self, source: usize, target: usize) -> Result<()> {
        if source == target || self.quantities.get(source).copied().unwrap_or(0) == 0 {
            return Ok(());
        }
        let weapon = &self.configuration.stations[source].weapon;
        let capacity = self.capacity(target, weapon);
        if capacity <= 0 || capacity >= 32767 {
            return Err(super::invalid(
                "This store cannot be loaded at this station.",
            ));
        }
        let existing = if self.configuration.stations[target].weapon.source == weapon.source {
            i32::from(self.quantities[target])
        } else {
            0
        };
        let moved = quantity_step(capacity)
            .min(i32::from(self.quantities[source]))
            .min((capacity - existing).max(0)) as u16;
        if moved > 0 {
            self.configuration.stations[target].weapon = weapon.clone();
            self.configuration.stations[target].count = capacity as u16;
            self.quantities[target] = existing as u16 + moved;
            self.quantities[source] -= moved;
            self.clear_tank_on_weapon_slot(target)?;
        }
        Ok(())
    }
    pub fn restrict_to_guns(&mut self) {
        for (station, count) in self.configuration.stations.iter().zip(&mut self.quantities) {
            if !self
                .aircraft
                .guns()
                .contains(&station.weapon.source.as_str())
            {
                *count = 0;
            }
        }
        let _ = self.refresh_equipment();
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
                .filter(|(s, _)| !s.internal && s.weapon.source != "SUU16.JT")
                .map(|(s, n)| f64::from(s.weapon.weight) * f64::from(*n))
                .sum::<f64>()
    }
    pub fn validate(&self) -> Result<()> {
        self.ammunition()?;
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
        if self.configuration.tanks.len() != self.tank_hardpoints.len() {
            return Err(super::invalid("Invalid tank station state."));
        }
        for (index, tank) in self.configuration.tanks.iter().enumerate() {
            if tank.quantity > 0 {
                let store = tank
                    .store
                    .as_ref()
                    .ok_or_else(|| super::invalid("Tank quantity has no selected type."))?;
                if i32::from(tank.quantity) > self.tank_capacity(index, store) {
                    return Err(super::invalid("Tank quantity exceeds station capacity."));
                }
                if let Some(Some(slot)) = self.configuration.hardpoint_slots.get(tank.hardpoint)
                    && self.quantities[*slot] > 0
                {
                    return Err(super::invalid(
                        "A weapon and a tank cannot share the same station.",
                    ));
                }
            }
        }
        let mut derived = self.configuration.clone();
        for pod in &mut derived.gun_pods {
            pod.quantity = if derived.stations[pod.station].weapon.source == "SUU16.JT" {
                self.quantities[pod.station]
            } else {
                0
            };
        }
        derived.refresh_tanks()?;
        if derived.external_equipment_lbs != self.configuration.external_equipment_lbs
            || derived.external_fuel_lbs != self.configuration.external_fuel_lbs
        {
            return Err(super::invalid(
                "Tank fuel and mass disagree with installed quantities.",
            ));
        }
        if self.total_lbs() > self.maximum_lbs {
            return Err(super::invalid(
                "This plane is too heavy to take off. Remove stores or fuel.",
            ));
        }
        Ok(())
    }
}
fn quantity_step(capacity: i32) -> i32 {
    if capacity > 300 {
        100
    } else if capacity > 100 {
        10
    } else {
        1
    }
}
/// Weapons connected to flight. Shared by launch validation and the ordnance
/// catalog, including Cheat loading; importing a definition does not enable it.
pub fn supported(name: &str) -> bool {
    [
        "AA11.JT",
        "AA11B.JT",
        "AA12.JT",
        "AA2.JT",
        "AA8.JT",
        "AAML.JT",
        "AS7.JT",
        "B13.JT",
        "B8.JT",
        "GSH23.JT",
        "GSH301.JT",
        "GSH6_30.JT",
        "M61.JT",
        "DEFA.JT",
        "MK12.JT",
        "MK82.JT",
        "LAU61.JT",
        "AIM54C.JT",
        "AIM9X.JT",
        "AIM120.JT",
        "AIM9M.JT",
        "AGM65G.JT",
        "MICA.JT",
        "R530.JT",
        "R550.JT",
        "AA10.JT",
        "AGM88.JT",
        "AIM7.JT",
        "AIM7E.JT",
        "AIM9B.JT",
        "AT2.JT",
        "C_105.JT",
        "C_25.JT",
        "C_40.JT",
        "GAU12.JT",
        "GAU8.JT",
        "GSH30.JT",
        "SUU16.JT",
        "T12_4.JT",
        "T30_1.JT",
    ]
    .contains(&name)
}
