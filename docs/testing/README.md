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

| Lane | Covers | Scenario file |
| --- | --- | --- |
| `menus` | Menu screens, the Quick Mission creator, the loadout page, captured screens | `menus.py` |
| `flight` | Ground start, takeoff, landing, every aircraft's flight, weapons, countermeasures, damage, ejection | `flight.py` |
| `ai` | One against one up to fifteen against fifteen, missions, skills, damage, wing orders | `ai.py` |
| `replay` | Recording, playback, the replay menu, radio and crew comms, audio start-up | `replay.py` |

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
