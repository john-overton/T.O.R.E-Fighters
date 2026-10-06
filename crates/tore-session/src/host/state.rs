//! The session's state parts (stage K; docs/ARCHITECTURE.md, "What moves
//! with the host"): players, session, court, scores, revivals, rejoin,
//! candidates and listing, each coded whole in a child module beside the
//! state it codes and sent in a State record after the tick in which it
//! changed ([`crate::journal::Part`]). Slice K0 places the module; slice K1
//! fills it.
