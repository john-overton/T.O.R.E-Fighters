# Parity plan

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

One page. What is specified, what is built, what is next. Milestones live in
[ROADMAP.md](ROADMAP.md); how agents work is in [AGENTS.md](../AGENTS.md).

Parity is measured by **expression of feature**: does the player experience what
they experience in Fighters Anthology? A behaviour's provenance, `spec-derived`,
`native`, `fitted`, `opinionated`, records where it came from and never gates
acceptance.

Heading/altitude autopilot and toggleable waypoint guidance are implemented from
the requested USNF-ATF behavior. Waypoint mode currently falls back to heading
hold; route selection is pending. See [scope and provenance](spec/autopilot.md).

## The decision this plan follows

**D30, 2026-09-15. Parity by expression of feature.** Recorded from John's
direction after eighteen plan revisions were built on a misreading of "replicate
only native functionality":

1. Parity is 1:1 gameplay parity by expression of feature, not a recreation of
   the original program's code, control flow, caches or RNG ordering.
2. "Native" is a provenance label only. It is never a requirement or a gate.
3. Ground, terrain and object contact is reclassified as **opinionated**,
   authored behaviour, no longer waiting on a recovered native producer. In
   practice this means both things: the ground contact that exists today is
   accepted as shipped, and contact is now a feature to design deliberately
   rather than a gap to be filled by recovery.
4. Research mode produces prose specs under [`spec/`](spec/). Implementation mode
   reads a spec and builds the behaviour idiomatically.
5. `spec-derived` is the default provenance for gameplay code. Fitted and
   opinionated components do not have to be replaced before acceptance.

> John first referred to this as D27. That ID was already taken by "NE-00.1p
> scheduler/clock ownership" in the frozen environment plan, whose log ends at
> D29, so the strategy was renumbered **D30** on 2026-09-16, the next free ID,
> which leaves every existing link and reference intact. See
> [the realignment report](doc-realignment-2026-09-15.md).

## Specs

Behaviour specs live in [`spec/`](spec/). One file per feature a player would
name, with the numbers a player would notice.

| Spec | Covers | Status |
| --- | --- | --- |
| [Missiles](spec/missiles.md) | Four guidance types, 63-candidate inventory, pitbull, range, motor and guidance lifetime | Implemented for A2A stores and controlled surface-seeker fixtures. Surface designation remains deferred; [missile validation](baselines/missiles.md), [HUD cleanup and tone limits](baselines/hud-cleanup.md) |
| [Aircraft radar](spec/radar.md) | Twelve-aircraft radar stats, automatic range modes, installed visual and ECM records, and look-down evidence | Implemented as one shared component; [component guide](radar.md), [validation](baselines/radar.md) |
| [Roster expansion](spec/roster-aircraft.md) | Seven REDFOR/F-22A initial player ports | [Acceptance and limits](baselines/aircraft-roster-expansion.md) |
| [Additional aircraft](spec/additional-aircraft.md) | F-14D, A-4E and X-31 source flight configuration and fitted presentation | Initial ports implemented; [acceptance](baselines/aircraft-fa-expansion.md) |
| [AI experience](spec/ai-experience.md) | Experience channels, initial tactical thresholds and aircraft/surface family scope | Partial research specification; runtime implementation pending |
| [AI behavior](spec/ai.md) | Fighter decisions, family differences, required observations and proposed API boundaries | Timing, pursuit, steering consumers, launch gates, ammunition and wing receivers researched; remaining contracts and runtime implementation pending |
| [Ocean](spec/ocean.md) | Short ripples, close pixelation and distance filtering; original textures/colors | Implemented; [acceptance](baselines/ocean.md) |
| [Terrain shorelines](spec/terrain-shorelines.md) | Beach/water coverage and absence of land-color strips | Implemented; validation in the viewer baseline |

The research to build the first specs from already exists: recovered numbers are
in [`formats/`](formats/) (weather, native flight, quick mission, ordnance menu,
theater, weapons, aircraft) and measured results are in
[`baselines/`](baselines/). Writing a spec is mostly a matter of lifting the
player-visible numbers out of those files and leaving the byte layouts behind.

## What is built

| Area | State | Provenance | Evidence |
| --- | --- | --- | --- |
| Main menu and Choose Activity | Original art, fonts, sounds, five backgrounds | native assets, spec-derived layout | [main-menu](baselines/main-menu.md) |
| Quick Mission creator | Briefing screen, aircraft and theater selection, editable fields | mixed | [creator/ordnance](baselines/creator-ordnance.md) |
| Load Ordnance screen | Original art, compatible weapon and fuel edits | mixed | [creator/ordnance](baselines/creator-ordnance.md) |
| Theaters | All 16 selectable, terrain renderer, free camera; shoreline water cutouts corrected | native data, fitted rendering | [viewer](baselines/ukraine-viewer.md) |
| Weather | Day/night palettes, horizon, sun/moon/stars, cloud sheets, fog maps | mixed | [weather](baselines/weather.md), [review](baselines/weather-review.md) |
| Twelve aircraft in free flight | Source cockpits, HUD, instrument windows, external views, initial device rigs; new control-surface schedules remain open | fitted flight laws, native-derived components | [flight response](baselines/flight-response.md), [additional FA aircraft](baselines/aircraft-fa-expansion.md), [roster](baselines/aircraft-roster-expansion.md) |
| Ground contact and landing | Runway contact, taxi, brakes, touchdown | **opinionated**, authored, not awaiting a recovered producer | [land foundation](baselines/native-land-foundation.md) |
| Input | Keyboard, gamepad, joystick, profiles, rumble, rebinding | opinionated (authored layer) | [input](baselines/input.md) |
| Weapons | 135 definitions imported; development range with manual firing, damage fixtures, ECM | mixed | [weapons systems](baselines/weapons-systems.md), [manual weapons](baselines/manual-weapons.md) |
| Sensors | One shared radar, infrared and visual component for all twelve aircraft: imported capability profiles, contacts, one fire-control track, click selection, history, jammer noise, the RCS exposure page and radar weapon support | **opinionated** detection/notch/jamming/RCS tuning over spec-derived equipment data | [radar](baselines/radar.md) |
| Combat AI | Research and planning requested 2026-09-17; initial FA trace and family inventory recorded, runtime implementation and hookup pending | researched evidence, remaining rules unknown | [AI research](baselines/ai-research.md), [delivery stages](ROADMAP.md#1e-ai) |

Flight has three selectable paths and they stay distinct: the compatibility `--legacy-flight`, the
default hybrid `--researched-flight`, and the restricted `--native-flight-tables`
research path. Do not change the default without being asked.

## Next

Current requested work: **aircraft and surface AI research and planning**.
The [main AI spec](spec/ai.md), [experience spec](spec/ai-experience.md) and
[FA source map](formats/ai.md) are written. Research now reaches the Quick Mission
skill writer, maneuver clocks, pursuit regulation, target ranking, weapon-service
delays, steering consumers, seeker/launch gates, ammunition and wing receivers.
Next trace skill loading, performance/signature producers, in-flight support
transitions and remaining wing orders, then build isolated
components before live hookup. The [M1e stages](ROADMAP.md#1e-ai) include all
aircraft families and a separate surface workstream. Initial implementation and
acceptance cover [all twelve ported aircraft](spec/ai-experience.md#currently-ported-aircraft)
at all four experience levels. Earlier no-AI restrictions
on the weapon/radar slices below do not prohibit this newly requested work.

1. **Missile tuning and remaining evidence.** Stages 1 through 5 shipped for
   current stores, with controlled emitter fixtures. Four guidance types, silent
   active flight, same-target reacquisition through guidance expiry, inherited
   velocity, BORESIGHT, HUD and fitted tone are implemented. The
   [baseline](baselines/missiles.md) records 3,920 reach cases and the remaining
   fitted tuning, unavailable human review and original evidence gaps. No new
   catalog stores or combat AI were added.
2. **Finish the shared aircraft radar tuning pass.** The component John
   requested on 2026-09-16 shipped on the same day: shared profiles and contact
   state using PT signatures with authored look-down, notch and jamming, the
   RCS/aspect display, persistent click selection, Y history and infrared A2A,
   one fire-control track only, and detectable destroyed aircraft. Target-view
   IFF is unchanged and A2G is still deferred. Stages 1 to 4 of the
   [component guide](radar.md#delivery-and-acceptance) are done. Stage 5 is
   partly done: all twelve aircraft produce the capability summary and pass their
   combat smokes, but no side-by-side tuning review of the twelve has happened,
   so the presets and matchups have not been played against each other yet.
   [Capability spec](spec/radar.md), [validation](baselines/radar.md). No AI
   work.
3. **Continue behaviour specs.** Start with the features that already have
   the most recovered numbers and the least prose: weather, then flight
   envelope/departure, then the quick-mission and ordnance screens. Each spec
   replaces a pile of source notes with one page a player would recognise.
4. **Ground contact and landing as an authored feature.** Contact is opinionated
   now. Make takeoff, landing, taxi and deck behaviour feel right and hold up in
   tests; stop waiting on a recovered contact producer.
5. **Maneuver audio and rumble**, then the remaining
   [flight-response](research/flight-response-plan.md) items.
6. **Continue F-14D, A-4E and X-31 acceptance.** Initial FA-only ports are implemented,
   requested by John on 2026-09-16. Resolve the [documented limits](baselines/aircraft-fa-expansion.md),
   especially original X-31 vector animation schedules and remaining pitch/yaw response
   tuning. PT roll response and low-speed auxiliary control are implemented.
7. **Remaining weather work**: wind, turbulence and vapor.

## Frozen

These were plans. They are now archives under [`research/`](research/), kept for
their recovered facts, dated checkpoints and evidence links. Their gates, status
columns and sequencing are not authoritative.

- [native environment and systems plan](research/native-environment-systems-plan.md)
- [progress log](research/progress.md)
- [weather plan](research/weather-plan.md)
- [flight response plan](research/flight-response-plan.md)
- [ordnance plan](research/ordnance-plan.md)
- [quick mission plan](research/quick-mission-plan.md)
- [menu parity matrix](research/menu-parity-matrix.md)
- [weapons implementation plan](research/weapons-plan.md)
