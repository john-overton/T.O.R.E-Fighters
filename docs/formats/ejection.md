# Ejection source notes

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Static research, 2026-09-23. Input EXE/SMS/archive identities are those in
[the AI baseline](../baselines/ai-research.md#inputs-and-identity), including
FA.EXE SHA-256 e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c.
No original executable or module was run. The manual page image at PDF page 165
confirms Shift+E, twice, on printed page 161.

## Reviewed source points

- FlightKey 0x414b76..0x414d08 tests PLANE flags bit 0x10, reports no seat when
  absent, checks previous key 0x45, and calls EJECTAdd on confirmation. The
  2-second host confirmation timeout is fitted. Later speed-dependent injury
  branches at 0x414bb5..0x414c26 are unresolved, not reproduced as guessed odds.
- EJECTAdd 0x469970 loads EJECT.NT, transfers position/motion, and calls the
  effect service with `&eject.5K` at 0x469c36. _EJECTProc 0x4692d0 dispatches
  the independent object movement and event callbacks.
- EJECTMoveProc 0x4694d0..0x469949 handles states 0x22..0x26. The descending
  branch at 0x469611 compares surface height plus 0xfa000 fixed8 units, 4,000
  feet, before transitioning and scheduling `&CHUTE.5K` at 0x46967d.
- _PLANECheckEject 0x49fa10 checks an ejection deadline and remaining crew byte,
  spawns a pilot, decrements the byte and reschedules. PLANESetEjectTime
  0x49d6e0 checks seat capability and selects immediate or delayed ejection.
  It does not itself assess lift or dive recovery. Death handling at 0x49d890
  can schedule it. Full initiating predicates and crew-count producers remain
  unknown. Next research: trace these producers, not just the scheduler.
- Player ejection selects among speech pointers at 0x4ee388 via 0x414cf9.
  Entries include `^EJECT.5K` and `^EJECTNG.5K`. The selector 0x490f30 rewrites
  ejection clips to `#EJECT.5K` for nationalities 0x14/0x15, North and South
  Vietnamese (see [cockpit voice](../spec/cockpit-voice.md#the-second-voice-set)).
  The `#` set is a second voice for those speakers, not the RIO.
- Reviewed phrase/sample pairs at 0x4ff4a8 and 0x4ff4b8 identify `^EJECT` and
  `^PUNCH` as ejection announcements. Pair 0x4ff5b8 identifies `^OUTFUEL` as
  a fuel-empty ejection announcement. These pairs alone do not establish speakers.
- A fatal countdown warning at 0x410c97..0x410cea submits `^EJECTX3.5K` within
  three clock seconds of the deadline, once, only with a seat. Complete warning
  causes and speaker identity remain unknown. Host danger-event timing and
  assigning this warning to a second crew member are fitted.

## Resource census

FA_2.LIB contains EJECT.NT, EJECT.SH, EJECT_S.SH, four textures
_EJECTA.PIC through _EJECTD.PIC, KBAIL.SEQ, M_EJECT.MUS, &EJECT.5K,
&EJECT.11K, &CHUTE.5K, #EJECT.5K, ^EJECT.5K, ^EJECTNG.5K, ^EJECTX3.5K,
^PUNCH.5K and ^OUTFUEL.5K. Presence is asset evidence, not a trigger contract.
EJECT.NT explicitly binds _EJECTProc and the pilot/shadow shapes, naming Pilot.
EJECT.SH SHA-256 is 61daf6359c2770dbf28cec23b52656d006d396e6dcb2a313e69a16539fbeb92c.
Its inert guard records reference _PLstate and _PLdead. Static projections for
states 34..38 produce 95, 6, 79, 79 and 78 faces respectively. The top-level
selection is a bounded chain of word compares and short conditional branches.
The inferred BC/0x96 line grammar contains two aligned vertex-slot references
at +4/+6 and a palette byte at +1. The app uses these for suspension cords;
line interpretation and 0.05-foot ribbon thickness are fitted until the original
primitive drawer is traced. Missing or unaligned slots fail closed. The initial guard also
sets effectsAllowed; the reader must not execute that side effect or any code.

The USNF-ATF documentation identifies ejection as unfinished and is not used as
an implementation or retail-behaviour source. No runtime comparison is available.
Campaign inventory/rescue/capture, separate additional crew, high-speed injury
odds, native animation timing and exact speaker routing remain open research.
