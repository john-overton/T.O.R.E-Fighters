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

The whole lane is 464 scenarios (118 of them open a window) and takes about 45 minutes on the
dev machine with five jobs and two windows, of which `replay-validate-creator` alone is about
eight minutes. The last full run passed all of them except that scenario, which timed out under
the old 10 minute limit and now has 30 minutes; the combat-smoke scenarios were known failures then and pass now that the
flight lane brought the probe up to date. The recording, corrupt-file, option, snapshot and input scenarios are
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
| `replay-view-model-*` | Each of the fourteen aircraft as the player and as the enemy, seen from outside in the viewer | A real picture. The agent looked at all fourteen: each shows its own model, and the F/A-XX differs from the F-22N |
| `replay-flight-panels-*` | The debug panels (thought, telemetry, guidance, comms, right-click menu, all together) over a live flight in the air and on the ground, by GPU capture | A real picture |
| `replay-speed-*` | 1/8x to 16x forwards and backwards with `TORE_PERF_FRAMES` | The timing report prints |
| `replay-live-*` | Real windowed flights that record themselves: every aircraft in free flight, three Quick Mission separations, ground starts at three airports with wings of 1, 3 and 5, with and without sound | The recording is the only one in `replays/`, finished, reads back through info, log, Tacview and diff, and passes the same state checks. Ground starts must produce tower radio |
| `replay-keys-*` | Key presses sent to the game's own window: bookmarks, end flight (Ctrl+Q), chaff and flares, target designation and fire, Alt+F4 over the controls, graphics and sound screens (flight and viewer) and on the main menu, creator, terrain viewer, flight help, flight menu, flight map and first-run locate screen, and 17 seeded random key sequences in free flight, Quick Mission, ground start and the viewer (20 more seeds were run by hand once, all clean) | The game never crashes or hangs, exits on Alt+F4 (a random key may pick the menu's Exit to Desktop first, which is fine), and the recording it made is finished and valid with no pause without a resume. The chaff and flare keys leave four countermeasure events from the player |
| `replay-cli-bad-*` | 41 wrong options, paths and probe settings (numbers that are words, out-of-range rates and ticks, unknown presets, empty or missing output paths) | Exit 1 with a message that names the option and what was typed; no panic; no bare parser text |
| `replay-quick-restart-*` | Quick Mission launch and restart over 30 setups: five airports by three wing sizes, ten separations, five mission presets | The restart restores the accepted start (`Quick Mission restart: PASS`) |
| `replay-settings-corrupt-*` | A damaged preferences, graphics, sound, replays or controls file in the data folder, then a real launch | The game starts on defaults and the session log says which file was ignored; a damaged controls file stops the start with a message that names the file and says to fix or delete it |
| `replay-screen-auto-delete-and-delete` | The Replays screen on real files, reached from the viewer's End Replay | Auto-delete keeps the newest N and everything marked Keep, never touches files that are not recordings, and Delete asks first and removes only the chosen one |
| `replay-import-keeps-good-data-*` | A failed import into a data folder that already holds a good import | The existing pack and remembered source are unchanged and the menu still draws |
| `replay-snapshot-*` | 22 menu screens from `--snapshot-state`, including replays, controls, sound, graphics and the locate screens | A real picture |
| `replay-diag-*` | `--diagnostics-self-test` in every mode and `tools/check_startup_diagnostics.py` | Right exit codes, right messages, the report and session log written |
| `replay-audio-*` | Start-up with sound on and the audio device missing or wrong (`PULSE_SERVER`, `PIPEWIRE_REMOTE`, `ALSA_CONFIG_PATH` pointed nowhere), and a replay played with sound | Never a crash. A missing device is reported as `Continuing without audio` |
| `replay-input-*`, `replay-tape-*` | `--list-inputs`, `--write-input-profile` then load it, refusing to overwrite, nine kinds of bad profile, eight kinds of bad tape, hand-made pilot input tapes replayed twice on every aircraft | Profiles that cannot load fail with a message that names the file. Tapes that cannot load exit 1 with a message. Two replays of one tape give identical results on all fourteen aircraft |
| `replay-import-bad-*` | Eleven kinds of bad game media given to `--import` in a fresh data folder | Exit 1 with an actionable message, nothing left behind. A damaged archive names the file and says what to do |
| `replay-combat-smoke-*` | The weapons acceptance probe on each aircraft | All 14 aircraft pass (the flight lane fixed the probe) |
| `replay-validate-creator` | The creator acceptance probe | Passes |
| `replay-script-*` | Hand-flown windowed missions from `--input-script` files: a ground-start takeoff (afterburner, pulsed stick, gear and flaps up once airborne) on all 14 aircraft, the same with the tower's landing clearance, a takeoff that presses G at about 45 knots with the wheels down and must slide on its belly instead of taking off ([gear on the ground](../spec/gear-on-the-ground.md)), gear, flaps, hook, airbrake, afterburner and countermeasures on all 14 (hooks only on carrier types, no afterburner on the A-4E and Su-25), a held gun trigger, a missile fired at a designated target, an ejection, autopilot and NAV modes, a 6 G pull and roll, views, the pause menu and bookmarks, and all sixteen cheats on and off through the menu | The recording holds the events the keys should cause: device positions over time, exactly one chaff and one flare, launches from the player, `aircraft.ejected`, autopilot effects, three bookmarks, paired `system.cheat` events |
| `replay-script-*-mouse` | The mouse: the Replays screen's Keep, Tacview, Debug log, Delete with confirmation and auto-delete panel; the debrief's NEXT, PREV and OK; the Escape menu's tab, row, Keyboard shortcuts, Restart and Resume | Files written and removed, settings saved, five distinct debrief pages, a restart recording pair, and every screen a real picture |

## Bugs found and fixed

| Symptom | Cause | Fix |
| --- | --- | --- |
| Alt+F4 did nothing while the controls, graphics or sound screen was open in flight, so the game could not be quit from the keyboard there (found when a random key sequence opened the controls screen, in the fuzz scenario) | Those screens take every key press first and the quit shortcuts were only checked after them, against the controls screen's own note and INPUT.md | `10663de`. The quit shortcuts are checked first. Regression scenarios `replay-keys-alt-f4-*` (three fail without the fix) |
| A flight started with `--free-flight` never recorded, even with `TORE_RECORD_MISSIONS=1`, against REPLAYS.md and DEVELOPMENT.md | Only the menu's Free Flight action started the recorder | `c0558da`. A direct start records like a menu start. `replay-live-*` cover it |
| The debug panels said "Mission recording is off (TORE_RECORD_MISSIONS=0)" in runs where that variable was not set | One message for every reason recording is off | `c0558da`. It lists the real reasons |
| A truncated or empty `FA_1.LIB` or `FA_2.LIB` ended an import with "invalid archive sentinel" or "failed to fill whole buffer" | The reader's error was passed up unchanged | `b6ce6d9`. The message names the file and says to copy it again or reinstall. Unit test in `media_source.rs` |
| Recordings, summaries and Tacview files called the F-22N and the F/A-XX "F-22" | Both borrow the F-22's PT name | `441ecab`. They use their own labels, F-22N Raptor and F/A-XX |
| Alt+F4 did nothing on the first-run locate screen (nothing detected), so it could only be left with the mouse or the desktop's own close binding | The screen handled only its own keys | `f5f6f04`. It quits on Alt+F4 and Command-Q like every other screen. `replay-keys-alt-f4-on-locate-screen` |
| A damaged `graphics-v1.conf` was ignored without a word, unlike the other settings files | The loader threw the error away | `6e6b95e`. It logs the file and the reason |
| A damaged `input-v1.conf`, which loads by itself at start, stopped the game with a message that did not say what to do | Only the file name and parser text were given | `2aff9c2`. It said to fix the file or delete it. Since 2026-09-29 the game falls back to the default controls with a logged warning instead of stopping (an explicit `--input-profile` still fails), checked by `replay-settings-corrupt-input*` |
| `--ai-probe-ticks 0` said "AI probe tick limit exceeded"; a word for `--ai-probe-ticks` or `--ground-start` gave the parser's bare message | Same | `a609062`. They name the option and the range |
| `--record-mission ""` ended with "the mission recording could not be finished; see the session log" | The empty path was accepted | `00b524c`. Refused up front with a clear message |
| The ground start hint said "PageUp adds throttle", but PageUp stopped setting throttle when the Fighters Anthology keys replaced the development ones on 2026-09-26 | The text was never updated | `ddba864`. It names key 5, full throttle. Checked by the ground start `replay-live-*` scenarios |
| An input profile that could not load (empty, wrong header, bad action, bad bytes) did not say which file | The parse error was passed up without the path | `9e1feb6`. Every profile error starts with the path |
| Name labels of aircraft close together in the replay viewer printed on top of each other and could not be read | Each label was placed at its aircraft with no regard for the others | `e3a97f7`. Labels stack upward until clear |
| The mission summary reported a parked wingman as "airborne 0:20.1" for the whole flight | The airborne flag means "in play" and stays set on the ground | `98db86d`. Only frames off the ground count. Unit test in `tore-replay` |
| A cockpit message from the game (the ground start hint) read "someone: ..." in the transcript | A missing speaker fell back to "someone" | `b0a0007`. Cockpit messages read "cockpit". Two text golden files updated |
| `--rate abc`, `--ids a,b`, `--from x`, `--replay-tick -5`, `--replay-speed fast`, `--replay-aircraft x` said "invalid float literal" or "invalid digit found in string" | Parser errors passed up bare | `f4e2a9d`. They name the option and what was typed |
| `--recording-log --out FILE` and `--recording-acmi --out FOLDER` said "File exists (os error 17)" or "Is a directory" | Bare I/O errors | `cb1fd77`. They name the path and what it was used for |
| `--recording-log --from 20 --to 10` wrote an empty log, and `--ids` with an aircraft that is not in the recording silently dropped it (round two) | Nothing checked either | Both are clear errors now (`--from 20 is later than --to 10`, `--ids names aircraft 999, which is not in the recording`) |
| `--import notes.txt` named the file's folder, not the file (round two) | The fallback to the folder lost the chosen path | The message names the file and its folder |
| Every remaining numeric option (about twenty: `--separation`, `--flight-zoom`, `--window-size`, `--probe-*`, `--weapon-slot` and others) gave the parser's bare message (round two) | Parse errors were passed up unchanged | All say `--option needs a number (why)`; 23 scenarios cover them |
| `--list-inputs --no-controllers` still probed and warned about devices (round two) | The diagnostics ignored the flag | It opens no device and says so |

Round two also added the `--input-script` development option (docs/DEVELOPMENT.md) so tests can
press any key and click at any point in a windowed run; the keyboard handler is now a method
shared by the window and the script.

## Found and not fixed

- **`--combat-smoke`** failed on 13 of the 14 aircraft when this lane first ran. The flight lane
  found the probe was stale rather than the game, and fixed it; all 14 pass now.
- **AI behaviour flags in the recordings, for the AI lane.** The summary flags `ai_flipping` (an
  AI aircraft changing activity six times in a few seconds), `track_lost_early` (a missile losing
  its track within seconds of launch) and `control_oscillation` (a pitch control reversing eight
  times in under a second). The scenarios and recording files that carry them are listed in
  `.local/battery/ai-churn-flags.txt` (regenerate it from a run's `work/*/log/summary.txt`).
  None was investigated here.
- Running several windows at once, the head tracker warns "UDP 4242: Address already in use".
  Harmless.
- The recording's `fuel_lb` is internal fuel only, so a flight that burns its external tanks
  first (the default loadout) shows no fuel used in `summary.txt`.
- No landing was flown. A key script cannot fly an approach open loop; the tower's landing
  clearance is checked, the touchdown is not.

## Needs a decision

- **Direct `--free-flight` recording.** The fix makes a direct start record. Say if a developer
  launch should stay unrecorded instead (the docs and `TORE_RECORD_MISSIONS=1` say it records).
- **The airborne flag's meaning.** In a recording `airborne` means "in play" and `on_ground`
  is separate, so a parked aircraft is both. The summary now counts only the frames off the
  ground; Tacview and the log still write the flag as recorded.
- **Retracting the gear on the runway.** A scripted takeoff pressed G at 80 kt with the wheels
  on the ground: the gear came up, the aircraft kept rolling on its "wheels" and took off with
  the gear already up, with no crash or message. No spec or doc says what the game should do
  (retail refuses on the ground, but that is recollection, not evidence here). The scenarios
  do not press G until the aircraft is airborne.

## Needs a human eye or ear

- **Sound.** The lane proves that start-up with sound on works and that a missing device
  degrades to silence. It cannot judge how anything sounds: the radio and crew lines, tower
  voice, seeker and RWR tones, situation music, replay sound at 1x, and the volume mixer.
- **The captured frames.** The checks find blank, black and broken frames. They cannot judge
  taste, exact placement or art: the twelve views, name labels, trails, the timeline markers,
  the comms panel, the Escape menu pages. The agent that ran the lane looked at the views,
  interface parts, panels, menu pages and ground-start frames and found the label overlap
  above and nothing else. Frames after that fix were not all reviewed again.
- **Anything the mouse does that a snapshot cannot show.** The Replays screen, the debrief and
  the Escape menu are clicked through by script and checked by the files, settings and screens
  that result, but the main menu's own buttons, the Quick Mission creator's controls and the
  loadout screen are left to the menus lane.
- **The viewer's keys.** The replay viewer takes its own window events, so `--input-script` keys
  do not reach it (its mouse does); its keys are covered by the Hyprland driver, which reaches
  named keys and Ctrl combinations only.
- **The feel of flying.** The scripts hold a stick position for a set time; nothing judges how
  the aircraft responds beyond G, bank, speed and events.
- **Real controllers.** No gamepad or joystick was attached; profiles were tested as files.

## How the checks work

`tools/battery.py` runs each scenario's main command, then any `then` steps (more app runs, or
plain Python), and hands the work folder and the joined output to `check_work`. A scenario can
carry `known_failure`, which reports its failure as known and fails the run once it passes.
`_replay_checks.py` holds the Tacview, log and summary checkers (with unit tests in
`tools/test_replay_checks.py`). `_replay_tools.py` damages recordings, builds damaged game
media and tests captured frames. `_replay_drive.py` sends keys to a game window through Hyprland by process id. A window on a hidden
workspace has no focus, and the game ignores most keys without it (as it should: a focus loss
pauses the flight and drops held controls), so only named keys and Ctrl combinations that go
straight to the menus reached the game. That, not a defect, is why letter and digit keys did
nothing through `hyprctl`. The hand-flown scenarios therefore use `--input-script` (see
[DEVELOPMENT.md](../DEVELOPMENT.md#windowed-runs-from-scripts-and-agents)), which feeds keys and
mouse events through the game's own handlers with focus assumed, and the driver stays for the
replay viewer and for quitting with Alt+F4. The driver's scenarios set `TORE_PERF_ACTIVE=1`
with a large `TORE_PERF_FRAMES` (which keeps a timing run unpaused) and `TORE_RECORD_MISSIONS=1`
(which makes it record); the script ones need neither.
