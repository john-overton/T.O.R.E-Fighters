//! The command line. The options are the guide's table (docs/DEDICATED-SERVER.md,
//! "Running it").

use std::path::PathBuf;

/// What the command line asked for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub config: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub import: Option<PathBuf>,
    pub port: Option<u16>,
    pub mission: Option<PathBuf>,
    pub check: bool,
    /// The game's developer switch, which a server refuses.
    pub retail_stall_speeds: bool,
    /// AI test setting: how often AI aircraft observe, in hertz (120, 60,
    /// 40, 30 or 20); `None` is today's 120.
    pub ai_observe_hz: Option<u32>,
    /// AI test setting: whether AI aircraft skip the visual channel and build
    /// the received-radar list only when a warning needs it; `None` is the
    /// default, on.
    pub ai_lean_sensors: Option<bool>,
    pub help: bool,
    pub version: bool,
}

/// The usage text.
pub const USAGE: &str = "\
Usage: tore-server [options]

  --config FILE        The configuration file (default: server.conf in the data folder)
  --data-dir DIR       The data folder holding the import and the logs
                       (default: the game's own; TORE_DATA_DIR also sets it)
  --import FOLDER      Import Fighters Anthology from an installed game or disc folder, then exit
  --port N             Listen on this UDP port instead of the configuration file's
  --mission FILE       Fly this mission file instead of the configuration file's
  --check              Load the import and the mission, print its aircraft, the theater's
                       runways and the content manifest, then exit without opening the port
  --ai-observe-hz N    Test setting: AI aircraft observe N times a second, 120 (default),
                       60, 40, 30 or 20, staggered across ticks; the flight stays at 120
  --ai-lean-sensors [on|off]
                       Test setting: AI aircraft skip the visual channel and build the
                       received-radar list only when a warning needs it (default: on)
  --version            Print the version and exit
  --help               Print this text and exit

The configuration file, the mission file and the console commands are described in
docs/DEDICATED-SERVER.md.";

impl Options {
    /// The AI test settings the command line asked for: the defaults (120 Hz,
    /// lean sensors) without the flags.
    pub fn ai_test(&self) -> tore_session::AiTestSettings {
        tore_session::AiTestSettings::from_flags(self.ai_observe_hz, self.ai_lean_sensors)
            .unwrap_or_default()
    }

    /// Reads the arguments after the program name.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut args = args.into_iter().peekable();
        while let Some(arg) = args.next() {
            let (flag, inline) = match arg.split_once('=') {
                Some((flag, value)) if flag.starts_with("--") => {
                    (flag.to_owned(), Some(value.to_owned()))
                }
                _ => (arg.clone(), None),
            };
            let mut value = |what: &str| -> Result<String, String> {
                inline
                    .clone()
                    .or_else(|| args.next())
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| format!("{flag} needs {what}"))
            };
            match flag.as_str() {
                "--config" => options.config = Some(PathBuf::from(value("a file")?)),
                "--data-dir" => options.data_dir = Some(PathBuf::from(value("a folder")?)),
                "--import" => {
                    options.import = Some(PathBuf::from(value(
                        "an installed game folder or a disc folder",
                    )?))
                }
                "--port" => {
                    let text = value("a port number")?;
                    options.port = Some(
                        text.parse::<u16>()
                            .ok()
                            .filter(|port| *port != 0)
                            .ok_or_else(|| {
                                format!("--port must be a number from 1 to 65535, not `{text}`")
                            })?,
                    );
                }
                "--mission" => options.mission = Some(PathBuf::from(value("a file")?)),
                "--check" => options.check = true,
                "--retail-stall-speeds" => options.retail_stall_speeds = true,
                "--ai-observe-hz" => {
                    let text = value("a rate: 120, 60, 40, 30 or 20")?;
                    options.ai_observe_hz = Some(
                        text.parse::<u32>()
                            .ok()
                            .filter(|hz| {
                                tore_session::AiTestSettings::default()
                                    .with_observation_hz(*hz)
                                    .is_some()
                            })
                            .ok_or_else(|| {
                                format!(
                                    "--ai-observe-hz must be 120, 60, 40, 30 or 20, not `{text}`"
                                )
                            })?,
                    );
                }
                "--ai-lean-sensors" => {
                    // Bare means on; `=on`, `=off`, or `on` or `off` as the
                    // next argument say so.
                    let word = match &inline {
                        Some(text) => Some(text.clone()),
                        None => args.next_if(|next| matches!(next.as_str(), "on" | "off")),
                    };
                    options.ai_lean_sensors = Some(match word {
                        None => true,
                        Some(text) => tore_session::AiTestSettings::parse_on_off(&text)
                            .ok_or_else(|| {
                                format!("--ai-lean-sensors must be on or off, not `{text}`")
                            })?,
                    });
                }
                "--help" | "-h" => options.help = true,
                "--version" | "-V" => options.version = true,
                _ => return Err(format!("unknown option `{arg}`; --help lists the options")),
            }
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn the_ai_test_flags_set_the_rate_and_lean_sensors() {
        let none = parse(&[]).unwrap();
        assert!(none.ai_test().is_default());
        let slow = parse(&["--ai-observe-hz", "30", "--ai-lean-sensors"]).unwrap();
        assert_eq!(slow.ai_test().observation_hz(), 30);
        assert!(slow.ai_test().lean_sensors);
        for (args, lean) in [
            (&["--ai-lean-sensors", "on"][..], true),
            (&["--ai-lean-sensors", "off"][..], false),
            (&["--ai-lean-sensors=off"][..], false),
            (&["--ai-lean-sensors=on"][..], true),
        ] {
            assert_eq!(parse(args).unwrap().ai_test().lean_sensors, lean);
        }
        // A bare flag leaves the next option alone.
        let bare = parse(&["--ai-lean-sensors", "--check"]).unwrap();
        assert!(bare.ai_test().lean_sensors && bare.check);
        let off = parse(&["--ai-lean-sensors", "off", "--check"]).unwrap();
        assert!(!off.ai_test().lean_sensors && off.check);
        assert!(!off.ai_test().is_default());
        assert!(
            parse(&["--ai-lean-sensors=maybe"])
                .unwrap_err()
                .contains("on or off")
        );
        assert_eq!(
            parse(&["--ai-observe-hz=20"]).unwrap().ai_test().divisor(),
            6
        );
        assert!(
            parse(&["--ai-observe-hz", "25"])
                .unwrap_err()
                .contains("`25`")
        );
        assert!(
            parse(&["--ai-observe-hz"])
                .unwrap_err()
                .contains("needs a rate")
        );
    }

    #[test]
    fn no_arguments_is_the_default() {
        assert_eq!(parse(&[]).unwrap(), Options::default());
    }

    #[test]
    fn every_option_of_the_guide_parses() {
        let options = parse(&[
            "--config",
            "a.conf",
            "--data-dir",
            "/data",
            "--port",
            "27000",
            "--mission=m.txt",
            "--check",
        ])
        .unwrap();
        assert_eq!(options.config, Some("a.conf".into()));
        assert_eq!(options.data_dir, Some("/data".into()));
        assert_eq!(options.port, Some(27000));
        assert_eq!(options.mission, Some("m.txt".into()));
        assert!(options.check);
        assert_eq!(
            parse(&["--import", "/games/FIGHTERS"]).unwrap().import,
            Some("/games/FIGHTERS".into())
        );
    }

    #[test]
    fn the_retail_stall_switch_is_read_so_it_can_be_refused() {
        assert!(
            parse(&["--retail-stall-speeds"])
                .unwrap()
                .retail_stall_speeds
        );
    }

    #[test]
    fn bad_arguments_are_refused_plainly() {
        assert!(
            parse(&["--port"])
                .unwrap_err()
                .contains("needs a port number")
        );
        assert!(parse(&["--port", "0"]).unwrap_err().contains("1 to 65535"));
        assert!(
            parse(&["--port", "99999"])
                .unwrap_err()
                .contains("1 to 65535")
        );
        assert!(parse(&["--port", "x"]).unwrap_err().contains("`x`"));
        assert!(parse(&["--config"]).unwrap_err().contains("needs a file"));
        assert!(
            parse(&["--frobnicate"])
                .unwrap_err()
                .contains("unknown option")
        );
    }
}
