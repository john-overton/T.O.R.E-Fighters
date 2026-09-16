# Stall warnings and default flight mode

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Player behavior

John requested researched flight as the default on 2026-09-16. Normal menu,
free-flight and headless launches now select the hybrid model. Explicit
`--researched-flight` remains supported. `--legacy-flight` selects the old
compatibility adapter. Restricted `--native-flight-tables DIR` remains distinct
and cannot combine with either explicit adapter flag. The selected model persists
across aircraft changes and new flights in the running process.

Use one departure alert for HUD and audio. Hybrid and restricted-table modes
report their actual warning, extended-warning, stalled or spinning state.
Legacy reports a fitted warning below the aircraft's clean 1G envelope speed at
current altitude; this does not add spin physics to that compatibility model.
Suppress alerts after a crash or while at/below the model's ground clearance.
Engine shutdown does not suppress an airborne stall warning.

Display steady `STALL` at HUD logical position (301,274) using the existing
font and HUD color. Warning and extended-warning loop source `&STALLWR.5K`;
stalled/spinning loop source `&STALL.5K` at mixer gain 0.4. Play one dedicated
warning voice, not a queue entry each frame. Change or stop it with the alert;
clear it on aircraft change, restart or leaving flight. Effects mute and flight
pause silence the voice; pause preserves its playback position.

The presentation mapping, looping, gain and HUD placement are fitted agent
choices. Imported PCM bytes are original assets; their exact original-game
scheduling remains unknown. No real-aircraft recovery behavior is asserted.
The existing A-4 recovery-lock limitation remains unchanged.

`--maneuver spin` provides a reproducible test setup: 180 ft/s, engine off,
full aft stick and full positive rudder. Use altitude and speed telemetry to
interpret the result. It is a test setup, not a new autonomous pilot.
