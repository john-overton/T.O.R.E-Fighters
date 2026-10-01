//! The game as a client of a dedicated server (slice D8b of stage D): the
//! command line that joins one, the files a networked flight keeps, and the
//! debrief it shows from the report the host sends. The session itself,
//! prediction included, is `tore_session::Client`; see docs/ARCHITECTURE.md,
//! "Network sessions" and docs/DEDICATED-SERVER.md, "Joining from the game".
// Nothing here is called until the client session is wired in (D8b); the
// tests run it all.
#![allow(dead_code)]

pub mod files;
pub mod options;
