//! A gun round's release and flight without a live combat state, for a
//! networked client that draws the rounds the host does not send (docs/
//! ARCHITECTURE.md, "Hits and lag compensation" and "The client session").
//!
//! The rules are [`super::live::State::step`]'s own, written out for the one
//! case a client needs: an unguided round with no target. The muzzle, the
//! spread ([`projectile_launch_direction`]), the launch speed, the way the
//! round moves each tick (the speed command, the 120 Hz service cadence, the
//! fall) and its life are the simulation's; the trigger's cadence is the
//! same schedule of rounds ([`Cadence`]), and a burst whose first tick is
//! known can be laid out without a trigger ([`release_tick`]). Nothing here
//! hits anything: a client's rounds are for the eye, the host decides every
//! hit.
//!
//! Ticks here are the combat tick a step starts at (the one a world reports
//! before it steps), the number a host tags a burst event with.
use super::{
    FallState, PlayerTrigger, axial_speed, commanded_speed, engine_phase, launch_speed,
    live::projectile_launch_direction, live::terrain_hit, removal_due,
};
use crate::attitude::Vector;
use tore_formats::weapons::{Movement, Weapon};

/// How many "service" units combat's step n advances rounds by: 256 a tick
/// shared out over the 120 ticks of a second, so alternately 2 and 3 in a
/// pattern that repeats every 15 ticks. The host keeps a running remainder
/// from tick 0; this is the same cumulative sum.
pub fn service_ticks(tick: u64) -> i16 {
    (((tick + 1) * 256) / 120 - (tick * 256) / 120) as i16
}

/// The physical rounds a gun fires in one burst of its record: rounds in a
/// burst times rounds per game round, at least 1.
fn physical_rounds(weapon: &Weapon) -> u64 {
    u64::from(weapon.burst.game_rounds_in_burst.max(1))
        * u64::from(weapon.burst.actual_rounds_per_game.max(1))
}

/// Scaled ticks between a held gun's rounds: its burst time in 30ths of a
/// second of 120 Hz ticks, to be shared over [`physical_rounds`].
fn round_span(weapon: &Weapon) -> u64 {
    u64::from(weapon.burst.game_burst_t.max(1)) * 30
}

/// The tick the `n`th round of a burst (the first is 0) leaves a gun held
/// down without a break from `first`, the tick of the first round: combat
/// spaces the rounds `burst time * 30 / physical rounds` ticks apart and
/// fires on the first tick at or after each one.
pub fn release_tick(weapon: &Weapon, first: u64, n: u64) -> u64 {
    first + (n * round_span(weapon)).div_ceil(physical_rounds(weapon))
}

/// What the trigger's cadence let go this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fired {
    /// Rounds the station has let go before this one.
    pub ordinal: u64,
    /// Every third round carries a tracer (fitted marker, as combat's).
    pub tracer: bool,
}

/// One gun station's trigger and round schedule, as combat keeps them: the
/// press, the rounds waiting and the scaled deadline of the next.
#[derive(Clone, Debug, Default)]
pub struct Cadence {
    trigger: PlayerTrigger,
    pending: u16,
    next_scaled: u64,
    ordinal: u64,
}

impl Cadence {
    /// Drops the rounds waiting and the held trigger, as changing weapon does.
    pub fn discard(&mut self) {
        self.trigger.release();
        self.pending = 0;
    }

    /// The rounds the station has let go.
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }

    /// One combat tick starting at `tick`: whether the trigger is held (and
    /// the aircraft whole) and whether the weapon may fire (armed, with
    /// rounds, nothing in the way: the readout's readiness). Returns the round
    /// let go, if one.
    pub fn step(&mut self, weapon: &Weapon, held: bool, allowed: bool, tick: u64) -> Option<Fired> {
        let now = (tick / 30) as u16;
        // Combat counts the tick it is stepping to from here on.
        let after = tick + 1;
        let physical = physical_rounds(weapon);
        let pressed = held && !self.trigger.was_held;
        let polled = self
            .trigger
            .poll(held, weapon.flags, weapon.burst.game_burst_t, now);
        let due = polled || pressed;
        if due && allowed {
            self.pending = self
                .pending
                .saturating_add(physical as u16)
                .min(physical as u16);
            self.next_scaled = self.next_scaled.max(after.saturating_mul(physical));
        }
        if !held {
            self.pending = 0;
        }
        if !allowed && self.pending > 0 {
            self.next_scaled = after
                .saturating_mul(physical)
                .saturating_add(round_span(weapon));
        }
        let ready =
            self.pending > 0 && allowed && after.saturating_mul(physical) >= self.next_scaled;
        if !ready {
            return None;
        }
        let fired = Fired {
            ordinal: self.ordinal,
            tracer: self.ordinal.is_multiple_of(3),
        };
        self.pending -= 1;
        self.ordinal = self.ordinal.wrapping_add(1);
        self.next_scaled = self.next_scaled.saturating_add(round_span(weapon));
        Some(fired)
    }
}

/// One gun round in flight.
#[derive(Clone, Debug, PartialEq)]
pub struct Round {
    /// The weapon record it was fired from.
    pub weapon: String,
    pub tracer: bool,
    pub position: Vector,
    /// Where it was a tick ago.
    pub previous: Vector,
    pub direction: Vector,
    /// Feet per second in 1/256.
    pub speed_f8: i32,
    movement: Movement,
    flags: u32,
    fall: FallState,
    launched_t: u16,
}

impl Round {
    /// A round let go at the start of the step beginning at `tick`, from
    /// `muzzle` along `forward` (the aircraft's nose) with the aircraft
    /// flying at `speed_fps`. `seed` is the number the spread is drawn from:
    /// combat uses the round's projectile number, which only the host knows,
    /// so a client's spread differs from the host's within its cone (a
    /// quarter of a degree). The round has not moved yet: call
    /// [`Round::step`] with the same tick, as combat does.
    pub fn release(
        weapon: &Weapon,
        muzzle: Vector,
        forward: Vector,
        speed_fps: f64,
        seed: [u32; 3],
        tick: u64,
        tracer: bool,
    ) -> Option<Self> {
        let [id, owner, station] = seed;
        let direction = projectile_launch_direction(weapon, forward, id, owner, station as usize);
        let mut round = Self::aimed(weapon, muzzle, direction, speed_fps, tick)?;
        round.tracer = tracer;
        Some(round)
    }

    /// A round let go along exactly `direction`, with no spread: the centre of
    /// the gun's dispersion cone, which is where a pipper points. Otherwise as
    /// [`Round::release`].
    pub fn aimed(
        weapon: &Weapon,
        muzzle: Vector,
        direction: Vector,
        speed_fps: f64,
        tick: u64,
    ) -> Option<Self> {
        let speed = launch_speed(&weapon.movement, (speed_fps * 256.) as i32).ok()? * 256;
        Some(Self {
            weapon: weapon.source.clone(),
            tracer: false,
            position: muzzle,
            previous: muzzle,
            direction,
            speed_f8: speed,
            movement: weapon.movement,
            flags: weapon.flags,
            fall: FallState::default(),
            launched_t: (tick / 30) as u16,
        })
    }

    /// Whether the round's life is over at the step beginning at `tick`: the
    /// check [`Round::step`] makes first, before it moves the round.
    pub fn expired(&self, tick: u64) -> bool {
        removal_due(
            &self.movement,
            (tick / 30) as u16,
            self.launched_t,
            (self.position[1] * 256.) as i32,
        )
    }

    /// Flies the round one tick, the one starting at `tick`, over `ground`
    /// (height by x and z). `false` once its life is over, or it is in the
    /// ground.
    pub fn step(&mut self, tick: u64, ground: &impl Fn(f64, f64) -> f64) -> bool {
        let now = (tick / 30) as u16;
        let m = &self.movement;
        if self.expired(tick) {
            return false;
        }
        self.previous = self.position;
        let phase = engine_phase(m, now, self.launched_t);
        let service = service_ticks(tick);
        if self.flags & 0x40 != 0 {
            let target =
                commanded_speed(m, phase, self.speed_f8, (self.position[1] * 256.) as i32) as i16;
            let Ok(speed) = axial_speed(m, self.speed_f8, target, false, service) else {
                return false;
            };
            self.speed_f8 = speed;
        }
        let distance = f64::from(self.speed_f8) * f64::from(service) / 65536.;
        for i in 0..3 {
            self.position[i] += self.direction[i] * distance;
        }
        let Ok(height) = self.fall.advance(
            self.flags & 4 != 0,
            phase,
            service,
            (self.position[1] * 256.) as i32,
        ) else {
            return false;
        };
        self.position[1] = f64::from(height) / 256.;
        terrain_hit(self.previous, self.position, ground).is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_is_256_a_tick_shared_over_120_and_repeats_every_15_ticks() {
        // Combat keeps a running remainder; the cumulative sum is the same.
        let mut remainder = 0u16;
        for tick in 0..500 {
            remainder += 256;
            let service = (remainder / 120) as i16;
            remainder %= 120;
            assert_eq!(service_ticks(tick), service, "tick {tick}");
        }
        assert_eq!((0..15).map(service_ticks).sum::<i16>(), 32);
        assert_eq!(service_ticks(3), service_ticks(18));
    }

    #[test]
    fn rounds_leave_a_held_gun_as_often_as_its_burst_record_says() {
        let resources_weapon = |burst: (u8, u8, u8)| {
            let mut weapon = crate::combat::gunsight::tests::weapon();
            weapon.burst.game_rounds_in_burst = burst.0;
            weapon.burst.actual_rounds_per_game = burst.1;
            weapon.burst.game_burst_t = burst.2;
            weapon
        };
        // 4 x 3 rounds in a burst of 2 (60 ticks): a round every 5 ticks.
        let weapon = resources_weapon((4, 3, 2));
        assert_eq!(
            (0..5)
                .map(|n| release_tick(&weapon, 100, n))
                .collect::<Vec<_>>(),
            [100, 105, 110, 115, 120]
        );
        // 7 x 3 rounds in a burst of 1 (30 ticks): 30 / 21 ticks, so rounds
        // fall on the first tick at or after each, 100, 101.43, 102.86, ...
        let weapon = resources_weapon((7, 3, 1));
        assert_eq!(
            (0..5)
                .map(|n| release_tick(&weapon, 100, n))
                .collect::<Vec<_>>(),
            [100, 102, 103, 105, 106]
        );
    }
}
