//! The standby's side of host migration (stage K; docs/ARCHITECTURE.md,
//! "Standbys", "On the standby's side"): a state machine that takes the
//! standby stream's records in order ([`crate::journal::StreamReader`]),
//! builds the fresh world, assembles and restores checkpoints, replays ticks
//! with [`crate::journal::apply_tick`], keeps the state parts, checks the
//! hashes and reports, on a worker thread. Slice K0 places the module; slice
//! K2 builds it. A joined client passes the records on unread
//! ([`crate::Client::take_standby_records`]).
