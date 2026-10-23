//! The operating system a player's game runs on (protocol 7).
//!
//! A joining game names its own in the Challenge answer; the host keeps it
//! per player and sends it in the lobby's player list, so a lobby can show a
//! Windows, macOS or Linux mark beside each callsign. It is shown, never
//! trusted: nothing in the session depends on it.

use std::fmt;

/// An operating system, as the wire codes it.
///
/// The codes are fixed: a new system takes the next free code and raises the
/// protocol version. The Challenge answer carries the code in a byte, the
/// lobby in 3 bits, so codes up to 7 fit both.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Platform {
    /// Not one the protocol names: a build for another system, or a player
    /// a test made up.
    #[default]
    Unknown = 0,
    /// Microsoft Windows.
    Windows = 1,
    /// Apple macOS.
    MacOs = 2,
    /// Linux.
    Linux = 3,
}

impl Platform {
    /// Every platform, in code order.
    pub const ALL: [Self; 4] = [Self::Unknown, Self::Windows, Self::MacOs, Self::Linux];

    /// The highest code the protocol names now.
    pub const MAX_CODE: u8 = 3;

    /// The platform this build was compiled for.
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Unknown
        }
    }

    /// The wire code.
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// The platform for a wire code, if the protocol names it.
    pub const fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Self::Unknown,
            1 => Self::Windows,
            2 => Self::MacOs,
            3 => Self::Linux,
            _ => return None,
        })
    }

    /// The system's name as a player reads it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Windows => "Windows",
            Self::MacOs => "macOS",
            Self::Linux => "Linux",
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_round_trips_and_the_rest_are_refused() {
        for platform in Platform::ALL {
            assert_eq!(Platform::from_code(platform.code()), Some(platform));
            assert!(platform.code() <= Platform::MAX_CODE);
        }
        for code in Platform::MAX_CODE + 1..=u8::MAX {
            assert_eq!(Platform::from_code(code), None);
        }
    }

    #[test]
    fn this_build_knows_its_own_system() {
        let expected = if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(target_os = "linux") {
            Platform::Linux
        } else {
            Platform::Unknown
        };
        assert_eq!(Platform::current(), expected);
        assert_eq!(Platform::default(), Platform::Unknown);
    }
}
