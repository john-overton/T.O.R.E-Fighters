# F/A-XX implementation validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-18. Contract: [F/A-XX](../spec/fa-xx.md).
The source donor is the existing reviewed F-22 import, identified in the
[roster baseline](aircraft-roster-expansion.md). No retail assets are committed.

Validated on Linux with an NVIDIA RTX 4070 Vulkan renderer:

- All 846 Rust tests passed, including synthetic selection identity, fin removal,
  split flap direction, fixed hinges, texture coordinate preservation, and
  donor-matched yaw response and bay support. Hook tests cover stowed startup,
  F/A-XX-only capability, three-second travel, reversal, interpolation, rigid
  geometry, complete concealment and the fully deployed tip at source z=-23.
  Local F22.SH projection with gear word 5e0e=1 adds 12 gear faces; their
  lowest vertices also have z=-23. The synthetic hook test checks this plane
  within 0.0001 source units.
- Formatting, workspace Clippy with warnings denied, locked workspace build,
  all 40 Python tests, documentation headers and source/binary asset checks passed.
- GPU smoke test presented successfully. Captures inspected: F-22 neutral,
  F/A-XX neutral, full right rudder and full left rudder. Fin removal and
  unilateral split flap motion are visible. Underside captures also confirm the
  inset hinge placement with the extended tip meeting the wheel-bottom plane.
  Current deployed capture: `faxx-hook-wheel-plane.ppm`. Stowed concealment
  is also covered by the animation test. Captures remain under
  `.local/`.
- F/A-XX ran 120 ticks in default hybrid, legacy and restricted native research
  modes without crashing. The research probe used local tables from
  `.local/weapons-research/native/tables`.
  Quick Mission aircraft selection was captured with F/A-XX selected.

Retail comparison was not run. Separate split-leaf drag, roll coupling and
finless stability are not modeled. Damaged-body fin masks were checked against
local decoded source polygons; an in-flight damage transition was not captured.
Existing wing integration only receives the new identity and donor configuration;
no autonomous behavior laws were added or tuned.

## F-22N donor, 2026-09-22

Implementation mode. John moved the concept donor to the retail F-22N and added
the F-22N itself as a selectable aircraft. Donor identities and the parsed PT
differences are in the [packaging baseline](fa-xx-packaging.md#f-22n-donor-export).

Validated on Linux with the same NVIDIA Vulkan host:

- The F22N.SH rig gate measured 20146 code bytes, 248 neutral faces and the
  seven state words 5e70/5e7c/5e82/5e8e/5e9a/5ea0/5ea6; branches add 8 burner,
  12 bay, 4 brake, 12 gear and 2 hook faces, all textured `_F22N.PIC`. The rig
  error messages now print measured values so the next donor is easy to review.
- All 1028 Rust tests passed (154 tore-formats, 626 tore-sim, 208 tore-app, the
  rest in examples and integration tests), plus 69 Python tests, formatting,
  Clippy with warnings denied, the locked workspace build, source and binary
  asset checks and documentation headers. New synthetic tests cover the native
  blade: exact source geometry at full extension, hidden at zero, rigid rotation
  about (0,-9,-9), tip at z=-23 rising monotonically as it stows and above the
  belly plane when nearly stowed. Fin and flap tests use the F-22N addresses
  and separately check the F-22A's own fins still draw.
- Captures under `.local/faxx-f22n/`, inspected: F/A-XX neutral is tailless
  and the F-22N keeps both fins; from above, full right rudder opens only the
  right inboard flap as two leaves and full left only the left; the F-22N's
  rudder panels deflect at full rudder and are clean at neutral; from below the
  striped native hook hangs to the wheel plane at extension 1, sits nearly
  flush at 0.5 and is absent at 0, identically on F-22N and F/A-XX; the F-22N
  bay opens with a solid belly behind the doors, matching the F-22A capture;
  the F-22A neutral capture is unchanged.
- Headless 120-tick runs for f22n, faxx and f22 in the default hybrid,
  `--legacy-flight` and `--native-flight-tables` modes all completed without
  crashing with identical end states within each mode. The GPU smoke test
  presented successfully. Media were re-imported once into a separate
  `TORE_DATA_DIR` because the cache now requires the F22N resources.
- The F/A-XX now takes F22N.PT values: the flight suite departs earlier in the
  stall scenario (tick 7215 against 7965 for the F-22A) and ends the spin
  scenario at 262.17 kt against 264.19. Donor values, not tuning.

Retail comparison was not run. An in-flight damage transition was not captured.
Original FA flight of the new export is pending John's Windows check.

## F/A-18 panel grey, 2026-09-22

The runtime F/A-XX applies the same 156/146/147 to 150 remap to its intact rig
faces, both damaged bodies and both fragments; the F-22A and F-22N are
unchanged. A synthetic test checks the map and that trim, flame and
texture-only colours are kept. Local captures `.local/faxx-color/f18.png`,
`faxx.png` and `f22n.png`, taken from the same external view, show the F/A-XX
airframe in the F/A-18's light grey while the F-22N keeps its darker stock grey.
