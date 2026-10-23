# Multiplayer debrief and statistics validation, 2026-10-23

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. This validates the requested [mission record](../spec/debrief.md#multiplayer-mission-record),
including both follow-up requests. The starting T.O.R.E revision was
`b87f1ebc66285c53cc814214849bfb5df1e208c5`, protocol 67. The change uses protocol 69.
Validation ran on Linux with the pinned Rust toolchain and locked dependencies.
Retail game behavior was not compared.

## Reproduction and live result

Both builds ran an isolated dedicated co-op server, unlimited revival, a
windowed F/A-18D client through `tools/agent-run.sh`, an observer bot and dummy
enemies. The client fired, ejected, revived, fired again and left before the
mission ended. All ports were allocated on loopback; the production servers
were outside the test.

| Observation | Starting revision | Changed build |
| --- | --- | --- |
| Player kills on K after two lives | 2 | 2 |
| PLAYER kills in the early-exit debrief | 1 | 2 |
| Hit totals on K and SCORES | No hit column | 4 |
| Losses, sorties, revivals in the partial report | No sortie history | 1 loss, 2 sorties, 1 revival |
| Early report identification | Five-page ordinary debrief | PARTIAL REPORT, departure time and mission continued |
| Human flight position after revival | Friendly 1-1 became 1-2 | Friendly 1-1 stayed 1-1 |
| AI flight position for Enemy 1-2 | Enemy 1-2 returned as 1-6 | Enemy 1-2 stayed 1-2 |
| Departed player on the remaining observer's scores | Removed | Kept as left, 2 kills and 1 loss |
| AI taking over the departed player's aircraft | Earlier human score not listed | Separate AI row gained 0.98 aircraft of damage; human contribution stayed |

This reproduces the revival-related loss of kills. The original host already
sent a personal Debrief on a clean leave, but its per-aircraft capture omitted
previous lives. Results were sent only at mission end. The original app had
no client-side fallback when a connection ended without a Debrief. John's
specific session log was unavailable; the reproduced bug fits his report of
leaving just after revival, but his exact five or six kills were not verified.

Evidence is local, not committed retail art. The task scratch directory is
`/home/john/Development/T.O.R.E-Fighters/.local/next-round/debrief-fix/`:

- Baseline capture and observer log: `battery/20261023-101015-before-final/`.
  Its expected failures are the absent partial report, departed-player row
  and AI score rows. `shots/11-kills.png` under its window scenario shows one
  kill; the observer log records two on K.
- Final capture: `battery/20261023-110102-final/`. Its window scenario's
  `00-second-score.png`, `01-report.png`, `02-report.png`, `03-report.png` and
  `11-kills.png` show K, the partial heading, sorties, SCORES and KILLS.
- The earlier live battle `battery/20261023-091648-after/` demonstrated AI
  kills reaching the default five-kill PvP limit before the one-minute time
  limit. The time-limit-only fixture now explicitly disables the kill limit.

## Checks

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked --no-fail-fast` | 7,046 passed, 0 failed, 104 ignored by default |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo fmt --all -- --check` | Passed |
| `cargo build --workspace --locked` | Passed |
| Python tools tests | 504 run, 5 skipped, no failures |
| Source and app/extract binary asset checks | Passed |
| Documentation checks | Passed |
| Retail-art debrief layout test, explicitly run with imported profile | Passed for all sample pages and columns |
| Windowed GPU smoke through `tools/agent-run.sh` | Passed |
| Final live partial-debrief, score and result scenarios | 3/3 passed |
| Live revival and AI-respawn scenarios | Passed |
| Final live host-loss migration | Passed; `battery/20261023-111533-migration-final/` |

The regressions exercise three human lives, cumulative kills/hits/losses,
early leave without final Results, connection loss without any Debrief,
normal completion, departed players after token expiry, AI takeover,
AI respawn and scoring, host handover with a departed player's contribution
still reaching the kill limit, state serialization, stable labels, and the
single-player debrief's existing behavior. They also prevent an old player
wreck appearing as the player's wingman and check scoreboard visibility over
opaque instruments. K's new paging keys are in the generated controls list.

Commands used `CARGO_BUILD_JOBS=8`, `nice -n 5`, and battery `--jobs 4 --windows 1`.
The new scenario is `net-window-partial-debrief`; the score scenario also
requires a nonzero AI kill, and the window scenario checks retained human and
AI position numbers. Those added assertions were checked against the final
live logs.

## Remaining validation limits

A planned-handover probe resumed all three clients and delivered final results,
but later backup-host checkpoint continuity warnings made that battery run
fail (`battery/20261023-101016-handover-isolated/`). The unmodified starting
revision also failed the planned-handover probe, with resume refusals
(`battery/20261023-104553-handover-baseline/`). These are different symptoms;
the comparison does not prove that the checkpoint warning has the same cause.
This separate handover reliability issue is not claimed fixed. The dedicated
statistics handover regression and live host-loss migration passed.

Windows and macOS builds were not run. Default-ignored hardware and extended
retail tests were not all run. A dropped connection can only report data
already received; the partial report names the latest statistics time. Its
history and display bounds, and the AI individual-limit and succession choices,
are recorded in the behavior specs.
