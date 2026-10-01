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

## Testing tiers

How much to run depends on the moment. Three tiers, from cheapest to most thorough:

| Tier | When | What | About |
| --- | --- | --- | --- |
| Per change | While you work, after each edit worth checking | `python3 tools/quick_check.py` | 5 minutes |
| Before merge | Before you finish, and what the pre-push hook runs | The [AGENTS.md](../../AGENTS.md) check list, and the full single-player baseline where it exists | The list, plus about 5 minutes for the baseline |
| Before a release | Ad hoc, by hand, before tagging | The whole battery with `TORE_AI_FUZZ=all`, and the lane pages' human checks; once multiplayer ships, also the [network matrix](#the-network-matrix) at five minutes a cell | About 85 minutes of wall clock at 6 to 8 jobs, and about 2 minutes more for the matrix |

The pre-push hook stays as it is. The full battery and the seeded fuzz games are not
part of any routine: run them when a release is near, or when you change something
broad and want the whole picture.

### Per change: the quick check

```sh
python3 tools/quick_check.py [--base REF] [--budget SECONDS]
```

It looks at the files changed since `REF` (default: the merge base with `multiplayer`,
or `HEAD~1` when you are on it; uncommitted and new files count) and runs, in order:

1. **Formatting**, `cargo fmt --all -- --check`.
2. **Clippy and the Rust tests** for the crates the change touches and every crate that
   depends on them. It runs the whole workspace when a shared crate's public API changed
   (a `pub` item's line was added or removed) or a `Cargo.*` or toolchain file changed.
3. **The Python tests** when anything under `tools/` changed.
4. **The documentation check** when anything under `docs/` changed.
5. **A build, then the battery scenarios the change can affect**, fitted to the budget
   (default 120 seconds of wall clock). See the next section.
6. **The quick single-player guard**, a small set of runs compared against the recorded
   single-player baseline, when the local harness is present and Rust changed (see
   "Before merge").

Each step prints its time, the summary ends with a total, and the exit status is nonzero
if any step failed. Logs are kept under `.local/quick-check/`. Steps with nothing to do
say so and are skipped. The battery and the guard need an imported data profile: pass
`--profile DIR` or set `TORE_DATA_DIR` (see "Running it"). `--jobs N` sets the parallel
runs (default: half the cores, 4 to 12), `--with-windows auto|yes|no` controls windowed
scenarios, `--no-battery` and `--no-guard` skip those steps, and `--plan` prints what
would run and stops.

On a warm build the quick check takes about five minutes at 4 to 12 jobs. The quick
check finds problems early; it does not replace the merge list.

### Choosing battery scenarios by change

`python3 tools/battery.py --changed [REF] [--budget SECONDS]` is the battery half of the
quick check and works on its own. It prints what it chose and why, then runs it.

- **A reviewed map** in `tools/battery_selection.py` turns changed source paths into
  scenario *families*. A family is a named group of scenarios by name pattern (for
  example `flight-stall`, `ai-airfield`, `radio`, `menus-screens`). Flight-model code
  maps to liftoff, approach, takeoff, combat G and stall families; AI code to the
  regression, fight, ground and landing families; airports to the creator and ILS
  scenarios; radio to the scenarios that check radio; menus and HUD code to their
  snapshots; a change to the battery's own files to its Python unit tests (which it runs
  first) and a few cheap scenarios of that lane. Documentation, tests and examples select
  nothing. A `Cargo.*` change selects every family, trimmed by the budget. A file the map
  does not know selects everything the budget allows and is named in the output.
- **The budget** is wall-clock seconds at the given `--jobs`. Each scenario's time comes
  from the newest full battery run under `.local/battery/` (a run that covers at least 90
  percent of today's scenarios; scenarios it lacks take their time from newer partial
  runs, then a default of 10 seconds). The estimate packs those times onto the parallel
  jobs, longest first.
- **What it keeps.** At least one scenario of every selected family, even when that alone
  costs more than the budget (the output says so). Then it adds more in rounds, fastest
  first: round one is the preferred aircraft and theater of each kind of scenario (the
  F/A-18D, Ukraine, the lowest airport number), round two a second aircraft or theater
  (for example the Rafale, or a second theater). No family gets more than 12, and no extra
  scenario may take more than a third of the budget by itself.
- **Headless by default.** Windowed scenarios run only when a changed file touches
  rendering or windowed input (shaders, renderers, the HUD and instruments, the menus,
  input handling, the scripts the windowed runs use) and the budget allows, through
  `tools/agent-run.sh` as always. `--with-windows yes` forces them, `no` forbids them.
  A family whose scenarios all open a window (damage and ejection, for instance) is
  reported as needing a window rather than silently dropped.

`--plan` prints the whole choice and runs nothing. When you add a scenario, put it in a
family (and a file in `tools/` or `crates/` needs a rule); the unit tests in
`tools/test_battery_selection.py` fail until the map covers it.

### Before merge

The check list in [AGENTS.md](../../AGENTS.md) is the merge requirement. Where the
single-player guard exists on the machine (`.local/mp-baseline/`, local tooling that is
not in the repository), also record the full baseline with `run.sh` and compare it with
`compare.sh`: any difference in single-player output is a finding. The same folder has
`quick.sh`, the quick guard the quick check uses: about 32 of the same runs, without
timing or the test log, compared only against the same items of the canonical baseline,
in about half a minute.

### Before a release

Run the whole battery by hand, with the seeded AI fuzz games on:

```sh
TORE_AI_FUZZ=all python3 tools/battery.py --jobs 8
```

Then read the lane pages' lists of things that need a human eye or ear. Record the
result as a pass in [`docs/baselines/`](../baselines/) (see "Reports").

### The network matrix

The network code has its own matrix, a test rather than a battery lane: a host and two
bots fly a scripted fight on the network simulator for each round trip (50, 150 and
300 ms) and each loss (0, 2 and 5 percent each way, with 1 percent duplicates), and every
limit of the [netcode acceptance table](../MULTIPLAYER.md#netcode-numbers) is measured
and asserted. It uses the synthetic fixtures, so it needs no import and runs anywhere.

```sh
cargo test --locked -p tore-session --lib matrix_tests                      # 60 simulated seconds a cell, in the normal suite
cargo test --locked -p tore-session --lib matrix_tests::full -- --ignored --nocapture   # five minutes a cell, before a release
```

The short form is part of `cargo test --workspace` (about 12 seconds on a quiet
machine); the full form takes about 2 minutes 15 seconds at four cells at a time and
prints one line of figures per bot. The CI job `Network loopback`
(`.github/workflows/network.yml`) runs a host and two real `tore-bot` processes over
loopback UDP on Linux, Windows and macOS. The measurements with real data, which need an
import, are the ignored `host_players` test; see [the baseline](../baselines/net-2026-09-30.md)
for the commands and results.

## Running it

Build once, import the retail media once into a profile folder, then run:

```sh
cargo build --locked -p tore-app
TORE_DATA_DIR=.local/bugbash-data target/debug/tore-app --import gameassets/fighters-anthology --import-only
python3 tools/battery.py --list
python3 tools/battery.py --lane ai --jobs 8
python3 tools/battery.py --scenario 'ai-fight-15v15*'
python3 tools/battery.py --changed --budget 120   # only what your changes can affect
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
unique across lanes; the runner refuses duplicates. Also make sure the scenario's name
matches a family in `tools/battery_selection.py` (see "Choosing battery scenarios by
change"), or the selection map's unit test fails.

## Reports

Measured evidence of a battery pass goes in [`docs/baselines/`](../baselines/),
one file per pass, following the baseline rules in [AGENTS.md](../../AGENTS.md).
The first pass is [battery-2026-09-28](../baselines/battery-2026-09-28.md).
