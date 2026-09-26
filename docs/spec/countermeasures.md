# Player countermeasures

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research and implementation, 2026-09-26. Build: reviewed FA.EXE 1.02F, SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`. Evidence
is static disassembly. Nothing was run and no retail session was observed.

## Recovered behaviour

- **Insert releases one chaff cartridge and Delete releases one flare**, one per
  press. The original reads keyboard scan codes without the extended bit, so
  keypad 0 and keypad period do the same whatever NumLock says. The full key
  table is in the [keyboard spec](keyboard.md).
- Each press takes one device from the first dispenser that still holds that
  class and reports what is left: `Chaff launched, 3 left` or
  `Flare launched, 3 left`. An empty dispenser reports `Out of chaff` or
  `Out of flares` and releases nothing.
- With **Unlimited ammo**, the count does not fall and the message repeats the
  same number.
- Counts start from the aircraft's ECM record and are shown on the WEAPONS
  window as `N CHAFF` and `N FLARE`
  ([weapons window](weapon-navigation-selection.md)). ECM damage can empty a
  dispenser ([systems damage](systems-damage.md)).
- Chaff can decoy radar-guided missiles; flares can decoy infrared missiles.
  Only a missile whose seeker is guiding on the releasing aircraft can be
  decoyed. It is decoyed with probability susceptibility × effectiveness / 100
  percent, both whole percentages from the weapon and ECM records. This is the
  rule the AI already uses ([missile defense](ai-awareness.md#missile-awareness-and-defense),
  [missiles](missiles.md)). A decoyed missile loses its target and coasts.

### Release sound

- **Every device released makes one sound:** `&CHAFF.5K` for a chaff cartridge,
  `&FLARE.5K` for a flare. The same holds for AI aircraft and for other players
  in multiplayer. An empty dispenser releases nothing and makes no sound.
- The chaff recording lasts 1.15 seconds and the flare recording 2.38 seconds.
- The level is 200 on the original's 0 to 255 scale, before the effects volume
  setting. A weapon launch plays at 255, so a release is about 2 dB quieter
  than a shot.
- The sound comes from the releasing aircraft and moves with it while it plays.
  It is at full level within 100 feet of the camera and fades in a straight line
  to silence at 4,000 feet. A release farther than 4,000 feet away is not
  played at all. Its stereo position follows its direction from the camera.
- In a cockpit view of the releasing aircraft, which for the player means their
  own cockpit, it plays at full level and centered, with no distance.

## Host rules

- A held key does not repeat. Nothing is released while paused, after the
  aircraft is destroyed or after ejection.
- The release shows the existing chaff or flare burst effect at the aircraft
  for 45 ticks (0.375 seconds), shared with AI releases. `fitted`.
- Gamepad: hold View and press D-pad left for chaff, D-pad right for flare.
  `opinionated`, agent decision 2026-09-26.
- Each device the player or an AI aircraft releases plays its recording once.
  The original's 100 and 4,000 feet become the reference and maximum distances
  of T.O.R.E's [traveling sound](../audio.md#traveling-sound), and its level
  becomes a peak gain of 0.31, against the 0.4 of a weapon release.
  `spec-derived`.
- In the player's own cockpit, the player's release plays at once, centered, at
  that peak gain. In every other view it travels from where it was released.
  `spec-derived`.
- Known difference: the level falls with inverse distance, as every T.O.R.E
  traveling sound does, instead of the original's straight line. At 1,000 feet
  the original is at 77 percent of full level and T.O.R.E at 10 percent. The
  sound also stays where the device left the aircraft instead of moving with
  it. `opinionated`: John requested this acoustic model on 2026-09-23; using it
  for releases is an agent decision of 2026-09-26.

## Unknown

- **Dispenser damage messages.** The strings `CHAFF DISPENSER DAMAGED` and
  `FLARE DISPENSER DAMAGED` exist; when they are shown is not traced.
- **Which views count as cockpit views** for the sound rule. The evidence points
  to the cockpit views; which key selects each internal view case was not
  traced ([sound evidence](../formats/sound.md#countermeasure-release-sound)).

## Source notes

In-flight key dispatch `0x414690`: scan code 0x52 (Insert) branches at
`0x41493b` to `0x415c47`, 0x53 (Delete) at `0x414946` to `0x415c9b`. Both call
the release routine `0x4c39a0` (ECX 1 chaff, 0 flare), which returns the count
before release. A result of zero or less prints the "Out of" string
(`0x4ee4fc`, `0x4ee4d4`). Otherwise the count minus one is printed with
`0x4ee50c` or `0x4ee4e4`, unless global flag `0x4eb6f8` bit 0x8 is set, which
keeps the count. The routine scans the player's dispenser records and creates
one device object per call.

Creating a chaff or flare device requests its recording once, with the
releasing aircraft as the source. The AI device schedule and the network
release message reach the same request. Addresses, the request's arguments,
the distance and pan rules, and the recording hashes are in the
[sound format notes](../formats/sound.md#countermeasure-release-sound).
