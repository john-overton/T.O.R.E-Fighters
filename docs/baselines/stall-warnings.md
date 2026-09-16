# Stall warning and default-mode validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-16. The normal launcher previously selected legacy
flight, which has low-speed lift reduction but no explicit spin state. HUD and
audio did not consume departure state. John requested the researched default.
[Behavior contract](../spec/stall-warnings.md).

## Reproduction and results

`target/debug/tore-app --aircraft a4e --headless-flight 600 --maneuver spin`
now reports hybrid mode, Spinning, direction +1, 120.975 knots TAS and 4686.418 ft
MSL after five seconds from the 5000-ft fixture. Explicit `--researched-flight`
matches. `--legacy-flight` reports legacy, Warning and spin direction zero.
The source A4E.PT is the hashed media in [A-4E acceptance](aircraft-a4e.md).
Its 13-scenario flight suite, including spin entry/recovery, passes.

All 384 Rust tests, 40 Python tests, formatting, workspace Clippy, locked build,
documentation and source/binary asset checks passed. Synthetic checks cover
shared alert state, ground/crash suppression, engine-off operation, sample
selection, continuous loop output and pause/effects mute. Both imported stall
samples are now required by the runtime cache validator.

Linux/Vulkan smoke rendering passed. The five-second A-4 spin cockpit capture
shows STALL and ENGINE OFF together. Logs and capture are under
`.local/stall-warning-checks/`. Audio scheduling/mixing was tested with synthetic
PCM; subjective audition and physical controller spin entry were not performed.
Native-table trajectory tests were not rerun. Its separate explicit selection
and restrictions remain in place. No retail parity claim is made.
