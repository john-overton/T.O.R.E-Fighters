# Two-aircraft manual weapons integration

Continuation of [live fire](live-fire.md), 2026-09-14. Scope remains F18.PT
(F/A-18D) and RAFALE.PT (Rafale C): ten PT-default JT stations, eight distinct
weapons. Imported compatible alternatives are a separate acceptance backlog;
no additional flyable aircraft or combat AI is enabled.

## Implementation sequence

1. Resolve readiness/inhibit reasons, separate launch acquisition from tracking,
   and repair target replacement identity and stale-track handling.
2. Resolve damage class from source object category, retain bounded per-hit
   amounts/cumulative damage, expose class fixtures and station failure testing.
3. Connect manual controls/instruments and carried source weapon geometry,
   jettison/payload updates, and deterministic command replay checks.
4. Exercise every default slot through positive and negative lifecycle cases;
   run workspace/Python/asset/GPU checks and capture both aircraft.

## Additional native evidence

Same FA executable/SMS hashes as [weapon research](../formats/weapons.md).
`0x411470..0x4114eb` maps the object category word at +0x0d to damage index:
0x40/0x200/0x800/0x1000 -> 4; 0x100 -> 2; 0x400 -> 3;
0x2000 -> 1; other categories -> 0 (including 0x80/0x4000/0x8000).
This is an exact category switch, not a bitwise membership test.
`DAMAGEDoHit 0x40f9b0..0x40f9e8` selects damage[class], applies the caller's
percentage, then a random 80..119 percent factor with integer divisions.
The live adapter retains explicit full-strength, unrandomized damage until the
caller/RNG/difficulty contract is recovered. It must not claim native hit amounts.

`PROJLock 0x4c2fbc..0x4c3011` gates signature-3 player launch on radar for
flag 0x10000; `0x4c3027..0x4c30e7` checks the launcher for flags 0x700,
with the radar dependency specifically under 0x200 and 0x10000. This supports
separating launch radar acquisition from illumination-dependent tracking;
it does not establish native active-seeker activation range or PN guidance.

`DAMAGEDoHit 0x4103f1..0x4104b7` locates a hardpoint, marks its ammo word
with 0x8000, and invokes HARDSetFlags for projectile/equipment damage.
Station failure can therefore inhibit a live station without deleting its mass.
Choosing which subsystem fails from a hit remains unverified; do not invent
HP-percentage engine/radar failures or represent fault injection as native rolls.

## Acceptance

Implementation and validation results will be recorded here as completed.
AI is deferred until the complete manual weapon acceptance pass; no AI work is
part of this change. Broader native parity contracts remain tracked explicitly.
