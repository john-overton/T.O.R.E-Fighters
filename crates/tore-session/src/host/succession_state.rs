//! The *candidates* state part (stage K, slice K6; docs/ARCHITECTURE.md,
//! "What moves with the host"): each player's Candidate report and
//! measures by join order, the address the host sees it at, the house's
//! flight figure and the pinned player's callsign, so a new host punches
//! every player and appoints standbys at once.
//!
//! Coded whole with the checkpoint trait: a field added to a part below
//! fails to compile in its `checkpoint_struct!` until it is coded. Connection
//! ids never appear; players are their join orders. A part's bytes are
//! read by the same build only, like a checkpoint.

use super::super::Host;
use super::{Measure, Upload};
use crate::wire::Platform;
use crate::wire::migration::{CandidateReport, Processor};
use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use tore_net::master::candidate::{Candidate, CandidateKind, MappingType};
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Models, Saver, invalid};
use tore_sim::checkpoint_struct;

/// An address: its octets (4 or 16) and port.
#[derive(Clone, Debug, PartialEq, Eq)]
struct AddressPart {
    octets: Vec<u8>,
    port: u16,
}
checkpoint_struct!(AddressPart { octets, port });

impl AddressPart {
    fn of(address: SocketAddr) -> Self {
        let octets = match address.ip() {
            IpAddr::V4(v4) => v4.octets().to_vec(),
            IpAddr::V6(v6) => v6.octets().to_vec(),
        };
        Self {
            octets,
            port: address.port(),
        }
    }

    fn address(&self) -> Result<SocketAddr, CheckpointError> {
        let ip = match self.octets.len() {
            4 => IpAddr::from(<[u8; 4]>::try_from(self.octets.as_slice()).expect("4 octets")),
            16 => IpAddr::from(<[u8; 16]>::try_from(self.octets.as_slice()).expect("16 octets")),
            n => return invalid(format!("an address of {n} octets")),
        };
        Ok(SocketAddr::new(ip, self.port))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CandidatePart {
    kind: u8,
    address: AddressPart,
}
checkpoint_struct!(CandidatePart { kind, address });

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReportPart {
    may_host: bool,
    platform: u8,
    processor: u8,
    candidates: Vec<CandidatePart>,
    mapping: u8,
    cpu_micros: u32,
    cpu_mission: u32,
}
checkpoint_struct!(ReportPart {
    may_host,
    platform,
    processor,
    candidates,
    mapping,
    cpu_micros,
    cpu_mission,
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UploadPart {
    players: u16,
    per_mille: u16,
}
checkpoint_struct!(UploadPart { players, per_mille });

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlayerPart {
    order: u64,
    seen: AddressPart,
    report: Option<ReportPart>,
    reached_by: BTreeMap<u64, Option<u16>>,
    upload: Option<UploadPart>,
}
checkpoint_struct!(PlayerPart {
    order,
    seen,
    report,
    reached_by,
    upload,
});

/// The whole part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::host) struct CandidatesPart {
    players: Vec<PlayerPart>,
    house_upload: Option<UploadPart>,
    /// The pinned player's callsign, for the fallback's words.
    pinned: Option<String>,
}
checkpoint_struct!(CandidatesPart {
    players,
    house_upload,
    pinned,
});

impl From<Upload> for UploadPart {
    fn from(upload: Upload) -> Self {
        Self {
            players: upload.players,
            per_mille: upload.per_mille,
        }
    }
}

impl From<UploadPart> for Upload {
    fn from(part: UploadPart) -> Self {
        Self {
            players: part.players,
            per_mille: part.per_mille,
        }
    }
}

impl ReportPart {
    fn of(report: &CandidateReport) -> Self {
        Self {
            may_host: report.may_host,
            platform: report.platform.code(),
            processor: report.processor.code(),
            candidates: report
                .candidates
                .iter()
                .map(|c| CandidatePart {
                    kind: c.kind.code(),
                    address: AddressPart::of(c.address),
                })
                .collect(),
            mapping: report.mapping.code(),
            cpu_micros: report.cpu_micros,
            cpu_mission: report.cpu_mission,
        }
    }

    fn report(&self) -> Result<CandidateReport, CheckpointError> {
        let bad = |what: &str| CheckpointError::Invalid(format!("a candidate's {what}"));
        let candidates = self
            .candidates
            .iter()
            .map(|c| {
                Ok(Candidate::new(
                    CandidateKind::from_code(u64::from(c.kind)).ok_or_else(|| bad("kind"))?,
                    c.address.address()?,
                ))
            })
            .collect::<Result<_, CheckpointError>>()?;
        Ok(CandidateReport {
            may_host: self.may_host,
            platform: Platform::from_code(self.platform).ok_or_else(|| bad("platform"))?,
            processor: Processor::from_code(self.processor).ok_or_else(|| bad("processor"))?,
            candidates,
            mapping: MappingType::from_code(u64::from(self.mapping))
                .ok_or_else(|| bad("mapping"))?,
            cpu_micros: self.cpu_micros,
            cpu_mission: self.cpu_mission,
        })
    }
}

/// A restored part: what a new host's selection starts from.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::host) struct Restored {
    /// By join order: the address the old host saw the player at, and its
    /// measures.
    pub(in crate::host) players: BTreeMap<u64, (SocketAddr, Measure)>,
    pub(in crate::host) house_upload: Option<Upload>,
    pub(in crate::host) pinned: Option<String>,
}

// Slices K3 (the State record) and K4 (`Host::resume`) call these; until
// they land only the tests do.
#[allow(dead_code)]
impl CandidatesPart {
    /// The part's bytes, as a State record carries them.
    pub(in crate::host) fn encode(&self) -> Result<Vec<u8>, CheckpointError> {
        let mut s = Saver::new();
        self.save(&mut s, None)?;
        Ok(s.finish_section())
    }

    /// Reads what [`CandidatesPart::encode`] wrote, every bit.
    pub(in crate::host) fn decode(bytes: &[u8]) -> Result<Self, CheckpointError> {
        let models = Models::default();
        let mut l = Loader::new(bytes, &[], &models);
        let part = Self::load(&mut l, None)?;
        l.finish()?;
        Ok(part)
    }

    /// The part as a new host's selection takes it.
    pub(in crate::host) fn restore(&self) -> Result<Restored, CheckpointError> {
        let mut players = BTreeMap::new();
        for player in &self.players {
            let measure = Measure {
                report: player.report.as_ref().map(ReportPart::report).transpose()?,
                reached_by: player.reached_by.clone(),
                upload: player.upload.map(Upload::from),
            };
            players.insert(player.order, (player.seen.address()?, measure));
        }
        Ok(Restored {
            players,
            house_upload: self.house_upload.map(Upload::from),
            pinned: self.pinned.clone(),
        })
    }
}

#[allow(dead_code)]
impl Host {
    /// The candidates state part as it stands (stage K): every connected
    /// player's report and measures by join order (slice K3 sends it after
    /// the tick in which [`Host::take_candidates_changed`] says it changed).
    pub(in crate::host) fn candidates_part(&self) -> CandidatesPart {
        let players = self
            .live_peers()
            .map(|(_, peer)| {
                let measure = self.succession.measures.get(&peer.lobby.order);
                PlayerPart {
                    order: peer.lobby.order,
                    seen: AddressPart::of(peer.address),
                    report: measure.and_then(|m| m.report.as_ref()).map(ReportPart::of),
                    reached_by: measure.map(|m| m.reached_by.clone()).unwrap_or_default(),
                    upload: measure.and_then(|m| m.upload).map(UploadPart::from),
                }
            })
            .collect();
        CandidatesPart {
            players,
            house_upload: self.succession.house_upload.map(UploadPart::from),
            pinned: self.succession.pinned.as_ref().map(|(_, c)| c.clone()),
        }
    }

    /// Takes a restored part's measures (slice K4's `Host::resume`): the
    /// reports, reaches and uploads by join order, and the house's figure.
    pub(in crate::host) fn restore_candidates(&mut self, restored: &Restored) {
        self.succession.measures = restored
            .players
            .iter()
            .map(|(order, (_, measure))| (*order, measure.clone()))
            .collect();
        self.succession.house_upload = restored.house_upload;
        self.succession.changed = true;
    }
}
