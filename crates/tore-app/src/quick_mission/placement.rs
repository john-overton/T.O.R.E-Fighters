//! Asynchronous placement audit for the creator's distance choices.
use super::*;
use std::sync::mpsc;
use tore_world::mission_layout::availability::{self, Availability};

pub const CHECKING: &str = "Checking distances and target placement...";
pub const TOO_FAR: &str = "This distance does not fit the selected theater, target and start.";

/// Inputs consumed by the placement audit, rather than the entire mission text.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    theater: String,
    condition: usize,
    weather: tore_world::terrain::Overrides,
    // None is airborne, Some(None) automatic ground, Some(Some(id)) a runway.
    ground: Option<Option<u32>>,
    target: Option<tore_world::surface::resolve::GroundTarget>,
    area: Option<tore_world::surface::resolve::AreaDefense>,
    player_count: usize,
    enemy_offsets: Vec<[f64; 2]>,
}

impl Key {
    fn of(spec: &MissionSpec) -> Result<Self, String> {
        use tore_world::{mission::Start, surface::resolve};
        let wings = spec.wing_launches().map_err(|e| e.to_string())?;
        Ok(Self {
            theater: spec.theater.clone(),
            condition: spec.condition.index(),
            weather: spec.weather.clone(),
            ground: match spec.start {
                Start::Airborne { .. } => None,
                Start::GroundAuto { .. } => Some(None),
                Start::Ground { runway, .. } => Some(Some(runway)),
            },
            target: resolve::GroundTarget::from_spec(spec),
            area: resolve::AreaDefense::from_spec(spec),
            player_count: spec.player_wing_size(),
            enemy_offsets: tore_world::ai_wings::enemy_group_offsets(&wings),
        })
    }
}

#[derive(Default)]
pub struct Check {
    key: Option<Key>,
    pending: Option<mpsc::Receiver<Result<Availability, String>>>,
    ready: Option<Result<Availability, String>>,
}

impl Check {
    fn poll(&mut self) {
        if let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.ready = Some(result);
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.ready = Some(Err(
                        "Placement check did not finish. Change the target to retry.".into(),
                    ));
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
    }
    fn request(
        &mut self,
        resources: Arc<BTreeMap<String, Vec<u8>>>,
        spec: MissionSpec,
    ) -> Option<Result<Availability, String>> {
        let key = match Key::of(&spec) {
            Ok(key) => Some(key),
            Err(error) => return Some(Err(error)),
        };
        self.poll();
        if self.key == key {
            return self.ready.clone();
        }
        if self.pending.is_some() {
            return None;
        }
        let (tx, rx) = mpsc::channel();
        self.key = key;
        self.ready = None;
        match std::thread::Builder::new()
            .name("tore-placement-check".into())
            .spawn(move || {
                let _ = tx.send(
                    availability::audit(resources.as_ref(), &spec).map_err(|e| e.to_string()),
                );
            }) {
            Ok(_) => self.pending = Some(rx),
            Err(e) => self.ready = Some(Err(format!("Cannot check placement: {e}"))),
        }
        self.ready.clone()
    }
}

impl QuickMission {
    /// CPU snapshot evidence uses the same availability as the selector.
    pub fn placement_snapshot_report(&self) -> String {
        let enabled = tore_world::mission_layout::SEPARATION_NM
            .into_iter()
            .enumerate()
            .filter(|(i, _)| self.separation_problem(*i).is_none())
            .map(|(_, nm)| nm.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "start {} enabled-distances {enabled}",
            if self.ground_start() {
                "ground"
            } else {
                "airborne"
            }
        )
    }

    pub(super) fn reserve_placement_seed(&mut self) {
        if self.lays_out_surface() && !self.seed_pending {
            if self.view.is_none() {
                self.surface_seed = super::fresh_seed();
            }
            self.seed_pending = true;
        }
    }
    /// CPU snapshot/probe entry point. Live menus only poll the worker.
    pub fn wait_placement(&mut self) {
        if self.resources.is_none() {
            return;
        }
        self.reserve_placement_seed();
        for _ in 0..2 {
            if self.placement_check().is_some() {
                return;
            }
            let mut state = self.placement.borrow_mut();
            if let Some(rx) = state.pending.take() {
                state.ready = Some(
                    rx.recv()
                        .unwrap_or_else(|_| Err("Placement check did not finish.".into())),
                );
            }
        }
    }
    pub(super) fn placement_check(&self) -> Option<Result<Availability, String>> {
        let resources = self.resources.as_ref()?;
        let spec = self.mission_spec().ok()?;
        self.placement.borrow_mut().request(resources.clone(), spec)
    }
    pub(super) fn separation_problem(&self, index: usize) -> Option<&'static str> {
        self.resources.as_ref()?;
        self.mission_spec().ok()?;
        match self.placement_check() {
            Some(Ok(result)) if result.separations.get(index).copied().unwrap_or(false) => None,
            Some(Ok(_)) => Some(TOO_FAR),
            Some(Err(_)) => Some("Placement could not be checked. Change the target or theater."),
            None => Some(CHECKING),
        }
    }
    pub(super) fn placement_notice(&self) -> Option<String> {
        self.resources.as_ref()?;
        match self.placement_check() {
            Some(Ok(a)) if !a.limitations.is_empty() => Some(a.limitations.join(" ")),
            Some(Err(e)) => Some(e),
            None => Some(CHECKING.into()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::aircraft::AircraftId;
    use tore_world::mission::{Condition, Defense, Skill, Start};

    #[test]
    fn changing_skill_keeps_a_ready_placement_check_available() {
        let mut spec = MissionSpec::new("UKR", AircraftId::F18);
        spec.ground_target = Some("QUCITY".into());
        spec.wings[3].count = 3;
        let key = Key::of(&spec).unwrap();
        let mut check = Check {
            key: Some(key.clone()),
            ready: Some(Ok(Availability {
                separations: [true; 11],
                limitations: vec![],
                blocked: false,
            })),
            pending: None,
        };
        for wing in &mut spec.wings {
            wing.skill = Skill::Ace;
        }
        spec.enemy_skill = Some(tore_sim::ai::experience::EnemySkillOverride::AllNovice);
        spec.guns_only = true;
        spec.objectives[0] = tore_sim::ai::engagement::GroupObjective::Hold;
        assert_eq!(Key::of(&spec).unwrap(), key);
        assert!(matches!(
            check.request(Arc::new(BTreeMap::new()), spec),
            Some(Ok(_))
        ));
        assert!(check.pending.is_none(), "no terrain worker was started");
    }

    #[test]
    fn placement_inputs_invalidate_the_cache() {
        let mut spec = MissionSpec::new("UKR", AircraftId::F18);
        spec.ground_target = Some("QUCITY".into());
        let key = Key::of(&spec).unwrap();
        let changes: [fn(&mut MissionSpec); 8] = [
            |s| s.surface_seed += 1,
            |s| s.sam = Defense::Heavy,
            |s| s.separation_nm = 100,
            |s| s.start = Start::GroundAuto { altitude_ft: 5000 },
            |s| s.wings[3].count = 5,
            |s| s.theater = "CUB".into(),
            |s| s.condition = Condition::Night,
            |s| s.weather.redrawn_airports = false,
        ];
        for change in changes {
            let mut changed = spec.clone();
            change(&mut changed);
            assert_ne!(Key::of(&changed).unwrap(), key);
        }
    }
}
