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
