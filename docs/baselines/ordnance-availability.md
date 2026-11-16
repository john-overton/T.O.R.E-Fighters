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

## The Mirage 2000 carries no wingtip row (M2K-5, 2026-11-10)

John decided on 2026-11-10 that TORE carries no store on the Mirage 2000's
wingtip row (the Rafale template's rail; the real aircraft has nine stations and
no tip rail). `aircraft_stations::retire` drops the decoded row 8 after decoding,
so normal loading, Cheat loading, the default load and the AI all see eight
rows. The pins moved on purpose: 4,076 to 4,066 weapon placements (the ten
compatible placements of the row) and 194 to 192 Cheat-only novelty placements;
the 199 tank and 71 pod placements did not move. The per-aircraft line reads
`ordnance M2000.PT: 5 source rows, 0 optional weapon rows, 174 compatible weapon
placements, 7 tank placements; defaults and restart passed; catalog 62 normal,
83 Cheat`. The `menus-ordnance-availability` and `a2g-catalog` battery expected
lines now read 4,066 and 192.

```text
ordnance audit: 40 aircraft, 14 recovered weapon stations, 4066 weapon placements, 199 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 2 novelty records, 192 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```


## Mirage 2000 ordnance (M2K-6, 2026-11-10)

The Mirage 2000's eight rows (three fixed sensors, the DEFA gun, the centreline
tank row and the three weapon rows; the wingtip row is retired) are reviewed and
measured on the branch. No placement count moved, so the audit pins are the ones
recorded in the [wingtip section above](#the-mirage-2000-carries-no-wingtip-row-m2k-5-2026-11-10):
40 aircraft, 4,066 weapon placements, 199 tank placements, 192 Cheat-only
novelty placements and 71 designator-pod placements. The per-aircraft line reads
`ordnance M2000.PT: 5 source rows, 0 optional weapon rows, 174 compatible weapon
placements, 7 tank placements; defaults and restart passed; catalog 62 normal,
83 Cheat`, and the store-pair audit reads `M2000.PT row=5 maxItems=4 ...
default=AS15.JT count=4`, `row=6 maxItems=2 ... default=R530.JT count=2`,
`row=7 maxItems=2 ... default=R550.JT count=2`, then `store-pair audit: M2000.PT
defaults, maxima, steps, saved loads and lobby checked`. The default load, the
countermeasures (30 chaff and 30 flares from `F18.ECM`) and the gun (250 DEFA
rounds) are described in [aircraft ordnance](../formats/aircraft-ordnance.md#the-mirage-2000-m2k-6-2026-11-10).

Headless and windowed runs on the branch (all with `TORE_DATA_DIR` set to the
local profile):

| Check | Result |
| --- | --- |
| `--live-fire --aircraft m2000 --weapon-slot 1` (DEFA) | 250 shots, 62 hits, 1 kill, ammunition `[0, 4, 2, 2]` |
| `--weapon-slot 3` (Super 530) and `4` (Magic) | one shot each, one hit and one kill, the other stations untouched |
| `--weapon-slot 2` (AS-15) | refused at the practice aircraft (a surface missile), zero shots, recorded in `REFUSED_SLOTS` |
| `--a2g-run --attack sarh --load AS15 --station 1` | designated, CUED, launched, locked 23 seconds, target killed |
| `--a2g-run --attack release` on stations 1, 2 and 3 (`a2g-release-origin-m2000`) | each store leaves from where it hangs: AS-15s two tandem pairs, front first, left then right at 1.82 ft out; Super 530s 4.99 ft out; Magics 9.08 ft out; left before right |
| `--a2g-run --attack jettison --load MK82` | 990 lb of tank fuel and 4,680 lb to 3,582 lb payload at the tank release, then the belly bombs, `PASS released 5 ... no-explosion` |
| `--live-fire --combat-command jettison` on slots 2 to 4 | each station jettisons or fires, the others untouched |
| `--combat-command chaff` and `flare` with `--countermeasure-preview 60` | carried 29 chaff or 29 flares, capacity 30 and 30 |

`tools/battery.py` now carries `flight-livefire-m2000-slot1` to `slot4`,
`flight-jettison-m2000-slot2` to `slot4`, `a2g-release-origin-m2000`,
`a2g-sarh-as15-m2000` and `a2g-jettison-m2000`, all passing. The Load Ordnance
page (`--quick-mission --snapshot-state ordnance --aircraft m2000`) reads `Internal
Gun DEFA 30mm 250 (max 250)`, `Centerline 150 gallon tank 1 loaded (max 1)`,
`Fuselage AS 15 4 loaded (max 4)`, `Wing Super 530D 2 loaded (max 2)` and `Wing
R-550 Magic 2 2 loaded (max 2)`, with no wingtip row.

## Batch 5 identities join the roster (B5-1, 2026-11-10)

`F15E.PT`, `GRIPEN.PT`, `F5E.PT`, `TOR.PT` and `MF1.PT` joined `AircraftId` at
the end (checkpoint codes 41 to 45), so the audit now checks 45 aircraft. The
pins in `ordnance_audit.rs` moved on purpose, with no record, mask or station
position added: the roster from 40 to 45, recovered (optional) weapon stations
from 14 to 17 (the Gripen has one and the Mirage F1 two rows with no default
weapon), weapon placements from 4,066 to 4,663, tank placements from 199 to 231,
Cheat-only novelty placements from 192 to 222 and designator-pod placements from
71 to 80. The weapon-record (82) and tank-record (9) counts are unchanged:
`BK27.JT`, the Gripen's and the Tornado's gun, is not a supported weapon. It is
admitted only as the station's own internal gun default (`loadout.rs`, the
`own_gun` case in `validate_for_editing`, and the live-fire list), and stays out
of the catalog and the Cheat catalog; the ordnance slices own it. These are first
measurements of records not yet reviewed station by station. Per-aircraft lines,
measured with the decoded records and the interim live-fire lists:

```text
ordnance F15E.PT: 4 source rows, 0 optional weapon rows, 65 compatible weapon placements, 3 tank placements; defaults and restart passed; catalog 51 normal, 83 Cheat
ordnance GRIPEN.PT: 5 source rows, 1 optional weapon rows, 190 compatible weapon placements, 12 tank placements; defaults and restart passed; catalog 60 normal, 83 Cheat
ordnance F5E.PT: 4 source rows, 0 optional weapon rows, 71 compatible weapon placements, 5 tank placements; defaults and restart passed; catalog 61 normal, 83 Cheat
ordnance TOR.PT: 5 source rows, 0 optional weapon rows, 96 compatible weapon placements, 8 tank placements; defaults and restart passed; catalog 61 normal, 83 Cheat
ordnance MF1.PT: 5 source rows, 2 optional weapon rows, 175 compatible weapon placements, 4 tank placements; defaults and restart passed; catalog 62 normal, 83 Cheat
```

The `menus-ordnance-availability` and `a2g-catalog` battery expected lines now
read 45 aircraft, 17 recovered stations, 4,663 / 231 / 222.

```text
ordnance audit: 45 aircraft, 17 recovered weapon stations, 4663 weapon placements, 231 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 222 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

## Batch 5 record and station corrections (B5-2, 2026-11-10)

The Tornado's outer wing row is retired, the F-5E's wing pair and jammer defaults
are cleared (its radar is the F-104's) and the Mirage F1's centreline pod default
is cleared ([stores on pylons](../spec/stores-on-pylons.md#batch-5-stations-tornado-outer-wing-mirage-f1-centreline-f-5e-defaults-b5-2-2026-11-10)).
The audit pins moved: recovered weapon stations 17 to 18 (the F-5E wing pair has no
default weapon), weapon placements 4,663 to 4,652 (the Tornado's outer row held
eleven), tank placements 231 to 233 (the Mirage F1 centreline takes two tanks now
that no pod occupies it), Cheat-only placements 222 to 220 (the Tornado row held
two). Pods (80), weapon records (82) and tank records (9) are unchanged. The
per-aircraft lines changed:

```text
ordnance F5E.PT: 4 source rows, 1 optional weapon rows, 71 compatible weapon placements, 5 tank placements; defaults and restart passed; catalog 61 normal, 83 Cheat
ordnance TOR.PT: 4 source rows, 0 optional weapon rows, 85 compatible weapon placements, 8 tank placements; defaults and restart passed; catalog 61 normal, 83 Cheat
ordnance MF1.PT: 5 source rows, 2 optional weapon rows, 175 compatible weapon placements, 6 tank placements; defaults and restart passed; catalog 62 normal, 83 Cheat
ordnance audit: 45 aircraft, 18 recovered weapon stations, 4652 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 220 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The `menus-ordnance-availability` and `a2g-catalog` battery expected lines read 18,
4,652 (menus only), 233 and 220.

## F-15E stations (F15E-5, 2026-11-10)

The F-15E's bomb pair (row 7) refuses seven large stores in normal loading
(`GBU29.JT`, `GBU29P.JT`, `GBU30.JT`, `GBU38.JT`, `AGM88.JT`, `AGM45.JT`, `AT2.JT`)
and its wing pair (row 8) refuses `AA6.JT` (John, 2026-11-10, opinionated; Cheat
loading unchanged; see [the F-15E in stores on
pylons](../spec/stores-on-pylons.md#the-f-15e-f15e-5-2026-11-10)). The audit pin
moved: weapon placements 4,652 to 4,644 (`F15E.PT` 65 to 57); tank placements
(233), Cheat-only placements (220), pods (80), records (82 and 9), recovered
stations (18) and the roster (45) are unchanged:

```text
ordnance F15E.PT: 4 source rows, 0 optional weapon rows, 57 compatible weapon placements, 3 tank placements; defaults and restart passed; catalog 48 normal, 83 Cheat
ordnance audit: 45 aircraft, 18 recovered weapon stations, 4644 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
```

The `menus-ordnance-availability` battery expected line reads 4,644 weapon
placements; the `a2g-catalog` line matches any count.

## F-15E ordnance (F15E-6, 2026-11-10)

The F-15E's nine rows (five fixed sensors and the placeholder, the M61 gun, the
centreline tank row, the belly Mk 82 row and the wing AIM-120 row) are reviewed and
measured on the branch. No placement count moved: the audit pins are the ones in the
[F-15E stations section above](#f-15e-stations-f15e-5-2026-11-10) (45 aircraft, 18
recovered weapon stations, 4,644 weapon placements, 233 tank placements, 80 pod
placements, 220 Cheat-only). The default load, the gun (450 M61 rounds, the real jet's
512 a finding only), the countermeasures (30 chaff and 30 flares from `F15.ECM`) and the
release positions are described in [aircraft
ordnance](../formats/aircraft-ordnance.md#the-f-15e-strike-eagle-f15e-6-2026-11-10).

Headless and windowed runs on the branch (all with `TORE_DATA_DIR` set to the local
`profile-f15e`; logs under `.local/m3-aircraft/checks/F15E-6/`):

| Check | Result |
| --- | --- |
| `--live-fire --aircraft f15e --weapon-slot 1` (M61), 1,200 ticks | 320 shots, 52 hits, 1 kill, ammunition `[130, 12, 4]` |
| `--weapon-slot 2` (Mk 82) | one shot, one hit and one kill, ammunition `[450, 11, 4]` |
| `--weapon-slot 3` (AIM-120), 1,200 / 1,800 / 3,600 ticks | one shot, hit 0 at 1,200 (still flying), hit and kill at 1,800 and 3,600; ammunition `[450, 12, 3]` |
| `--a2g-run --attack strafe` | 64 rounds in 2 s, 22 tracers, 450 to 386 |
| `--a2g-run --attack release` stations 1 and 2 (`a2g-release-origin-f15e`) | Mk 82s from under the rack bars (6.33 ft out), then the 7.02 ft and 5.64 ft columns, front pair first; AIM-120s outboard pair (14.96 ft) then inboard (13.70 ft); left before right |
| `--a2g-run --attack jettison --load MK82` | tank (2,300 lb fuel; payload 9,940 to 7,392 lb) then 12 bombs (to 1,392 lb), `PASS released 13 ... no-explosion` |
| `--a2g-run --attack laser --load GBU10 --station 1 --count 2` from 15,000 ft | Pave Spike lases, released 15.2 s, guided, impact 3.3 ft from the tank, killed |
| `--a2g-run --attack laser --load GBU12 --station 1 --count 4` | released 15.2 s, impact 4.0 ft from the tank, tank at 100 of 200 hit points |
| `--live-fire --combat-command chaff` and `flare` with `--countermeasure-preview 60` | carried 29 chaff or 29 flares, capacity 30 and 30 |
| `--ai-probe-ticks 60 --aircraft f15e` | `tanks=1 external_fuel=2298 payload=9938 stations=[M61 x450, MK-82 x12, AIM-120 x4]` |
| `--quick-mission --snapshot-state ordnance --aircraft f15e` | `M61 Vulcan Cannon 450 (max 450)`, `350 gallon tank 1 loaded`, `MK-82 Slick 12 loaded (max 12)`, `AIM-120 AMRAAM 4 loaded (max 4)`; weight 52,202 of 68,000 lb |
| `TORE_STORES_PREVIEW_FIRE=5`, `7`, `8` | `Stores firing: f15e row=N PASS` for the gun, Mk 82 and AIM-120 rows |

`tools/battery.py` now carries `flight-livefire-f15e-slot1` to `slot3`,
`flight-jettison-f15e-slot2` and `slot3`, `a2g-release-origin-f15e`,
`a2g-jettison-f15e` and `a2g-laser-f15e`, all passing (8 of 8 on the last run).

## Saab Gripen ordnance (GRI-6, 2026-11-10)

The Gripen's default load, BK 27 gun and countermeasures were measured on the
`m3-gripen` branch (HEAD d8c6725b plus this slice), profile
`.local/m3-aircraft/profile-gripen`, away mode, everything through `light3.sh`.
No station, mask, record or pin changed, so the audit counts are the B5-2 ones
(45 aircraft, 18 recovered stations, 4,652 weapon placements, 233 tank
placements, 82 and 9 records, 80 pods, 220 Cheat-only) and no pin note is needed.
Per-aircraft line, unchanged:

```text
ordnance GRIPEN.PT: 5 source rows, 1 optional weapon rows, 190 compatible weapon placements, 12 tank placements; defaults and restart passed; catalog 60 normal, 83 Cheat
```

What was measured (details in [aircraft ordnance](../formats/aircraft-ordnance.md#the-saab-gripen-gri-6-2026-11-10)):

| Check | Command | Result |
| --- | --- | --- |
| Default rows | retail test `gripen_ordnance` | nine rows: three sensors, AAS-38, `BK27.JT` 570, `F150.GAS` 1, `AGM65G.JT` 2, `AIM120.JT` 4, `AIM9M.JT` 2; `F18.ECM` 30 chaff and 30 flares |
| BK 27 record | the same test | initial speed 2,933 ft/s, 2 rounds per shot, damage 25 / 2 / 7 / 7 / 25, `&FSTGUN3.11K`, burst, movement and damage equal to M61 and DEFA |
| Gun | battery `flight-livefire-gripen-slot1` (`--live-fire --weapon-slot 1`) | 320 shots, 59 hits, 1 kill, 570 to 250 |
| Gun key | `--a2g-run --attack strafe` | Tab held 2 s: 64 rounds (32 a second), 22 tracers |
| AIM-9M | `flight-livefire-gripen-slot4` | 1 shot, hit, kill |
| AIM-120 | `flight-livefire-gripen-slot3` (1,200 ticks), then `--combat-probe-ticks 3600` | 1 shot in flight at 1,200 ticks; hit and kill at 3,600 (the F-16A behaves the same) |
| AGM-65G | `a2g-maverick-gripen` (`--attack maverick`) | designated, CUED, launched, 18 s locked, target killed (hp 0/200) |
| Release origins | `a2g-release-origin-gripen` | AGM-65G +-5.00 / -2.21 / -4.67, AIM-120 +-10.63 then +-9.37 / -1.96 / -7.00, AIM-9M +-13.67 / -0.67 / -8.67, left then right, each at its hang point |
| Jettison | `a2g-jettison-gripen`, `flight-jettison-gripen-slot2..4` | tank (990 lb of fuel) then the Mk 82s, no explosion |
| Countermeasures | `--combat-command chaff` and `flare` with `--countermeasure-preview 60` | 29 / 30 and 30 / 29 carried, capacity 30 and 30 |
| AI | `--ai-probe-ticks 60 --probe-fight 1:1` | `tanks=1 external_fuel=989 payload=4209 stations=[BK27 x570, AGM-65 x2, AIM-120 x4, AIM-9M x2, AGM-65 x0]` |

Battery scenarios added (all pass): `a2g-release-origin-gripen`,
`a2g-maverick-gripen`, `a2g-jettison-gripen`, and the `STATIONS["gripen"]`
entry `[570, 2, 4, 2, 0]` that creates `flight-livefire-gripen-slot1..5` and
`flight-jettison-gripen-slot2..4` (`("gripen", 2)` joined `REFUSED_SLOTS`, the
AGM-65G refusing the practice aircraft). Raw logs are local only
(`.local/m3-aircraft/checks/GRI-6/`).

Frames (local, `.local/m3-aircraft/frames/GRI-6/`): `default-load-below`,
`default-load-side`, `missile-agm65g-just-after-release-t2.40` (a crop of the
a2g-run scene), `missile-row7-just-after-release-t004` (AIM-120),
`missile-row8-just-after-release-t004` (AIM-9M), `gun-firing` (the tracer leaving
the nose), `chaff-release`, `flare-release`. The AGM-65G row cannot fire in the
stores preview (it refuses without a designated target), so its frame is the
headless run's scene.

## F-5E ordnance (F5E-6, 2026-11-10)

The F-5E's seven rows (the sensor, the `F104R.SEE` radar, the `F104.ECM` dispenser
row, the M61 gun, the centreline tank row, the empty wing pair and the wingtip
pair) are reviewed and measured on the branch. No placement count moved, so the
audit pins are the B5-2 ones recorded in the
[corrections section above](#batch-5-record-and-station-corrections-b5-2-2026-11-10):
45 aircraft, 18 recovered weapon stations, 4,652 weapon placements, 233 tank
placements, 220 Cheat-only novelty placements and 80 designator-pod placements.
The per-aircraft line is unchanged (`ordnance F5E.PT: 4 source rows, 1 optional
weapon rows, 71 compatible weapon placements, 5 tank placements; defaults and
restart passed; catalog 61 normal, 83 Cheat`) and the store-pair audit reads
`F5E.PT row=5 maxItems=4 ... default=AIM9B.JT count=0` and `row=6 maxItems=2 ...
default=AIM9B.JT count=2`. The default load, countermeasures and gun are described in
[aircraft ordnance](../formats/aircraft-ordnance.md#the-f-5e-tiger-f5e-6-2026-11-10).

Headless and windowed runs on the branch (profile `profile-f5e`):

| Check | Result |
| --- | --- |
| `--live-fire --aircraft f5e --weapon-slot 1` (M61) | 320 shots, 47 hits, 1 kill, ammunition `[240, 2, 0]` |
| `--weapon-slot 2` (AIM-9B) | one shot, one hit, no kill, the other stations untouched, ammunition `[560, 1, 0]` |
| `--weapon-slot 3` (empty wing pair) | stays at 0; the cycle fires the gun, 320 shots |
| `--a2g-run --attack release` on stations 2 and 1 (`a2g-release-origin-f5e`) | Mk 82 and AGM-65G leave their 1 ft wing pylons 8.02 / 6.64 and 8.18 / 6.48 ft out, left first then alternating; AIM-9Bs 14.33 ft out, left then right |
| `--a2g-run --attack maverick --load AGM65G --station 2` (`a2g-maverick-f5e`) | launched from the wing pair, target killed |
| `--a2g-run --attack jettison --station 2 --load MK82 --count 4` (`a2g-jettison-f5e`) | tank 990 lb to 0, then four bombs, `PASS released 5 ... no-explosion` |
| `--a2g-run --attack strafe` | 64 rounds in a 2 second hold, 22 tracers |
| `--combat-command chaff` and `flare` with `--countermeasure-preview 60` | carried 19 chaff or 19 flares, capacity 20 and 20; five chaff and three flares leave 15 and 17 |

`tools/battery.py` carries `flight-livefire-f5e-slot1` to `slot3`,
`flight-jettison-f5e-slot2`, `a2g-release-origin-f5e`, `a2g-maverick-f5e` and
`a2g-jettison-f5e`, all passing.

## Mirage F1 belly row capped at two (MF1-5b, 2026-11-11)

John's round-3 answer (M1, 2026-11-11, opinionated): `MF1.PT` row 5 (Wing, paired,
four `F250.GAS`, class 46) holds two stores, one a side, and hangs 3.24 ft out; the
wingtip Magic (row 7) stands 1.1 ft outboard of the tip. No placement count moved:
`--validate-ordnance` still reads `ordnance MF1.PT: 5 source rows, 2 optional weapon
rows, 175 compatible weapon placements, 6 tank placements` and `ordnance audit: 45
aircraft, 18 recovered weapon stations, 4652 weapon placements, 233 tank placements,
82 weapon and 9 tank records; passed` with 80 pod and 220 Cheat-only placements, so
neither the pins in `ordnance_audit.rs` nor the battery expected lines changed. What
changed is capacity: all 181 catalog choices remain; the 46 row 5 records that took
four now take two (the other 19 already took two), the odd-weight "3 to 2" store-pair
reductions for `AGM84A`, `AGM84E`, `AM39`, `AS14` and `MK82P3` on row 5 no longer
occur, and the bomb carriage of `GBU12`, `GBU16` and `GBU38` reads 2 on that row (was
4). Frames and the per-record clearance of the alternatives: [stores on
pylons](../spec/stores-on-pylons.md#the-mirage-f1-stations-finished-mf1-5b-2026-11-11).

Update (MF1-6, 2026-11-11): the pins in the paragraph above no longer hold after the
mask below ([the Mirage F1 refusals](../spec/stores-on-pylons.md#the-mirage-f1-refusals-mf1-6-2026-11-11)).

## Mirage F1 ordnance masks and default load (MF1-6, 2026-11-11)

Agent decision following John's F-15E A2 answer (the 0.3 ft line; the F-5E kept its
wing entries, C2): normal loading refuses 33 stores on the Mirage F1's belly pair
(row 5, 3.24 ft out) that enter the airframe by more than about 0.3 ft and the
`AS30.JT` on its paired wing row (row 6, 0.33 ft); the default `R550.JT` is never
refused. The audit pins moved: weapon placements 4,652 to 4,618 (`MF1.PT` 175 to
141). Recovered stations (18), tank
placements (233), Cheat-only placements (220), pods (80) and the weapon and tank
records (82 and 9) did not move. The per-aircraft and audit lines:

```text
ordnance MF1.PT: 5 source rows, 2 optional weapon rows, 141 compatible weapon placements, 6 tank placements; defaults and restart passed; catalog 60 normal, 83 Cheat
ordnance audit: 45 aircraft, 18 recovered weapon stations, 4618 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 220 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The `menus-ordnance-availability` expected line reads 4,618 (menus only). The audit
also fails when a refused store is still offered on a row that has a weapon slot.
The totals in this file for other aircraft's refusals (the F-15E's eight) are the
coordinator's to recompute at integration (B4). `--stores-preview` with the catalog
flag lists 147 choices for the Mirage F1 (was 181); the diff is exactly the 34
refusals. Default load: gun 250, centreline empty, two `F250.GAS`, wing and wingtip
Magics (26,143 lb of 30,203 lb); live-fire stations `[250, 2, 2, 0, 0]`. Commands,
frames and the release origins are in [aircraft
ordnance](../formats/aircraft-ordnance.md#the-mirage-f1-mf1-6-2026-11-11).

## Mirage F1 merge: audit pins recomputed (2026-11-11)

The Mirage F1 branch (`m3-mf1`, through MF1-9) merged into `m3-aircraft`. The F-15E's
refusals (eight placements, F15E-5) and the Mirage F1's (34, MF1-6) are
independent, so the weapon placements add: 4,652 less 8 less 34 is 4,610. The
numbers below are measured with `--validate-ordnance --no-audio` on the merged tree
(profile `.local/m3-aircraft/profile`), not computed; the two sections above that quote
4,644 (the F-15E, before the merge) and 4,618 (the Mirage F1, before the merge) stay
as written for their own states:

```text
ordnance F15E.PT: 4 source rows, 0 optional weapon rows, 57 compatible weapon placements, 3 tank placements
ordnance MF1.PT: 5 source rows, 2 optional weapon rows, 141 compatible weapon placements, 6 tank placements
ordnance audit: 45 aircraft, 18 recovered weapon stations, 4610 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 220 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

Only the weapon placements moved against `m3-aircraft` (4,644 to 4,610). The pin in
`ordnance_audit.rs` and the `menus-ordnance-availability` expected line read 4,610;
the `a2g-catalog` line matches any count. `weapon_allowed` is one function with the
F-15E's and the Mirage F1's arms, and each loading path (`Loadout::capacity`, the
retained-selection rule in `combat/live.rs`, the audit) calls it once.

## Tornado ordnance (TOR-6, 2026-11-11)

The Tornado's eight rows (four fixed sensors, the BK27 gun, the centreline tank row,
the shoulder Paveways and the inner wing Mk 82 pair; the outer wing row is retired) are
reviewed and measured on `m3-tornado`. Normal loading now refuses 36 stores that enter the
airframe over 0.3 ft (33 on the shoulder pair, 3 on the inner wing pair; the
[measured catalog](../spec/stores-on-pylons.md#the-tornado-ordnance-masks-and-the-measured-catalog-tor-6-2026-11-11)),
so the audit pins move on this branch:

```text
ordnance TOR.PT: 4 source rows, 0 optional weapon rows, 49 compatible weapon placements, 8 tank placements; defaults and restart passed; catalog 31 normal, 83 Cheat
ordnance audit: 45 aircraft, 18 recovered weapon stations, 4616 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 220 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

Weapon placements 4,652 to 4,616 (`TOR.PT` 85 to 49); recovered stations, tank
placements, Cheat-only placements, pods, records and aircraft unchanged. The F-15E and
Mirage F1 branches' own refusals move the same pin; the coordinator recomputes the
total at integration (B4). The `menus-ordnance-availability` expected line reads 4,616
on this branch.

Headless and windowed runs on the branch (profile `profile-tornado`, windows through
`tools/agent-run.sh`):

| Check | Result |
| --- | --- |
| `--live-fire --aircraft tornado --weapon-slot 1` (BK27) | 320 shots, 51 hits, 1 kill, ammunition `[40, 4, 2]` |
| `--weapon-slot 2` (GBU-10) | one shot, ammunition `[360, 3, 2]` (the laser bomb has no designated target, no hit) |
| `--weapon-slot 3` (Mk 82 pack) | one release of three bodies: 3 shots, 3 hits, 1 kill; one round taken (`SHOTS_PER_ROUND` in `flight.py`) |
| `--weapon-slot 3 --flight-speed 200` and `650` | the same release with the wing spread (25 degrees) and swept (67 degrees); frames `mk82-sweep25-below` and `mk82-sweep67-below` |
| `--a2g-run --attack release` on stations 1 and 2 at 200 and 650 kt (`a2g-release-origin-tornado`) | shoulder Paveways from 2.00 ft out, 12.27 then -2.93 ft forward, left then right, depth 0.17 ft, identical at both sweeps; Mk 82 packs from 9.50 ft out, 1.50 ft forward at 25 degrees, and from -10.53 ft out, 0.81 ft aft (left) and 10.36 ft out, 1.26 ft aft (right) at 67 degrees, depth 1.00 ft |
| `--a2g-run --attack jettison --station 2 --load MK82P3` at 200 and 650 kt (`a2g-jettison-tornado`) | the tank leaves x 0, 5.94 ft down, 5.67 ft ahead at both sweeps; the Mk 82 packs leave right then left from 9.50 / 1.50 ft (25 degrees) and 10.36 / -1.26 and -10.53 / -0.81 ft (67 degrees), `PASS released 3 ... no-explosion`, payload 12,848 lb to 11,000 to 8,000 |
| `--live-fire --combat-command jettison` on slots 2 and 3 | the station jettisons (8,000 lb for the four Paveways, 3,000 lb for the two packs), the others untouched; after the Paveways go the selection moves on to the packs |
| `--combat-command chaff` and `flare` with `--countermeasure-preview 60` | five chaff leave 55, three flares leave 57, capacity 60 and 60 |

`tools/battery.py` carries `flight-livefire-tornado-slot1` to `slot3`,
`flight-jettison-tornado-slot2` and `slot3`, `a2g-release-origin-tornado` and
`a2g-jettison-tornado`. A new `jettison: offset` line in the `jettison` attack prints each
store's start against the aircraft (right, up, forward). The Load Ordnance page
(`--quick-mission --snapshot-state ordnance --aircraft tornado`) reads `BK27 360 (max
360)`, `250 gallon tank 1 loaded (max 2)`, `GBU-10E Paveway 4 loaded (max 4)` and `MK-82
3-Pack 2 loaded (max 2)`, 55,134 of 61,620 lb. Frames:
`.local/m3-aircraft/frames/TOR-6/`.

## Tornado AIM-9 side rails (TOR-6b, 2026-11-11)

Update to the TOR-6 section above, whose "outer wing row is retired" and eight rows are now nine:
John asked on 2026-11-11 for an AIM-9 side rail on the inboard swivelling pylon, which
`aircraft_stations::side_rail` appends as row 8 in the retired outer pair's place (the
[rules and measurements](../spec/stores-on-pylons.md#the-tornado-aim-9-side-rails-on-the-inboard-swivelling-pylons-tor-6b-2026-11-11)).
Seven stores stay on it (AIM-9B, AIM-9M, AIM-9X, AA-2, AA-8, AA-11, AA-11B) and four are refused
(PL-7, R.550, AT-12, GBU-12), so the audit pins move on this branch:

```text
ordnance TOR.PT: 5 source rows, 0 optional weapon rows, 56 compatible weapon placements, 8 tank placements; defaults and restart passed; catalog 36 normal, 83 Cheat
ordnance audit: 45 aircraft, 18 recovered weapon stations, 4623 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 222 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

Weapon placements 4,616 to 4,623 (`TOR.PT` 49 to 56) and Cheat-only placements 220 to 222 (the
row's mask admits Boulder and Moose under Cheat loading, as the retired outer row did); recovered
stations (18), tank placements, pods, records and the roster unchanged. Dated pin notes are in
`ordnance_audit.rs`, and the `menus-ordnance-availability` and `a2g-catalog` expected lines carry
the new totals. The F-15E and Mirage F1 branches move the same pins: the coordinator recomputes
the totals at integration (B4).

Runs on the branch (profile `profile-tornado`, windows through `tools/agent-run.sh`):

| Check | Result |
| --- | --- |
| `--live-fire --aircraft tornado --weapon-slot 4` at the default speed, and with `--flight-speed 200` and `650` | one AIM-9M leaves the left rail (left first), ammunition `[360, 4, 2, 1]`, payload 13,228 to 13,038 lb; frames `aim9-sweep25-*` and `aim9-sweep67-*` |
| `--a2g-run --attack release --station 3 --load AIM9M --count 2` at 200 and 650 kt (`a2g-release-origin-tornado-rail`) | left then right from (-10.75, -0.74, 1.50) and (10.75, -0.74, 1.50) at 25 degrees; (-11.78, -0.74, -0.81) and (11.61, -0.74, -1.26) at 67 degrees; depth 0.53 ft, the point it hangs at |
| `--a2g-run --attack jettison --station 3 --load AIM9M` at 200 and 650 kt (`a2g-jettison-tornado-rail`) | the tank, then both missiles from the same points; payload 13,228 to 11,380 to 11,000 lb |
| CPU stores preview, every catalog store of row 8 at 25 and 67 degrees (`checks/TOR-6b/matrix.sh`) | AIM-9B, AIM-9M, AIM-9X, AA-2, AA-8, AA-11, AA-11B 0.00 ft into the airframe; R.550 0.40, AT-12 0.44, PL-7 0.50 (0.83 at 67), GBU-12 1.02 |

Frames: `.local/m3-aircraft/frames/TOR-6b/`.

## Tornado merge: audit pins recomputed (2026-11-11)

The Tornado branch (`m3-tornado`, through TOR-9 with TOR-5 round 3, TOR-6 and TOR-6b)
merged into `m3-aircraft`. The Tornado's refusals and its AIM-9 side rail (`TOR.PT` 85 to
56 weapon placements, 29 fewer) are independent of the F-15E's and the Mirage F1's, so the
weapon placements add: 4,610 less 29 is 4,581. The numbers below are measured with
`--validate-ordnance --no-audio` on the merged tree (profile `.local/m3-aircraft/profile`),
not computed; the sections above that quote 4,616 and 4,623 (the Tornado, before the merge)
and 4,610 (the Mirage F1 merge) stay as written for their own states:

```text
ordnance F15E.PT: 4 source rows, 0 optional weapon rows, 57 compatible weapon placements, 3 tank placements; defaults and restart passed; catalog 48 normal, 83 Cheat
ordnance TOR.PT: 5 source rows, 0 optional weapon rows, 56 compatible weapon placements, 8 tank placements; defaults and restart passed; catalog 36 normal, 83 Cheat
ordnance MF1.PT: 5 source rows, 2 optional weapon rows, 141 compatible weapon placements, 6 tank placements; defaults and restart passed; catalog 60 normal, 83 Cheat
ordnance audit: 45 aircraft, 18 recovered weapon stations, 4581 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 222 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

Against `m3-aircraft` before the merge the weapon placements moved 4,610 to 4,581 and the
Cheat-only placements 220 to 222 (the Tornado's side rail row, TOR-6b); tank placements
(233), pods (80), recovered stations (18) and the roster are unchanged. The pin in
`ordnance_audit.rs` and the `menus-ordnance-availability` expected line read 4,581; the
`a2g-catalog` line matches any count and `tools/test_battery_a2g.py` pins no live count.
`weapon_allowed` is one function with the F-15E's (rows 7 and 8), the Mirage F1's (rows 5
and 6) and the Tornado's (rows 6, 7 and 8) arms, and each loading path
(`Loadout::capacity`, the retained-selection rule in `combat/live.rs`, the audit) calls it
once.

## Tornado AIM-9 rail on the inboard flank (TOR-11, 2026-11-12)

Update (TOR-11, 2026-11-12): John moved the Tornado's AIM-9 rail to the pylon's inboard flank
and shortened its drawn arm to 3.0 ft ([spec](../spec/stores-on-pylons.md#the-tornado-aim-9-rail-on-the-inboard-flank-shorter-and-the-sweep-from-350-mph-tor-11-2026-11-12)). Nothing about the stores
the row accepts changed: the inboard entries into the airframe match the outboard ones for the four
refused stores (R550 0.40, AT12 0.44, PL7 0.50, GBU12 1.02 ft, Cheat loading) and the seven kept
missiles read 0.00 ft at 25, 45 and 67 degrees, so the audit pins stand at 4,581 weapon placements,
233 tank placements and 222 Cheat-only placements (`--validate-ordnance --no-audio`).

## Bomber batch A identities join the roster (BMB-A1, 2026-11-15)

`B52.PT`, `B2.PT` and `F117.PT` joined `AircraftId` at the end (checkpoint codes
47 to 49, after the Tornado IDS (Clean)'s 46), so the audit now checks 48
aircraft. The pins in `ordnance_audit.rs` moved on purpose, with no record, mask
or station position added or corrected (retail record values; the corrections are
BMB-A2's): the roster from 45 to 48, recovered (optional) weapon stations from 18
to 20, weapon placements from 4,581 to 4,792, Cheat-only novelty placements from
222 to 240 and designator-pod placements from 80 to 84. Tank placements (233) and
the weapon-record (82) and tank-record (9) counts are unchanged. Per pin:

- **Recovered stations 18 to 20.** The B-52's bay rows 5 and 6 (flags `0x1120`,
  no default store) kept no empty selection: the Harpoon and the M61, its only
  defaults, fit neither row, so the audit reported the rows as hiding compatible
  weapons. `live.rs` now gives them an empty Mk 82 selection (the same fallback
  device as the Mirage F1's belly pair), which makes the two rows recovered
  stations.
- **Weapon placements 4,581 to 4,792.** B-52 71, B-2 102, F-117 38, first
  measurements of the retail rows (the B-2's 32-item Mk 82 rows and 4-item
  Paveway rows, the F-117's two single GBU-27 rows, the B-52's three bay rows).
- **Cheat-only placements 222 to 240.** 18 more pairs on the three aircraft's bay
  rows.
- **Designator pods 80 to 84.** The B-2's four bay rows carry flags `0x1504`, which
  include the designator bit `0x400`, so each takes one pod. This is a retail row
  flag on an aircraft the real B-2 gives no pod station (B7); it is left as the
  record has it for the B-2's station slice (or BMB-A2) to decide.

Per-aircraft lines, measured with the decoded records and the live-fire lists of
BMB-A1:

```text
ordnance B52.PT: 4 source rows, 2 optional weapon rows, 71 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 26 normal, 83 Cheat
ordnance B2.PT: 4 source rows, 0 optional weapon rows, 102 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 26 normal, 83 Cheat
ordnance F117.PT: 2 source rows, 0 optional weapon rows, 38 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 19 normal, 83 Cheat
```

The `menus-ordnance-availability` and `a2g-catalog` battery expected lines and
`tools/test_battery_a2g.py` now read 48 aircraft, 20 recovered stations, 233 tank
placements and 240 Cheat-only placements (4,792 weapon placements in the menus
line; the a2g line matches any count).

```text
ordnance audit: 48 aircraft, 20 recovered weapon stations, 4792 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 84 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 240 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

Earlier sections that quote 45 aircraft, 4,581 weapon placements and 80 pods
stay as written for their own states.

## Bomber batch A record and station corrections (BMB-A2, 2026-11-15)

BMB-A2 changed one retail row flag and the ordnance audit pins moved on purpose
(no record, mask or default weapon of any other aircraft changed; the B-52 gun
rounds and the F-117 jammer row are not placements). The B-2's four bay rows
(`B2.PT` rows 5 to 8) had flags `0x1504`; the designator bit `0x400` is cleared
(`0x1104`, fitted, agent decision 2026-11-15, B7: the real B-2 has no pod station;
the BMB-A1 finding the coordinator assigned to this slice). The bit also admits
stores with more than one projectile in a pod (`pod_count > 1`, `loading.rs`), so
four such stores leave each bay:

| Pin | From | To | Why |
| --- | ---: | ---: | --- |
| Weapon placements | 4,792 | 4,776 | `B2.PT` 102 to 86 (the B-2 catalog 26 to 22 normal stores) |
| Designator-pod placements | 84 | 80 | The B-2's four bay rows lose the bit |
| Aircraft, recovered stations, tank placements, Cheat-only, records | 48, 20, 233, 240, 82 / 9 | unchanged | |

The F-117's row 2 has no default `F18.ECM` and the B-52's tail gun row holds 1,242
rounds; neither moves a placement. The audit also runs the loadout validator for every
aircraft, so it covers the B-52's default load: the new starting-tank cap
(303,000 lb, [aircraft records](../spec/aircraft-records.md#bomber-batch-a-record-corrections-b-52h-b-2a-f-117a-bmb-a2-2026-11-15))
lets the load pass its maximum take-off check.

```text
ordnance B2.PT: 4 source rows, 0 optional weapon rows, 86 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 22 normal, 83 Cheat
ordnance audit: 48 aircraft, 20 recovered weapon stations, 4776 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 240 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The `menus-ordnance-availability` battery expected line now reads 4,776 weapon
placements. The `a2g-catalog` line and `tools/test_battery_a2g.py` match any weapon
and pod count, so they are unchanged. Earlier sections that quote 4,792 weapon
placements and 84 pods stay as written for their own states.

## F-117 weapon bays take the Paveway II bombs (F117-5, 2026-11-15)

Opinionated (John, 2026-11-15, decision 25: "extra loads only where the weapons data
already has them"). The F-117's two bay rows (`F117.PT` rows 4 and 5, flags `0x1100`, one
store each) are now fitted magazine stations
([F-117 weapon bays](../spec/stores-on-pylons.md#f-117-weapon-bays-f117-5-2026-11-15));
the retail mask admits 19 stores and the bomb-only bit leaves out the laser bombs, whose
seeker signature needs the infrared bit. `aircraft_stations::extra_weapon` now admits the
Paveway II bombs on those two rows (`GBU10.JT`, `GBU10A.JT`, `PAVEWAY.JT`, `GBU12.JT`, the
row's own count of one, no weight cap), and `weapon_allowed` refuses `GBU16.JT` by name (the
audit's donor rule: `GBU12.JT` and `GBU16.JT` share the `PAVEWAY.JT` donor, so a variant
that is not offered must be stated). The measured bay clearances leave every store the mask
and the extras admit clear of the doors, so no store is refused for fit. One pin moved on
purpose:

| Pin | From | To | Why |
| --- | ---: | ---: | --- |
| Weapon placements | 4,776 | 4,784 | `F117.PT` 38 to 46 (four stores on each of two rows; its normal catalog 19 to 23) |
| Aircraft, recovered stations, tank placements, pods, Cheat-only, records | 48, 20, 233, 80, 240, 82 / 9 | unchanged | |

```text
ordnance F117.PT: 2 source rows, 0 optional weapon rows, 46 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 23 normal, 83 Cheat
ordnance audit: 48 aircraft, 20 recovered weapon stations, 4784 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 240 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The `menus-ordnance-availability` expected line reads 4,784. Earlier sections stay as
written for their own states.


## B-2 weapon bays: grid, alternatives and four refusals (B2-5, 2026-11-15)

Opinionated (John, 2026-11-15, decision 21). The B-2's four bay rows (`B2.PT` rows 5 to 8) are
fitted magazine stations with a grid
([B-2 weapon bays](../spec/stores-on-pylons.md#b-2-weapon-bays-b2-5-2026-11-15)); the pair flag is
cleared (flags `0x1104` to `0x1100`), a bay's Mk 82 row and Paveway III row are alternatives, and
the Paveway rows start empty. Every bay's capacity is capped to what fits under its roof (GBU-38 24, CBU-87 18,
Mk 84 8, GBU-27 4; the default Mk 82 32). The two Mk 82 rows refuse four stores that reach more than 0.3 ft
through the roof (`GBU29`, `GBU29P`, `GBU30`, `MK20`). One pin moved on purpose:

| Pin | From | To | Why |
| --- | ---: | ---: | --- |
| Weapon placements | 4,784 | 4,776 | `B2.PT` 86 to 78 (four stores refused on each of two rows) |
| Aircraft, recovered stations, tank placements, pods, Cheat-only, records | 48, 20, 233, 80, 240, 82 / 9 | unchanged | |

```text
ordnance B2.PT: 4 source rows, 0 optional weapon rows, 78 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 22 normal, 83 Cheat
ordnance audit: 48 aircraft, 20 recovered weapon stations, 4776 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
```

The `menus-ordnance-availability` expected line reads 4,776. Earlier sections quoting 4,784 stay as
written for their own states. Load Ordnance on the B-2: loading a row empties the other row of its bay.

## B-52 bay rows, underwing pylons and default load (B52-5, 2026-11-15)

Opinionated (John, 2026-11-15, decisions 7 to 9), fitted limits
([B-52 bay, pylons and tail gun](../spec/stores-on-pylons.md#b-52-bay-pylons-and-tail-gun-b52-5-2026-11-15)).
`B52.PT` rows 3, 5 and 6 (the internal bay) are capped to nine stores and 7,000 lb a row and start
empty; row 7 is appended, the underwing pylon pair (capacity 18, 17,000 lb class, flags `0x1105`),
holding the default 12 `AGM84E.JT`. The bay rows refuse 20 stores that the retail masks admit
and that do not fit the measured open bay (`MK82`, `MK82P3`, `FAB250`, `GBU38` and `RBK250`
stay); the pylon row refuses `AGM45`, `AGM88`, `GBU29`, `GBU29P` and `GBU30`. The row-7 class is 170,
not B52-0's 200: a default weapon loads at its weight limit and the saved-load check needs it to
equal the default count, and 20,000 lb admitted 14 Harpoons. Pins moved:

| Pin | From | To | Why |
| --- | ---: | ---: | --- |
| Recovered weapon stations | 20 | 21 | row 3 is a third empty bay row with an empty Mk 82 selection |
| Weapon placements | 4,784 | 4,746 | `B52.PT` 71 to 33 (five stores on each of three bay rows, 18 on the pylon row, less refusals; normal catalog 18) |
| Cheat-only placements | 240 | 242 | the pylon row takes the two novelty bombs under Cheat loading |
| Aircraft, tank placements, pod placements, records | 48, 233, 80, 82 / 9 | unchanged | |

```text
ordnance B52.PT: 5 source rows, 3 optional weapon rows, 33 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 18 normal, 83 Cheat
ordnance audit: 48 aircraft, 21 recovered weapon stations, 4746 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 242 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The `menus-ordnance-availability` and `a2g-catalog` expected lines read 21 stations, 4,746 and 242.
At the merge with the B-2's bay work the coordinator recomputes the weapon total (B4). Earlier
sections stay as written for their own states.

## Batch A integration: the B-2 bays and the B-52 on the shared grid (2026-11-15)

The B-2's bays (B2-5, 4,784 to 4,776) and the B-52's (B52-5, 4,784 to 4,746) were built
independently, each against its own pin, and the B-52 had its own small grid. At the integration
the B-52's three bay rows moved onto the B-2's shared magazine grid (floor 6.0 by 8.67 ft, the
roof 6 ft over the belly plane, the pitch the span plus 0.03 ft at most 1.3 ft) and the bay sweep
was re-run. Three stores clear by at least 0.05 ft (`MK82`, `MK82P3`, `FAB250`); `GBU38` and
`RBK250` now touch their neighbours' fins, so the refused list is 22, not 20
([B-52 bay, pylons and tail gun](../spec/stores-on-pylons.md#b-52-bay-pylons-and-tail-gun-b52-5-2026-11-15)).
The totals are recomputed from `--validate-ordnance`, not added:

| Pin | From | To | Why |
| --- | ---: | ---: | --- |
| Weapon placements | 4,784 | 4,732 | `B2.PT` 86 to 78 (minus 8) and `B52.PT` 71 to 27 (minus 44: three stores on each of three bay rows, 18 less five refusals on the pylon row); the F-117 slices F117-6 to F117-9 move none (`F117.PT` stays 46) |
| Recovered weapon stations | 20 | 21 | row 3 of the B-52 is a third empty bay row (B52-5) |
| Cheat-only placements | 240 | 242 | the B-52's pylon row (B52-5); the bay refusals change no novelty count |
| Aircraft, tank placements, pod placements, records | 48, 233, 80, 82 / 9 | unchanged | |

```text
ordnance B52.PT: 5 source rows, 3 optional weapon rows, 27 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 18 normal, 83 Cheat
ordnance B2.PT: 4 source rows, 0 optional weapon rows, 78 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 22 normal, 83 Cheat
ordnance audit: 48 aircraft, 21 recovered weapon stations, 4732 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 242 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The `menus-ordnance-availability` expected line reads 4,732; `a2g-catalog` and `test_battery_a2g.py`
match any weapon count and carry 21 and 242. Earlier sections quoting 4,776, 4,746 or 4,784 stay as
written for their own states.

## Bomber batch B identities join the roster, with their station corrections (BMB-B1 and BMB-B2, 2026-11-15)

`B1.PT`, `TU160.PT` and `F111.PT` joined `AircraftId` at the end (checkpoint codes 50 to
52, after the F-117's 49), so the audit now checks 51 aircraft. The pins in
`ordnance_audit.rs` moved on purpose (each carries a dated note):

| Pin | From | To | Why |
| --- | ---: | ---: | --- |
| Aircraft | 48 | 51 | The three identities |
| Recovered (optional) weapon stations | 20 | 20 | None of the three has a default-less weapon row |
| Weapon placements | 4,784 | 5,145 | B-1B 63, Tu-160 70, F-111F 228 |
| Tank placements | 233 | 249 | The F-111F's four wing pair rows take 16 |
| Cheat-only novelty placements | 240 | 258 | 18 more on the batch B rows |
| Designator-pod placements | 80 | 80 | BMB-B1 alone measured 87; BMB-B2 clears the bit (below) |
| Weapon records, tank records | 82, 9 | 82, 9 | No record added |

BMB-B1 alone (retail rows, measured with the corrections off in a separate profile):
5,173 weapon placements (B-1B 75, Tu-160 70, F-111F 244) and 87 pod placements (the
B-1B's three bay rows, `0x1500`, and the F-111F's four wing rows, `0x07e5`, carry the
designator bit `0x400`). BMB-B2 clears that bit on those seven rows (fitted: the B-1B's
bays are bomb bays, B7, and the F-111F's Pave Tack rides in the bay, not on a pylon), which
also removes the multi-store pods the bit admitted (B-1B 12, F-111F 16 placements) and
brings the pods back to 80. The F-111F's wing weight classes (70, 70, 70, 60) drop to 50
(5,000 lb, the real pylon rating): no placement is lost, because the heaviest store the
rows accept is the GBU-28 at 4,500 lb. The Tu-160's bay rows (`0x1184`) have no designator
bit and are untouched.

```text
ordnance B1.PT: 3 source rows, 0 optional weapon rows, 63 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 21 normal, 83 Cheat
ordnance TU160.PT: 2 source rows, 0 optional weapon rows, 70 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 35 normal, 83 Cheat
ordnance F111.PT: 4 source rows, 0 optional weapon rows, 228 compatible weapon placements, 16 tank placements; defaults and restart passed; catalog 57 normal, 83 Cheat
ordnance audit: 51 aircraft, 20 recovered weapon stations, 5145 weapon placements, 249 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 258 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The audit also runs the loadout validator for every aircraft: the B-1B's default load
(twelve bay stores) and the F-111F's start with a lowered tank (243,000 and 28,838 lb) so
that they pass their maximum take-off check ([aircraft records](../spec/aircraft-records.md#bomber-batch-b-record-corrections-b-1b-tu-160-f-111f-bmb-b2-2026-11-15)).
The `menus-ordnance-availability` battery expected line reads 51 aircraft, 5,145 weapon and
249 tank placements; the `a2g-catalog` expected lines and `tools/test_battery_a2g.py` read
51 aircraft and 249 tank placements (and match any weapon count). Earlier sections that
quote 48 aircraft, 4,784 weapon placements and 233 tank placements stay as written for
their own states.

## Batch A and batch B integration: the pins recomputed (2026-11-15)

Batch B (BMB-B1 and BMB-B2) was built from the integration of batch A's first round (4,784 weapon
placements, 240 Cheat-only, 20 recovered stations), before batch A's second round moved the B-2
and B-52 bays onto the shared grid. The two sets of pins were not added: every total below is
measured by `--validate-ordnance` on the merged tree (profile refreshed from the retail media
first). Batch B moves no row of the B-2, B-52 or F-117, so no placement moves twice.

| Pin | Batch A (round 2) | Batch B alone | Merged | Why |
| --- | ---: | ---: | ---: | --- |
| Aircraft | 48 | 51 | 51 | the B-1B, Tu-160 and F-111F join |
| Weapon placements | 4,732 | 5,145 (from 4,784) | 5,093 | 4,732 plus 361: B-1B 63, Tu-160 70, F-111F 228 |
| Tank placements | 233 | 249 | 249 | the F-111F's four wing pair rows take 16 |
| Recovered weapon stations | 21 | 20 | 21 | the B-52's third empty bay row (batch A); batch B adds none |
| Cheat-only placements | 242 | 258 (from 240) | 260 | 242 plus the 18 pairs on the batch B rows |
| Designator pod placements | 80 | 80 | 80 | the B-1B bay rows and F-111F wing rows lose the designator bit (BMB-B2) |
| Weapon and tank records | 82 / 9 | 82 / 9 | 82 / 9 | |

```text
ordnance B1.PT: 3 source rows, 0 optional weapon rows, 63 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 21 normal, 83 Cheat
ordnance TU160.PT: 2 source rows, 0 optional weapon rows, 70 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 35 normal, 83 Cheat
ordnance F111.PT: 4 source rows, 0 optional weapon rows, 228 compatible weapon placements, 16 tank placements; defaults and restart passed; catalog 57 normal, 83 Cheat
ordnance audit: 51 aircraft, 21 recovered weapon stations, 5093 weapon placements, 249 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 80 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 260 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The `menus-ordnance-availability` expected line reads 5,093 weapon placements, 249 tank placements
and 21 recovered stations; `a2g-catalog` matches any weapon count and carries 51, 21, 249 and 260;
`test_battery_a2g.py` carries 51, 21 (any count) and 249. Earlier sections quoting 4,776, 4,746,
4,784, 4,732 or 5,145 stay as written for their own states.

## Bomber batch C identities join the roster, with their station corrections (BMB-C1 and BMB-C2, 2026-11-15)

`TU26.PT`, `TU95.PT` and `SU34.PT` joined `AircraftId` at the end (checkpoint codes 53 to 55,
after the F-111F's 52), so the audit now checks 54 aircraft. The pins in `ordnance_audit.rs`
moved on purpose (each carries a dated note):

| Pin | From | To | Why |
| --- | ---: | ---: | --- |
| Aircraft | 51 | 54 | The three identities |
| Recovered (optional) weapon stations | 20 | 22 | The Tu-26's empty bay row (5) and the Tu-95's empty pylon row (8): each keeps an empty AS-16 selection (the default store of another row), which the Tu-26's `0x20` bit and the Tu-95's `0x1025` mask now admit |
| Weapon placements | 5,145 | 5,447 | Tu-26 129, Tu-95 36, Su-34 137 |
| Tank placements | 249 | 257 | The Su-34's two `0x07e5` rows take 8 (the Tu-26 and Tu-95 have none; the Tu-95's pylon row loses the tank bit) |
| Cheat-only novelty placements | 258 | 276 | 18 more on the batch C rows |
| Designator-pod placements | 80 | 82 | The Su-34's rows 6 and 7 keep the retail designator bit `0x400`, one pod each; the Tu-26's bay row and the Tu-95's pylon row lose it (BMB-C2) |
| Weapon records, tank records | 82, 9 | 82, 9 | No record added |

Unlike batch B, the retail rows were not measured separately: the Tu-26's bay row has no
compatible default selection without the active-radar bit, so the audit stops at
"TU26.PT source station 6 hides compatible AGM45.JT" on the retail rows. The Tu-26's bay
(`0x1500` to `0x1120`) takes the AS-16 (and the Harpoon and AM-39 the same bit admits, as on
the B-52); its glove pairs hold two each (4 and 6 before). The Tu-95's pylon row goes from
the F-16C wing mask `0x17e5` to the AS-16 alone (`weapon_allowed`), one placement, ten
stores, no weight class; its Bear-A to F turret rows (5 and 6) hold nothing.

```text
ordnance TU26.PT: 5 source rows, 1 optional weapon rows, 129 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 37 normal, 83 Cheat
ordnance TU95.PT: 3 source rows, 1 optional weapon rows, 36 compatible weapon placements, 0 tank placements; defaults and restart passed; catalog 35 normal, 83 Cheat
ordnance SU34.PT: 4 source rows, 0 optional weapon rows, 137 compatible weapon placements, 8 tank placements; defaults and restart passed; catalog 62 normal, 83 Cheat
ordnance audit: 54 aircraft, 22 recovered weapon stations, 5447 weapon placements, 257 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 82 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 276 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The `menus-ordnance-availability` battery expected line reads 54 aircraft, 22 recovered
stations, 5,447 weapon and 257 tank placements; the `a2g-catalog` expected lines (the
Cheat-only count, stale at 240 since BMB-A1, now 276) and `tools/test_battery_a2g.py` read
54 aircraft, 22 recovered stations and 257 tank placements (and match any weapon count).
Earlier sections that quote 51 aircraft, 5,145 weapon placements and 249 tank placements
stay as written for their own states.

## Batches A, B and C integration: the pins recomputed (2026-11-16)

Batch C (BMB-C1 and BMB-C2) was built from batch B's base (5,145 weapon placements, 249 tank
placements, 20 recovered stations), before round 2 moved the B-2 and B-52 bays onto the
shared grid. The pins were not added: every total below is measured by `--validate-ordnance`
on the merged tree (profile refreshed from the retail media first). Batch C moves no row of
batches A or B, so no placement moves twice. B52-6/7 moves none.

| Pin | Batches A and B | Batch C alone | Merged | Why |
| --- | ---: | ---: | ---: | --- |
| Aircraft | 51 | 54 (from 51) | 54 | the Tu-26, Tu-95 and Su-34 join |
| Weapon placements | 5,093 | 5,447 (from 5,145) | 5,395 | 5,093 plus 302: Tu-26 129, Tu-95 36, Su-34 137 |
| Tank placements | 249 | 257 | 257 | the Su-34's two `0x07e5` rows take 8 |
| Recovered weapon stations | 21 | 22 (from 20) | 23 | 21 plus the Tu-26's bay row and the Tu-95's pylon row |
| Cheat-only placements | 260 | 276 (from 258) | 278 | 260 plus the 18 pairs on the batch C rows |
| Designator pod placements | 80 | 82 | 82 | the Su-34's rows 6 and 7 keep the retail designator bit |
| Weapon and tank records | 82 / 9 | 82 / 9 | 82 / 9 | |

```text
ordnance audit: 54 aircraft, 23 recovered weapon stations, 5395 weapon placements, 257 tank placements, 82 weapon and 9 tank records; passed
ordnance audit: 1 designator pods, 82 pod placements, one to an aircraft, loaded and restored through the saved mission; passed
ordnance audit: 2 novelty records, 278 Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed
```

The `menus-ordnance-availability` expected line reads 54 aircraft, 23 recovered stations,
5,395 weapon and 257 tank placements; `a2g-catalog` carries 54, 23, 257 and 278;
`test_battery_a2g.py` carries 54, 23 and 257. Earlier sections quoting 22 recovered stations,
5,447 weapon placements or 276 Cheat-only placements stay as written for their own states.

## Tu-160 weapon bays: five refusals (T160-5, 2026-11-16)

The Tu-160's two bay rows (`TU160.PT` rows 4 and 5, flags `0x1180` once the pair flag is cleared)
refuse five stores that do not fit the fitted 7.6 ft wide, 6 ft deep, 42 ft long bays with the doors
open (`aircraft_stations::TU160_BAY_REFUSED`: `AT12.JT`, `GBU29.JT`, `GBU29P.JT`, `GBU30.JT`,
`PAVEWA3.JT`; measured with the bay clearance sweep, see
[Tu-160 stations, bays and pylons](../formats/aircraft-variety.md#tu-160-stations-bays-and-pylons-t160-5-2026-11-16)),
so `TU160.PT` goes from 70 to 60 placements (35 admitted records a row, 30 after the refusals).
Measured by `--validate-ordnance` on `m3-tu160` with the profile refreshed:

| Pin | Before | After | Why |
| --- | ---: | ---: | --- |
| Aircraft | 54 | 54 | |
| Weapon placements | 5,395 | 5,385 | `TU160.PT` 70 to 60 (two rows, five stores each) |
| Tank placements | 257 | 257 | |
| Recovered weapon stations | 23 | 23 | the Tu-160's rows were already recovered |
| Cheat-only placements | 278 | 278 | |
| Designator pod placements | 82 | 82 | the bay rows never carried the designator bit |
| Weapon and tank records | 82 / 9 | 82 / 9 | |

The bays' grid caps no store below its record count (the Kh-15 holds 12 by room, six by record). The
pin is this slice's alone: batch B's other aircraft and the merged batches are not touched, so the
coordinator's integration total moves by minus 10 only. The `menus-ordnance-availability` expected line
reads 54 aircraft, 23 recovered stations, 5,385 weapon and 257 tank placements. Earlier sections
quoting 5,395 weapon placements stay as written for their own state.
