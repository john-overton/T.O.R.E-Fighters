# Projectile cap raised to 5,000

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-10-09, branch `fix-projectiles` from `95d402a0`.

## What changed and why

The battery's fuzz seed 317 (`ai-fuzz-0317`: an A-4E and an Su-25 against four
experienced MiG-21s, guns only, over France) failed with "dropped launches: 29".
Four MiG-21s fired 624 cannon rounds and the game threw 29 away, because no
more than 256 rounds, missiles and bombs could be in flight at once and an AI
gun's queued rounds were held to the same number. John's decision, 2026-10-09:
raise the cap to 5,000.

| Limit | Before | After |
| --- | --- | --- |
| Rounds, missiles and bombs in flight (`live::MAX_PROJECTILES`) | 256 | 5,000 |
| One AI gun's queued rounds | 256 | 5,000 (the same constant) |
| A network player's drawn gun rounds, own and others' each | 256 | 5,000 (the same constant) |
| Projectiles in one recording frame | 1,024 | 8,192 |
| Projectile records in one network snapshot | 256 | 256, unchanged: a per-packet limit, and gun rounds never travel as records |

No byte of any format changed. A recording frame's projectile count is a
variable-length number, so the replay format stays version 2; a build from
before this change refuses a frame with more than 1,024 projectiles. The
network protocol and the checkpoint coding are unchanged (a checkpoint list
may hold 16 million items).

The [architecture](../ARCHITECTURE.md#projectiles-in-flight) describes the
three shortcuts that keep a full sky affordable. Each skips work that cannot
change an answer, so every fight below flies exactly as before.

## Machine and method

AMD Ryzen 9 7900X, Linux, release builds with debug information, run under
`nice -n 10` one at a time while John used the machine, so single runs vary
by up to about 30 percent. "Before" is `95d402a0`; "after" is this change.
Both carried a local, uncommitted measurement patch to the headless AI probe
that times each tick of the probe loop and, with `TORE_PROBE_FLOOD=5000`,
keeps the sky topped up to 5,000 rounds by copying fresh AI cannon rounds with
small changes of direction (`.local/tmp-fixproj/timing.patch`). Times are
milliseconds a tick, the median of three runs with the lowest and highest in
brackets; p99 is the slowest tick in a hundred. The fixed tick is 8.3 ms.

## Ordinary fights

| Fight | Ticks | Most rounds in flight | Before mean | Before p99 | After mean | After p99 |
| --- | --- | --- | --- | --- | --- | --- |
| Seed 317, 1 against 4, guns only | 14,400 | 256 before, 284 after | 0.85 [0.75, 0.99] | 3.70 [3.09, 4.07] | 0.27 [0.26, 0.29] | 0.98 [0.88, 1.28] |
| `ai-long-guns-3v3`'s fight | 21,600 | 133 | 0.51 [0.38, 0.53] | 2.80 [1.43, 3.01] | 0.27 [0.25, 0.38] | 0.81 [0.71, 1.28] |
| `ai-long-15v15`'s fight | 21,600 | 52 | 1.02 [0.96, 1.20] | 4.25 [3.93, 4.42] | 0.78 [0.69, 0.79] | 2.80 [1.93, 3.36] |
| Seed 23, 15 against 14, guns only | 14,400 | 37 | 1.77 [1.53, 1.96] | 3.29 [2.34, 3.40] | 1.10 [1.05, 1.68] | 1.87 [1.86, 4.24] |

The last three fly identically before and after: every line of the probe's
output, the position checksum among them, is the same. Seed 317 changes,
because its rounds are no longer thrown away: 665 shots and none dropped,
against 624 and 29. Every fight got faster, mostly from the runway shortcut,
which every AI aircraft's ground queries use.

## A full sky

With the sky held at 5,000 rounds from the first burst on (the measurement
patch), over the seed 317 fight for 7,200 ticks:

| AI aircraft | Cap raised, no shortcuts | After |
| --- | --- | --- |
| 4 | mean 17.3, p99 72.2, slowest 79.6 (one run) | mean 1.37 [1.17, 1.72], p99 5.66 [4.97, 6.94], slowest 7.9 |
| 22 (8 against 15) | not measured | mean 2.89 [2.87, 3.02], p99 17.6 [17.1, 17.8], slowest 30.3 |

Without the shortcuts, a full sky cost about 7 microseconds a round a tick in
the contact search (every round tested against all 180 targets, ground objects
included) and about 0.8 microseconds a round for each AI aircraft's
incoming-fire check (nine ground samples along its sight line to each tracer,
each scanning every runway). After, the contact search costs about 0.07
microseconds a round. What remains grows with the AI aircraft times the
rounds: each AI aircraft still looks at every round for incoming fire, so with
22 AI aircraft under a full sky the average tick fits the 8.3 ms budget but
the slowest take about two. A full sky needs about 60 guns firing together;
the ordinary fights above stay under 300 rounds.

## Sizes

| What | Size |
| --- | --- |
| A round in memory | 840 bytes, plus an AI round's own copy of its weapon record's names |
| A round in a checkpoint | about 93 bytes (256 rounds 23,693 bytes, 5,000 rounds 466,039 bytes, one shared weapon record) |
| Seed 317's recording, 14,400 ticks | 5,240,416 bytes before, 5,391,378 after (more rounds kept) |
| A recording of a full sky, 7,200 ticks | 64,259,458 bytes: about 5.4 bytes a round a frame, 27 KB a frame or 3.2 MB a second at 5,000 rounds |

The peak resident memory of the probe did not move (438 MB). Nothing allocates
by the cap: every list grows with the rounds actually in flight. A full sky
would fill a recording's 1 GiB file limit in about five minutes, and adds about
0.47 MB to a host's checkpoint for its standbys.

## Validation

- The contact search's first pass keeps every pair the exact tests can report:
  randomized tests against the gun volume, fuzed spheres and ground boxes
  (`combat/live/broad.rs`). The runway shortcut matches the rotated test on
  200,000 random boxes and points, corners included (`airport.rs`). The
  ground ceiling never clears a line that touches the terrain or a sloped
  runway, on and off the grid (`terrain.rs`).
- The four fights above, and the full-sky runs with the shortcuts added one at
  a time, print the same output before and after each shortcut.
- Linux golden fingerprints: all 428 lines of `TORE_GOLDEN_VERBOSE=1` from
  `tore-sim`'s golden tests and `tore-world`'s tick tests are the same before
  and after. None of their scenarios reached 256 rounds.
- Checks: formatting, clippy with warnings as errors, the workspace tests
  (4,745 passed, 69 ignored), the tools' Python tests, the documentation and
  asset checks.
- Battery, debug build: `--changed 95d402a0` 48 of 48; the 100 seeds
  `ai-fuzz-03*` (`TORE_AI_FUZZ=all`) with `ai-long-*`, `ai-big-*`,
  `ai-record-*`, `replay-rec-*`, `replay-tape*` and `replay-combat-smoke*`,
  250 of 250, no dropped launch in any of their 235 probes.
- Battery, release builds, all 400 fuzz seeds with `ai-long-*`, `ai-big-*`,
  `ai-record-*` and `ai-guns-*`: before, 438 of 440, and only seed 317 and
  its copy `ai-guns-mig21-pack` dropped rounds (29 each); after, every fuzz
  seed and `ai-guns-*` 411 of 411, none dropped.

Not run: windowed scenarios, the net lane and network loopback tests (no
wire change), Windows and macOS (the golden values recorded on Apple silicon
should not move, since the Linux ones did not), and a full sky in a networked
game.
