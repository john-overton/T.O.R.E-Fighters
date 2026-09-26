//! Hostile bytes: random changes inside chunks whose checksums are then
//! repaired, so the damage reaches every decoder. Opening, decoding and every
//! export may fail with an error, but must never panic.

mod common;

use common::*;
use tore_replay::export::{
    AcmiOptions, CompareOptions, JsonlOptions, SummaryOptions, Thresholds, compare, detect,
    write_acmi, write_jsonl, write_summary,
};
use tore_replay::*;

fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// `(offset, body length)` of every chunk, from the documented layout.
fn chunks(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut pos = 12;
    while pos + 32 <= bytes.len() && &bytes[pos..pos + 4] == b"TORC" {
        let len = u32::from_le_bytes(bytes[pos + 8..pos + 12].try_into().unwrap()) as usize;
        out.push((pos, len));
        pos += 32 + len;
    }
    out
}

fn reseal(bytes: &mut [u8], pos: usize, len: usize) {
    let hash = fnv1a(
        fnv1a(0xcbf2_9ce4_8422_2325, &bytes[pos..pos + 24]),
        &bytes[pos + 32..pos + 32 + len],
    );
    bytes[pos + 24..pos + 32].copy_from_slice(&hash.to_le_bytes());
}

/// Everything a viewer or an export would do with a file.
fn exercise(recording: &Recording) {
    for frame in recording.frames(0, u64::MAX) {
        if frame.is_err() {
            break;
        }
    }
    for i in 0..recording.chunks().len() {
        let _ = recording.decode_chunk(i);
        let _ = recording.chunk_trees(i);
    }
    if let Some(last) = recording.last_tick() {
        let _ = recording.tree(0, "flight.telemetry", last);
        let _ = recording.live_puffs(last);
        let _ = recording.live_effects(last, 600);
    }
    let _ = detect(recording, &Thresholds::default());
    let _ = write_summary(recording, &SummaryOptions::default(), std::io::sink());
    let _ = write_jsonl(recording, &JsonlOptions::default(), std::io::sink());
    let _ = write_acmi(recording, &AcmiOptions::default(), std::io::sink());
    let _ = compare(recording, recording, &CompareOptions::default());
}

#[test]
fn corrupted_chunks_with_repaired_checksums_never_panic() {
    let dir = temp_dir("fuzz");
    let scenario = rich_scenario(500, 48, 21);
    let options = WriterOptions {
        chunk_ticks: 16,
        ..WriterOptions::default()
    };
    let path = write(&dir, "fuzz.tore-replay", &scenario, options);
    let bytes = std::fs::read(&path).unwrap();
    let list = chunks(&bytes);
    assert!(list.len() >= 5);
    exercise(&Recording::from_bytes(bytes.clone()).unwrap());
    let mut rng = Rng::new(99);
    let mut opened = 0;
    // TORE_FUZZ_ITERATIONS runs a longer pass locally.
    let iterations: u32 = std::env::var("TORE_FUZZ_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600);
    for _ in 0..iterations {
        let mut edited = bytes.clone();
        let (pos, len) = list[(rng.next() % list.len() as u64) as usize];
        if len == 0 {
            continue;
        }
        for _ in 0..1 + rng.next() % 3 {
            let at = pos + 32 + (rng.next() % len as u64) as usize;
            edited[at] = match rng.next() % 4 {
                0 => 0,
                1 => 0xff,
                2 => edited[at] ^ (1 << (rng.next() % 8)),
                _ => rng.next() as u8,
            };
        }
        reseal(&mut edited, pos, len);
        if let Ok(recording) = Recording::from_bytes(edited) {
            opened += 1;
            exercise(&recording);
        }
    }
    assert!(opened > 0);
    let _ = std::fs::remove_dir_all(dir);
}
