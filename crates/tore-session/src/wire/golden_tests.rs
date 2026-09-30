//! The wire golden test: a fixed set of sample sections and messages,
//! encoded and compared with the committed copy in `wire-golden.txt`.
//!
//! When the bytes change the test fails: raise [`PROTOCOL_VERSION`], then
//! refresh the copy with
//! `TORE_UPDATE_WIRE_GOLDEN=1 cargo test --locked -p tore-session wire_golden`.
//! Refreshing without raising the version is refused, so the bytes cannot
//! change under an old version number.

use super::own_state::OwnStateHeader;
use super::{PROTOCOL_VERSION, samples};
use std::fmt::Write as _;
use tore_codec::BitWriter;

const GOLDEN: &str = "wire-golden.txt";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// Every sample, one line each: its name and its bytes in hex.
fn lines() -> Vec<(String, String)> {
    let mut out = Vec::new();
    out.push(("inputs".into(), hex(&samples::inputs().encode().unwrap())));
    let (full, delta) = samples::snapshots();
    out.push(("snapshot-full".into(), hex(&full)));
    out.push(("snapshot-delta".into(), hex(&delta)));
    out.push(("events".into(), hex(&samples::events().encode().unwrap())));
    // The own state's header; its body is the exact state's own coding,
    // which `tore-world` tests.
    let mut w = BitWriter::new();
    OwnStateHeader {
        tick: 7_204,
        number: 65_535,
        back: 31,
    }
    .write(&mut w);
    out.push(("own-state-header".into(), hex(&w.finish())));
    for message in samples::messages(vec![1, 2, 3, 255]) {
        out.push((
            format!("message-{}", message.kind()),
            hex(&message.encode().unwrap()),
        ));
    }
    // Added under protocol 1 before anything shipped (the lead, 2026-09-30).
    let ended = super::messages::Message::MissionEnded(super::messages::MissionEnded {
        reason: super::messages::EndReason::EndedByServer,
        next_in_seconds: Some(30),
    });
    out.push((
        "message-11-ended-by-server".into(),
        hex(&ended.encode().unwrap()),
    ));
    out
}

fn render(version: u16) -> String {
    let mut text = format!(
        "# T.O.R.E wire golden: tore-session's sample sections and messages.\n\
         # Raise PROTOCOL_VERSION, then refresh with TORE_UPDATE_WIRE_GOLDEN=1.\n\
         version {version}\n"
    );
    for (name, bytes) in lines() {
        let _ = writeln!(text, "{name} {bytes}");
    }
    text
}

fn committed_version(text: &str) -> Option<u16> {
    text.lines()
        .find_map(|line| line.strip_prefix("version "))
        .and_then(|v| v.trim().parse().ok())
}

#[test]
fn wire_golden() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN);
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    let now = render(PROTOCOL_VERSION);
    if now == committed {
        return;
    }
    let old_version = committed_version(&committed);
    let bytes_changed = render(old_version.unwrap_or(PROTOCOL_VERSION)) != committed;
    if std::env::var_os("TORE_UPDATE_WIRE_GOLDEN").is_some() {
        assert!(
            !(bytes_changed && old_version == Some(PROTOCOL_VERSION)),
            "the wire bytes changed: raise PROTOCOL_VERSION in crates/tore-session/src/wire/mod.rs \
             before refreshing {GOLDEN}"
        );
        std::fs::write(&path, now).unwrap();
        return;
    }
    panic!(
        "the wire bytes differ from {GOLDEN} (protocol version {PROTOCOL_VERSION}, committed {old_version:?}): \
         raise PROTOCOL_VERSION in crates/tore-session/src/wire/mod.rs, then run \
         TORE_UPDATE_WIRE_GOLDEN=1 cargo test --locked -p tore-session wire_golden"
    );
}
