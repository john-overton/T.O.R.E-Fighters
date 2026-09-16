# Additional FA aircraft acceptance

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-16. John requested FA sources throughout; the
shared response and animation fits are agent choices. No AI work or adapter
default changes. Source records: [F-14D](aircraft-f14d.md), [A-4E](aircraft-a4e.md),
[X-31 EFM](aircraft-x31.md). Player-visible rules live in the
[behavior spec](../spec/additional-aircraft.md). This is initial port acceptance,
not complete systems or retail parity.

## Reproduction

```sh
python3 tools/extract_assets.py --aircraft f14 --aircraft a4e --aircraft x31 --exclude-archive 'disc1/*' --exclude-archive 'disc2/*' --exclude-archive 'swpatch.lib' --out .local/aircraft-fa --validate-flight
cargo run --locked -p tore-app -- --import gameassets/fighters-anthology --import-only
cargo run --locked -p tore-app -- --aircraft f14 --free-flight
cargo run --locked -p tore-app -- --aircraft a4e --free-flight --researched-flight
cargo run --locked -p tore-app -- --aircraft x31 --combat-smoke
cargo run --locked -p tore-app -- --validate-creator
```

The source FA.EXE is the existing reviewed build identified in
[weapon source notes](../formats/weapons.md). The application imports aircraft
only from base FA_1/FA_2. The CLI preserves archive boundaries and the explicit
exclusions avoid the toolkit F-14 override. Imported data and captures stay in
ignored `.local/` or application data. No original module was executed.

## Coverage and evidence

- Shared extraction selected 219 resources without errors, with hashes and
  transitive dependencies. Unsupported executable symbols remain explicit report
  edges. This is the discoverable dependency closure, not complete module semantics.
- Each aircraft has its own typed model, with its source mass/thrust/fuel,
  envelopes, loading, controls and departure fields. Shared response constants
  and clearance are fitted. Three existing flight adapters remain distinct.
- All three load headlessly and render their own shape, textures and cockpit.
  Shape state differences identify each rig's parts. Geometry/normal rotations,
  device endpoints and sweep are tested independently of rendering.
- Source gun, missile, sensor, ECM, damage and feedback integration uses each
  aircraft's own stations. F14R.SEE and F4BR.SEE replace two Hornet-only radar
  assumptions. X-31's extra IR sensor is imported; additional IR control modes
  remain unimplemented. Low-speed auxiliary control now follows the FA rate
  and authority specification; X-31 plume deflection is fitted.
- F-14D and X-31 pass the five-class manual gun/missile scenarios, incoming
  missile/ECM, damage, ammunition and deterministic replay probes. A-4E passes
  gun scenarios and separate MK82/LAU61 release, safety/failure inhibition,
  ground contact and jettison probes. Ballistic checks do not establish area
  blast damage or same-altitude air-target interception.
- F-14D/A-4E cockpit mirror masks use reviewed source-art fills; X-31 has none.
  Cockpit, instrument and camera composition use the existing renderer.
- Engine/start/stop sounds follow PT references. Aircraft switching clears old
  engine/burner loops and queued effect voices. A-4E burner commands cannot
  enable afterburner audio, thrust, flames or feedback.

Local evidence: `.local/aircraft-flight-suite.log`, `aircraft-unit.log`,
`f14-combat.log`, `a4e-combat.log`, `x31-combat.log`, `aircraft-creator.log`,
and the per-aircraft cockpit/exterior captures under `.local/`.

## Limits

Linux rendering uses NVIDIA GeForce RTX 4070/Vulkan. Windows/macOS execution,
retail comparison, subjective audio audition, controller rumble and a manual
flight handling session were not performed. The attached controller was not
available as a readable evdev device during capture runs.

Native animation schedules, complete instruments, LOD/damage/shadow models,
full exterior-store placement and complete systems remain open. F-14's
quantized hook triangles need a fitted 0.25-source-unit root width repair. Sweep/gear/control hinges and F-14 vapor attachment are fitted.
Original X-31 paddle schedules, translational thrust redirection and additional
aerodynamic sweep effects are not established.
The restricted native-table flight path retains its existing limitations;
this pass does not claim new native-table trajectory acceptance.

## Validation results

All required repository checks passed: formatting, Clippy with warnings denied,
375 Rust tests, locked workspace build, 40 Python tests, source/binary asset
guards and documentation headers. Logs are in `.local/aircraft-final-checks/`.

The CLI extraction plus flight validation passed end to end: 39 hybrid flight
scenarios across three source PTs, including deterministic replay, loops, bank
symmetry, stall/spin policy, wind, fuel and explicit runway/contact cases.
Separate 1200-tick legacy and hybrid app probes passed for all three identities.
The X-31 test requires no spin entry, honoring its source spinEntry=2 instead
of weakening the spin requirement for aircraft that support it.

Creator validation passed all five identities, including edited fuel, compatible
alternate loads, empty stations and accepted-load restart. An isolated previous
cache was rejected for missing F14.HUD, refreshed from base FA media, and then
launched F-14 headlessly. Evidence: `.local/aircraft-cache-refresh.log`.

The required `cargo run --locked -p tore-app -- --smoke-test` passed. Fifteen
new-aircraft GPU captures passed: neutral/deployed exteriors, underside views,
1280x720 cockpits and 720x960 cockpits for each aircraft. Cockpit, deployed,
underside and tall-view images were visually inspected. F-14D and A-4E each
report three live mirror regions; X-31 reports zero. F/A-18D and Rafale C
regression cockpit captures also passed. Captures and driver logs are under
`.local/aircraft-gpu/`. A capture establishes that view and pose, not complete
animation travel, frame-rate or retail parity acceptance.

No commit or push was made.

## Handling and vector presentation validation

FA PT control values were read from the same three hashed sources above.
The executable evidence is the reviewed auxiliary scale/control consumers at
0x47b0e7..0x47b182 and 0x47c235..0x47c682, plus primary roll control at
0x47c18c..0x47c1e5. The behavior contract records the player-visible values;
continuous host integration and engine/fuel cutoff are explicitly distinguished
from recovered values. All three PTs have vtLimitDown=vtLimitUp=vtSpeed=0.
This is not evidence for a player-adjustable X-31 nozzle.

Local validation logs live in `.local/handling-checks/`. Synthetic tests cover
source response rates, release, throttle/speed/ground gates, both host adapters,
determinism, fuel-out cutoff, plume direction and preservation of its root.
The source-profile flight suite passes 13 scenarios on each of the three planes.
Retail comparison and subjective handling evaluation remain unavailable.

The handling pass passed all 379 Rust tests, 40 Python tests, formatting,
workspace Clippy with warnings denied, locked workspace build, documentation
checks and asset scans of the repository and both binaries. Linux/Vulkan smoke
rendering passed. Each new aircraft also completed 1,200 roll-maneuver ticks in
both legacy and researched flight. No new native-table trajectory acceptance or
manual low-speed visual inspection is claimed by these checks.

## Paddle and elevator animation correction

Implementation pass: X-31 source paddle face pairs now rotate around their
forward edges using the same fitted demand as the plume, including without
burner. A-4 tail face inspection identified a diagonal source partition that
crosses the fitted elevator strip. Clipping all eight tail faces at the hinge
replaces the incomplete rear-polygon selection. Geometry identities are recorded
in [objects and shapes](../formats/objects-and-shapes.md); the animation rules
are in the aircraft spec. These fits do not establish original animation laws.

All required checks passed: 381 Rust tests, 40 Python tests, locked build,
Clippy, formatting, source/binary asset scans, documentation and Linux rendering
smoke test. Synthetic regression cases cover paddle skins and fixed roots,
movement without burner, fixed cold nozzle, and A-4 elevator continuity across
source diagonals without rotating the forward stabilizer. A-4 pitched exterior
captures are in `.local/paddle-checks/`, alongside validation logs. X-31 live
low-speed paddle motion was not manually flown during this pass.
