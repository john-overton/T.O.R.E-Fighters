# Parity plan

One page. What is specified, what is built, what is next. Milestones live in
[ROADMAP.md](ROADMAP.md); how agents work is in [AGENTS.md](../AGENTS.md).

Parity is measured by **expression of feature**: does the player experience what
they experience in Fighters Anthology? A behaviour's provenance — `spec-derived`,
`native`, `fitted`, `opinionated` — records where it came from and never gates
acceptance.

## The decision this plan follows

**D27 — 2026-09-15. Parity by expression of feature.** Recorded from John's
direction after eighteen plan revisions were built on a misreading of "replicate
only native functionality":

1. Parity is 1:1 gameplay parity by expression of feature, not a recreation of
   the original program's code, control flow, caches or RNG ordering.
2. "Native" is a provenance label only. It is never a requirement or a gate.
3. Ground, terrain and object contact is reclassified as **opinionated** —
   authored behaviour, no longer waiting on a recovered native producer.
4. Research mode produces prose specs under [`spec/`](spec/). Implementation mode
   reads a spec and builds the behaviour idiomatically.
5. `spec-derived` is the default provenance for gameplay code. Fitted and
   opinionated components do not have to be replaced before acceptance.

> **ID conflict, needs John's ruling.** The frozen environment plan already uses
> D27 for "NE-00.1p scheduler/clock ownership", and its decision log runs to D29.
> This strategy is labelled D27 because that is what it was called when it was
> given, so two different decisions now share the ID. See
> [the realignment report](doc-realignment-2026-09-15.md).

## Specs

Behaviour specs live in [`spec/`](spec/). One file per feature a player would
name, with the numbers a player would notice.

| Spec | Covers | Status |
| --- | --- | --- |
| — | — | None written yet |

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
| Theaters | All 16 selectable, terrain renderer, free camera | native data, fitted rendering | [viewer](baselines/ukraine-viewer.md) |
| Weather | Day/night palettes, horizon, sun/moon/stars, cloud sheets, fog maps | mixed | [weather](baselines/weather.md), [review](baselines/weather-review.md) |
| F/A-18D and Rafale C free flight | Cockpit, HUD, instrument windows, mirrors, external views, animation rigs | fitted flight laws, native-derived components | [flight response](baselines/flight-response.md), [mirrors](baselines/mirrors.md) |
| Ground contact and landing | Runway contact, taxi, brakes, touchdown | **opinionated** — authored, not awaiting a recovered producer | [land foundation](baselines/native-land-foundation.md) |
| Input | Keyboard, gamepad, joystick, profiles, rumble, rebinding | opinionated (authored layer) | [input](baselines/input.md) |
| Weapons | 135 definitions imported; development range with manual firing, damage fixtures, ECM | mixed | [weapons systems](baselines/weapons-systems.md), [manual weapons](baselines/manual-weapons.md) |
| Combat AI | Not started, not authorized | — | — |

Flight has three selectable paths and they stay distinct: the legacy default, the
hybrid `--researched-flight`, and the restricted `--native-flight-tables`
research path. Do not change the default without being asked.

## Next

1. **Write the first behaviour specs.** Start with the features that already have
   the most recovered numbers and the least prose: weather, then flight
   envelope/departure, then the quick-mission and ordnance screens. Each spec
   replaces a pile of source notes with one page a player would recognise.
2. **Ground contact and landing as an authored feature.** Contact is opinionated
   now. Make takeoff, landing, taxi and deck behaviour feel right and hold up in
   tests; stop waiting on a recovered contact producer.
3. **Maneuver audio and rumble**, then the remaining
   [flight-response](research/flight-response-plan.md) items.
4. **F-14, A-4E and X-31** through the [aircraft import gates](aircraft-import.md).
5. **Remaining weather work**: wind, turbulence and vapor.

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
