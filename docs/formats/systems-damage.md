# Reviewed ownship damage events

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Build and bounded evidence

Static evidence: reviewed FA.EXE SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
The bounded dispatch at 0x40fe34..0x4105fb, its 44-entry table at 0x410600,
and message-pointer table at 0x4eca60 identify these events. Message identity
is corroborated by state writes in each dispatch branch, not names alone.
The update at 0x410a18 tests throttle strictly above 25% for compressor risk.
No original module was executed. Rates in the linked behavior spec are fitted,
not reconstructed original control flow. Existing hit selection,
repeat limits and equipment-specific faults remain in use.

| Source index | Behavior |
| --- | --- |
| 0 | No distinct subsystem effect in the reviewed switch; structural hit points still apply. |
| 1-3 | Internal fuel leak, severed fuel lines, fuel fire. |
| 4-10 | Restartable flameout, engine damage, serious engine damage, compressor damage, disabled afterburner, partial/total engine shutdown. |
| 11-15 | Engine fire, oil pump damage, oil leak, hydraulic leak, hydraulic fire. |
| 16-18 | Jammed gear, flaps, airbrake, retaining their current positions. |
| 19-24 | Reduced or bent elevator, aileron, rudder. |
| 25-30 | Wing damage/destruction, damaged/unstable controls, stuck throttle, structural weakness. |
| 31-33 | Flight sensor failure, instrument display failure, navigation failure. The index-32 dispatch selects radar/RWR display flags, despite its generic message label. |
| 34-35 | Pilot wound, imminent explosion. |
| 36-44 | Actual hardpoint equipment failures: weapons, sensors, ECM/dispensers or external tanks. Never substitute another equipment identity. |

## Scope

This is an event identity map, not a requirement to reproduce original dispatch,
RNG ordering or timer machinery. The message pointers corroborate effect writes;
the manual establishes the visible consequences. Timing, probability and several
branches remain unknown. [Implementation contract](../spec/systems-damage.md)
records fitted choices. Equipment records, not names or aircraft variants, resolve
hardpoint effects. The source's `.GAS` fuel field supplies external capacity in
pounds and remains separate from empty tank and other equipment weight.

The earlier `.local/esa-probe/FA.EXE` is a different build and was rejected for
this pass after checking its hash. Only the reviewed gameassets build above was
used for the event map. Raw disassembly stays local and ignored.
