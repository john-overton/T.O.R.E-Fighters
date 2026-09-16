> **Frozen as of 2026-09-15. Superseded by the parity strategy (D30) — see [AGENTS.md](../../AGENTS.md) and [the parity plan](../parity-plan.md).** Kept for its Load Ordnance screen description, source research and dated checkpoints. Its sequencing, gates and status columns are no longer authoritative.

# Load Ordnance implementation plan

> **T.O.R.E — we trace what the player does, not what the code did.**
> This project reverse-engineers *player interaction*: what you press, see, hear
> and feel in Fighters Anthology, and the numbers behind it. It does not
> reproduce the original program byte by byte. Anything here about the original
> executable is evidence toward a behaviour spec — never a specification for what
> we build. If a sentence below reads like an instruction to reproduce the
> original's internals, it is out of date.
> <!-- tore-header v1 -->

Scope added 2026-09-14 to the [Quick Mission pass](quick-mission-plan.md).
Build the retail screen and connect supported loadouts for F/A-18D and Rafale C.
The existing weapons service supplies a useful foundation; full catalog projectile
behavior is not implied by the presence of a weapon definition or thumbnail.

## Screen specification

Use the user's [640×480 retail reference](https://www.old-games.com/screenshot/6252-5-jane-s-fighters-anthology.jpg)
and local `gameassets/reference-photos/load-ordnance-screen.jpeg`. They show F-22A
and F-14D respectively; those aircraft are visual references, not newly authorized
flyable models. Source displayed aircraft/station values from F18.PT or RAFALE.PT.

| Area | Required presentation and behavior |
| --- | --- |
| Shell | Original Load Ordnance art/palette, ?, Weapons and Airbase menu roots |
| Catalog | Two columns × four cards, original weapon art, name, weight and guidance text |
| Catalog controls | Air-to-air / air-to-surface dial, previous/next rocker, page count |
| Aircraft panel | Aircraft name; source station groups with location, store art/name, count and capacity; empty slots |
| Selection | Original selected text/card treatment; recover click/load/unload/count gestures |
| Weight | MAX, CURRENT, AVAIL; signed remaining weight and source overweight response |
| Fuel | Internal fuel mass/percentage, plus/minus rocker; recover increments and bounds |
| Navigation | Select Plane, Fly, source menu/back behavior and standard/custom creator branching |

A screenshot establishes appearance, not drag-and-drop, right-click, repeat rate,
catalog eligibility or default loading semantics. Recover those from handlers.
The catalog includes foreign weapons in both references; nationality filtering
must follow native rules rather than assumptions about real-world compatibility.

## A. Complete native screen and loading research

Extend repeatable hash-gated static extraction around `ArmPlane` at `0x4197d0`,
its menu/data dependencies and `HARDLoad` at `0x452c20`. Existing translations of
`HARDCanLoad`/`StoreWeight` are reusable components. Record call arguments, record
layout, station ordering/grouping, catalog sort/category/page rules, selection
gestures, stock/year/airbase restrictions, internal-gun treatment, counts, fuel
arithmetic, weight checks, initialization and accepted/canceled output state.

Resolve original card art, background, fonts, dial/rocker pieces and menu nodes
through the importer. Do not assume PTS is a loadout preset format. The reviewed
PTS modules contain data/icon references with unresolved dependencies.
Preserve exact aircraft identities and missing-resource evidence.

**Gate:** each control and rule has source evidence or an explicit unresolved
status. A station-capacity result alone is not mission/stock/year eligibility.

First implementation pass: shared MNU decoding confirms Unload All and Cheat
(load anything anywhere), Airbase next/previous aircraft, and a Campaign root
whose quick-mission visibility remains unresolved. Recover the native cheat's
scope and keep normal compatibility distinct from that explicit option; its
presence does not make unsupported projectile execution available. See
[menu contract evidence](../baselines/menu-contract-pass.md).

## B. Build a shared typed loadout model

- Represent station ID, selected store ID and quantity, internal gun ammunition,
  internal fuel and supported external equipment/tanks. Separate source station
  limits from mutable loaded state and catalog display data.
- Resolve compatibility using reviewed native masks/default exceptions and
  capacity/weight arithmetic. Validate nonnegative user quantities, membership,
  maximum counts, fuel bounds and total weight before accepting changes.
- Centralize mass accounting so the screen, flight model, weapons instruments
  and post-release state agree. Track ammunition, store bodies, empty tanks and
  fuel without double counting. Native group counts are not necessarily pylons.
- Keep edits transactional; cancel/back preserves the prior accepted state.
  Revalidate on aircraft change; never transfer Hornet station indices or model
  tuning into Rafale. No silent substitutions or zero defaults for missing data.
- Separate catalog visibility, aircraft compatibility and runtime weapon support.
  Show recovered options; unsupported execution has concise availability feedback
  and cannot silently launch as a different store or disappear from the aircraft.

## C. Implement the retail screen

Add an app screen/state module and bounded format readers. Import all artwork at
runtime via the shared CLI/app resolver and update the cache contract. Render at
the original menu canvas using the screen palette and recovered font strips.
Provide all category pages, source station cards, fuel/weight panels and navigation.
Use original weapon images; do not substitute generic silhouettes or photo crops.

Implement verified click/count behavior, keyboard traversal, nested Escape,
matching press/release and focus-loss cancellation. Hover/focus stay silent;
review original armament cues for actual load/ammunition/fuel changes. Preserve
source menus, with unavailable airbase operations as explicit placeholders.
Fuel controls need bounded repeat behavior if holding is supported natively.

## D. Connect loadout to flight and combat

The current `Combat::new` constructs `live::Configuration::from_source` from PT
defaults, and the range flag also controls loaded state/fixture behavior. Replace
that coupling with an explicit accepted loadout and independent range-fixture
selection. A creator mission can carry weapons without creating a practice target.

Resolve configuration before launch, using each aircraft model's own typed
configuration boundary. Pass selected quantities/fuel into mutable initial state;
rebuild store geometry and instruments from that same accepted loadout. Preserve
direct clean free flight and the explicit manual range workflows.

Wire the already supported gun/missile execution first, with legal alternate
placements/counts only after compatibility and runtime checks. Keep unsupported
bombs, rockets, special guidance branches and auxiliary-tank transfer/jettison
explicit until implemented; merely showing a catalog card does not enable them.
Connect supported internal-fuel edits and loaded mass to flight without retuning
either aircraft's laws. Document remaining store drag/visual pairing limitations.

Fly, restart, return to setup and replay must carry the same accepted loadout.
Extend combat tape configuration/versioning if needed; replay must reconstruct
custom stores rather than silently restoring PT defaults. Preserve held-fire
cancellation, fixed ticks, pause without catch-up, damage reset and cue ownership.

## E. Acceptance

- Synthetic reader and state tests: malformed records, unknown IDs, illegal
  compatibility/counts, fixed gun stations, zero/full fuel, weight boundaries,
  transaction cancel, aircraft switching, category/page edges and release isolation.
- Source-backed F18/Rafale cases: default and supported edited loads, exact
  quantities/fuel/weight, launch/restart parity, ammo debit, mass after release,
  damage behavior and deterministic replay. Keep existing combat smoke cases.
- Retail visual comparisons: both categories, every catalog page, selected/empty
  stations, gun counts, fuel edits, overweight message and menu placeholders.
  Capture wide/tall windows for menu letterboxing and pointer alignment.
- Run locked formatting/Clippy/tests/build, Python tests and asset guards;
  creator, ordnance, viewer and both flight GPU smokes; both aircraft flight
  validation suites after load/mass integration. Record platform limitations.
- Update progress, README/setup docs, menu/weapon coverage and acceptance evidence.

Delivery gate: creator → supported Load Ordnance edits → Fly → correct armed
airborne state → restart/return without losing the accepted setup. Unrelated
menus, combat AI and complete native mission/projectile parity stay open.

## Mapping checkpoint — 2026-09-14

See the [controls, eligibility and geometry contract](../formats/ordnance-menu.md) and
[validation evidence](../baselines/menu-behavior-mapping.md). Verified source rules
are recorded separately from unresolved behavior. Implementation and original-game
acceptance gates above remain open.

Track remaining source questions in the [mapping ledger](menu-parity-matrix.md).

## Implementation checkpoint — 2026-09-14

Bounded imports, editable briefing, original-art ordnance and supported armed
airborne launch/restart are implemented. [Validation and remaining gates](../baselines/creator-ordnance.md).
Full original-game parity and custom-load replay remain open; the checklist above
is the full target, not a claim that every acceptance gate passed.
