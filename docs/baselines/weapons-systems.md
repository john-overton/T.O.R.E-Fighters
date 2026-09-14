# Manual weapons and systems follow-up

2026-09-14, F18.PT F/A-18D and RAFALE.PT Rafale C only. No combat AI.
This extends [manual weapons](manual-weapons.md); it does not establish whole-game
native parity. Static code is read as data, never executed. The original C/C++
source and a matched original-game differential harness remain unavailable.

## Newly traced contracts

Same FA.EXE/FA.SMS hashes as [weapon evidence](../formats/weapons.md).

- `DAMAGEInit 0x40f77e..0x40f7a4` doubles OBJECT.hitPoints into the player
  capacity and current HP words. `0x50d2b1` is hitPoints, **not** the PLANE
  structureLimit used by the separate flight model.
- `DAMAGEDoHit 0x40f9b0..0x40f9e8` applies integer source damage × caller
  percentage / 100, then × (80 + random(40)) / 100.
- `0x40fd0a..0x40fd71` skips ordinary subsystem selection below capacity/3,
  then tests min(90, min(70, cumulativeDamage × 50 / capacity) + hit/4).
- `0x40fdc4..0x40fe40` makes ten weighted random(200) attempts using the low
  nibble of the 45 PT systemDamage entries. Its fallback scans up to 45 entries,
  wrapping and excluding 31..33. `0x410810..0x4108a9` checks nonzero weight,
  current count < bits 4..5, difficulty restrictions for bit 7, and nonzero
  aftThrust specifically for index 8. Zero repeat limits are not silently raised.
- Selected indices 36..44 map to source hardpoints 0..8 (`0x4103f1`). JT
  failure marks ammo's 0x8000 bit without discarding rounds. SEE signature-3
  failure also clears radar state (`0x410578..0x4105a8`).
- ECM damage (`0x4104c1..0x410576`) tries ten random(100) draws: below 25
  fails a jammer with mode flags 0x110 and clears both dispenser counts;
  25..64 clears chaff if the source carries it; 65..99 clears flares if carried.
  Failed eligibility retries. A selected ECM index does not invariably kill
  the jammer.
- HARDFindJammer/HARDFindECMForObj (`0x452ea0..0x452f74`) gate signature 3
  on mode flag 0x10 and radar jammer power, signature 2 on 0x100 and IR power.
  `PROJHitChance 0x4c348a..0x4c34e0` multiplies current chance by
  (100 − rdChance/irdChance) / 100. This is **hit probability**, not an
  automatic acquisition failure or unconditional midflight lock break.

The static extraction manifest includes these reviewed regions. Small arithmetic
translations live in `tore-sim::combat::systems`, with explicit random draws.
Runtime coupling, RNG stream/order, difficulty, collision, selection side effects
and destruction timing must be assessed separately from those translations.
