# Ukraine airport extraction baseline

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Initial research pass, 2026-09-20, Linux. Retail was inspected as inert data; no
retail execution or side-by-side comparison was performed. The implementation
and independent review results are recorded below.

## Input identity

- FA.EXE SHA-256: `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
- FA_2.LIB SHA-256: `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`.
- UKR.MM SHA-256: `c36e68b16ae98691e4b3356baf5f15fec2c031907caab9553ee3bcd062b2bbbc`.
- Local manual PDF SHA-256: `1a082378a8e8cd163ed6b398efcc1df80b67c2f104f6b90ac0733c88d58e26c3`.
  Text inspected at `.local/missile-update/manual.txt`, printed pages 65, 67, 87,
  plus the ground-target tutorial and mission-editor object placement section.
  This is supplied FA documentation, not proof of this executable's thresholds.

## Placement inventory

The inventory reads object blocks through their terminating dot and retains the
preceding meaningful source comment. Full source records remain local at
`.local/airport-research/ukraine-inventory.json`. Counts below exclude each runway.
Source comment spellings are retained.

| Airport section | Associated section objects |
| --- | ---: |
| Kiev Airport | 5 |
| Odesa Airport | 13 |
| Simferopol Airport | 19 |
| Dnipropetrov Airport | 3 |
| L'viv Airport | 4 |
| Ivano Airport | 4 |
| Kherson Airport | 4 |
| Kryvyy Rih Airport | 5 |
| Zaporizhzhya Airport | 8 |
| Kharkiv Airport | 8 |
| Donets Airport | 7 |
| Voronezh Airport | 8 |
| Rostov AirPort | 7 |
| Krashodar Airport | 4 |

Total: 14 runway records, 99 airport-section objects, 144 other objects, 257 overall.
Four hangars and one control tower are explicitly placed in the Kiev section.
These are map placements, not buildings inferred from a terrain texture.

## Asset probes

Targeted extraction from FA_2.LIB produced 37 resources with zero errors:
UKR.MM, 18 OT definitions and 18 SH files. A second targeted extraction produced
18 referenced PICs with zero errors. Reports under `.local/airport-research/source/`
and `textures/` preserve per-resource hashes, offsets and archive provenance.

For each extracted SH, ran `cargo run --quiet --locked -p tore-formats --example
shape_inspect -- PATH`. All eighteen completed successfully; logs and structured
results remain under `.local/airport-research/`. All reported empty state-word
sets for this projection. This does not establish the absence of other branches.

| Definition | Explicit shape | Projected faces | Exposed texture |
| --- | --- | ---: | --- |
| APTLA.OT | APTLA.SH | 5 | _APTLA.PIC |
| APTLC.OT | APTLC.SH | 5 | _APTLC.PIC |
| APTOLD.OT | APTOLD.SH | 8 | _APTOLD.PIC |
| BARKSA.OT | BARKSA.SH | 8 | _BARKSA.PIC |
| BARKSB.OT | BARKSB.SH | 8 | _BARKSB.PIC |
| BUNKER.OT | BUNKB.SH | 5 | _BUNKB.PIC |
| COMM.OT | SHELT.SH | 5 | _SHELT.PIC |
| CTOWR.OT | CTOWR.SH | 30 | _CTOWR.PIC |
| FCTYA.OT | FCTYA.SH | 5 | _FCTYA.PIC |
| FCTYB.OT | FCTYB.SH | 6 | _FCTYB.PIC |
| FUEL.OT | FUEL.SH | 11 | _FUEL.PIC |
| HANGR.OT | HANGR.SH | 22 | _HANGR.PIC |
| HANGRB.OT | HANGRB.SH | 14 | _HANGRB.PIC |
| MICRO.OT | MICRO.SH | 23 | _MICRO.PIC |
| MICROM.OT | MICROM.SH | 47 | _MICROM.PIC |
| STORE.OT | STORE.SH | 13 | _STORE.PIC |
| STRIP.OT | RUNWAY.SH | 63 | _RUNWAY.PIC |
| TOWER.OT | TOWER.SH | 14 | _TOWER.PIC |

The initial broad directory extraction selected only profile-filtered resources
and encountered four unsupported LHX archive compression flags. It was not used
as proof of airport closure. A separate FA_1.LIB texture query matched nothing.
The successful, explicit FA_2.LIB passes above resolved the requested resources.

## Cross-theater planning census

Targeted `--include '*.MM'` extraction from the same FA_2.LIB produced all 75
layouts with zero errors. A text census of indented type fields found 10,252
object records across those layouts, including 1,458 references to the thirteen
runway definitions below. Counts include repeated campaign layouts and separately
placed runway variants; they are not counts of unique airports.
Reports and census remain in `.local/airport-research/layouts/` and
`layout-inventory.json`. This census is not production mission parsing.

| Base layout | Object records | Runway-definition references |
| --- | ---: | ---: |
| APA.MM | 107 | 20 |
| BAL.MM | 165 | 34 |
| CUB.MM | 129 | 23 |
| EGY.MM | 152 | 25 |
| FRA.MM | 218 | 32 |
| GRE.MM | 191 | 17 |
| IRA.MM | 160 | 21 |
| KURILE.MM | 85 | 4 |
| LFA.MM | 64 | 5 |
| NSK.MM | 124 | 22 |
| PGU.MM | 154 | 21 |
| SPA.MM | 135 | 21 |
| TVIET.MM | 544 | 10 |
| UKR.MM | 257 | 14 |
| VLA.MM | 160 | 26 |
| WTA.MM | 158 | 16 |

A second targeted pass extracted all thirteen referenced runway OT definitions,
with zero errors, under `.local/airport-research/runway-types/`. Their explicit
identity strings describe airports and all select `_STRIPProc`: STRIP,
STRIP1/2/3/4/5/6/7, STRIP3A/5A/6A/7A and DTSTRP. Main-shape references are
RUNWAY.SH, RNWY1/2/3/4/5/6/7.SH, RNWY3A/5A/6A/7A.SH and DTSTRP.SH respectively.
Only RUNWAY.SH received the earlier geometry probe. Suffix A is not evidence of
a damage variant: these are independently placed types with airport callbacks.
Their exact relationship to adjacent runways is still unresolved.

FRA.MM directly uses `sides2` and `nationality2`, unlike the reviewed UKR.MM
fields. Generalization must support both contracts rather than treating a missing
`nationality` as neutral. Comments cannot be assumed to provide airport grouping
in every theater. All layouts were inventoried, but none beyond Ukraine had its
full ground-object visual dependency set decoded in this pass.

A plain ASCII string scan of FA.EXE found landing-grade text but no explicit
landing-clearance command text. This negative scan does not establish the absence
of radio commands, because strings may be encoded or stored elsewhere. No new
ILS or tower-command executable consumer was resolved in this planning pass.

## Validation and limits

Repository-required checks passed: formatting, warnings-denied workspace/all-target
Clippy, locked workspace tests and build, Python tool tests, repository and both
debug executable asset guards, and documentation checks. Logs are stored in
`.local/airport-research/check-*.log` with a `checks.json` summary. These checks
include the existing working tree; they do not establish airport gameplay parity.
No rendering change was made, so a display smoke test was not run for this pass.

No imported mesh was visually reviewed in this pass. Shape projection and PIC
extraction do not validate world scale, terrain alignment, damage geometry,
collision behavior, landing controls or a complete visual dependency traversal.
Only Ukraine received object-section and visual dependency probes. Other layouts
received the limited census above; overlay semantics and campaign resolution
remain planned work. Retail-derived outputs remain ignored.
[Recovered data contract](../formats/airport-placements.md).

## Implementation and independent review

Implementation mode, 2026-09-20. Sol implemented the base-theater scene and airport
service. The parent reviewed it, requested a follow-up, and fixed integration
issues before the final checks. No commit or push was made. Host: Linux,
NVIDIA GeForce RTX 4070, Vulkan. Retail modules were never executed.

Review corrections include preserving ground targets after reset and range
commands; importing per-object damage categories and sensor signatures; keeping
combat health authoritative; automatic ILS without requiring clearance; airport-
relative altitude; stable approach-end selection and bounded repeat replies;
full placement transforms; SH header scale; original texture UVs, masks and
static depth bias; large texture compatibility; and explicit replay reset modes.
The fitted primary approach line uses the source runway anchor rather than the
whole airport mesh midpoint. [Rules and remaining approximations](../spec/airports.md).

### Final checks

- Formatting, warnings-denied workspace/all-target Clippy, locked workspace build
  and **896 Rust tests** passed. Two GPU unit tests remain ignored, separately
  covered here by real display checks. **68 Python tests**, all three asset guards,
  documentation checks and diff whitespace checks passed.
- Source CLI dependency dry runs selected 295 Ukraine-profile and 717 all-theater
  resources from FA_2.LIB, both with zero errors. The latter includes matching
  campaign layout resource dependencies, not campaign runtime interpretation.
- All sixteen base worlds construct with actual live targets after Combat reset.
  Across their 2,803 placement records, 2,744 supported bodies render and receive
  targets. No-body controllers and unsupported projections remain manifest-only.
  The diagnosed unsupported SH programs are CHAP.SH and SA2.SH. All thirteen
  runway shape variants used by the base layouts project successfully.
- Ukraine has **257 live targets**, including all fourteen runway records and the
  99 airport-section objects. Its final static scene contains 30,327 vertices.
- The actual app probe confirms Simferopol ground at 1,024 ft, ILS active at
  5,024 ft MSL, inactive at 5,024.01 ft, and inactive with NAV off or gear up.
- The required `cargo run --locked -p tore-app -- --smoke-test` passed. Fourteen
  Ukraine overhead captures, Kiev approaches from both ends, one first-airport
  view in each other base theater, and the elevated-airport cockpit ILS capture
  rendered successfully. The parent inspected the contact sheets and approaches.
  The original striped/coplanar rendering defect is absent in the final views.
  Unusual tan Ukraine paving was independently checked against _RUNWAY.PIC and
  PALETTE.PAL, and retained rather than replaced with assumed asphalt artwork.
- Version-6 authored replay fixtures exercised selection, clearance, repeat,
  cancellation and reset. Each was replayed twice with identical output and
  expected final service state. A version-5 range fixture also passed. These
  test the replay/service integration, not a manually piloted recorded landing.
- A synthetic projectile test confirms the ground target's own damage class and
  one destruction event. Tilted box contact, target persistence and external
  health synchronization have independent regression tests.

Local evidence: `.local/airport-review/final-checks.json`, `probes.json`,
`render-checks.json`, `render-ils-refined.json`, `replay-checks.json`, and their
logs/captures. `ukraine-contact.png`, `base-contact.png`, `kiev-south.png`,
`kiev-north.png` and `simferopol-ils-refined.png` were visually inspected.
The isolated runtime cache is `/tmp/tore-airport-import2`; the user's saved cache
and preferences were not replaced. Headless/capture checks used `TORE_WIND=0,0`
to isolate this feature from the existing negative default-wind issue in some maps.

### Runtime scope and remaining work

This completes the base-airport implementation slice, not the whole original
parity target. Mission/campaign overlay replacement and generated terrain alias
resolution remain unsupported. No automatic substitution of base terrain is made.
Some source shape programs, original tower speech mappings, destroyed replacement
art, aircraft-specific speed brackets and target-relative camera imagery remain
open. Large PIC sheets use documented nearest-sampled GPU layers. Airport grouping,
contact volumes, service policy and guidance scaling remain fitted where stated.
All base airports are neutral with explicit permission in the current free-flight
host; a future mission must supply its own allegiance/permission state.

No manual joystick landing session, audible tower acceptance, Windows/macOS
runtime check, before/after performance benchmark or retail side-by-side comparison
was performed. The synthetic
service/contact checks and captured guidance are not a claim of those validations.
