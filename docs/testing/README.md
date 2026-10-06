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
| Before a release | Ad hoc, by hand, before tagging | The whole battery with `TORE_AI_FUZZ=all` (every lane, the [net lane](lane-net.md) among them), the lane pages' human checks, and the [network tests outside the battery](#network-tests-outside-the-battery) | About 85 minutes of wall clock at 6 to 8 jobs for the battery, plus about 90 seconds for the net lane and about 15 minutes for the network tests |

The pre-push hook stays as it is. The full battery and the seeded fuzz games are not
part of any routine: run them when a release is near, or when you change something
broad and want the whole picture.

### Large projects: commit locally, push at the end

John, 2026-10-06, for large project items (a milestone's remaining stages, a
long run of slices): work locally until the whole project is finished, then
test it as a whole and push once.

1. **Commit locally, in logical order.** Each slice lands on the project's
   integration branch as small commits that build, in the order the work
   depends on. Nothing is pushed while the project runs, so CI does not queue
   a run for every merge.
2. **Micro tests per change.** Each change runs only the tests for what it
   touched: the quick check, the touched crates' tests, the battery scenarios
   the change can affect, and the single-player baseline when it touches
   simulation code. Every new test is added to the suite and listed in the
   project's test ledger, so the final run covers it.
3. **When the project is finished, run the battery.** The whole battery with
   `TORE_AI_FUZZ=all`, the network tests outside the battery and the ignored
   tests the ledger lists, and the single-player baseline. Fix what it finds
   locally.
4. **Then push, and work through CI.** One push (the pre-push hook runs the
   check list), then fix whatever CI finds on the other platforms, each fix a
   further commit.

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

Run the whole battery by hand, with the seeded AI fuzz games on. It includes the `net`
lane, which needs `tore-server` and `tore-bot` built beside the game (see
[lane-net](lane-net.md)):

```sh
cargo build --locked -p tore-app -p tore-server -p tore-session
TORE_AI_FUZZ=all python3 tools/battery.py --jobs 8
```

Then run the [network tests outside the battery](#network-tests-outside-the-battery),
and read the lane pages' lists of things that need a human eye or ear. Record the
result as a pass in [`docs/baselines/`](../baselines/) (see "Reports").

### Network tests outside the battery

The `net` lane covers the real programs on one machine. The network code also has Rust
tests that the battery does not run: some are in the normal `cargo test --workspace`, and
the slow or data-reading ones are `#[ignore]`d. The full run before a release includes
all of them. Build in release for the ones marked so, run them one at a time on a quiet
machine, and keep `TORE_DATA_DIR` pointed at an imported data folder for the ones that
read the real import.

| Test | What it measures or proves | Command |
| --- | --- | --- |
| The matrix, short form | Every cell of the [netcode acceptance table](../MULTIPLAYER.md#netcode-numbers) over 60 simulated seconds: a host and two bots on the network simulator at 50, 150 and 300 ms and 0, 2 and 5 percent loss each way with 1 percent duplicates; synthetic data. In the normal suite (about 12 s) | `cargo test --locked -p tore-session --lib matrix_tests` |
| **The matrix, five minutes a cell** (ignored) | The same nine cells over five simulated minutes each: the acceptance itself; about 2 min 15 s at four cells at a time; one line of figures per bot | `cargo test --locked -p tore-session --lib matrix_tests::full -- --ignored --nocapture` |
| The five-minute prediction (ignored) | A joined client's prediction equals the host's at every snapshot for 300 simulated seconds | `cargo test --locked -p tore-session --lib the_prediction_equals_the_host_for_five_minutes -- --ignored --nocapture` |
| The five-minute fight (ignored) | Two bots fight for 300 simulated seconds at 150 ms and 2 percent loss | `cargo test --locked -p tore-session --lib two_bots_fight_for_five_minutes -- --ignored --nocapture` |
| A bot over real UDP (ignored) | One bot flies a minute of real time over a real socket on this machine | `cargo test --locked -p tore-session --lib a_bot_flies_a_minute_over_real_udp -- --ignored --nocapture` |
| Loopback | A host and two real `tore-bot` processes over real UDP; synthetic data. CI's job `Network loopback` runs it on Linux, Windows and macOS with 20 seconds of flight; the default is 6 | `TORE_LOOPBACK_SECONDS=20 cargo test --locked -p tore-session --test loopback -- --nocapture` |
| The strict real-time tests (ignored) | The game's hosting thread: no correction of the hosting player's plane, a 2 second window stall stalls nobody, an 8 second stall drops nobody and the King still reigns, a joined game stalled for 15 seconds recovers with no correction (CI-fix), each to the strict limits. CI's `strict-real-time` job runs them on Linux; the normal suite runs forms a starved runner meets ([why](../ARCHITECTURE.md#real-time-tests-on-shared-runners-ef-y)). Needs a machine whose sleeps are accurate: nothing else heavy running | `cargo test --locked -p tore-app --bin tore-app -- --ignored --exact --test-threads=1 --nocapture net::hosting::tests::a_hosted_mission_flies_with_no_correction_at_all net::hosting::tests::a_two_second_window_stall_stalls_nobody_strictly net::hosting::tests::an_eight_second_window_stall_drops_nobody_and_the_king_still_reigns_strictly net::session::keepalive_tests::a_joined_game_stalled_for_fifteen_seconds_is_kept_and_recovers_strictly` |
| The name lookup (ignored) | A name the system's resolver cannot resolve fails (it asks the system's resolver, so it needs a normal network setup) | `cargo test --locked -p tore-app --bin tore-app -- --ignored --exact net::lookup::tests::a_name_the_system_cannot_resolve_fails` |
| The local search (ignored) | `find_games_on_this_machine` looks at the real network | `cargo test --locked -p tore-app --bin tore-app -- --ignored --exact net::search::tests::find_games_on_this_machine --nocapture` |
| Host cost and bandwidth with real data (ignored) | The host's time per tick and per human, and bytes each way per player, with 0, 2, 8, 15 or 30 bots on a 15 against 15 mission; release, one count at a time; the figures and commands are in [the baseline](../baselines/net-2026-09-30.md) | `TORE_DATA_DIR=$PWD/.local/DATA TORE_MEASURE_BOTS=15 cargo test --release --locked -p tore-session --test host_players -- --ignored --nocapture` (`TORE_MEASURE_OPEN=all` above 15 bots) |
| The host's empty cost (ignored) | A 15 against 15 mission with nobody connected against the 20 percent of one core budget, and the same mission stepped alone for comparison; ten simulated minutes; release | `TORE_DATA_DIR=$PWD/.local/DATA cargo test --release --locked -p tore-session --test host_load -- --ignored --nocapture` |
| Bytes per snapshot (ignored) | Bytes per snapshot on a 15 against 15 mission, against the plan's bandwidth budget; release | `TORE_DATA_DIR=$PWD/.local/DATA cargo test --release --locked -p tore-session --test bandwidth -- --ignored --nocapture` |
| The bot's pursuit on real data (ignored, slice BOT) | `bot::tests::real_dummies_fall_to_a_bot_at_every_separation_and_height`: a bot shoots down a dummy MiG-29 within a minute at 2, 5 and 10 miles, 5,000, 20,000 and 40,000 feet, on a clean link and a 40 ms one (at most one of the 18 may miss); `bot::tests::real_guide_mission_pvp_ends_by_the_kill_limit`: two bots on the guide's mission 5 nm apart, AI hostile, the kill limit ends it within a minute on links of 0 to 120 ms and with the second bot joining up to 5 seconds late. About a minute each on the simulator | `TORE_DATA_DIR=$PWD/.local/DATA cargo test --locked -p tore-session --lib bot::tests::real_ -- --ignored` |
| The multiplayer debrief pages with the retail fonts (ignored) | Draws the SCORES and RESULTS pages for 30 aircraft (even, lopsided and all-human missions) on the retail clipboard and checks every cell against its column in the retail fonts; with `TORE_CREATOR_DUMP` it writes each page as a picture | `TORE_DATA_DIR=$PWD/.local/DATA cargo test --locked -p tore-app --bin tore-app -- --ignored --exact debrief::net_pages::retail_art_draws_the_multiplayer_pages` |
| Joining through the master in the game | The game's own session joins a game listed on a real master on 127.0.0.1 from the Internet Lobby screen, directly and through the relay only, each seated and flying, with the Messages lines in order; a relayed game kept through a 7-second stall by framed keepalives; a refused relay and a refused introduction in plain words; the rules for asking for the relay and giving up (slice J5). In the normal suite, about 14 s | `cargo test --locked -p tore-app --bin tore-app join_tests` |
| The standby's replay | A standby fed the crowd fight's records in process (slice K2): warm, equal to its source at every Check over a minute; cold, restored at ten moments and replayed to the end; a damaged chunk refused and asked for again; starved of time it goes cold and comes back warm; a mismatch asks for a checkpoint; the worker thread takes over and stops cleanly; synthetic data. In the normal suite, about 7 s | `cargo test --locked -p tore-session --lib standby::tests` |
| **The standby's five minutes** (ignored) | The same warm standby over 300 simulated seconds: 60 Checks, each equal, and the takeover's world the source's; about 30 s in a debug build | `cargo test --locked -p tore-session --lib a_warm_standby_equals_its_source_at_every_check_for_five_minutes -- --ignored` |
| The journal and the session's state | A host and three bots fly a crowd fight on the simulator with an `ai-slot` revival, scoring on and a late joiner; the journal through the standby stream's coders replays over the Flight's world to the host's checkpoint every 30 ticks, as do checkpoints at three ticks with the journal from each; every state part restores into a fresh host (slice K1). 40 simulated seconds in the normal suite, about 19 s | `cargo test --locked -p tore-session --lib host::journal` |
| **The journal, five minutes** (ignored) | The same over 300 simulated seconds, checkpoints at four ticks: slice K1's acceptance; about 105 s in a debug build | `cargo test --locked -p tore-session --lib a_five_minute_crowd_fight -- --ignored --nocapture` |
| **Ten minutes of the observer screen's recording** (ignored) | Ten minutes of 30 aircraft through the observer screen's feeder and store: the file's size (16 MB measured) and how long the viewer's read of it takes (0.18 s in a debug build), against the store's three-times rule (slice F2-O2); release for the real figure | `cargo test --locked -p tore-app --bin tore-app -- --ignored --exact net::observe::tests::ten_minutes_of_thirty_aircraft_are_read_in_the_time_the_store_allows --nocapture` |
| Host selection | The peers router along every row of the punching table; a host and its players' games on the simulator, each with its router: reach tests, ranking by each measure in turn, a relayed player never chosen nor pinnable, the pin and its fallback, the warnings' words, an upload test at half and at the full need, the punching table end to end (slice K6); synthetic data. In the normal suite, about 3 s | `cargo test --locked -p tore-net --lib peers` and `cargo test --locked -p tore-session --lib -- succession candidate` |
| **The CPU threshold on real data** (ignored) | The CPU measure against the busiest minute of five on the 15 against 15 mission, three runs; the ratio `CPU_BUSY_PER_MILLE` is fitted from ([baseline](../baselines/host-selection-2026-10-05.md)); about 75 s in release | `TORE_DATA_DIR=$PWD/.local/DATA cargo test --release --locked -p tore-session --lib cpu_measure_against_the_busiest_minute -- --ignored --nocapture` |
| The host's standby stream | A host and its bots fly the crowd fight on the simulator, two of the bots' games standing by in process (slice K3): a warm and a cold standby marked in the lobby, the warm one equal at every Check, the cold one's checkpoints each within its pace, both holding the host's world; a forced mismatch resynced; two failed checks going cold; standbys appointed in flight ready within their checkpoint's pace; a leaving standby replaced and one behind dismissed for the flight; a dedicated server and a relayed player standing by for nobody; synthetic data. In the normal suite, about 15 s | `cargo test --locked -p tore-session --lib host::standby` |
| **The standby stream, five minutes** (ignored) | The crowd fight with a warm and a cold standby over 300 simulated seconds: slice K3's acceptance | `cargo test --locked -p tore-session --lib a_warm_and_a_cold_standby_follow_a_five_minute_crowd_fight -- --ignored --nocapture` |
| **The standby stream of thirty humans** (ignored) | Thirty bots in PvP for a minute: the stream's bytes a seat a tick and a second, warm and cold, both standbys holding the host's world; release | `cargo test --release --locked -p tore-session --lib the_stream_of_thirty_humans_is_measured -- --ignored --nocapture` |
| **The standby stream on real data** (ignored) | The real 15 against 15 mission with four humans and a warm and a cold standby for five minutes (`K3_SECONDS` to shorten): the stream's bytes, each cold checkpoint's time against its pace, the busiest second on the wire, both standbys holding the host's world; release | `TORE_DATA_DIR=$PWD/.local/DATA cargo test --release --locked -p tore-session --lib real_data_15_against_15_with_a_warm_and_a_cold_standby -- --ignored --nocapture` |
| The listing end to end | A host's rendezvous against the real master (`tore_master::Master`) on the simulator, browsed by the Internet Lobby's client: listed within its first exchange, summary changes within 5 seconds, ten minutes within the master's limits with the mapping test paired, gone within 90 seconds of vanishing, a master restart healed within one heartbeat, a quiet master asked at most once a second; and a real `tore-server` with `broadcast on` against a master on 127.0.0.1 (slice I3). In the normal suite, under a second | `cargo test --locked -p tore-server listing_test` |

The matrix uses the synthetic fixtures, so it needs no import and runs anywhere; its
short form is part of `cargo test --workspace`. The measurements with real data need
an import. `tore-app`'s other `#[ignore]`d tests (the timing and mock-screen render ones in
`widgets/mock_screen.rs`, `net/chat.rs`, `net/lobby_chat.rs`, `direct_screen/tests.rs` and
`lobby_screen/tests.rs`) are developer tools that write pictures or print timings and
assert nothing about the network, so the full run leaves them out.

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
`tools/battery_scenarios/`, and one page here. A scenario is one run of the game, or, in the
`net` lane, a Python driver that starts and feeds several programs ([lane-net](lane-net.md#how-a-scenario-works)).

| Lane | Covers | Scenario files | Page |
| --- | --- | --- | --- |
| `menus` | Menu screens, the Quick Mission creator, the loadout page, captured screens, terrain and weather captures, text decoding, the retail manual audit | `menus.py` | [lane-menus](lane-menus.md) |
| `flight` | Ground start, takeoff, landing, every aircraft's flight, weapons, jettison, countermeasures, damage, ejection, environment | `flight.py` | [lane-flight](lane-flight.md) |
| `ai` | One against one up to fifteen against fifteen, every theater, missions, skills, damage, wing orders, invariants on every tick | `ai.py` | [lane-ai](lane-ai.md) |
| `replay` | Recording, playback, the Replays screen, radio and crew comms, audio start-up, input, hand-flown and mouse scripts, cheats, import errors | `replay.py` and `_replay_*.py` | [lane-replay](lane-replay.md) |
| `net` | Multiplayer on one machine over real UDP: the dedicated server (`--check`, a flown fight, chat, the console), the local-network search, a joined game that stalls, a hosted game, host migration with bots that host (`tore-bot --host`) and stand by (`--standby on`). Each scenario runs several programs (`tore-server`, `tore-bot`, `tore-app`) from a Python driver | `net.py` | [lane-net](lane-net.md) |

Each lane page lists what the lane covers, how long it takes, every bug found and
fixed, the known failures, the behaviours that need a decision and the things
that need a human eye or ear. Run one lane with `python3 tools/battery.py --lane
NAME --jobs 6`; a full pass of all five takes roughly two hours on a 24-thread
machine with a debug build (the `net` lane is about 80 seconds of it).

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

Add a `Scenario(...)` to the lane's file (a multiplayer scenario goes in `net.py` as a driver,
see [lane-net](lane-net.md#how-a-scenario-works)), run it alone with
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
