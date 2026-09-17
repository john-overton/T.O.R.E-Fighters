# Missile implementation and inventory baseline

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-17. The inventory evidence below is retained.
Stages 1 through 5 are implemented and validated on Linux. Remaining tuning
and unavailable reviews are listed below.
[Specification and matrix](../spec/missiles.md),
[field interpretation](../formats/missiles.md),
[implementation milestones](../missile-update-plan.md).

## Reviewed state and source identity

Radar/RCS work is already committed as `60f2863`, `Implement shared aircraft radar,
RCS exposure and sensor contacts`. The working tree was clean before this planning
pass. No additional commit or push was needed for that work.

The inventory uses the existing complete extraction report at
`.local/combat-implementation/catalog/extraction-report.json` and its FA_2.LIB
JT files. All 135 unique JT records come from that provider. Its recorded SHA-256
is `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`.
The original extraction and EXE/SMS build identity are recorded in the
[combat component baseline](combat-components.md). This pass checked every
extracted JT's SHA-256 against the report, not a fresh hash of the retail archive.
No executable or module from the retail game was run.

## Method and inventory coverage

A local Rust probe used the existing `tore_formats::weapons::Weapon::parse` on
all 135 JT files. Its tab-separated output is retained at
`.local/missile-plan/inventory.tsv`; local probe source is `inventory.rs` in that
directory. Compared every record's signature, flags, both zone ranges/angles,
ignition, fuel, removal and track fields against the extraction report. All
matched. No retail bytes or generated art/audio are added to the repository.

The inventory inclusion rule starts with nonzero motor cutoff, then excludes
four rocket/pod records B8, B13, LAU10 and LAU61, and the special ~MOTHB record.
This gives 63 missile or missile-like candidates. The remaining 67 records have
zero motor cutoff; their source labels cover guns, artillery, bombs and special
objects, not additional identified missile candidates. This is a catalog audit,
not proof that every powered candidate shares one missile lifecycle.

The matrix accounts for every candidate once: 25 supported-radar proposals,
9 active-radar proposals, 20 IR proposals, 2 emitter proposals and 7 held rows.
Held rows comprise 3 designator records, 3 unresolved radar/special roles and AT2.
Fifteen candidate identities appear in current default-store allowlists; 48 are
catalog-only. These counts do not establish loadout compatibility or playability.

Current allowlisted missile identities by aircraft, read from live configuration:

| Aircraft identity | Missile records |
| --- | --- |
| F18.PT, F/A-18D | AIM120, AIM9M, AGM65G |
| RAFALE.PT, Rafale C | MICA, R530, R550, AGM65G |
| F14.PT, F-14D | AIM54C, AIM120, AIM9M |
| A4E.PT, A-4E | None; current stores are gun, bomb and rocket records |
| F31.PT, X-31 | AIM120, AIM9X, AGM65G |
| MIG29.PT | AA8 |
| SU27.PT | AA11, AA12 |
| MIG21.PT | AA2 |
| SU25.PT | AA8, AS7 |
| MIG23.PT | AS7 |
| SU35.PT | AA11B, AA12, AAML |
| F22.PT | AIM120, AIM9X, AGM65G |

These are code allowlists, not a new audit of carried station counts. Inventory
of ground and ship missiles authorizes no autonomous launcher implementation.

Reproduce the underlying extraction and parser component checks with the
[existing commands](combat-components.md#export-evidence). The local inventory
probe can be rerun with:

```sh
rustc --edition=2024 .local/missile-plan/inventory.rs --extern tore_formats=target/debug/libtore_formats.rlib -L dependency=target/debug/deps -o .local/missile-plan/inventory
.local/missile-plan/inventory .local/combat-implementation/catalog/FA_2.LIB/*.JT
```

The probe and catalog are local-only research artifacts, not fresh-clone tools.

## Manual and host-interface review

Reviewed the public text mirror of the **1999 EA/Jane's electronic FA manual**,
with chapter-4 production stamps dated 1999-05-24 and chapter-5 stamps from the
same date. This is manual evidence, not a match to an executed retail build.
The concise findings and page references have one home in the
[behavior spec](../spec/missiles.md#manual-supported-behavior). The source is
[the FA manual mirror](https://pdfcoffee.com/famanual-pdf-free.html).
No screenshot geometry or retail audio was validated from that text.

The earlier scalar adapter remains the explicit compatibility path. The new
launcher bridge carries world velocity and bay permission, while fitted heat,
seeker geometry and target ownership live in simulation. Original behavior and
fitted host rules remain separate in the specification.

## Validation and limits

The original inventory pass checked all 135 record hashes and decoded fields,
with 63 unique matrix candidates. The implementation retains that inventory and
the existing fifteen playable missile identities. No held or catalog-only store
was enabled. Full Linux repository checks and the rendered smoke passed. Logs
and captures are local-only in `.local/missile-update/`. Windows/macOS runtime,
a human flying session, human listening review and retail comparison were not
available. Automated range engagements and capture review do not establish retail
parity or physical fidelity.

## Implementation validation

The new profile module has synthetic tests for vector inheritance, the 1,600 ft/s
boost example, 1,900/1,300 ft/s closure, crossing lead, impossible intercepts,
finite burn, early removal, long ages and independent angle boundaries.
Live accepted profiles use this motion integrator. Version 4 combat records carry
world velocity; version 2/3 playback explicitly keeps compatibility motion.
All 177 simulation tests pass, including new heat/aspect/dwell, emitter shutdown,
shared RCS and reacquisition beyond the memory timeout cases. Seeker observations
and mounted acquisition now run in the live adapter. Fourteen missile integration tests cover all nine activation boundaries, failed
acquisition, hidden movement, early guidance expiry, target-free release,
post-shot mounted reset and two-target ownership. Activation and first pitbull
produce distinct shot-ID events. HUD, controls, bay permission, fitted tone and version-4 replay inputs are
implemented. Projection tests cover 4:3, widescreen and portrait viewports at
0.5x/1x/2x/4x zoom. Fixed-step replay agrees at 30/60/144 rendering cadences,
including a pause. The tone envelope tests also cover reduced configured volume.

## Local manual and presentation evidence

John supplied `/home/john/Downloads/fa-manual_compress.pdf`, copied to ignored
`.local/missile-update/manual.pdf`; SHA-256
`1a082378a8e8cd163ed6b398efcc1df80b67c2f104f6b90ac0733c88d58e26c3`.
Inspected rendered PDF pages 87/88 (printed pages 83/84): fixed circular reticle,
square target box, diamond lock cue and vertical range scale. The implementation
uses those forms with fitted placement and the existing imported HUD11 font and
HUD module color. It does not claim pixel matching. Printed page 119 describes
stronger A2A growl and A2G ringing. Inspected the local audio inventory and
&MISSILE.11K (22,783 bytes) and &SQUEAL.5K (2,392 bytes); their names and PCM
content do not establish lock-sound mapping. The temporary cue is authored,
110 Hz modulated growl or 660 Hz ringing, with a 0.1-second amplitude slew.
Envelope, timbre separation and pause/mute have deterministic tests. No retail
audio or PDF bytes are committed. Human listening review remains unavailable.

## Roster and range acceptance

All twelve supported aircraft passed `--combat-smoke`, exercising their current
stations and all five source damage classes. The pass includes AGM65G and AS7.
Forty-six per-slot version-4 tapes were written and replayed with full state
comparisons. A separate BORESIGHT tape includes heat and emitter fixture inputs,
and replays to the same 151 ticks, one shot and ammunition state. Old tape versions
retain explicit compatibility rules. An archived version-2 tape was correctly
rejected because its asset fingerprint differs from the current import; it was
not silently reinterpreted with new assets. A new compatibility recording and
a version-3-format fixture derived from its inputs both replayed successfully
with matching tick, shot and ammunition results. This fixture is not an archived
retail recording. Controlled emitter tests cover receiver
filtering and signal shutdown without reflective-radar or IR fallback.

Run `cargo run --locked -p tore-app -- --aircraft f18 --missile-acceptance`
for the repeatable imported-store probe, and repeat for the other registered
aircraft. The no-AI target is at 10,000 feet. Cases use 300/600/900 ft/s launches,
stationary or 600 ft/s approaching/receding/crossing targets, and 25/50/75/100
percent launch-envelope samples. Matched 600 ft/s climb and slip cases carry a
100 ft/s vertical or sideways component and reduce forward speed to preserve
total speed. Both launch modes are exercised where supported. Target aspect is
set before acquisition; velocity is applied at release. This is a controlled
launch fixture, not an aircraft opponent.

Across eleven missile-carrying aircraft the 3,920 cases produced **3,331 hits,
395 acquisition inhibits and 194 expiry misses**. A-4E has no current missile
stores. Duplicate weapon identities across aircraft remain separate cases because
their launcher sensors and target signatures differ. Largest sampled hit range is
not a guaranteed reach, and the probe does not search beyond the source maximum.

| Weapon | Cases | Hits | Inhibited | Expiry misses | Largest sampled hit nmi |
| --- | ---: | ---: | ---: | ---: | ---: |
| AA11.JT | 160 | 117 | 30 | 13 | 9.87 |
| AA11B.JT | 160 | 117 | 30 | 13 | 9.87 |
| AA12.JT | 320 | 320 | 0 | 0 | 24.69 |
| AA2.JT | 160 | 124 | 25 | 11 | 3.95 |
| AA8.JT | 320 | 253 | 50 | 17 | 3.95 |
| AAML.JT | 160 | 160 | 0 | 0 | 74.06 |
| AGM65G.JT | 640 | 389 | 155 | 96 | 9.87 |
| AIM120.JT | 640 | 636 | 0 | 4 | 23.70 |
| AIM54C.JT | 160 | 160 | 0 | 0 | 98.75 |
| AIM9M.JT | 320 | 264 | 40 | 16 | 3.95 |
| AIM9X.JT | 320 | 259 | 45 | 16 | 3.95 |
| AS7.JT | 160 | 160 | 0 | 0 | 4.94 |
| MICA.JT | 160 | 160 | 0 | 0 | 24.69 |
| R530.JT | 80 | 80 | 0 | 0 | 16.46 |
| R550.JT | 160 | 132 | 20 | 8 | 3.95 |

Rendered checks used the NVIDIA GeForce RTX 4070 Vulkan renderer. Reviewed
CUED MIDCOURSE, BORESIGHT IR lock at 2x zoom, target-free search, and individual
shot/motor/guidance-time captures. The scene smoke presented successfully.
A separate overlay avoids applying cockpit zoom twice to seeker geometry; a
fitted translucent backing keeps weapon details readable over the zoomed cockpit.
Original HUD color and font remain in use.

Remaining approximations: authored activation distances, heat and boost tuning;
a straight-path intercept estimate that does not price turn losses; conservative
radar-only emitter defaults; unspecified missile-specific notch/jammer rejection
and original rear-aspect/target-class consumer details. Existing airborne target
eligibility is preserved. No new ground, ship, SAM or aircraft combat AI exists.
The fitted tone is not a verified retail sample. Source `trackT` remains unused.
