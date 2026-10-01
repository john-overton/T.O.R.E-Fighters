//! The game as a client of a dedicated server (slice D8b of stage D): the
//! command line that joins one, the files a networked flight keeps, and the
//! debrief it shows from the report the host sends. The session itself,
//! prediction included, is `tore_session::Client`; see docs/ARCHITECTURE.md,
//! "Network sessions" and docs/DEDICATED-SERVER.md, "Joining from the game".
//! The game also hosts a session itself, on a thread, and joins it as a
//! client (slice EF3, `hosting`).
pub mod debrief;
pub mod files;
pub mod guns;
pub mod hosting;
pub mod options;
pub mod play;
pub mod session;
