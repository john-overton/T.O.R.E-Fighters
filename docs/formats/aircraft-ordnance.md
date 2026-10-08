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

The 37 distinct source-compatible imported JT names outside the live list are:

`AA6.JT`, `AA9.JT`, `AEMP1.JT`, `AGM45.JT`, `AGM65A.JT`, `AGM84A.JT`, `AGM84E.JT`, `AM39.JT`, `AS14.JT`, `AS15.JT`, `AS16.JT`, `AS30.JT`, `AT12.JT`, `CBU87.JT`, `CBU89.JT`, `FAB1000.JT`, `FAB250.JT`, `FAB500.JT`, `GBU10.JT`, `GBU10A.JT`, `GBU28.JT`, `GBU29.JT`, `GBU29P.JT`, `GBU30.JT`, `LAU10.JT`, `MK20.JT`, `MK82AIR.JT`, `MK82HD.JT`, `MK82P.JT`, `MK82P3.JT`, `MK84.JT`, `PAVEWA3.JT`, `PAVEWAY.JT`, `PL10.JT`, `PL7.JT`, `RBK250.JT`, `RBK500.JT`.

These include additional radar/IR missiles, passive emitter and surface weapons,
laser/designator-dependent cases, bomb/cluster variants and another rocket pod.
The [missile inventory](../spec/missiles.md#first-pass-inventory-matrix) owns
candidate guidance and hold decisions. A name, physical fit or imported JT
never establishes implemented behavior. The per-aircraft gap counts above
include only names with a positive normal capacity, not every catalog name.

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
