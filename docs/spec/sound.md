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

Each chaff cartridge or flare released plays `&CHAFF.5K` or `&FLARE.5K`, for
the player and AI alike. Its level, distances and cockpit rule are in the
[countermeasures spec](countermeasures.md#release-sound).

## Numbers and boundaries

| Component | Established value |
| --- | --- |
| IR growl | Unsigned 8-bit mono, 11,025 samples/second |
| Aircraft pass and sonic boom | Unsigned 8-bit mono, 11,025 samples/second |
| Missile pass | Unsigned 8-bit mono, 5,512 samples/second |
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
