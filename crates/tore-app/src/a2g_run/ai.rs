//! The AI strike driver: `--a2g-run THEATER STEM --pilot ai` (plan section 5,
//! slice ST-D, `.local/ai-a2g/plan.md`).
//!
//! The player-side kinds of this driver fly an aircraft through the seat's
//! own controls. This one lets the AI fly it: the aircraft starts `--from` NM
//! short of the target on the pass `--heading`, `--altitude`, `--speed` and
//! `--dive` describe, and from the first tick the AI's own controller flies
//! it, in the same mission step as the game. The driver only watches and
//! reports what the simulation shows. Every line starts `ai-strike:`.
//!
//! ```text
//! --a2g-run THEATER STEM --pilot ai [--aircraft ID] [--load REC] [--target N]
//!     [--station N] [--count N] [--skill 0..3] [--profile straight|popup|fasthigh|dive|level]
//!     [--handback S [--mode ccip|lvl] [--trigger]] [--order KIND [--order-at S]]
//!     [--quick-wing]
//!     [--from NM] [--altitude FT] [--speed KT] [--heading DEG] [--dive DEG]
//!     [--defenses AAA SAM] [--surface-seed N] [--seconds S]
//! ```
//!
//! - `--aircraft`, `--load`, `--station`, `--count`: the striker and its store,
//!   fitted as the player-side kinds fit it (a store the aircraft does not
//!   carry is fitted to a bomb, pod or missile station for the run).
//! - `--target N`: the Nth ground target of the stem (0 by default).
//! - `--skill 0..3`: the striker's wing, Novice to Ace (Average by default).
//! - `--profile`: **forces** the pass for coverage. The real AI chooses its
//!   pass from the perceived threat and its energy (John, 2026-10-11, answer
//!   3); the flag exists so a test can fly each pass once.
//! - `--handback S`: a human-flown run handed to the AI. A seat flies the
//!   striker on the scripted pass, designates the target, selects the store
//!   (and `--mode` of the bomb sight) and gives the aircraft back to the AI
//!   `S` seconds into the pass, for the takeover tests. `--trigger` has the
//!   human hold the trigger from the selection on (pickle and hold), so a
//!   bomb is released, and reported, before the handoff.
//! - `--order KIND`: wing orders. The seat leads a two-ship wing on the
//!   scripted pass, designates the target and gives the wingman (the striker)
//!   the order `--order-at S` seconds into the pass (5 by default). Kinds:
//!   see [`ORDERS`]; the orders the strike round adds join that table.
//! - `--quick-wing`: a Quick Mission's own strike wing (slice ST-O). No seat
//!   flies and nothing orders it: friendly wing 2 is two `--aircraft`, and the
//!   mission's default role (bomber-class aircraft strike when a ground target
//!   is set) sends it. It starts where the mission puts it and flies to the
//!   target itself; `ROLE` reports the role it was given.
//!
//! What it prints, in this order, then one `summary` and `done`:
//!
//! - `setup`: the aircraft, store, target, skill, forced profile, handback
//!   and order.
//! - `start`: the striker's place at tick 0 (range to the target, altitude).
//! - `HANDBACK`, `ORDER`: the handoff and the order, with what was kept.
//! - `PROFILE name=P requested=R`: the pass the AI reports it flies. `none`
//!   until slice ST-T reports one (see [`reported_profile`]).
//! - `RELEASE`: a store left the striker (time, store, station, height above
//!   the ground, speed, range to the target, rounds left).
//! - `IMPACT miss_ft=F`: a released round ended, and how far (horizontal) from
//!   the target it ended.
//! - `KILL`: a ground object the striker destroyed.
//! - `EGRESS`: after a release the striker is opening from the target (one
//!   nautical mile past its closest approach).
//! - `LOST`: the striker was destroyed, crashed, ejected or left the AI.
//! - `ACTIVITY`: what the AI says it is doing, each time that changes.
//!
//! The hooks the strike slices fill are in one place, [`hooks`]: ST-T makes
//! the AI take the target and report its profile, ST-K reports what a takeover
//! kept, ST-O adds order kinds. Until they land the driver reports what the
//! existing AI does with a ground target, which is nothing (no RELEASE), and
//! says so.
use super::{FEET_PER_NM, FPS_PER_KNOT, Options, SEAT, Scene, ccip::Pass, fit_store};
use crate::AppResult;
use std::collections::BTreeMap;
use tore_sim::{
    ai::{
        controller::Activity, engagement::StrikeRole, mission::AiActor, strike::Pass as StrikePass,
        weapon_service::Rounds, wing::PlayerOrder,
    },
    attitude::Vector,
    combat::{
        blast::MarkKind,
        delivery::DeliveryMode,
        live::{self, Command, EffectKind},
    },
};
use tore_world::{
    mission::{Defense, MissionSpec, Skill},
    seats::{PlaneId, SeatCommand, SeatId},
    world::{OrderOutcome, Seating, TickOutput, World},
};

/// The seat a handoff of the striker uses when the main seat leads the wing.
const WINGMAN_SEAT: SeatId = SeatId(1);
/// Seconds the seat may spend designating the target.
const DESIGNATE_SECONDS: f64 = 12.;
/// Seconds after the last release or loss the run goes on, to see where the
/// rounds land and the striker leaves.
const SETTLE_SECONDS: f64 = 20.;
/// An impact is the burst nearest the round's last place within this many feet.
const BURST_WITHIN_FT: f64 = 60.;
/// The striker is egressing once it is this far past its closest approach
/// after a release.
const EGRESS_OPENING_FT: f64 = FEET_PER_NM;

/// The passes `--profile` can force (docs/spec/ai.md, the retail script's
/// four passes, and the level run the heavy bombers fly).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Profile {
    Straight,
    Popup,
    FastHigh,
    Dive,
    Level,
}

impl Profile {
    pub(crate) const ALL: [Self; 5] = [
        Self::Straight,
        Self::Popup,
        Self::FastHigh,
        Self::Dive,
        Self::Level,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Straight => "straight",
            Self::Popup => "popup",
            Self::FastHigh => "fasthigh",
            Self::Dive => "dive",
            Self::Level => "level",
        }
    }

    fn pass(self) -> StrikePass {
        match self {
            Self::Straight => StrikePass::Straight,
            Self::Popup => StrikePass::PopUp,
            Self::FastHigh => StrikePass::FastHigh,
            Self::Dive => StrikePass::Dive,
            Self::Level => StrikePass::Level,
        }
    }

    fn from_pass(pass: StrikePass) -> Self {
        match pass {
            StrikePass::Straight => Self::Straight,
            StrikePass::PopUp => Self::Popup,
            StrikePass::FastHigh => Self::FastHigh,
            StrikePass::Dive => Self::Dive,
            StrikePass::Level => Self::Level,
        }
    }

    fn parse(text: &str) -> AppResult<Self> {
        Self::ALL
            .into_iter()
            .find(|p| p.label() == text.to_ascii_lowercase())
            .ok_or_else(|| {
                format!("unknown --profile {text}; use straight, popup, fasthigh, dive or level")
                    .into()
            })
    }
}

/// The wing orders `--order` can give, by name. The last three are the strike
/// round's (slice ST-O): Alt+F, Alt+W and "Lase my target".
const ORDERS: [(&str, PlayerOrder); 11] = [
    ("engage-my-target", PlayerOrder::EngageMyTarget),
    ("engage-from-formation", PlayerOrder::EngageFromFormation),
    ("attack-on-contact", PlayerOrder::AttackOnContact),
    ("protect-me", PlayerOrder::ProtectMe),
    ("disengage", PlayerOrder::Disengage),
    ("bug-out", PlayerOrder::BugOut),
    ("land-selected", PlayerOrder::LandAtSelected),
    ("sort", PlayerOrder::Sort),
    ("engage-ir-laser", PlayerOrder::EngageIrLaser),
    ("engage-class", PlayerOrder::EngageClass),
    ("lase-my-target", PlayerOrder::LaseMyTarget),
];

/// The command-line options of this driver. The shared parser in
/// `a2g_run.rs` fills it.
#[derive(Default)]
pub(crate) struct Config {
    /// `--pilot ai`.
    pub active: bool,
    pub profile: Option<Profile>,
    pub handback: Option<f64>,
    pub mode: Option<DeliveryMode>,
    /// `--trigger`: the human leg holds the trigger once the store is
    /// selected (pickle and hold), so the human releases before it hands back.
    pub trigger: bool,
    pub order: Option<(&'static str, PlayerOrder)>,
    pub order_at: f64,
    /// `--quick-wing`.
    pub quick_wing: bool,
}

impl Config {
    pub(crate) fn new() -> Self {
        Self {
            order_at: 5.,
            ..Self::default()
        }
    }

    /// `--pilot player|ai`.
    pub(crate) fn pilot(&mut self, text: &str) -> AppResult<()> {
        self.active = match text.to_ascii_lowercase().as_str() {
            "ai" => true,
            "player" => false,
            other => return Err(format!("unknown --pilot {other}; use player or ai").into()),
        };
        Ok(())
    }

    pub(crate) fn set_profile(&mut self, text: &str) -> AppResult<()> {
        self.profile = Some(Profile::parse(text)?);
        Ok(())
    }

    pub(crate) fn set_mode(&mut self, text: &str) -> AppResult<()> {
        self.mode = Some(match text.to_ascii_lowercase().as_str() {
            "ccip" => DeliveryMode::Ccip,
            "lvl" | "level" => DeliveryMode::Level,
            other => return Err(format!("unknown --mode {other}; use ccip or lvl").into()),
        });
        Ok(())
    }

    pub(crate) fn set_order(&mut self, text: &str) -> AppResult<()> {
        let kinds: Vec<&str> = ORDERS.iter().map(|(name, _)| *name).collect();
        self.order = Some(
            *ORDERS
                .iter()
                .find(|(name, _)| *name == text.to_ascii_lowercase())
                .ok_or_else(|| format!("unknown --order {text}; kinds: {}", kinds.join(", ")))?,
        );
        Ok(())
    }

    /// Refuses combinations the driver cannot fly.
    pub(crate) fn check(&self, skill: Option<i32>, records: bool) -> AppResult<()> {
        let flags = self.handback.is_some()
            || self.order.is_some()
            || self.mode.is_some()
            || self.quick_wing;
        if !self.active {
            if self.profile.is_some() || flags {
                return Err(
                    "--profile, --handback, --mode, --order and --quick-wing need --pilot ai"
                        .into(),
                );
            }
            return Ok(());
        }
        if self.quick_wing
            && (self.handback.is_some()
                || self.order.is_some()
                || self.profile.is_some()
                || self.mode.is_some())
        {
            return Err("--quick-wing is the mission's own wing: no --handback, --order, --profile or --mode".into());
        }
        if self.handback.is_some() && self.order.is_some() {
            return Err("--handback and --order are separate runs: pick one".into());
        }
        if (self.mode.is_some() || self.trigger) && self.handback.is_none() {
            return Err("--mode and --trigger set the human leg: they need --handback".into());
        }
        if records {
            return Err("--record and --render are not built for --pilot ai (slice ST-RA)".into());
        }
        if let Some(skill) = skill
            && !(0..=3).contains(&skill)
        {
            return Err("--skill is 0 (Novice) to 3 (Ace)".into());
        }
        Ok(())
    }
}

/// What the strike slices fill in. Each hook says what it does now.
mod hooks {
    use super::*;

    /// Slice ST-T: orders the AI onto the target (an ordered ground target
    /// outranks any briefing, docs/spec/ai-strike.md "What the AI knows"),
    /// keeps it to the driver's store, and forces its pass when `--profile`
    /// asks. Returns the note the `PROFILE` line carries.
    pub(super) fn assign(
        world: &mut World,
        plane: PlaneId,
        target: u32,
        station: usize,
        forced: Option<Profile>,
    ) -> &'static str {
        let Some(wings) = world.ai_wings.as_mut() else {
            return "no-ai";
        };
        if !wings.set_strike_order(plane.0, Some(target)) {
            return "not-ai-flown";
        }
        wings.force_strike(plane.0, forced.map(Profile::pass), Some(station));
        if forced.is_some() { "forced" } else { "chosen" }
    }

    /// Slice ST-T: the pass the AI reports it is flying, if any.
    pub(super) fn reported_profile(actor: &AiActor) -> Option<Profile> {
        actor
            .strike()
            .and_then(|s| s.pass())
            .map(Profile::from_pass)
    }

    /// The aim of the striker's current pass: the drawn offset's distance
    /// from the target, feet, whether it lies inside the circle, and the
    /// pass number. A smart weapon aims at the target itself (0 ft).
    pub(super) fn aim(actor: &AiActor) -> Option<(f64, bool, u32)> {
        let s = actor.strike()?;
        let (offset, inside) = s.aim();
        let unguided = s.selection().is_none_or(|sel| sel.kind.unguided());
        let distance = if unguided {
            offset[0].hypot(offset[1])
        } else {
            0.
        };
        Some((distance, inside || !unguided, s.pass_number()))
    }

    /// Slice ST-O: a wing order is the seat's command, as in the game: the
    /// world reads the ground target from the seat's designation and sight,
    /// and the wingman takes or refuses it.
    pub(super) fn order_command(order: PlayerOrder) -> SeatCommand {
        SeatCommand::WingOrder(order)
    }
}

/// A straight pass: where the striker starts and what it flies.
fn pass(scene: &Scene, o: &Options) -> AppResult<Pass> {
    let row = scene.target_row().ok_or("no target row")?;
    let aim = [
        row.position[0],
        scene.ground(row.position[0], row.position[2]),
        row.position[2],
    ];
    let heading = o.heading_deg.to_radians();
    let pitch = -o.dive_deg.abs().to_radians();
    let speed = o.speed_kt * FPS_PER_KNOT;
    let forward = [
        heading.sin() * pitch.cos(),
        pitch.sin(),
        heading.cos() * pitch.cos(),
    ];
    // The pass runs through the point `--altitude` above the target.
    let length = o.from_nm * FEET_PER_NM / pitch.cos();
    let start = std::array::from_fn(|i| {
        let through = if i == 1 { aim[1] + o.altitude } else { aim[i] };
        through - forward[i] * length
    });
    Ok(Pass {
        heading,
        pitch,
        speed,
        forward,
        start,
        aim,
    })
}

/// Puts `plane`'s cockpit on the pass at `along` feet from its start and
/// `right` feet to the right of it.
fn place(world: &mut World, plane: PlaneId, pass: &Pass, along: f64, right: f64) {
    let Some(cockpit) = world.cockpits.iter_mut().find(|c| c.plane == plane) else {
        return;
    };
    let side = [pass.heading.cos(), 0., -pass.heading.sin()];
    let position: Vector =
        std::array::from_fn(|i| pass.start[i] + pass.forward[i] * along + side[i] * right);
    let flight = &mut cockpit.flight;
    flight.position = position;
    flight.velocity = pass.forward.map(|v| v * pass.speed);
    flight.speed = pass.speed;
    flight.yaw = pass.heading;
    flight.pitch = pass.pitch;
    flight.bank = 0.;
    flight.vertical_speed = pass.forward[1] * pass.speed;
    flight.roll_rate = 0.;
    flight.pitch_rate = 0.;
}

/// One mission step. `human` is whether the main seat flies a plane.
fn step(scene: &mut Scene, human: bool, trigger: bool) -> AppResult<()> {
    if human {
        return scene.step(trigger);
    }
    scene.recorded(|world, out| world.step(&[], out))
}

/// Everything the watcher reports about the striker.
struct Watch {
    plane: PlaneId,
    target: u32,
    /// Rounds in flight: id to (last place, release number).
    flying: BTreeMap<u32, (Vector, usize)>,
    seen: std::collections::BTreeSet<u32>,
    releases: usize,
    impacts: usize,
    kills: usize,
    last_release: Option<f64>,
    last_impact: Option<f64>,
    closest: Option<(f64, f64)>,
    egress: bool,
    lost: Option<f64>,
    activity: Option<Activity>,
    profile: Option<Profile>,
}

impl Watch {
    fn new(plane: PlaneId, target: u32) -> Self {
        Self {
            plane,
            target,
            flying: BTreeMap::new(),
            seen: Default::default(),
            releases: 0,
            impacts: 0,
            kills: 0,
            last_release: None,
            last_impact: None,
            closest: None,
            egress: false,
            lost: None,
            activity: None,
            profile: None,
        }
    }
}

fn horizontal(a: Vector, b: Vector) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

impl Scene {
    /// The striker as the AI holds it, while the AI flies it.
    fn actor(&self, plane: PlaneId) -> Option<&AiActor> {
        self.world.ai_wings.as_ref()?.mission().actor(plane.0)
    }

    /// The striker's position, from the AI's flight or the seat's cockpit.
    fn striker_at(&self, plane: PlaneId) -> Option<(Vector, f64)> {
        if let Some(actor) = self.actor(plane) {
            let f = actor.flight();
            return Some((f.position, f.speed));
        }
        let f = &self
            .world
            .cockpits
            .iter()
            .find(|c| c.plane == plane)?
            .flight;
        Some((f.position, f.speed))
    }
}

/// The reporter: called after every tick.
fn observe(scene: &mut Scene, w: &mut Watch) {
    let t = scene.seconds();
    let tick = scene.world.combat.state.tick();
    let target_at = scene.target_row().map(|r| r.position);
    // The AI's activity, when it changes.
    let (activity, alive) = match scene.actor(w.plane) {
        Some(actor) => (Some(actor.activity()), actor.alive()),
        None => (
            None,
            scene.world.cockpits.iter().any(|c| c.plane == w.plane),
        ),
    };
    if let Some(activity) = activity
        && w.activity != Some(activity)
    {
        w.activity = Some(activity);
        println!("ai-strike: ACTIVITY t={t:.3} name={}", activity.label());
    }
    if let Some(profile) = scene.actor(w.plane).and_then(hooks::reported_profile)
        && w.profile != Some(profile)
    {
        w.profile = Some(profile);
        println!("ai-strike: PROFILE t={t:.3} name={}", profile.label());
    }
    let state = &scene.world.combat.state;
    // Releases: rounds of the striker not seen before.
    let fresh: Vec<_> = state
        .projectiles
        .iter()
        .filter(|p| p.owner == w.plane.0 && !w.seen.contains(&p.id))
        .filter(|p| p.weapon.as_ref().is_none_or(|wp| !live::is_gun(wp)))
        .filter(|p| p.gun_round.is_none())
        .map(|p| (p.id, p.position, p.station, p.weapon.clone()))
        .collect();
    let at = scene.striker_at(w.plane);
    for (id, position, station, weapon) in fresh {
        w.seen.insert(id);
        w.releases += 1;
        w.flying.insert(id, (position, w.releases));
        w.last_release = Some(t);
        let own = scene
            .world
            .combat
            .state
            .ownship(w.plane.0)
            .map(|o| o.configuration().stations[station].weapon.source.clone());
        let name = weapon.map(|wp| wp.source).or(own).unwrap_or_default();
        let (alt, speed, range) = match (at, target_at) {
            (Some((p, s)), Some(tp)) => (
                p[1] - scene.ground(p[0], p[2]),
                s / FPS_PER_KNOT,
                horizontal(p, tp),
            ),
            _ => (f64::NAN, f64::NAN, f64::NAN),
        };
        let aim = scene.actor(w.plane).and_then(hooks::aim).map_or(
            String::new(),
            |(offset, inside, pass)| {
                format!(
                    " pass={pass} aim_ft={offset:.0} inside={}",
                    if inside { "yes" } else { "no" }
                )
            },
        );
        println!(
            "ai-strike: RELEASE t={t:.3} n={} weapon={} station={station} alt_ft={alt:.0} speed_kt={speed:.0} range_ft={range:.0}{aim}",
            w.releases,
            if name.is_empty() { "unknown" } else { &name },
        );
    }
    // Rounds that ended this tick.
    let state = &scene.world.combat.state;
    let gone: Vec<u32> = w
        .flying
        .keys()
        .copied()
        .filter(|id| !state.projectiles.iter().any(|p| p.id == *id))
        .collect();
    for id in gone {
        let (last, n) = w.flying.remove(&id).expect("tracked");
        let burst = state
            .marks
            .iter()
            .filter(|m| m.born == tick && matches!(m.kind, MarkKind::Crater(_)))
            .map(|m| m.position)
            .chain(
                state
                    .effects
                    .iter()
                    .filter(|e| {
                        matches!(
                            e.kind,
                            EffectKind::Ground | EffectKind::Hit | EffectKind::Destroyed
                        )
                    })
                    .map(|e| e.position),
            )
            .map(|at| (horizontal(at, last), at))
            .filter(|(d, _)| *d < BURST_WITHIN_FT)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(last, |(_, at)| at);
        w.impacts += 1;
        w.last_impact = Some(t);
        match target_at {
            Some(tp) => println!(
                "ai-strike: IMPACT t={t:.3} n={n} miss_ft={:.1} at={:.0},{:.0},{:.0}",
                horizontal(burst, tp),
                burst[0],
                burst[1],
                burst[2]
            ),
            None => println!("ai-strike: IMPACT t={t:.3} n={n} miss_ft=nan"),
        }
    }
    // Rounds still flying keep their last place for the next tick's impact.
    for (id, (last, _)) in w.flying.iter_mut() {
        if let Some(p) = state.projectiles.iter().find(|p| p.id == *id) {
            *last = p.position;
        }
    }
    // Kills credited to the striker.
    let kills = state
        .ledger
        .kills()
        .iter()
        .filter(|k| k.owner == w.plane.0)
        .count();
    while w.kills < kills {
        let kill = state
            .ledger
            .kills()
            .iter()
            .filter(|k| k.owner == w.plane.0)
            .nth(w.kills)
            .expect("counted");
        println!(
            "ai-strike: KILL t={t:.3} victim={:#010x} category={:#06x} target={}",
            kill.victim,
            kill.category,
            if kill.victim == w.target { "yes" } else { "no" }
        );
        w.kills += 1;
    }
    // The approach, the closest point and the egress.
    if let (Some((p, _)), Some(tp)) = (scene.striker_at(w.plane), target_at) {
        let range = horizontal(p, tp);
        if w.closest.is_none_or(|(r, _)| range < r) {
            w.closest = Some((range, t));
        }
        if let (Some(_), false) = (w.last_release, w.egress)
            && let Some((closest, _)) = w.closest
            && range > closest + EGRESS_OPENING_FT
            && w.flying.is_empty()
        {
            w.egress = true;
            println!(
                "ai-strike: EGRESS t={t:.3} range_ft={range:.0} alt_ft={:.0}",
                p[1] - scene.ground(p[0], p[2])
            );
        }
    }
    if w.lost.is_none() && !alive {
        w.lost = Some(t);
        let reason = scene
            .world
            .combat
            .state
            .targets
            .iter()
            .find(|row| row.id == w.plane.0)
            .map_or("left-the-mission", |row| {
                if row.hp <= 0 {
                    "destroyed"
                } else {
                    "not-flying"
                }
            });
        println!("ai-strike: LOST t={t:.3} reason={reason}");
    }
}

/// The seat designates the target on the A-G page, as a pilot does: the
/// nearest contact to the target on the page, else after a few seconds the
/// Easy targeting cheat's Ctrl+X (an aircraft with no ground radar). Returns
/// the designated id, if any.
fn designate(scene: &mut Scene, pass: &Pass, plane: PlaneId) -> AppResult<Option<u32>> {
    scene.queue(SeatCommand::Combat(Command::ToggleAirGround));
    let easy = scene.world.combat.state.cheats.easy_targeting;
    let mut found = None;
    for tick in 0..(DESIGNATE_SECONDS * 120.) as u64 {
        pass.fly(scene, 0.);
        let own = scene
            .world
            .combat
            .state
            .ownship(plane.0)
            .ok_or("no ownship")?;
        if let Some(id) = own.designated() {
            found = Some(id);
            break;
        }
        let aim = pass.aim;
        let nearest = own
            .sensors
            .ground()
            .iter()
            .min_by(|a, b| {
                let d = |p: [f64; 3]| (p[0] - aim[0]).hypot(p[2] - aim[2]);
                d(a.contact.position).total_cmp(&d(b.contact.position))
            })
            .map(|g| g.contact.id);
        if let Some(id) = nearest {
            scene.queue(SeatCommand::Combat(Command::DesignateTarget(id)));
        } else if tick > 4 * 120 && tick % 60 == 0 {
            scene.world.combat.state.cheats.easy_targeting = true;
            scene.queue(SeatCommand::Combat(Command::NearestGround));
        }
        scene.step(false)?;
    }
    scene.world.combat.state.cheats.easy_targeting = easy;
    Ok(found)
}

pub(super) fn run(o: &Options, resources: &BTreeMap<String, Vec<u8>>) -> AppResult<()> {
    let cfg = &o.ai;
    let skill = Skill::ALL[o.skill.unwrap_or(1) as usize];
    let ordering = cfg.order.is_some();
    let mut spec = MissionSpec::new(&o.theater, o.aircraft);
    spec.ground_target = Some(o.stem.clone());
    spec.aaa = Defense::from_level(o.defenses.0).ok_or("defense levels are 0 to 3")?;
    spec.sam = Defense::from_level(o.defenses.1).ok_or("defense levels are 0 to 3")?;
    spec.surface_seed = o.seed;
    spec.wings[0].skill = skill;
    spec.wings[0].count = if ordering { 2 } else { 1 };
    // A Quick Mission's own strike wing: friendly wing 2, which the
    // mission's role rule sends when the aircraft is a bomber.
    let quick = cfg.quick_wing;
    if quick {
        spec.wings[1] = tore_world::mission::WingSpec {
            aircraft: o.aircraft,
            count: 2,
            skill,
        };
    }
    spec.validate()?;
    let mut world = World::new(&spec, resources, Seating::Open)?;
    if let Some(why) = &world.terrain.surface.unresolved {
        return Err(format!("the ground target stands nowhere: {why}").into());
    }
    let targets =
        tore_world::ai_wings::outcome::ground_targets(&world.terrain.surface, &world.combat.state);
    let target = match o.unit {
        Some(id) => id,
        None => *targets
            .get(o.target)
            .ok_or_else(|| format!("{} has {} ground targets", o.stem, targets.len()))?,
    };
    // The striker is plane 0, or the wingman (plane 1) when a seat leads.
    let striker = PlaneId(if quick { 1 } else { u32::from(ordering) });
    let lead = PlaneId(0);
    let striker_seat = if ordering { WINGMAN_SEAT } else { SEAT };

    // Take the striker on a seat to fit its store and put it on the pass: the
    // AI's own aircraft take a loadout that way for the player-side kinds too.
    world.take_plane(striker_seat, striker)?;
    let station = fit_store(&mut world, striker.0, o, resources)?;
    let mut scene = Scene {
        world,
        out: TickOutput::default(),
        commands: Vec::new(),
        station,
        target,
        owner: striker.0,
        record: None,
    };
    let pass = pass(&scene, o)?;
    let row = scene.target_row().ok_or("the target has no row")?;
    let (target_at, hp) = (row.position, row.hp);
    {
        let own = scene
            .world
            .combat
            .state
            .ownship(striker.0)
            .ok_or("no ownship")?;
        let s = &own.configuration().stations[station];
        println!(
            "ai-strike: setup aircraft={:?} load={} station={station} count={} rounds={} target={target:#010x} target_at={:.0},{:.0},{:.0} hp={hp} skill={} profile={} handback={} order={} mode={}",
            o.aircraft,
            s.weapon.source,
            s.count,
            own.rounds(station),
            target_at[0],
            target_at[1],
            target_at[2],
            o.skill.unwrap_or(1),
            cfg.profile.map_or("none", Profile::label),
            cfg.handback.map_or("none".to_owned(), |s| format!("{s}")),
            cfg.order.map_or("none", |(name, _)| name),
            cfg.mode.map_or("none", DeliveryMode::label),
        );
    }
    let mut human = false;
    if ordering {
        // The wingman starts 600 ft right of the lead and 300 ft behind; the
        // seat then flies the lead.
        place(&mut scene.world, striker, &pass, -300., 600.);
        scene.world.give_back_plane(striker_seat)?;
        scene.world.take_plane(SEAT, lead)?;
        place(&mut scene.world, lead, &pass, 0., 0.);
        scene.owner = lead.0;
        scene.station = 0;
        human = true;
    } else if quick {
        // The wing stays where the mission put it.
        scene.world.give_back_plane(SEAT)?;
    } else if cfg.handback.is_some() {
        place(&mut scene.world, striker, &pass, 0., 0.);
        human = true;
    } else {
        place(&mut scene.world, striker, &pass, 0., 0.);
        scene.world.give_back_plane(SEAT)?;
    }
    let mut watch = Watch::new(striker, target);
    // With a seat leading, the order gives the wingman its target; without
    // one the driver does.
    let note = if ordering {
        "ordered"
    } else if quick {
        "quick-wing"
    } else {
        hooks::assign(&mut scene.world, striker, target, station, cfg.profile)
    };
    println!(
        "ai-strike: PROFILE t=0.000 name=none requested={} note={note}",
        cfg.profile.map_or("none", Profile::label)
    );
    if quick && let Some(actor) = scene.actor(striker) {
        println!(
            "ai-strike: ROLE t=0.000 plane={} role={}",
            striker.0,
            match actor.strike_role() {
                StrikeRole::Strike => "strike",
                StrikeRole::Cap => "cap",
            }
        );
    }
    if let Some((p, _)) = scene.striker_at(striker) {
        println!(
            "ai-strike: start t=0.000 plane={} range_ft={:.0} alt_ft={:.0} speed_kt={:.0}",
            striker.0,
            horizontal(p, target_at),
            p[1] - scene.ground(p[0], p[2]),
            o.speed_kt
        );
    }

    // The human leg: designate, select the store, fly the pass, then hand
    // back (`--handback`) or give the order (`--order`).
    let mut leg_t = 0.;
    if human {
        let flier = if ordering { lead } else { striker };
        let designated = designate(&mut scene, &pass, flier)?;
        match designated {
            Some(id) => {
                scene.target = id;
                watch.target = id;
                println!(
                    "ai-strike: DESIGNATE t={:.3} target={id:#010x} {}",
                    scene.seconds(),
                    scene.world.combat.ground_name(id).unwrap_or("unnamed")
                );
            }
            None => println!("ai-strike: DESIGNATE t={:.3} target=none", scene.seconds()),
        }
    }
    let mut ordered = false;
    let mut handed = !human || ordering;
    let started = scene.seconds();
    for tick in 0..(o.seconds * 120.) as u64 {
        if human {
            leg_t = tick as f64 / 120.;
            pass.fly(&mut scene, leg_t);
            if !ordering && !handed {
                scene.select();
                if let Some(mode) = cfg.mode
                    && scene.selected()
                    && scene.commands.is_empty()
                    && scene
                        .world
                        .combat
                        .state
                        .ownship(striker.0)
                        .is_some_and(|own| own.delivery_mode != mode)
                {
                    scene.queue(SeatCommand::Combat(Command::ToggleSeekerMode));
                }
            }
            if ordering
                && !ordered
                && leg_t >= cfg.order_at
                && let Some((name, order)) = cfg.order
            {
                scene.queue(hooks::order_command(order));
                ordered = true;
                println!(
                    "ai-strike: ORDER t={:.3} kind={name} designated={}",
                    scene.seconds(),
                    scene
                        .world
                        .combat
                        .state
                        .ownship(lead.0)
                        .and_then(|own| own.designated())
                        .map_or("none".to_owned(), |id| format!("{id:#010x}"))
                );
            }
        }
        if let (Some(at), false) = (cfg.handback, handed)
            && leg_t >= at
        {
            let (designated, selected, mode) = {
                let own = scene
                    .world
                    .combat
                    .state
                    .ownship(striker.0)
                    .ok_or("no ownship to hand back")?;
                (own.designated(), own.selected, own.delivery_mode)
            };
            scene.world.give_back_plane(SEAT)?;
            handed = true;
            human = false;
            println!(
                "ai-strike: HANDBACK t={:.3} designated={} selected={selected} mode={}",
                scene.seconds(),
                designated.map_or("none".to_owned(), |id| format!("{id:#010x}")),
                mode.label()
            );
        }
        let trigger = human && cfg.trigger && scene.selected();
        step(&mut scene, human, trigger)?;
        for reply in &scene.out.orders {
            let (result, message) = match &reply.outcome {
                OrderOutcome::Given { message } => ("given", message),
                OrderOutcome::Refused { message } => ("refused", message),
                OrderOutcome::Failed { message } => ("failed", message),
            };
            println!(
                "ai-strike: ORDER-REPLY t={:.3} result={result} message=\"{message}\"",
                scene.seconds()
            );
            // What the wingman holds now: the order it took, if it took one.
            if let Some(actor) = scene.actor(striker) {
                let object =
                    |id: Option<u32>| id.map_or("none".to_owned(), |id| format!("{id:#010x}"));
                println!(
                    "ai-strike: ORDER-STATE t={:.3} strike_order={} class_order={} lase_order={}",
                    scene.seconds(),
                    object(actor.strike_order()),
                    actor
                        .ground_picture()
                        .map_or(0, |picture| picture.class_order().len()),
                    object(actor.lase_order())
                );
            }
        }
        observe(&mut scene, &mut watch);
        let t = scene.seconds();
        // Done when the striker is gone or the last round has landed and the
        // striker has had time to leave.
        let over = [
            watch.lost,
            watch.last_impact.filter(|_| watch.flying.is_empty()),
        ]
        .into_iter()
        .flatten()
        .fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.max(b))));
        if over.is_some_and(|at| t > at + SETTLE_SECONDS) {
            break;
        }
    }
    let t = scene.seconds();
    let hp_end = scene.target_row().map(|r| r.hp);
    println!(
        "ai-strike: summary t={t:.3} ran_s={:.1} profile={} releases={} impacts={} kills={} egress={} lost={} activity={} target_hp={}/{} closest_ft={} store_rounds_left={}",
        t - started,
        watch.profile.map_or("none", Profile::label),
        watch.releases,
        watch.impacts,
        watch.kills,
        if watch.egress { "yes" } else { "no" },
        if watch.lost.is_some() { "yes" } else { "no" },
        watch.activity.map_or("human", Activity::label),
        hp_end.map_or("gone".to_owned(), |hp| hp.to_string()),
        hp,
        watch
            .closest
            .map_or("none".to_owned(), |(r, _)| format!("{r:.0}")),
        scene
            .actor(striker)
            .and_then(|actor| actor.stations().get(station))
            .map_or("human".to_owned(), |s| match s.store.rounds {
                Rounds::Finite(n) => n.to_string(),
                Rounds::Unlimited => "unlimited".to_owned(),
            }),
    );
    if watch.releases == 0 {
        println!("ai-strike: no release");
    }
    println!("ai-strike: done");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(args: &[&str]) -> AppResult<Config> {
        let mut c = Config::new();
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            match *arg {
                "--pilot" => c.pilot(it.next().unwrap())?,
                "--profile" => c.set_profile(it.next().unwrap())?,
                "--mode" => c.set_mode(it.next().unwrap())?,
                "--order" => c.set_order(it.next().unwrap())?,
                "--handback" => c.handback = Some(it.next().unwrap().parse()?),
                "--trigger" => c.trigger = true,
                other => panic!("{other}"),
            }
        }
        Ok(c)
    }

    #[test]
    fn profiles_round_trip_and_unknown_ones_are_refused() {
        for profile in Profile::ALL {
            assert_eq!(Profile::parse(profile.label()).unwrap(), profile);
        }
        assert_eq!(Profile::parse("DIVE").unwrap(), Profile::Dive);
        assert!(Profile::parse("loft").is_err());
    }

    #[test]
    fn order_names_are_unique_and_parse() {
        let mut names: Vec<_> = ORDERS.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ORDERS.len());
        let c = config(&["--order", "Engage-My-Target"]).unwrap();
        assert_eq!(c.order.map(|(_, o)| o), Some(PlayerOrder::EngageMyTarget));
        for kind in ["engage-ir-laser", "engage-class", "lase-my-target"] {
            assert!(config(&["--order", kind]).is_ok(), "{kind}");
        }
        assert!(config(&["--order", "lase-your-target"]).is_err());
    }

    #[test]
    fn the_options_only_make_sense_with_the_ai_flying() {
        assert!(config(&[]).unwrap().check(None, false).is_ok());
        let c = config(&["--profile", "dive"]).unwrap();
        assert!(c.check(None, false).is_err());
        let c = config(&["--pilot", "ai", "--profile", "dive"]).unwrap();
        assert!(c.check(Some(3), false).is_ok());
        assert!(c.check(Some(4), false).is_err());
        assert!(c.check(None, true).is_err());
    }

    #[test]
    fn a_handback_and_an_order_are_separate_runs() {
        let c = config(&["--pilot", "ai", "--handback", "5", "--order", "bug-out"]).unwrap();
        assert!(c.check(None, false).is_err());
        let c = config(&["--pilot", "ai", "--mode", "lvl"]).unwrap();
        assert!(c.check(None, false).is_err());
        let c = config(&[
            "--pilot",
            "ai",
            "--handback",
            "5",
            "--mode",
            "lvl",
            "--trigger",
        ])
        .unwrap();
        assert!(c.check(None, false).is_ok());
        assert_eq!(c.mode, Some(DeliveryMode::Level));
    }
}
