> **Frozen as of 2026-09-15. Superseded by the parity strategy (D27) — see [AGENTS.md](../../AGENTS.md) and [the parity plan](../parity-plan.md).** Kept as a record of menu source-recovery status; it measured coverage by source evidence, not by what a player experiences. Its sequencing, gates and status columns are no longer authoritative.

# Creator / ordnance mapping ledger

This ledger covers the mapping scope in the two implementation plans. **Mapped**
means static source evidence exists, not that the app implements it or that a
retail interaction comparison passed. **Partial** names the remaining source
question. Every planned control group has an entry; full 1:1 recovery is open.

| Planned group | Mapping status | Source / next evidence |
| --- | --- | --- |
| Both nationalities | Mapped | 60-entry active list; 16 theater defaults |
| Six wings: count, skill, aircraft | Partial | Count/skill tables, defaults, player minimum and masks mapped; dynamic aircraft flag construction/catalog closure open |
| Theater and ground target options | Mapped | 16 active theater tables, target reset and nationality dependencies |
| Altitude, conditions, situation, separation | Partial | Active values/defaults mapped; downstream environment/placement units and effects need acceptance |
| Start condition / BARCAP and other tasks | Partial | No standalone SP selector exists; mission-generator/task placement remains open |
| Standard/custom and guns restrictions | Partial | Values/defaults and custom → armplane directive → screen flag mapped; default store assignment and restriction propagation open |
| AAA / SAM | Partial | Four levels and no-target reset mapped; generated defenses remain open |
| Aircraft menu | Partial | Era/Fly all IDs/masks/checkmarks mapped; final catalog identities and filter-change retention open |
| Creator OK/Cancel and list dialogs | Partial | Active popup and accept/cancel mapping, static positions, text-line geometry mapped; final glyph hit extents, complete launch/back flow open |
| Ordnance actions/categories/pages | Partial | Five action IDs, eight cards, independent category pages mapped; dynamic dial label/art pairing and repeat timing open |
| Catalog/station cards | Partial | Art/name anchors, fit/sort and separate pickup/drop grids mapped; complete text/art/palette closure open |
| Store eligibility | Partial | Native capacity helper, cheat exceptions, year/stock gates mapped; complete catalog/resource initialization open |
| Load/unload/count/transfer | Partial | Quantity scaling/clamping, stock debit/refund, drag and keyboard branches mapped; event/repeat/cancel sequence acceptance open |
| Fuel and mass | Partial | 500-unit edit/clamp, fixed-point serialization and overweight gate mapped; full mass accounting and flight initialization open |
| Ordnance menus | Partial | Menu tree, Cheat and fort-only aircraft availability mapped; campaign-root visibility open |
| Fly / Select Plane / return | Partial | Action 5 → state 18, other exits →13; custom directive link mapped; outer state-machine and cancel/retry ownership open |
| Runtime import and implementation | Partial | Shared bounded active tables, app cache, typed drafts, both screens and supported armed launch/restart implemented; filters, auxiliary stores and custom replay open |
| Original-game acceptance | Open | Supplied screenshots constrain appearance; popup, gestures, restart and cross-platform comparisons still required |

Detailed contracts and source addresses:
[creator](../formats/quick-mission.md), [ordnance](../formats/ordnance-menu.md),
[validation](../baselines/menu-behavior-mapping.md).

Do not fill unresolved entries with the custom reference app's behavior. The next
acceptance gate is hands-on creator/ordnance/flight testing and original-game
comparison; source gaps above remain explicit. See [implementation evidence](../baselines/creator-ordnance.md).
