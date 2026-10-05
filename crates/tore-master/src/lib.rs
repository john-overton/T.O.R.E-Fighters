//! `tore-master`: the master server behind the game's Internet Lobby.
//!
//! It keeps the list of listed games, answers the Internet Lobby's browse
//! and details requests, answers the mapping test's probes on its two ports
//! and counts anonymous telemetry. It introduces a joining player to a host
//! (slice J2, [`introduce`]). The relay (slice J3) has its dispatch here
//! already and drops its packets, counted, until that slice builds it. Standard library only; it never
//! reads Fighters Anthology's data.
//!
//! - [`master::Master`] is the server as a state machine that never reads a
//!   clock or touches a socket, so the tests run it on the network simulator.
//! - [`run::Running`] is the real loop on two UDP ports; [`config`] reads
//!   its configuration file; [`flood`] is the load tool.
//!
//! The wire is `tore_net::master`
//! ([`docs/formats/master-protocol.md`](../../../docs/formats/master-protocol.md));
//! running it is [`docs/MASTER-SERVER.md`](../../../docs/MASTER-SERVER.md); the
//! design is the architecture guide's "Master server and connectivity".

pub mod browse;
pub mod config;
pub mod flood;
pub mod introduce;
pub mod limits;
pub mod listings;
pub mod log;
pub mod master;
pub mod probe;
pub mod relay;
pub mod run;
pub mod stats;
pub mod telemetry;

pub use config::Config;
pub use master::{Counters, Master, MasterPort, Outgoing, Settings};

#[cfg(test)]
mod introduce_tests;
#[cfg(test)]
mod master_tests;
