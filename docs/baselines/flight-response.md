# Flight response, steps 1–3 — 2026-09-15

## Scope and result

Recorded the adapter producer/response/departure pass for FA F/A-18D and
Rafale C. This completes selected components and regression checks, not native
steps 2–3. The earlier completion wording is superseded by the
[provenance policy](../behavior-provenance.md) and current native recovery plan. The legacy adapter remains the default; native-derived departure
states and stall attenuation belong to the explicit hybrid adapter. Legacy
low-speed lift loss remains fitted and does not claim warning/spin support.
Maneuver audio/rumble (step 4) and retail/platform acceptance (step 5) remain open.

### Source identity

| Input | SHA-256 |
| --- | --- |
| FA.EXE | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |
| FA.SMS | `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0` |
| FA_2.LIB/F18.PT | `2cd308c5b94b4560726c35c37d2f0735db179a6e9c7cd0ddef66ab70dc303abd` |
| FA_2.LIB/RAFALE.PT | `fca3e30c372b3cabffffa95d399c023da49d7ce21d21936d704703786f9eb5b2` |

The static pass requires the reviewed EXE **and** SMS hashes before emitting
fixed-address slices. Symbols from other builds remain inventories only.
Native modules were disassembled as data, never executed. Source channel facts
and producer/consumer addresses are in [native flight research](../formats/native-flight.md#response-producerconsumer-ledger--2026-09-15).

## Reproduction

From the repository root, with user-owned media:

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/flight-response/native
python3 tools/extract_assets.py --aircraft f18 --exclude-archive 'disc1/LHX/*' --out .local/flight-response/validated-f18 --validate-flight
python3 tools/extract_assets.py --aircraft rafale --exclude-archive 'disc1/LHX/*' --out .local/flight-response/validated-rafale --validate-flight
TORE_RESPONSE_TRACE=.local/flight-response/traces cargo run --locked -p tore-sim --example response_probe -- .local/flight-response/validated-f18/FA_2.LIB/F18.PT .local/flight-response/validated-rafale/FA_2.LIB/RAFALE.PT
```

Use fresh output directories or the extraction tool's explicit overwrite option
when repeating extraction. `TORE_RESPONSE_TRACE` is optional; omit it for a short
summary. The probe records initial complete configuration, explicit flat terrain
at zero MSL, zero wind and adapter atmosphere/lapse. Per-tick records include
inputs, pose, world velocity, geometric AoA/slip, fuel/devices, response snapshot,
departure timers, fractional clock and RNG state. No live atmosphere sensor is
claimed. All source-derived traces/extracted resources remain ignored locally.

Before-edit traces used the same eleven core scenarios for both aircraft/adapters
in `.local/flight-response/before/`; their summaries are `before.txt`. The final
probe adds low/high speed pull/push, devices and payload (17 scenarios per pair),
and full-loop checks for both vertical attitudes. Final traces are in `after/`;
`after-summary.txt` also includes the four loop checks. Each ordinary scenario
replays every tick from cloned initial state with exact equality assertions.

Core conditions: 15,000 ft, 450 knots, clean/full internal fuel, throttle 70%;
controls held ten seconds and then released for twenty. Low/high probes use
300 fps at 5,000 ft and 1,500 fps at 30,000 ft. Device probes deploy gear, flaps
and brake together; payload uses half the remaining legal mass allowance.
Stall/spin start at 180 fps, engine off. Spin probes use a small signed bank to
select direction, then negative pitch/opposite rudder from ten seconds onward.
Separate tests cover exact level/random ties and threshold/timer boundaries.
Loops use full throttle/afterburner, continuous pull, at most ninety seconds.

## Measurements and regression evidence

| Quantity | F/A-18D | Rafale C |
| --- | --- | --- |
| Before pull peak, filtered lift-command G | 5.022 | 6.454 |
| After pull peak, achieved normal-force G | 5.087 | 6.514 |
| Before hybrid stall minimum filtered G | 0.454 | 0.441 |
| After hybrid stall minimum achieved G | 0.011 | 0.011 |
| Loop completion, either adapter | 3,615 ticks | 2,866 ticks |
| Hybrid left/right spin | Both enter and recover | Both enter and recover |
| Hybrid stall trace | Warning → stalled → normal | Warning → stalled → normal |
| Roll response maximum, legacy / hybrid | 1.8 / 4.1888 rad/s | 1.8 / 4.1888 rad/s |

The before/after G columns intentionally measure different channels: previously
`State::g` was the filtered lift command. It now measures applied aerodynamic
specific force projected on aircraft-up, excluding gravity and ground impulses.
Small normal drag components can make it exceed lift demand. The command and
attenuated lift are separately retained in `Maneuver`. Ordinary roll/release
reaches zero; body-rate telemetry also represents spin overrides correctly.

The 30-second fixed-input probes are diagnostics, not recovery autopilots.
Long push and some roll cases hit terrain, including before the change; these
finite deterministic impacts are recorded rather than labeled successful flight.
The separate loop and spin gates require completion/recovery, and the existing
13-scenario hybrid suite retains its landing, crash, takeoff, wind and fuel gates.

Synthetic tests verify:

- Applied acceleration reproduces achieved normal G, for both models/adapters.
- Body-rate projections reconstruct attitude rotation near either vertical and
  inverted attitudes, independently of Euler wrapping.
- Left/right rudder motion is symmetric; release decays; fitted sideslip drag
  removes energy. Full-loop probes cross both vertical attitudes.
- Both native entry profiles' inclusive rudder/strict pitch thresholds;
  recovery speed/pitch/rudder boundaries, both signs, lock inhibition and neutral
  rudder; interrupted recovery resets its timer.
- Spin entry precedes dispatch, new entry resets intensity, severity uses the
  pre-increment stall timer, and ground state clears departure.
- Existing fixed-rate replay, wind-advection, input isolation and pause tests.

Linux validation: formatting, warnings-denied workspace Clippy, workspace tests,
locked build, Python tests and repository/app/extractor asset guards passed.
Both `--validate-flight` extraction workflows passed all 13 hybrid scenarios.
The response probe passed for both identities and adapters. Linux GPU smoke
checks presented Quick Mission, the viewer, and 400-tick pull maneuvers for
F18/Rafale in both adapters on the NVIDIA GeForce RTX 4070 (Vulkan). These are
startup/presentation regression checks, not visual retail acceptance. No renderer,
layout or frame scheduling changes were made; wide/tall and performance gates
are unchanged.

## Remaining acceptance boundaries

The continuous adapter still uses a fitted clean-envelope stall gate and severity
reference speed. Native current-G envelope classification, difficulty/VTOL
producers, damage/device authority and complete force ordering remain separate
research gates. Native movement-angle pitch/roll fall, tumble deadlines and
random draws are not connected without their complete contracts. Spin yaw is
still fitted continuous coupling; native movement-roll, speed slew and display
bank/AoA offsets are diagnostic translations, not transplanted onto body Euler
angles. Neither aircraft's supported `spinExit=-2` needs throttle for recovery;
other variants' lock/lifecycle support is not certified by these aircraft tests.

The sideslip loss and rudder/yaw alignment are fitted independently owned model
parameters. Native display-slip is not relabeled measured sideslip; no inferred
sound intensity or invented native rudder-to-roll law supplies forces.
There are no matched retail maneuver recordings, Windows/macOS runtime checks,
or physical controller/audio acceptance in this evidence. Whole-tick native
parity, exact global RNG ordering and cross-platform bit identity remain open.
