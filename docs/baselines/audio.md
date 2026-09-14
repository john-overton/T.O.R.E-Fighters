# Recorded audio acceptance — 2026-09-14

Host: Linux x86_64, Rust 1.91.1, NVIDIA RTX 4070 / Vulkan / Wayland.
This is recorded-PCM readiness and the first NORMAL free-flight music slice,
not full native situation/mixer parity. The user explicitly excluded MIDI and
synthesis from this work. [Source grammar and limits](../formats/music.md).

## Source identities and extraction

| Archive | SHA-256 |
| --- | --- |
| FA_2.LIB | fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198 |
| FA_4B.LIB | 34b04faaae90b857c04cb0ab7fedb6d00f19523871a3c075cf0e357d66a19f62 |
| FA_4D.LIB | 247ff0fe70975d24f8f4187fe8d62e3833546b71ba199d8117fc0546865a453c |

```sh
python3 tools/extract_assets.py --music --wav-previews --exclude-archive 'disc1/LHX/*' --out .local/audio-ready
cargo run --locked -p tore-app -- --import gameassets/fighters-anthology --import-only
```

Result: 108 original resources, zero errors; nine MUS modules and 99 PCM
recordings totaling 48,679,357 sample bytes. Repeat extraction reports all
108 originals unchanged and reuses identical WAVs. Python's standard `wave`
reader independently confirmed all 99 previews are 11,025 Hz, mono, one byte
per sample and reproduce the original sample bytes exactly. Preview SHA-256
values match the wrapper's report. Raw files, previews, hashes and original
native disassembly remain ignored under `.local/audio-ready/` and
`.local/audio-exploration/`.

Native app import succeeded and the refreshed cache reloads through the regular
runtime loader. Both app and CLI use `tore-formats::music::resource`. NORMAL
has 43/43 referenced phrases. AIR029, AIR041, AIR015 and VALK001 are missing
from their other scores and are reported, never substituted. Extraction success
does not certify those other scores complete.

A local Rust probe against the new parser exercised each retail script with
64 explicit audio RNG seeds and up to 256 phrase requests per seed. NORMAL
yielded 16,384 phrases with no missing references or execution-budget failure.
SUCC stopped after one phrase for each seed; LAUNCH/HOME also reached stop.
The other scripts produced only the documented missing-reference sets. This
tests our interpreter against actual script data, not native RNG/host parity.
The scratch probe remains `.local/audio-checks/score_probe.rs`.

## Automated and desktop checks

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 174 tests passed, zero failures.
- `cargo build --workspace --locked`: passed.
- Python tool tests: 11 passed.
- Asset guard: repository, debug app and debug extractor passed.
- Menu, Quick Mission Creator, terrain viewer, F18 and Rafale window smoke
  checks passed with `--smoke-test` and the corresponding screen/aircraft flags.
- Both aircraft completed a bounded 360-frame active flight with the real CPAL
  default output at 44,100 Hz stereo, without audio initialization/stream errors.
  Commands used `TORE_PERF_FRAMES=360 TORE_PERF_ACTIVE=1 target/debug/tore-app
  --free-flight --aircraft f18` and the equivalent `rafale` selection.

Logs remain under `.local/audio-checks/`. Those brief device checks establish
startup/stream operation, not listening acceptance, displayed FPS, or a
performance improvement. No renderer composition changed.

Synthetic coverage includes malformed/truncated/overlapping MUS operands,
invalid chances and empty random groups, nonproductive-cycle bounds, phrase
advance/stop/restart, missing-clip silence, pause/mute playhead retention,
independent effects, paused UI click playback, resampling/loop boundaries, WAV
round-trip, extraction profile exclusion of MIDI, preview provenance paths,
dry-run, repeat extraction and protection of edited preview files.

Both aircraft use the real fixed-tick `State::step` path with synthetic aircraft
fixtures to verify one cue per brake state change, silent repeated desired-state
commands, burner isolation and release sound. Existing input modifier/release
and pause/no-catch-up suites remain green. Ground squeal selection uses explicit
hybrid contact state; no arbitrary terrain sample is treated as native contact.

## Remaining acceptance

Original host priorities, transition timings, mixer gains, wheel-brake behavior
and hook/flap argument polarity need further native/audible checks. All situation
scripts are ready as data, but only NORMAL is automatically selected during free
flight. Combat, danger, carrier, success and ejection events remain unavailable.
No MIDI-only fallback is planned in this slice. Main/briefing context resets and
their use in the development creator/viewer are explicitly authored.

Windows/macOS runtime and human listening comparison with the original game
were not performed. The flight Sound command still toggles effects only; music
uses the saved main-menu preference. No new submenu screen or volume mixer was
scheduled. No retail bytes or derivative audio are committed.
