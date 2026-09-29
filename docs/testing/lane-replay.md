# Battery lane: replay

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The replay lane tests everything that surrounds flight rather than the flight itself:
mission recordings and the viewer, the exports (info, log, summary, Tacview, diff), radio and
crew lines, audio start-up, controller input and profiles, input tapes, start-up
diagnostics, first-run import errors, and the game's key handling. Its scenarios live in
`tools/battery_scenarios/replay.py` and the `_replay_*.py` modules beside it.

## Running it

```sh
cargo build --locked -p tore-app
python3 tools/battery.py --lane replay --jobs 5 --windows 2 --tag replay
python3 tools/battery.py --scenario 'replay-rec-*'          # headless recordings only
python3 tools/battery.py --scenario 'replay-keys-*' --windows 2   # key-pressing scenarios
```

The whole lane is 305 scenarios (37 of them open a window) and takes about 21 minutes on the dev
machine with five jobs and two windows. The recording, corrupt-file, option, snapshot and input scenarios are
headless and take a few seconds each. Anything that opens a window goes through
`tools/agent-run.sh` and counts against `--windows`. The import scenarios copy the local retail
media with a copy-on-write copy (no extra disk space on btrfs) and need `gameassets/` present.

## What each group checks

| Group | Scenarios | What has to hold |
| --- | --- | --- |
| `replay-rec-*` | Fights 1v1 to 15v15 (and 1v15, 15v1, 4v9), every aircraft as enemy and as player, four skills by three geometries, six mission presets, guns, missiles with chaff and flares, three threat kinds, nine faults, ground starts at three airports with wings of 1, 3 and 5, four wing orders | `--verify-render` passes with nothing missing or differing. `--recording-info` says finished normally with no problems and lists the probe's aircraft. `log.jsonl` is valid JSON with one header, unique aircraft ids and monotonic time. `summary.txt` has its sections and one line per aircraft. The Tacview file has a header, monotonic time, no NaN, no object back from the dead and one object per aircraft. A recording diffed against itself matches. For the fights, guns and missiles a second identical run makes a recording that matches the first, and the probe prints the same output with recording off |
| State checks on every log | (inside the above) | Alive with zero hit points, dead with hit points, more hit points than the maximum, negative fuel, outside the world, over 20 G, a launch after the shooter was lost, two outcomes for one shot, a decoy count that goes up or below zero, an aircraft that comes back to life, a delivered radio line with no speaker or text, the same line twice within 0.2 s, an aircraft speaking more than half a second after it was lost |
| `replay-corrupt-*` | Thirteen kinds of damage to a real recording: empty, 100 bytes, text, random bytes, bad magic, future version, half and a third of the file, missing tail, one byte short, flipped bytes, zeroed block, duplicated tail | Unreadable files exit 1 with a clear message. Readable but damaged files still export and always say `Problem`. No panic, no bare error |
| `replay-watch-corrupt-*` | The same damaged files given to the viewer (windowed) | Unreadable ones exit 1 with the same messages. Damaged ones open, draw a real frame and exit 0 |
| `replay-view-*`, `replay-ticks-*`, `replay-aircraft-*`, `replay-drone-*`, `replay-ui-*`, `replay-panels`, `replay-menu-pages` | About 140 captured frames: all twelve flight views, first tick, second, last, one past the end and far past it, each aircraft, the drone, look-at objects (including missing ones), every interface part, every debug panel, all seven Escape menu pages | Every frame is a real picture (not blank, not black, the timeline strip present unless hidden), the run reports the capture, and frames that should differ do differ. A person still has to look at them (see below) |
| `replay-speed-*` | 1/8x to 16x forwards and backwards with `TORE_PERF_FRAMES` | The timing report prints |
| `replay-live-*` | Real windowed flights that record themselves: every aircraft in free flight, three Quick Mission separations, ground starts at three airports with wings of 1, 3 and 5, with and without sound | The recording is the only one in `replays/`, finished, reads back through info, log, Tacview and diff, and passes the same state checks. Ground starts must produce tower radio |
| `replay-keys-*` | Key presses sent to the game's own window: bookmarks, end flight (Ctrl+Q), chaff and flares, target designation and fire, Alt+F4 over the controls, graphics and sound screens (flight and viewer), and 17 seeded random key sequences in free flight, Quick Mission, ground start and the viewer | The game never crashes or hangs, exits on Alt+F4, and the recording it made is finished and valid with no pause without a resume. The chaff and flare keys leave four countermeasure events from the player |
| `replay-cli-bad-*` | 22 wrong options and paths | Exit 1 with a message that names the option and what was typed; no panic; no bare parser text |
| `replay-snapshot-*` | 22 menu screens from `--snapshot-state`, including replays, controls, sound, graphics and the locate screens | A real picture |
| `replay-diag-*` | `--diagnostics-self-test` in every mode and `tools/check_startup_diagnostics.py` | Right exit codes, right messages, the report and session log written |
| `replay-audio-*` | Start-up with sound on and the audio device missing or wrong (`PULSE_SERVER`, `PIPEWIRE_REMOTE`, `ALSA_CONFIG_PATH` pointed nowhere), and a replay played with sound | Never a crash. A missing device is reported as `Continuing without audio` |
| `replay-input-*`, `replay-tape-*` | `--list-inputs`, `--write-input-profile` then load it, refusing to overwrite, nine kinds of bad profile, eight kinds of bad tape, hand-made pilot input tapes replayed twice on every aircraft | Profiles that cannot load fail with a message that names the file. Tapes that cannot load exit 1 with a message. Two replays of one tape give identical results on all fourteen aircraft |
| `replay-import-bad-*` | Eleven kinds of bad game media given to `--import` in a fresh data folder | Exit 1 with an actionable message, nothing left behind. A damaged archive names the file and says what to do |
| `replay-combat-smoke-*`, `replay-validate-creator` | The weapons and creator acceptance probes | Known failures, see below |

## Bugs found and fixed

| Symptom | Cause | Fix |
| --- | --- | --- |
| Alt+F4 did nothing while the controls, graphics or sound screen was open in flight, so the game could not be quit from the keyboard there (found when a random key sequence opened the controls screen, in the fuzz scenario) | Those screens take every key press first and the quit shortcuts were only checked after them, against the controls screen's own note and INPUT.md | `10663de`. The quit shortcuts are checked first. Regression scenarios `replay-keys-alt-f4-*` (three fail without the fix) |
| A flight started with `--free-flight` never recorded, even with `TORE_RECORD_MISSIONS=1`, against REPLAYS.md and DEVELOPMENT.md | Only the menu's Free Flight action started the recorder | `c0558da`. A direct start records like a menu start. `replay-live-*` cover it |
| The debug panels said "Mission recording is off (TORE_RECORD_MISSIONS=0)" in runs where that variable was not set | One message for every reason recording is off | `c0558da`. It lists the real reasons |
| A truncated or empty `FA_1.LIB` or `FA_2.LIB` ended an import with "invalid archive sentinel" or "failed to fill whole buffer" | The reader's error was passed up unchanged | `b6ce6d9`. The message names the file and says to copy it again or reinstall. Unit test in `media_source.rs` |
| Recordings, summaries and Tacview files called the F-22N and the F/A-XX "F-22" | Both borrow the F-22's PT name | `441ecab`. They use their own labels, F-22N Raptor and F/A-XX |
| The ground start hint said "PageUp adds throttle", but PageUp stopped setting throttle when the Fighters Anthology keys replaced the development ones on 2026-09-26 | The text was never updated | `ddba864`. It names key 5, full throttle. Checked by the ground start `replay-live-*` scenarios |
| An input profile that could not load (empty, wrong header, bad action, bad bytes) did not say which file | The parse error was passed up without the path | `9e1feb6`. Every profile error starts with the path |
| Name labels of aircraft close together in the replay viewer printed on top of each other and could not be read | Each label was placed at its aircraft with no regard for the others | `e3a97f7`. Labels stack upward until clear |
| The mission summary reported a parked wingman as "airborne 0:20.1" for the whole flight | The airborne flag means "in play" and stays set on the ground | `98db86d`. Only frames off the ground count. Unit test in `tore-replay` |
| A cockpit message from the game (the ground start hint) read "someone: ..." in the transcript | A missing speaker fell back to "someone" | `b0a0007`. Cockpit messages read "cockpit". Two text golden files updated |
| `--rate abc`, `--ids a,b`, `--from x`, `--replay-tick -5`, `--replay-speed fast`, `--replay-aircraft x` said "invalid float literal" or "invalid digit found in string" | Parser errors passed up bare | `f4e2a9d`. They name the option and what was typed |
| `--recording-log --out FILE` and `--recording-acmi --out FOLDER` said "File exists (os error 17)" or "Is a directory" | Bare I/O errors | `cb1fd77`. They name the path and what it was used for |

## Found and not fixed

- **`--combat-smoke` fails on 13 of the 14 aircraft** (only the MiG-29 passes), each at a
  different check: "radar-off launch was not inhibited" (F/A-18D, X-31, Su-27, F-22, F-22N,
  F/A-XX; the launch does go off with the radar off because boresight and fire-and-forget
  guidance no longer need it), "source guidance probe did not launch" (Rafale C, Su-25, MiG-23),
  a gun kill expectation (F-14D, MiG-21), "automatic damage/destruction failed" (A-4E) and an
  ammunition expectation (Su-35). With `TORE_COMBAT_EVIDENCE` set the F/A-18D also stops at
  "serialized live-fire replay diverged before reset". The failures are on the checkout the
  battery started from, so they predate this lane; they look like the probe's expectations
  falling behind the 2026-09-28 damage rework (`cbde197`) and the missile pass (`261e8bf`), but
  no one has decided whether the probe or the game is wrong. The probe is not in CI. The lane
  keeps one scenario per aircraft marked as a known failure; each turns red when it starts to
  pass, as a reminder to remove the mark.
- **AI behaviour flags in the recordings, for the AI lane.** Across the lane's recordings the
  summary flags `ai_flipping` (an AI aircraft changing activity six times in a few seconds, 89
  times), `track_lost_early` (a missile losing its track within seconds of launch, 48),
  `control_oscillation` (a pitch control reversing eight times in under a second, 5) and
  `call_suppressed` (radio calls dropped by cooldown, 116, by design). None was investigated
  here.
- **`--validate-creator` fails** with "F18: damage region 3 at 0.1 has no distinct finite
  geometry". `aircraft.rs` deliberately draws surviving aircraft intact ("Temporarily keep
  surviving aircraft visually intact", `c6eef9d`, 2026-09-21), so the check that a damaged
  region changes the mesh cannot hold. Also not in CI, marked as a known failure.
- `--recording-log --from 20 --to 10` (from after to) and `--ids` with an id that is not in the
  recording write an empty log without saying so.
- `--import FILE.txt` names the file's folder in the "not a Fighters Anthology source" message,
  not the file.
- Several hundred other command-line numbers (for example `--ground-start x`) still give the
  parser's bare message; only the replay and recording options were changed.
- Running several windows at once, the head tracker warns "UDP 4242: Address already in use".
  Harmless.

## Needs a decision

- **Direct `--free-flight` recording.** The fix makes a direct start record. Say if a developer
  launch should stay unrecorded instead (the docs and `TORE_RECORD_MISSIONS=1` say it records).
- **The airborne flag's meaning.** In a recording `airborne` means "in play" and `on_ground`
  is separate, so a parked aircraft is both. The summary now counts only the frames off the
  ground; Tacview and the log still write the flag as recorded.
- **What the acceptance probes should require now** (`--combat-smoke`, `--validate-creator`):
  bring them up to the current behaviour, or change the game. See "Found and not fixed".
- **From after to** in `--recording-log`: reject it, or leave it silent.

## Needs a human eye or ear

- **Sound.** The lane proves that start-up with sound on works and that a missing device
  degrades to silence. It cannot judge how anything sounds: the radio and crew lines, tower
  voice, seeker and RWR tones, situation music, replay sound at 1x, and the volume mixer.
- **The captured frames.** The checks find blank, black and broken frames. They cannot judge
  taste, exact placement or art: the twelve views, name labels, trails, the timeline markers,
  the comms panel, the Escape menu pages. The agent that ran the lane looked at the views,
  interface parts, panels, menu pages and ground-start frames and found the label overlap
  above and nothing else. Frames after that fix were not all reviewed again.
- **Anything that needs the main menu.** The battery cannot click through the main menu, the
  Replays screen (Keep, Delete, Tacview, auto-delete), the debrief screen or the pause menu's
  buttons; these are covered by unit tests on synthetic recordings and by the snapshot
  screens only. Key presses reach a flight or the viewer, so "end mission" (Ctrl+Q) is covered,
  but Restart has no key and the debrief screen is not inspected.
- **Cheats.** They can only be toggled through the Escape menu. The unit tests cover the menu
  rows and the simulation; the battery does not exercise them in a live flight.
- **Live gameplay keys.** A window on a hidden workspace gets keys, but plain letters and
  digits had no effect while named keys (Insert, Delete, Space, F1 to F12, arrows) and Ctrl
  combinations did, so the gear, flaps, throttle, eject and sensor keys were not exercised in a
  real window, and no key can hold a stick. A hand-flown takeoff, landing, ejection and the
  tower's "airborne" and "good hunting" calls after a real departure were seen only in
  headless probes and unpiloted live flights.
- **Real controllers.** No gamepad or joystick was attached; profiles were tested as files.

## How the checks work

`tools/battery.py` runs each scenario's main command, then any `then` steps (more app runs, or
plain Python), and hands the work folder and the joined output to `check_work`. A scenario can
carry `known_failure`, which reports its failure as known and fails the run once it passes.
`_replay_checks.py` holds the Tacview, log and summary checkers (with unit tests in
`tools/test_replay_checks.py`). `_replay_tools.py` damages recordings, builds damaged game
media and tests captured frames. `_replay_drive.py` sends keys to a game window through
Hyprland by process id, so nothing else on the desktop sees them. A window on a hidden
workspace loses focus at the first key, which pauses a flight; the scenarios set
`TORE_PERF_ACTIVE=1` with a large `TORE_PERF_FRAMES` (which keeps a timing run unpaused) and
`TORE_RECORD_MISSIONS=1` (which makes it record). Game time then runs a little slower than the
clock. The driver reads the game's process use before Alt+F4 so a hang shows.
