//! Exact checkpoints: every piece of mutable mission state coded bit for bit,
//! so a `World` restored from the bytes steps on exactly as the original
//! (docs/formats/checkpoint.md; stage H of docs/multiplayer-plan.md).
//!
//! This module holds what every coder shares:
//!
//! - [`Checkpoint`], a value coded exactly against an optional baseline of
//!   its own type, and [`InPlace`], for a holder that keeps its mission setup
//!   and restores only its mutable fields;
//! - [`Saver`] and [`Loader`], a bit writer and reader with the shared-record
//!   table ([`Saver::shared`]) and the world's flight [`Models`];
//! - the codings of the standard types: integers and floats by their bits,
//!   `bool`, `String`, `Option`, `Box`, arrays, tuples, `Vec`, `VecDeque`,
//!   `BTreeMap` and `BTreeSet`;
//! - the macros [`checkpoint_struct!`](crate::checkpoint_struct),
//!   [`checkpoint_enum!`](crate::checkpoint_enum) and
//!   [`checkpoint_tuple!`](crate::checkpoint_tuple), which name every field so
//!   a field added without coding it fails to compile;
//! - [`save_flight`] and [`load_flight`], a flight state through the wire's
//!   exact coder with its aircraft's identity and model;
//! - the leaf types more than one module's coder needs (`checkpoint_shared.rs`).
//!
//! A module's own coders live beside it in a child module named `checkpoint`
//! (the file `<module>_checkpoint.rs`), where they can read private fields.

use crate::flight::exact::ExactError;
use crate::models::AircraftModel;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use tore_codec::{BitReader, BitWriter, CodecError};
use tore_formats::aircraft::AircraftId;

#[path = "checkpoint_records.rs"]
mod records;
#[path = "checkpoint_shared.rs"]
mod shared;
#[cfg(test)]
#[path = "checkpoint_tests.rs"]
mod tests;

/// The most items one collection may claim, whatever the bytes say.
const MAX_ITEMS: usize = 1 << 24;

/// Why a value could not be coded or restored.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckpointError {
    /// The bytes are damaged or do not belong to this coder.
    Codec(CodecError),
    /// A flight state's exact coding refused (the native research adapter)
    /// or did not fit its aircraft type.
    Flight(ExactError),
    /// The named type's coder is not written yet (stage H's slices fill
    /// these in).
    NotCovered(&'static str),
    /// The bytes decode, but describe something this world cannot hold.
    Invalid(String),
}

impl From<CodecError> for CheckpointError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

impl From<ExactError> for CheckpointError {
    fn from(error: ExactError) -> Self {
        Self::Flight(error)
    }
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Codec(error) => write!(f, "checkpoint: {error}"),
            Self::Flight(error) => write!(f, "checkpoint: {error}"),
            Self::NotCovered(what) => write!(f, "checkpoint: {what} is not coded yet"),
            Self::Invalid(why) => write!(f, "checkpoint: {why}"),
        }
    }
}

impl std::error::Error for CheckpointError {}

/// The error of a coder that is not written yet: a stub returns it until its
/// slice fills it in.
pub fn not_covered<T>(what: &'static str) -> Result<T, CheckpointError> {
    Err(CheckpointError::NotCovered(what))
}

/// An invalid-value error with its reason.
pub fn invalid<T>(why: impl Into<String>) -> Result<T, CheckpointError> {
    Err(CheckpointError::Invalid(why.into()))
}

/// A value coded exactly, against an optional baseline: an earlier value of
/// the same field the reader also has. Checkpoints code against `None`
/// except where a coder chooses a baseline inside its own value.
pub trait Checkpoint: Sized {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError>;
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError>;
}

/// A holder that keeps its mission setup (imported data, the layout, the
/// fresh world's configuration) and restores only its mutable fields in
/// place. `restore_in_place` reads what `save_in_place` wrote.
pub trait InPlace {
    fn save_in_place(&self, s: &mut Saver) -> Result<(), CheckpointError>;
    fn restore_in_place(&mut self, l: &mut Loader<'_>) -> Result<(), CheckpointError>;
}

/// The distinct flight models a world holds, as the import built them
/// (before weight scaling), by aircraft identity. A flight state is coded as
/// its identity and its model's ordinal among that identity's models here;
/// a world built from an import has one model per identity, so the ordinal
/// is 0, but test fixtures can carry two. Both sides build the table from
/// the same mission, and the order within an identity is canonical (by a
/// fingerprint of the model), so the ordinals agree.
#[derive(Clone, Debug, Default)]
pub struct Models {
    models: Vec<(AircraftId, u64, AircraftModel)>,
}

impl Models {
    /// Adds `model` under `aircraft`, once.
    pub fn insert(
        &mut self,
        aircraft: AircraftId,
        model: AircraftModel,
    ) -> Result<(), CheckpointError> {
        if self
            .models
            .iter()
            .any(|(id, _, known)| *id == aircraft && *known == model)
        {
            return Ok(());
        }
        let fingerprint = tore_codec::fnv1a64(format!("{model:?}").as_bytes());
        if self
            .models
            .iter()
            .any(|(id, print, _)| *id == aircraft && *print == fingerprint)
        {
            return invalid(format!(
                "two flight models of the aircraft {aircraft:?} share a fingerprint"
            ));
        }
        self.models.push((aircraft, fingerprint, model));
        self.models
            .sort_by_key(|(id, print, _)| (*id as u8, *print));
        Ok(())
    }

    /// The ordinal of `model` among `aircraft`'s models.
    pub fn ordinal(&self, aircraft: AircraftId, model: &AircraftModel) -> Option<u32> {
        self.models
            .iter()
            .filter(|(id, _, _)| *id == aircraft)
            .position(|(_, _, known)| known == model)
            .and_then(|n| u32::try_from(n).ok())
    }

    /// `aircraft`'s model at `ordinal`.
    pub fn model(&self, aircraft: AircraftId, ordinal: u32) -> Option<&AircraftModel> {
        self.models
            .iter()
            .filter(|(id, _, _)| *id == aircraft)
            .nth(ordinal as usize)
            .map(|(_, _, model)| model)
    }

    pub fn len(&self) -> usize {
        self.models.len()
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}

/// The shared records written so far: each record's coding, and its index by
/// that coding, so a record equal to an earlier one costs only its index.
#[derive(Default)]
struct Table {
    bodies: Vec<Vec<u8>>,
    index: BTreeMap<Vec<u8>, u32>,
}

/// The writing side: a bit writer and the shared-record table, which every
/// section of one checkpoint shares.
pub struct Saver {
    w: BitWriter,
    table: Table,
    models: Models,
}

impl Default for Saver {
    fn default() -> Self {
        Self::new()
    }
}

impl Saver {
    pub fn new() -> Self {
        Self::with_models(Models::default())
    }

    /// A saver that can code flight states against the world's `models`.
    pub fn with_models(models: Models) -> Self {
        Self {
            w: BitWriter::new(),
            table: Table::default(),
            models,
        }
    }

    /// The bit writer, for a hand-written coder's own fields.
    pub fn writer(&mut self) -> &mut BitWriter {
        &mut self.w
    }

    /// Writes a collection's item count.
    pub fn count(&mut self, count: usize) {
        self.w.write_varint(count as u64);
    }

    /// Writes `value` as a shared record: coded once in the record table and
    /// referred to here by its index. Equal codings share one record.
    pub fn shared<T: Checkpoint>(&mut self, value: &T) -> Result<(), CheckpointError> {
        let mut inner = Saver {
            w: BitWriter::new(),
            table: std::mem::take(&mut self.table),
            models: std::mem::take(&mut self.models),
        };
        let saved = inner
            .w
            .write_bits(u64::from(type_tag::<T>()), 32)
            .map_err(CheckpointError::from)
            .and_then(|()| value.save(&mut inner, None));
        self.table = inner.table;
        self.models = inner.models;
        saved?;
        let body = inner.w.finish();
        let id = match self.table.index.get(&body) {
            Some(&id) => id,
            None => {
                let id = u32::try_from(self.table.bodies.len())
                    .map_err(|_| CheckpointError::Invalid("too many shared records".into()))?;
                self.table.bodies.push(body.clone());
                self.table.index.insert(body, id);
                id
            }
        };
        self.w.write_varint(u64::from(id));
        Ok(())
    }

    /// The bytes written since the last section ended, padded to a byte; the
    /// shared-record table carries on into the next section.
    pub fn finish_section(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.w).finish()
    }

    /// The shared records, in index order, once every section is written.
    pub fn into_records(self) -> Vec<Vec<u8>> {
        self.table.bodies
    }
}

/// The reading side: a bounded bit reader, the shared records and the fresh
/// world's flight models.
pub struct Loader<'a> {
    r: BitReader<'a>,
    records: &'a [Vec<u8>],
    /// Records this loader may refer to: all of them for a section, only
    /// earlier ones inside a record, so damaged bytes cannot recurse forever.
    limit: usize,
    models: &'a Models,
}

impl<'a> Loader<'a> {
    pub fn new(bytes: &'a [u8], records: &'a [Vec<u8>], models: &'a Models) -> Self {
        Self {
            r: BitReader::new(bytes),
            records,
            limit: records.len(),
            models,
        }
    }

    /// The bit reader, for a hand-written coder's own fields.
    pub fn reader(&mut self) -> &mut BitReader<'a> {
        &mut self.r
    }

    pub fn models(&self) -> &'a Models {
        self.models
    }

    /// Reads a collection's item count, refusing one that the bits left
    /// could not hold before anything is allocated.
    pub fn count(&mut self) -> Result<usize, CheckpointError> {
        let count = self.r.read_varint()?;
        let count = usize::try_from(count).map_err(|_| CodecError::ValueOutOfRange)?;
        if count > MAX_ITEMS || count > self.r.bits_remaining() {
            return Err(CodecError::ValueOutOfRange.into());
        }
        Ok(count)
    }

    /// Reads a value [`Saver::shared`] wrote.
    pub fn shared<T: Checkpoint>(&mut self) -> Result<T, CheckpointError> {
        let id = self.r.read_varint()?;
        let id = usize::try_from(id).map_err(|_| CodecError::ValueOutOfRange)?;
        if id >= self.limit {
            return invalid(format!("shared record {id} of {} referred to", self.limit));
        }
        let mut inner = Loader {
            r: BitReader::new(&self.records[id]),
            records: self.records,
            limit: id,
            models: self.models,
        };
        if inner.r.read_bits(32)? != u64::from(type_tag::<T>()) {
            return invalid(format!(
                "shared record {id} is not a {}",
                std::any::type_name::<T>()
            ));
        }
        let value = T::load(&mut inner, None)?;
        inner.finish()?;
        Ok(value)
    }

    /// Checks that everything was read: only zero padding may be left.
    pub fn finish(self) -> Result<(), CheckpointError> {
        if self.r.only_zero_padding_left() {
            Ok(())
        } else {
            invalid(format!(
                "{} bits left unread at the end",
                self.r.bits_remaining()
            ))
        }
    }
}

/// A shared record's type tag: FNV-1a of the type's name, so a record read as
/// another type is refused. Checkpoints never cross builds, so the compiler's
/// naming is stable enough.
fn type_tag<T>() -> u32 {
    tore_codec::fnv1a64(std::any::type_name::<T>().as_bytes()) as u32
}

/// A value and its shared records, coded on their own: what a coder's tests
/// round-trip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coded {
    pub body: Vec<u8>,
    pub records: Vec<Vec<u8>>,
}

/// Codes `value` with no baseline.
pub fn to_bytes<T: Checkpoint>(value: &T, models: &Models) -> Result<Coded, CheckpointError> {
    let mut s = Saver::with_models(models.clone());
    value.save(&mut s, None)?;
    let body = s.finish_section();
    Ok(Coded {
        body,
        records: s.into_records(),
    })
}

/// Decodes what [`to_bytes`] wrote, requiring every bit to be read.
pub fn from_bytes<T: Checkpoint>(coded: &Coded, models: &Models) -> Result<T, CheckpointError> {
    let mut l = Loader::new(&coded.body, &coded.records, models);
    let value = T::load(&mut l, None)?;
    l.finish()?;
    Ok(value)
}

/// Codes `value`, decodes a copy and codes the copy again: the copy, when it
/// codes to the same bytes. Types with `PartialEq` should also compare it.
pub fn round_trip<T: Checkpoint>(value: &T, models: &Models) -> Result<T, CheckpointError> {
    let coded = to_bytes(value, models)?;
    let copy: T = from_bytes(&coded, models)?;
    let again = to_bytes(&copy, models)?;
    if again != coded {
        return invalid(format!(
            "a {} decoded from its coding codes differently",
            std::any::type_name::<T>()
        ));
    }
    Ok(copy)
}

/// [`round_trip`] for a holder restored in place: saves `value`, restores the
/// bytes into `fresh` (a holder built from the same setup) and saves `fresh`
/// again, which must give the same bytes.
pub fn round_trip_in_place<T: InPlace>(
    value: &T,
    fresh: &mut T,
    models: &Models,
) -> Result<(), CheckpointError> {
    let mut s = Saver::with_models(models.clone());
    value.save_in_place(&mut s)?;
    let body = s.finish_section();
    let coded = Coded {
        body,
        records: s.into_records(),
    };
    let mut l = Loader::new(&coded.body, &coded.records, models);
    fresh.restore_in_place(&mut l)?;
    l.finish()?;
    let mut s = Saver::with_models(models.clone());
    fresh.save_in_place(&mut s)?;
    let body = s.finish_section();
    if body != coded.body || s.into_records() != coded.records {
        return invalid(format!(
            "a {} restored from its coding codes differently",
            std::any::type_name::<T>()
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Flight states.

/// Codes a flight state as its aircraft's identity, its model's ordinal
/// among the world's models of that identity (see [`Models`]) and the wire's
/// exact coding with no baseline (docs/formats/checkpoint.md, "Flight
/// states"). The write-only trace is not coded; the native research adapter
/// is refused.
pub fn save_flight(
    s: &mut Saver,
    flight: &crate::flight::State,
    aircraft: AircraftId,
) -> Result<(), CheckpointError> {
    let Some(ordinal) = s.models.ordinal(aircraft, &flight.import_model()) else {
        return invalid(format!(
            "a flight of the aircraft {aircraft:?} flies a model this world does not hold"
        ));
    };
    aircraft.save(s, None)?;
    s.w.write_varint(u64::from(ordinal));
    flight.write_exact(&mut s.w, None)?;
    Ok(())
}

/// Reads what [`save_flight`] wrote, with the flight model from the
/// loader's models.
pub fn load_flight(
    l: &mut Loader<'_>,
) -> Result<(AircraftId, crate::flight::State), CheckpointError> {
    let aircraft = AircraftId::load(l, None)?;
    let ordinal = u32::try_from(l.r.read_varint()?).map_err(|_| CodecError::ValueOutOfRange)?;
    let Some(model) = l.models.model(aircraft, ordinal) else {
        return invalid(format!(
            "no flight model {ordinal} of the aircraft {aircraft:?} in this world"
        ));
    };
    let flight = crate::flight::State::read_exact(&mut l.r, None, model)?;
    Ok((aircraft, flight))
}

// ---------------------------------------------------------------------------
// Macros.

/// Implements [`Checkpoint`] for a struct by coding each field in turn. The
/// lists must name every field, since the writer destructures the struct and
/// the reader builds it, neither with `..`:
///
/// ```ignore
/// checkpoint_struct!(Ownship { hp, ammo } shared { config } skip { scratch = Vec::new() });
/// ```
///
/// Fields in the first list are coded against the baseline's; `shared`
/// fields are coded as shared records ([`Saver::shared`]); `skip` fields are
/// not coded and are rebuilt by their expression. Every skipped field needs a
/// comment at the macro naming its class (why-record, scratch with its proof,
/// setup or local).
#[macro_export]
macro_rules! checkpoint_struct {
    (
        $ty:ident { $($field:ident),* $(,)? }
        $(shared { $($shared:ident),* $(,)? })?
        $(skip { $($skip:ident = $rebuild:expr),* $(,)? })?
    ) => {
        impl $crate::checkpoint::Checkpoint for $ty {
            #[allow(unused_variables)]
            fn save(
                &self,
                s: &mut $crate::checkpoint::Saver,
                base: ::core::option::Option<&Self>,
            ) -> ::core::result::Result<(), $crate::checkpoint::CheckpointError> {
                let $ty { $($field,)* $($($shared,)*)? $($($skip: _,)*)? } = self;
                $( $crate::checkpoint::Checkpoint::save($field, s, base.map(|b| &b.$field))?; )*
                $($( s.shared($shared)?; )*)?
                ::core::result::Result::Ok(())
            }
            #[allow(unused_variables)]
            fn load(
                l: &mut $crate::checkpoint::Loader<'_>,
                base: ::core::option::Option<&Self>,
            ) -> ::core::result::Result<Self, $crate::checkpoint::CheckpointError> {
                $( let $field = $crate::checkpoint::Checkpoint::load(l, base.map(|b| &b.$field))?; )*
                $($( let $shared = l.shared()?; )*)?
                ::core::result::Result::Ok($ty {
                    $($field,)*
                    $($($shared,)*)?
                    $($($skip: $rebuild,)*)?
                })
            }
        }
    };
}

/// Implements [`Checkpoint`] for a field-less enum by each variant's fixed
/// number, a varint. The list must name every variant: the writer's `match`
/// has no catch-all, so a new variant fails to compile.
#[macro_export]
macro_rules! checkpoint_enum {
    ($ty:path { $($variant:ident = $n:literal),* $(,)? }) => {
        impl $crate::checkpoint::Checkpoint for $ty {
            fn save(
                &self,
                s: &mut $crate::checkpoint::Saver,
                _: ::core::option::Option<&Self>,
            ) -> ::core::result::Result<(), $crate::checkpoint::CheckpointError> {
                type This = $ty;
                let number: u64 = match self {
                    $( This::$variant => $n, )*
                };
                s.writer().write_varint(number);
                ::core::result::Result::Ok(())
            }
            fn load(
                l: &mut $crate::checkpoint::Loader<'_>,
                _: ::core::option::Option<&Self>,
            ) -> ::core::result::Result<Self, $crate::checkpoint::CheckpointError> {
                type This = $ty;
                match l.reader().read_varint()? {
                    $( $n => ::core::result::Result::Ok(This::$variant), )*
                    other => $crate::checkpoint::invalid(::std::format!(
                        "{} has no variant {other}",
                        ::core::stringify!($ty)
                    )),
                }
            }
        }
    };
}

/// Implements [`Checkpoint`] for a tuple struct by coding each field in turn,
/// with no baseline. The names are only bindings, one per field, so a field
/// added fails to compile: `checkpoint_tuple!(PlaneId(id));`.
#[macro_export]
macro_rules! checkpoint_tuple {
    ($ty:ident ( $($field:ident),* $(,)? )) => {
        impl $crate::checkpoint::Checkpoint for $ty {
            fn save(
                &self,
                s: &mut $crate::checkpoint::Saver,
                _: ::core::option::Option<&Self>,
            ) -> ::core::result::Result<(), $crate::checkpoint::CheckpointError> {
                let $ty($($field),*) = self;
                $( $crate::checkpoint::Checkpoint::save($field, s, ::core::option::Option::None)?; )*
                ::core::result::Result::Ok(())
            }
            fn load(
                l: &mut $crate::checkpoint::Loader<'_>,
                _: ::core::option::Option<&Self>,
            ) -> ::core::result::Result<Self, $crate::checkpoint::CheckpointError> {
                $( let $field = $crate::checkpoint::Checkpoint::load(l, ::core::option::Option::None)?; )*
                ::core::result::Result::Ok($ty($($field),*))
            }
        }
    };
}

/// Implements [`Checkpoint`] for types that have an
/// [`Exact`](crate::flight::exact::Exact) coder already (the flight state's
/// parts, turbulence, cheats), by that coder: one coder to keep complete.
macro_rules! checkpoint_via_exact {
    ($($ty:ty),* $(,)?) => {$(
        impl $crate::checkpoint::Checkpoint for $ty {
            fn save(
                &self,
                s: &mut $crate::checkpoint::Saver,
                base: Option<&Self>,
            ) -> Result<(), $crate::checkpoint::CheckpointError> {
                $crate::flight::exact::Exact::write(self, s.writer(), base)?;
                Ok(())
            }
            fn load(
                l: &mut $crate::checkpoint::Loader<'_>,
                base: Option<&Self>,
            ) -> Result<Self, $crate::checkpoint::CheckpointError> {
                Ok(<$ty as $crate::flight::exact::Exact>::read(l.reader(), base)?)
            }
        }
    )*};
}
pub(crate) use checkpoint_via_exact;

// ---------------------------------------------------------------------------
// The standard types.

impl Checkpoint for bool {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        s.w.write_bool(*self);
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(l.r.read_bool()?)
    }
}

/// Integers are coded as their 64-bit pattern (sign-extended) against the
/// baseline's, as the exact own-plane coder does; one that does not fit its
/// type on reading is an error.
macro_rules! checkpoint_int {
    ($($ty:ty),*) => {$(
        impl Checkpoint for $ty {
            fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
                s.w.write_u64_xor(*self as i64 as u64, base.map_or(0, |b| *b as i64 as u64));
                Ok(())
            }
            fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
                let bits = l.r.read_u64_xor(base.map_or(0, |b| *b as i64 as u64))?;
                Ok(<$ty>::try_from(bits as i64).map_err(|_| CodecError::ValueOutOfRange)?)
            }
        }
    )*};
}
checkpoint_int!(i8, i16, i32, i64, u8, u16, u32);

/// Unsigned 64-bit values as they are; a `usize` as a 64-bit value that must
/// fit this platform on reading (32-bit Windows is a CI target).
macro_rules! checkpoint_wide {
    ($($ty:ty),*) => {$(
        impl Checkpoint for $ty {
            fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
                s.w.write_u64_xor(*self as u64, base.map_or(0, |b| *b as u64));
                Ok(())
            }
            fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
                let bits = l.r.read_u64_xor(base.map_or(0, |b| *b as u64))?;
                Ok(<$ty>::try_from(bits).map_err(|_| CodecError::ValueOutOfRange)?)
            }
        }
    )*};
}
checkpoint_wide!(u64, usize);

impl Checkpoint for f64 {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        s.w.write_f64_xor(*self, base.copied().unwrap_or(0.));
        Ok(())
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(l.r.read_f64_xor(base.copied().unwrap_or(0.))?)
    }
}

impl Checkpoint for f32 {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        s.w.write_u64_xor(
            u64::from(self.to_bits()),
            base.map_or(0, |b| u64::from(b.to_bits())),
        );
        Ok(())
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        let bits =
            l.r.read_u64_xor(base.map_or(0, |b| u64::from(b.to_bits())))?;
        Ok(f32::from_bits(
            u32::try_from(bits).map_err(|_| CodecError::ValueOutOfRange)?,
        ))
    }
}

/// A byte count and UTF-8 bytes, with no length cap beyond the bits left
/// (composed radio text can pass 255 bytes). Never against a baseline.
impl Checkpoint for String {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        s.count(self.len());
        s.w.write_bytes(self.as_bytes());
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let len = l.count()?;
        let bytes = l.r.read_bytes(len)?;
        Ok(String::from_utf8(bytes).map_err(|_| CodecError::InvalidUtf8)?)
    }
}

impl Checkpoint for () {
    fn save(&self, _: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        Ok(())
    }
    fn load(_: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(())
    }
}

/// One bit for presence, then the value against the baseline's value when
/// the baseline has one.
impl<T: Checkpoint> Checkpoint for Option<T> {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        s.w.write_bool(self.is_some());
        match self {
            Some(value) => value.save(s, base.and_then(Option::as_ref)),
            None => Ok(()),
        }
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        if l.r.read_bool()? {
            Ok(Some(T::load(l, base.and_then(Option::as_ref))?))
        } else {
            Ok(None)
        }
    }
}

impl<T: Checkpoint> Checkpoint for Box<T> {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        (**self).save(s, base.map(|b| &**b))
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(Box::new(T::load(l, base.map(|b| &**b))?))
    }
}

impl<T: Checkpoint, const N: usize> Checkpoint for [T; N] {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        for (index, value) in self.iter().enumerate() {
            value.save(s, base.map(|b| &b[index]))?;
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        let mut values = Vec::with_capacity(N);
        for index in 0..N {
            values.push(T::load(l, base.map(|b| &b[index]))?);
        }
        values
            .try_into()
            .map_err(|_| CodecError::ValueOutOfRange.into())
    }
}

macro_rules! checkpoint_tuple_impl {
    ($(($($name:ident $index:tt),+)),*) => {$(
        impl<$($name: Checkpoint),+> Checkpoint for ($($name,)+) {
            fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
                $( self.$index.save(s, base.map(|b| &b.$index))?; )+
                Ok(())
            }
            fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
                Ok(($( $name::load(l, base.map(|b| &b.$index))?, )+))
            }
        }
    )*};
}
checkpoint_tuple_impl!(
    (A 0),
    (A 0, B 1),
    (A 0, B 1, C 2),
    (A 0, B 1, C 2, D 3),
    (A 0, B 1, C 2, D 3, E 4)
);

/// A count, then each item against the baseline's item at the same index.
impl<T: Checkpoint> Checkpoint for Vec<T> {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        s.count(self.len());
        for (index, value) in self.iter().enumerate() {
            value.save(s, base.and_then(|b| b.get(index)))?;
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        let count = l.count()?;
        let mut values = Vec::with_capacity(count);
        for index in 0..count {
            values.push(T::load(l, base.and_then(|b| b.get(index)))?);
        }
        Ok(values)
    }
}

impl<T: Checkpoint> Checkpoint for VecDeque<T> {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        s.count(self.len());
        for (index, value) in self.iter().enumerate() {
            value.save(s, base.and_then(|b| b.get(index)))?;
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        let count = l.count()?;
        let mut values = VecDeque::with_capacity(count);
        for index in 0..count {
            values.push_back(T::load(l, base.and_then(|b| b.get(index)))?);
        }
        Ok(values)
    }
}

/// A count, then each key (no baseline) and its value against the baseline
/// map's value at the same key. Keys must arrive strictly ascending, as the
/// writer writes them.
impl<K: Checkpoint + Ord, V: Checkpoint> Checkpoint for BTreeMap<K, V> {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        s.count(self.len());
        for (key, value) in self {
            key.save(s, None)?;
            value.save(s, base.and_then(|b| b.get(key)))?;
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        let count = l.count()?;
        let mut map = BTreeMap::new();
        for _ in 0..count {
            let key = K::load(l, None)?;
            if map.last_key_value().is_some_and(|(last, _)| *last >= key) {
                return invalid("map keys out of order");
            }
            let value = V::load(l, base.and_then(|b| b.get(&key)))?;
            map.insert(key, value);
        }
        Ok(map)
    }
}

impl<T: Checkpoint + Ord> Checkpoint for BTreeSet<T> {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        s.count(self.len());
        for value in self {
            value.save(s, None)?;
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let count = l.count()?;
        let mut set = BTreeSet::new();
        for _ in 0..count {
            let value = T::load(l, None)?;
            if set.last().is_some_and(|last| *last >= value) {
                return invalid("set items out of order");
            }
            set.insert(value);
        }
        Ok(set)
    }
}
