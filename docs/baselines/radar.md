# Radar research validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-16, followed by an implementation-mode validation pass on
the same day, recorded in the final section. The research sections are local
static inspection only. No retail execution or matched retail comparison is
claimed anywhere on this page.

## Inputs and method

Recomputed SHA-256 on the supplied installation:

- FA.EXE: `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`
- FA.SMS: `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0`

Read nine distinct radar SEE records and the twelve imported aircraft PT
hardpoint lists from the existing ignored
`.local/weapons-research/supplement/FA_2.LIB/` extraction, whose report identifies
the source FA_2.LIB records, offsets and decoded hashes. Original extraction
method is in [weapon research](weapons-research.md). Inspected the existing full
FA disassembly and SMS inventory under `.local/weapons-research/native/` plus
reviewed FOV/lock spans under `.local/systems-pass/native/reviewed/`.

The ignored reference checkout's sensor notes were leads only. Its engine was
not reused and its other-title values were not substituted for FA records.

## Reproduction and follow-up results

The bounded local audit is `.local/radar-research/audit.py`. From the repository
root, `python3 .local/radar-research/audit.py` asserts the FA.EXE hash, resolves
PT default hardpoint pointers, decodes SEE fields and reads the range/mode tables
through PE section bounds. It writes ignored `roster.json` with per-record hashes.
This is local research tooling, not a new runtime dependency or committed retail
fixture. Recreate the extracted inputs using the linked weapons baseline.

Observed: twelve aircraft, one radar each, nine distinct radar records. The
follow-up also reads OBJECT sigs[2] and sigs[3] from all twelve PT prefixes using
the existing schema field order. Their IR/radar values are listed in the spec;
these are relative game statistics, not measured physical cross sections. All
bindings agree with the existing Rust identity mapping. Extra IR/laser records
are listed separately in the spec. Static inspection confirms the six-position
range ladder, the display-mode labels, automatic range-versus-track mode selection
and the ordinary RWS target-cycle restriction. No original module was executed.

The USNF manual is supporting context only; the FA tables and consumer checks
establish the new range/mode claims. Complete mouse/history/support transitions
remain unknown. The scope reset's 10-mile default and the app's different range
ladder and default were a real implementation gap at the time of this research,
not another radar profile; the implementation pass below closed it.

## Results and limitations

The [spec](../spec/radar.md) records the measured nominal ranges, signatures and
parameters. The [component guide](../radar.md) follows John's subsequent
request for an authored signature-based model, with agent-authored look-down,
notch, equipment-generation and jammer tuning, plus the subsequently requested
jammer-generation matchup and directional scope interference. The public Naval
Air Warfare Center handbook is qualitative background for this authored model,
not evidence of retail behaviour; the component guide links the relevant section.
None of its gameplay constants was validated against retail. They were
implemented in simulation by the pass recorded below, which likewise makes no
retail comparison.
[Source notes](../formats/radar.md) record the newly inspected look-down and
Doppler branches, unresolved flags and next evidence needed. A provisional flat
look-down range reduction was rejected after consumer inspection. Implementation
edits made before the planning clarification were removed; gameplay is unchanged.

No new behavioural tests or rendering tests applied to this research-only change.
All nine required repository checks passed on the research-only documentation
change: formatting, Clippy, Rust tests/build with --locked, Python tests, three
asset scans and documentation headers. A rendered smoke test was not run because
no rendering or gameplay code changed at that point. The range/angle boundaries,
terrain-relative look-down examples, capability modes, mouse identity selection
and missile acquisition versus sustained illumination are covered by the
implementation pass below.

## RCS scope evidence

The user supplied an RCS panel image, retained outside the repository. Checked
FA CPComputeRCS 0x43e8c0..0x43ea33, its display call at 0x43ea4e and COSig call
at 0x478339. The latter proves that the helper also affects radar signature.
Read the original FA manual's RCS/RWR discussion through a public mirror, linked
in the RCS spec. No original executable or reference oracle was run. The old
reference checkout's prior oracle results are not presented as this pass's tests.

[The RCS spec](../spec/rcs.md) records retail facts. The component guide's 1/2/4
aspect weights, configuration multipliers and reference contour are agent tuning,
not retail results. Full glyph-state/contact eligibility and original RCS zoom
steps remain unresolved. No renderer or simulation code changed in this research
pass.

## History, selection and A2A scope review

Re-read the local USNF manual's historical-mode, contact-symbol, IR and missile
guidance sections;
recomputed the transcript hash recorded in the source notes. No history timing
was found in those passages. Audited the scope/control and live-combat code of
the day for Y/I action conflicts and HP-based contact suppression. That review
was read-only research, not implementation or live validation.

The scope includes Y history and IR A2A, persistent current-contact selection,
retained detectable destroyed aircraft and a single shared fire-control track.
These directions are John's; the trail length/cadence, IR range law, key
migration and detailed loss transitions remain labelled agent choices.
Reviewed the per-projectile launch target snapshot and weapon-specific
radar-dependency gate. John's clarification preserves independent missile targets
for sequential fire-and-forget shots while continuous-lock weapons still require
support for their own target. The acceptance cases named there were planned at
the time; they were written and run in the implementation pass below.
Existing target-view IFF stays gamified. A2G/HARM and detailed ground/remnant
systems are deferred. No new code was implemented or committed in this review.

All nine required repository checks passed again after this scope review:
formatting, Clippy, Rust tests/build with --locked, Python tests, three asset
scans and documentation headers. No rendering smoke test was run because that
change only updated research and planning documents. Those checks did not
validate the radar behaviour or establish retail parity.

## Implementation-mode validation, 2026-09-16

Implementation mode. The shared sensor component described in the
[component guide](../radar.md) is in code and validated locally. Host identity
for every runtime result below: Linux x86_64, NVIDIA GeForce RTX 4070 with
Vulkan, pinned Rust 1.91.1, locked dependencies. **No retail comparison was made
and none is claimed.** Nothing here establishes retail parity; it establishes
that the local contracts in the component guide hold.

### What was run

- **Workspace checks, all clean.** `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`,
  `cargo test --workspace --locked` and `cargo build --workspace --locked`.
  **447 Rust tests pass.** The sensor component contributes **35** of them, its
  unit tests plus the scenario tests in the `sensors::acceptance` module; the
  combat integration cases are inside the tore-sim total, and the scope
  projection, mouse pick and hover tests are in the app crate.
- **Python tests.** `python3 -m unittest discover -s tools -p 'test_*.py'`,
  **40 tests pass.** The three asset scans and the documentation header check
  pass.
- **Per-aircraft capability review.**
  `cargo run -q --locked -p tore-app -- --sensor-summary` prints one line per
  registered aircraft: radar record with search/track volumes, look-down
  coefficient and preset; infrared record; visual record; jammer record with
  generation and strength; and the PT radar and infrared signatures. All twelve
  imported aircraft report the expected equipment, and the radar records,
  nominal ranges and look-down coefficients match
  [the spec](../spec/radar.md#aircraft-capabilities). The saved output is
  `capability-summary.txt` in the evidence directory below.
- **The recovered automatic mode rule reproduces on the imported profiles.**
  F/A-18D reports TWS at the 25 and 50 settings and RWS at 100 and 150. A-4E and
  MiG-29 both report TWS at 25 and RWS at 50. In RWS a clicked contact stays
  selected while the weapon reports RWS SEARCH ONLY, so a search selection never
  supplies a lock.
- **Combat smokes.** `--combat-smoke` passes for **all twelve aircraft**, and the
  Rafale run additionally records a version-3 tape and replays it to an identical
  state. Scripted probes observe for one step before designating and for the full
  60-step acquisition before firing, the same wait a player has.
- **Rendered smoke.** `cargo run --locked -p tore-app -- --smoke-test` passes on
  Linux x86_64, NVIDIA GeForce RTX 4070, Vulkan.
- **Formula recheck.** Every formula in the component guide was recomputed by
  hand against the code, including the 63.64-nmi signature-50 case,
  L = 0.75/0.85/1.0, N = 0.20/0.45, J = 1 at B and 0.40 at 2B, burn-through at
  18.26 and 3.95 nmi, and the 12.5/17.68/25-nmi contour radii. All agree. The
  same review found and fixed five presentation and equipment-state defects and
  added the two missing noise-presentation rules; the results are described in
  the guide.
- **Captures inspected by eye.** The radar page showing TWS at the 10-mile
  setting with a selected and acquired contact and TRACK status; the same page
  with the target jammer powered, showing the directional noise band with
  sidelobe haze and the contact still burned through at close range; the infrared
  channel with history trails; and the RCS page showing the exposure contour
  wider abeam than nose-on.

Evidence is ignored under `.local/radar-implementation/`: `capability-summary.txt`,
`scope-tws-track.png`, `scope-jammer-noise.png`, `scope-infrared-history.png`,
`rcs-exposure-contour.png` and `implementation-notes.md`. None of it is committed.
Headless captures can set the scope with `--sensor-channel radar|ir`,
`--scope-range 5|10|25|50|100|150` and `--scope-history`.

### What was not run

- No retail execution, no matched retail recording, no side-by-side comparison
  against the original game. There is no oracle for any number below.
- No side-by-side tuning review of the twelve aircraft against each other. Stage
  5 of the component guide's delivery table is therefore only partly done: the
  capability summary and the combat smokes pass for every aircraft, but nobody
  has sat down with the twelve results together and tuned them.
- No multi-hour or soak run, no cross-CPU bitwise comparison, and no hardware
  input or haptics acceptance specific to the sensor keys.
- No automated GPU test of the scope raster. The capture inspection above is a
  human looking at images, not an assertion in a test.

### What stays approximate

- Every detection, look-down, notch, jamming, selection, history and RCS constant
  is agent-authored gameplay tuning. They are labelled `opinionated` and were
  chosen for play, not measured from retail or from any real system.
- The preset and jammer-generation groupings are agent classifications assigned
  per installed record. They are reviewable in code and in the component guide,
  and they are not recovered retail categories.
- Band compatibility is 1 for every current radar and jammer pair, because the
  retail records establish no frequency coverage.
- The infrared channel uses the base PT infrared signature only. Engine and
  throttle heat, rear-aspect bonuses, weather attenuation and post-destruction
  cooling are not modelled.
- The destroyed-aircraft ballistic fall is a minimal fitted addition, not a
  breakup or debris simulation.
- Terrain visibility keeps the existing sampled height-query approximation, not
  recovered native masking.
- The remaining departures from the plan and from retail data behaviour are
  listed in the component guide's
  [deliberate departures](../radar.md#deliberate-departures-and-known-approximations).

## MiG-31 radar and jammer review, 2026-11-08 (M31-7)

John's decision of 2026-11-08 reviewed `MIG31R.SEE` as `Preset::Advanced` and
`MIG31.ECM` as `Generation::LateColdWar`, one match arm each in
`crates/tore-sim/src/sensors/profile.rs` (opinionated, John). This replaced the
unreviewed profiles EQ-1 loaded for that aircraft.

- `--sensor-summary` reads: `Mig31: radar MIG31R.SEE search 124 nmi 60x60 deg
  track 100 nmi 45x45 deg look-down 30 ADVANCED; infrared none; visual VIS240.SEE
  search 10 nmi 120x90 deg track 5 nmi 120x90 deg; jammer MIG31.ECM LATE
  strength 0.30; signature radar 100 infrared 100`. The 124 nmi search range is
  the record's own decoded value, kept, not the preset's reference.
- The radar now supplies the semi-active `AA9.JT` its required illumination.
  `--live-fire --aircraft mig31 --weapon-slot 2 --combat-probe-ticks 3600` reads
  `shots=1 hits=1 kills=1 active=0`; before the review (M31-6, no flying radar)
  the same shot showed `RADAR OFF` and did not hit. At 1200 ticks the cockpit
  RADAR window reads `TWS 10NM 1 CONTACTS TRACK` with the missile still in
  flight, where M31-6 read `RADAR OFF`.
- The chaff and flare capacity read from `MIG31.ECM` is 60 and 60, the reviewed
  record's own counts, unchanged by the generation review.

Validated: the new synthetic record test and the whole `tore-sim` sensor suite
pass, and the windowed runs above. Not run: no full workspace check list (M31-10)
and no side-by-side tuning against the other aircraft.

## Mirage 2000 radar and jammer, 2026-11-10 (M2K-7)

The Mirage 2000 flies the already reviewed `F18R.SEE` (`Preset::Advanced`) and
`F18.ECM` (`Generation::LateColdWar`) of its record; no match arm and no code
changed. `--sensor-summary` reads: `M2000: radar F18R.SEE search 90 nmi 60x60 deg
track 50 nmi 45x45 deg look-down 0 ADVANCED; infrared none; visual VIS340.SEE
search 10 nmi 170x140 deg track 5 nmi 170x140 deg; jammer F18.ECM LATE strength
0.30; signature radar 100 infrared 100`. `--live-fire --aircraft m2000
--weapon-slot 3` shows `RADAR TWS 10NM 1 CONTACTS TRACK` and the semi-active
Super 530D (`R530.JT`) locks, hits and kills (`shots=1 hits=1 kills=1` at 700
ticks). Not run: any side-by-side tuning, or an RWR with a plotted emitter (no
scripted path exists).

## F-15E radar, FLIR and jammer, 2026-11-10 (F15E-7)

The F-15E flies the already reviewed `F15R.SEE` and `F15.ECM` of its record (the
F-15C's), plus the record's internal FLIR as an infrared channel; no match arm and
no code changed. `--sensor-summary` reads: `F15E: radar F15R.SEE search 150 nmi
60x60 deg track 100 nmi 45x45 deg look-down 30 ADVANCED; infrared AV8IR.SEE search
15 nmi 60x60 deg track 15 nmi 45x45 deg; visual VIS340.SEE search 10 nmi 170x140
deg track 5 nmi 170x140 deg; jammer F15.ECM LATE strength 0.30; signature radar
100 infrared 100`. `--live-fire --aircraft f15e --weapon-slot 3` shows `TWS 10
TRACK` and an AIM-120 that locks, hits and kills (`shots=1 hits=1 kills=1` at
1,500 ticks); `--sensor-channel ir` shows the `IR` scope. Not run: any
side-by-side tuning, or an RWR with a plotted emitter (no scripted path exists).

## Gripen radar, jammer and infrared channel, 2026-11-10 (GRI-7)

The Gripen flies the already reviewed `F18R.SEE` (`Preset::Advanced`) and `F18.ECM`
(`Generation::LateColdWar`) of its record and the AAS-38 infrared channel; no match
arm and no code changed. `--sensor-summary` reads: `Gripen: radar F18R.SEE search 90
nmi 60x60 deg track 50 nmi 45x45 deg look-down 0 ADVANCED; infrared AAS38.SEE search
15 nmi 60x60 deg track 15 nmi 45x45 deg; visual VIS340.SEE search 10 nmi 170x140 deg
track 5 nmi 170x140 deg; jammer F18.ECM LATE strength 0.30; signature radar 100
infrared 90`. `--live-fire --aircraft gripen --weapon-slot 3` shows `RADAR TWS 10NM
1 CONTACTS TRACK` and the AIM-120 locks, hits and kills (`shots=1 hits=1 kills=1` at
3,600 ticks; at 700 still flying). `--sensor-channel ir` shows `IR IR 10NM 1
CONTACTS` on the Radar window. `--jammer-on --combat-command incoming` lights `JAM` on
the RWR. Not run: an RWR with a plotted emitter (no scripted path), side-by-side
tuning.

## F-5E radar and countermeasures, 2026-11-10 (F5E-7)

The F-5E flies the already reviewed `F104R.SEE` (the F-104's `Preset::Basic`
radar, John's pick D1 as the AN/APQ-159 stand-in, B5-2) and the `F104.ECM`
dispenser record (`Generation::EarlyColdWar`, 20 chaff and 20 flares, John's
pick D2, B5-3), with the record's `VIS340.SEE` visual sensor; no match arm and no
code changed. `--sensor-summary` reads: `F5E: radar F104R.SEE search 50 nmi
60x60 deg track 25 nmi 45x45 deg look-down 50 BASIC; infrared none; visual
VIS340.SEE search 10 nmi 170x140 deg track 5 nmi 170x140 deg; jammer F104.ECM
EARLY strength 0.00 (no RF mode); signature radar 100 infrared 100` (the F-104's
line for radar and jammer, with the F/A-18's visual sensor). `--live-fire
--aircraft f5e --weapon-slot 2` (the wingtip AIM-9B pair) shows `TWS 10 ...
TRACK` on the radar window, the HUD `ARM 1 AIM-9B 89%` and the target camera on
the practice aircraft at 1.1 nmi; the round flies and hits (`shots=1 hits=1
kills=0 active=0` at 1,500 ticks, a single AIM-9B hits but does not kill the
practice aircraft). `--scope-range 25` redraws the scope at 25. The infrared
channel does not exist (`infrared none`). There is no jammer to light in the
RWR (strength 0.00, no RF mode); the diagnostic incoming round plots at the
window centre, as for the other aircraft. The Weapons window shows `20 CHAFF`
and `20 FLARE` (F5E-6). Not run: any side-by-side tuning, or an RWR with a
plotted emitter (no scripted path exists).

## Mirage F1 radar and jammer, 2026-11-10 (MF1-7)

The Mirage F1 flies the already reviewed `F18R.SEE` (`Preset::Advanced`) and
`AV8.ECM` (`Generation::Early`, no radio-frequency mode) of its record; no match
arm and no code changed. `--sensor-summary` reads: `Mf1: radar F18R.SEE search 90
nmi 60x60 deg track 50 nmi 45x45 deg look-down 0 ADVANCED; infrared none; visual
VIS340.SEE search 10 nmi 170x140 deg track 5 nmi 170x140 deg; jammer AV8.ECM EARLY
strength 0.00 (no RF mode); signature radar 100 infrared 100`. `--live-fire
--aircraft mf1 --weapon-slot 2` (and 3) shows `RADAR TWS 10NM 1 CONTACTS TRACK`
and the `R550.JT` Magic locks (`LOCK`), flies, hits and kills (`shots=1 hits=1
kills=1` at 700 and 1,500 ticks; still flying at 150 to 450). The F1's jammer
differs from the Mirage 2000's (`F18.ECM`, `LATE`, strength 0.30): it has chaff
and flares only. Not run: any side-by-side tuning, or an RWR with a plotted
emitter (no scripted path exists).

## Tornado radar, jammer and Pave Spike row, 2026-11-11 (TOR-7)

The Tornado flies the already reviewed `F18R.SEE` (`Preset::Advanced`), the `MIG31.ECM`
record (`Generation::LateColdWar`, strength 0.30, 60 chaff and 60 flares) and its own
`VIS240.SEE`, all as retail (decision 22: no substitute, no pod body drawn); no match arm
and no code changed. `--sensor-summary` reads: `Tornado: radar F18R.SEE search 90 nmi
60x60 deg track 50 nmi 45x45 deg look-down 0 ADVANCED; infrared none; visual VIS240.SEE
search 10 nmi 120x90 deg track 5 nmi 120x90 deg; jammer MIG31.ECM LATE strength 0.30;
signature radar 100 infrared 100`. `--live-fire --aircraft tornado --weapon-slot 1` shows
`TWS 10 ... TRACK` on the radar window, the HUD `LCOS 354 BK27 RADAR` and the target
camera `TORNADO IDS`; `--scope-range 50` redraws the scope at 50 (the other values are
shared code). `--jammer-on --combat-command incoming` lights `JAM` on the RWR. The default
load has no AIM-9 (B5-2 retired the outer wing station), so the lock is shown with the gun
and the laser bomb. The fixed `PAVESPK.SEE` row (flag `0x8`, as the A-10's) gives the
laser designator: `--weapon-slot 2 --combat-command sight-pin` reads `ARM 1`, `3 GBU-10`,
`IN RNG`, `LSR 4.6`, `TTI 0:32` and the seeker view on the target camera; without a
designation it reads `NO LASER`. Not run: any side-by-side tuning, or an RWR with a
plotted emitter (no scripted path exists).

## B-2 radar, jammer and the Pave Spike and IR rows, 2026-11-15 (B2-7)

The B-2 keeps the B-1B's five sensor rows as retail (decision 18: no substitute, no pod
body drawn): the F/A-18's `F18R.SEE` (`Preset::Advanced`, not the B-2's APQ-181), the
`F18.ECM` record (`LateColdWar`, strength 0.30), `VIS340.SEE`, `PAVESPK.SEE` and
`AV8IR.SEE`. No match arm and no code changed. `--sensor-summary` reads: `B2: radar
F18R.SEE search 90 nmi 60x60 deg track 50 nmi 45x45 deg look-down 0 ADVANCED; infrared
AV8IR.SEE search 15 nmi 60x60 deg track 15 nmi 45x45 deg; visual VIS340.SEE search 10
nmi 170x140 deg track 5 nmi 170x140 deg; jammer F18.ECM LATE strength 0.30; signature
radar 1 infrared 15`. The radar gives a bomber only the shared air-to-air modes (RWS,
TWS, and the IR channel on M); there is no ground-mapping mode. `--live-fire --aircraft
b2 --weapon-slot 1` reads `TWS 10 ... TRACK`, `--sensor-channel ir` reads `IR 10` with
the contact, `--weapon-slot 3 --combat-command sight-pin` (a GBU-27 row) reads `LSR 4.5`
and `REL 0:14` with `PIN`, `ZOOM 3` and `LASE` on the target camera, and the RCS page
reads `SIG 1` with a dot for a contour (an F/A-18 reads `SIG 154` with an oval). Not run:
an RWR with a plotted emitter (no scripted path exists). See [the B-2
section](../formats/aircraft-variety.md#b-2-sensors-hud-and-cockpit-b2-7-2026-11-15).

## F-117 sensors: no radar, IRADS and FLIR, 2026-11-15 (F117-6, F117-7)

The F-117 has no radar row (retail already right), so its radar window reads `NOT INSTALLED` and
`--sensor-summary` reads `F117: radar none; infrared AV8IR.SEE search 15 nmi 60x60 deg track 15 nmi 45x45
deg; visual VIS340.SEE ...; jammer none; signature radar 1 infrared 20`. John's decision 26: the
`PAVESPK.SEE` row is the IRADS turret and the `AV8IR.SEE` row the FLIR, in text only (the target
camera window's title, the Load Ordnance sensor line, the radar window's infrared channel and the replay's
Sensor View name; no other aircraft changed). The jammer row is gone (BMB-A2), so there is no `JAM`, no
chaff and no flare line. The laser works from the fixed row with a GBU-27 released from the bay (`a2g-laser-f117`:
lased, released, guided, the tank destroyed). The RCS page reads `SIG 1` closed and `SIG 2` with the bay open.
See [the F-117 section](../formats/aircraft-variety.md#f-117-ordnance-sensors-and-cockpit-f117-67-2026-11-15).

## B-1B sensors: the F/A-18 radar and jammer, five fixed rows, 2026-11-16 (B1B-6, B1B-7)

The B-1B keeps its five bomber-template rows as retail (the coordinator's first pass: no pod body, no substitute): `VIS340.SEE`,
the F/A-18's `F18R.SEE` (`Preset::Advanced`, nothing added), `F18.ECM` (`LateColdWar`, strength 0.30), `PAVESPK.SEE` and `AV8IR.SEE`, none with
the designator bit. `--sensor-summary` reads `B1b: radar F18R.SEE search 90 nmi 60x60 deg track 50 nmi 45x45 deg look-down 0 ADVANCED;
infrared AV8IR.SEE search 15 nmi 60x60 deg track 15 nmi 45x45 deg; visual VIS340.SEE search 10 nmi 170x140 deg track 5 nmi 170x140 deg; jammer
F18.ECM LATE strength 0.30; signature radar 40 infrared 60`. `--live-fire --aircraft b1b --combat-command chaff` (five) and `flare` (three) leaves `25
CHAFF 27 FLARE` of 30 and 30 (battery `flight-countermeasures-b1b`), and `--jammer-on` lights `JAM` on the RWR window. The RCS page reads `SIG 40` closed
and `SIG 60` with the bays open (`--flight-bay 1`). The radar gives the bomber the shared air-to-air modes only (no ground-mapping mode), as for the B-2
and B-52. Not run: an RWR with a plotted emitter (no scripted path exists). See [the B-1B
section](../formats/aircraft-variety.md#b-1b-lancer-ordnance-sensors-hud-and-cockpit-b1b-67-2026-11-16).

## Tu-160 radar, jammer and the laser row, 2026-11-16 (T160-6, T160-7)

`TU160R.SEE` (the record `B52R.SEE` renames) is reviewed `Preset::Transitional` and `TU160.ECM` (the
`B52.ECM` and `AC130.ECM` record with 16 bytes changed) `Generation::Transitional`, strength 0.30
(`sensors/profile.rs`, fitted). `--sensor-summary` reads: `Tu160: radar TU160R.SEE search 150 nmi 60x60
deg track 150 nmi 45x45 deg look-down 0 TRANSITIONAL; infrared none; visual VIS300.SEE search 10 nmi
150x120 deg track 5 nmi 150x120 deg; jammer TU160.ECM TRANSITIONAL strength 0.30; signature radar 100
infrared 100` (both were `UNREVIEWED (not enabled)`). The radar window reads `RADAR OFF`, then `TWS 10
... TRACK` on a contact; the weapons window `99 CHAFF`, `99 FLARE`, the RWR page `JAM` with the jammer on;
the RCS page `SIG 100` closed and `SIG 150` with the bay open. `SU37L.SEE` is the built-in laser (the
HUD box reads `LSR RDY` with an AS-16 selected), a fixed row with no pod body. The AS-16 now wants a
surface target, so `a2g-antiship-tu160` flies it at the Krivak (150 of 500 left). See [the Tu-160
section](../formats/aircraft-variety.md#tu-160-ordnance-sensors-hud-and-cockpit-t160-67-2026-11-16).

## F-111F sensors: radar and jammer reviewed, Pave Tack, 2026-11-16 (F111-6, F111-7)

The F-111F's `A6R.SEE` and `A6.ECM` (the A-6's records) were unreviewed, so it flew with no radar
and no jammer. Both are reviewed now (fitted): `--sensor-summary` reads `F111: radar A6R.SEE search 50 nmi
60x60 deg track 25 nmi 45x45 deg look-down 0 TRANSITIONAL; infrared AV8IR.SEE search 15 nmi 60x60 deg track
15 nmi 45x45 deg; visual VIS240.SEE search 10 nmi 120x90 deg track 5 nmi 120x90 deg; jammer A6.ECM
TRANSITIONAL strength 0.30; signature radar 100 infrared 100`. The radar window reads `TWS 10 ... TRACK`
once on, the RWR lights `JAM` with the jammer on, and the Weapons window reads `30 CHAFF`, `30 FLARE`. The laser
and FLIR pair (`PAVESPK.SEE`, `AV8IR.SEE`, and the pod on the bay row) show as PAVE TACK in text only: the
target camera window's title, the Load Ordnance sensor line and pod card, and the replay Sensor View
(the infrared channel reads FLIR). The RCS page reads `SIG 165` with the doors shut and `SIG 248` with the bay
open (default load, first GBU-28 released). Laser designation works: `a2g-laser-f111`. See [the F-111F
section](../formats/aircraft-variety.md#f-111f-ordnance-sensors-hud-and-cockpit-f111-67-2026-11-16).
