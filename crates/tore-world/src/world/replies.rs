//! A human wingman's replies and requests to its flight: the four kinds of
//! [`crate::seats::SeatCommand::WingReply`]. Stage F phase 2; see
//! docs/ARCHITECTURE.md, "Orders to human wingmen, and their replies".
//!
//! Slice F2-0 adds the type the wire and the keys share. The call to the
//! flight and its refusals are slice F2-R's: until it lands, the step does
//! nothing with a reply.

/// What a wingman tells its flight. The wire codes it in 2 bits, in this
/// order (docs/formats/net-protocol.md, "Changed messages").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reply {
    /// "Engaging" (Alt+Shift+E, retail recording `^ENGAGE`).
    Engaging,
    /// "Winchester": out of weapons (Alt+Shift+W, text only: no retail
    /// recording says it).
    Winchester,
    /// "Bingo fuel" (Alt+Shift+B, `^BINGO`).
    BingoFuel,
    /// "Need help" (Alt+Shift+H, the retail recording whose phrase asks for
    /// help).
    NeedHelp,
}

impl Reply {
    /// Every kind, in wire order.
    pub const ALL: [Reply; 4] = [
        Reply::Engaging,
        Reply::Winchester,
        Reply::BingoFuel,
        Reply::NeedHelp,
    ];

    /// The line a flight's humans read, after the speaker's place
    /// ("Two: Winchester").
    pub fn text(self) -> &'static str {
        match self {
            Reply::Engaging => "Engaging",
            Reply::Winchester => "Winchester",
            Reply::BingoFuel => "Bingo fuel",
            Reply::NeedHelp => "Need help",
        }
    }
}
