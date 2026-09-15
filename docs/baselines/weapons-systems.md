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

## Connected runtime behavior

Both aircraft resolve their own ECM, all 45 systemDamage bytes, afterburner
availability and source hardpoint identities once during construction. Player
capacity is 232 for F/A-18D and 200 for Rafale C; range-target HP remains the
previous explicit base-HP surrogate (116/100). F18.ECM is an explicit reference
in **both** PTs: mode flags 0x1f0, 30 chaff, 30 flares, radar deception 30,
IR deception 40, radar/IR signature additions 100, noise distances 0/0.

- Existing J now affects incoming signature-2/3 contact resolution. Y powers
  the same PT equipment on the range target for outgoing ECM testing. Source
  mode flags distinguish IR/radar support. Failed equipment cannot protect
  the player. A geometric contact starts at an **authored base chance of 100**;
  the reviewed ECM multiplier and an explicit adapter RNG decide defeat/hit.
  No periodic random lock breaking is invented. Defeated contacts consume the
  attacking projectile, leave HP unchanged and generate feedback text.
- I launches **one explicit incoming fixture** using the selected source JT,
  without debiting player ammunition or emitting an own-launch haptic cue.
  It starts 1,800 feet ahead and flies toward the player through the existing
  source movement/pursuit/fuze/contact path. Its illumination is an explicit
  always-on fixture; no AI or autonomous launch decision exists.
- Swept player/projectile contacts use relative previous/current positions.
  Applied damage is clamped to remaining HP; cumulative damage retains the
  pre-clamp amount for selection. D requests one gun-strength player hit on
  the next tick, using the aircraft's own gun damage and reviewed amount roll.
- Ordinary player hits automatically run the weighted selection and repeat
  gates. JT failures preserve mass/ammo but inhibit release; visual failure
  removes visual acquisition; radar failure removes radar contacts, inhibits
  radar launch and breaks illumination-dependent tracking. ECM failure follows
  the separate jammer/chaff/flare branch. Unmapped selected indices retain
  their bounded occurrence counts and report their source index; they do not
  silently alter engine thrust or hydraulic pressure.
- Weapon instruments display player HP and V/R/E state: V is visual availability,
  R radar power, E jammer power; `+` available/powered, `-` off, `!` failed.
  Source fault index, readiness, ammo and existing target hit data remain visible.
  Combat radar/jammer power now uses the existing instrument engine-power gate;
  an engine-off radar cannot display off while still authorizing launches.
- Confirmed player damage/destruction uses existing original explosion audio
  and distinct haptic cues. Gun and missile cues come from actual own releases.
  Incoming launches and distant target hits cannot produce player-damage rumble.
  [Controller mapping and bounded haptic envelopes](../INPUT.md#manual-combat-layer--2026-09-14).

New commands are development mappings, not recovered retail shortcuts. I and Y
replace previously unavailable IR/history shortcuts only in this development
workflow; other native systems remain unavailable. Each fixture command requires
`--live-fire`. Native J and R mappings are retained. The in-game keyboard help and
controls editor include the new actions and modifier layer. Old saved profiles
are preserved; automatic defaults apply to newly mapped standard Linux pads.

## Replay and diagnostic setup

Combat tape **version 2** adds explicit launcher jammer power and the new
commands. Version-1 tapes are rejected; this is a deliberate format/behavior
boundary. Full state comparisons include RNG, player HP, subsystem counts,
failed equipment, incoming projectiles and previous player contact position.
No ticks run while paused; controller/keyboard holds must release after an
interruption. Reset reconstructs damage/equipment state and cancels held fire.
The tape is still a combat-service replay with explicit launcher inputs, not
native scheduling parity or full pilot-flight replay.

`--combat-command` additionally accepts `damage`, `incoming`, `target-jammer`.
`--jammer-on` starts the explicit range with own jammer power requested. For example:

```sh
cargo run --locked -p tore-app -- --live-fire --aircraft rafale --jammer-on --weapon-slot 2 --combat-command arm --combat-command incoming --combat-probe-ticks 300 --record-combat .local/incoming.tape --instrument-page 8 --capture-flight .local/incoming.ppm
cargo run --locked -p tore-app -- --aircraft rafale --replay-combat .local/incoming.tape
```

The arm command makes the player safe while the incoming fixture runs. Probe
logs include confirmed cue counts and mixer pulses; probes do not play old
haptic events on hardware after opening the window. Normal live events use the
native backend with the user's rumble preference.

## Remaining native-parity gates

This is a further integration pass, **not completion of every gap in the previous
baseline**. Important limits remain:

- ECM signature addition/noise, decoy emission/geometry/lifetime, chaff/flare
  deployment and seeker countermeasure terms, sensor cadence/signatures/Doppler,
  sun/angle terms, active activation, lead/PN and automatic reacquisition.
  Chaff/flare inventory and native fault depletion are represented; there is no
  player dispenser action or claim of a complete countermeasure lifecycle.
- Native RNG/scheduler, difficulty/caller percentage, forced-damage branch,
  catastrophic/delayed destruction and all subsystem side effects. Ordinary
  selection uses difficulty mask zero and the PT afterburner-availability gate.
  The authored xorshift stream is deterministic, not FA's generator. Native
  player HP protection/delayed fire/death branches are not reproduced: reaching
  zero HP destroys the adapter player immediately. Unknown subsystem effects
  are observable source-index records, not complete engine/fuel/hydraulic damage.
- Source entries with zero repeat limits stay ineligible. Native dynamic edits
  to the table, if any, and complete station eligibility/side effects need review;
  do not claim every loaded station can already suffer automatic damage.
- Outgoing range-target damage retains the prior nominal source-class policy;
  the new integer percentage/random amount path is connected to player damage.
  Native collision geometry, full hit-probability terms, collateral and effects
  remain partial. Existing gun/flight/projectile tuning is not retuned.
- Only ten PT-default slots/eight weapons are enabled. Alternate compatible
  loadouts, bombs, rockets, laser/anti-radiation/cruise/cluster branches, racks,
  complete store textures/drag, auxiliary tank transfer/jettison remain open.
  Bomb/rocket haptic envelopes are ready and tested but have no live producer.
- No combat AI, no new aircraft, no mission/loadout screen expansion. A future
  basic fly-forward target remains deferred until full manual weapon acceptance.
