//! The Own state section (host to client): the player's plane's exact state,
//! in its own packet beside a snapshot, coded against an exact state the
//! client has acknowledged (net-protocol.md, "The own aircraft").
//!
//! The section is the connection's flight (8 bits, protocol 3), the tick
//! of the state (32 bits), the state's number
//! (16 bits, counting the connection's own states), how many numbers back its
//! baseline is (5 bits, 1 to 31; 0 is none), then
//! [`ExactState`]'s bits and zero padding. *Agent decision:* the baseline is
//! named by own-state numbers, not by packets, since own states go out only
//! when needed, about once a second.

use super::{WireError, WireResult, bits};
use std::collections::VecDeque;
use tore_codec::{BitReader, BitWriter};
use tore_sim::models::AircraftModel;
use tore_world::world::plane::ExactState;

/// The most own states back a baseline may be.
pub const MAX_BACK: u16 = 31;
/// Own states the client keeps as baselines.
const KEPT: usize = 64;

/// The fields before the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnStateHeader {
    /// The connection's flight the state belongs to (protocol 3); see
    /// [`super::snapshot::SnapshotHeader::flight`].
    pub flight: u8,
    /// The host tick the state is at, after that tick's step.
    pub tick: u32,
    /// The state's number.
    pub number: u16,
    /// Own states back to the baseline, or 0 for none.
    pub back: u8,
}

impl OwnStateHeader {
    pub(crate) fn write(&self, w: &mut BitWriter) {
        let _ = w.write_bits(u64::from(self.flight), 8);
        let _ = w.write_bits(u64::from(self.tick), 32);
        let _ = w.write_bits(u64::from(self.number), 16);
        let _ = w.write_bits(u64::from(self.back), 5);
    }

    fn read(r: &mut BitReader<'_>) -> WireResult<Self> {
        Ok(Self {
            flight: r.read_bits(8)? as u8,
            tick: r.read_bits(32)? as u32,
            number: r.read_bits(16)? as u16,
            back: r.read_bits(5)? as u8,
        })
    }

    /// The header of a section, for a check that has no aircraft model.
    pub fn peek(bytes: &[u8]) -> WireResult<Self> {
        Self::read(&mut BitReader::new(bytes))
    }

    /// The number of the baseline, if the section has one.
    pub fn baseline(&self) -> Option<u16> {
        (self.back != 0).then(|| self.number.wrapping_sub(u16::from(self.back)))
    }
}

/// Writes a section: `state` at `header.tick` against `base`, which must be
/// the state numbered `header.baseline()` (or `None` when `back` is 0).
pub fn encode(
    header: &OwnStateHeader,
    state: &ExactState,
    base: Option<&ExactState>,
) -> WireResult<Vec<u8>> {
    if (header.back == 0) != base.is_none() || u16::from(header.back) > MAX_BACK {
        return Err(WireError::Invalid("own state baseline"));
    }
    let mut w = BitWriter::with_capacity(512);
    header.write(&mut w);
    state.write(&mut w, base)?;
    Ok(bits::finish(w))
}

/// Reads a section against `base`, the state its header names (see
/// [`OwnStateHeader::baseline`]), with the plane's aircraft `model`.
pub fn decode(
    bytes: &[u8],
    base: Option<&ExactState>,
    model: &AircraftModel,
) -> WireResult<(OwnStateHeader, ExactState)> {
    let mut r = BitReader::new(bytes);
    let header = OwnStateHeader::read(&mut r)?;
    if (header.back == 0) != base.is_none() {
        return Err(WireError::Invalid("own state baseline"));
    }
    let state = ExactState::read(&mut r, base, model)?;
    bits::end(&mut r)?;
    Ok((header, state))
}

/// The host's own-state bookkeeping for one connection.
#[derive(Clone, Debug, Default)]
pub struct OwnStateSender {
    /// The connection's flight its sections carry.
    pub flight: u8,
    next: u16,
    /// Acknowledged states, newest last.
    acked: VecDeque<(u16, ExactState)>,
    /// States sent and not yet heard of: sequence (none while staged),
    /// number and state.
    packets: VecDeque<(Option<u16>, u16, ExactState)>,
}

impl OwnStateSender {
    /// Nothing sent yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds the section for `state` at `tick`, against the newest
    /// acknowledged state no more than 31 numbers back. Staged like the
    /// snapshot's.
    pub fn build(&mut self, tick: u32, state: &ExactState) -> WireResult<Vec<u8>> {
        self.discard();
        let number = self.next;
        let base = self.acked.iter().rev().find(|(n, _)| {
            let back = number.wrapping_sub(*n);
            (1..=MAX_BACK).contains(&back)
        });
        let header = OwnStateHeader {
            flight: self.flight,
            tick,
            number,
            back: base.map_or(0, |(n, _)| number.wrapping_sub(*n) as u8),
        };
        let bytes = encode(&header, state, base.map(|(_, s)| s))?;
        self.next = self.next.wrapping_add(1);
        self.packets.push_back((None, number, state.clone()));
        Ok(bytes)
    }

    /// The staged section went out in the packet numbered `sequence`.
    pub fn sent(&mut self, sequence: u16) {
        if let Some(packet) = self.packets.back_mut()
            && packet.0.is_none()
        {
            packet.0 = Some(sequence);
        }
        while self.packets.len() > 64 {
            self.packets.pop_front();
        }
    }

    /// The staged section was not sent; its number is used again.
    pub fn discard(&mut self) {
        if self.packets.back().is_some_and(|p| p.0.is_none()) {
            self.packets.pop_back();
            self.next = self.next.wrapping_sub(1);
        }
    }

    /// The packet numbered `sequence` was delivered.
    pub fn delivered(&mut self, sequence: u16) {
        let Some(index) = self.packets.iter().position(|p| p.0 == Some(sequence)) else {
            return;
        };
        if let Some((_, number, state)) = self.packets.remove(index) {
            let at = self
                .acked
                .iter()
                .position(|(n, _)| number.wrapping_sub(*n) as i16 <= 0)
                .unwrap_or(self.acked.len());
            if self.acked.get(at).is_none_or(|(n, _)| *n != number) {
                self.acked.insert(at, (number, state));
            }
            while self.acked.len() > usize::from(MAX_BACK) {
                self.acked.pop_front();
            }
        }
    }

    /// The packet numbered `sequence` was lost.
    pub fn lost(&mut self, sequence: u16) {
        self.packets.retain(|p| p.0 != Some(sequence));
    }
}

/// The client's own states, kept as baselines.
#[derive(Clone, Debug, Default)]
pub struct OwnStateReceiver {
    states: VecDeque<(u16, ExactState)>,
}

impl OwnStateReceiver {
    /// None received yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The baseline a section needs, or an error when the client does not
    /// have it: the check a client makes before accepting the packet.
    pub fn baseline(&self, bytes: &[u8]) -> WireResult<Option<&ExactState>> {
        let header = OwnStateHeader::peek(bytes)?;
        match header.baseline() {
            None => Ok(None),
            Some(number) => self
                .states
                .iter()
                .find(|(n, _)| *n == number)
                .map(|(_, state)| Some(state))
                .ok_or(WireError::Invalid("own state baseline not received")),
        }
    }

    /// Reads a section and keeps its state as a baseline.
    pub fn receive(
        &mut self,
        bytes: &[u8],
        model: &AircraftModel,
    ) -> WireResult<(OwnStateHeader, ExactState)> {
        let base = self.baseline(bytes)?.cloned();
        let (header, state) = decode(bytes, base.as_ref(), model)?;
        if !self.states.iter().any(|(n, _)| *n == header.number) {
            self.states.push_back((header.number, state.clone()));
            while self.states.len() > KEPT {
                self.states.pop_front();
            }
        }
        Ok((header, state))
    }
}
