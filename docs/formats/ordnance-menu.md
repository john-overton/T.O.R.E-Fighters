# Load Ordnance screen contract

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes — research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature — see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


Static research against the hash-identified FA.EXE/SMS pair recorded in
[menu evidence](../baselines/menu-contract-pass.md). Addresses below are virtual
addresses in that executable. No imported instructions are executed. This is a
partial source contract, not an implemented screen or full interaction parity.

## Actions, pages and fuel

The five-entry dispatch at `0x41b336`, consumed at `0x41b316`, maps actions:

| ID | Branch | Behavior |
| ---: | --- | --- |
| 1 | `0x41b0d1` | Page rocker |
| 2 | `0x41b139` | Select category bank one |
| 3 | `0x41b18d` | Select category bank two |
| 4 | `0x41b1e1` | Internal fuel rocker |
| 5 | `0x41b2d6` | Fly / weight validation |

The bounded research reader emits `ordnance-controls.json`, preserving table
order and rejecting unknown or duplicated branch targets. Dial labels still need
their dynamic draw contract linked to these two bank identities.

Pages contain eight entries. Previous stops at zero; next stops at
`(entry_count - 1) / 8` for a nonempty catalog. Category switches save the old
page and restore the selected category's page. Selecting the current category
is a no-op. Empty-catalog behavior needs an explicit port-side bound.

Fuel rocker result 1 adds 500 source mass units; its other result subtracts 500.
Fuel clamps to zero/internal capacity and adjusts current aircraft weight by the
actual clamped delta. The mass display uses pounds, but the simulation conversion
must use the reviewed aircraft contract. The branch reuses an active fuel cue
handle; it does not start another cue on every loop iteration. Repeat timing is
not yet mapped. Fly rejects current weight greater than maximum, preserving the
ability to correct the load. Equality passes this comparison.

## Catalog and station geometry

All coordinates are in the original 640×480 shell. LOADORD.DLG is only the bottom
control area; see [static dialog geometry](quick-mission.md).

| Element | Recovered contract |
| --- | --- |
| Catalog draw anchors | x=68/188, initial y=108, row step 68; eight cards |
| Catalog hit rectangle | x=64, y=103, width=235, height=272 (`0x41c4f0`) |
| Catalog hit index | `8*page + 2*((y-103)/68) + (x>184)`; reject index >= count |
| Catalog local pointer x | Subtract 70 in left column or 190 in right column |
| Station draw anchors | x=350/469, initial y=121, row step 71 |
| Station hit rectangle | Initialized at (296,115), size (235,213), then passed to `0x41c460` |
| Station hit index | Two per 71-pixel row; column split is passed rectangle midpoint; reject index >= station count |
| Station local pointer x | Subtract 354 left or 473 right |

The drop rectangle's x differs from the drawing anchors. A separate pickup
rectangle starts at (349,115), size (235,213), and is passed by `0x41bb24`. The wider-left rectangle is used on release/drop at
`0x41b368`. `MouseInBox` calls `0x412170`: left/top edges are inclusive and
right/bottom edges are exclusive. Preserve pickup and drop regions separately.

`0x41c610–0x41c6f6` is the shared card art/name helper. It uses an image area
107×21 at x+1,y, conditionally draws selection treatment, and draws the name at
y+25. The display-name choice measures the second source string against 111
pixels and falls back to the first when it is too wide. Original fonts and
thumbnail resources must be imported; a screenshot crop is not a replacement.

## Catalog ordering and eligibility

`0x41c700–0x41c81c` sorts category indices by the display name chosen with the
same 111-pixel fit rule. It compares source strings and swaps indices, rather
than reordering mutable station records. Names resolve through projectile +5
or other-store +1 pointers. Filename sorting would produce a different order.

The normal catalog loop at `0x419cfa` scans stations with HARDCanLoad. Projectile
category selection at `0x419ebb` uses flag `0x10000` from projectile +0xa6 for
bank one, otherwise bank two. Nonprojectile entries require bit 1 at resource +7
and go into bank two. Resource type and compatibility must be resolved before
using those offsets; they are not one generic store record layout.

The projectile year gate at `0x419df8–0x419eba` is conditional on Fly all and two
mode flags (`0x4fb1b8`, `0x4fb264`) being clear. Era cutoffs compare projectile
+0x37 to 1976, 1982 and 1996; the final era has no cutoff in this span. SMS identifies these as the first byte of `campaignFile` and
`freeFlightMission`: the cutoff applies outside a campaign and free flight. A universal year or nationality restriction
would not reproduce this branch.

With an airbase context, stock entries are 16 bytes starting at context +0x1c60;
the signed quantity is at +0x1c6e. Quantity -1 bypasses finite-stock clamping.
Normal catalog resource-name records use a separate 13-byte stride. Neither
layout is a PTS preset contract.

## Quantity, transfers and cheat

`0x41b4d2–0x41b685` transforms a requested signed delta, after the caller has
selected a store/station:

- Existing quantity `0x7fff` zeros the requested change.
- Capacity >300 scales the delta by 100; capacity >100 scales it by 10;
  capacity <=100 leaves it unchanged. Comparisons are strict.
- Clamp to capacity minus existing quantity, then to avoid negative quantity.
- Station-to-station transfer additionally clamps to available source quantity
  and debits it; selecting the same source/destination takes a separate path.
- Finite airbase stock clamps positive additions. Later branches refund replaced
  stores and debit stock; this is more than a station capacity check.
- A zero resulting quantity uses HARDUnLoad; nonzero uses HARDLoad with an
  explicit count. HARDLoad's count-zero auto-fill convention is not UI unload.

The input branch `0x41b955` starts a drag on shell-button bit 1. Catalog pickup
sets an initial delta of 100, which subsequently clamps to capacity; station
pickup starts a transfer with delta 1. The drag replaces the cursor with the
store thumbnail, retaining the cursor's original state. Release enters the
station drop resolver; a station drag released outside a valid drop uses a
large negative delta to remove the source load. Bit 2 on a station routes to
quantity decrement. The exact shell event/repeat cadence remains open.

Keyboard `+`, `=`, keypad plus and `-`/keypad minus route to the same signed
quantity-change path (`0x41aea7`). The strict capacity thresholds therefore
apply to keyboard changes too. Physical mouse naming needs the shell/MOUSERead
chain verified end to end before acceptance.

The Cheat menu branch at `0x41be33` first calls the unload-all helper, toggles
`0x4f6a04`, disconnects or restores the saved airbase context, and rebuilds the
catalog. It does not simply set one compatibility flag while preserving the load.

The capacity wrapper `0x41c3a9–0x41c456` normally calls HARDCanLoad. The cheat
branch still uses normal compatibility for fixed stations (bit 8), a matching
station default name, or participant count >1. Otherwise it returns station
capacity at +0x15, except a projectile without flag 2 is rejected when a
nonmatching station default name exists. Cheat does not authorize unsupported
projectile execution in the port.

## Entry and outstanding flow

`0x47fa50–0x47fa96` calls ArmPlane only when global `0x552820` is set; otherwise
it returns state 13 or 18 from the incoming flag. After calling ArmPlane it sets
that flag according to whether the returned state equals 18. The creator's
nonzero custom-load choice emits an extra mission directive at `0x430d40`.
The directive string at `0x4f3768` is `armplane`. The parser compares that exact
string at `0x481daf` and sets `_doArmPlane` (`0x552820`) at `0x481de1`. This
establishes the custom-load-to-ordnance link. Standard omits the directive;
mission initialization clears `_doArmPlane` at `0x4808b5`.

At `0x41c2ff`, action 5 returns state 18; other exit actions return 13. Fuel is
serialized as the integer screen quantity shifted left eight bits at `0x41c278`,
while initial screen fuel is shifted right eight at `0x41a0b4`. The station copy
uses 17 bytes per station. These serialization facts do not establish complete
cancel/restart semantics: the airbase context backup is copied back at exit.

SMS names `0x5528bc` as `_fortMission`. `0x419a6b` disables Cheat for participant
count >1, and `0x419a82` disables next/previous aircraft outside fort missions.
The campaign root's hide-versus-disable behavior remains an outer-menu question.

Remaining source work: mode/menu visibility, complete card text offsets and art
closure, full event/repeat timing, stock
rollback on cancel, accepted fuel/weight serialization, start/task generation,
and original-game visual/interaction acceptance. Use this contract alongside
[the implementation plan](../research/ordnance-plan.md), not as proof that its gates pass.

## Additional card recovery and implementation

Static FA evidence: hardpoint byte +0x17 indexes table 0x4ee7e8 (Centerline,
Fuselage, Internal Gun, Internal Bay, Wing, Wingtip). Station anchors are headings;
the shared card is offset +2,+14. Heading font is PANELFNT; normal card labels use
SMLFONT and selected labels FNTWPNY. Thumbnail naming derives `$<store stem>.PIC`.
Guidance branch 0x41a5ae..0x41a65d uses flag bit 1 for guided/unguided, then
signature 0 optical, 1 laser, 2 IR, 3 radar; radar flag 0x200 selects SARH,
otherwise active radar. These facts are separate from fitted border colors,
dial angle selection, menu presentation and event timing.

The app implements JT catalog cards, station compatibility, quantity/fuel controls,
weight validation and custom-load flight/restart. Auxiliary stores and native stock,
year, cheat and airbase lifecycle remain open. See
[implementation evidence and user testing gate](../baselines/creator-ordnance.md).
