# Quick Mission ground-target templates

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research notes for the surface-AI round, implementation mode, 2026-10-10. The
Quick Mission Creator's ground target (field 30) loads one of 129 template
missions from `FA_2.LIB`, `~Q<theater><target>.M`. This page is the grammar
and fact contract for `tore_formats::quick_template`
(`crates/tore-formats/src/quick_template.rs`, facts in
`quick_template/tables.rs`) and for `nationality3` in
`tore_formats::mission`. How the creator rolls and places the contents is in
[quick-mission.md](quick-mission.md); player
behaviour will live in [the surface defenses spec](../spec/surface-defenses.md).
Evidence: `FA.EXE` 1.02F, SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`, read as
data and disassembly, never executed.

## Template files

Plain mission text, CRLF line ends, no `textFormat` header, ending in a Ctrl-Z
(`0x1a`) or a NUL after trailing blank lines. The reader stops at the first of
those bytes. Top level statements:

| Statement | Count in 129 files | Notes |
| --- | ---: | --- |
| `obj` ... `.` | 5,315 | one object, fields below |
| `waypoint2 N` ... `.` | 15 | a route block of N waypoints, ending in a `w_for` |
| `quickpos x y z` or `quickPos x y z` | 18 | only in the 18 "nothing" templates; both spellings occur, the reader folds case |

Object fields, tab indented: `type`, `pos`, `angle`, `flags`, `speed`, `alias`
on every object; one of `nationality`, `nationality2`, `nationality3` on every
object; `skill`, `react`, `searchDist` on 4,034; `startTime` on 159. Numbers are
decimal or `$` hex in the same file (`flags $413` and `flags 19`). `speed` is 0
on all 5,315 objects. Unknown fields are kept in source order.

`type` is a resource (`T72.NT`, `BNK6.OT`, `SU35.PT`) or a placeholder in angle
brackets. The files write lower case (`<sam>`); the executable compares
case-sensitively against the lower case spelling, the reader folds case. The
placeholders: `<sam>` 891, `<aaa>` 879, `<tank>` 298, `<afv>` 244,
`<destroyer>` 44, `<cargo>` 31, `<cruiser>` 14, `<small>` 8, `<hovercraft>` 6,
`<carrier>` 4. `<vehicle>` and `<nothing>` are known to the generator but appear
in no shipped template.

Largest template: 127 objects. Aliases are unique inside every template.

## Routes

A `waypoint2 N` block lists N waypoints (1 to 10; manual p. 209 "up to ten")
and ends with `w_for <alias>`, the alias of the object that follows it. Each
waypoint has `w_index` (0 up), `w_flags`, `w_goal`, `w_next`, `w_pos2`,
`w_speed`, `w_wng`, `w_react`, `w_searchDist`, `w_preferredTargetId` (or
`...Id2`), `w_name` (two Ctrl-A marks around the name, or empty). The `w_for`
line is indented with two spaces in some files.

| Field | Values |
| --- | --- |
| `w_flags` | 1 start, 4 leg, 2 end (also `$1`, `$4`, `$2`) |
| `w_pos2` | five numbers: `0 0 x y z` on the start and end, `1 0 x y z` on legs; the end waypoint reads `0 0 0 0 0` |
| `w_speed` | 0 on start and end, 16 or 50 on legs, in feet per second |
| `w_wng` | `1 0 2048 0` on legs (the aircraft wing format), zeros elsewhere |

Five templates hold routes: `~QUCOL` (nine tanks, five waypoints each, three
legs at 50 ft/s, about 60,500 ft per tank), `~QTCARGO` (three CARGO2 ships,
three waypoints, one leg each at 16 ft/s, 87,000 to 102,000 ft long, all
converging on one point), `~QUFACT` (one TRUCK, six waypoints, four legs at
50 ft/s, about 6,100 ft) and the unreferenced `~QFACT` (a copy of it) and
`~QUBUNK` (one TRUCK, about 72,000 ft). Every other object is stationary. A
`w_for` whose alias matches no object or more than one object is an error.

## Owners and `nationality3`

The nationality byte's bit `0x80` is the side: set means Redfor (the retail
"them" flag), clear Blue. The low seven bits index the 60-entry creator
nationality list. Three spellings occur:

| Field | Index rule | Used by |
| --- | --- | --- |
| `nationality` (legacy) | map-dependent remap, `mission_nationality`; values 0 and 137 only | the UKR layout and 196 template objects in 11 templates (`~QUCITY`, `~QUFACT`, `~QUBRI`, `~QUNUKE`, the Ukraine and Kuril fleets, `~QFACT`, `~QUBUNK`, `~QURADAR`) |
| `nationality2` | already numbered | BAL, EGY, FRA, KURILE, TVIET and VLA layouts, 1,592 template objects in 44 templates |
| `nationality3` | already numbered | CUB, LFA, GRE, IRA, SPA, APA, PGU, NSK and WTA layouts, 3,527 template objects in 56 templates |

`mission::Placement` keeps `nationality3` (a bool beside `nationality2`) and
decodes it like `nationality2`. `Placement::nationality_field()` reports the
spelling and `Placement::redfor()` the side. The creator rewrites a template's
`nationality` and `nationality2` to the enemy nationality and passes
`nationality3` through unchanged, so `~QIRRETR` keeps 40 American (Blue) objects
and `~QSPFRU` 21 Pakistani (Blue) objects. A placement may carry only one of the
three fields; two is an error. With `nationality3` read, the enemy and friendly
placement counts of all 16 base layouts equal the survey table (for example
Cuba 118 and 11, Taiwan 29 and 129).

## Recorded facts from FA.EXE (1.02F)

All of the following live in `quick_template/tables.rs` and are checked
against the executable by an ignored import test. Addresses are virtual
addresses in `.data` of the 1.02F build; the disc 1.0 build holds the same
tables elsewhere and is not covered.

| Fact | Address | Value |
| --- | --- | --- |
| Template names | `0x4f3278..0x4f348c` | 124 name pointers, theaters in the order UKR, KURILE, TVIET, CUB, PGU, LFA, APA, NSK, SPA, WTA, IRA, GRE, EGY, VLA, FRA, BAL, each in menu order; zero words separate some theaters |
| Unreferenced extras | none | `~QFACT`, `~QUBUNK`, `~QURADAR`, `~QMANOTH`, `~QOSNOTH` |
| Defense percent, SAM and AAA | `0x4f3148`, `0x4f31b8` | 0, 25, 60, 100 by strength setting |
| Nationality to group | `0x4f1e58` | 60 words, groups 0 to 4 |
| Placeholder unit lists | `0x4f2e68..0x4f31b0` | eleven blocks of five lists, below |
| Night list | `0x4f31b0` | ZSU23 |
| Placeholder spellings | `0x4f3914..0x4f3984` | `<aaa>` `<nothing>` `<sam>` `<afv>` `<tank>` `<vehicle>` `<hovercraft>` `<small>` `<destroyer>` `<carrier>` `<cargo>` `<cruiser>` |

Each block stores its five lists in the order group 4, 0, 1, 2, 3, each list
zero terminated and padded to eight bytes. The placeholder code selects a list
by group (confirmed from the jump tables at `0x4321e4`, `0x4321f8` and the
argument order of the helper at `0x432850`), so only the group index matters:

| Placeholder | Group 0 | Group 1 | Group 2 | Group 3 | Group 4 |
| --- | --- | --- | --- | --- | --- |
| `<sam>` | FIM92, ROLAND, CHAP | MIS, ASA5 | SA6, SA7, SA9, SA13, SA14, SA15, 2S6 | M113, CHAP, SA6, SA9, SA14, ASA5 | M113, CHAP, ASA5, SA7, SA13 |
| `<aaa>` | M163, M113 | M113, ZSU23 | ZSU23, ZSU57 | M113, ZSU23, ZSU57, ZIF31 | ZSU23, ZIF31, M113 |
| `<tank>` | M1, M2 | T80, T90 | T72, T80, T90 | T72, M1, M2 | M1, M2 |
| `<afv>` | M113, HUMVEE | M113, HUMVEE | BMP2, BTR80 | M113, BMP2, BTR80 | M113, HUMVEE |
| `<vehicle>` | TRUCK, TANKER | TRUCK, TANKER, SRDR1, SRDR2 | TRUCK, TANKER, LTRACK, SFLUSH | TRUCK, TANKER, LTRACK, SFLUSH | TRUCK, TANKER, LTRACK |
| `<small>`, `<hovercraft>` | SL100, LCAC, SESHDW | SL100 | PMORN, SARAN | SARAN | SL100 |
| `<destroyer>` | TICON | TYPE69 | KRIVAK, JIANC | JIANE, KNOX | JIANE, KNOX |
| `<cruiser>` | IOWA, TICON | TYPE69 | KIROV, SOVR, JIANC | JIANE, KNOX | JIANE, KNOX |
| `<carrier>` | NIMZ, WASP | CLEM | KIEV | KIEV | NIMZ |
| `<cargo>` | CARGO, SACRAM | CARGO | CARGO, OLEKMA | CARGO | CARGO |

The night and stealth rule: conditions index 6 (night) with `F117.PT` or
`B2.PT` in a friendly wing replaces every manned `<aaa>` pick with ZSU23 and
writes skill 0 (novice). Neither aircraft is imported, so the rule is dormant.

Template names per theater (source order of creator field 13):

| Theater | Templates |
| --- | --- |
| BAL | QBNOTH QBFLT QBAIR QBBRD QBXING QBACOL QBFAIR QBSPPY QBSHAR |
| CUB | QCNOTH QCFAIR QCSCUD QCSUB QCLST QCCARG QCCMHQ |
| EGY | QENOTH QESFLT QESAIR QELAIR QECMHQ QERDRI QEARMOR QECDEF |
| LFA | QLFNOTH QLFCARG QLFPATR QLFSAM QLFFAIR QLFSTOR QLFCMHQ |
| FRA | QFNOTH QFFLT QFSAIR QFLAIR QFSUP QFRDRI QFCMHQ QFFACT |
| GRE | QGRNOTH QGRSAIR QGRPATR QGRRDR QGRCARG QGRSTOR |
| IRA | QIRNOTH QIRRDR QIRFAIR QIRPOW QIRCCC QIRARM QIRSCUD QIRCWP QIRRETR |
| KURILE | QKNOTH QKSFLT QKLFLT QKSCFT QKSUB QKPLNGR QKSILO QKARMOR |
| TVIET | QTNOTH QTBARG QTCARGO QTBRDG QTBUNK QTCOMM QTSTRG QTTRUCK QTAAA QTSAM |
| SPA | QSPNOTH QSPFAIR QSPSAM QSPASA QSPFRU QSPSUP QSPCMHQ |
| APA | QAPNOTH QAPFAIR QAPBLK QAPPATR QAPHELO QAPSAM QAPCMHQ |
| PGU | QPGNOTH QPGPATR QPGFAIR QPGSAM QPGSRUN QPGRDR QPGWSHP |
| NSK | QNSNOTH QNSFAIR QNSARM QNSFOA QNSBORD QNSCOL QNSSUP |
| WTA | QWTNOTH QWTFAIR QWTPATR QWTHYDO QWTWARS QWTCARG QWTLAND |
| UKR | QUNOTH QUSFLT QULFLT QUCITY QUFACT QUSTRIP QUCOL QUNUKE QUBRI |
| VLA | QVNOTH QVSFLT QVSAIR QVLAIR QVCMHQ QVARMOR QVRDRI QVSUP |

The resource is `~<name>.M`. The creator's own target-label counts per theater
equal these list lengths.

## Reader bounds

4 MiB per file, 4,096 bytes per line, 128 fields per object, 512 objects per
template, 1 to 10 waypoints per route, one route per object, routes bound by
alias. Anything else is an error; unknown top-level statements are errors too
(unlike the theater reader, which skips them).

## Not established

- How `quickpos` is consumed (the engagement anchor).
- The unit and consumer of `startTime` (60 to 5,400 on 159 objects; 98 are
  aircraft).
- Whether `w_next` and `w_goal` loop a route; all retail values are 0 and 0/1.
- The disc 1.0 build's table addresses.

## Reproduce

```sh
cargo run -p tore-formats --example surface_inspect -- gameassets/fighters-anthology/FA_2.LIB template UCOL
cargo test -p tore-formats quick_template::import_tests -- --ignored
```
