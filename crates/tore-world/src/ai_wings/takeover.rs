//! Takeover (docs/spec/ai-strike.md, "Takeover and handback"; slice ST-K):
//! when a human gives back an aircraft it was attacking the ground with,
//! the AI continues that attack.
//!
//! [`AiWings::continue_attack`] reads the cockpit the human left
//! ([`live::Kept`]): its ground target (the designation, else the object
//! under the sight's pin), the selected station and the bomb sight's mode.
//! When the aircraft has a usable surface store and is one the strike
//! tactics fly, the new actor gets the Strike role (so a dead target is
//! followed by the mission's next), an order onto the human's target and a
//! strike that starts as a takeover of it
//! ([`tore_sim::ai::strike::Takeover`]). The tactics then attack only when
//! safe, with the kept store and mode, and the bridge
//! ([`super::strike`]) leaves the human's mode and laser hand setting alone.
use super::AiWings;
use super::strike::kind;
use tore_sim::ai::engagement::StrikeRole;
use tore_sim::ai::strike::{StoreKind, Strike, Takeover};
use tore_sim::ai::weapon_service::{Rounds, StationId};
use tore_sim::combat::live::{self, StrikeTarget};
use tore_sim::combat::missiles::TargetRole;
use tore_sim::combat::sight::SightMode;

/// The object under a pinned sight is the human's target when it stands
/// this close to the pin, horizontally (fitted, agent).
pub const PIN_RADIUS_FT: f64 = 1_000.;

/// The ground object a human's cockpit attacks: the designated object when
/// it is a standing hostile one on the ground, else the standing hostile
/// ground object nearest the sight's pin within [`PIN_RADIUS_FT`]. `side`
/// is the aircraft's.
pub fn ground_target(
    cockpit: &live::StrikeCockpit,
    side: live::Side,
    state: &live::State,
) -> Option<u32> {
    let hostile = |t: &&live::Target| {
        t.hp > 0
            && (t.role == TargetRole::Surface || !t.airborne)
            && t.side != side
            && t.side != live::NO_SIDE
    };
    if let Some(StrikeTarget::Object(id)) = cockpit.target
        && let Some(row) = state.targets.iter().find(|t| t.id == id)
    {
        return hostile(&row).then_some(id);
    }
    let pin = match cockpit.sight.map(|s| s.mode) {
        Some(SightMode::Pinned(pin)) => pin,
        _ => return None,
    };
    state
        .targets
        .iter()
        .filter(hostile)
        .map(|t| ((t.position[0] - pin[0]).hypot(t.position[2] - pin[2]), t.id))
        .filter(|(d, _)| *d <= PIN_RADIUS_FT)
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, id)| id)
}

impl AiWings {
    /// The AI continues the ground attack the human left in `kept` on AI
    /// aircraft `id`, just given back (see the module notes). Returns the
    /// target it continues, or `None` (changing nothing) when the cockpit
    /// holds no ground target, the aircraft carries no usable surface
    /// store, or it is a helicopter or the gunship (slice ST-F).
    pub fn continue_attack(
        &mut self,
        id: u32,
        kept: &live::Kept,
        state: &live::State,
    ) -> Option<u32> {
        let config = self.configs.get(&id)?;
        if !Strike::flies(config.strike_class) {
            return None;
        }
        let target = ground_target(&kept.cockpit, kept.side, state)?;
        let actor = self.mission.actor(id)?;
        let usable = |station: usize| -> bool {
            let has_rounds = actor.stations().iter().any(|spec| {
                usize::from(spec.station.0) == station
                    && !spec.store.inhibited
                    && !matches!(spec.store.rounds, Rounds::Finite(0))
            });
            has_rounds
                && config
                    .stations
                    .get(station)
                    .and_then(|s| kind(&s.weapon))
                    .is_some_and(|k| k != StoreKind::Command)
        };
        if !(0..config.stations.len()).any(usable) {
            return None;
        }
        let station = (kept.armed && usable(kept.selected)).then(|| StationId(kept.selected as u8));
        let takeover = Takeover::new(target, station, kept.delivery_mode);
        let actor = self.mission.actor_mut(id)?;
        actor.set_strike_role(StrikeRole::Strike);
        actor.set_strike_order(Some(target));
        actor.take_over_strike(takeover).then_some(target)
    }
}
