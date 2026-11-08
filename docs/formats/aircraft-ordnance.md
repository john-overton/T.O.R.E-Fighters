# Aircraft ordnance source rows

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research and implementation evidence, 2026-10-05. This pass audits the 36
registered retail PT identities at workspace base
`c1962c5d1250e43a60bd2566af1d8cdb0abf3ac7`. F/A-XX reuses the F22N source rows
and adds no retail identity. The [station behavior spec](../spec/ordnance-presentation.md#source-stations-and-availability)
owns the player contract. No original module was executed or retail gameplay
comparison performed. No aircraft rendering changes belong to this pass.

## Build identity and method

The source PT/JT records were parsed by the existing bounded Rust readers from
local FA_2.LIB extraction. Each source slot was tested against every installed JT
and against the current supported-name list with the shared loading component.
The local machine-readable evidence is `.local/ordnance-audit/slots.tsv`; retained
source files and derived scratch reports remain ignored. Source-build hashes:

| File | SHA-256 |
| --- | --- |
| `FA_2.LIB` | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |
| `FA.EXE` | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |
| `FA.SMS` | `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0` |

Static instruction references below use the hash-reviewed FA.EXE/SMS pair,
through the existing local disassembly. Only the stated short spans were
reviewed. The [existing ordnance notes](ordnance-menu.md) own previously recovered
geometry, category, quantity-step and sound facts.

## Source identity is a row, not a pylon count

A HARDPOINT contains 12 BRF tokens: flags; x/y/z; slew heading/pitch; slew limits;
default resource; byte maxWeight; word maxItems; byte name. The final byte maps to
Centerline, Fuselage, Internal Gun, Internal Bay, Wing and Wingtip. It is not the
maxWeight byte. A source ordinal stays stable even when the default resource is
null, a tank or equipment. Names are location labels, not proof of physical
mount count. maxItems is a quantity/capacity field, not the number of pylons.

At 0x41a12a..0x41a14e, original ArmPlane skips a row when fixed bit 8 is set and
the loaded resource is not projectile type 7. Other rows proceed regardless of
JT/GAS/SEE/ECM/null default class. At 0x41a31f the original source ordinal is
stored in the display list; 0x41a7aa uses that ordinal for the card. This
establishes one aggregate card per selected source row. It does not establish
that a multi-item row should be expanded into left/right physical pylons.

## Per-aircraft audit matrix

All indices below are zero-based original PT ordinals. W is the old default-JT
weapon map. T is the pre-pass tank-candidate map. Extra W names omitted rows
that accept at least one currently supported JT under normal source compatibility.
Roles count default JT/GAS/SEE/ECM/null entries. UI is the unified editable-row
count after adding Extra W and retaining occupied auxiliary hardware as locked.
Gaps counts distinct imported, source-compatible JT names not in the live
supported-name list. It does not count these weapons as implemented.

| PT | Total | Roles JT/GAS/SEE/ECM/null | W | T | Extra W | UI | Gaps |
| --- | ---: | --- | --- | --- | --- | ---: | ---: |
| `F18.PT` | 9 | 5/1/2/1/0 | 3, 5, 6, 7, 8 | 4, 5, 6, 7 | None | 6 | 36 |
| `RAFALE.PT` | 9 | 5/0/3/1/0 | 3, 5, 6, 7, 8 | 4 | None | 5 | 37 |
| `F14.PT` | 8 | 4/1/2/1/0 | 3, 4, 6, 7 | 5, 6 | None | 5 | 36 |
| `A4E.PT` | 7 | 3/1/2/1/0 | 3, 4, 6 | 4, 5 | 5 | 4 | 27 |
| `F31.PT` | 9 | 4/0/3/1/1 | 4, 6, 7, 8 | 5 | 5 | 5 | 32 |
| `MIG29.PT` | 8 | 4/0/3/1/0 | 4, 5, 6, 7 | 5, 6, 7 | None | 4 | 32 |
| `SU27.PT` | 9 | 5/0/3/1/0 | 4, 5, 6, 7, 8 | 5, 6, 7, 8 | None | 5 | 32 |
| `MIG21.PT` | 7 | 3/0/2/1/1 | 3, 5, 6 | 4, 5, 6 | 4 | 4 | 16 |
| `SU25.PT` | 9 | 4/0/3/1/1 | 4, 6, 7, 8 | 5, 6, 7, 8 | 5 | 5 | 36 |
| `MIG23.PT` | 9 | 4/0/2/1/2 | 3, 6, 7, 8 | 4, 6, 7, 8 | 5 | 6 | 35 |
| `SU35.PT` | 9 | 5/0/3/1/0 | 4, 5, 6, 7, 8 | 5, 6, 7 | None | 5 | 36 |
| `F22.PT` | 8 | 4/0/3/1/0 | 3, 5, 6, 7 | None | None | 4 | 36 |
| `F22N.PT` | 8 | 4/0/3/1/0 | 3, 5, 6, 7 | None | None | 4 | 36 |
| `C130.PT` | 1 | 0/0/1/0/0 | None | None | None | 0 | 0 |
| `AC130.PT` | 7 | 3/0/3/1/0 | 4, 5, 6 | None | None | 3 | 0 |
| `E3.PT` | 3 | 0/0/2/1/0 | None | None | None | 0 | 0 |
| `IL76.PT` | 3 | 0/0/2/1/0 | None | None | None | 0 | 0 |
| `E2.PT` | 3 | 0/0/2/1/0 | None | None | None | 0 | 0 |
| `AV8.PT` | 9 | 4/0/3/2/0 | 4, 5, 6, 7 | 5, 8 | None | 4 | 36 |
| `YAK141.PT` | 6 | 3/0/2/1/0 | 3, 4, 5 | 4, 5 | None | 3 | 33 |
| `V22.PT` | 4 | 1/0/2/1/0 | 3 | None | None | 1 | 0 |
| `AH64.PT` | 8 | 3/0/3/1/1 | 3, 5, 6 | 4 | None | 4 | 24 |
| `MI24.PT` | 3 | 2/0/1/0/0 | 1, 2 | None | None | 2 | 0 |
| `CH47.PT` | 1 | 0/0/1/0/0 | None | None | None | 0 | 0 |
| `MIG17F.PT` | 4 | 2/1/1/0/0 | 1, 2 | 3 | 3 | 3 | 18 |
| `F4B.PT` | 7 | 3/1/2/1/0 | 3, 4, 5 | 3, 4, 5, 6 | None | 4 | 36 |
| `F4J.PT` | 8 | 3/1/2/1/1 | 3, 5, 6 | 4, 5, 6, 7 | 4 | 5 | 36 |
| `F4E.PT` | 8 | 3/1/2/1/1 | 3, 5, 6 | 4, 6, 7 | 7 | 5 | 36 |
| `F4.PT` | 8 | 3/1/2/1/1 | 3, 5, 6 | 4, 6, 7 | 7 | 5 | 36 |
| `A7.PT` | 7 | 4/0/2/1/0 | 3, 4, 5, 6 | 5, 6 | None | 4 | 29 |
| `F15.PT` | 6 | 3/0/2/1/0 | 3, 4, 5 | 4, 5 | None | 3 | 37 |
| `F16C.PT` | 9 | 4/1/3/1/0 | 4, 6, 7, 8 | 5, 6 | None | 5 | 34 |
| `F104.PT` | 9 | 4/2/2/1/0 | 3, 6, 7, 8 | 4, 5, 6, 7, 8 | None | 6 | 32 |
| `A10.PT` | 9 | 5/0/3/1/0 | 3, 5, 6, 7, 8 | None | None | 5 | 28 |
| `B747.PT` | 1 | 0/0/1/0/0 | None | None | None | 0 | 0 |
| `A310.PT` | 1 | 0/0/1/0/0 | None | None | None | 0 | 0 |

The maximum aggregate editable-row count is six for these 36 records. Transport
and airborne-radar aircraft with no carrier rows keep zero; do not invent mounts
for them. Nine omitted source rows accept supported weapons. No omitted
SEE/ECM default row accepted a supported JT in the normal-compatibility probe.

## Omitted weapon-capable rows

These examples use actual source compatibility, not real-aircraft expectations.
They establish source placement eligibility; weapon behavior and
current sensor/target prerequisites remain separate.

| PT / source slot | Default | Location | Flags | maxItems / maxWeight | Example supported compatible JT |
| --- | --- | --- | --- | --- | --- |
| `A4E.PT` / 5 | `F150.GAS` | Wing | `0x1785` | 4 / 32 | MK82.JT, AGM65G.JT |
| `F31.PT` / 5 | `NONE` | Centerline | `0x0681` | 1 / 15 | AIM9M.JT |
| `MIG21.PT` / 4 | `NONE` | Centerline | `0x07e1` | 1 / 10 | AIM120.JT, AIM7.JT |
| `SU25.PT` / 5 | `NONE` | Centerline | `0x07e5` | 1 / 20 | AS7.JT, MK82.JT |
| `MIG23.PT` / 5 | `NONE` | Fuselage | `0x0085` | 4 / 6 | AIM9M.JT |
| `MIG17F.PT` / 3 | `F150.GAS` | Wing | `0x0705` | 2 / 40 | MK82.JT |
| `F4J.PT` / 4 | `NONE` | Centerline | `0x0785` | 1 / 40 | AIM9B.JT, MK82.JT |
| `F4E.PT` / 7 | `NONE` | Wing | `0x07e5` | 2 / 20 | AIM120.JT, AIM7.JT |
| `F4.PT` / 7 | `NONE` | Wing | `0x0765` | 2 / 20 | AIM7.JT, AGM88.JT |

F4.PT slot 7 has 0x0765 and lacks the IR/laser compatibility mask 0x80; it must
not inherit the F4E slot's IR choices. SUU16 appears source-compatible on
some rows, but the current host's fitted gun-pod behavior is restricted to the
reviewed source-installed F4J slot. A compatibility match alone does not extend
that gameplay support.

## F-14 exact current-to-source mapping

F14.PT has eight source records, not eight weapon cards or eight proved pylons.
Three fixed equipment rows are not ordnance carriers. The remaining five rows
are the complete original aggregate editing group. The tank row omitted from
the weapon-only map is source slot 5, location Fuselage, maxItems 2. That is one
source row containing a tank quantity, not evidence for two separately editable
source rows.

| Source slot | Default | Location | Flags | maxItems | maxWeight | Old weapon index | Old tank index |
| ---: | --- | --- | --- | ---: | ---: | --- | --- |
| 0 | `VIS340.SEE` | Fuselage | `0x0008` | 1 | 0 | - | - |
| 1 | `F14R.SEE` | Fuselage | `0x0008` | 1 | 0 | - | - |
| 2 | `F14.ECM` | Fuselage | `0x0008` | 1 | 0 | - | - |
| 3 | `M61.JT` | Internal Gun | `0x0008` | 675 | 0 | 0 | - |
| 4 | `AIM54C.JT` | Fuselage | `0x0155` | 4 | 40 | 1 | - |
| 5 | `F250.GAS` | Fuselage | `0x0605` | 2 | 38 | - | 0 |
| 6 | `AIM120.JT` | Wing | `0x07f5` | 2 | 30 | 2 | 1 |
| 7 | `AIM9M.JT` | Wing | `0x0485` | 2 | 5 | 3 | - |

Before this pass, source slot 6 appeared separately in the weapon and tank
views. It remains one source row and may hold only one selected store class. Its
position is 18 / -20 / 14 source units. Slot 5 is 9 / -23 / 20. Tank capacity,
fuel and shell values live in the [F-14 source review](aircraft.md#f-14-default-external-fuel).
Normal source compatibility permits no supported JT on dedicated tank slot 5.
It does permit supported JTs and compatible tanks on slot 6.

## Occupied auxiliary rows and sensor prerequisites

RAFALE.PT slot 4 defaults to AAS38.SEE with flags 0x0601. AV8.PT slot 8 defaults
to ALQ167.ECM with flags 0x0601. Both are nonfixed original screen rows and
source tank-compatible, but the current host cannot remove/rebuild the
selected auxiliary equipment suite when substituting a tank. Exposing them as
empty tank rows can overlap retained hardware. This pass locks those occupied
rows rather than claiming equipment replacement support. Fixed sensors and
countermeasures remain separate from editable ordnance.

The current catalog tests imported presence, supported-name status and station
compatibility. It does not require a current target or sensor lock. In the host,
supported-radar missiles with radar power on require actual aircraft support;
missing radar yields NoRadar. The requested radar-off dumb-release rule is
separate. Own IR seekers do not require an aircraft FLIR installation. Emitter
weapons require a compatible emitting surface target. None of the radarless
registered aircraft admits a supported-radar missile under normal source
compatibility in this audit; Cheat can bypass the normal station mask and may
show such a store without creating radar hardware. A selectable store must not
be described as guaranteed guided operation.

## Imported but not live-supported compatibility gaps

At the 2026-10-11 audit, these 37 source-compatible imported JT names were
outside the live list: AA6, AA9, AEMP1, AGM45, AGM65A, AGM84A, AGM84E, AM39,
AS14, AS15, AS16, AS30, AT12, CBU87, CBU89, FAB1000, FAB250, FAB500, GBU10,
GBU10A, GBU28, GBU29, GBU29P, GBU30, LAU10, MK20, MK82AIR, MK82HD, MK82P,
MK82P3, MK84, PAVEWA3, PAVEWAY, PL10, PL7, RBK250 and RBK500. Later passes
changed that: the W1 to W4 air-to-ground work enabled every name except
AEMP1, and JT-4 enabled AA6, AA9, PL10 and PL7 on 2026-11-06. The per-aircraft table
and gap counts above are that earlier snapshot. The per-record sections below
and the [missile inventory](../spec/missiles.md#first-pass-inventory-matrix)
own current status. A name, physical fit or imported JT never establishes
implemented behavior. The gap counts include only names with a positive normal
capacity, not every catalog name.

## Cannon records nothing mounts (JT-1, 2026-11-06)

Research and implementation note for the six decoded cannon records the ordnance
catalog still hides. `ADEN.JT`, `BK27.JT`, `GAU13.JT` and `GSH6_23.JT` are named
cannon pods; `20MM_4.JT` (Colt 20mmx4) and `ADENIN.JT` (Aden 30mm Cannon) are
guns. All six decode from `FA_2.LIB` and their full records are in
[`fa-weapons.csv`](fa-weapons.csv).

| Record | Weight lb | projsInPod | Decoded default carriers ([`fa-aircraft.csv`](fa-aircraft.csv)) |
| --- | ---: | ---: | --- |
| `ADEN.JT` | 500 | 200 | `DRAK.PT`, `SEAHAR.PT` |
| `ADENIN.JT` | 1 | 1 | `HWK.PT`, `JAG.PT` |
| `BK27.JT` | 1 | 1 | `ALPH.PT`, `E2000.PT`, `GRIPEN.PT`, `TOR.PT`, `VIG.PT` |
| `GAU13.JT` | 1 | 1 | `ASTOVL.PT`, `ASTOVLE.PT`, `ASTOVLF.PT`, `ASTOVLV.PT` |
| `GSH6_23.JT` | 1 | 1 | `SU24.PT` |
| `20MM_4.JT` | 1 | 1 | `F8J.PT` |

Every decoded carrier is outside `AircraftId::ALL`
(`crates/tore-formats/src/aircraft.rs:308`), the 36 imported aircraft, so no
ported or variety aircraft gains any of the six. The carriers are the same rows
the source catalog already records as their `internal_gun` or default store
(`tools/catalog_fa.py`).

The records cannot load as pylon stores either. Each carries projectile flags
`0x0348c4`, with the compatibility bit 2 (value 2) clear. `Station::allowed_count`
(`crates/tore-sim/src/combat/loading.rs:118`) refuses a projectile with flag 2
clear on any station that does not name it as its own default, and the SUU-16
gun pod (`SUU16.JT`, flags `0x0348c6`) sets flag 2 as the reviewed external
case. A built-in-gun-style record therefore loads only on the aircraft built
with it, and no imported aircraft is built with any of the six. `is_gun`
(`crates/tore-sim/src/combat/live.rs:6893`) likewise derives its set from each
aircraft's `guns()` and the surface gun table, so a Cheat-loaded record would
not fire as a gun.

Implementation (slice JT-1, 2026-11-06): the six stay out of
`SUPPORTED_WEAPONS` (`crates/tore-sim/src/combat/loadout.rs:771`) and
`NOVELTY_WEAPONS`, so nothing is enabled, because the decoded station data
permits no carrier. Do not alias a record to a same-family gun on an imported
aircraft: `GSH6_23.JT` is not `GSH6_30.JT` (Su-24 against MiG-23), and ADEN,
ADENIN, BK27, GAU13 and 20MM_4 are their own records. Next step: when a carrier
is ported, or an M3 mission loads one with its own guns, add the record to that
aircraft's `guns()` or `gun_pods()` and to `SUPPORTED_WEAPONS`.

Guards: `a_built_in_gun_loads_only_as_its_stations_own_default`
(`crates/tore-sim/src/combat/loading.rs`) checks the flag rule,
`the_decoded_cannon_records_stay_hidden_and_no_registered_aircraft_mounts_them`
(`crates/tore-sim/src/combat/loadout.rs`) checks the supported set and the
mounted-gun table, and `crates/tore-import/tests/hidden_guns.rs` (retail, skips
without media) checks the decoded flags and every imported aircraft's station
list.

## AA-6, AA-9, PL-7 and PL-10 (JT-4, 2026-11-06)

Research and implementation note for the four decoded air-to-air records JT-2
found station-compatible but unsupported: `AA6.JT` (AA-6A Acrid,
`fa-weapons.csv:9`), `AA9.JT` (AA-9 Amos, `fa-weapons.csv:11`), `PL10.JT`
(PL10-R, `fa-weapons.csv:97`) and `PL7.JT` (PL-7, `fa-weapons.csv:98`). All
four decode from `FA_2.LIB` as guided missiles with the normal-store
compatibility bit 2 set, so unlike the JT-1 cannon records a compatible station
mask can admit them. Their reviewed seeker classification
(`crates/tore-formats/src/weapons.rs`, `every_inventoried_air_to_air_missile_has_the_right_seeker_and_fox_call`,
2026-10-15) is semi-active radar for AA6, AA9 and PL10 and infrared for PL7.

Decoded default carriers, from [`fa-aircraft.csv`](fa-aircraft.csv) store
lists, are all aircraft outside `AircraftId::ALL`
(`crates/tore-formats/src/aircraft.rs:308`): `AA6.JT` on `M25.PT`
(`fa-aircraft.csv:97`, MiG-25), `AA9.JT` on `MIG31.PT` (`fa-aircraft.csv:110`,
MiG-31) and `PL7.JT` on `Q5.PT` (`fa-aircraft.csv:115`, Q-5). No retail record
lists `PL10.JT` as a default store. So no imported aircraft is **built with**
one. All four are reached through the same decoded hardpoint compatibility as
the live stores. The rows below are measured with `--validate-ordnance` on the
installed media; the [even-paired loads
baseline](../baselines/even-paired-loads.md) records the pairing changes at
that earlier pass.

Every row below is checked by `--validate-ordnance`, which loads and restores
the store on it. Numbers are the 0-based source slot used by the per-aircraft
matrix and the even-paired baseline, not the Load Ordnance page's station
order.

| Aircraft | AA-6 | AA-9 | PL-7 | PL-10 |
| --- | --- | --- | --- | --- |
| A10 | - | - | 5, 6, 7, 8 | - |
| A7 | - | - | 4, 6 | - |
| AH64 | - | - | 5, 6 | - |
| AV8 | 5 | 5 | 5, 6, 7 | 5 |
| F104 | 6, 7 | 6, 7 | 6, 7, 8 | 6, 7 |
| F14 | 4, 6 | 4, 6 | 6, 7 | 4, 6 |
| F15 | 4, 5 | 4, 5 | 4, 5 | 4, 5 |
| F16C | 6, 9 | 6, 9 | 6, 7, 8, 9 | 6, 9 |
| F18 | 5, 6, 7 | 5, 6, 7 | 5, 6, 7, 8 | 5, 6, 7 |
| F22 | 5, 6 | 5, 6 | 5, 6, 7 | 5, 6, 7 |
| F22N | 5, 6 | 5, 6 | 5, 6, 7 | 5, 6, 7 |
| F31 | 6, 7 | 6, 7 | 5, 6, 7, 8 | 6, 7 |
| F4 | 5, 6, 7 | 5, 6, 7 | 5 | 5, 6, 7 |
| F4B | 4 | 4 | 3, 4, 5 | 4 |
| F4E | 5, 6, 7 | 5, 6, 7 | 5, 7 | 5, 6, 7 |
| F4J | 5 | 5 | 4, 5, 6 | 5 |
| MIG21 | - | - | 4, 5, 6 | 4, 5, 6 |
| MIG23 | 6, 7, 8 | 6, 7, 8 | 5, 6, 7, 8 | 6, 7, 8 |
| MIG29 | 5, 6, 7 | 5, 6, 7 | 5, 6, 7 | 5, 6, 7 |
| RAFALE | 5, 6, 7 | 5, 6, 7 | 5, 6, 7, 8 | 5, 6, 7 |
| SU25 | 5, 6, 7, 8 | 5, 6, 7, 8 | 5, 6, 7, 8 | 5, 6, 7, 8 |
| SU27 | 7 | 7 | 5, 6, 7, 8 | 5, 6, 7, 8 |
| SU35 | 5, 6, 7 | 5, 6, 7 | 5, 6, 7 | 5, 6, 7 |
| YAK141 | 4, 5 | 4, 5 | 4, 5 | 4, 5 |

No decoded default row lists any of the four on a ported aircraft, so these
are all compatible-station loads.

Implementation (slice JT-4, 2026-11-06, requested by John): each record was
added to the reviewed profile list
(`Profile::reviewed_activation`, `crates/tore-sim/src/combat/missiles.rs`) and
to `SUPPORTED_WEAPONS` (`crates/tore-sim/src/combat/loadout.rs`). They follow
the same classes as the live stores: AA6, AA9 and PL10 use the semi-active
radar behaviour of `AA10.JT`/`R530.JT`, and PL7 uses the infrared behaviour of
`R550.JT`/`AIM9M.JT`. Each keeps its own record numbers for launch envelope,
seeker range and angles, ignition, burn, removal, mass, damage, splash and
launch sound. None is active radar, so none takes an activation distance,
PITBULL state or maddog release. The player-facing contract and the numbers are
in [the missile spec](../spec/missiles.md#air-to-air-missiles-enabled-jt-4-2026-11-06).
Do not alias a record to a same-class live store: `AA6.JT` is not `R40`,
`AA9.JT` is not `AA10` or `R530`, and `PL10.JT` is not `AA12` or `MICA`.

Guards: `the_reviewed_air_to_air_missiles_are_supported`
(`crates/tore-sim/src/combat/loadout.rs`) checks the supported set,
`the_reviewed_air_to_air_missiles_have_profiles_from_their_seeker_class`
(`crates/tore-sim/src/combat/missile_tests.rs`) pins each guidance class, the
absence of an active transition and the record-derived guidance lifetime, and
`crates/tore-import/tests/reviewed_air_to_air_missiles.rs` (retail, skips
without media) checks every spec number from the decoded records and that no
imported aircraft is built with one.

## Surface records nothing mounts (JT-3, 2026-11-06)

Research and implementation note for the ten decoded records the surface weapon
lists leave out: six surface missiles (`HQ2J.JT`, `HQ61.JT`, `MIM23.JT`,
`ROLAND.JT`, `SAN5.JT`, `SAN8.JT`) and four gun-round records (`AAA20.JT`,
`M163.JT`, `T20_1.JT`, `T23_1.JT`). All ten decode from `FA_2.LIB` and their
full records are in [`fa-weapons.csv`](fa-weapons.csv).

A decoded scan of every NT hardpoint in `FA_1.LIB` and `FA_2.LIB` finds no
`defaultTypeName` naming any of the ten (`SurfaceUnit::parse`,
`crates/tore-formats/src/surface_unit.rs:411`). The scan is
`no_nt_unit_mounts_a_decoded_only_surface_record`
(`crates/tore-import/tests/unloaded_surface_weapons.rs`, retail, skips without
media); it reads all 84 NT records and every PT record. Only `T23_1.JT` appears
in a PT: the turret record of the Tu-26 and Tu-95 (`fa-aircraft.csv:138`,
`fa-aircraft.csv:139`), both outside `AircraftId::ALL`
(`crates/tore-formats/src/aircraft.rs:308`). `T20_1.JT` is named by no record.

| Record | Decoded class | NT units carrying it | Why it stays out |
| --- | --- | --- | --- |
| `HQ2J.JT` (`fa-weapons.csv:71`) | radar SAM, seeker 3, support flag 0x200 | none | no carrier; Catalog, no reviewed flight profile |
| `HQ61.JT` (`fa-weapons.csv:72`) | radar SAM, seeker 3, support flag 0x200 | none | no carrier; Catalog, no reviewed flight profile |
| `MIM23.JT` (`fa-weapons.csv:84`) | radar SAM, seeker 3, support flag 0x200 | none | no carrier; HAWK uses SA19; Catalog |
| `ROLAND.JT` (`fa-weapons.csv:104`) | radar SAM, seeker 3, support flag 0x200 | none | no carrier; Roland uses SA19; Catalog |
| `SAN5.JT` (`fa-weapons.csv:118`) | infrared SAM, seeker 2 | none | no carrier; Catalog, no reviewed flight profile |
| `SAN8.JT` (`fa-weapons.csv:120`) | infrared SAM, seeker 2 | none | no carrier; Catalog, no reviewed flight profile |
| `AAA20.JT` (`fa-weapons.csv:12`) | 20 mm radar AAA round, seeker 3 | none | no carrier; no gun-table row |
| `M163.JT` (`fa-weapons.csv:79`) | 20 mm round, seeker 3, ZSU23 copy | none; `M163.NT` mounts `PHALANX.JT` (`surface-units.md:212`) | no carrier; the M163 row is PHALANX with replaced zones ([AAA tuning](../spec/surface-defenses.md#aaa-tuning)) |
| `T20_1.JT` (`fa-weapons.csv:127`) | 20 mm turret round, seeker 0 | none | aircraft turret record, no carrier; not a surface weapon |
| `T23_1.JT` (`fa-weapons.csv:128`) | 23 mm turret round, seeker 0 | none | Tu-26/Tu-95 turret record, both unported; not a surface weapon |

The six missiles stay out for two independent reasons. First, no surface unit
carries one, so a surface list entry would enable a record nothing can fire.
Second, none has a reviewed flight profile: `Profile::reviewed_activation`
(`crates/tore-sim/src/combat/missiles.rs:253`) excludes all six, so
`Profile::for_weapon` returns `None` and a shot would be refused as unreviewed.
[The missile spec](../spec/missiles.md#first-pass-inventory-matrix) marks all
six **Catalog** (`missiles.md:297`, `:298`, `:300`, `:307`, `:321`, `:323`).
Do not alias a record to a live same-class store: `HQ2J.JT` and `HQ61.JT` are
not `SA2A.JT` or `SA3.JT`, `MIM23.JT` and `ROLAND.JT` are not `SA19.JT`, and
`SAN5.JT` and `SAN8.JT` are not `SA7.JT` or `SA14.JT`.

`AAA20.JT` and `M163.JT` decode as gun rounds but have no row in the AAA
tuning table (`crates/tore-sim/src/combat/surface_guns.rs`, `TABLE`), so
`is_surface_gun` is false and no unit could fire either as a surface gun.
Adding a row would also need the reviewed rate, magazine and reload the table
carries. `M163.JT` is the unused ZSU23 copy the M163 record does not mount;
switching the M163 to it was already rejected because it keeps the 60,000 ft
sight ([AAA tuning](../spec/surface-defenses.md#aaa-tuning)).

`T20_1.JT` and `T23_1.JT` are aircraft turret gun rounds, not surface weapons.
`T23_1.JT` is the turret record named by the Tu-26 and Tu-95, neither imported;
`T20_1.JT` is referenced by no record. They wait for aircraft turret work, not
for the surface lists.

Implementation (slice JT-3, 2026-11-06): nothing is enabled. The six missiles
stay out of `SURFACE_SUPPORTED` and `SURFACE_INFRARED`
(`crates/tore-sim/src/combat/missiles.rs:110` and `:127`), and the four gun
records have no `TABLE` row. Next step: if an M3 mission or a later NT record
mounts one, review its flight profile, or add a reviewed gun-table row, before
adding the record to the surface list. Until then it stays catalog-only.

Guards: `the_decoded_only_surface_records_are_not_wired`
(`crates/tore-sim/src/combat/live/surface_tests.rs`) keeps all ten out of the
surface missile lists and the gun table, and checks the six have no reviewed
profile. `crates/tore-import/tests/unloaded_surface_weapons.rs` (retail, skips
without media) checks that no NT mounts any of the ten, that only the Tu-26 and
Tu-95 PT records name `T23_1.JT`, and that no imported aircraft is built with
one.

## ASROC and the Ticonderoga's SM-2 (JT-6, 2026-11-06)

ASROC is not one of the ten: it has a carrier. The Ticonderoga's NT is the only
record that mounts it, on two rails (`ASROC.JT` hardpoints 2 and 3) beside two
Phalanx, and no PT or other NT names it. Its guidance word is `0x12341`, the
same as the SA-6's: the air-target flag `0x10000` with the support flag `0x200`,
no surface flag, and seeker signature 3. The store-choice rule gives an
aircraft target the stores flagged `0x10000` and a surface target only those
flagged `0x20000` ([AI release](weapons.md#ai-release-against-ground-targets)),
so the data alone would let the Ticonderoga fire ASROC at aircraft, and never at
a ship or the surfaced OSCAR. Retail has no submarine target class. The record
zone 1 is 500 to 75,000 ft at 1,000 to 30,000 ft, its seeker reaches 100,000 ft,
and its motor is 2 s ignition, 20 s burn, 40 s removal.

The Ticonderoga is placed widely: 181 distinct `.M` missions name it, all in
`FA_2.LIB`. The first reading's four names were a small sample; the exact count
and the breakdown by series are in
[the missile format page](missiles.md#asroc).

**Resolution (reworked):** John kept ASROC **held** on 2026-11-06. It is an
anti-submarine weapon, there is no submarine target, and the SA-6 copy looks
like a placeholder. The Ticonderoga instead carries a fitted derived **SM-2**
on its two ASROC rails: a virtual `SM2.JT` whose every number is the SA-6's,
unchanged, drawing a private alias of the AMRAAM shape (`AMRAM.SH`, so the
AIM-120 keeps its own size) refitted to the real RIM-66's 15.5 ft by 13.5 in
(fitted). It fires as supported radar at aircraft through the same surface
firing path. The fit lives in `crates/tore-world/src/surface/fire.rs`
(`fitted_store`, Ticonderoga-only), the derived record in
`crates/tore-formats/src/weapon_corrections.rs` (`SURFACE_DERIVED`), and the
size in `crates/tore-sim/src/combat/store_sizes.rs`. It is still **Catalog**
for Load Ordnance, because no aircraft carries it. Evidence and the exact
decode are in [the missile format page](missiles.md#asroc); the gameplay
statement is in
[the missile spec](../spec/missiles.md#asroc-and-the-ticonderogas-sm-2-jt-6-2026-11-06).
The retail guard `crates/tore-import/tests/asroc.rs` (skips without media) pins
the flags, the zones, the motor, the Ticonderoga-only carrier and the 181-mission
count, and the world tests `the_ticonderoga_fit_carries_sm2_and_no_other_unit_changes`
and `a_ticonderoga_fires_sm2_at_aircraft_on_its_own_radar`
(`crates/tore-world/src/surface/fire_tests.rs`) cover the fit and the launch.

## Paired-point counts and physics

Research review, 2026-10-25, using the FA.EXE, FA.SMS and FA_2.LIB identities
above. The reviewed `HARDCanLoad` span below calculates one total capacity
for the source row. Pair flag 0x4 does not multiply that capacity or its
weight by two. `ArmPlane` creates one card per source ordinal. A paired
point's maximum of 3 therefore means 3 stores for that aggregate row, not
3 on each wing. The maximum can come from the weight quotient, rather than
an odd `maxItems` in the PT record. This distinction matters for the Hornet.

The [release-state evidence](weapons.md#release-state) reads the same row's
remaining count to alternate the lateral release offset. Thus retail counts
do have a release-side consequence, but do not establish a physical layout
of loaded stores. John reports that retail did not render those pylons or
stores. No retail execution or image comparison was performed in this pass.
The old T.O.R.E right-extra layout was an inference from release alternation,
not a recovered retail visualization rule.

The user-owned manual, printed pp.17 and 59 (local transcription
`.local/manual/manual.txt`, lines 1291-1316 and 3933-3942), describes
individual-count editing, total aircraft/weapon/fuel weight, and performance
penalties from ordnance weight and drag. It does not specify left/right
mass, CG or drag. The requested `.local/missile-update/manual.txt` was absent;
the existing manual transcription was used instead. AS-16 Kickback is a
separate store from AGM-84A Harpoon (manual weapon table, printed p.129).

The [reviewed flight loading](native-flight.md#force-assembly-now-translated)
uses aggregate mass buckets selected by hardpoint flag 1, not left/right
position, at `0x4516f2..0x451814`. Its loaded drag at `0x47a690/0x4784a0`
uses load percentages. The documented equipment resolver at `0x452770`
multiplies weapon quantity by weight, with pod/ammunition handling, and does
not double a paired count. This supports aggregate row weight and drag,
not a per-wing CG calculation, in the recovered path.

**Unknown:** whether another retail flight path computes a lateral store
moment or asymmetric drag. The reviewed loading, weight and release spans
do not prove absence across the executable. Next research step: trace the
flight consumers of the row quantities and `HARDStoreWeight`, specifically
any lateral-offset multiplication feeding roll or yaw. T.O.R.E currently
uses aggregate payload mass and aggregate load drag, with no store-CG term.
The [even-paired-load contract](../spec/stores-on-pylons.md#even-paired-loads)
is explicitly opinionated and does not depend on closing that research.

## Quantity, weight and pod follow-up

HARDStoreWeight 0x452940..0x45296c dispatches type 7 to OBJECT weight +0x57,
GAS type 8 to empty weight +5 plus fuel weight +8, and other equipment types to
word weight +5. HARDCanLoad 0x452bdd..0x452bfd uses
`min(maxItems, maxWeight * 100 / signed-word store weight)` for nonzero class and
weight. There is no paired multiplier in that reviewed weight branch. This
resolves that arithmetic, but does not establish the original default-loader
quantity or a physical pylon pairing rule.

HARDLoad 0x452c60..0x452c6d calls capacity only for count zero; an explicit
nonzero count uses the supplied number. The GAS branch 0x452cd1..0x452cde stores
fuel weight times installed count. The known A4E/F104 source-default versus
capacity discrepancies therefore still need the caller's initial/default-load
normalization, not a guessed paired weight factor. Keep the documented fitted
host default-capacity rule until that behavior is specified.

HARDPodHack 0x45377f..0x4537b0 multiplies or divides a projectile's stored
quantity by word projsInPod +0xaa. It does not divide by actualRoundsPerGame in
this span. For SUU16 this transforms one pod to 1200 stored projectile units.
The earlier host 600-logical-unit interpretation remains explicitly fitted;
confirm the player firing/HUD counter consumer before replacing it with a
claimed original displayed-ammunition number. No unrelated rocket-pod behavior
is changed by this evidence-only finding.

Unknown: original default-load normalization, player pod-counter display/debit
units, meaning of any physical pairing flag, stock/year/airbase lifecycle and
complete auxiliary-equipment replacement. Next source steps are the narrow
initial/default loader and player counter consumers. Full routine closure is
not needed to implement stable source-row editing with honest support limits.

## Hardpoint positions

Research for bug B3, 2026-10-12 (slice BP), from the same `FA_2.LIB` PT
records, read as text with the field order above. The location byte maps
0 Centerline, 1 Fuselage, 2 Internal Gun, 3 Internal Bay, 4 Wing, 5 Wingtip.
x / y / z are signed source words, right / up / forward of the aircraft's
reference point; the host divides by three for feet, the same conversion the
AC-130 gun mounts use ([aircraft variety](aircraft-variety.md)). One record
carries one position. Retail mirrors its lateral offset for flag 0x4 records
([release state](weapons.md#release-state)), so a flag 0x4 record's position is
its right-hand pylon. The records hold no pylon geometry, rack layout or store
attach point, and no retail shape draws a pylon, rack, rocket pod or fuel
tank (the weapon shapes in `FA_2.LIB` are missiles and bombs only; `.GAS`
records name no shape). Those are unknown in retail; the drawn ones are fitted
([stores on pylons](../spec/stores-on-pylons.md)).

What the table shows, by inspection:

- Wingtip rows sit at or just above the reference height (y 1 to -3) at the
  wing's tip: 19.3 ft out on the F/A-18D, 19.7 ft on the Rafale C.
- Wing rows step outward in about 15 to 17 unit (5 to 6 ft) increments, mostly
  a few feet aft of the reference point and 2 to 7 ft below it.
- Fuselage pairs sit 3 to 5 ft either side of the centreline, 5 to 8 ft below.
- The F-22's internal bay rows are one-sided (no flag 0x4): row 5 at x -10
  (left bay), row 6 at x +10 (right bay); its side bays (row 7) are flagged.
- Two Wing rows lack flag 0x4: `A4E.PT` row 4 (four Mk 82s at x 8) and
  `MI24.PT` row 2 (four AT-2s at x 25, flags `0x0188` with the fixed-slot
  bit). Retail releases those from one side; T.O.R.E pairs every Wing and
  Wingtip row (agent decision, [stores on pylons](../spec/stores-on-pylons.md#rules)),
  except a one-sided wing row less than 3 ft out, the A-4E's, which it hangs as a
  centreline rack (slice PY2, [the A-4E](../spec/stores-on-pylons.md#the-a-4e)).
  T.O.R.E's default A-4E load puts one 150 gallon tank on that row in place of the
  Mk 82s and two (not four) on row 5, the real aircraft's three tanks; the rows'
  capacities stay the source's (slice PY3).
- `F4E.PT` and `F4.PT` (F-4G) row 5 records their four Sparrows at x 0 with flag
  0x4: retail releases them from the centreline. T.O.R.E hangs them 3 ft either side,
  in the Sparrow wells (fitted, slice PY2).
- The records' heights (y) are where retail releases stores; T.O.R.E hangs and
  releases every wing store 1 ft under the model's underside over its pylon, a
  fuselage or centreline store 2 inches under it, and a wingtip missile on the tip's
  edge instead (opinionated, John 2026-10-13), so the y column is not where a store leaves in
  T.O.R.E ([pylon depth](../spec/stores-on-pylons.md#pylon-depth)). Fuselage rows
  with two stores a side hang them one behind another, which no record says either.
- `F14.PT` rows 4 to 7 hang at the real F-14's stations, not at these positions:
  the Phoenixes in the tunnel 1.25 ft out, the tanks under the intake nacelles,
  the AIM-120 under the glove pylon and the AIM-9 on its shoulder rail
  (opinionated, John 2026-10-14; [the F-14](../spec/stores-on-pylons.md#the-f-14)).
- `YAK141.PT` rows 4 and 5 hang at the Yak-141's inner and outer underwing pylons, 7.5 ft out
  2.8 ft back and 11 ft out 4.4 ft back, not at these positions (row 5's 16.7 ft at 0 ft forward is
  ahead of the swept wing, by the tip) (opinionated, John 2026-10-14;
  [the Yak-141](../spec/stores-on-pylons.md#the-yak-141)).
- `SU27.PT` and `SU35.PT` rows 5 to 7 hang at the Flankers' real stations: the centreline row
  in the tunnel one behind the other, row 6 under the intake ducts, row 7 one missile each on the
  middle and outer underwing pylons (opinionated, John 2026-10-14;
  [the Su-27 and Su-35](../spec/stores-on-pylons.md#the-su-27-and-su-35)).

Weapon and tank rows of the 37 registered identities (sensor, ECM and gun rows
omitted; Count is the default quantity):

| PT | Row | Location | Flags | x / y / z | Feet right / up / fwd | Pair (0x4) | Default | Count |
| --- | ---: | --- | --- | --- | --- | --- | --- | ---: |
| `F18.PT` | 4 | Fuselage | `0x0605` | 0 / -15 / 0 | 0.0 / -5.0 / 0.0 | yes | F150.GAS | 1 |
| `F18.PT` | 5 | Fuselage | `0x07e5` | 10 / -15 / 0 | 3.3 / -5.0 / 0.0 | yes | AIM120.JT | 2 |
| `F18.PT` | 6 | Wing | `0x07e5` | 25 / -6 / -3 | 8.3 / -2.0 / -1.0 | yes | AGM65G.JT | 4 |
| `F18.PT` | 7 | Wing | `0x07e5` | 41 / -6 / -6 | 13.7 / -2.0 / -2.0 | yes | AGM65G.JT | 4 |
| `F18.PT` | 8 | Wingtip | `0x0085` | 58 / 1 / -9 | 19.3 / 0.3 / -3.0 | yes | AIM9M.JT | 2 |
| `RAFALE.PT` | 5 | Fuselage | `0x05e5` | 10 / -20 / 0 | 3.3 / -6.7 / 0.0 | yes | AGM65G.JT | 4 |
| `RAFALE.PT` | 6 | Wing | `0x05e5` | 20 / -20 / 8 | 6.7 / -6.7 / 2.7 | yes | MICA.JT | 2 |
| `RAFALE.PT` | 7 | Wing | `0x05e5` | 36 / -20 / -18 | 12.0 / -6.7 / -6.0 | yes | R530.JT | 2 |
| `RAFALE.PT` | 8 | Wingtip | `0x0085` | 59 / -3 / -24 | 19.7 / -1.0 / -8.0 | yes | R550.JT | 2 |
| `F14.PT` | 4 | Fuselage | `0x0155` | 9 / -23 / 24 | 3.0 / -7.7 / 8.0 | yes | AIM54C.JT | 4 |
| `F14.PT` | 5 | Fuselage | `0x0605` | 9 / -23 / 20 | 3.0 / -7.7 / 6.7 | yes | F250.GAS | 2 |
| `F14.PT` | 6 | Wing | `0x07f5` | 18 / -20 / 14 | 6.0 / -6.7 / 4.7 | yes | AIM120.JT | 2 |
| `F14.PT` | 7 | Wing | `0x0485` | 26 / -16 / 5 | 8.7 / -5.3 / 1.7 | yes | AIM9M.JT | 2 |
| `A4E.PT` | 4 | Wing | `0x1781` | 8 / -16 / -6 | 2.7 / -5.3 / -2.0 | no | MK82.JT | 4 |
| `A4E.PT` | 5 | Wing | `0x1785` | 17 / -16 / -10 | 5.7 / -5.3 / -3.3 | yes | F150.GAS | 4 |
| `A4E.PT` | 6 | Wing | `0x1585` | 26 / -16 / -13 | 8.7 / -5.3 / -4.3 | yes | LAU61.JT | 2 |
| `F31.PT` | 5 | Centerline | `0x0681` | 0 / -20 / 0 | 0.0 / -6.7 / 0.0 | no | none | 1 |
| `F31.PT` | 6 | Wing | `0x05e5` | 11 / -20 / -6 | 3.7 / -6.7 / -2.0 | yes | AIM120.JT | 2 |
| `F31.PT` | 7 | Wing | `0x05e5` | 22 / -20 / -12 | 7.3 / -6.7 / -4.0 | yes | AGM65G.JT | 2 |
| `F31.PT` | 8 | Wingtip | `0x0085` | 35 / -7 / -17 | 11.7 / -2.3 / -5.7 | yes | AIM9X.JT | 2 |
| `MIG29.PT` | 5 | Wing | `0x07e5` | 20 / -10 / -5 | 6.7 / -3.3 / -1.7 | yes | AA8.JT | 2 |
| `MIG29.PT` | 6 | Wing | `0x07e5` | 25 / -10 / -5 | 8.3 / -3.3 / -1.7 | yes | AA8.JT | 2 |
| `MIG29.PT` | 7 | Wing | `0x07e5` | 50 / -10 / -5 | 16.7 / -3.3 / -1.7 | yes | AA8.JT | 2 |
| `SU27.PT` | 5 | Centerline | `0x07e5` | 0 / -25 / 12 | 0.0 / -8.3 / 4.0 | yes | AA12.JT | 2 |
| `SU27.PT` | 6 | Fuselage | `0x07e5` | 24 / -25 / 8 | 8.0 / -8.3 / 2.7 | yes | AA12.JT | 2 |
| `SU27.PT` | 7 | Wing | `0x07e5` | 51 / -18 / -4 | 17.0 / -6.0 / -1.3 | yes | AA11.JT | 4 |
| `SU27.PT` | 8 | Wingtip | `0x07e5` | 84 / -8 / -16 | 28.0 / -2.7 / -5.3 | yes | AA11.JT | 2 |
| `MIG21.PT` | 4 | Centerline | `0x07e1` | 0 / -20 / -5 | 0.0 / -6.7 / -1.7 | no | none | 1 |
| `MIG21.PT` | 5 | Wing | `0x07e5` | 20 / -13 / -5 | 6.7 / -4.3 / -1.7 | yes | AA2.JT | 2 |
| `MIG21.PT` | 6 | Wing | `0x07e5` | 30 / -13 / -12 | 10.0 / -4.3 / -4.0 | yes | AA2.JT | 2 |
| `SU25.PT` | 5 | Centerline | `0x07e5` | 0 / -21 / 6 | 0.0 / -7.0 / 2.0 | yes | none | 1 |
| `SU25.PT` | 6 | Fuselage | `0x07e5` | 15 / -21 / 4 | 5.0 / -7.0 / 1.3 | yes | B13.JT | 2 |
| `SU25.PT` | 7 | Wing | `0x07e5` | 31 / -13 / 2 | 10.3 / -4.3 / 0.7 | yes | AS7.JT | 4 |
| `SU25.PT` | 8 | Wing | `0x07e5` | 48 / -13 / -1 | 16.0 / -4.3 / -0.3 | yes | AA8.JT | 4 |
| `MIG23.PT` | 4 | Centerline | `0x0601` | 0 / -20 / 17 | 0.0 / -6.7 / 5.7 | no | none | 1 |
| `MIG23.PT` | 5 | Fuselage | `0x0085` | 6 / -20 / 15 | 2.0 / -6.7 / 5.0 | yes | none | 4 |
| `MIG23.PT` | 6 | Fuselage | `0x07e5` | 11 / -20 / 11 | 3.7 / -6.7 / 3.7 | yes | B8.JT | 2 |
| `MIG23.PT` | 7 | Fuselage | `0x07e5` | 15 / -20 / 8 | 5.0 / -6.7 / 2.7 | yes | AS7.JT | 2 |
| `MIG23.PT` | 8 | Wing | `0x07e5` | 18 / -20 / 3 | 6.0 / -6.7 / 1.0 | yes | AS7.JT | 2 |
| `SU35.PT` | 5 | Centerline | `0x07e5` | 0 / -25 / 12 | 0.0 / -8.3 / 4.0 | yes | AAML.JT | 1 |
| `SU35.PT` | 6 | Fuselage | `0x07e5` | 24 / -25 / 8 | 8.0 / -8.3 / 2.7 | yes | AAML.JT | 2 |
| `SU35.PT` | 7 | Wing | `0x07e5` | 51 / -18 / -4 | 17.0 / -6.0 / -1.3 | yes | AA12.JT | 4 |
| `SU35.PT` | 8 | Wingtip | `0x0fed` | 84 / -8 / -16 | 28.0 / -2.7 / -5.3 | yes | AA11B.JT | 2 |
| `F22.PT` | 5 | Internal bay | `0x05e0` | -10 / -25 / 0 | -3.3 / -8.3 / 0.0 | no | AIM120.JT | 2 |
| `F22.PT` | 6 | Internal bay | `0x05e0` | 10 / -25 / 0 | 3.3 / -8.3 / 0.0 | no | AGM65G.JT | 2 |
| `F22.PT` | 7 | Internal bay | `0x05e4` | 27 / -11 / 0 | 9.0 / -3.7 / 0.0 | yes | AIM9X.JT | 4 |
| `F22N.PT` | 5 | Fuselage | `0x05e1` | -10 / -25 / 0 | -3.3 / -8.3 / 0.0 | no | AIM120.JT | 2 |
| `F22N.PT` | 6 | Fuselage | `0x05e1` | 10 / -25 / 0 | 3.3 / -8.3 / 0.0 | no | AGM65G.JT | 2 |
| `F22N.PT` | 7 | Fuselage | `0x05e5` | 27 / -11 / 0 | 9.0 / -3.7 / 0.0 | yes | AIM9X.JT | 4 |
| `AV8.PT` | 5 | Wing | `0x07e5` | 20 / -13 / -20 | 6.7 / -4.3 / -6.7 | yes | AGM65G.JT | 4 |
| `AV8.PT` | 6 | Fuselage | `0x0585` | 9 / -22 / -16 | 3.0 / -7.3 / -5.3 | yes | AGM65G.JT | 2 |
| `AV8.PT` | 7 | Wing | `0x0085` | 32 / -13 / -25 | 10.7 / -4.3 / -8.3 | yes | AIM9M.JT | 2 |
| `YAK141.PT` | 4 | Wing | `0x07e5` | 25 / -10 / 0 | 8.3 / -3.3 / 0.0 | yes | AA10.JT | 2 |
| `YAK141.PT` | 5 | Wing | `0x07e5` | 50 / -10 / 0 | 16.7 / -3.3 / 0.0 | yes | AA11.JT | 2 |
| `AH64.PT` | 4 | Centerline | `0x0605` | 0 / -10 / 0 | 0.0 / -3.3 / 0.0 | yes | none | 1 |
| `AH64.PT` | 5 | Fuselage | `0x0185` | 20 / -20 / 0 | 6.7 / -6.7 / 0.0 | yes | LAU61.JT | 2 |
| `AH64.PT` | 6 | Wing | `0x0085` | 30 / -20 / 0 | 10.0 / -6.7 / 0.0 | yes | AIM9M.JT | 2 |
| `MI24.PT` | 2 | Wing | `0x0188` | 25 / -10 / 0 | 8.3 / -3.3 / 0.0 | no | AT2.JT | 4 |
| `MIG17F.PT` | 3 | Wing | `0x0705` | 22 / -8 / -6 | 7.3 / -2.7 / -2.0 | yes | F150.GAS | 2 |
| `F4B.PT` | 3 | Centerline | `0x0785` | 0 / -19 / 0 | 0.0 / -6.3 / 0.0 | yes | AIM9B.JT | 4 |
| `F4B.PT` | 4 | Fuselage | `0x07e5` | 13 / -19 / 0 | 4.3 / -6.3 / 0.0 | yes | MK82.JT | 4 |
| `F4B.PT` | 5 | Wing | `0x0785` | 26 / -19 / -9 | 8.7 / -6.3 / -3.0 | yes | MK82.JT | 4 |
| `F4B.PT` | 6 | Wing | `0x0605` | 38 / -19 / -20 | 12.7 / -6.3 / -6.7 | yes | F350.GAS | 2 |
| `F4J.PT` | 4 | Centerline | `0x0785` | 0 / -20 / 0 | 0.0 / -6.7 / 0.0 | yes | none | 1 |
| `F4J.PT` | 5 | Fuselage | `0x07e5` | 20 / -20 / -9 | 6.7 / -6.7 / -3.0 | yes | AIM7E.JT | 4 |
| `F4J.PT` | 6 | Wing | `0x0785` | 31 / -20 / -17 | 10.3 / -6.7 / -5.7 | yes | AIM9B.JT | 4 |
| `F4J.PT` | 7 | Wing | `0x0605` | 44 / -20 / -25 | 14.7 / -6.7 / -8.3 | yes | F350.GAS | 2 |
| `F4E.PT` | 4 | Centerline | `0x0605` | 0 / -20 / 0 | 0.0 / -6.7 / 0.0 | yes | F150.GAS | 1 |
| `F4E.PT` | 5 | Fuselage | `0x00c5` | 0 / -20 / 0 | 0.0 / -6.7 / 0.0 | yes | AIM7.JT | 4 |
| `F4E.PT` | 6 | Wing | `0x0765` | 20 / -20 / -9 | 6.7 / -6.7 / -3.0 | yes | AGM65G.JT | 4 |
| `F4E.PT` | 7 | Wing | `0x07e5` | 39 / -20 / -15 | 13.0 / -6.7 / -5.0 | yes | none | 2 |
| `F4.PT` | 4 | Centerline | `0x0605` | 0 / -20 / 0 | 0.0 / -6.7 / 0.0 | yes | F150.GAS | 1 |
| `F4.PT` | 5 | Fuselage | `0x00c5` | 0 / -20 / 0 | 0.0 / -6.7 / 0.0 | yes | AIM7.JT | 4 |
| `F4.PT` | 6 | Wing | `0x0765` | 20 / -20 / -9 | 6.7 / -6.7 / -3.0 | yes | AGM88.JT | 4 |
| `F4.PT` | 7 | Wing | `0x0765` | 39 / -20 / -15 | 13.0 / -6.7 / -5.0 | yes | none | 2 |
| `A7.PT` | 4 | Fuselage | `0x0085` | 7 / -28 / 0 | 2.3 / -9.3 / 0.0 | yes | AIM9M.JT | 2 |
| `A7.PT` | 5 | Wing | `0x0705` | 19 / -13 / -5 | 6.3 / -4.3 / -1.7 | yes | MK82.JT | 2 |
| `A7.PT` | 6 | Wing | `0x0785` | 35 / -12 / -11 | 11.7 / -4.0 / -3.7 | yes | AGM65G.JT | 4 |
| `F15.PT` | 4 | Fuselage | `0x07e5` | 25 / -10 / 0 | 8.3 / -3.3 / 0.0 | yes | AIM120.JT | 4 |
| `F15.PT` | 5 | Wing | `0x07e5` | 50 / -20 / 0 | 16.7 / -6.7 / 0.0 | yes | AIM9M.JT | 4 |
| `F16C.PT` | 5 | Centerline | `0x0601` | 0 / -20 / 0 | 0.0 / -6.7 / 0.0 | no | F150.GAS | 1 |
| `F16C.PT` | 6 | Wing | `0x07e5` | 32 / -10 / -7 | 10.7 / -3.3 / -2.3 | yes | AGM65G.JT | 4 |
| `F16C.PT` | 7 | Wing | `0x0185` | 43 / -10 / -11 | 14.3 / -3.3 / -3.7 | yes | AIM120.JT | 2 |
| `F16C.PT` | 8 | Wingtip | `0x0085` | 54 / 2 / -14 | 18.0 / 0.7 / -4.7 | yes | AIM9M.JT | 2 |
| `F104.PT` | 4 | Centerline | `0x0605` | 0 / -15 / -7 | 0.0 / -5.0 / -2.3 | yes | F250.GAS | 1 |
| `F104.PT` | 5 | Wingtip | `0x0605` | 37 / -5 / -10 | 12.3 / -1.7 / -3.3 | yes | F250.GAS | 2 |
| `F104.PT` | 6 | Fuselage | `0x07e5` | 10 / -15 / -7 | 3.3 / -5.0 / -2.3 | yes | AIM7.JT | 2 |
| `F104.PT` | 7 | Wing | `0x07e5` | 18 / -15 / -7 | 6.0 / -5.0 / -2.3 | yes | AIM7.JT | 2 |
| `F104.PT` | 8 | Wing | `0x0285` | 25 / -15 / -9 | 8.3 / -5.0 / -3.0 | yes | AIM9M.JT | 2 |
| `A10.PT` | 5 | Wing | `0x0585` | 10 / -10 / 0 | 3.3 / -3.3 / 0.0 | yes | AGM65G.JT | 4 |
| `A10.PT` | 6 | Wing | `0x0585` | 25 / -10 / 0 | 8.3 / -3.3 / 0.0 | yes | AGM65G.JT | 4 |
| `A10.PT` | 7 | Wing | `0x0585` | 40 / -10 / 0 | 13.3 / -3.3 / 0.0 | yes | AGM65G.JT | 2 |
| `A10.PT` | 8 | Wing | `0x0185` | 55 / -10 / 0 | 18.3 / -3.3 / 0.0 | yes | AGM65G.JT | 2 |
| `M25.PT` | 3 | Wing | `0x00c5` | 38 / -11 / -15 | 12.7 / -3.7 / -5.0 | yes | AA6.JT | 2 |
| `M25.PT` | 4 | Wing | `0x00c5` | 51 / -11 / -21 | 17.0 / -3.7 / -7.0 | yes | AA2.JT | 2 |
| `M25.PT` | 5 | Centerline | `0x0601` | 0 / -25 / -4 | 0.0 / -8.3 / -1.3 | no | F250.GAS | 1 |

The F-15C and F-16C rows above are source coordinates. Their current
fitted placement is documented in [F-15C stores](../spec/stores-on-pylons.md#the-f-15c)
and [F-16C stores](../spec/stores-on-pylons.md#the-f-16c-and-fleet-rail-audit).
The Mi-24 row 2 also reads its attachment from below the stub wing
([placement](../spec/stores-on-pylons.md#mi-24-lower-wing-attachment)).
The MiG-25's three carrier rows (3, 4 and 5) hang at their decoded positions
under the standard rules, with the measured depths and John's accepted AA-6 fin
entry recorded in [the MiG-25](../spec/stores-on-pylons.md#the-mig-25).
No source rows or station capacities were added.

## Original icons and geometry candidates

The four original tank thumbnails `$F150.PIC`, `$F250.PIC`, `$F350.PIC` and
`$F500.PIC` are present in FA_1.LIB and decode to 105 by 19 pixels with identical
artwork. GAS records have no thumbnail pointer. The editor uses the generated
`$<stem>.PIC` name, and selective dependency reports now record that generated
edge. Runtime menu imports already carried the icons independently.

`F250.SH` and `F500.SH` exist in FA_2.LIB and each decodes to 68 untextured
faces without state words. Matching `F150.SH` and `F350.SH` were not found.
Those filename matches do not prove an installed or jettisoned tank association.
No mesh is attached by this pass. Source Shift+J text describes external fuel
jettison, but does not establish shell removal, dumping versus dropping, or the
shape to use. Review the attachment/jettison consumer before using these shapes.

## Internal ammunition weight follow-up

John's supplied retail page reference shows the F-14 at 65,186 lb with 675 M61
units, four Phoenix, two F250 tanks, two AMRAAM and two Sidewinder, with 15,741 lb
internal fuel. The host shows 64,511 lb for that selection. The screenshot's
executable identity is unknown, but reviewed source arithmetic explains the
675 lb difference: ArmPlane retains fixed projectile rows, the card helper at
0x41c68c calls HARDStoreWeight and returns its weight at 0x41c6e9, and the weight
loop at 0x41a957..0x41a964 adds finite loaded quantity times that weight without
an internal-gun exclusion. Quantity 0x7fff takes a separate sentinel branch.
M61.JT has OBJECT weight 1 lb and F14.PT supplies 675 units. M61.JT SHA-256:
`0b79689a728911d1097569f6e6612050e414ba9703c9f131a51fe1ce9a7eb33d`.

The current host excludes internal-gun quantities from loadout weapon mass.
This is a known screen-weight discrepancy, not unknown arithmetic. The current
layout and availability pass preserves that existing mass rule. Next review
flight initialization and ammunition mass debit together before changing the
shared loadout/flight weight behavior; a display-only correction would make the
page disagree with launch validation and flight mass.

## External equipment pods name no shape (EQ-2, 2026-11-06)

The designator, FLIR and jammer pods the import keeps are equipment records
(SEE/ECM), not weapons, so a store's shape pointer never names them. A decoded
read of the installed 1.02F `FA_2.LIB` confirms it: the `SENSOR` and `ECM`
schemas (`crates/tore-formats/src/aircraft_schema.rs`) carry no shape field, and
neither `PAVESPK.SEE`, `AAS38.SEE` nor `ALQ167.ECM` has one in its bytes. The
media holds only their thumbnails (`$PAVESPK.PIC`, `$AAS38.PIC`, `$ALQ167.PIC`
in `FA_1.LIB`) and the unrelated Paveway bomb shape `PAVE.SH`; there is no
`PAVESPK.SH`, `AAS38.SH` or `ALQ167.SH`.

The six external pods are exactly the SEE/ECM records that carry a weight and
the equipment flag bit 1:

| Record | Weight lb | flags | Decoded carriers ([fa-aircraft.csv](fa-aircraft.csv)) |
| --- | ---: | ---: | --- |
| `PAVESPK.SEE` | 425 | `0x1` | A-10, F-16C, B-1, B-2, F-111, F-117, F-15E, Jaguar, Kfir, Tornado |
| `PAVEKNF.SEE` | 425 | `0x1` | retired (a loadout naming it loads the Pave Spike) |
| `AAS38.SEE` | 350 | `0x1` | Rafale, Gripen |
| `ALQ167.ECM` | 310 | `0x1` | A-6, AV-8B, Kfir, Mirage F1 |
| `ALQ72.ECM` | 224 | `0x1` | M5 |
| `ALE40.ECM` | 100 | `0x1` | none (a dispenser) |

Every other SEE/ECM record has weight 0 and flags `0x0`; those are the internal
radars, FLIRs, visual sensors and ECM suites. So the import treats a hardpoint
record as an external pod when it ends `.SEE` or `.ECM`, has weight, and sets
flag bit 1 (`crates/tore-sim/src/combat/live.rs`, `Configuration::fixed_pods`).
The carriers are the same auxiliary rows the source catalog already records.

Implementation (EQ-2, 2026-11-06): a fixed pod is drawn as a fitted body on its
hardpoint, and a designator pod loaded through Load Ordnance is drawn the same
way on its station (`crates/tore-world/src/snapshot.rs` `store_poses_of`). The
body sizes are fitted public pod dimensions
(`crates/tore-sim/src/combat/store_sizes.rs` `equipment_pod_body`), because the
records name no shape. A fixed pod's source row is a placeholder position, not
a real pylon (the F-16C's Pave Spike is recorded at the reference height,
inside the fuselage), so it hangs under the model's lowest underside across its
own length (`Configuration::from_source`). Nothing is loaded, jettisoned or
counted twice: a fixed pod's mass stays in `fixed_external_equipment_lbs` and
it is not an external store in the radar signature.

Addition (2026-11-07): the Pave Spike, AAS-38 and ALQ-167 now draw authored
low-poly models in place of the fitted body, at John's request. The records
still name no shape; the models are ours, and the AAS-38's size is corrected to
6.0 by 1.08 ft
([authored pod models](../spec/stores-on-pylons.md#authored-pod-models-2026-11-07)).

Addition (2026-11-07, John's decision): a fixed pod draws only when its row is a
real station. The A-10's and AV-8B's pod rows sit at the reference point and the
F-16C's at [0, 0, 50], inside the airframe, so they draw nothing; the Rafale's
AAS-38 row is under its belly and draws
([rule and evidence](../spec/stores-on-pylons.md#fixed-pods-draw-only-on-a-real-station-2026-11-07)).

Guards: `a_fixed_pod_draws_a_fitted_body_and_an_internal_sensor_does_not`
(`crates/tore-world/src/content_tests.rs`),
`a_loaded_designator_pod_draws_a_fitted_body_on_its_station`
(`crates/tore-world/src/snapshot.rs`) and the retail
`crates/tore-world/src/pod_equipment_tests.rs` (skips without media).
