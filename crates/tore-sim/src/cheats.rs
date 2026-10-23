//! Session cheats from the in-flight Cheat menu. Behaviour: docs/spec/cheats.md.

/// The G every aircraft may pull with Pull extra G on (John, 2026-09-23).
pub const EXTRA_G: f64 = 9.;
/// Easy aiming (John, 2026-09-23): target hit volumes for the player's rounds,
/// the player's missile turn rate (proposed) and its seeker cone.
pub const EASY_AIMING_HITBOX: f64 = 1.5;
pub const EASY_AIMING_TURN: f64 = 1.5;
pub const EASY_AIMING_CONE: f64 = 1.25;

/// The Damage cheat: how hits on the player are applied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Damage {
    /// Hits do no damage at all.
    Invulnerable,
    /// Hits only take hit points: no system faults and no pilot kill
    /// (John, 2026-09-28).
    #[default]
    Normal,
    /// Hit points plus system faults and the pilot-kill hits (John, 2026-09-28).
    Realistic,
}

/// Every switch starts off and Damage starts at Normal. Only the player's
/// flight and combat state carry them; AI actors keep the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cheats {
    pub damage: Damage,
    pub unlimited_ammo: bool,
    pub unlimited_fuel: bool,
    pub no_spins: bool,
    pub no_turbulence: bool,
    pub extra_g: bool,
    pub ignore_weapon_weights: bool,
    pub no_sun_whiteout: bool,
    pub no_g_effects: bool,
    pub no_screen_shake: bool,
    pub no_crashes: bool,
    pub easy_aiming: bool,
    pub ignore_midair_collisions: bool,
    pub easy_targeting: bool,
    pub guns_only: bool,
    /// Easy flight physics (VTOL overhaul, design 4.12; John, 2026-10-08):
    /// the six powered-lift aircraft fly without the hazards of their
    /// physics: torque, the vortex ring state, retreating blade stall,
    /// unrecoverable rotor stall, the Harrier and Yak-141 roll-off, the
    /// undamped puffers at stability level Off and dynamic rollover; and the
    /// rotorcraft get a weak attitude retention about the trim attitude at
    /// Damper and Off (`flight::powered::sas`). Weight,
    /// power and the V-22's corridor protection stay. Fixed-wing aircraft
    /// ignore it. It changes the simulation, so the server alone sets it in
    /// a session.
    pub easy_physics: bool,
    /// Enemy AI: every enemy aircraft at this level; None is Unchanged.
    pub enemy_ai: Option<crate::ai::Experience>,
}

impl Cheats {
    pub fn invulnerable(&self) -> bool {
        self.damage == Damage::Invulnerable
    }
    pub fn system_damage(&self) -> bool {
        self.damage == Damage::Realistic
    }
}
