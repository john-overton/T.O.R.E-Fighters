# HUD cleanup validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-17. [Behavior and fitted constants](../spec/missiles.md).

## Evidence

The locally retained 1999 EA/Jane's FA manual at
`.local/missile-update/manual.pdf`, pp. 83-84, 112, 116-119 and 129 was reviewed.
It describes weapon/count, range, closure, aspect, hit probability, range scale,
seeker diamond and IR/radar lock tones. Its targeting list does not establish
a release-lock key. No retail comparison was run. The user's two images are
presentation references for the requested opinionated behavior.

Four unsigned mono PCM resources were extracted with the repository reader from
FA_2.LIB, SHA-256
`fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`.
Extraction reported four resources and zero errors. Local previews and report
are under `.local/hud-cleanup/audio/`; no retail data is committed.

| Resource | Bytes | SHA-256 | Normalized RMS |
| --- | ---: | --- | ---: |
| &IRTRY.5K | 12030 | c482fa3fe5b05f1548cf1faf454d6e6b29fa8fe909e6d480d66d0f78d7ebe4a4 | 0.3275 |
| &IRLOCK.5K | 4928 | 9fdad0a22c29011d0d7b963db8d2d2af047e978f5e0354b4cdd2349775222f2c | 0.3283 |
| &RDRTRY.5K | 8896 | 7eda202c4339e6b856daa746a67e8646b9b92221147bf7de07795e6c66d3a720 | 0.3165 |
| &RDRLOCK.5K | 7439 | a1d210e70580eb9d72b8faa2298206962099c091572fa6184b70adb89dd0e5ea | 0.3877 |

Playback uses the existing `.5K` reader's 5,512 Hz interpretation. RMS uses
`(byte - 128) / 128`. Filenames suggest search/lock roles, but original sample
assignment, looping and gain rules are unknown. Current assignments are agent
choices. Original call-site research or comparison listening is the next step;
resource names alone do not establish retail behavior. A2G-specific IR tone
remains unresolved. No human listening comparison was performed.

## Validation

Synthetic regressions cover automatic bore entry, clearing designation, returning
to CUED, supported-radar restrictions, immediate active-seeker enablement,
circular search boundaries and stronger IR/radar returns winning over centering.
The seven-degree circular edge, centre weighting, stable ties and estimated hit
formula have numeric regressions. A reference case yields 58 percent centred
and 15 percent at the edge; missing intercept and out-of-envelope cases yield
zero. Existing missile identity, pause and cadence tests remain relevant.

The local AIM120.JT, AIM9M.JT and AGM65G.JT resources' `si_names` blocks were
inspected. The first and second strings supply the short and long labels listed
in [weapon formats](../formats/weapons.md). The loadout menu had been replacing
the short name with its longer description; the HUD now retains a separate
short label. No retail code was executed to infer label selection.

Validation on Linux with a display-capable host:

- `cargo fmt --all -- --check`, Clippy with warnings denied, workspace build
  and all 484 Rust tests passed using `--locked` where applicable.
- All 40 Python tool tests passed. Source and both built executable asset checks
  passed. Documentation header checks passed, including a separate check of this
  new untracked document.
- Runtime media import added all four samples successfully. The render smoke
  test presented a frame successfully.
- Inspected captures at 1280x720 for seven-degree radar bore, right look at
  12 degrees, and left look at 12 degrees with 1.5x zoom. The circle and weapon
  text move with the flight HUD without the old stationary clipping edge.
  The annotated-layout capture at 1280x720 verifies the additional 15% layout
  shrink, weapon/percentage alignment with speed, compact right-aligned range
  scale beneath altitude, and aspect in debug. BORE READY and EST HIT text are
  absent. The physical seven-degree bore is preserved.
  Earlier safe and 720x1000 IR captures remain local. Bore has a provisional blinking diamond without a target box; safe restores
  AGL, vertical speed and bank scale.
  Upper-right diagnostics no longer overlap the large instrument there.
- Minimum-engagement regressions cover AIM-120 and AIM-9 release below, at and
  above an imported 1,000-foot synthetic minimum, unchanged ammunition on
  inhibited release, blind-shot acquisition and damage rejection inside minR,
  and retained terminal tracking after valid acquisition. Surface-profile tests
  reject aircraft even with ground damage categories or airborne=false; AGM-65
  contrast is independent of fighter exhaust aspect/power. All seven explicit
  surface profiles reject A2A bore and aircraft designation for release.
  No interactive surface-designation path was validated or added.
- Synthetic checks cover the 2 Hz radar-diamond timing, radar crosshair pointer
  transforms, shifted upper-right instrument picking, recorded PCM looping,
  pause/mute, stronger mounted IR reacquisition and leaving the bore circle.

Captures remain local under `.local/hud-cleanup/`. No Windows/macOS runtime,
interactive crosshair screenshot, human sound comparison or retail parity test
was run. The original probability formula remains unknown. The bare HUD percentage uses the
specified fitted heuristic, with numeric regression tests, not a calibrated
claim of actual hit frequency.

