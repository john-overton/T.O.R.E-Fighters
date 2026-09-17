# Aircraft animation and material validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation pass, 2026-09-16. Contract: [aircraft animation](../spec/aircraft-animation.md)
and [engine materials](../spec/engine-material.md). Source build identities and
import coverage are recorded in the [roster baseline](aircraft-roster-expansion.md).
This pass uses the same reviewed FA base shapes, not damage or LOD variants.

## Results

The new round afterburning outlets on MiG-21, MiG-23, MiG-29, Su-27 and Su-35
use the supplied runtime texture and the existing magenta-mask throttle grade.
F-22, Su-25 and A-4E remain excluded from this material. Single-engine split
faces share one set of texture bounds; twins map each complete outlet separately.

All seven added aircraft now have fitted rigid gear, moving control surfaces
and continuous brakes. MiG-21 and MiG-23 brakes split fitted strips from their
own source skin. MiG-29 and Su-25 upper/lower brakes use separate closing
angles about their forward edges. F-14's existing brake branch now hinges
continuously too. Existing applicable carrier hooks remain supported.
F-22 main bays support manual Shift+O, recorded/rebound input, and the documented
manual-combat request. Its exterior glazing retains the amber grade at 75% opacity, with a clear
cockpit view. The nearest glazing surface blends once over the opaque scene.

Validation on Linux, NVIDIA RTX 4070, Vulkan:

- Workspace formatting, clippy with warnings denied, locked build and all
  402 Rust tests pass. Synthetic tests cover rigid gear lengths, unchanged
  deployed geometry/UVs, fixed rudder skin, bay area/hinges/outward travel,
  clamshell closing direction, brake-strip limits, sweep values, 1-second bay
  travel/reversal/interpolation, unsupported commands and input roundtrip.
- All 40 Python tests pass. Source and both executable asset checks pass.
  Documentation headers pass. No retail-derived captures or inspector outputs
  are tracked.
- Display smoke passes. Local GPU captures inspect 0/0.5/1 device/control
  fractions on all seven, 0/0.65/1 throttle on the five new nozzle families,
  0/0.5/1 F-22 main bays, exterior canopy and cockpit, and separate brake poses
  on MiG-29, Su-27, Su-25, Su-35, F-22 and F-14. Captures and logs live in
  `.local/animation-gpu/`; checks use `.local/animation-*.log`.
  The 75%-opacity follow-up passes the same checks, with logs in
  `.local/canopy-*.log`. Matched F-22 exterior captures show detail through
  the orange glazing; the cockpit before/after comparison has zero changed pixels.

The bay geometry test caught and corrected opening direction. The GPU pass
caught a shader typo and an inspection-pose reset ordering error; final captures
show distinct closed, partial and open main bays. Close brake review replaced
shared upper/lower rotation with independent clamshell angles.

## Limits

Hinges, motion mixing and continuous schedules are fitted. Original linkage,
F-22 side bays, launch sequencing, damage/LOD variants and retail comparison
are not established. No flight-force, weapon launch eligibility or autonomous
behavior changes were made. Windows/macOS display execution and live physical
controller operation were not run in this pass.
