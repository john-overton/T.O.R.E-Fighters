# Ordnance sound effects

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research and implementation modes, 2026-09-22. Static retail inspection found
and identified the three original ordnance editing samples. The implementation
follows the [sound behavior specification](../spec/ordnance-presentation.md#sound-effects).

## Source identity and evidence

The locally supplied files have these SHA-256 identities:

| File | SHA-256 |
| --- | --- |
| FA.EXE | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |
| FA.SMS | `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0` |
| FA_2.LIB | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |

The EXE/SMS match the reviewed pair. Bounded static disassembly and PE string
reads, plus the SMS BasicSound symbol, establish the
[selection and successful-edit gates](../formats/ordnance-menu.md#sound-selection).
The reviewed OBJECT + PROJECTILE schema independently places projectile flags
at offset `0xa6`. No imported instructions were executed.

The shared Rust extractor read exactly three requested archive records with
zero errors. They are unsigned PCM8 mono. Measured sample sizes and durations
under the existing PCM reader are:

| Sample | Bytes/samples | Rate | Duration |
| --- | ---: | ---: | ---: |
| &ARMWPN.5K | 3,488 | 5,512 Hz | 0.632801 s |
| &ARMBLLT.5K | 2,309 | 5,512 Hz | 0.418904 s |
| &ARMDRIP.11K | 2,832 | 11,025 Hz | 0.256871 s |

Ignored local evidence: `.local/ordnance-audio/source.json`, bounded disassembly
text in that directory, and `samples/extraction-report.json`. The extraction
report records each archive offset, stored/decoded size and individual sample
hash. The shared `--native-menus` research profile now includes both reviewed
sound branches for reproducible extraction.

## Implementation and limits

Successful station edits emit a dedicated weapon or ammunition audio event;
unsuccessful and unchanged edits emit none. Fuel edits use their own sample and
reuse a still-playing clip. The shared creator resource profile imports all
three samples; cache validation requires and parses them, causing the existing
local-media refresh path to update older caches. Sound-effects preferences and
`--no-audio` retain their existing behavior.

## Validation

All required workspace checks passed: formatting, warnings-denied Clippy,
locked build/tests, 70 Python tests, source and both debug-binary asset guards,
documentation headers and diff checks. Rust results: 1,152 passed, three explicit
GPU tests ignored. Nine focused app tests cover edit cues, successful/rejected
transfers, quantity limits, ammunition flags, fuel limits, effects muting and
fuel-clip reuse. The shared creator profile has a synthetic resource-selection
test. No retail bytes are embedded in these tests.

An isolated copy of the previous cache was rejected for its missing ordnance
sample, automatically refreshed from local media, and successfully loaded the
Ordnance CPU preview. Its import report records all three decoded samples at
the expected sizes. Main-menu and Ordnance GPU smoke tests passed on NVIDIA
RTX 4070 / Vulkan. Logs and the isolated cache remain under
`.local/ordnance-audio/`.

After the audio fix, the headless-development instructions were verified with
DISPLAY and WAYLAND_DISPLAY unset: the dev build completed a 1,200-tick F/A-18D
hybrid flight probe, and the CPU Ordnance snapshot exited successfully. Neither
run opened a window or audio device. These commands are linked from AGENTS.md
and documented in [headless development](../DEVELOPMENT.md#headless-development).

Original device volume, shell repeat timing, Unload All audio and catalog
selection audio remain unverified. No original executable was run, no retail
playthrough comparison or manual listening acceptance was performed, and no
retail bytes or derived audio were committed. Windows/macOS execution was not
performed.
