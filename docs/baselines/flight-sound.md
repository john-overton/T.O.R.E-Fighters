# Flight sound validation, 2026-09-23

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode established recording selection; implementation mode delivered
those recordings and the requested acoustic extensions. Worktree
`T.O.R.E-Fighters-sounds`, branch `sound-research`, base `02242a1`.
[Behavior](../spec/sound.md), [acoustic rules and limits](../audio.md), and
[build identity and static source evidence](../formats/sound.md) are separate.

## Source and import checks

Fresh bounded `llvm-objdump` disassembly of InitSound and ServiceSounds matched
all 938 previously inspected instructions. Both source file hashes matched the
reviewed identities. No original code was executed. Extraction selected nine
PCM resources from FA_2.LIB with zero errors; all raw bytes and lossless local
WAV previews stay under `.local/sound-research/`.

The normal app importer completed into `.local/dev-profile/`. Its report includes
IR1, AIRPASS, MPASS and SNCBOOM. A separate bounded weapon decode confirmed
AIM9M has signature 2 and flags `0x1204f`; AGM65G has signature 2 and flags
`0x2a06f`. The reviewed bit therefore sends the former to IR1 and the latter
to the radar-named tone pair without changing guidance eligibility.

## Automated checks

All required checks passed: format, workspace Clippy with warnings denied,
workspace tests, workspace build, Python tool tests, repository asset guard,
both executable asset guards, and documentation validation. Rust: 1258
passed, three existing GPU tests ignored. Python: 75 passed. No dependencies,
controls, flight adapters, aircraft identities or AI behavior changed.

Synthetic checks cover the 10-second delayed explosion, moving-listener
interception, inverse-distance pressure, fade limits, altitude-dependent sound
speed, bounded pending/active queues, independent simultaneous positions,
repeat suppression, camera switches, formation neighbors, Mach-cone arrival,
external-only ownship transition and rearming. Mixer tests cover the corrected
IR recording, playhead continuity on lock, percentage gain, distinct surface IR,
spatial stereo, moving-listener fade/pan, mute and pause without catch-up.
Bore-mode IR tests exercise the no-designation path with radar off: live tracked
percentage, half-volume acquisition, same-target lock, candidate switching,
empty bore, too-weak provisional returns, track loss, terrain masking, safety,
ammunition/failure gates, death and release. Uncued radar audio stays silent.

A 1,200-tick isolated F/A-18D headless flight completed with hybrid flight,
436.693 knots, altitude 5,014.249 ft, and no crash. The window smoke test passed.
A bounded 180-frame active external F/A-18D run opened the default CPAL device
at 44,100 Hz stereo and exited without audio errors. This verifies startup and
stream operation, not human listening. Existing missing optional score phrases
were reported without substitution; the optional head-tracker UDP port was
already occupied. That did not prevent the run.

## Offline recordings

An ignored Rust probe uses the production PCM resampler, seeker envelope,
spatial scene and acoustic model with the imported recordings. Output is
48,000 Hz signed-16 stereo. No device is needed. First nonzero audible samples
are measured above one signed-16 sample unit:

| Scenario | First audible time | Peak absolute sample |
| --- | ---: | ---: |
| Explosion at 11,150 ft, stationary listener at sea level | 10.00052 s | 0.02283 |
| Aircraft at 700 ft/s, starting 2,500 ft left, passing 200 ft above | 3.75004 s | 0.35355 |
| Missile at 3,000 ft/s, starting 8,000 ft left, passing 100 ft above | 2.75838 s | 0.27781 |
| Aircraft at 2,230 ft/s, starting 8,000 ft left, passing 1,000 ft above | 4.37525 s | 0.19133 |

The growl preview has five two-second stages: tracking at 0, 25 and 50 percent,
then lock at 50 and 100 percent. It retains the IR1 playhead across every stage
and peaks at 0.29810. Files include `ir-growl-quality.wav`,
`explosion-11150ft.wav`, `aircraft-pass.wav`, `missile-pass.wav`, and
`mach-two-pass.wav`, all ignored local retail derivatives.

Retail playthrough comparison is unavailable. Human listening acceptance,
Windows/macOS execution and full acoustic pressure calibration were not done.
This model's documented refraction, wind, reflection and occlusion omissions
remain fitted limits.
