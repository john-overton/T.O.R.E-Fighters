//! The exact coder of a flight state: every field the next step reads, private
//! ones included, coded bit for bit so a copy decoded from it steps exactly as
//! the original (docs/formats/net-protocol.md, "The own aircraft").
//!
//! Each value is coded against an optional baseline of the same type, an
//! earlier state the reader also has: 64-bit values as the exclusive-or with
//! the baseline's (`tore_codec`'s `write_u64_xor`), so an unchanged field costs
//! one bit. With no baseline everything is coded against zero. Every struct's
//! coder names each of its fields, with no `..`, through [`exact_struct`], so a
//! field added to a state without coding it fails to compile.
//!
//! Not coded: the second crew member's chute (`crew_escape`, which a
//! client's prediction never needs; a checkpoint codes it after this coding,
//! `checkpoint::save_flight`), the write-only trace, which equality ignores
//! and the next step rewrites, the display-only gun poses populated in draw
//! clones (the combat ownship owns their authoritative state), and the
//! imported tables (the aircraft model and its raw envelope polygons), which
//! the decoder takes from the caller. The weight-scaled envelopes are rebuilt
//! from those tables and the coded scale exactly as the step builds them. The
//! native research adapter is refused.

use crate::flight::State;
use crate::models::FlightModel;
use tore_codec::{BitReader, BitWriter, CodecError};

/// A value coded exactly against an optional baseline of its own type.
pub trait Exact: Sized {
    fn write(&self, w: &mut BitWriter, base: Option<&Self>) -> Result<(), CodecError>;
    fn read(r: &mut BitReader<'_>, base: Option<&Self>) -> Result<Self, CodecError>;
}

/// Implements [`Exact`] for a struct by coding each named field in turn. The
/// field list must name every field: the writer destructures the struct and
/// the reader builds it, neither with `..`.
macro_rules! exact_struct {
    ($ty:ident { $($field:ident),* $(,)? }) => {
        impl $crate::flight::exact::Exact for $ty {
            fn write(
                &self,
                w: &mut tore_codec::BitWriter,
                base: Option<&Self>,
            ) -> ::core::result::Result<(), tore_codec::CodecError> {
                let $ty { $($field),* } = self;
                $( $crate::flight::exact::Exact::write($field, w, base.map(|b| &b.$field))?; )*
                ::core::result::Result::Ok(())
            }
            fn read(
                r: &mut tore_codec::BitReader<'_>,
                base: Option<&Self>,
            ) -> ::core::result::Result<Self, tore_codec::CodecError> {
                ::core::result::Result::Ok($ty {
                    $( $field: $crate::flight::exact::Exact::read(r, base.map(|b| &b.$field))?, )*
                })
            }
        }
    };
}
pub(crate) use exact_struct;

/// Implements [`Exact`] for a field-less enum by its variant's number, coded
/// as a 64-bit value against the baseline's. An unknown number is an error.
macro_rules! exact_enum {
    ($ty:path { $($variant:ident = $n:literal),* $(,)? }) => {
        impl $crate::flight::exact::Exact for $ty {
            fn write(
                &self,
                w: &mut tore_codec::BitWriter,
                base: Option<&Self>,
            ) -> ::core::result::Result<(), tore_codec::CodecError> {
                type This = $ty;
                let number = |value: &This| -> u64 {
                    match value {
                        $( This::$variant => $n, )*
                    }
                };
                w.write_u64_xor(number(self), base.map_or(0, number));
                ::core::result::Result::Ok(())
            }
            fn read(
                r: &mut tore_codec::BitReader<'_>,
                base: Option<&Self>,
            ) -> ::core::result::Result<Self, tore_codec::CodecError> {
                type This = $ty;
                let number = |value: &This| -> u64 {
                    match value {
                        $( This::$variant => $n, )*
                    }
                };
                match r.read_u64_xor(base.map_or(0, number))? {
                    $( $n => ::core::result::Result::Ok(This::$variant), )*
                    _ => ::core::result::Result::Err(tore_codec::CodecError::ValueOutOfRange),
                }
            }
        }
    };
}
pub(crate) use exact_enum;

impl Exact for f64 {
    fn write(&self, w: &mut BitWriter, base: Option<&Self>) -> Result<(), CodecError> {
        w.write_f64_xor(*self, base.copied().unwrap_or(0.));
        Ok(())
    }
    fn read(r: &mut BitReader<'_>, base: Option<&Self>) -> Result<Self, CodecError> {
        r.read_f64_xor(base.copied().unwrap_or(0.))
    }
}

impl Exact for bool {
    fn write(&self, w: &mut BitWriter, _: Option<&Self>) -> Result<(), CodecError> {
        w.write_bool(*self);
        Ok(())
    }
    fn read(r: &mut BitReader<'_>, _: Option<&Self>) -> Result<Self, CodecError> {
        r.read_bool()
    }
}

/// Integers are coded as their 64-bit pattern (sign-extended) against the
/// baseline's; one that does not fit its type on reading is an error.
macro_rules! exact_int {
    ($($ty:ty),*) => {$(
        impl Exact for $ty {
            fn write(&self, w: &mut BitWriter, base: Option<&Self>) -> Result<(), CodecError> {
                w.write_u64_xor(*self as i64 as u64, base.map_or(0, |b| *b as i64 as u64));
                Ok(())
            }
            fn read(r: &mut BitReader<'_>, base: Option<&Self>) -> Result<Self, CodecError> {
                let bits = r.read_u64_xor(base.map_or(0, |b| *b as i64 as u64))?;
                <$ty>::try_from(bits as i64).map_err(|_| CodecError::ValueOutOfRange)
            }
        }
    )*};
}
exact_int!(i8, i16, i32, i64, u8, u16, u32);

/// Unsigned 64-bit values are coded as they are, all 64 bits.
macro_rules! exact_wide {
    ($($ty:ty),*) => {$(
        impl Exact for $ty {
            fn write(&self, w: &mut BitWriter, base: Option<&Self>) -> Result<(), CodecError> {
                w.write_u64_xor(*self as u64, base.map_or(0, |b| *b as u64));
                Ok(())
            }
            fn read(r: &mut BitReader<'_>, base: Option<&Self>) -> Result<Self, CodecError> {
                let bits = r.read_u64_xor(base.map_or(0, |b| *b as u64))?;
                <$ty>::try_from(bits).map_err(|_| CodecError::ValueOutOfRange)
            }
        }
    )*};
}
exact_wide!(u64, usize);

impl<T: Exact, const N: usize> Exact for [T; N] {
    fn write(&self, w: &mut BitWriter, base: Option<&Self>) -> Result<(), CodecError> {
        for (index, value) in self.iter().enumerate() {
            value.write(w, base.map(|b| &b[index]))?;
        }
        Ok(())
    }
    fn read(r: &mut BitReader<'_>, base: Option<&Self>) -> Result<Self, CodecError> {
        let mut values = Vec::with_capacity(N);
        for index in 0..N {
            values.push(T::read(r, base.map(|b| &b[index]))?);
        }
        values.try_into().map_err(|_| CodecError::ValueOutOfRange)
    }
}

/// One bit for presence, then the value against the baseline's value when the
/// baseline has one.
impl<T: Exact> Exact for Option<T> {
    fn write(&self, w: &mut BitWriter, base: Option<&Self>) -> Result<(), CodecError> {
        w.write_bool(self.is_some());
        match self {
            Some(value) => value.write(w, base.and_then(Option::as_ref)),
            None => Ok(()),
        }
    }
    fn read(r: &mut BitReader<'_>, base: Option<&Self>) -> Result<Self, CodecError> {
        if r.read_bool()? {
            Ok(Some(T::read(r, base.and_then(Option::as_ref))?))
        } else {
            Ok(None)
        }
    }
}

/// The systems messages still queued: a count and each line, never against a
/// baseline. The queue holds at most 64.
impl Exact for Vec<String> {
    fn write(&self, w: &mut BitWriter, _: Option<&Self>) -> Result<(), CodecError> {
        w.write_varint(self.len() as u64);
        for text in self {
            w.write_str(text)?;
        }
        Ok(())
    }
    fn read(r: &mut BitReader<'_>, _: Option<&Self>) -> Result<Self, CodecError> {
        let count = r.read_varint()?;
        if count > crate::aircraft_systems::MAX_MESSAGES as u64 {
            return Err(CodecError::ValueOutOfRange);
        }
        (0..count).map(|_| r.read_str()).collect()
    }
}

impl Exact for tore_formats::flight_model::clock_rng::NativeRng {
    fn write(&self, w: &mut BitWriter, base: Option<&Self>) -> Result<(), CodecError> {
        let (seed, shuffle, table) = self.raw_parts();
        let base = base.map(Self::raw_parts);
        seed.write(w, base.as_ref().map(|b| &b.0))?;
        shuffle.write(w, base.as_ref().map(|b| &b.1))?;
        table.write(w, base.as_ref().map(|b| &b.2))
    }
    fn read(r: &mut BitReader<'_>, base: Option<&Self>) -> Result<Self, CodecError> {
        let base = base.map(Self::raw_parts);
        let seed = i32::read(r, base.as_ref().map(|b| &b.0))?;
        let shuffle = i32::read(r, base.as_ref().map(|b| &b.1))?;
        let table = <[i32; 32]>::read(r, base.as_ref().map(|b| &b.2))?;
        Ok(Self::from_raw_parts(seed, shuffle, table))
    }
}

impl Exact for tore_formats::flight_model::clock_rng::FixedClock {
    fn write(&self, w: &mut BitWriter, base: Option<&Self>) -> Result<(), CodecError> {
        self.remainder()
            .write(w, base.map(|b| b.remainder()).as_ref())
    }
    fn read(r: &mut BitReader<'_>, base: Option<&Self>) -> Result<Self, CodecError> {
        let remainder = u16::read(r, base.map(|b| b.remainder()).as_ref())?;
        Self::from_remainder(remainder).ok_or(CodecError::ValueOutOfRange)
    }
}

use tore_formats::flight_model::departure::{DepartureMode, StallState};
exact_enum!(DepartureMode {
    Normal = 0,
    Warning = 1,
    Stalled = 2,
    Spinning = 3,
    ExtendedWarning = 4,
});
exact_struct!(StallState { mode, elapsed });

use crate::telemetry::Maneuver;
exact_struct!(Maneuver {
    tick,
    commanded_g,
    lift_g,
    achieved_g,
    body_rates_rad_per_second,
    rudder_command,
    rudder_deflection,
    effective_rudder,
    departure,
    stall_severity_f8,
});

use crate::cheats::{Cheats, Damage};
exact_enum!(Damage {
    Invulnerable = 0,
    Normal = 1,
    Realistic = 2,
});
exact_enum!(crate::ai::Experience {
    Novice = 0,
    Average = 1,
    Experienced = 2,
    Ace = 3,
});
exact_struct!(Cheats {
    damage,
    unlimited_ammo,
    unlimited_fuel,
    no_spins,
    no_turbulence,
    extra_g,
    ignore_weapon_weights,
    no_sun_whiteout,
    no_g_effects,
    no_screen_shake,
    no_crashes,
    easy_aiming,
    ignore_midair_collisions,
    easy_targeting,
    guns_only,
    enemy_ai,
});

// The powered-lift stability level; the rest of the powered-lift state is
// coded beside its types in `flight/powered/state.rs`.
exact_enum!(tore_input::StabilityLevel {
    Off = 0,
    Damper = 1,
    Attitude = 2,
});

use crate::sensors::{Channel, Controls};
exact_enum!(Channel {
    Radar = 0,
    Infrared = 1,
    Visual = 2,
});
exact_struct!(Controls {
    channel,
    range_index,
    history,
});

// Combat's broken section, which a plane's ownship terms carry.
exact_enum!(crate::combat::live::DamageSection {
    Nose = 0,
    Cockpit = 1,
    Core = 2,
    LeftWing = 3,
    RightWing = 4,
    Tail = 5,
});

/// Why a flight state cannot be coded or rebuilt.
#[derive(Debug, Clone, PartialEq)]
pub enum ExactError {
    /// The bytes are damaged, or were written against another baseline.
    Codec(CodecError),
    /// The native research adapter's state is not coded; it is refused.
    Native,
    /// The aircraft type given to the decoder cannot carry the coded
    /// weight-scaled envelopes: it is not the type the state was coded from.
    Configuration(String),
}

impl From<CodecError> for ExactError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

impl std::fmt::Display for ExactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "exact flight state: {error}"),
            Self::Native => write!(f, "the native research adapter's state is not coded"),
            Self::Configuration(error) => {
                write!(
                    f,
                    "exact flight state does not fit its aircraft type: {error}"
                )
            }
        }
    }
}

impl std::error::Error for ExactError {}

impl State {
    /// Codes this state exactly against `base`, an earlier state of the same
    /// aircraft the reader also has, or against zero. Refuses the native
    /// research adapter.
    pub fn write_exact(&self, w: &mut BitWriter, base: Option<&State>) -> Result<(), ExactError> {
        let State {
            // The aircraft type comes from the reader's own import.
            model: _,
            research,
            raw_envelopes,
            envelope_scale,
            native,
            position,
            yaw,
            pitch,
            bank,
            speed,
            velocity,
            roll_rate,
            pitch_rate,
            auxiliary_rates,
            lift_controls,
            gun_aim: _,
            gun_group: _,
            vertical_speed,
            g,
            maneuver,
            lift_g,
            throttle,
            fuel,
            systems,
            payload_lbs,
            engine,
            burner,
            exhaust,
            rudder,
            elevator,
            aileron,
            gear,
            flaps,
            brake,
            hook,
            bay,
            bay_open,
            bay_auto_open,
            gear_down,
            flaps_down,
            brake_out,
            hook_down,
            radar,
            jammer,
            sensors,
            damage_fraction,
            damage_variant,
            damage_regions,
            autopilot,
            escape,
            eject_armed_at,
            crashed,
            wreck,
            ticks,
            overspeed_ticks,
            failure_rng,
            cheats,
            jolt,
            // The second crew member's chute: a client's prediction never
            // needs it (it draws it from the host's pilot entities, slice
            // B5), so the wire leaves it out. It is world state all the same,
            // so a checkpoint codes it after this coding, in
            // `checkpoint::save_flight` (slice B6, docs/ARCHITECTURE.md).
            crew_escape: _,
            // Write-only, outside equality; the next step rewrites it.
            trace: _,
        } = self;
        if native.is_some() {
            return Err(ExactError::Native);
        }
        macro_rules! put {
            ($($field:ident),*) => {$(
                $field.write(w, base.map(|b| &b.$field))?;
            )*};
        }
        // The raw polygons are the aircraft type's own; only whether the
        // step has scaled them yet is coded.
        w.write_bool(raw_envelopes.is_some());
        put!(
            research,
            envelope_scale,
            position,
            yaw,
            pitch,
            bank,
            speed,
            velocity,
            roll_rate,
            pitch_rate,
            auxiliary_rates,
            lift_controls,
            vertical_speed,
            g,
            maneuver,
            lift_g,
            throttle,
            fuel,
            systems,
            payload_lbs,
            engine,
            burner,
            exhaust,
            rudder,
            elevator,
            aileron,
            gear,
            flaps,
            brake,
            hook,
            bay,
            bay_open,
            bay_auto_open,
            gear_down,
            flaps_down,
            brake_out,
            hook_down,
            radar,
            jammer,
            sensors,
            damage_fraction,
            damage_variant,
            damage_regions,
            autopilot,
            escape,
            eject_armed_at,
            crashed,
            wreck,
            ticks,
            overspeed_ticks,
            failure_rng,
            cheats,
            jolt
        );
        Ok(())
    }

    /// The flight model as the import built it, before the weight scaling:
    /// what [`Self::read_exact`] takes. A checkpoint's restore collects these
    /// from the fresh world (docs/formats/checkpoint.md, "Restoring").
    pub fn import_model(&self) -> crate::models::AircraftModel {
        let mut model = self.model.clone();
        if let Some(raw) = &self.raw_envelopes {
            let mut configuration = model.configuration().clone();
            configuration.aerodynamics.envelopes = raw.as_ref().clone();
            // The raw polygons are the import's own and passed validation
            // when it built the model.
            model
                .set_configuration(configuration)
                .expect("the imported envelopes are valid");
        }
        model
    }

    /// Reads a state [`Self::write_exact`] wrote against the same `base`.
    /// `model` is the aircraft type's flight model as the import builds it,
    /// before any weight scaling: the decoder rebuilds the scaled envelopes
    /// from it and the coded scale exactly as the step does.
    pub fn read_exact(
        r: &mut BitReader<'_>,
        base: Option<&State>,
        model: &crate::models::AircraftModel,
    ) -> Result<State, ExactError> {
        macro_rules! get {
            ($field:ident) => {
                Exact::read(r, base.map(|b| &b.$field))?
            };
        }
        let scaled = r.read_bool()?;
        let mut state = State {
            model: model.clone(),
            research: get!(research),
            raw_envelopes: None,
            envelope_scale: get!(envelope_scale),
            native: None,
            position: get!(position),
            yaw: get!(yaw),
            pitch: get!(pitch),
            bank: get!(bank),
            speed: get!(speed),
            velocity: get!(velocity),
            roll_rate: get!(roll_rate),
            pitch_rate: get!(pitch_rate),
            auxiliary_rates: get!(auxiliary_rates),
            lift_controls: get!(lift_controls),
            gun_aim: [[0.; 2]; 3],
            gun_group: 0,
            vertical_speed: get!(vertical_speed),
            g: get!(g),
            maneuver: get!(maneuver),
            lift_g: get!(lift_g),
            throttle: get!(throttle),
            fuel: get!(fuel),
            systems: get!(systems),
            payload_lbs: get!(payload_lbs),
            engine: get!(engine),
            burner: get!(burner),
            exhaust: get!(exhaust),
            rudder: get!(rudder),
            elevator: get!(elevator),
            aileron: get!(aileron),
            gear: get!(gear),
            flaps: get!(flaps),
            brake: get!(brake),
            hook: get!(hook),
            bay: get!(bay),
            bay_open: get!(bay_open),
            bay_auto_open: get!(bay_auto_open),
            gear_down: get!(gear_down),
            flaps_down: get!(flaps_down),
            brake_out: get!(brake_out),
            hook_down: get!(hook_down),
            radar: get!(radar),
            jammer: get!(jammer),
            sensors: get!(sensors),
            damage_fraction: get!(damage_fraction),
            damage_variant: get!(damage_variant),
            damage_regions: get!(damage_regions),
            autopilot: get!(autopilot),
            escape: get!(escape),
            eject_armed_at: get!(eject_armed_at),
            crashed: get!(crashed),
            wreck: get!(wreck),
            ticks: get!(ticks),
            overspeed_ticks: get!(overspeed_ticks),
            failure_rng: get!(failure_rng),
            cheats: get!(cheats),
            jolt: get!(jolt),
            crew_escape: None,
            trace: Default::default(),
        };
        if scaled {
            let raw = std::sync::Arc::new(model.configuration().aerodynamics.envelopes.clone());
            let configuration = state.scaled_configuration(&raw, state.envelope_scale);
            state
                .model
                .set_configuration(configuration)
                .map_err(|error| ExactError::Configuration(error.to_string()))?;
            state.raw_envelopes = Some(raw);
        }
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight::integration_tests::profile;
    use crate::models::AircraftModel;
    use crate::research::Surface;
    use tore_input::{PilotCommand, PilotInput, Switch};

    /// The synthetic aircraft, with an ejection seat.
    fn model() -> AircraftModel {
        let mut model = AircraftModel::for_aircraft(&profile()).unwrap();
        let mut configuration = model.configuration().clone();
        configuration.ejection_seat = true;
        model.set_configuration(configuration).unwrap();
        model
    }

    fn encode(state: &State, base: Option<&State>) -> Vec<u8> {
        let mut w = BitWriter::new();
        state.write_exact(&mut w, base).unwrap();
        w.finish()
    }

    fn decode(bytes: &[u8], base: Option<&State>) -> Result<State, ExactError> {
        State::read_exact(&mut BitReader::new(bytes), base, &model())
    }

    fn runway(_: f64, _: f64) -> Surface {
        Surface::runway(0.)
    }

    fn step(state: &mut State, ticks: usize, input: impl Fn(usize) -> PilotInput) {
        for tick in 0..ticks {
            state.step_surface(&input(tick), runway);
        }
    }

    /// Codes `state` against `base`, decodes it, and checks that the copy
    /// equals it, codes to the same bytes, and flies the next two seconds
    /// exactly as the original does. Returns the encoding's size in bytes.
    fn round_trip(state: &State, base: Option<&State>) -> usize {
        let bytes = encode(state, base);
        let copy = decode(&bytes, base).unwrap();
        assert_eq!(&copy, state);
        assert_eq!(encode(&copy, None), encode(state, None));
        let (mut a, mut b) = (state.clone(), copy);
        let input = |tick: usize| PilotInput {
            pitch: if tick < 60 { 0.3 } else { 0. },
            roll: if (60..120).contains(&tick) { -0.4 } else { 0. },
            ..PilotInput::default()
        };
        for tick in 0..240 {
            a.step_surface(&input(tick), runway);
            b.step_surface(&input(tick), runway);
            assert_eq!(a, b, "tick {tick} after the round trip");
            assert_eq!(encode(&a, None), encode(&b, None), "tick {tick}");
        }
        bytes.len()
    }

    fn airborne() -> State {
        let mut s = State::from_model(model(), [0., 5000., 0.]);
        s.enable_research(7).unwrap();
        s.payload_lbs = 800.;
        s.cheats.no_spins = true;
        s.sensors.range_index = 3;
        step(&mut s, 900, |tick| {
            let mut input = PilotInput {
                pitch: if (100..200).contains(&tick) { 0.5 } else { 0. },
                roll: if (300..360).contains(&tick) { 0.6 } else { 0. },
                ..PilotInput::default()
            };
            if tick == 10 {
                input.commands.push(PilotCommand::Set(Switch::Burner, true));
            }
            if tick == 400 {
                input.commands.push(PilotCommand::Toggle(Switch::Autopilot));
            }
            input
        });
        s
    }

    #[test]
    fn a_state_on_the_ground_round_trips() {
        let mut s = State::from_model(model(), [0., 0., 0.]);
        s.enable_research(3).unwrap();
        s.start_on_runway([100., 0., -200.], 0.4).unwrap();
        step(&mut s, 120, |tick| PilotInput {
            throttle: Some(if tick < 60 { 0.2 } else { 0.8 }),
            ..PilotInput::default()
        });
        assert!(s.weight_on_wheels());
        round_trip(&s, None);
    }

    #[test]
    fn an_airborne_state_round_trips_with_its_scaled_envelopes() {
        let s = airborne();
        assert!(
            s.raw_envelopes.is_some(),
            "the weight never scaled the envelopes"
        );
        assert!(s.stall_scale() != 1.);
        assert_eq!(s.autopilot.mode(), crate::autopilot::Mode::Heading);
        round_trip(&s, None);
    }

    #[test]
    fn a_damaged_state_round_trips() {
        let mut s = airborne();
        for index in [3, 12, 21, 30, 38] {
            s.systems.hit(index, s.throttle);
        }
        s.systems.report_impact(s.ticks, 0.4);
        s.damage_fraction = 0.4;
        s.damage_variant = Some(2);
        s.damage_regions = [0., 0.1, 0.35, 0., 0.2, 0.];
        s.jolt_from([s.position[0] + 50., s.position[1], s.position[2]], 1.);
        assert!(!s.systems.messages.is_empty());
        round_trip(&s, None);
        step(&mut s, 60, |_| PilotInput::default());
        round_trip(&s, None);
    }

    #[test]
    fn overspeed_countdown_and_rng_survive_exact_snapshot_restore() {
        for research in [false, true] {
            let mut s = State::from_model(model(), [0., 10000., 0.]);
            if research {
                s.enable_research(19).unwrap();
            }
            s.speed = s.speed / s.overspeed_ratio().unwrap() * 1.1;
            for _ in 0..719 {
                s.check_overspeed();
            }
            let mut copy = decode(&encode(&s, None), None).unwrap();
            for _ in 0..481 {
                s.check_overspeed();
                copy.check_overspeed();
                assert_eq!(s, copy);
            }
            assert!(s.crashed);
        }
    }

    #[test]
    fn a_wreck_round_trips() {
        let mut s = airborne();
        s.crashed = true;
        step(&mut s, 90, |_| PilotInput::default());
        assert!(s.wreck.is_some());
        round_trip(&s, None);
    }

    #[test]
    fn a_state_after_ejection_round_trips() {
        let mut s = airborne();
        for tick in 0..240 {
            let mut input = PilotInput::default();
            if tick < 2 {
                input.commands.push(PilotCommand::Eject);
            }
            s.step_surface(&input, runway);
        }
        assert!(s.escape.is_some(), "the pilot never ejected");
        round_trip(&s, None);
    }

    /// The acknowledged baseline one snapshot (4 ticks) back: the size an
    /// airborne plane's exact state costs on the wire.
    #[test]
    fn an_airborne_state_against_one_snapshot_back_is_small() {
        let mut s = airborne();
        let mut sizes = Vec::new();
        for _ in 0..30 {
            let base = s.clone();
            step(&mut s, 4, |_| PilotInput {
                roll: 0.2,
                pitch: 0.1,
                ..PilotInput::default()
            });
            sizes.push(round_trip(&s, Some(&base)));
        }
        let full = encode(&s, None).len();
        let mean = sizes.iter().sum::<usize>() as f64 / sizes.len() as f64;
        eprintln!(
            "exact flight state: {full} bytes with no baseline; against 4 ticks back {} to {} \
             bytes, mean {mean:.0}",
            sizes.iter().min().unwrap(),
            sizes.iter().max().unwrap(),
        );
        assert!(
            mean < full as f64,
            "a baseline saves nothing: {mean} of {full}"
        );
        assert!(mean < 400., "{mean} bytes against 4 ticks back");
    }

    #[test]
    fn the_native_adapter_is_refused() {
        let mut s = airborne();
        // Tables of the right size; their values do not matter here.
        let sine = vec![0; 321 * 2];
        let atan = vec![0; 514 * 2];
        let tables = crate::native::Tables::parse(&sine, &atan).unwrap();
        s.native = Some(crate::native::Native::new(std::sync::Arc::new(tables), 1).unwrap());
        let mut w = BitWriter::new();
        assert_eq!(s.write_exact(&mut w, None), Err(ExactError::Native));
    }

    #[test]
    fn a_state_decoded_against_the_wrong_baseline_differs_and_damaged_bytes_never_panic() {
        let s = airborne();
        let mut base = s.clone();
        step(&mut base, 4, |_| PilotInput::default());
        let bytes = encode(&s, Some(&base));
        // Against no baseline the same bytes read as something else, or fail.
        assert!(decode(&bytes, None).map_or(true, |copy| copy != s));
        // Truncated, flipped and random bytes are errors, never panics.
        for cut in 0..bytes.len() {
            let _ = decode(&bytes[..cut], Some(&base));
        }
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..2000 {
            let mut damaged = bytes.clone();
            let at = next() as usize % damaged.len();
            damaged[at] ^= 1 << (next() % 8);
            let _ = decode(&damaged, Some(&base));
            let random: Vec<u8> = (0..(next() % 600)).map(|_| next() as u8).collect();
            let _ = decode(&random, None);
            let _ = decode(&random, Some(&base));
        }
    }
}
