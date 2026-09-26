# Mission replays

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Every flight records itself so it can be watched again from any viewpoint,
exported to Tacview, and turned into logs that explain what the AI, the
weapons, the flight model and the radio did, with exact numbers. This is an
**opinionated addition requested by John on 2026-09-26**; the original game
has no such feature. Its "Replay Last Mission" button means *fly the last
mission again* and is unrelated. Every design choice below is an agent
decision unless it says otherwise.

A recording keeps **what happened**, not the stick inputs. Replaying inputs
would need frame-exact determinism the game does not have, while a record of
states seeks instantly, plays backwards, feeds Tacview directly and keeps the
exact values that happened.

The code lives in the dependency-free `crates/tore-replay` crate: the data
model, the writer, the bounded reader and every export. It knows nothing
about the simulation or the renderer; the app converts its own state into
the crate's model once per tick.

## Recording

### When

Every flight records itself; there is nothing to switch on. Recording starts
when a flight starts (Free Flight, a Quick Mission, the range), right after
the scene is placed, and the first frame is the scene before the first
tick. It stops when the flight ends: End mission (after the debrief is
taken, so the footer carries its result), Restart (the old recording
finishes and a new one starts), quitting the game, or re-importing media.
The session log names each file as it starts and when it is saved; nothing
is printed to the terminal. Captures, `--smoke-test` and frame timing runs
do not record. `TORE_RECORD_MISSIONS=0` turns recording off for a run and
`=1` forces it on, for example to time frames with recording on and off.

### Where and what name

Recordings live in `replays/` in the app data folder (which
`TORE_DATA_DIR` overrides), named after the UTC date and time the flight
started, the map and the player's aircraft:
`2026-09-26_1540_UKR_F18.tore-replay`. A second flight starting in the same
minute gets `-2`, `-3` and so on. The time is UTC, like the header's; the
game has no time-zone data without a new dependency (agent decision,
2026-09-26). Map names drop the `~` of variant layouts, and the aircraft
part is the exact identity: the F/A-XX is `FAXX`, never the F-22N it
borrows from.

### Auto-delete

`replays-v1.conf`, beside the other settings files, holds the rules:
auto-delete on or off, keep the last N recordings (default 20) or delete
those older than N days (default 30), and the recordings marked Keep. It is
on by default with keep the last 20, so recordings never fill a disk
unnoticed (agent decision, 2026-09-26). Cleanup runs when a recording
finishes, and later when the Replays screen asks. It deletes only files it
can prove it may: a recording's name and the format's magic bytes, not
marked Keep, not the recording in progress, and not a `.partial` file
written in the last ten minutes, which another running game may own.
Anything else in the folder is never touched.

### How

The recorder runs inside the flight's fixed 120 Hz tick. Right after the
AI's step, when combat and the AI have both written the tick's poses, it
reads the same render snapshot live flight draws (see the
[architecture](ARCHITECTURE.md#mission-recordings)) plus the flight data a
snapshot does not carry: airspeed, G, fuel, the pilot's controls, ground
contact, and whether the pilot is still flying it. It compares them with
the previous tick to find what
changed, and hooks later in the tick add the radio lines delivered, the
sounds released and cockpit messages. Encoding and writing happen on a
background thread fed by a queue two seconds deep. The flight never waits
for the disk: if the queue is ever full, that tick's frame is dropped and
the next frame starts with a `system.gap` event naming the missing ticks.
Events noted between ticks (a pause, a bookmark, a wing order) go on the
tick that was on screen.

Recording reads only state the tick already computed. The behaviour probes
give byte-identical output with recording on and off, and the simulation's
golden fingerprints are unchanged.

### What is recorded now

| Family | Events |
| --- | --- |
| Weapons | `weapon.launch` with range, aspect, off-boresight angle, closure, heights and speeds at release (a gun round or rocket, which has no target of its own, is aimed at the shooter's current target: the AI's, or the player's designated one); `weapon.seeker_active`, `weapon.pitbull`, `weapon.track_lost` once per shot (the target let go, or the seeker lost it); `weapon.outcome` (hit, missed, spoofed, jammed) for every shot the debrief ledger closes |
| Combat | `combat.hit` from each aircraft's hit points, with the attacker, damage, hit points after and the region hit (plus damaged systems for the player); `combat.destroyed` with the killer; `combat.ground_impact` |
| Aircraft | `aircraft.crashed` (flying into the ground or a structure, or a destroyed aircraft's wreck coming down or exploding), `aircraft.ejected`, `aircraft.pilot_killed`, `aircraft.took_off`, `aircraft.landed`, `aircraft.flameout`, `aircraft.fuel_out` |
| Flight | `flight.departure` (mode changes), `flight.stall` and `flight.spin` on and off |
| AI | `ai.activity`, `ai.target` and `ai.airfield_phase` changes, without reasons |
| Communication | `comms.radio`, `comms.crew` and `comms.tower` for every line delivered (speaker, words and recordings), manual tower replies, `comms.order` for the player's wing orders (the wingmen addressed, the reply, or why it was refused), and `comms.hud` for every cockpit message line: shown, a repeat that moved the line on screen to the bottom with a fresh timer, or pushed off the screen by newer lines |
| Audio | `audio.effect` (impacts and explosions), `audio.release` (weapon release sounds), `audio.tone` (seeker tones), `audio.stall_warning`, `audio.ejection` (warnings, seat, parachute, a wingman ejecting) and `audio.device` (gear, flaps, hook, brake) |
| Player and system | `player.command` (combat commands and trigger releases), `player.bookmark`, `system.pause`, `system.resume`, `system.time_scale`, `system.cheat`, `system.restart` (first in a recording that follows a restart), `system.end`, `system.gap`, and `system.note` when a tick held more than the format stores |

Every second a frame carries a checksum of all aircraft's exact state,
which `--recording-diff` uses. Every gun round is its own launch and
outcome, as the debrief counts them, so the summary's shot table lists a
burst round by round.

Not recorded yet, awaiting the milestone that records the reasons behind
decisions: the AI thinking and flight-model telemetry trees, AI weapon
phases, defensive reactions, fallbacks and ejection decisions,
`weapon.decoyed`, `flight.g_limit` and `flight.effect`, messages between AI
aircraft (`comms.request`, `comms.report`, `comms.delivery`), calls that
were queued, delayed, suppressed or dropped, and music changes. Reasons on
recorded events are left empty where the game does not yet say why. The
attacker on `combat.hit` comes from the debrief ledger's last shooter, and
its projectile from the same tick's shot outcomes; two hits on one aircraft
in one tick can be credited to the later shooter. A headless probe runs no
weather, crew voice, music or cockpit messages, and its header says so.

## File format

### Layout

A recording is one file. While it is being written it ends in `.partial`;
finishing renames it to its final name. All integers are little endian.

| Part | Contents |
| --- | --- |
| Prelude, 12 bytes | `TOREREPL`, the format version (currently 1), two reserved bytes |
| Header chunk | The text header: `key=value` lines in UTF-8 |
| Data chunks | One to two seconds of frames each (120 by default) |
| Footer chunk | End tick, the writer's totals and the mission result |
| Index chunk | Where every data chunk starts, for seeking |
| Trailer, 16 bytes | Where the index chunk starts, then `TORE-IDX` |

Every chunk starts with a 32-byte header: the marker `TORC`, the chunk kind,
the body length, the frame count, the first tick, and an **FNV-1a 64**
checksum over the header and body. FNV-1a was chosen because it is fast,
needs no dependency and catches accidental damage; it is not a security
measure.

A data chunk's body is a list of **sections**, each written as an id, a
length and the bytes, so a reader skips sections it does not know:

| Section | Contents |
| --- | --- |
| Strings | Strings first used in this chunk |
| Entities | Aircraft and weapons registered in this chunk |
| Frames | Aircraft, projectiles, debris, ejected pilots and surface damage, tick by tick |
| Spawns | Effects and smoke or contrail puffs released, tick by tick |
| Events | Everything that happened, tick by tick |
| Trees | Display tree samples (AI thinking, telemetry, missile guidance) |
| Checksums | The once-per-second state checksums |

Spawns, events and trees sit in their own sections so the viewer can
rebuild smoke, and a reader can list every event, without decoding the
aircraft.

### Keyframes and changes

The first frame of every chunk is a **keyframe**: every value is stored
exactly, all 64 bits. Later frames store only changes, **predicted from the
values the reader will reconstruct**, so rounding never builds up over a
long flight. Smooth quantities (positions, angles, velocities, airspeed) are
predicted to keep moving as they did on the last tick; controls and devices
are predicted to stay where they were. Only the difference from the
prediction is written, as small whole numbers, and groups of values that did
not change cost nothing. An aircraft or projectile that appears in the
middle of a chunk also starts with an exact record.

Aircraft positions start from their velocity, and projectile positions from
their direction and speed, so even the first changed frame costs little.
Angles are compared the short way round, so turning through north or
rolling through inverted is a small step.

A value that is not a finite number (or is absurdly large) cannot be
predicted. That aircraft or projectile is stored exactly on that tick and
the next, so the log shows the bad value bit for bit.

Strings are stored once. Each is defined in the chunk where it is first
used, and referenced by number afterwards. Aircraft and weapon identities
(for example `F18.PT`, shown as `F/A-18D`, never aliased) are registered
once in the same way.

### Units and axes

Feet, feet per second, radians, pounds and ticks of 1/120 second. X is east,
Y is height above mean sea level, Z is north. Attitude is yaw, pitch and
bank: yaw 0 faces north and grows clockwise, pitch is positive nose up, bank
is positive right wing down.

### What each tick stores

| Item | Values |
| --- | --- |
| Aircraft | Position, attitude, velocity, airspeed, G, the 11 animated devices, engine heat, flags (engine, afterburner, airborne, on the ground, crashed, wreck gone, alive, ejected, and animated: whether anything moves the devices, since straight-flight fixtures keep the model's neutral pose), wreck phase, fuel, pilot controls, the auxiliary body rates that thrust-vectoring paddles and plumes follow, hit points, regional damage and the failed structural section |
| Projectiles | Owner, weapon, target, position, previous position, direction, speed, tracer, inbound on the player, age, and the seeker's state |
| Debris and ejected pilots | Position and attitude or heading |
| Effects and puffs | Only those released this tick; the viewer ages them itself |
| Surface objects | Hit points, when they change |
| Events and display trees | See [the vocabulary](../crates/tore-replay/src/vocab.rs) |
| Checksum | Once per second: a hash of every aircraft's exact state |

### Precision

Quantized values are never further from the truth than half a step. A
10-minute test flight with smooth and violent maneuvers measured exactly
these bounds on every tick, with no drift.

| Quantity | Step | Largest error |
| --- | --- | --- |
| Positions of aircraft, projectiles, debris, pilots, effects and puffs | 1/32 ft | 1/64 ft (about 5 mm) |
| Attitude, headings | 2^-20 of a turn (0.00034 deg) | 0.00017 deg |
| Projectile direction | 2^-20 of a turn per angle | under 0.0005 deg |
| Velocity | 1/64 ft/s | 1/128 ft/s |
| Airspeed, speed device, projectile speed | 1/64 ft/s | 1/128 ft/s |
| G | 1/1024 G | 1/2048 G |
| Devices from 0 to 1, engine heat | 1/255 | 1/510, kept within 0 to 1 |
| Elevator, aileron and rudder | 1/127 | 1/254, kept within -1 to 1 |
| Pilot controls | 1/1024 | 1/2048 |
| Auxiliary body rates | 1/4096 rad/s | 1/8192 rad/s |
| Fuel | 1/16 lb | 1/32 lb |
| Ids, flags, hit points, regional damage, wreck phase, seeker, events, trees, checksums | exact | none |

Keyframes are exact, so a value that has not changed since the chunk began
reads back bit for bit.

### Size

On a synthetic 60-second flight with 17 aircraft, contrails, AI thinking 10
times a second, player telemetry 30 times a second and thrust vectoring on
the violent flights, the file took 13.5 bytes per aircraft per tick (11.7
bytes for the aircraft alone). That is about **16.5 MB for a 10-minute
mission with 17 aircraft**. Real AI control inputs are noisier than the
test's, so expect somewhat more.

### Limits

The writer refuses input beyond these limits with a clear message and writes
nothing for that frame; the recording carries on. The reader refuses files
beyond them.

| Limit | Value |
| --- | --- |
| Aircraft per frame | 64 |
| Projectiles per frame | 1,024 |
| Debris pieces per frame | 256 |
| Ejected pilots per frame | 64 |
| New effects per frame | 64 |
| New puffs per frame | 4,096 |
| Surface changes per frame | 4,096 |
| Events per frame, fields per event | 1,024, 64 |
| Ids in one value | 1,024 |
| Display trees per frame | 256 |
| Nodes per tree, depth | 4,096, 32 levels |
| Strings in the table | 65,536 (later strings are stored inline) |
| Bytes per string | 1,024 |
| Chunk body | 16 MiB (a frame too large for one chunk is refused) |
| Frames per chunk | 240 |
| Registered aircraft, registered weapons | 4,096 each |
| Header | 64 KiB, 256 extra entries |
| Events in one file | 4,000,000 |
| File | 1 GiB (the writer stops accepting frames about 20 MiB short of it, keeping room to finish) |

### Versions and damage

- The format version is in the prelude. A reader refuses a newer version
  with a plain message. Within a version, unknown sections, unknown chunk
  kinds and unknown header keys are skipped or kept, never fatal.
- A file cut short (a crash, or a recording still in progress) opens as
  **incomplete** and keeps every whole chunk; at most the last second is
  lost. The writer asks the system to put the file on disk every 30 seconds
  of flight, so a power cut can lose up to that much.
- A chunk that fails its checksum is skipped and reported; the chunks
  around it still play. Strings first defined in a lost chunk read as a
  placeholder.
- A jump in ticks, for example if the recorder falls behind, starts a new
  chunk and reads back as a gap.

## Exports

All exports read a recording and write text. They work on finished and
incomplete recordings alike.

### Debug log (JSONL)

One JSON object per line, for tools and for Claude:

| Line type | Contents |
| --- | --- |
| `header` | Game version, world, settings, tick range, completeness, damage notes, and the units of every field |
| `aircraft`, `weapon` | The registered identities |
| `event` | Every event with its fields, at its exact tick |
| `sample` | Each aircraft's state, once a second by default: position, attitude in degrees, velocity, airspeed and knots, G, fuel, heat, hit points, damage, flags, devices and controls |
| `tree` | Each display tree sample that differs from the previous one |
| `anomaly` | Every anomaly flag (below) |
| `footer` | End tick, result, completeness |

Options: a tick or seconds range, a list of aircraft ids (events with no
aircraft are always kept), the sample rate, and whether to include trees.
Numbers that are not finite are written as `null`, and an anomaly line
names them.

### Summary

A plain-English text file: the mission, conditions, result and length; each
aircraft's airborne time, highest and lowest G, lowest height above sea
level and above the ground (when telemetry records it), stalls, spins, fuel
used, shots, hits, kills, final state and time in each AI activity; a table
of every shot (launch geometry, time of flight, peak speed, closest approach
to the intended target, outcome and why); the communication transcript with
reasons; a timeline of key events; each bookmark with the events of the ten
seconds around it and every aircraft's state at that moment; and the
anomaly flags.

### Anomaly flags

Computed at export, so flight pays nothing. Each is a suspect worth a look,
not a verdict. Thresholds are **fitted**, agent decisions (2026-09-26), and
adjustable.

| Flag | Raised when |
| --- | --- |
| Non-finite | Any number in a state, event, tree or the header is not finite |
| Teleport | One tick's move differs from what the average velocity explains by more than 50 ft |
| Attitude jump | The nose or wings turn more than 20 degrees in one tick (2,400 degrees a second) |
| G excess | A live aircraft pulls more than 9.5 G or less than -4.5 G |
| Control oscillation | A stick axis reverses 8 times within 2 seconds, each swing at least 0.5 |
| AI stuck | An AI aircraft stays in one activity longer than 5 minutes |
| AI flipping | An AI aircraft changes activity, or target, 6 times within 10 seconds |
| Track lost early | A guided weapon loses its target within 2 seconds of launch |
| Fuel exhausted | An aircraft runs out of fuel |
| Order rejected | A recipient rejects an order, request or report |
| Call dropped, call suppressed | A call is dropped, or suppressed by a cooldown, limit or radio silence |
| Long wait | A call waits in a queue more than 3 seconds |
| Repeated call | The same speaker makes the same call 3 times within 10 seconds |
| Below terrain | Telemetry puts a live aircraft more than 5 ft below the ground |
| Crash undamaged | An AI aircraft crashes with no damage: it flew into the ground |

### Tacview

A `.txt.acmi` file in Tacview's ACMI 2.2 text format, sampled 10 times a
second by default (adjustable). Gun rounds are left out by default.

The game world is flat and has no latitude or longitude, so each theater's
map centre is pinned to a real place. Positions use Tacview's flat-world
form `T=Lon|Lat|Alt|Roll|Pitch|Yaw|U|V|Heading`: **U and V are the game's own
east and north coordinates in metres**, and longitude and latitude are
offsets from the anchor on a local flat-earth approximation (a 6,371 km
sphere). The map's north is treated as true north. Unchanged values are
left out after an object's first line, as the format allows.

| Game | Tacview |
| --- | --- |
| Height above sea level (ft) | `Alt` (m) |
| Bank, positive right wing down | `Roll`, the same sign: Tacview's roll is positive when rolling to the right, so a right turn shows a positive roll |
| Pitch, positive nose up | `Pitch` |
| Yaw, clockwise from north | `Yaw` and `Heading` in degrees, and `HDG` |
| Airspeed | `TAS` (m/s) |
| Telemetry tree lines Mach, AoA, Sideslip, AGL | `Mach`, `AOA`, `AOS`, `AGL` (m) |
| Pilot throttle, afterburner flag | `Throttle`, `Afterburner` |
| Gear, flaps, air brake, hook devices | `LandingGear`, `Flaps`, `AirBrakes`, `Tailhook` |
| Fuel (lb), G | `FuelWeight` (kg), `VerticalGForce` |

| Object | Tacview type and properties |
| --- | --- |
| Aircraft | `Air+FixedWing`; `Name` is the exact display name (for example `F/A-18D`), `Pilot` and `CallSign` the label, `Group` the wing, `Coalition` and `Color` the side (friendly blue, enemy red, neutral green) |
| Missiles, bombs, rockets | `Weapon+Missile`, `Weapon+Bomb`, `Weapon+Rocket`, with `Parent` set to the launcher |
| Gun rounds (optional) | `Projectile+Bullet` |
| Flares and chaff | `Misc+Decoy+Flare`, `Misc+Decoy+Chaff`, removed when they burn out |
| Ejected pilots | `Ground+Light+Human+Air+Parachutist`, with `Parent` set to the aircraft |

Object ids are hexadecimal and never zero: aircraft, projectiles, decoys and
parachutes each have their own range. Objects are removed with `-id` when
they leave the recording (a wreck when it is gone).

Events: `Destroyed` for kills, `Message` for radio calls the player could
hear, `Bookmark` for the player's bookmarks, `TakenOff` and `Landed`, and
`Debug` (shown with Tacview's `/Debug:on`) for AI decisions, orders and
answers, comms reasons and flight-model changes. Commas in text are escaped
as Tacview requires; line breaks become spaces.

**Reference time.** The recording's date at the mission's local time of
day, shifted by the anchor's longitude at 15 degrees an hour, so Tacview's
sun sits roughly where the game's does. For example, noon in Ukraine (34 E)
becomes 09:44 UTC.

**Theater anchors.** Provenance: **fitted**. The region of each theater
comes from the retail theater names, as recorded in the
[Quick Mission format](formats/quick-mission.md) and the
[viewer baseline](baselines/ukraine-viewer.md). Where each map sits inside
its region, and whether its north is true north, is not known; the points
below are agent estimates (2026-09-26). Next research step: match map
landmarks such as coasts, rivers and airfields to real geography. A theater
code outside this table is placed at 0 N 0 E in open ocean, provenance
**unknown**. An export can override the anchor.

| Code | Region | Latitude | Longitude |
| --- | --- | ---: | ---: |
| APA | Panama | 9.0 | -79.6 |
| BAL | The Baltics | 57.0 | 24.0 |
| CUB | Cuba | 22.0 | -79.5 |
| EGY | Egypt | 30.0 | 32.5 |
| FRA | France | 46.5 | 2.5 |
| GRE | Greece | 38.5 | 23.5 |
| IRA | Iraq | 33.0 | 44.0 |
| KURILE | Kuril Islands | 44.5 | 146.5 |
| LFA | Falkland Islands | -51.7 | -59.5 |
| NSK | North and South Korea | 38.0 | 127.5 |
| PGU | Persian Gulf | 27.0 | 51.5 |
| SPA | Pakistan | 30.0 | 71.5 |
| TVIET | North Vietnam | 21.0 | 105.8 |
| UKR | Ukraine | 45.3 | 34.0 |
| VLA | Vladivostok | 43.1 | 132.0 |
| WTA | Taiwan | 24.0 | 120.5 |

The map centre comes from the map size in the recording's header, or else
from the theater's terrain grid (8,192 ft between samples).

**Known limit.** Tacview draws real-world terrain, which will not match the
game's maps. Tacview itself has not opened these files in testing yet; that
check is manual.

### Comparing two recordings

Reports header differences, differences in registered aircraft and weapons,
the **first second where the state checksums differ** (and the last that
matched), the **first tick where any aircraft's state differs** and which
aircraft, largest difference first, and, per category (kills, launches,
hits, shot outcomes, AI decisions, comms, flight events, player and system
events), the counts and the first event that differs or, when the events
match, the first difference in timing.

The checksum is FNV-1a 64 over every aircraft's exact state in id order,
computed by the app from live values once a second. States are compared
after decoding with tolerances of one and a half steps, so two recordings of
the same flight never differ just because their keyframes fall on different
ticks.

## Replays screen

Filled in by a later milestone (M3): the list of recordings and its buttons.

## Viewer

Filled in by a later milestone (M3): playback, reverse, speeds, cameras,
hiding the interface and saving pictures.

## Debug panels

Filled in by a later milestone (M4): the AI thinking, telemetry, missile and
timer panels.

## Communication journal

Filled in by a later milestone (M2): how orders, reports, radio calls and
cues are recorded, heard or unheard, with their reasons.

## Command line

These commands read recordings of what happened and need no game media.
They are unrelated to `--record-input`, `--replay-input`, `--record-combat`
and `--replay-combat`, which store controls or combat inputs and simulate
them again.

| Command | What it does |
| --- | --- |
| `--recording-info FILE` | Prints who, where and when, the weather, length, size, settings, result, every aircraft and weapon, events by kind and any damage |
| `--recording-log FILE [--out DIR] [--from S] [--to S] [--ids 0,7] [--rate HZ]` | Writes `log.jsonl` and `summary.txt` to DIR (default: a `-log` folder beside the recording). `--from` and `--to` are mission seconds, `--ids` limits the log to those aircraft, `--rate` sets aircraft samples per second (default 1) |
| `--recording-acmi FILE [--out FILE] [--rate HZ] [--guns]` | Writes a Tacview file (default: `.txt.acmi` beside the recording), 10 samples a second by default; `--guns` adds gun rounds |
| `--recording-diff A B` | Prints how two recordings differ: header, identities, the first second their checksums differ, the first tick any aircraft's state differs, and event counts by family |
| `--ai-probe-ticks N --record-mission PATH [--verify-render]` | Records a headless AI probe to PATH (never overwritten) without changing its output. `--verify-render` then rebuilds every tick from the file, compares it with the picture the probe drew, and prints one line: `AI probe verify-render: PASS ticks=... missing=0 differing=0`, or the first difference |

For example, after John says "look at the replay from 3:40 pm" (15:40 UTC
in the file name):

```sh
tore-app --recording-info replays/2026-09-26_1540_UKR_F18.tore-replay
tore-app --recording-log replays/2026-09-26_1540_UKR_F18.tore-replay --from 200 --to 260
```

[Development](DEVELOPMENT.md#mission-recordings-for-debugging) has the
headless workflow.

## For developers

- Write with `tore_replay::Writer` (`create`, `register_aircraft`,
  `register_weapon`, `push`, `finish`); a writer dropped without `finish`
  keeps its `.partial` file and flushes what it holds.
- Read with `tore_replay::Recording` (`open`, `frame`, `frames`,
  `decode_chunk`, `tree`, `spawns`, `live_puffs`, `live_effects`, `events`).
  `Recording::peek` reads only the header, seek index and footer, for
  listings; an unfinished file peeks with no footer.
- In the app, `replay/recorder.rs` captures a flight (`start_tick`,
  `begin`, `note` and the other noting methods, `end`, `finish`),
  `replay/library.rs` owns the folder and auto-delete, and `replay/cli.rs`
  the command line.
- Export with `tore_replay::export` (`write_jsonl`, `write_summary`,
  `detect`, `write_acmi`, `compare`, `write_diff`).
- Event kinds, field names, tree channels, well-known tree labels, units
  and outcomes live in `tore_replay::vocab`, with each kind's fields listed.
- The app converts between its per-tick `RenderSnapshot` and the recording
  in `tore-app/src/replay/convert.rs`: `aircraft_state`, `projectile_state`,
  `debris_states`, `escapee_state` and `EffectWatch` (effects are recorded
  once, when they start) on the way in; `snapshot` on the way out, which
  draws the same picture through the same helpers; `difference` names the
  first thing two snapshots disagree on beyond the precision above. The
  draw rules that hold for a whole flight (which models are loaded, which
  aircraft draw with them) are header extras, read with
  `Presentation::from_header`. Ground objects are not aircraft: only their
  hit points are recorded.
- `terrain::World::for_identity` rebuilds the recorded world from the
  header's resolved settings (layout, weather choice and layer, start time,
  wind and cloud deck) without reading `TORE_WEATHER_TIME`, `TORE_WIND` or
  `TORE_CLOUD_ALTITUDE`; `World::identity` captures them from a live world.
- Tests use synthetic recordings only; golden outputs live in
  `crates/tore-replay/tests/golden` and are rewritten with
  `TORE_UPDATE_GOLDEN=1 cargo test --locked -p tore-replay`.
