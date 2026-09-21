# Dummy aircraft skill option

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-21. John requested a Dummy entry in the Quick
Mission Creator's AI skill selector, flying straight ahead at 400 knots instead
of the existing roughly 178-knot development fixtures. This is an opinionated
addition, not a recovered Fighters Anthology skill level.

All six wing skill selectors append `Dummy (400 KTS)` after Ace. It applies to
every AI member of that wing; the player's slot remains human controlled. Normal
Novice through Ace selections retain their behavior. Enemy-skill overrides do
not convert a Dummy wing into a combat wing. Restart rebuilds the selected mode.

Dummy aircraft hold their initial heading and altitude at 400 knots. Agent
choice: knots mean ground speed, matching the target window's speed display and
the existing constant-velocity fixture convention. The existing simulation uses 6,076 feet per nautical mile. The same 120 Hz fixed simulation step drives displacement and pause.
The mode bypasses aerodynamic flight, fuel consumption, tactical decisions,
weapon firing and evasion. It ignores wing commands and reports that rejection.
It keeps its selected aircraft identity, side, damage, collision target and
rendered geometry. Destruction hands the wreck to the existing combat path.

The mode is separate from the four experience levels, so it never indexes a
fifth entry into the recovered experience tables. The internal inactive
controller has a Novice placeholder which is not displayed as a pilot skill.
View 4 shows `DUMMY 400 KTS`, neutral goal N, and no skill dots. Existing explicit
`--fixture-wings` and `--dummy-aircraft` compatibility paths keep their behavior.

This training target does not avoid terrain or other aircraft. Terrain-following,
autopilot speed control and combat AI are outside this constant-motion mode.
