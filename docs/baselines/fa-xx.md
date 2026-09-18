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
