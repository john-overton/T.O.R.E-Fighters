# Aircraft variety integration validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-05, on Linux in the `import-variety` worktree,
started from multiplayer at `7d25975b`. This is the initial 23-aircraft import
pass, bringing the catalog to 36 reviewed retail identities plus F/A-XX.
The [matrix](../spec/aircraft-variety.md) tracks remaining feature coverage.
Retail media identity and dependency hashes are in the
[source inventory](../formats/aircraft-variety.md); no original executable or
imported module was run. Retail gameplay comparison is unavailable.

## Checks completed

- Workspace formatting, all-target warnings-denied Clippy, locked Rust tests
  and build, Python tool tests, documentation headers and source/binary asset
  guards passed. Logs are local in `.local/variety-final-checks/`. New Markdown
  files were also checked directly for headers and local links before staging.
- The 23-aircraft selected extraction completed with 598 resources and no
  required dependency errors. A separate runtime import built the application
  pack, and all 36 retail identities loaded through `--sensor-summary` without
  an unavailable-aircraft result. No-radar and unarmed types stay explicit.
- Every new aircraft passed 1,200 application headless hybrid ticks, with
  finite motion, no crash and bounded fuel/energy output. The
  `flight-variety-*` battery scenarios check those results. Original profiles
  remain covered by their existing flight regressions.
- All 23 aircraft produced wrapped GPU exterior/device captures, stored only
  in `.local/variety-captures/`. Unsupported hook requests were correctly
  rejected; the capture setup was corrected to use each aircraft's capability.
  A V-22 flight-screen smoke presented successfully. Rotor/propeller phase
  corrections were separately inspected; see the
  [rotor geometry evidence](../formats/variety-rotors.md).
- Source-backed conventional and powered flight, source-geometry ground
  clearances, hover initialization at final mission mass/altitude and replay
  compatibility are recorded in the [flight baseline](variety-flight.md).
- All 37 selectable loadouts passed the creator's loadout stage, including
  unarmed aircraft, applicable gun/store checks and accepted-load restoration.
  F-14 tanks and F-4J installed-pod/ammunition separation have their own
  [feature evidence](external-fuel-tanks.md).
- AC-130 synthetic integration checks cover linked simultaneous starts,
  independent cadence/ammunition, release, group edits, empty/failed members,
  target loss, slew/arcs and obstruction. A separate renderer test compares
  the three animated barrel tips to the simulation's muzzle locations at
  several angles. Source records and fitted rules live in the
  [gunship contract](../spec/ac130-linked-guns.md).
- Input tests cover aircraft capability filters, unchanged old gamepad
  behavior, explicit custom binding precedence, calibrated axes, keyboard
  reset priority and saved profiles. The generated controls list passed.
  Keyboard-map review covered two viewport sizes, 1080p/4K PNG, all five ZIP
  sheets and five PDF pages; the README preview was refreshed.
- Protocol 7 carries the expanded aircraft identities, powered controls,
  actual device/gun poses, linked groups and tank loadouts. Session codec,
  round-trip, malformed-input and loopback tests passed. Capture format 3
  rejects incompatible captures explicitly. Mission replay format 2 retains
  reading of original format 1 recordings, as tested in the flight baseline.

## Scope and remaining acceptance

The source values and deterministic host tests establish a working initial
port, not original-game trajectory parity. No Windows/macOS runtime flight,
physical HOTAS/gamepad flight, audible review or human multi-machine LAN
session was performed in this pass. The detected local controller did not
provide a readable native input device; no driver or system settings changed.

Some moving surfaces, VTOL jet nozzle geometry, new cockpit live mirrors and
unlocated hook geometry remain static or incomplete. Separate tank/pod
geometry, in-flight fuel-tank jettison, fuller AWACS sharing/operator behavior,
carrier operations and recovered AT-2 guidance remain open. Rotorcraft use
explicit simple fits, without detailed rotor engineering or autorotation.

New-family AI behavior was not added. Original-roster AI goldens remain scoped
to their established aircraft; structural ownership changes only carry the
new source configuration and human handoff state.

## Final acceptance battery

The final targeted battery passed all 25 scenarios: the 23 aircraft flight
checks, F-14 tank selection and the full creator validator. The creator tested
37 player identities across 16 listed theaters and 75 layouts: 13,979 setups
started, 43 invalid setups were refused with a message, and zero problems were
reported. Creator input fuzz covered 147,000 events and screen input fuzz
76,000 events, both with zero problems. The full creator case took about
14 minutes; it is separate from the fast per-aircraft probes.

Local results are in
`.local/battery/20261005-114216-variety-final/summary.md`. The workspace Rust
run recorded 2,941 passing tests and 42 ignored tests, with zero failures.
These broad setup/input checks do not establish new-family AI tactics or
replace human handling, controller and multi-machine acceptance.
