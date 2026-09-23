//! Session cheats from the in-flight Cheat menu. Behaviour: docs/spec/cheats.md.

/// The G every aircraft may pull with Pull extra G on (John, 2026-09-23).
pub const EXTRA_G: f64 = 9.;

/// Every switch starts off. Only the player's flight and combat state carry
/// them; AI actors keep the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cheats {
    pub invulnerable: bool,
    pub unlimited_ammo: bool,
    pub unlimited_fuel: bool,
    pub no_spins: bool,
    pub no_turbulence: bool,
    pub extra_g: bool,
    pub ignore_weapon_weights: bool,
    pub no_sun_whiteout: bool,
}
