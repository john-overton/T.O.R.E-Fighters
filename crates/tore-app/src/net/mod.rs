//! The game as a client of a dedicated server (slice D8b of stage D): the
//! command line that joins one, the files a networked flight keeps, and the
//! debrief it shows from the report the host sends. The session itself,
//! prediction included, is `tore_session::Client`; see docs/ARCHITECTURE.md,
//! "Network sessions" and docs/DEDICATED-SERVER.md, "Joining from the game".
//! The game also hosts a session itself, on a thread, and joins it as a
//! client (slice EF3, `hosting`). Slice EF5 adds finding games and servers
//! without a screen: `search` (the local network), `lookup` (a typed address,
//! off the screen's thread) and `settings` (what is remembered). Slice I4 adds
//! `browse` (the Internet Lobby's list, from the master) and `telemetry` (the
//! anonymous statistics a game that uses the master may send). Stage F
//! phase 2 adds `away` (the AI flies an idle player's aircraft); stage K adds
//! `standby` (a joined game's standby and the migration lines).
pub mod away;
pub mod browse;
pub mod chat;
pub mod debrief;
pub mod files;
pub mod guns;
pub mod hosting;
pub mod lobby_chat;
pub mod lookup;
pub mod observe;
pub mod options;
pub mod play;
pub mod scoreboard;
pub mod search;
pub mod session;
pub mod settings;
pub mod standby;
pub mod telemetry;
