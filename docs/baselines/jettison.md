# Inert jettison validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation pass on base `ba9712a0`, 2026-10-15. Contract:
[jettison](../spec/jettison.md). No retail comparison or GPU capture was run.

## Before and after

The base `State::apply_command` simply cleared the selected external ammunition.
It created no falling object, release sound or impact. Tank jettison was absent.
This is source evidence, not a before-image comparison.

The new headless `a2g-jettison` scenario flies an F-16C with its centreline tank
(`F150.GAS`, 108 lb shell) and four Mk 82s, level at 5,000 ft and 300 kt. It drops the tank at 1 second
and the selected bombs at 3 seconds. No trigger is pressed.

| Observation | Result |
| --- | --- |
| Initial payload | 4,168 lb, including 990 lb of external fuel |
| Fuel just before tank release | About 988.6 lb external remains after the first second's burn; internal remains 6,972 lb |
| After tank jettison | 3,070 lb payload, 0 lb external fuel, four bombs still loaded |
| After selected-store jettison | 1,070 lb payload, zero bombs at that station |
| Detached objects | One tank and four bombs |
| Release velocity | 506.34 ft/s forward, 6 ft/s down |
| Tank fall time | 17.484 s after release |
| Bomb fall time | 17.489 s after release |
| Impact position error | Less than 0.001 ft against the terrain-aware predictor |
| Impact effects | Five 2.5 ft wide pock marks; no armed effects, motors or missile smoke |
| Ground target | 200/200 hit points after the pass |
| Recorded sound events | Two jettison releases, two type-15 and three type-16 ground thuds |
| Replay scene comparison | 3,601 ticks, zero missing and zero differing snapshots |

The scripted setup relocates and turns the aircraft at tick 1. The log therefore
reports one setup teleport and one attitude jump before either jettison command;
these are probe setup, not gameplay movement.

The synthetic tests also cover two wing tanks plus a centreline tank, external
fuel bookkeeping, protected internal ammunition, water splashes without craters,
inert checkpoint continuation, release/impact sample selection, actual stock
key resolution, custom binding preservation and the extended wire command.
All simulation and format tests passed after the sound-only lookup fix.

## Reproduce

Run against a private copy of an imported profile:

```sh
TORE_DATA_DIR=<private-profile> nice -n 10 target/debug/tore-app \
  --a2g-run UKR QUCOL --attack jettison --aircraft f16c --load MK82 \
  --altitude 5000 --speed 300 --seconds 30 --defenses 0 0 \
  --record <new-path>.tore-replay --verify-render
```

`--verify-render` compares recorded scene data with live snapshots on the CPU;
it does not certify GPU pixels or audible playback. The slice report under
`.local/next-round/reports/nr-jett.md` records the full validation results,
sandbox restrictions, guard comparison and exact GPU still command.
