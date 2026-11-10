# Aircraft ordnance availability validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation, 2026-10-05, on the local `import-variety` branch.
The source identities, logical station matrix and compatibility evidence live
in the [source review](../formats/aircraft-ordnance.md). Player-facing rules live
in the [ordnance specification](../spec/ordnance-presentation.md). Retail runtime
comparison is unavailable; these checks validate imported data and host behavior.

## Scope

Recover nine weapon-capable source stations previously omitted by the default-JT
filter. Preserve their empty/default-tank state until selected, preserve existing
weapon indices, and read older saved loads without filling the new rows. Keep
fixed device mounts occupied and unavailable to tank editing. Use shared,
source-numbered editor rows for weapons and tanks, with original thumbnails and
compatible replacement. Tanks lead the air-to-surface catalog; the extra Tanks
button and external-fuel/tank-shell banner are removed per the supplied retail
reference. Aircraft geometry and rendering are unchanged.

## Reproducible checks

With the development profile imported from user-owned media:

```sh
TORE_DATA_DIR=.local/dev-profile target/debug/tore-app --validate-ordnance --no-audio
TORE_DATA_DIR=.local/dev-profile target/debug/tore-app --validate-tanks --aircraft f14 --no-audio
```

`menus-ordnance-availability` runs the first command in the battery. It checks
all 36 retail identities, all imported supported weapon definitions against
source station rules, installed and optional tank placements, source-default
quantities, legacy station-prefix loads, accepted-load and mission-text round
trips, shared-mount exclusion, live configuration construction, and lightweight
multiplayer load validation. Full-capacity loads may exceed takeoff weight, so
placement/persistence checks precede a separate one-unit-per-station, zero-
internal-fuel validation. This does not assert every full-capacity combination
can take off.

The audit pins what it must find, so a partial import cannot pass with fewer
checks (2026-10-08): all supported weapon records and all 4 tank records
present, 36 aircraft, nine recovered weapon rows, and exact weapon and tank
placement counts. The A2G round (2026-10-10) raised the supported records from
40 to 54 (14 bomb, pack, cluster and LAU-10 records) and the weapon placements
from 1,464 to 2,251 before W2; tank placements stay 146. It also audits the two novelty
bombs (Boulder, Moose) apart from the 54: every aircraft gives them no
capacity and refuses them without Cheat loading, 160 station and record pairs
take them under Cheat loading, a loadout carrying one fails validation once
Cheat loading is off, and the normal and Cheat catalogs are checked against the
station rules on all 36 aircraft (the novelty bombs only in the Cheat catalog). It also loads the two laser designator pods (Pave Spike, Pave Knife) on every station that takes them, 130 placements, checking the one-pod rule, the pod's removal by a weapon, the catalog lists and the pod's restoration through the saved mission. A missing record, or a count that moves,
fails the run; raise the pins in `crates/tore-app/src/ordnance_audit.rs` and
here when the supported list, the tanks or the roster change on purpose.
Slice W2 of the air-to-ground round (2026-10-10) added seven air-to-ground
missiles (AGM-65A, AGM-84E, AGM-45, AGM-84A, AM-39, AS-16, AS 15): 47 supported
records and 1,802 weapon placements, measured with `--validate-ordnance` on the
surface-data import (the tank and recovered-row counts are unchanged). Slice W1 added the laser bombs, the JDAMs and the
laser missiles (GBU-10E, GBU-10, GBU-10E Paveway, GBU-27, GBU-28, GBU-29,
GBU-29/P, GBU-30, AS-14, AS 30L, AT-12): with W2's, 58 supported records and
2,359 weapon placements, measured the same way (2026-10-11). That pass's audit included the three derived R-27 stores and the three
[smaller guided bombs](../spec/missiles.md#smaller-guided-bomb-stores): 78
supported records and 3,319 weapon placements.
The smaller bombs add 201 placements to the prior 3,118. Every donor-capable
station is checked for variant capacity, and every variant uses the existing
load/save, catalog and mission-validation checks. Tank placements remain 146.
R-27 family/rail restrictions replace generic compatibility. All four family
carriers pass saved-load roundtrips; see [missile identities](../spec/missiles.md#corrected-identities-and-r-27-stores).

Slice JT-4 (2026-11-06) added the four decoded air-to-air missiles AA6, AA9,
PL7 and PL10 on their compatible ported stations. The current audit is 82
supported records and 3,626 weapon placements (pins in `ordnance_audit.rs`),
measured with `--validate-ordnance` on the surface-data import; the tank,
recovered-row and novelty counts are unchanged. The station rows are in
[aircraft ordnance](../formats/aircraft-ordnance.md#aa-6-aa-9-pl-7-and-pl-10-jt-4-2026-11-06).

The [derived F-15C and F-16C stations](../spec/stores-on-pylons.md#derived-f-15c-and-f-16c-stations)
(John, 2026-10-15) add the F-15C centreline and wing tank rows and the F-16C
inner wet pair, with two authored tank records (610 and 370 gallon). The audit
now pins 10 recovered weapon rows, 3,372 weapon placements, 161 tank placements,
6 tank records and 162 Cheat-only novelty placements. For both aircraft it also
loads the full requested set (F-15C: four fuselage missiles, four AIM-9M and
three 610 gallon tanks; F-16C: two 370 gallon tanks beside its default load),
checks fuel feed, Shift+K jettison, the F-16C wet row's selected-store jettison,
mission and profile saves, and an old saved load restoring with the new rows empty.

The [optional carriers of 2026-10-16](../spec/stores-on-pylons.md#optional-carriers-2026-10-16)
turn the F-15C centreline row into a tank-or-bomb-rack station and add the
F-22 inboard wing pair, the MiG-29 centreline and the Rafale C centreline,
with three more authored tank records (600 gallon, 1,500 litre and 2,000
litre). The audit now pins 12 recovered weapon rows (the F-15C centreline and
F-22 wing rows take weapons), 3,401 weapon placements (+25 bombs on the F-15C
centreline, +4 AIM-9/AIM-120 on the F-22 pair), 175 tank placements (+5
F-22, +4 MiG-29, +5 Rafale C), 9 tank records and 166 Cheat-only novelty
placements (Boulder and Moose on the F-15C centreline and F-22 rows, still
refused without Cheat loading). For each new row it loads
the full set, checks capacity, tank and weapon exclusion, fuel feed, Shift+K
and selected-store jettison, saves, and an old saved load with the row empty.

The [F-16C wingtip AIM-120](../spec/stores-on-pylons.md#the-f-16c-wingtip-aim-120)
(John, 2026-10-16) adds one weapon placement, so the audit now pins 3,402
(the AIM-120 on the F-16C tip row, two a load; nothing else moves: tanks, records and
Cheat-only placements are unchanged). The audit also checks that every other
supported weapon keeps the tip row's source rule, loads two AIM-120s, saves and
restores them, and fires one from each tip's edge with guidance.

The [Phantoms' gun pod of 2026-10-16](../spec/stores-on-pylons.md#the-phantoms-gun-pod-2026-10-16)
lets the F-4B's centreline row take the SUU-16 in place of its stores. The audit
now pins 3,403 weapon placements (+1 on the F-16C wingtip's 3,402: the pod on `F4B.PT` row 3, one a station,
the row's own capacity of four capped at one); every other count is unchanged,
the Cheat catalog still lists the pod only for an aircraft that has one (the
F-4B and F-4J), and the pod is refused on every other station. A dedicated step
("gun pod F4B.PT: ...") loads it through the selector, checks its 1,702 lb and
600 logical units, taking it off, Guns only, the saved mission and the lobby rule,
the Tab gun, a held trigger firing from the pod's nose and the jettison.

The [F-15C wing pylons of 2026-10-20](../spec/stores-on-pylons.md#the-f-15c-wing-pylons-2026-10-20)
(John, "a wing pylon that only accepts tanks") take an AIM-120 or an AIM-9B,
AIM-9M or AIM-9X in place of the tank, one a pylon. That pass added four weapon placements and two Cheat-only novelty
placements (Boulder and Moose on that row), with tank placements unchanged. It checks the row's capacity of two for each, the exclusion with the tank, the refusal of the
Sparrow, the Archer, bombs and the Maverick, a save and restore, and an older
saved load (one or two weapon slots short) restoring with the new rows empty.

The [F-22N carriage](../spec/stores-on-pylons.md#f-22-naval-stores) pass
recorded 14 optional weapon rows, 3,411 weapon placements, 180 tank placements
and 170 Cheat-only novelty placements. The F-22N row adds four AIM-9/AIM-120
choices, five tank choices and two novelty choices to the previous totals
of 13, 3,407, 175 and 168. Weapon/tank record totals were 78 and 9 at that
pass; see the JT-4 paragraph above for the current 82 and 9.
Both Raptors report the same normal choices on row 8 and pass tank/weapon
exclusion, fuel feed, jettison, save/load and old-load restoration. Their
source default weapons and quantities remain unchanged.

The F-14 probe separately covers zero, one and two F250 tanks, external fuel,
shell mass, total mass, consumption order and accepted-load restoration. It is
the reviewed F-14 contract only: `--validate-tanks` refuses any other
`--aircraft` (`menus-tanks-refuses-other-aircraft`) rather than silently
probing the F-14; `--validate-ordnance` covers every aircraft's tanks.

## Results

The final workspace formatting, all-target clippy, build and tests passed:
2,952 Rust tests passed, with 42 ignored. All 230 Python tool tests passed.
Source and app/extractor binary asset guards passed, as did the documentation
check. The wrapped Linux GPU smoke test presented its requested screen.

The source audit passed for all 36 retail aircraft: nine recovered weapon rows,
1,464 compatible supported-weapon placements (2,251 from the A2G round) and 146 tank placements. It also
checks explicit rejection of overlapping tank/weapon loads, genuinely older
loads without tank records, unarmed aircraft and restoring an empty row with
plus. The F-14 zero/one/two-tank probe passed. All 36 scenarios in the final
ordnance battery passed; results are in the ignored local directory
`.local/battery/20261005-131933-ordnance-final/`.

All 22 focused editor tests passed, covering shared source rows, category-stable
occupants, mixed tank/weapon paging, tank-first surface ordering, cross-kind
replacement, tank transfer/unload, retained tank kind, rejected edits and future
six-card station pagination. The existing creator drag probe now resolves UI
positions through the source-row union and explicitly loads optional empty rows
before testing their unload/refill gestures.

Fresh F-14 air-to-air and air-to-surface captures are retained locally as
`.local/ordnance-editor-audit/f14-retail-aa.png` and `f14-retail-ag.png`.
Visual inspection confirms tanks first on the surface page, five stable loaded
station rows and no clipping. Both removed areas exactly match the imported
ORD_AIR3 background in both categories: button region `(278,35,60,24)` and banner
region `(343,69,295,20)`, with zero differing bytes. Earlier source-row captures
also cover A-4E, MiG-17, F-4E, AC-130 and unarmed C-130.

The broad quick-check battery completed 162 other scenarios. Its initial creator
run exposed the old compressed-index gesture assumption, corrected above. The
unrelated exhaustive AI matrix was stopped after about 20 minutes; no pass is
claimed for it. The final full creator run passed against the implementation tree committed as
`4371259`: all 37 playable profiles passed loadout/dragging/removal/restart checks,
13,979 setups started, 43 were refused with an explanatory message, and zero
problems were reported. The render sweep produced 606 pictures; creator input
fuzz exercised 147,000 events and screen input fuzz exercised 76,000 events, all
with zero problems. This supersedes the initial creator-probe failure.

Validation logs are under `.local/ordnance-audit/retail-layout-*`. Selective
extractor checks also passed with no errors: original tank thumbnails are
included across FA archives, and aircraft-only MiG-17/MiG-23 profiles include
their explicitly documented empty-station seed definitions.

## Limits

The weapon catalog still requires implemented flight behavior. Importing an
unsupported JT does not make it usable. Source compatibility is broader than
reviewed gun-pod support; SUU16 remains restricted to its source-installed F-4J
mount. Fitted source-default tank capacity and gun-pod ammunition rules remain
as documented in their feature specifications. Campaign stock, era/airbase
restrictions, sensor-pod replacement and in-flight fuel-tank jettison remain
outside this pass. The supplied F-14 reference also exposes an existing
[675 lb internal-ammunition mass gap](../formats/aircraft-ordnance.md#internal-ammunition-weight-follow-up);
this layout change preserves the shared host mass rule pending a consistent
loadout/flight correction. No Windows or macOS interactive run is claimed.

## Roster change (2026-11-07, slice M25-1)

The MiG-25 (`M25.PT`) joined `AircraftId::ALL` in slice M25-1 on branch
`m3-aircraft`, so the pinned aircraft count in `crates/tore-app/src/ordnance_audit.rs`
is now 37. The MiG-25's default stores, tank and the resulting weapon and tank
placement counts are wired and re-measured in slice M25-6 with
`--validate-ordnance`; the placement counts above still describe the last
measured 36-aircraft pass.

## MiG-25 ordnance (2026-11-07, slice M25-6)

The MiG-25's three carrier rows are pinned and measured on the branch. The audit
now reports:

```
ordnance audit: 37 aircraft, 14 recovered weapon stations, 3684 weapon placements, 182 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 65 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 174 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The pins in `ordnance_audit.rs` moved from 3,626 to 3,684 weapon placements (the
MiG-25's two wing rows, 58 compatible placements together), from 180 to 182 tank
placements (its centreline row, `F150.GAS` or `F250.GAS`) and from 170 to 174
Cheat-only novelty placements (Boulder and Moose on both wing rows). The
recovered-station, tank-record, pod and weapon-record counts are unchanged. The
per-aircraft line is `ordnance M25.PT: 3 source rows, 0 optional weapon rows, 58
compatible weapon placements, 2 tank placements; defaults and restart passed;
catalog 29 normal, 83 Cheat`. The stations, the 29 normal records and the default
load are in [aircraft ordnance](../formats/aircraft-ordnance.md#the-mig-25-m3-2026-11-07).
No record, mask or station position was added for the aircraft; the counts move
only because an already reviewed identity is now measured.

## Roster change (2026-11-08, slice F16A-1)

The F-16A (`F16.PT`) joined `AircraftId::ALL` in slice F16A-1 on branch
`m3-aircraft`, so the pinned aircraft count in `crates/tore-app/src/ordnance_audit.rs`
is now 38. The F-16A has its own nine source rows (the fixed visual sensor, radar
and jammer in rows 0 to 2, the internal gun in row 3, the centreline tank in row
4, the inner wing AGM-65G in row 5, the wing AIM-120 in row 6, the outer wing
AIM-9M in row 7 and the tip AIM-9M in row 8), and no Pave Spike row. Its default
stores, tank and the resulting weapon and tank placement counts are wired and
re-measured in slice F16A-4 with `--validate-ordnance`; the placement counts
above still describe the last measured 37-aircraft pass.

## F-16A ordnance (2026-11-08, slice F16A-4)

The F-16A's five carrier rows are pinned and measured on the branch. The audit
now reports:

```
ordnance audit: 38 aircraft, 14 recovered weapon stations, 3817 weapon placements, 188 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 67 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 182 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The pins in `ordnance_audit.rs` moved from 3,684 to 3,817 weapon placements (the
F-16A's two `0x07e5` wing rows, 56 compatible records each, and two `0x0085`
wing/wingtip rows, 10 each, 132 placements, plus the built-in `M61.JT` on its
own row), from 182 to 188 tank placements (its centreline row and the two wing
rows, each taking `F150.GAS` or `F250.GAS`) and from 174 to 182 Cheat-only
novelty placements (Boulder and Moose on the four wing and wingtip rows). The
designator-pod count moved from 65 to 67: the F-16A's two `0x07e5` rows carry
the decoded pod bit `0x400`, so a `PAVESPK.SEE` pod loads on each, even though
the record has no designator row, and the Load Ordnance page warns there is no
built-in designator. The recovered-station, tank-record and weapon-record counts
are unchanged. The per-aircraft line is `ordnance F16.PT: 6 source rows, 0
optional weapon rows, 133 compatible weapon placements, 6 tank placements;
defaults and restart passed; catalog 57 normal, 83 Cheat`. The stations, the
compatibility masks, the default load and the clipping comparison are in
[aircraft ordnance](../formats/aircraft-ordnance.md#the-f-16a-m3-2026-11-08) and
[stores on pylons](../spec/stores-on-pylons.md#the-f-16a). No record, mask or
station position was added for the aircraft; the counts move only because an
already reviewed identity is now measured.

The battery `menus-ordnance-availability` expected line was stale at 37 aircraft
/ 3,684 / 182 from slice F16A-1; it now reads the 38-aircraft line above. The
`a2g-catalog` scenario was staler still, at 36 aircraft / 13 recovered stations
/ 175 tank placements / 168 novelty placements (from before the MiG-25); its
audit and catalog counts now read the current 38-aircraft values.

## The MiG-31 joins the roster (M31-1, 2026-11-08)

`MIG31.PT` joined `AircraftId` at the end (checkpoint code 39), so the audit now
checks 39 aircraft. The pins in `ordnance_audit.rs` moved on purpose, with no
record, mask or station position added: from 3,817 to 3,892 weapon placements,
from 188 to 192 tank placements, from 182 to 186 Cheat-only novelty placements
and from 67 to 68 designator-pod placements. The recovered-station, tank-record
and weapon-record counts are unchanged. The per-aircraft line measured with the
decoded record and the interim live-fire list is `ordnance MIG31.PT: 3 source
rows, 0 optional weapon rows, 75 compatible weapon placements, 4 tank
placements; defaults and restart passed; catalog 62 normal, 83 Cheat`. These
are first measurements of a record that is not yet reviewed station by
station; slices M31-5 and M31-6 own the station, pylon and ordnance review and
may move them again. The `menus-ordnance-availability` and `a2g-catalog`
battery expected lines now read 39 aircraft, 3,892 / 192 / 186.

```text
ordnance audit: 39 aircraft, 14 recovered weapon stations, 3892 weapon placements, 192 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 68 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 186 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

## MiG-31 ordnance (2026-11-08, slice M31-6)

The MiG-31's three stations (the internal gun and the two carrier rows) are
reviewed and measured on the branch. M31-5 moved only the drawn positions of the
two carrier rows, so no pin moved here: the audit still reports the same 39
aircraft, 3,892 weapon placements, 192 tank placements, 186 Cheat-only novelty
placements and 68 designator-pod placements as the M31-1 measurement above. The
per-aircraft line is unchanged: `ordnance MIG31.PT: 3 source rows, 0 optional
weapon rows, 75 compatible weapon placements, 4 tank placements; defaults and
restart passed; catalog 62 normal, 83 Cheat`. The 75 weapon placements are 61 on
the row 4 `0x07e5` mask, 13 on the row 5 `0x0085` mask and the gun's own
`GSH6_30.JT` on row 3; the 4 tank placements are the four 150 to 500 gallon
records the row 4 tank bit admits. The stations, the two compatibility masks, the
default load and the countermeasures (chaff 60, flare 60 from `MIG31.ECM`) are in
[aircraft ordnance](../formats/aircraft-ordnance.md#the-mig-31-m3-2026-11-08).
No record, mask or station position was added or changed by this review.

The branch's `flight` battery now registers `mig31` in `STATIONS`
(`[260, 4, 2]`): the three `flight-livefire-mig31-slot1` to `slot3` scenarios
fire the gun, the `AA9.JT` row and the `AA8.JT` row, and the two
`flight-jettison-mig31-slot2` and `slot3` scenarios empty each carrier row, all
passing. The gun's live release is 260 rounds, the `AA9.JT` row four stores and
the `AA8.JT` row two, matching the source defaults. `--validate-tanks --aircraft
mig31` is refused by design (the F-14 contract only); the MiG-31's tank row is
the empty row 4 tank station and is covered by `--validate-ordnance`.

## The Mirage 2000 joins the roster (M2K-1, 2026-11-09)

`M2000.PT` joined `AircraftId` at the end (checkpoint code 40), so the audit now
checks 40 aircraft. The pins in `ordnance_audit.rs` moved on purpose, with no
record, mask or station position added: from 3,892 to 4,076 weapon placements,
from 192 to 201 tank placements, from 186 to 194 Cheat-only novelty placements
and from 68 to 71 designator-pod placements. The recovered-station, tank-record
and weapon-record counts are unchanged. The per-aircraft line measured with the
decoded record and the interim live-fire list (`DEFA.JT`, `R530.JT`, `R550.JT`,
`AS15.JT`) is `ordnance M2000.PT: 6 source rows, 0 optional weapon rows, 184
compatible weapon placements, 9 tank placements; defaults and restart passed;
catalog 62 normal, 83 Cheat`. These are first measurements of a record that is
not yet reviewed station by station; slices M2K-5 and M2K-6 own the station,
pylon and ordnance review and may move them again. The
`menus-ordnance-availability` and `a2g-catalog` battery expected lines now read
40 aircraft, 4,076 / 201 / 194.

```text
ordnance audit: 40 aircraft, 14 recovered weapon stations, 4076 weapon placements, 201 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 71 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 194 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

## The Mirage 2000 belly pairs take no big tanks (M2K-5, 2026-11-10)

John decided on 2026-11-10 that the Mirage 2000's belly pairs (`M2000.PT` row 5)
take no 350 or 500 gallon tank (they crossed the centreline tank). Normal
loading refuses `F350.GAS` and `F500.GAS` there (`tank_allowed`); Cheat loading is
unchanged. The audit pin moved on purpose from 201 to 199 tank placements; the
weapon (4,076), Cheat-only (194) and pod (71) pins did not move. The per-aircraft
line reads `ordnance M2000.PT: 6 source rows, 0 optional weapon rows, 184
compatible weapon placements, 7 tank placements; defaults and restart passed;
catalog 62 normal, 83 Cheat`. The `menus-ordnance-availability` and `a2g-catalog`
battery expected lines now read 199.

```text
ordnance audit: 40 aircraft, 14 recovered weapon stations, 4076 weapon placements, 199 tank placements, 82 weapon and 9 tank records; passed
```

