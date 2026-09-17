# Missile inventory and planning baseline

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-17. This pass audits existing game data, reviews the manual and writes a
proposed specification. It does not implement or validate new missile behavior.
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

Inspected the Rust scalar launch helper, powered/coast command, launcher bridge,
readiness and bay conditions, IR range function, HUD and combat-audio entry
points. The current flight state has a velocity vector, but the combat launcher
bridge exposes scalar speed. The existing shared IR function scales range with
base IR signature; the proposed heat-aspect/power terms are additional fitted
behavior. Neither the new velocity law nor uncued search/HUD/tone has been run.

## Validation and limits

All prescribed repository checks passed on Linux: formatting, warnings-denied
Clippy, locked workspace tests and build, Python tests, source/app/extractor
asset guards and the documentation header check. Logs for the first eight are
in `.local/missile-plan/checks.log`. Matrix verification checked 63 unique rows,
135 matching record hashes and decoded field comparisons. The revised matrix
retains those rows and adds nine active-on values and fitted uncued cone caps;
non-applicable and held entries are explicit. Documentation links
and absence of em dashes were checked for the changed documents.

No rendering changes, so no rendering smoke was required. Windows/macOS runtime,
interactive missile engagements, seeker tuning and retail comparisons were not
run. The new guidance, activation, memory and lifetime rules remain proposals;
passing existing tests does not validate those future features.
