# Airports and attached ground objects

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation status, 2026-09-20. Base-layout placements, static scenes,
runway surfaces, individual targets and the deterministic landing service are
implemented. The HUD displays airport and runway names, range, and automatic
localizer/glide bars. The reviewed clear-to-land and welcome-home recordings are
connected to the player landing service. Campaign overlay application and
destroyed replacement art remain open.
[Input identities and measured extraction](../baselines/ukraine-airports.md).
[Placement contract](../formats/airport-placements.md).

## Placement and identity

Spec-derived target: each map displays its source airport runways and explicitly
placed surrounding objects at their authored positions and orientations. Airports
retain source names. Buildings remain individually identifiable objects. Ukraine's
base-layout acceptance inventory is 14 runways and 99 airport-section objects.
Those section associations are research metadata, not a recovered gameplay link.
The eventual every-map scope includes base layouts and separately resolved mission
and campaign variants, with missing dependencies reported explicitly.

## Ground targeting

The manual's ground-attack tutorial cycles ground targets until a tower is selected,
shows its type and image in the Target Window, and attacks it with an AGM-65.
Mission-goal examples separately identify runways, a control tower and barracks.
Therefore airport structures must be independently selectable and able to serve as
mission targets when their definitions and mission settings allow it.

Unknown: exact runway and building detection rules, damage thresholds, destroyed
appearance, whether runway damage disables landing, and effects of tower loss.
Next research: decode each OT's relevant fields, trace target eligibility and
weapon-damage consumers, then follow airport availability after destruction.
Do not treat destroying a tower as destroying or capturing the entire airport.

## Landing guidance

Manual-described behavior: navigation mode provides automatic ILS guidance near a
runway. The horizontal indicator corrects height and the vertical indicator corrects
lateral alignment. Above-path guidance moves down; right-of-path guidance moves
left. Centered indications show alignment. Airspeed brackets show the advised
landing range. Manual page 65 gives the player first landing clearance while other
aircraft hold at marshal. That is evidence to preserve for a later traffic feature,
not authorization here to implement autonomous traffic.

Both pages 67 and 87 give a 5 nautical mile activation distance. Page 67 gives an
altitude below 2,000 feet; page 87 gives below 4,000 feet and requires gear down
and navigation mode. The retail altitude threshold is **unknown**. John selected
4,000 feet above airport ground level on 2026-09-20, allowing room for larger
aircraft in future. This is an opinionated user-requested TORE rule, regardless
of which retail threshold is eventually confirmed. ILS becomes altitude-eligible
at or below 4,000 feet above the selected airport's ground elevation, not mean
sea level or terrain directly beneath the aircraft. Other entry gates still apply.
The retail altitude reference, distance metric, runway selection, glide slope angle,
indicator scaling, speed bracket numbers and boundary inclusivity are also unknown.
Next step: trace the ILS activation/display consumers in the identified FA.EXE,
using those two candidate altitude values and existing airport geometry as leads.
Do not generalize a tutorial's aircraft-specific approach speeds to every aircraft.

### ILS arming envelope

John requested a 90-degree forward cone and ILS-band restriction after the HUD
checkpoint `cfd35e5`. The interpretation is a full 90-degree cone, inclusive
of 45 degrees from the aircraft's body-forward direction. Compare the real
three-dimensional nose vector with the line of sight to the chosen runway
threshold. Head-look, bank about the nose axis and velocity/sideslip do not
change which way the nose points. Pitch does affect the cone. Reject missing,
zero-length or non-finite direction data.

The existing geometric band is horizontal range at most 5 nautical miles,
altitude at or below 4,000 feet above the airport's ground elevation, and the
approach side of the runway threshold. Outside this band or the forward cone,
return no ILS guidance: neither `ILS ARM` nor active ILS indicators appear.
Inside it, existing NAV, gear-down and alive gates still control active guidance.
An eligible NAV approach with gear up may show `ILS ARM` while awaiting gear.
The 2.5-degree localizer and 0.7-degree glide values remain display scaling,
not newly invented capture limits.

Apply the same eligibility to explicit selections, cleared approaches and
automatic discovery. Explicit selection/clearance persists while out of the
cone or band and can regain guidance on re-entry. Automatic discovery filters
ineligible airports before choosing the nearest valid candidate, so a closer
airport behind the aircraft cannot hide one in front. Tower requests and
clearance behavior are unchanged. These are requested gameplay rules with
fitted geometry, not assertions about retail or real-world ILS receivers.

## Commands and ownership

John clarified on 2026-09-20 that commands means landing/tower radio commands.
Unknown: supported player radio requests, responses and availability conditions. The reviewed STRIP definition
selects airport initialization, shared events and speech, and airport comments;
callback names alone do not establish a player command menu.
Next step: identify player input/menu producers and their visible responses.
Capture or transfer of airport ownership is not established by nationality fields.
Mission orders and ownership changes are outside this requested command scope.

## Integration choices

Proposed agent choice: use typed world instances, stable source identities,
render meshes and independent targeting/contact state within TORE's existing
120 Hz simulation. Reuse existing terrain grounding and reviewed runway anchors;
use the planned fitted defaults below where retail evidence is incomplete.
Imported callbacks remain inert. Existing flight adapters stay distinct.
There is no requirement to reproduce the original scheduler or global state.
[Staged implementation plan](../ROADMAP.md#airport-and-ground-object-expansion).

## Planned TORE defaults where retail is unresolved

Except for John's explicitly identified ILS altitude choice, these are agent-selected
proposals for implementation, not claims about retail or choices attributed to John. They make the plan executable without requiring full
reverse engineering. Later measured evidence can replace a fitted rule locally.

| Component | Planned rule | Provenance |
| --- | --- | --- |
| Building association | Explicit mission relation first; otherwise source airport comment groups as diagnostic/import metadata. Never propagate damage or ownership through a proximity guess. Preserve unassigned placements. | Opinionated host data model |
| Runway grouping | Keep every placed runway identity, including A types. UI may group confirmed co-located facilities only after layout review; selection and damage retain individual IDs. | Opinionated |
| Grounding | Use TORE terrain height at a zero-Y instance origin and preserve authored heading; apply supported source pitch/bank. One rigid transform for rendering, anchors and contact. | Fitted host placement |
| Ground wind grip | Imported maximum-takeoff-weight crosswind thresholds, universal 10-knot tailwind limit and fitted rollout coupling; stationary tire grip is retained. Headwind has no ground-rule penalty. | [Runway-wind specification](runway-wind.md), opinionated user request 2026-09-21 |
| Runway support | Use reviewed runway extents and a plane through the grounded origin. Inside the surface footprint, aircraft contact and the visual runway use that plane. Terrain remains unchanged outside it. No automatic large-area flattening. | Fitted contact |
| Unresolved collision shapes | Use an oriented bounding box of the reviewed scaled mesh for buildings, never one huge sphere for a runway. Contact and blast rules are distinct. | Fitted collision |
| ILS entry | NAV mode, gear down, selected usable runway, horizontal threshold distance at most 5 nautical miles, altitude at or below 4,000 ft above the selected airport's ground elevation, and threshold inside the aircraft's full 90-degree forward cone. No armed indication outside that geometric envelope. Use 6,076.12 ft per nautical mile. | User-requested opinionated altitude/reference, 2026-09-20; agent-selected inclusive boundary; manual-derived NAV/gear/range gates; fitted distance metric and conversion |
| ILS path | Fitted primary centerline through source contact anchor 0x11, extending toward the projected shape's forward bound, with an opposite approach end. Fall back to mesh centerline only if that anchor is absent/outside the longitudinal footprint. A 3-degree straight approach to the selected threshold. Localizer displacement is lateral error divided by forward approach distance; glide error is elevation angle minus 3 degrees. Suppress behind-threshold guidance and guard zero distance. | Fitted geometry |
| ILS display | Full-scale lateral indication at 2.5 degrees and vertical at 0.7 degrees error, clamped to the existing HUD area; centered at zero. | Fitted presentation |
| Runway selection | Explicit user selection persists. Otherwise choose the nearest usable threshold eligible for the ILS band and forward cone, breaking equal distances by stable ID; choose the approach end nearest ownship. No wind-based automatic switching during final. | Opinionated selection |
| Tower command set | Select airport, request approach/landing, repeat last reply, cancel approach. A reply identifies the airport and selected runway/end. | Opinionated player interface pending recovered menu evidence |
| Tower availability | The current base-layout free-flight host assigns airports neutral status with explicit landing permission because it has no mission player-side assignment. The service can also reject hostile, unknown or unpermitted neutral airports when a mission supplies those states. Disabled runways decline. | Opinionated base-layout policy, agent choice 2026-09-20; fitted mission service policy |
| Clearance lifetime | Stays with the selected runway until cancellation, airport selection change, runway disablement, flight reset or landing completion. Repeating a request repeats status rather than allocating another clearance. | Opinionated |
| Landing completion | Existing flight state reports supported, alive, on-runway contact and speed below 30 knots for 240 consecutive 120 Hz ticks. Taxi remains manual. | Fitted service completion, not flight damage criteria |
| Radio output | Typed response and subtitle immediately at a simulation tick. A successful landing request and repeat use reviewed `^CLRLAND`. Deterministic landing completion and repeating its latest reply use reviewed `^WELHOME`. These event bindings are fitted because retail player-menu producers remain unresolved. Selection, cancellation, rejection and invalidation stay text only. Missing or old caches preserve text operation and report that a retail reimport is needed for optional airport audio. | Reviewed phrase/sample identity with fitted event binding |
| Runway damage | At zero imported hit points disable new clearance and ILS; preserve its surface for physical contact. Individual tower/building loss does not disable other runway services in this first host policy. | Fitted service consequence |
| Missing destruction artwork | Remove the intact mesh when combat HP reaches zero, retain target/mission identity, and use the existing impact effect. Do not infer an A-suffix replacement. | Fitted visual fallback |

Static geometry applies the reviewed SH CODE header scale `2^(word6-8)` to
visible and contact geometry. `RUNWAY.SH` uses scale 4, putting its visible
longitudinal span near -2748 through 3252 feet. That agrees with reviewed STRIP
anchors near -2512 through 3090 feet and avoids an invisible support footprint.

Runway support and the ILS datum use the authored airport ground elevation.
A dedicated static-surface rendering depth bias avoids terrain overlap without
changing that elevation. Textured coplanar detail faces use a separate depth-biased
render pass, without a separate physical face lift. For composite runway shapes,
the horizontal layer with the greatest aggregate polygon area defines pavement.
The mesh is translated vertically so that layer meets the runway surface, with
all relative geometry retained. For example, RNWY1's source paving at -4 feet
receives a fitted +4-foot mesh offset. This aligns pavement and wheel contact
without changing the airport-ground ILS datum. Building collision remains a separate
solid OBB query and never raises terrain to roof height. The restricted native
research adapter continues to reject unsupported ground contact.

Ground-target aim points use the upper half of the OBB while projectile and
aircraft contact retain the complete box. This fitted point keeps a flat runway
target above its own terrain occlusion without changing collision geometry.

Airport commands and NAV/gear/support state are queued at fixed-tick boundaries
and included in combat tape version 6. Versions 2 through 5 remain accepted.
Replay reconstructs selection, clearance, guidance gates and synchronized
runway damage from the same imported scene. Version 6 distinguishes a scene reset
from an explicit range-fixture reset; older tapes retain their range-only behavior.

Aircraft-specific ILS speed brackets and target-relative camera imagery remain
open. The target window currently shows object name, health and lock state.
Ground radar/infrared signatures and damage category come from the OT fields;
the existing host aspect/visibility model remains fitted. Unknown shape programs
stay manifest-only with a diagnostic, without invisible target/collision proxies.
Static geometry has a 32 MiB host input budget; oversized scenes fail explicitly.
Original PIC sheets larger than 256 pixels on either axis are nearest-sampled
into a 256-square GPU layer, with UVs adjusted to retain the entire artwork.
This is a fitted resolution reduction for the current shared texture-array path;
small sheets retain original texels, and extracted media remains unchanged.

The runway-plane rule requires visual inspection for burial or terrain protrusion.
If a site fails, record a per-shape/per-site fitted correction or implement a
bounded runway-footprint terrain cutout; do not silently move the airport.
Mesh scale comes from reviewed SH transform/header consumers, not the aircraft
renderer’s one-third-foot convention. If unresolved, record a measured per-type
scale before accepting that type. No universal guessed scale is approved here.

Landing speed brackets require aircraft-specific performance values. First use
reviewed PT/flight-model limits if their landing meaning is established. Otherwise
plan a fitted lower/upper bracket of 1.15/1.35 times the selected aircraft's current
landing-configuration stall speed. If that stall-speed input is unavailable, omit
brackets with an explicit diagnostic until it is provided; do not invent a shared
speed for all aircraft. This does not change aircraft physics.

Health and sensor configuration must come from reviewed OT fields. Visual loading
can proceed without them; interactive completion cannot silently substitute an
aircraft's configuration. An unresolved field gets an individually documented
fitted value and synthetic acceptance case during its implementation slice.
Tower commands provide guidance and responses only. They do not take control of
the player's aircraft, add autonomous airport traffic or guarantee a safe landing.
Airport speech is serial and has its own cancellation ownership. A new airport
reply replaces stale queued airport speech without removing wing radio. Cancel,
selection change, runway invalidation and flight reset remove stale airport
speech. Pause freezes it, and muting effects clears it with the other effects.
After landing, repeat replays the welcome reply, not an obsolete clearance.
Verified phrase/sample identity and airport-consumer evidence are recorded in
[radio metadata](../formats/radio.md).
