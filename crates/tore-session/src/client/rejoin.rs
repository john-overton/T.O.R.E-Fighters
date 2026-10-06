//! The client's side of rejoin (stage K; docs/ARCHITECTURE.md, "Rejoin
//! tokens and reservations"): the token the host grants, kept for the game's
//! token store, and the Rejoin a game sends when it joined without it.
//! Slice K0 places the seam; slice K5 builds it.

use super::Client;
use crate::wire::migration::TokenGrant;

impl Client {
    /// The host granted this player's token (message 39). Kept by slice K5.
    pub(super) fn rejoin_token(&mut self, grant: TokenGrant) {
        let _ = grant;
    }
}
