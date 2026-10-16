//! Running-flight respawn menus, using the flight Escape menu widget.
use crate::{
    AppResult,
    pause_menu::{Event, Look, PauseMenu},
};
use std::collections::BTreeMap;
use tore_formats::{aircraft::AircraftId, font::Font, ui::MenuNode, weapons::Weapon};
use tore_sim::combat::{live::TankStore, loadout::Loadout, sight::DesignatorStore};
use tore_world::{
    aircraft_type::AircraftType,
    mission::{LoadoutSpec, MissionSpec},
    world::revive::RevivalWeapons,
};

const LOOK: Look = Look {
    title: "RESPAWN LOADOUT",
    buttons: &["Done", "Cancel"],
    help: |_| vec![],
};
const PAGE: usize = 12;
fn row(label: String) -> MenuNode {
    MenuNode {
        label,
        shortcut: String::new(),
        children: vec![],
    }
}

pub struct Menu {
    widget: PauseMenu,
    tree: Vec<MenuNode>,
    pub load: Loadout,
    aircraft: Vec<AircraftId>,
    weapons: Vec<Weapon>,
    tanks: Vec<TankStore>,
    pods: Vec<DesignatorStore>,
    page: usize,
    cycles: BTreeMap<usize, usize>,
    rule: RevivalWeapons,
    pub message: Option<String>,
}
pub enum Outcome {
    None,
    Click,
    Cancel,
    Done(AircraftId, LoadoutSpec),
}
impl Menu {
    pub fn new(
        aircraft: AircraftId,
        allowed: Vec<AircraftId>,
        data: &BTreeMap<String, Vec<u8>>,
        spec: &MissionSpec,
        rule: RevivalWeapons,
        gap: impl Fn(&str) -> bool,
    ) -> AppResult<Self> {
        let load = Self::standard(aircraft, data, spec)?;
        let weapons = data
            .iter()
            .filter(|(name, _)| tore_sim::combat::loadout::supported(name) && !gap(name))
            .filter_map(|(name, bytes)| Weapon::parse(name, bytes).ok())
            .collect();
        let tanks = data
            .iter()
            .filter(|(name, _)| name.ends_with(".GAS") && !gap(name))
            .filter_map(|(name, bytes)| TankStore::parse(name, bytes).ok())
            .collect();
        let pods = tore_sim::combat::sight::DESIGNATOR_PODS
            .iter()
            .filter(|name| !gap(name))
            .filter_map(|name| DesignatorStore::parse(name, data.get(*name)?).ok())
            .collect();
        let mut menu = Self {
            widget: PauseMenu::default(),
            tree: vec![],
            load,
            aircraft: allowed,
            weapons,
            tanks,
            pods,
            page: 0,
            cycles: BTreeMap::new(),
            rule,
            message: None,
        };
        menu.refresh();
        Ok(menu)
    }
    fn standard(
        aircraft: AircraftId,
        data: &BTreeMap<String, Vec<u8>>,
        spec: &MissionSpec,
    ) -> AppResult<Loadout> {
        let kind = AircraftType::load(data, aircraft)?;
        let mut load = Loadout::new(&kind.profile, |name| {
            data.get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
        })?;
        load.cheat = spec.cheat_loadouts;
        if spec.guns_only {
            load.restrict_to_guns();
        }
        Ok(load)
    }
    fn restore_selection(
        &mut self,
        selected: &LoadoutSpec,
        data: &BTreeMap<String, Vec<u8>>,
        spec: &MissionSpec,
    ) {
        let restored = AircraftType::load(data, self.load.aircraft)
            .and_then(|kind| selected.check_in(&kind.profile, data, spec));
        match restored {
            Ok(mut load) => {
                load.cheat = spec.cheat_loadouts;
                self.load = load;
            }
            Err(error) => self.message = Some(format!("Using standard load: {error}")),
        }
        self.refresh();
    }

    fn refresh(&mut self) {
        let rows = self.load.editable_stations();
        let mut loads: Vec<MenuNode> = self
            .load
            .station_display_order()
            .into_iter()
            .map(|index| {
                let station = &rows[index];
                let heading = [
                    "Centerline",
                    "Fuselage",
                    "Internal Gun",
                    "Internal Bay",
                    "Wing",
                    "Wingtip",
                ]
                .get(usize::from(station.location))
                .copied()
                .unwrap_or("Station");
                let tank = station
                    .tank
                    .and_then(|slot| self.load.configuration.tanks.get(slot))
                    .filter(|tank| tank.quantity > 0);
                let pod = station.weapon.and_then(|slot| self.load.designator(slot));
                let store = if let Some(tank) = tank {
                    format!(
                        "{} x{}",
                        tank.store.as_ref().map_or("Empty", |s| s.name.as_str()),
                        tank.quantity
                    )
                } else if let Some(pod) = pod {
                    format!("{} x1", pod.name)
                } else if let Some(slot) = station.weapon {
                    let current = &self.load.configuration.stations[slot];
                    format!(
                        "{} x{}",
                        current.weapon.hud_name,
                        self.rule.cut(&current.weapon, self.load.quantities[slot])
                    )
                } else if let Some(slot) = station.tank {
                    let tank = &self.load.configuration.tanks[slot];
                    format!(
                        "{} x{}",
                        tank.store.as_ref().map_or("Empty", |s| s.name.as_str()),
                        tank.quantity
                    )
                } else {
                    "Empty".into()
                };
                row(format!("{heading}: {store}"))
            })
            .collect();
        loads.push(row(format!(
            "Internal fuel: {:.0}%",
            100. * self.load.fuel_lbs / self.load.internal_capacity_lbs.max(1.)
        )));
        loads.push(row("Standard load".into()));
        loads.push(row("Done".into()));
        self.tree = vec![MenuNode {
            label: "Loadout".into(),
            shortcut: String::new(),
            children: loads,
        }];
        if !self.aircraft.is_empty() {
            let start = self.page * PAGE;
            let mut rows = self
                .aircraft
                .iter()
                .skip(start)
                .take(PAGE)
                .map(|id| {
                    row(format!(
                        "{}{}",
                        if *id == self.load.aircraft { "* " } else { "" },
                        id.label()
                    ))
                })
                .collect::<Vec<_>>();
            if self.aircraft.len() > PAGE {
                rows.push(row("Next aircraft page".into()));
            }
            self.tree.push(MenuNode {
                label: "Aircraft".into(),
                shortcut: String::new(),
                children: rows,
            });
        }
    }
    fn check(
        &self,
        candidate: &Loadout,
        data: &BTreeMap<String, Vec<u8>>,
        spec: &MissionSpec,
    ) -> AppResult<()> {
        let kind = AircraftType::load(data, candidate.aircraft)?;
        LoadoutSpec::of(candidate).check_in(&kind.profile, data, spec)?;
        Ok(())
    }
    fn select(
        &mut self,
        index: usize,
        data: &BTreeMap<String, Vec<u8>>,
        spec: &MissionSpec,
    ) -> Outcome {
        self.message = None;
        if self.widget.root == 1 {
            self.cycles.clear();
            if let Some(id) = self
                .aircraft
                .get(self.page * PAGE + index)
                .copied()
                .filter(|_| index < PAGE)
            {
                match Self::standard(id, data, spec) {
                    Ok(load) => self.load = load,
                    Err(e) => self.message = Some(e.to_string()),
                }
                self.widget.show_tab(0);
            } else {
                self.page = (self.page + 1) % self.aircraft.len().div_ceil(PAGE);
                self.widget.show_tab(1);
            }
        } else {
            let order = self.load.station_display_order();
            if index == order.len() + 2 {
                return self.done(data, spec);
            }
            let mut candidate = self.load.clone();
            if index >= order.len() {
                self.cycles.clear();
            }
            if index == order.len() + 1 {
                match Self::standard(self.load.aircraft, data, spec) {
                    Ok(load) => candidate = load,
                    Err(e) => self.message = Some(e.to_string()),
                }
            } else if index == order.len() {
                candidate.fuel_lbs = if candidate.fuel_lbs >= candidate.internal_capacity_lbs {
                    candidate.internal_capacity_lbs * 0.1
                } else {
                    (candidate.fuel_lbs + candidate.internal_capacity_lbs * 0.1)
                        .min(candidate.internal_capacity_lbs)
                };
            } else if let Some(&station) = order.get(index) {
                let editable = self.load.editable_stations()[station];
                let mut candidates = Vec::new();
                if let Some(slot) = editable.weapon {
                    let mut empty = candidate.clone();
                    empty.quantities[slot] = 0;
                    let _ = empty.unload_designator(slot);
                    candidates.push(empty);
                    for weapon in &self.weapons {
                        let mut next = candidate.clone();
                        if next.select(slot, weapon.clone()).is_ok()
                            && self.rule.cut(weapon, next.quantities[slot]) > 0
                        {
                            let maximum = next.quantities[slot];
                            let quantities: Vec<u16> = if maximum <= 20 {
                                (1..=maximum).collect()
                            } else {
                                (1..=10)
                                    .map(|step| (u32::from(maximum) * step / 10) as u16)
                                    .collect()
                            };
                            for quantity in quantities {
                                let mut amount = next.clone();
                                amount.quantities[slot] = quantity;
                                candidates.push(amount);
                            }
                        }
                    }
                    for pod in &self.pods {
                        let mut next = candidate.clone();
                        if next.load_designator(slot, pod.clone()).is_ok() {
                            candidates.push(next);
                        }
                    }
                }
                if let Some(slot) = editable.tank {
                    let mut empty = candidate.clone();
                    let _ = empty.unload_tank(slot);
                    candidates.push(empty);
                    for tank in &self.tanks {
                        let mut next = candidate.clone();
                        if next.select_tank(slot, tank.clone()).is_ok() {
                            for quantity in 1..=next.configuration.tanks[slot].quantity {
                                let mut amount = next.clone();
                                amount.configuration.tanks[slot].quantity = quantity;
                                candidates.push(amount);
                            }
                        }
                    }
                }
                for load in &mut candidates {
                    let _ = load.refresh_equipment();
                }
                let current = LoadoutSpec::of(&candidate);
                let at = self.cycles.get(&index).copied().unwrap_or_else(|| {
                    candidates
                        .iter()
                        .position(|c| LoadoutSpec::of(c) == current)
                        .unwrap_or(0)
                });
                let next_index = (at + 1) % candidates.len().max(1);
                self.cycles.insert(index, next_index);
                if let Some(next) = candidates.get(next_index) {
                    candidate = next.clone();
                }
            }
            match self.check(&candidate, data, spec) {
                Ok(()) => self.load = candidate,
                Err(e) => self.message = Some(e.to_string()),
            }
        }
        self.refresh();
        Outcome::Click
    }
    fn done(&mut self, data: &BTreeMap<String, Vec<u8>>, spec: &MissionSpec) -> Outcome {
        match self.check(&self.load, data, spec) {
            Ok(()) => Outcome::Done(self.load.aircraft, LoadoutSpec::of(&self.load)),
            Err(e) => {
                self.message = Some(e.to_string());
                Outcome::Click
            }
        }
    }
    fn event(
        &mut self,
        event: Event,
        data: &BTreeMap<String, Vec<u8>>,
        spec: &MissionSpec,
    ) -> Outcome {
        match event {
            Event::Select(index) => self.select(index, data, spec),
            Event::Button(0) => self.done(data, spec),
            Event::Button(_) => Outcome::Cancel,
            Event::Tab(index) => {
                self.widget.show_tab(index);
                Outcome::Click
            }
            Event::None => Outcome::None,
            _ => Outcome::Click,
        }
    }
    pub fn key(
        &mut self,
        key: &str,
        data: &BTreeMap<String, Vec<u8>>,
        spec: &MissionSpec,
    ) -> Outcome {
        if key == "Escape" {
            return Outcome::Cancel;
        }
        let event = self.widget.key(key, &self.tree);
        self.event(event, data, spec)
    }
    pub fn pointer(
        &mut self,
        point: Option<(f64, f64)>,
        down: bool,
        data: &BTreeMap<String, Vec<u8>>,
        spec: &MissionSpec,
    ) -> Outcome {
        let event = self.widget.pointer(&self.tree, &LOOK, point, down);
        self.event(event, data, spec)
    }
    pub fn preview_aircraft(&mut self) {
        if self.tree.len() > 1 {
            self.widget.show_tab(1);
        }
    }

    pub fn draw(&self, pixels: &mut [u8], font: &Font, prompt: &str) {
        self.widget.draw(pixels, font, &self.tree, &LOOK, &|_| None);
        crate::menu::Canvas(pixels).rect((10, 398, 620, 20), [24, 34, 45, 230]);
        if self.message.is_some() {
            crate::menu::Canvas(pixels).rect((10, 422, 620, 22), [24, 34, 45, 230]);
        }
        let mut paint = crate::hud::Paint {
            pixels,
            clip: (0, 0, 640, 480),
            color: [230, 240, 245, 255],
        };
        paint.text(font, prompt, 16, 404);
        if let Some(message) = &self.message {
            paint.text(font, message, 16, 426);
        }
    }
}

impl crate::App {
    pub(crate) fn respawn_open(&self) -> bool {
        self.net_flight
            .as_ref()
            .is_some_and(|f| f.respawn_menu.is_some())
    }
    pub(crate) fn open_respawn_menu(&mut self) {
        let Some(flight) = &self.net_flight else {
            return;
        };
        if flight.respawn_seen {
            return;
        }
        let Some(session) = &self.net else { return };
        if session.client.revival().is_none_or(|kept| {
            kept.revival.rule != tore_session::settings::Respawn::Revive
                || kept.revival.lives == Some(0)
        }) {
            return;
        }
        let Some(spec) = session.client.spec() else {
            return;
        };
        let Some((_, plane)) = session.client.seat() else {
            return;
        };
        let Some(entry) = self.world.roster.plane(plane) else {
            return;
        };
        let Some(aircraft) = self.world.aircraft_of(plane) else {
            return;
        };
        let choice = spec.player_choice[MissionSpec::wing_index(entry.slot.wing)];
        let allowed = if choice {
            AircraftId::SELECTABLE
                .into_iter()
                .filter(|id| {
                    session
                        .client
                        .gap_refusal(
                            tore_session::wire::messages::ItemKind::Aircraft,
                            id.selection_key(),
                        )
                        .is_none()
                        && AircraftType::load(&*self.theater_resources, *id).is_ok()
                })
                .collect()
        } else {
            vec![]
        };
        let value = session
            .client
            .lobby()
            .and_then(|l| {
                l.settings
                    .iter()
                    .find(|(n, _)| *n == tore_session::settings::number::REVIVE_WEAPONS)
            })
            .map_or(0, |(_, v)| *v);
        let rule = RevivalWeapons::from_value(value).unwrap_or_default();
        let menu = Menu::new(
            aircraft,
            allowed,
            &self.theater_resources,
            spec,
            rule,
            |name| {
                session
                    .client
                    .gap_refusal(tore_session::wire::messages::ItemKind::Weapon, name)
                    .is_some()
            },
        );
        let menu = menu.map(|mut menu| {
            let lobby = session.client.lobby();
            let kept = session
                .revival_draft
                .as_ref()
                .filter(|(mission, id, _)| {
                    lobby.is_some_and(|l| l.mission == *mission) && *id == aircraft
                })
                .map(|(_, _, load)| load)
                .or_else(|| {
                    lobby
                        .and_then(|l| l.me())
                        .and_then(|p| p.slot)
                        .and_then(|slot| spec.plane_loadouts.get(&slot))
                });
            if let Some(kept) = kept {
                menu.restore_selection(kept, &self.theater_resources, spec);
            }
            menu
        });
        let flight = self.net_flight.as_mut().expect("checked");
        flight.respawn_seen = true;
        match menu {
            Ok(menu) => flight.respawn_menu = Some(menu),
            Err(error) => self.flight_ui.message(error.to_string()),
        }
        self.camera.keys.clear();
        self.mouse_look = None;
        self.input.release_keys();
        self.release_trigger();
        self.input.context(self.input_paused(), self.focused);
    }
    fn respawn_outcome(&mut self, outcome: Outcome) -> crate::menu::Action {
        let Some(flight) = &mut self.net_flight else {
            return crate::menu::Action::None;
        };
        match outcome {
            Outcome::None => return crate::menu::Action::None,
            Outcome::Cancel => flight.respawn_menu = None,
            Outcome::Done(aircraft, loadout) => {
                if let Some(session) = &mut self.net
                    && let Some(lobby) = session.client.lobby()
                {
                    session.revival_draft = Some((lobby.mission, aircraft, loadout.clone()));
                }
                flight.respawn_choice = Some((aircraft, loadout));
                flight.respawn_menu = None;
            }
            Outcome::Click => {}
        }
        self.camera.keys.clear();
        self.mouse_look = None;
        self.input.release_keys();
        self.release_trigger();
        self.input.context(self.input_paused(), self.focused);
        crate::menu::Action::Click
    }
    pub(crate) fn respawn_key(&mut self, key: &str) -> crate::menu::Action {
        let Some(spec) = self.net.as_ref().and_then(|s| s.client.spec()) else {
            return crate::menu::Action::None;
        };
        let Some(menu) = self
            .net_flight
            .as_mut()
            .and_then(|f| f.respawn_menu.as_mut())
        else {
            return crate::menu::Action::None;
        };
        let outcome = menu.key(key, &self.theater_resources, spec);
        self.respawn_outcome(outcome)
    }
    pub(crate) fn respawn_pointer(
        &mut self,
        point: Option<(f64, f64)>,
        down: bool,
    ) -> crate::menu::Action {
        let Some(spec) = self.net.as_ref().and_then(|s| s.client.spec()) else {
            return crate::menu::Action::None;
        };
        let Some(menu) = self
            .net_flight
            .as_mut()
            .and_then(|f| f.respawn_menu.as_mut())
        else {
            return crate::menu::Action::None;
        };
        let outcome = menu.pointer(point, down, &self.theater_resources, spec);
        self.respawn_outcome(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (BTreeMap<String, Vec<u8>>, MissionSpec) {
        (
            tore_world::test_support::resources::gunship_resources(),
            MissionSpec::new(
                tore_world::test_support::resources::THEATER,
                AircraftId::F18,
            ),
        )
    }
    #[test]
    fn respawn_overlay_uses_source_station_order_and_escape_discards() {
        let (data, spec) = setup();
        let mut menu = Menu::new(
            AircraftId::F18,
            vec![AircraftId::F18, AircraftId::Ac130],
            &data,
            &spec,
            RevivalWeapons::Missiles,
            |_| false,
        )
        .unwrap();
        assert_eq!(
            menu.tree[0].children.len(),
            menu.load.station_display_order().len() + 3
        );
        let before = LoadoutSpec::of(&menu.load);
        assert!(matches!(menu.key("Escape", &data, &spec), Outcome::Cancel));
        assert_eq!(LoadoutSpec::of(&menu.load), before);
        menu.widget.show_tab(1);
        menu.select(1, &data, &spec);
        assert_eq!(menu.load.aircraft, AircraftId::Ac130);
        assert!(matches!(
            menu.done(&data, &spec),
            Outcome::Done(AircraftId::Ac130, _)
        ));
    }
    #[test]
    fn respawn_overlay_store_cycles_keep_the_host_loading_rules() {
        let (data, spec) = setup();
        let mut menu = Menu::new(
            AircraftId::F18,
            vec![],
            &data,
            &spec,
            RevivalWeapons::Guns,
            |_| false,
        )
        .unwrap();
        for index in 0..menu.load.station_display_order().len() {
            for _ in 0..5 {
                menu.select(index, &data, &spec);
                menu.check(&menu.load, &data, &spec).unwrap();
            }
        }
        let mut heavy = menu.load.clone();
        heavy.maximum_lbs = 0.;
        assert!(
            heavy
                .validate()
                .unwrap_err()
                .to_string()
                .contains("too heavy")
        );
    }
}
