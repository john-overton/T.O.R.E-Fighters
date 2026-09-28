# AI gun-employment implementation checks

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Build and scope

Implementation mode, 2026-09-28, on `fix/visual-awareness-under-fire` in
`/home/john/Development/T.O.R.E-Fighters-ai-awareness`. The source is the
implementation change containing this document, based on research commit
`9d807f37b4e3acacb1eb3a1cf6e90c63513594bb`. Pre-commit recordings identify that
base commit. The prior [research baseline](ai-gun-employment.md) preserves the
fault evidence. Behavior and authored constants live in the
[gun-employment specification](../spec/ai-gun-employment.md).

Linux x86-64, locked workspace debug builds, user-owned imported media in the
shared isolated dev profile. No media, recordings or captures are committed.
Evidence is under `.local/ai-gun-implementation/` in this worktree.

## Component checks

- All 14 service profiles recover from expiry with a continuously selected
  target, both unchanged and replaced. Empty stations, failed lock and blocked
  paths do not become fire permission.
- A complete half-second burst emits 16 rounds at the shared 32-round/second
  cadence. Two complete bursts emit 32. Thirty starting clock phases check
  fractional spacing. Interruptions, changed targets, target loss and repeated
  ticks cannot bypass recovery or duplicate a round.
- Approaching, receding and crossing synthetic targets are hit by the flown
  projectile when the barrel follows the predicted lead. Rotating the barrel
  away misses. These cases isolate nominal physics from dispersion and use
  both intended-player and intended-AI metadata. The shooter remains undamaged.
  Lead angular rates are also checked against finite differences.
- A gun can hit the player while its intended target is another AI. Physical
  collision follows the path, not the targeting metadata. The discovered
  shooter-self-hit fault is covered by these shared-combat tests.
- The host emission test checks actual barrel direction and tracer metadata,
  no debit at authorization, one debit per emitted round, and no debit for
  misalignment, full projectile capacity, inhibited/empty stores or a dead
  shooter. Accepted hold and formation recall prevent later authorizations.
- Existing missile support, finite depletion, gun cadence/dispersion, sensor
  privacy, orders, defense and deterministic golden tests pass unchanged.

Exploratory runs before the shooter collision repair are excluded from acceptance.

## Imported encounters

29 two-minute acceptance probes: eight Su-27 skill/adapter combinations,
14 exact opponent profiles, two side/rear encounters, two mixed-store weapon
modes and three repeated Average/legacy runs. Each simulates 14,400 fixed ticks
and verifies every recorded render snapshot, 14,401 frames including tick zero.
All pass with zero missing or differing frames and no recorded gun self-hits.

The player follows the existing straight-flight probe script and does not fire.
There are two friendly AI and two enemy AI, with normal autonomous defense.
These are controlled encounters, not a human dogfight or retail comparison.
Except for the two mixed-store cases, AI carry only their own imported gun.

### Su-27 against F-22

Head-on starts are five nautical miles apart. Enemy physical gun rounds and hits
are counted across both Su-27s. Late rounds are those released after 35 seconds,
beyond the old player-session timeout.

| Flight adapter | Skill | Enemy rounds | Enemy hits | Rounds after 35 s |
| --- | --- | ---: | ---: | ---: |
| Legacy | Novice | 112 | 12 | 35 |
| Legacy | Average | 83 | 11 | 6 |
| Legacy | Experienced | 94 | 11 | 17 |
| Legacy | Ace | 91 | 11 | 14 |
| Researched | Novice | 84 | 8 | 3 |
| Researched | Average | 84 | 8 | 3 |
| Researched | Experienced | 84 | 8 | 3 |
| Researched | Ace | 84 | 8 | 3 |

In the Average legacy case, the leader fires 75 rounds at the player and the
wingman eight at a friendly AI. The player takes 11 hits and is killed. The
wingman still fires at 112.34 seconds. Gun accuracy has no new skill multiplier;
skill still affects the surrounding decisions, so similar initial head-on
results across skills are expected.

With a one-mile rear start, Average researched Su-27s fire 49 rounds at an AI
F-22 between 90.575 and 109.708 seconds, scoring 11 hits and one kill. With a
one-mile side start, Ace legacy Su-27s fire ten rounds at an AI F-22 between
74.842 and 110.133 seconds, with zero hits. Both demonstrate late opportunities;
the latter also shows that a valid predicted solution does not guarantee a hit
against a maneuvering aircraft.

### Exact aircraft roster

Average enemies, researched flight, five-mile head-on start. The player/control
wing uses F18.PT, except the F18.PT enemy case which uses F22.PT. F18.PT is the
F/A-18D, RAFALE.PT the Rafale C, and F31.PT the X-31. `faxx` is the existing
opinionated F/A-XX concept using F22N.PT donor data, not a retail aircraft record.
No variant substitutions. Each of the 14 opponent profiles emits physical gunfire
and scores a hit.

| Enemy record | Enemy rounds | Enemy hits | Rounds after 35 s |
| --- | ---: | ---: | ---: |
| A4E.PT | 36 | 5 | 0 |
| F14.PT | 36 | 7 | 4 |
| F18.PT | 22 | 10 | 0 |
| F22.PT | 38 | 6 | 0 |
| F22N.PT | 38 | 6 | 0 |
| faxx (F/A-XX concept) | 38 | 6 | 0 |
| MIG21.PT | 49 | 15 | 14 |
| MIG23.PT | 32 | 5 | 0 |
| MIG29.PT | 158 | 19 | 79 |
| RAFALE.PT | 42 | 11 | 10 |
| SU25.PT | 82 | 3 | 0 |
| SU27.PT | 102 | 5 | 13 |
| SU35.PT | 93 | 7 | 4 |
| F31.PT | 41 | 7 | 3 |

The MiG-29 leader emits exactly its 150 carried rounds and finishes with zero;
the wingman emits eight and finishes with 142. Other actors continue fighting.
This confirms depletion and emission accounting in a sustained imported fight.
A zero in the late-round column means no later firing opportunity in that run,
not a return of the terminal service fault.

### Mixed stores and repeatability

Researched Su-27/F-22 head-on runs with normal missile-plus-gun inventories,
one with normal weapon rules and one with `--compatibility-weapons`, each
produce four missile launches and 16 physical gun rounds. Both finish and pass
render verification. Existing missile lifecycle tests provide the deeper support
and guidance coverage; these two encounters are integration checks.

A repeat of the final Average legacy case matches all 121 state checksums.
`--recording-diff` reports identical launches (83), hits (11), kills (one),
shot outcomes, AI decisions, communications and aircraft/system events. The only
header difference is wall-clock recording time. See `determinism.log`.

## Reproduction and checks

From this worktree, one representative command is:

```sh
TORE_DATA_DIR=/home/john/Development/T.O.R.E-Fighters/.local/dev-profile \
  target/debug/tore-app --ai-probe-ticks 14400 --aircraft f22 \
  --probe-enemy-aircraft su27 --probe-enemy-skill average \
  --probe-flight-model researched --probe-geometry head --separation 5 \
  --probe-ai-guns-only --record-mission NEW_PATH.tore-replay \
  --verify-render --no-audio
```

`--recording-log FILE --out NEW_DIR` exports individual launch/hit events and
summaries. The local scripts `run_acceptance.py`, `run_roster.py` and
`summarize.py` record scenario parameters and derive counts.

Required checks passed: formatting; locked workspace Clippy with warnings denied;
1,826 Rust tests, eight existing ignored tests; locked workspace build; 84 Python
tool tests; source and both binary asset checks; documentation checks. The Linux
GPU smoke test presented successfully. A replay capture at tick 1850 shows the
F/A-18D firing with the thinking panel and correct activity. Gun solution,
ammunition and cycle rows are also present in the recorded thought trees.

Windows/macOS execution and a human-flown acceptance fight were not run. Original
gun duty cycle, alignment policy and skill accuracy remain unknown. Lead assumes
constant measured target motion, and aggressive turning fights remain a fitted
tracking/tactics tuning area. Surface firing AI remains outside this change.
