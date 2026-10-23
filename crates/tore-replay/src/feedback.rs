//! Player feedback uses ordinary named events, readable by formats 2 through 4.
use crate::{Event, Recording, Value};

pub const KIND: &str = "player.feedback";
pub const MAX_CHARS: usize = 1_000;
pub const MAX_BYTES: usize = MAX_CHARS * 4;

/// The flag position and submission, with identity supplied by the host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Feedback {
    pub tick: u64,
    pub submit_tick: u64,
    /// UTC Unix milliseconds at opening and submission.
    pub opened_at: u64,
    pub submitted_at: u64,
    pub player: String,
    pub seat: u8,
    pub plane: u32,
    pub aircraft: String,
    pub text: String,
}
impl Feedback {
    pub fn valid_text(text: &str) -> bool {
        !text.trim().is_empty()
            && text.len() <= MAX_BYTES
            && text.chars().count() <= MAX_CHARS
            && !text
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
    }
    pub fn event(&self) -> Event {
        let mut parts = Vec::new();
        let mut rest = self.text.as_str();
        while !rest.is_empty() {
            let mut n = rest.len().min(1_000);
            while !rest.is_char_boundary(n) {
                n -= 1;
            }
            parts.push(&rest[..n]);
            rest = &rest[n..];
        }
        let mut event = Event::new(KIND)
            .with_text(parts.first().copied().unwrap_or_default())
            .with("flag_tick", self.tick as i64)
            .with("mission_seconds", self.tick as f64 / 120.)
            .with("submit_tick", self.submit_tick as i64)
            .with("opened_at_ms", self.opened_at as i64)
            .with("submitted_at_ms", self.submitted_at as i64)
            .with("player", self.player.as_str())
            .with("seat", i64::from(self.seat))
            .with("plane", i64::from(self.plane))
            .with("aircraft", self.aircraft.as_str());
        for (i, part) in parts.iter().enumerate() {
            event = event.with(&format!("text.{i}"), *part);
        }
        event
    }
    pub fn from_event(event: &Event) -> Option<Self> {
        if event.kind != KIND {
            return None;
        }
        let int = |key: &str| {
            event
                .fields
                .iter()
                .find(|(k, _)| k == key)
                .and_then(|(_, v)| v.as_i64())
                .and_then(|v| u64::try_from(v).ok())
        };
        let text = |key: &str| {
            event
                .fields
                .iter()
                .find(|(k, _)| k == key)
                .and_then(|(_, v)| Value::as_str(v))
                .map(str::to_owned)
        };
        Some(Self {
            tick: int("flag_tick")?,
            submit_tick: int("submit_tick")?,
            opened_at: int("opened_at_ms")?,
            submitted_at: int("submitted_at_ms")?,
            player: text("player")?,
            seat: int("seat")?.try_into().ok()?,
            plane: int("plane")?.try_into().ok()?,
            aircraft: text("aircraft")?,
            text: {
                let pieces: Vec<_> = (0..5).map_while(|i| text(&format!("text.{i}"))).collect();
                if pieces.is_empty() {
                    event.text.clone()
                } else {
                    pieces.concat()
                }
            },
        })
    }
}
impl Recording {
    pub fn feedback(&self) -> impl Iterator<Item = Feedback> + '_ {
        self.events()
            .iter()
            .filter_map(|e| Feedback::from_event(&e.event))
    }
    /// Any feedback protects a recording, even an unfamiliar field layout.
    pub fn flagged(&self) -> bool {
        self.events().iter().any(|e| e.event.kind == KIND)
    }
}
