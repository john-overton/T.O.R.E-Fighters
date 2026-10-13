# Combat and flight sound

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## What the player hears

A selected air-to-air IR weapon uses the original `&IR1.11K` growl for both
tracking and lock. Lock replaces tracking, it does not layer a second tone over
it. The growl increases with shot quality, and the unlocked growl is quieter.
The original service scales IR volume with its hit-chance result and halves the
tracking level. Both states loop the same recording, so lock must not restart
or change the recording. The manual independently describes stronger A2A IR
growl with better lock.

The reviewed selector uses this pair of IR channels for signature 2 with weapon
flag `0x10000`. Other selected seeker channels use `&RDRTRY.5K` and
`&RDRLOCK.5K`, including the reviewed surface IR branch. This is a sound choice,
not permission to require aircraft radar for an IR weapon.

The original passing-object service selects `&AIRPASS.11K` for aircraft,
`&MPASS.5K` for the missile branch, and `&SNCBOOM.11K` for its supersonic
player-aircraft branch with view-dependent eligibility. A separate projectile
flag chooses `&BPASS.5K`; its full class mapping is unknown and is not enabled.

Explosions, impacts and splashes play the recordings their
[explosion type](explosions.md) names, heard to its own distances.

Each chaff cartridge or flare released plays `&CHAFF.5K` or `&FLARE.5K`, for
the player and AI alike. Its level, distances and cockpit rule are in the
[countermeasures spec](countermeasures.md#release-sound).

Rotorcraft: the engine loop plays at the rotor's speed, a share of its
governed 100 percent, so a rotor that bleeds drops in pitch and an
overspeeding one rises (the V-22 sits at 84 percent in airplane mode); the
vortex ring state and retreating blade stall add a warble of up to 4 percent.
LOW ROTOR plays the stall horn and ROTOR OVERSPEED and the V-22's GEAR SPEED
play the stall warning, each after the condition has held a quarter second,
with their cockpit messages repeating every four seconds. The same pitch is
heard from another rotorcraft's engine and in a replay, which records the rotor
speed ([replays](../REPLAYS.md)). Opinionated (VTOL overhaul slice P7, agent
decisions): the sizes and the choice of recordings are not retail.

## Weapon launch and bomb release

Every weapon plays the recording its own record names when it leaves the
aircraft, and nothing else chooses it. A Maverick (AGM-65A and AGM-65G) plays
`&ASHVMIS.11K`, the recording it shares with the Shrike, Harpoon, SLAM, HARM,
Exocet, Kedge, Kickback, AS 30L and Kerry; the MK-82 family and MK-84 play
`&BOMB2.11K`; the cluster and guided bombs play `&BOMB.11K`. Retail has no
second recording for any of them, so the Maverick sound is a loud, heavy one by
design: its energy builds to a peak about half a second in, and it is the
loudest missile launch in the game.

- **One sound per release.** The sound is asked for once per trigger call,
  after the call has made all its projectiles. A 3-, 6- or 7-bomb pack is one
  thump, not three, six or seven laid on top of each other; a rocket pod's held
  fire sounds once per rocket, four a second; a gun sounds per its own round
  cadence as before. The simulation reports each bomb of a pack, so the same
  recording asked for twice in one tick plays once.
- **From the cockpit.** The firing aircraft's own cockpit hears it at once,
  centered, at full level, wherever the weapon goes.
- **From outside.** In an external view it starts at once and is full within
  100 ft of the aircraft, then fades in a straight line to silence at the
  record's `maxSndDist`: 6,000 ft on most records, 7,000 on the AGM-65A,
  AGM-45, GBU-10 and a few more. A view 200 ft back hears it at nearly full
  level, where the earlier generic curve gave it about 40 percent. It is
  silent past 20,000 ft whatever the record says.
- **Pitch.** The record's `freqAdj` offsets the pitch (1/256 of the 11 kHz rate
  per unit). It is zero on every missile, bomb, rocket and gun a player's
  aircraft can carry and nonzero only on surface guns, so there is no pitch
  change to hear; it is not applied.

Agent decisions: a replay and a multiplayer client carry the recording name
only, so their launches fade over 6,000 ft (fitted: the value on 118 of the
135 records); the player's own launch no longer waits for sound to travel in an
external view, where the earlier build delayed it by distance. Evidence and the
disassembly addresses are in
[the sound format notes](../formats/sound.md#weapon-launch-and-bomb-release-sound).
Not heard by ear yet; whether `&ASHVMIS.11K` is what the Maverick should sound
like is John's call to make by listening.

## Other aircraft's launches

Retail plays an aircraft's launch sound for any aircraft, not only the one
flown: the sound request carries the record's recording and `maxSndDist`, and
it sounds when the listener is within that distance of the firing aircraft
(John asked for this on 2026-10-12). So an AI wingman's Sidewinder, an enemy's
bomb release and another player's Maverick online are heard.

- **Same rule as the external view.** Full within 100 ft of the firing
  aircraft, a straight-line fade to silence at the record's `maxSndDist`,
  nothing beyond, starting at once. In a cockpit it is heard through the
  canopy (40 percent, no treble), as every outside sound is.
- **One sound per aircraft per release.** A seven-bomb pack from one AI
  aircraft is one thump. Two aircraft releasing in the same tick are two
  sounds, even for the same recording.
- **Surface units.** A surface unit's missile launch (a SAM site, a ship's
  SAM or SSM, any surface launcher) plays its weapon record's recording the
  same way, from where the missile leaves the launcher: full within 100 ft,
  fading to the record's `maxSndDist` (6,000 ft on the SA-3, SA-6 and most
  others, 12,000 ft on the SA-2A), silent beyond. One sound per launcher per
  release: a battery firing two missiles at once from one unit is one sound,
  two units are two. Switched on by John on 2026-10-12 (item 4); BA2 had left
  them silent. Every surface missile record names a recording (`&SALTMIS.11K`,
  `&SAHVMIS.11K`, `&LTMISS.11K`, `&HVYMISS.11K`), so none needs a fitted
  default.
- **Guns.** Gun fire, surface guns included, keeps its own audio.
- **Voices.** At most the four nearest launches sound in a tick (agent
  decision); the placed-sound pool still drops the quietest when it is full.
- **Online and replays.** The wire and a recording name the weapon (and the
  shooter), not the recording or the reach, so the game looks the weapon up in
  its own weapon catalog by name: the same `fireSound` and `maxSndDist` the
  host used. Where a weapon is not in the catalog it is silent. Position is the
  firing aircraft's at that tick. No protocol change.

Retail evidence for surface launches: `PROJFire` (`0x4c2170`) and
`PROJFireSound` (`0x4c26f0`) test no object class before the sound request,
and every surface missile record carries the same `fireSound` and
`maxSndDist` fields
([evidence](../formats/sound.md#surface-launches)). Whether the original's
ground units reach `PROJFire` through the same NPC weapon service as aircraft
was not traced, so that a SAM site sounds in retail is expected, not seen.

Agent decisions: the four-voice limit, the canopy muffling inside a cockpit
(consistent with the other outside sounds) and starting at once with no travel
delay (as the player's own external launch). Not heard by ear yet.

## Numbers and boundaries

| Component | Established value |
| --- | --- |
| IR growl | Unsigned 8-bit mono, 11,025 samples/second |
| Aircraft pass and sonic boom | Unsigned 8-bit mono, 11,025 samples/second |
| Missile pass | Unsigned 8-bit mono, 5,512 samples/second |
| Weapon launch and bomb release | Level 255 (full), centered in the firing cockpit; outside, full within 100 ft, silent at the record's `maxSndDist` (6,000 or 7,000 ft on the player's weapons), none past 20,000 ft; one request per fire call |
| IR tracking amplitude relative to lock | One half before the original smoothing |
| IR quality input limit | 100 percent |

The remake's displayed percentage remains a fitted estimate. Its exact gain
mapping, delay, distance fade, observer geometry, view policy, and limits are
specified in the [audio guide](../audio.md). John requested realistic sound
travel, passing sounds, external-view booms, and percentage/lock-sensitive growl
on 2026-09-23. Those extensions are opinionated requirements, not newly claimed
retail measurements.

## Unknown and next research

The original hit-chance formula, absolute device loudness, smoothing time,
complete passing/view predicates, and sound travel behavior are not established.
Next research would trace the remaining selector and volume producers. Retail
comparison is unavailable. No original executable was run.

## Source notes

Static evidence from FA 1.02F and user-owned FA_2.LIB is recorded in
[the sound format notes](../formats/sound.md). This establishes the recording
identities through executable consumers, not through filenames alone.
