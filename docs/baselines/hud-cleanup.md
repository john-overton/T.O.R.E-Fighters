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
  and all 497 Rust tests passed using `--locked` where applicable.
- All 40 Python tool tests passed. Source and both built executable asset checks
  passed. Documentation header checks passed, including a separate check of this
  new untracked document.
- Runtime media import added all four samples successfully. The render smoke
  test presented a frame successfully.
- Inspected captures at 1280x720 for seven-degree radar bore, right look at
  12 degrees, and left look at 12 degrees with 1.5x zoom. The circle and weapon
  text move with the flight HUD without the old stationary clipping edge.
  The earlier annotated-layout capture at 1280x720 verifies the additional 15% layout
  shrink, weapon/percentage alignment with speed, compact right-aligned range
  scale beneath altitude, and aspect in debug. BORE READY and EST HIT text are
  absent. The physical seven-degree bore is preserved.
  Earlier safe and 720x1000 IR captures remain local. Bore has a provisional blinking diamond without a target box; the former safe-flight layout restored
  AGL, vertical speed and bank scale. Current visibility follows the
  [expanded layout](../spec/hud-layout.md).
  Upper-right diagnostics no longer overlap the large instrument there.
- Minimum-engagement regressions cover AIM-120 and AIM-9 release below, at and
  above an imported 1,000-foot synthetic minimum, unchanged ammunition on
  inhibited release, blind-shot acquisition and damage rejection inside minR,
  and retained terminal tracking after valid acquisition. Surface-profile tests
  reject aircraft even with ground damage categories or airborne=false; AGM-65
  contrast is independent of fighter exhaust aspect/power. All seven explicit
  surface profiles reject A2A bore and aircraft designation for release.
  No interactive surface-designation path was validated or added.
- Radar-power regressions cover disabling mounted radar cues, rejecting the bore
  toggle, unguided release for active and supported radar missiles,
  and no reacquisition after power returns. Armed IR remains in bore with radar
  power off without designation; minimum-range inhibition remains intact.
  Radar bore tests cover selected scope range and exact aircraft tracking limits.
  IR handoff checks select a weaker target outside bore, inhibit the bore toggle
  while designated, launch against that target, then clear designation and
  reacquire the stronger bore return without changing the airborne shot. Passive IR channel selection remains
  distinct from the power switch. Tape version 5 round-trips that distinction;
  older tapes retain their prior power-gate interpretation. Default search/lock
  amplitude is doubled from 0.15 to 0.30 for PCM and synthesized fallback.
  No human listening comparison was performed for the volume change.
- Latest 1280x720 captures in `/tmp/hud-retail-layout.ppm` and
  `/tmp/hud-ir-final.ppm` check the retail-reference ARM/count/percentage rows,
  range scale inside altitude, and radar R/C/A below altitude. The new bore
  half-angle is five degrees. Surface weapons still reject practice aircraft.
- The 1280x720 `/tmp/safe-hud.ppm` capture confirms a selected radar-contact
  box with master arm SAFE and the former AGL/VS/bank arrangement, no weapon HUD cues,
  and zero shots fired.
- The 1280x720 `/tmp/hud-spacing.ppm` capture checks the altitude-box gap
  and separation between tape labels and weapon rows. TARGET DESTROYED is
  excluded by the HUD warning filter; its release inhibit is unchanged.
- Synthetic checks cover the 2 Hz radar-diamond timing, radar crosshair pointer
  transforms, shifted upper-right instrument picking, recorded PCM looping,
  pause/mute, stronger mounted IR reacquisition and leaving the bore circle.

Captures remain local under `.local/hud-cleanup/`. No Windows/macOS runtime,
interactive crosshair screenshot, human sound comparison or retail parity test
was run. The original probability formula remains unknown. The bare HUD percentage uses the
specified fitted heuristic, with numeric regression tests, not a calibrated
claim of actual hit frequency.

## Expanded HUD and startup review

Implementation mode, 2026-09-21. Sol agents implemented the flight layout and
startup routing; the root agent integrated weapon readouts, target bounds and
NAV presentation, then reviewed the code and captures. The requested constants
have one home in the [HUD spec](../spec/hud-layout.md).

Expanded-layout evidence is `.local/hud-layout-review/`; compact ladder and
boxed-only readout checks are in `.local/hud-compact-review/`. Captures cover ground NAV and
airborne gun/SAFE starts for all thirteen selectable identities, armed guns,
missile readouts, a target below the former lower boundary, a banked NAV view,
and wide/tall windows. The fixed aircraft datum and surrounding speed/altitude tape marks and numbers
are absent. The flight-path marker remains, and the boxed current values move
down twelve reference pixels. Ladder spacing is compressed by 25%, with rung
width, five-degree labels and bank orientation preserved. The ladder window
subsequently trims from 208 to 166 pixels high, preserving its upper edge. NAV retains the selected-target cue. AGL/VS are absent in
ordinary NAV, including an inactive armed ILS, and retained for active ILS.
The common layout is not clipped to each aircraft's differently shaped glass;
low rows can overlay cockpit frames on aircraft with shorter HUD apertures.

Review corrected negative TAS graduations exposed by the extended tape and
ensured inactive ILS does not restore AGL/VS during normal NAV. Startup review
caught an unconditional default slot overriding canonical gun selection and a
default NAV resolution ignoring an explicit diagnostic NAV=0. Explicit options
now take precedence. A synthetic reordered loadout verifies gun selection and
master-arm safety independently of slot zero. Bank and targeting regressions
cover full rolls and the expanded lower boundary.

No retail comparison or Windows/macOS runtime check was run. Runtime checks and measured results below refer to Linux/Vulkan.

All required checks pass: 948 Rust tests, 68 Python tests, formatting,
warnings-denied workspace/all-target Clippy, locked workspace build, source and
both executable asset guards, and documentation headers. Two optional GPU unit
tests remain ignored; the explicit display smoke passes on NVIDIA RTX 4070/Vulkan.
Thirty-eight captured scenarios cover the roster and mode/layout combinations;
final NAV/GUN label captures confirm the displayed startup mode.
