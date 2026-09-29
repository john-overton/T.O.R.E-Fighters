# Testing

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

This folder explains how T.O.R.E is tested beyond the unit tests, what each kind
of test can and cannot tell you, and how to add more.

## The three layers

| Layer | What it is | Where it lives | Runs |
| --- | --- | --- | --- |
| Unit and integration tests | `cargo test`, Python `unittest` | `crates/*/src`, `tools/test_*.py` | Every push, on all platforms |
| Fixed acceptance probes | Single-purpose app modes such as `--validate-creator`, `--combat-smoke`, `--probe-matrix` | [headless development](../DEVELOPMENT.md#headless-development) | By hand when the area changes |
| The battery | Many app runs at once, with automatic checks on the output | `tools/battery.py`, `tools/battery_scenarios/` | By hand or overnight on a dev machine |

The battery is for finding bugs nobody has thought to write a test for: it plays
the game the way a player might (menus, takeoffs, fights of every size,
replays), all at once, and flags anything that looks wrong.

## Running it

Build once, import the retail media once into a profile folder, then run:

```sh
cargo build --locked -p tore-app
TORE_DATA_DIR=.local/bugbash-data target/debug/tore-app --import gameassets/fighters-anthology --import-only
python3 tools/battery.py --list
python3 tools/battery.py --lane ai --jobs 8
python3 tools/battery.py --scenario 'ai-fight-15v15*'
```

Each scenario gets its own copy of the profile (a cheap copy-on-write copy on
btrfs, APFS and similar), so runs never touch each other or your real profile.
Results land in `.local/battery/<timestamp>/`: `summary.md` first, then
`logs/<scenario>.log` for the full output of each run and `results.json` for
tools. `--keep-data` keeps each run's data folder (replays, logs) for a
closer look.

Scenarios that open a window go through [`tools/agent-run.sh`](../../tools/agent-run.sh)
so they never cover your active workspace. `--windows N` limits how many are open
at once (default 3).

## Lanes

A lane is a family of scenarios with one file each under
`tools/battery_scenarios/`, and one page here.

| Lane | Covers | Scenario files | Page |
| --- | --- | --- | --- |
| `menus` | Menu screens, the Quick Mission creator, the loadout page, captured screens, terrain and weather captures, text decoding, the retail manual audit | `menus.py` | [lane-menus](lane-menus.md) |
| `flight` | Ground start, takeoff, landing, every aircraft's flight, weapons, jettison, countermeasures, damage, ejection, environment | `flight.py` | [lane-flight](lane-flight.md) |
| `ai` | One against one up to fifteen against fifteen, every theater, missions, skills, damage, wing orders, invariants on every tick | `ai.py` | [lane-ai](lane-ai.md) |
| `replay` | Recording, playback, the Replays screen, radio and crew comms, audio start-up, input, hand-flown and mouse scripts, cheats, import errors | `replay.py` and `_replay_*.py` | [lane-replay](lane-replay.md) |

Each lane page lists what the lane covers, how long it takes, every bug found and
fixed, the known failures, the behaviours that need a decision and the things
that need a human eye or ear. Run one lane with `python3 tools/battery.py --lane
NAME --jobs 6`; a full pass of all four takes roughly two hours on a 24-thread
machine with a debug build.

Some tools the lanes added are worth knowing on their own:

- `--probe-fight FRIENDLY:ENEMY` sizes an AI probe up to 15 against 15.
- The AI probe checks every tick for impossible states and prints `AI probe anomaly:` lines.
- `--validate-creator` also sweeps thousands of Quick Mission setups, the loadout pages and the render of the creator; `TORE_CREATOR_STAGE` picks one part. `--validate-text` scans imported text.
- `--input-script FILE` presses keys and clicks the mouse in a windowed run, through the same handlers as real input (see [development](../DEVELOPMENT.md)).
- The headless flight probe has scripted pilots (spin recovery, landing, autopilot, eject) and an `extremes:` line.

## What counts as a problem

Every scenario is checked for a set of general problems: a crash or panic, a
non-zero exit, a timeout, a `NaN` or infinite number in the output. Scenarios add
their own: text that must appear, text that must not, files that must be
written, or a small function that reads the output and returns a list of
problems (for example "ammo went negative", "aircraft still alive with zero
hit points", "the ordnance page and the aircraft disagree").

## What it cannot tell you

- **Retail parity.** There is no retail comparison; the battery checks that the
  game is self-consistent and does not break, never that it matches the original.
- **How it looks or sounds.** Captured screens are inspected by the agent that
  ran them, which finds broken layouts, missing art and empty frames, not taste.
  Anything that needs a human eye or ear goes on the review list in the run
  report.
- **Other platforms.** Runs happen on the Linux dev machine. Golden values that
  are only recorded on Apple silicon are flagged, not edited.

## Adding a scenario

Add a `Scenario(...)` to the lane's file, run it alone with
`python3 tools/battery.py --scenario NAME`, and check its log. A scenario that
found a bug should stay in the battery as its regression check, next to a unit
test in the crate that owns the fix where one is practical. Scenario names are
unique across lanes; the runner refuses duplicates.

## Reports

Measured evidence of a battery pass goes in [`docs/baselines/`](../baselines/),
one file per pass, following the baseline rules in [AGENTS.md](../../AGENTS.md).
The first pass is [battery-2026-09-28](../baselines/battery-2026-09-28.md).
