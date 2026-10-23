//! `--input-script FILE`: a development script of key presses and mouse
//! clicks fed to a windowed run through the same handlers the window's own
//! events use, so a flight, menu or screen can be driven without a person, a
//! compositor that delivers focus, or a device. It is for tests; see
//! docs/DEVELOPMENT.md.
//!
//! One step per line, `#` starts a comment:
//!
//! ```text
//! wait 1.5              seconds of wall clock
//! stall 8               block the game's whole main loop for this many seconds (at most 600),
//!                       frames, the session pump and the script included, as a dragged window or a
//!                       long frame does; only a keepalive thread keeps a joined game connected
//!                       through it (SIGSTOP freezes that thread too, so it cannot test it)
//! waittick 600 [90]     until the flight has run this many 120 Hz ticks, or 90 s of wall
//!                       clock (the default) whichever is first, so a stuck menu cannot hang a run
//! key g                 press and release; Shift+e, Ctrl+B, F10, Space, Escape, Enter, Up
//! down Up               hold a key
//! up Up                 release it
//! move 320 240          mouse to window pixels
//! movemenu 320 240      mouse to a point of the 640 by 480 menu layer
//! click [left|right]    press and release
//! press [left|right]    hold a mouse button
//! release [left|right]
//! wheel 3               wheel notches, negative for down
//! snapshot out.ppm      the menu layer as it is drawn now (menu screens); a relative path
//!                       goes in the folder named by TORE_SCRIPT_OUT when that is set
//! shot out.ppm          the flight as it is drawn now, with the cockpit, HUD and
//!                       instruments (the same path rule)
//! exit                  quit
//! ```
use std::path::PathBuf;
use std::time::Instant;
use winit::event::MouseButton;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey, SmolStr};

/// The longest `stall` step, seconds: a typo cannot hang a run for hours.
pub const MAX_STALL_SECONDS: f64 = 600.;

/// A key press or release as the window handler sees it.
#[derive(Clone, Debug)]
pub struct KeyInput {
    pub logical: Key,
    pub physical: PhysicalKey,
    pub pressed: bool,
    pub repeat: bool,
    pub text: Option<SmolStr>,
}

/// A key named in a script, with the modifiers held for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeySpec {
    pub mods: ModifiersState,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Wait(f64),
    /// Blocks the main loop (the test of a stalled game's keepalive).
    Stall(f64),
    WaitTick(u64, f64),
    Tap(KeySpec),
    Down(KeySpec),
    Up(KeySpec),
    Move(f64, f64),
    MoveMenu(f64, f64),
    Press(MouseButton),
    Release(MouseButton),
    Click(MouseButton),
    Wheel(f32),
    Snapshot(PathBuf),
    Shot(PathBuf),
    Exit,
}

fn button(word: Option<&str>) -> Result<MouseButton, String> {
    match word.unwrap_or("left") {
        "left" => Ok(MouseButton::Left),
        "right" => Ok(MouseButton::Right),
        other => Err(format!("unknown mouse button {other:?}, use left or right")),
    }
}

/// Splits `Shift+Ctrl+e` into modifiers and the key name.
pub fn key_spec(text: &str) -> Result<KeySpec, String> {
    let mut mods = ModifiersState::empty();
    let mut parts: Vec<&str> = text.split('+').collect();
    // A key that is itself the plus sign ends the text with "+".
    let name = if text.ends_with('+') && text.len() > 1 {
        parts.truncate(parts.len().saturating_sub(2));
        "+".to_owned()
    } else {
        parts.pop().unwrap_or_default().to_owned()
    };
    for part in parts {
        match part.to_ascii_lowercase().as_str() {
            "shift" => mods |= ModifiersState::SHIFT,
            "ctrl" | "control" => mods |= ModifiersState::CONTROL,
            "alt" => mods |= ModifiersState::ALT,
            other => return Err(format!("unknown modifier {other:?} in {text:?}")),
        }
    }
    let spec = KeySpec { mods, name };
    if lookup(&spec.name).is_none() {
        return Err(format!("unknown key {:?}", spec.name));
    }
    Ok(spec)
}

fn punctuation(name: &str) -> Option<(KeyCode, &'static str)> {
    Some(match name {
        "[" | "bracketleft" => (KeyCode::BracketLeft, "["),
        "]" | "bracketright" => (KeyCode::BracketRight, "]"),
        "," | "comma" => (KeyCode::Comma, ","),
        "." | "period" => (KeyCode::Period, "."),
        ";" | "semicolon" => (KeyCode::Semicolon, ";"),
        "\\" | "backslash" => (KeyCode::Backslash, "\\"),
        "'" | "quote" => (KeyCode::Quote, "'"),
        "/" | "slash" => (KeyCode::Slash, "/"),
        "-" | "minus" => (KeyCode::Minus, "-"),
        "=" | "equal" => (KeyCode::Equal, "="),
        "`" | "grave" => (KeyCode::Backquote, "`"),
        _ => return None,
    })
}

fn letter(c: char) -> Option<KeyCode> {
    use KeyCode::*;
    Some(match c {
        'a' => KeyA,
        'b' => KeyB,
        'c' => KeyC,
        'd' => KeyD,
        'e' => KeyE,
        'f' => KeyF,
        'g' => KeyG,
        'h' => KeyH,
        'i' => KeyI,
        'j' => KeyJ,
        'k' => KeyK,
        'l' => KeyL,
        'm' => KeyM,
        'n' => KeyN,
        'o' => KeyO,
        'p' => KeyP,
        'q' => KeyQ,
        'r' => KeyR,
        's' => KeyS,
        't' => KeyT,
        'u' => KeyU,
        'v' => KeyV,
        'w' => KeyW,
        'x' => KeyX,
        'y' => KeyY,
        'z' => KeyZ,
        '0' => Digit0,
        '1' => Digit1,
        '2' => Digit2,
        '3' => Digit3,
        '4' => Digit4,
        '5' => Digit5,
        '6' => Digit6,
        '7' => Digit7,
        '8' => Digit8,
        '9' => Digit9,
        _ => return None,
    })
}

fn named(name: &str) -> Option<(NamedKey, KeyCode)> {
    use KeyCode as C;
    use NamedKey as N;
    Some(match name.to_ascii_lowercase().as_str() {
        "escape" | "esc" => (N::Escape, C::Escape),
        "enter" | "return" => (N::Enter, C::Enter),
        "space" => (N::Space, C::Space),
        "tab" => (N::Tab, C::Tab),
        "backspace" => (N::Backspace, C::Backspace),
        "delete" => (N::Delete, C::Delete),
        "insert" => (N::Insert, C::Insert),
        "home" => (N::Home, C::Home),
        "end" => (N::End, C::End),
        "pageup" | "prior" => (N::PageUp, C::PageUp),
        "pagedown" | "next" => (N::PageDown, C::PageDown),
        "up" | "arrowup" => (N::ArrowUp, C::ArrowUp),
        "down" | "arrowdown" => (N::ArrowDown, C::ArrowDown),
        "left" | "arrowleft" => (N::ArrowLeft, C::ArrowLeft),
        "right" | "arrowright" => (N::ArrowRight, C::ArrowRight),
        "f1" => (N::F1, C::F1),
        "f2" => (N::F2, C::F2),
        "f3" => (N::F3, C::F3),
        "f4" => (N::F4, C::F4),
        "f5" => (N::F5, C::F5),
        "f6" => (N::F6, C::F6),
        "f7" => (N::F7, C::F7),
        "f8" => (N::F8, C::F8),
        "f9" => (N::F9, C::F9),
        "f10" => (N::F10, C::F10),
        "f11" => (N::F11, C::F11),
        "f12" => (N::F12, C::F12),
        _ => return None,
    })
}

/// The logical key, physical key and typed text a key name stands for.
fn lookup(name: &str) -> Option<(Key, PhysicalKey, Option<SmolStr>)> {
    if let Some((key, code)) = named(name) {
        let text = (key == NamedKey::Space).then(|| SmolStr::new(" "));
        return Some((Key::Named(key), PhysicalKey::Code(code), text));
    }
    if let Some((code, text)) = punctuation(name) {
        return Some((
            Key::Character(SmolStr::new(text)),
            PhysicalKey::Code(code),
            Some(SmolStr::new(text)),
        ));
    }
    let mut chars = name.chars();
    let (c, rest) = (chars.next()?, chars.next());
    if rest.is_some() {
        return None;
    }
    let lower = c.to_ascii_lowercase();
    let code = letter(lower)?;
    let text = SmolStr::new(lower.to_string());
    Some((
        Key::Character(text.clone()),
        PhysicalKey::Code(code),
        Some(text),
    ))
}

impl KeySpec {
    /// The event for pressing or releasing this key.
    pub fn input(&self, pressed: bool) -> KeyInput {
        let (logical, physical, text) = lookup(&self.name).expect("checked when parsed");
        let typed = self.mods.control_key() || self.mods.alt_key();
        KeyInput {
            logical,
            physical,
            pressed,
            repeat: false,
            text: if typed { None } else { text },
        }
    }
}

/// Parses a whole script; an error names the line.
pub fn parse(text: &str) -> Result<Vec<Step>, String> {
    let mut steps = vec![];
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        let at = |e: String| format!("input script line {}: {e}", n + 1);
        let words: Vec<&str> = line.split_whitespace().collect();
        let number = |i: usize| -> Result<f64, String> {
            let word = words
                .get(i)
                .ok_or_else(|| at(format!("{} needs a number", words[0])))?;
            word.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| at(format!("{} needs a number, not {word:?}", words[0])))
        };
        let key = || key_spec(words.get(1).copied().unwrap_or_default()).map_err(at);
        steps.push(match words[0] {
            "wait" => Step::Wait(number(1)?.max(0.)),
            "stall" => Step::Stall(number(1)?.clamp(0., MAX_STALL_SECONDS)),
            "waittick" => Step::WaitTick(
                number(1)?.max(0.) as u64,
                if words.len() > 2 {
                    number(2)?.max(0.)
                } else {
                    90.
                },
            ),
            "key" => Step::Tap(key()?),
            "down" => Step::Down(key()?),
            "up" => Step::Up(key()?),
            "move" => Step::Move(number(1)?, number(2)?),
            "movemenu" => Step::MoveMenu(number(1)?, number(2)?),
            "click" => Step::Click(button(words.get(1).copied()).map_err(at)?),
            "press" => Step::Press(button(words.get(1).copied()).map_err(at)?),
            "release" => Step::Release(button(words.get(1).copied()).map_err(at)?),
            "wheel" => Step::Wheel(number(1)? as f32),
            "snapshot" => Step::Snapshot(PathBuf::from(
                words
                    .get(1)
                    .ok_or_else(|| at("snapshot needs a path".into()))?,
            )),
            "shot" => Step::Shot(PathBuf::from(
                words.get(1).ok_or_else(|| at("shot needs a path".into()))?,
            )),
            "exit" => Step::Exit,
            other => return Err(at(format!("unknown step {other:?}"))),
        });
    }
    Ok(steps)
}

/// Where a running script is.
pub struct Runner {
    steps: Vec<Step>,
    next: usize,
    wait_until: Option<Instant>,
    tick_wait: Option<Instant>,
    /// A tap's release, sent on the following turn so the game sees a frame between.
    pub release: Option<(KeySpec, bool)>,
    pub mods: ModifiersState,
}

impl Runner {
    pub fn new(steps: Vec<Step>) -> Self {
        Self {
            steps,
            next: 0,
            wait_until: None,
            tick_wait: None,
            release: None,
            mods: ModifiersState::empty(),
        }
    }

    /// The next step that is due, given the flight's tick count; `None` while waiting.
    pub fn due(&mut self, tick: u64, now: Instant) -> Option<Step> {
        if self.release.is_some() {
            return None;
        }
        if self.wait_until.is_some_and(|until| now < until) {
            return None;
        }
        self.wait_until = None;
        let step = self.steps.get(self.next)?.clone();
        match step {
            Step::Wait(seconds) => {
                self.wait_until = Some(now + std::time::Duration::from_secs_f64(seconds));
            }
            Step::WaitTick(n, seconds) => {
                let since = *self.tick_wait.get_or_insert(now);
                let timed_out = now.duration_since(since).as_secs_f64() >= seconds;
                if tick < n && !timed_out {
                    return None;
                }
                self.tick_wait = None;
            }
            _ => {}
        }
        self.next += 1;
        Some(step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_parses_to_its_steps() {
        let steps = parse(
            "# a comment\nwait 0.5\nkey g\nkey Shift+e\ndown Up\nup Up\nmove 10 20\nmovemenu 320 240\nclick right\nwheel -2\nsnapshot a.ppm # here\nwaittick 600\nexit\n",
        )
        .unwrap();
        assert_eq!(steps.len(), 12);
        assert_eq!(steps[0], Step::Wait(0.5));
        assert_eq!(
            steps[2],
            Step::Tap(KeySpec {
                mods: ModifiersState::SHIFT,
                name: "e".into()
            })
        );
        assert_eq!(steps[7], Step::Click(MouseButton::Right));
        assert_eq!(steps[10], Step::WaitTick(600, 90.));
        assert_eq!(steps[11], Step::Exit);
    }

    #[test]
    fn a_stall_step_parses_with_its_seconds_and_is_bounded() {
        assert_eq!(parse("stall 8\n").unwrap(), [Step::Stall(8.)]);
        assert_eq!(
            parse("stall 0.25 # a quarter\n").unwrap(),
            [Step::Stall(0.25)]
        );
        assert_eq!(parse("stall -3\n").unwrap(), [Step::Stall(0.)]);
        assert_eq!(
            parse("stall 1e9\n").unwrap(),
            [Step::Stall(MAX_STALL_SECONDS)]
        );
        assert!(
            parse("stall\n")
                .unwrap_err()
                .contains("stall needs a number")
        );
        assert!(
            parse("stall soon\n")
                .unwrap_err()
                .contains("needs a number")
        );
        // The runner hands it to the game like any other step, and the next
        // step follows at once.
        let mut runner = Runner::new(parse("stall 8\nexit\n").unwrap());
        let now = Instant::now();
        assert_eq!(runner.due(0, now), Some(Step::Stall(8.)));
        assert_eq!(runner.due(0, now), Some(Step::Exit));
    }

    #[test]
    fn bad_lines_name_the_line() {
        for (text, needle) in [
            ("wait\n", "line 1: wait needs a number"),
            ("key g\nkey\n", "line 2: unknown key"),
            ("key Hyper+g\n", "unknown modifier"),
            ("key f13\n", "unknown key"),
            ("click middle\n", "unknown mouse button"),
            ("dance\n", "unknown step"),
            ("move 1 x\n", "needs a number"),
            ("snapshot\n", "needs a path"),
        ] {
            let error = parse(text).unwrap_err();
            assert!(error.contains(needle), "{text:?} gave {error}");
        }
    }

    #[test]
    fn keys_carry_the_names_the_game_reads() {
        let g = key_spec("g").unwrap().input(true);
        assert_eq!(g.physical, PhysicalKey::Code(KeyCode::KeyG));
        assert_eq!(g.logical, Key::Character(SmolStr::new("g")));
        let five = key_spec("5").unwrap().input(true);
        assert_eq!(five.physical, PhysicalKey::Code(KeyCode::Digit5));
        let ctrl_b = key_spec("Ctrl+B").unwrap();
        assert!(ctrl_b.mods.control_key());
        assert!(ctrl_b.input(true).text.is_none());
        assert_eq!(
            key_spec("Insert").unwrap().input(true).logical,
            Key::Named(NamedKey::Insert)
        );
        assert_eq!(
            key_spec("Alt+F4").unwrap().input(true).logical,
            Key::Named(NamedKey::F4)
        );
        assert_eq!(
            key_spec("]").unwrap().input(true).physical,
            PhysicalKey::Code(KeyCode::BracketRight)
        );
    }

    #[test]
    fn a_tick_wait_gives_up_after_its_seconds() {
        let mut runner = Runner::new(parse("waittick 1000 2\nexit\n").unwrap());
        let now = Instant::now();
        assert_eq!(runner.due(0, now), None);
        assert_eq!(runner.due(0, now + std::time::Duration::from_secs(1)), None);
        assert_eq!(
            runner.due(0, now + std::time::Duration::from_secs(3)),
            Some(Step::WaitTick(1000, 2.))
        );
        assert_eq!(
            runner.due(0, now + std::time::Duration::from_secs(3)),
            Some(Step::Exit)
        );
    }

    #[test]
    fn the_runner_waits_for_time_and_ticks() {
        let mut runner = Runner::new(parse("wait 10\nwaittick 5\nexit\n").unwrap());
        let now = Instant::now();
        assert_eq!(runner.due(0, now), Some(Step::Wait(10.)));
        assert_eq!(runner.due(0, now), None);
        let later = now + std::time::Duration::from_secs(11);
        assert_eq!(runner.due(4, later), None);
        assert_eq!(runner.due(5, later), Some(Step::WaitTick(5, 90.)));
        assert_eq!(runner.due(5, later), Some(Step::Exit));
    }
}
