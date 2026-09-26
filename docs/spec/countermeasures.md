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

## Host rules

- A held key does not repeat. Nothing is released while paused, after the
  aircraft is destroyed or after ejection.
- The release shows the existing chaff or flare burst effect at the aircraft
  for 45 ticks (0.375 seconds), shared with AI releases. `fitted`.
- Gamepad: hold View and press D-pad left for chaff, D-pad right for flare.
  `opinionated`, agent decision 2026-09-26.

## Unknown

- **Release sound.** `&CHAFF.5K` and `&FLARE.5K` exist; their use is not traced.
  T.O.R.E plays no release sound yet. Next step: trace the sound request in the
  release routine.
- **Dispenser damage messages.** The strings `CHAFF DISPENSER DAMAGED` and
  `FLARE DISPENSER DAMAGED` exist; when they are shown is not traced.

## Source notes

In-flight key dispatch `0x414690`: scan code 0x52 (Insert) branches at
`0x41493b` to `0x415c47`, 0x53 (Delete) at `0x414946` to `0x415c9b`. Both call
the release routine `0x4c39a0` (ECX 1 chaff, 0 flare), which returns the count
before release. A result of zero or less prints the "Out of" string
(`0x4ee4fc`, `0x4ee4d4`). Otherwise the count minus one is printed with
`0x4ee50c` or `0x4ee4e4`, unless global flag `0x4eb6f8` bit 0x8 is set, which
keeps the count. The routine scans the player's dispenser records and creates
one device object per call.
