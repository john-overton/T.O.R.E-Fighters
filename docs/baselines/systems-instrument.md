# Systems instrument and damage validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Scope

Implementation pass on 2026-09-21, Linux with an NVIDIA Vulkan display.
[Behavior and fitted numbers](../spec/systems-damage.md),
[reviewed event identities](../formats/systems-damage.md).
The retail manual's printed pages 89 and 161 and John's supplied Systems image
establish the display and damage consequences. Original code was inspected as
bounded data only, never executed. No retail parity claim is made.

## Visual checks

All 13 selectable profiles produced Systems panel snapshots: F/A-18D, Rafale C,
F-14D, A-4E, X-31 EFM, MiG-29, Su-27, MiG-21, Su-25, MiG-23, Su-35, F-22A,
and the separately authored F/A-XX. Their imported tank capacities are used;
no aircraft is substituted to obtain the screenshot's fuel number.

Local captures are in `.local/systems-instrument/`. The visually inspected
`panels.png` compares healthy F/A-18D and Rafale panels, leaking oil/hydraulics,
critical fluid loss and fire. Healthy TEMP is 0%, OIL/HYD are 100%; F/A-18D's
default imported tank contributes 990 lb, while the default Rafale has no tank.
Ten seconds with oil pump, oil-line and hydraulic-line faults at full throttle
shows TEMP 32%, OIL 45%, HYD 90%. The critical fixture shows 100%, 0%, 0%.
A fire after 7.5 seconds shows TEMP 75%. No fault item or extra button is added.
All rows and values fit the content surface.

The GPU small-window capture `flight.png` confirms a real incoming hit produces
`Aircraft hit: 3% damage` in the existing bottom-center log while the Systems
panel retains its normal layout. The log uses its existing location and can
cover part of a bottom-row instrument while a notice is shown.

Example local commands:

```sh
target/debug/tore-app --aircraft f18 --instrument-page 7 --panel-snapshot .local/systems-instrument/healthy.ppm
target/debug/tore-app --instrument-page 7 --panel-snapshot .local/systems-instrument/oil-hydraulic.ppm --systems-preview 12,13,14 --flight-probe-ticks 1200 --flight-throttle 1
cargo run --locked -p tore-app -- --free-flight --no-audio --instrument-layout small --instrument-page 7 --capture-flight .local/systems-instrument/flight.ppm --smoke-test --combat-command damage
```

## Behavioral checks

Synthetic tests check healthy values, external-to-internal fuel debit and mass,
pressure/temperature progression, throttle's effect on overheating, the strict
25% compressor boundary, partial and total engine shutdown, inability to restart
a failed engine, flameout restart, fire and wound deadlines, landing treatment,
structural G failure, jammed devices, persistent frozen surfaces, control loss,
autopilot rejection, source RWR failure without losing radar and once-only fatal
combat events. Rendering/reporting does not advance timers. Existing deterministic
flight tests remain green. Fresh aircraft/reset state starts healthy.

An incoming-projectile regression now reproduces a 92% ownship nose hit through
the real collision/damage path with a synthetic projectile. It checks that the
aircraft remains alive, loses engine power and control authority, develops
hydraulic/oil faults, and emits no detached nose at either 92% or 99%. The final
hit to 100% releases exactly one fragment. Milestone tests cover the inclusive
25/50/75/90% boundaries, existing faults, disabled source entries and no repeated
milestone faults. The display caps surviving damage at 99% rather than rounding
99.x% to 100%.

Component tests isolate fluids, engine heating, pilot wounds and control authority.
A partial-wing flight test checks loss of lift and speed plus uncommanded roll,
without changing neutral aileron geometry. Regional fractions still feed aerodynamic penalties. The latest requested
presentation gate now hides all airframe marks and tears below 100%. Earlier GPU
captures under `.local/damage-followup/` showed complete noses at 92%/99%, nose
loss at 100% and a partial wing tear at 80%; that partial-tear appearance is now
superseded. Current captures in `.local/damage-visual-gate/` check intact surfaces
at 92% nose and 80% wing damage, retaining the destroyed appearance at 100%. Those `--damage-preview` images are
visual-only fixtures, so their healthy gauges are intentional; the incoming-hit
regression validates actual component damage instead.

The D-key test verifies reporting rather than damage injection, with modifier and
menu isolation. Reports and simultaneous events queue in the existing log instead
of silently replacing each other. The keyboard help and guides describe D's new
behavior. The legacy explicit developer/controller hit fixture remains available.
The D summary's text was unit-tested; physical keyboard input was not manually
exercised. Full-length pilot treatment, every rare fault combination and the
restricted native-table adapter with damage were not manually flown.

## Checks and limits

Required formatting, workspace Clippy with warnings denied, workspace tests and
build, Python tool tests, source/binary asset guards and documentation checks
passed. Cargo verification used `--locked` where applicable. Standard GPU smoke
and the small-window free-flight capture passed.

An extra full F/A-18D combat smoke passed subsystem selection/destruction,
incoming-damage feedback, all gun damage classes and all AIM-120 damage classes,
then failed at `source guidance probe did not launch`. This is the existing AGM
fixture targeting an ineligible aircraft, already recorded in the
[damage baseline](damage-smoke.md). It is not reported as a passing full combat
smoke and missile targeting was not changed by this pass.

Damage progression rates, thermal/pilot/fire deadlines, control penalties,
warning colors, display-failure choice and medical treatment remain fitted.
Source event identities and manual-described consequences do not establish exact
retail timing. Windows/macOS execution was not available on this host. All
captures and disassembly remain ignored; no retail assets were added.
