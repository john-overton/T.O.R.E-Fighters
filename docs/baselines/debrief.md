# Mission debrief validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-23, on top of `c3d61c3` (retail flight views, after pilot ejection). Covers
the [debrief spec](../spec/debrief.md).

## Visual comparison

The five pages of the retail reference result (`--quick-mission
--snapshot-state debrief-1` through `debrief-5`, DEBSCV background) were
rendered from a fresh import and compared with John's five retail screenshots
scaled to 640 × 480, side by side and in 3× and 4× nearest-neighbour crops.

- Background, clipboard, `?`, Cancel, OK, the striped default marker and the
  rocker land on the retail pixels.
- Page text: the original failed-mission comparison placed headings, body lines,
  the PLAYER/WINGMAN columns and table rows within one pixel of the supplied
  reference. First-page body text now follows the requested
  [centering correction](../spec/debrief.md#presentation). Fonts match by
  glyph width (for example "You failed this Quick Mission." is 168 pixels, 166.7
  measured).
- The page counter and PREV/NEXT labels match to within one pixel after moving
  PREV/NEXT up three pixels and dropping a redundant PAGEBOX overlay; every
  background already contains the box.
- Differences: the screenshots are softened by scaling, the retail Cancel label
  is the embossed retail button font where ours uses the creator's approved flat
  font.

Captures are local under the session scratch directory and are not committed.

## Behaviour checks

| Check | Result |
| --- | --- |
| Ledger unit tests: single resolution per missile, spoof not also failed, in-flight missile counted failed, host aims for AI gun rounds, bounded aims, kill credited once, last-attacker credit | Passed |
| Existing live-fire kill test extended: ledger launches equal the player's shot count; 2 hits for 20 damage; one kill of the class 0x80 target | Passed |
| Report tests: surviving target fails, all targets down succeeds, a friendly kill fails and is not a category kill, the wingman column and enemy fire follow owner and aim, an ejected enemy counts as destroyed and credited, an ejected wingman shows Ejected with 100% damage, flying alone gives an empty wingman column | Passed |
| Page text tests: `-` cells, rounded-down rates (`33% (340)`, gun `1%`), objective sentences, success/failure text section | Passed |
| Rocker test: one frame per 40 ms toward the pose, held at 00/04 while pressed, back to 02 on release, taps spring back | Passed |
| Rest frame: ROCKER02 against the retail screenshot rocker averages 22 colour levels of error per pixel; ROCKER00, used before, averaged 60 | Measured |
| Landing grade test: spawn touchdown and bounces ignored, gentle 100, firmer 50, average 75% | Passed |
| `MissionText` reader: sections, directives kept, bounds and malformed input rejected | Passed |
| Headless AI probe, 48,000 ticks intercept | Debrief line printed (`FAILURE`, `Destroyed 0 of 1`, 400 s); the probe's AI fired no shots, so it is not combat evidence |

## Repository validation

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check`, workspace Clippy with warnings denied | Passed |
| `cargo test --workspace --locked` | 1,346 passed, 3 existing ignored |
| Workspace build, Python tool tests, source and executable asset scans, documentation headers | Passed |
| `--smoke-test` on Linux/Wayland | Passed |

## Not run

- A human-flown mission ending in the debrief, including page turning, sounds
  and the return to the creator. This needs John's playtest.
- A live mission where AI aircraft fire, spoof or jam, so enemy-fire rows are
  covered by component tests only.
- Windows and macOS.
