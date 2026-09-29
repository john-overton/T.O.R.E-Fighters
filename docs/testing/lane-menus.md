# Battery lane: menus

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Covers the menu screens, the Quick Mission creator, the Load Ordnance page, the
preference, graphics, sound and controls screens, the briefing and debrief
screens, and the flight screen's own interface (HUD, instrument windows, views,
flight menu). Scenarios are in `tools/battery_scenarios/menus.py`. Run it with:

```sh
cargo build --locked -p tore-app
python3 tools/battery.py --lane menus --jobs 6 --windows 2 --tag menus
```

It takes about ten minutes on the dev machine with six jobs and two windows
(637 scenarios, 608 seconds at the last run): the headless ones (CPU snapshots,
instrument windows and headless mission starts, about 480) take a few minutes, the
windowed ones (about 170, each a real window through `tools/agent-run.sh`) about
eight, and the creator probe `menus-validate-creator` about eight on its own, in
parallel with the rest. The
scenarios count as the pass or fail; pictures the scenarios write are checked
for size and blankness, and were also looked at by the agent that wrote them.

## What each part checks

| Scenarios | What they do |
| --- | --- |
| `menus-validate-creator` | `--validate-creator`: loadouts for all 14 aircraft including every removed-store case, then the creator matrix, the render sweep and the input fuzz (below) |
| `menus-validate-text` | `--validate-text`: every imported string decodes without U+FFFD and is drawable in the original fonts |
| `menus-window-terrain-*` | Real-window captures on every base theater (clear, night), four theaters in each other weather, and ground starts in all six weathers, checked for blank, black, one-colour, dark-day and bright-night frames |
| `menus-validate-maps`, `menus-validate-weather` | Every imported map layout and every weather module and choice |
| `menus-snap-*` | A CPU snapshot of every `--snapshot-state` value (main menu, Pref, controls tabs, graphics, sound, replays, locate), each checked for the right size and not being blank |
| `menus-snap-quick-*` | The same for the creator, the aircraft and theater popups, the Load Ordnance page (normal, empty, drag, messages), the five debrief pages and both outcomes, for every aircraft and for every one of the 75 theater layouts |
| `menus-start-*` | The headless AI probe starts a mission through the creator's launch layout code: every aircraft on three theaters, every weather choice, every separation, every wing size. No aircraft may start off the map |
| `menus-loadout-*` | `--loadout none` and `--loadout guns` (stores taken off) for every aircraft, flown headless |
| `menus-window-launch-*` | The real launch path (`--launch-quick-mission --loadout none|guns`) for every aircraft: the launch line must show exactly the ammunition the page left, and the weapons window must list nothing (none) or only the gun (guns) |
| `menus-panel-*` | Instrument windows drawn on the CPU: every page (0 to 9) for every aircraft, and every panel-only systems fault (1 to 35) on the systems window |
| `menus-window-cockpit-*`, `-view-*`, `-flight-*`, `-flight-menu-*`, `-size-*` | Real-window captures: every aircraft's cockpit at 960 by 720, all 12 flight views, the flight and paused-menu screens at 960 by 720, 1280 by 720 and 640 by 900, and six more window sizes (out-of-range sizes must be refused with a message) |
| `menus-window-graphics-*`, `-preview-*`, `-weapons-page-*`, `-smoke-*` | Every graphics option value, the map, debug panels, weapon diagnostics, small layout, damage, ejection and chaff previews, the weapons window for stores-off loads, and smoke starts of the creator, the controls screen and free flight |
| `menus-combat-smoke-mig29` | The combat smoke probe, for the one aircraft it still passes (see below) |

### The creator probe

`--validate-creator` (`TORE_CREATOR_STAGE=loadouts|matrix|render` runs one part):

- **Matrix** (`quick_mission/matrix.rs`): about 11,600 setups started through the same
  steps a flown mission takes (the creator's own refusal check, the wing launch,
  ground layout, layout plan, altitude check, combat build, flight restart, AI wing
  build, group orders). It sweeps every theater layout with every separation and
  altitude, every runway with wings of one to five, every weather choice on every
  source theater, every player aircraft against every enemy aircraft, every wing's
  count, skill and aircraft, a sample of the whole wing-count product, every group
  order with survival on and off, every mission preset, and every other dropdown
  value. A setup must start with the player and every aircraft finite, on the map and
  above the ground, or be refused with a message. Result: 0 problems; 43 setups are
  refused with a clear message (listed in the probe output).
- **Render sweep**: draws the creator with every dropdown value chosen, every popup at
  every page, every group order and the help and notice overlays (518 pictures), none
  blank. `TORE_CREATOR_DUMP=DIR` also saves the worst-case-label pictures.
- **Input fuzz**: a fixed random stream of keys, pointer moves, clicks and right clicks
  (about 150,000 events) on the creator, the Load Ordnance page for every aircraft,
  the flight menu, the main menu and the graphics, sound, controls and replay screens.
  Nothing may panic, the creator's fields must stay inside their lists and the loadout
  must stay inside each station's capacity.

## Bugs found and fixed

| Symptom | Cause | Fix |
| --- | --- | --- |
| Stores taken off the Load Ordnance page (John's Mavericks) still appeared in the flight's WEAPONS window, and an aircraft with everything off started with an empty gun selected, armed and listed as `0 M61` | The weapons list, the selection ring and the flown-mission startup treated every configured station as on the aircraft, even with a count of zero. The ammunition itself was already correct (zero stayed zero) | The list leaves out empty stations, `[` and `]` step over them, startup falls back to the first loaded station or NAV, and a flown mission starts on NAV when the selected station is empty (`bb72502`, `042ec7b`, `242d106`) |
| `--validate-creator` stopped with `F18: damage region 3 at 0.1 has no distinct finite geometry` | The probe still expected visible partial damage on surviving aircraft, which the game deliberately draws intact | The probe now expects survivors intact and only a destroyed aircraft to change shape (`bb72502`) |
| The RCS window's "NO EXPOSURE DATA" ran over its 270 and 90 bearing labels | Same row as the labels | Moved below the axis (`2e79269`) |
| A broken `input-v1.conf` stopped start-up with "missing input profile version" and no file name | The error text carried no path | The message now starts with the file's path (`3570984`) |
| A malformed `graphics-v1.conf` was ignored without a word, unlike the preferences and sound files | The loader swallowed the error | It now logs why the defaults were used (`0a1d5af`) |

Also checked and fine: every settings file (preferences, graphics, sound) as empty,
random bytes, 400 KB, NUL bytes, a wrong version, duplicated lines, NaN and a directory
starts the game on the defaults, with the preferences and sound cases reported in the log
(no panics in 24 windowed starts); preference files with empty, duplicate or out-of-range
instrument pages load or are rejected cleanly.

New tests: `combat::tests::an_emptied_station_stays_empty_and_is_not_listed` and three more,
`live::tests::the_selection_ring_skips_stations_that_carry_nothing`,
`ordnance::tests::taking_every_store_off_by_any_route_leaves_zeros_that_still_validate`.

## Round two (2026-09-29)

**Garbled text.** Retail text is DOS code page 437, not UTF-8. The evidence is the
original fonts: `WIN11`, `HUD11` and the other FNT files draw exactly the CP437
text characters above 0x7F (accented letters at 0x80 to 0xA5, 0xA8, 0xAD, sharp s,
micro, degree) and no box-drawing cells. Across every imported text resource the only
byte above 0x7F is 0x89 in `KURILE.MM`, the airport `Ber\x89zovka`, an e with a
diaeresis (Berezovka with the diaeresis, "Berëzovka"). It was read lossily and printed
as U+FFFD; the map label dropped the letter; the bundled menu font had no cell for it.
Now mission layout text decodes as CP437 (`tore_formats::text`), every drawing path
(menus, creator, HUD, instruments, flight map, messages, replay and controls text, the
debrief) puts each character on its CP437 font cell, the two bundled menu font atlases
gain the CP437 letters (their ASCII columns are byte for byte unchanged), and theater
names may carry CP437 letters. Checked in the creator's airport popup ("Berëzovka") and
on the flight map ("BERËZOVKA"). `--validate-text` (scenario `menus-validate-text`)
scans 2,175 imported strings (creator lists, airports, aircraft, weapons) and every
text resource for U+FFFD and for characters the original fonts cannot draw; it found
one line and no problems. With `TORE_CREATOR_DUMP` the render sweep also draws a sample
with accented letters in every bitmap font: all the original menu fonts have real
letters at the CP437 cells, which confirms the character set. Commits `d0209f3`,
`b08cae4` and the theater-name commit between them.

**Terrain, sky and weather sweep.** 192 real-window captures at 960 by 720 (all 16
base theaters and eight variants, all six weather choices airborne, clear, dawn and
night ground starts, aircraft rotated through all 14) plus 60 in the lane
(`menus-window-terrain-*`) with an image check (`frame_stats`, `scene_problems` in
`menus.py`, tested in `tools/test_battery_menus.py`): not black, not one colour, day
bright, night dark, sky and ground both present. No blank, black or missing terrain,
no missing sky, no bright night or dark day. What the images show, all from the
imported haze tables and consistent with them:
- Cloudy and foggy weather at the default 5,000 ft start hide the ground completely (the
  tables put 256 of 256 haze at 6,000 ft cloudy and 500 ft foggy). Cloudy is visible at
  300 and 1,500 ft above ground, dim and washed out; fog is visible only on a ground
  start, and fades within a runway length.
- Athens (Greece) and Stanley (Falklands) airport pavement is pure white in daylight
  (grey at night); every other airport paving is grey or brown.
One genuine defect: **the HUD was unreadable over fog and cloud.** HUD green has a
luminance contrast of 1.04 to 1.08 to one against fog and cloud (1.3 over clear sky),
so it was told apart by hue alone. Each HUD pixel now casts a soft dark shadow one
pixel down and right (`hud::shadow`, fitted, see the HUD spec; one call to remove).

**Feature claim audit.** 44 claims from the menu, creator, briefing, preference,
ordnance and HUD rows of `docs/features.md` and their specs were checked against the
app (capture, snapshot, headless probe or code and test): the separation list, wings up
to 29 plus the player, right-click cycling and the player wing never reaching zero,
ground-target popup text and geometry, the objective and survival selectors, Replays
after Multi, the Sound/Music Prefs sliders, defaults and file name, graphics defaults
and file name, five debrief pages and the rocker, loadout outlines, drag, sounds and
Cheat, NAV/LCOS/ARM status text, the WEAPONS window, the 162 by 160 instrument frames,
seven message lines and five seconds with a 5 pixel margin, the W waypoint key, the pack
limits (32,768 resources, 1 GiB), `--version`, fullscreen and window-size rules,
diagnostics self-test, ground start on NAV and airborne start with the gun armed,
1,000 ft pipper and damage regions for every aircraft (`--validate-creator`), AGL and
VS only on an active ILS, and the greyed Replay Last Mission and Continue Old Campaign.
Stale and fixed: the window-mode row said preferences were format version 6 (it is 7,
reading 1 to 6); the damage row said twelve aircraft (fourteen are selectable);
`--help` listed four of the creator's thirty-odd snapshot states; the graphics spec said
an unreadable settings file was silent.

**Corrupt controls file.** `docs/INPUT.md` says a bad profile "fails with a line
number", and no document says the automatic `input-v1.conf` must fall back. The
behaviour is documented as failing, so it stays as it is; the round one change made the
error name the file. It is no longer an open decision, only a note that an empty or
binary file has no line number to report.

## Found and not fixed

- **`--combat-smoke` fails for 13 of the 14 aircraft** (only the MiG-29 passes). Its
  radar-off check expects any radar missile to be held back, but the active AIM-120
  keeps its own seeker and now launches in boresight; behind that, its guidance probe
  points the Maverick at an air target, which the missile rules now reject as the wrong
  target, and the A-4E's automatic damage check no longer holds. These are stale probe
  assumptions from the Sep 17 missile rework, not flight defects that anyone has seen,
  but the probe needs the weapons owner to redo it. Repro:
  `target/debug/tore-app --aircraft f18 --combat-smoke --no-audio` (also fails on the
  `bug-bash` base).

## Needs a decision

- **Su-35 wingtip station.** The imported Su-35 marks its wingtip AA-11B station as
  internal, so the Load Ordnance page prints it as `2 (max 2)` (no "loaded"), its weight
  is left out of the payload, and the Guns only rule treats it as a weapon to strip. The
  data says so; nobody has checked whether the retail game does the same.
- **Ground start on a theater with no airports.** Nine theater layouts have no imported
  runway (`~FRAF`, `~GREF`, `~IRAF`, `~KURILE`, `~NSKF`, `~PGUF`, `~TVIET`, `~UKRF`,
  `~WTAF`). The creator lets Start be set to Ground there (the Airport row reads
  Unavailable) and refuses at Fly with "No imported runways are available in this
  theater. Choose Airborne." Whether the creator should block the choice earlier is not
  defined.
- **Runways where a lone aircraft is obstructed.** Greece runway 13 and `~BALF` runway 0
  refuse even a single player ("The runway start is obstructed"); about a dozen more
  (for example Greece 12, 15 and 16 and Taiwan 13) refuse only a wing of two or more.
  This is the documented rule (obstructed starts are
  refused), listed here in case the scenery is wrong rather than the check.
- **Creator rows that change nothing yet.** The situation row (neutral, friendly,
  hostile) and both nationality rows are presentation only; the mission does not use
  them ([creator spec](../spec/quick-mission-menu.md)).
- **HUD line for a dry station.** After the last round is fired, the station stays
  selected and the HUD reads `0 M61`, the weapons window still lists it, until the next
  `[` or `]`. Retail behaviour here is not recorded.

## Needs a human eye or ear

- Night ground starts in desert theaters (Egypt, Iraq, Persian Gulf and others) show a
  brown ground clearly lighter than the sky; the image check passes it, but whether
  retail night ground was that lit is unknown.
- The HUD's new dark edge (round two) looks subtle in captures; a human should judge it
  against fog, cloud and a bright sky.
- Athens and Stanley airport pavement is pure white in daylight; fog and cloud hide the
  ground at the default 5,000 ft start (see round two).
- Portrait windows (640 by 900): the flight view fills the window while the paused menu
  and its bottom buttons sit in a centred 4 by 3 box in the middle. It looks
  deliberate but odd.
- The debug panels overlap the instrument windows and each other when all are open.
  They are a development feature.
- Whether the Load Ordnance page should print an empty station as NOTHING (the manual's
  wording) instead of an outlined empty box (the current, documented choice).
- Every screen's look against the retail game: the scenarios only find broken layouts,
  wrong sizes and blank pictures.
