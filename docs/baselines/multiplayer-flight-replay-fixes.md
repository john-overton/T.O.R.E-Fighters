# Multiplayer flight and replay bugfix pass

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-01. Review base: multiplayer `f7cd05e5`; initial
diagnosis and captures used `b1f2a7cb`. The rebase adds the upstream test-isolation
and timing-test fixes, with no new flight changes. Worktree:
`T.O.R.E-Fighters-flight-replay-fixes`, branch `mp/flight-replay-fixes`.
Retail media were read locally; no original executable was run and no media
or captures are committed. This is host validation, not a retail parity claim.

## Findings and changes

| Item | Cause and implemented behavior | Limits |
| --- | --- | --- |
| Rudder | Yaw and slip drag existed, but lateral aerodynamic force and roll coupling were absent. Sim now damps body-side air velocity and rolls with slip. | Per-aircraft coefficients are fitted, [rules](../spec/lateral-flight.md). No autonomous decisions changed. |
| Nosewheel | Steering used an airspeed-dependent yaw addition that never faded out. Sim now uses horizontal ground speed, 90 degrees through 10 mph, fading to zero at 25 mph. Aircraft art uses the sim's angle. | The bounded tire model and 18 ft effective wheelbase are fitted. Existing gear pivots remain fitted. |
| Sonic boom | `8237dde9`, the spatial-sound introduction, skipped all pass detection for the own aircraft and excluded Mach 1 from the cone while also suppressing its ordinary pass. `ad4aac9a` changed ownership to a real aircraft id but retained those exclusions. External own-aircraft passes now use cone detection; Mach 1 is included. | No later deletion of the boom was found. Existing supersonic other-aircraft detection above Mach 1 was present and is now also tested through PCM output. Sound was not auditioned by ear. |
| Overspeed | The former rule destroyed aircraft immediately at 1.5 times top speed. The requested timer, seeded 25 percent rolls at 6 to 9 seconds, and guaranteed loss at 10 seconds now live in sim. | Existing Invulnerable behavior remains. Protocol 6 includes the timer and legacy RNG in exact snapshots, so all connected builds must update together. [Spec](../spec/overspeed.md). |
| Target window | `b68977b1` moved mesh vertices to a nearby render origin, but framing still treated them as world positions. Framing now restores the origin before projecting. | Synthetic translated-origin tests and local GPU captures cover close and long range. F7/F8 use their existing relation-camera rules. |
| Pilot texture | Imported pilot panels have opposing skins at the same depth with different UV regions. The pilot builder emitted both without the normal-facing test used by aircraft. It now selects the facing skin. | UV normalization and atlas binding were already correct. One pilot batch is submitted once per scene pass; there was no second pilot draw. Source pose geometry remains unchanged. |
| Replay weapons | The empty-space menu built only aircraft rows; F12 chose the newest missile instead of keeping a cycle identity. Active weapon rows now refresh at the playhead and F12 cycles live missile ids, advancing past expired ones. | Guns are excluded, bombs and rockets are listed, F12 cycles missiles only. Alt/Ctrl references retain their existing meaning. |

The rebase's strict hosting check exposed a client initialization discrepancy:
wind-induced roll made the scripted pilot correct its bank immediately on joining.
The client applied this new input to its initial catch-up ticks, while the host
had already flown part of that interval with neutral input. Three short runs
reproduced a tiny tick-8 correction. A synthetic held-stick case at 60 ms RTT
reproduced three mismatches and a 1.69-degree startup correction. The client now
fills its initial forecast with neutral input and retains commands for the next
tick. Flight dynamics, host scheduling and strict test assertions are unchanged.
This is an integration fix in `tore-session`, not autonomous-behavior work.

## Validation

- Workspace formatting, strict clippy, build and all Rust tests passed on Linux.
  Supplemental replay tests cover overlapping missile and bomb lifetimes,
  spawn, expiry, wraparound, no-live-object fallback, backward seeking and
  a ground target present only in the recorded world.
- Apple Silicon CI exposed 19 stale saved fingerprints for the deliberately
  changed legacy/hybrid flight paths and two host mission scenarios that use
  those paths. Updated values were measured by the
  [macOS job](https://github.com/john-overton/T.O.R.E-Fighters/actions/runs/36956222636/job/110679640551).
  Native-flight, pure decision and standalone controller fingerprints stayed
  unchanged. All 1,004 other sim tests passed there. Linux checks repeatability
  but does not compare against the Apple Silicon saved bit patterns.
  After those sim comparisons passed, the subsequent
  [world test job](https://github.com/john-overton/T.O.R.E-Fighters/actions/runs/36958049119/job/110685347896)
  measured the corresponding mission-tick fingerprint, now `f0cb0dc46965e136`.
  Its other 329 world tests passed. The mission tick's scheduling and calls are
  unchanged; its named behavior fields include the requested flight response.
- Synthetic tests verify engine-off rudder motion, mirrored rudder input,
  nosewheel speed boundaries, overspeed roll times for 99 seeds in both
  adapters, timer reset, Invulnerable, and exact snapshot restore before a roll.
- Synthetic sound tests check Mach 1, 1.01 and 2 passes for own and other aircraft.
  App tests feed the sonic recording into the mixer and require audible PCM.
- The targeted battery passed 58/58 scenarios, plus 2/2 F8 captures: all 14
  aircraft's overspeed loss, invulnerability and dive scenarios; F/A-18D and
  Rafale C rudder and taxi scenarios; target and ejection captures.
- Unpowered F/A-18D after 3 seconds of full right rudder: heading 13.059 degrees,
  flight-path heading 8.444 degrees, bank 3.823 degrees and lateral velocity
  104.891 ft/s. The aircraft remained airborne and lost energy.
- Target captures at 500 and 60,000 feet visibly show the target at comparable
  size in its window. F7/F8 subjects are visible. Pilot seat, freefall and chute
  captures were generated for F/A-18D and Rafale C; the F/A-18D images were
  visually inspected, including enlarged before/after pilot textures.
- Twelve additional takeoff, landing, spin and spin-recovery scenarios passed
  for F/A-18D and Rafale C, checking the side-force and tire changes against
  existing flight behavior.
- All four strict real-time tests added by the upstream rebase passed after
  the startup fix, with the zero-correction assertions unchanged. The full
  session suite and the simulated 60/120 ms held-input regression also passed.
- All 14 sustained full-power sprint scenarios passed with the new overspeed
  timer, bringing the targeted battery total to 86 passing runs.
- The GPU smoke test passed through `tools/agent-run.sh`. Local evidence is in
  `.local/flight-replay-fixes-battery/` and `.local/tmp-flight-replay-fixes/`.
  No window was launched on the user's workspace.

Retail comparison remains unavailable. Cross-platform CI status is reported in
the session, rather than asserted by these Linux results. No merge into
multiplayer is authorized for this pass.
