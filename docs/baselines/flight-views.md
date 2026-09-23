# Flight view validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-23, branch `retail-views`, based on `61eedea`.
Behavior and manual identity belong in the [view spec](../spec/flight-views.md).
Local evidence lives in `.local/retail-views/` and `.local/keyboard-map/` in the
worktree. No source media, extracted images or capture derivatives are committed.

## Automated checks

Synthetic camera tests cover player/target endpoint reversal, wing selection,
nearest inbound missile, bank-relative tracking and the eye-line limit,
three-second fly-by placement and a fixed position after subject movement,
reselecting a fly-by, independent saved views, coincident subjects, missing
subjects, hidden remote aircraft, and last-missile identity after expiration.
An incoming diagnostic round sharing a shot counter cannot become the player's
last launched missile. Saved view changes reject an outstanding old panel frame.

Shortcut tests cover every retail function key with normal, Ctrl and Alt
references; Shift and Ctrl+Alt do not select views. F11 remains help and Alt+F4
remains exit. Imported View menu shortcuts use the same mapping. The existing
Envelope Current menu action remains intact. Catalog parsing and the generated
controls document check every new action and binding.

Repository checks: formatting, Clippy with warnings denied, workspace tests,
workspace build, 75 Python tests, source and both executable asset checks, and
documentation checks passed. Linux was tested; Windows and macOS runtime checks
were not run. Three pre-existing GPU tests remain ignored in the ordinary suite.
The standard 1,200-tick headless flight also passed at 436.693 knots and
5,014.249 feet, without a crash. The fixed 120 Hz simulation and all flight
adapter defaults are unchanged.

## GPU captures

Linux, NVIDIA GeForce RTX 4070, Vulkan, 4x anti-aliasing, 960x720 captures, original
F/A-18D assets imported into the worktree's isolated `.local/dev-profile`.
`cargo run --locked -p tore-app -- --smoke-test` passed with that profile.
Thirteen application capture runs exited successfully. The images were inspected.

Each example uses `TORE_DATA_DIR=.local/dev-profile target/debug/tore-app`, plus
`--no-audio --no-controllers --capture-flight .local/retail-views/NAME.ppm`:

| Case | Additional arguments | Result |
| --- | --- | --- |
| F1 | `--free-flight --flight-view 0` | Forward cockpit retained |
| F4 | `--hud-target-preview 40,20,6000 --flight-view 5` | Tracks the elevated right-hand target; cockpit remains nose-aligned |
| F5 | `--live-fire --weapon-slot 2 --combat-command incoming --combat-probe-ticks 1 --flight-view 6` | Player exterior faces the inbound round |
| F6 empty wing | `--launch-quick-mission --flight-view 7` | Default setup has only enemy aircraft; explicit no-wingman feedback and Forward fallback. Positive wing selection is covered synthetically, not by this capture |
| F7/F8 | `--hud-target-preview 40,20,6000 --flight-view 8` or `9` | The two exterior viewpoints reverse player/target endpoints |
| F9 | `--free-flight --flight-view 10` | Fixed fly-by viewpoint; motion and reselect behavior tested synthetically |
| F12 | `--live-fire --weapon-slot 2 --combat-probe-ticks 100 --flight-view 11` | Last player missile visible from behind, facing its target |
| Alt+F1/Alt+F10 | `--hud-target-preview 40,20,6000 --flight-view 0` or `1`, `--flight-reference target` | Remote forward omits the player cockpit/reference aircraft; remote external shows the target |
| Ctrl+F10 | `--live-fire --weapon-slot 2 --combat-probe-ticks 100 --flight-view 1 --flight-reference missile` | External missile reference |
| Missing F12 | `--free-flight --flight-view 11` | Forward fallback with no-missile feedback |
| Other View default | `--free-flight --instrument-page 3` | Backward scene in the original Other View frame |

Capture indices preserve the earlier CLI and differ from F-key numbers. The
plain smoke test, captures and synthetic checks are host validation, not a
retail comparison. Physical joystick/head-tracker operation and a complete
interactive sortie through every reference combination were not tested.

## Keyboard documents

The catalog generated `CONTROLS.md`; flight/input/development guides, architecture,
feature matrix and current planning links were updated. The standalone HTML
retains all three sheets, embedded font/badge bytes, palette and geometry.
F4-F9, F12 and V are highlighted on Cockpit & View. F4 keeps its separate Alt Exit
row, and the modifier callout explains target/missile references and F11 help
remains visible. The old F10-only callout leader was removed.

Chromium checks inspected all three sheets at 1920x1080 and 960x540. No leaf text
or callout overflow was detected. Tab, keyboard and hash navigation passed.
Cockpit & View PNG exports passed at 1920x1080 and 3840x2160 with the new labels.
ZIP, PDF and print implementations were unchanged and not separately exercised.
