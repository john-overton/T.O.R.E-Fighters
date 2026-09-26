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
unnoticed (agent decision, 2026-09-26). The
[Replays screen](#auto-delete-settings) changes the rules and the Keep
marks. Cleanup runs when a recording finishes and when the Replays screen
opens. It deletes only files it
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
background thread fed by a queue two seconds deep. Live flight never waits
for the disk: if the queue is ever full, that tick's frame is dropped and
the next frame starts with a `system.gap` event naming the missing ticks.
A headless probe recording (`--record-mission`) has no frame rate to
protect, so it waits for the writer instead and never has gaps.
Events noted between ticks (a pause, a bookmark, a wing order) go on the
tick that was on screen.

Recording reads only state the tick already computed. The behaviour probes
give byte-identical output with recording on and off, and the simulation's
golden fingerprints are unchanged.

What it costs, measured on the three-minute headless probes on an 8-core
Apple Silicon Mac with nothing else running: with 9 aircraft the recorder
takes about 10 microseconds of the simulation's tick (5 of them for the
reasons and display trees; draining the communication journal takes well
under a microsecond), and the whole process uses about 35 microseconds more
CPU a tick with the writer thread, against a tick of 8,333 microseconds.
With 5 aircraft the recorder takes about 7 microseconds (3 for the reasons
and trees). A busy machine can starve the writer thread: once, with four
probe runs and another agent's builds at the same time, a probe recording
lost 57 ticks to a `system.gap`. Probe recordings now wait for the writer
instead, so that cannot recur there; live flight keeps dropping rather than
stalling, which only a machine far busier than the game itself would cause.

### What is recorded now

| Family | Events |
| --- | --- |
| Weapons | `weapon.launch` with range, aspect, off-boresight angle, closure, heights and speeds at release (a gun round or rocket, which has no target of its own, is aimed at the shooter's current target: the AI's, or the player's designated one); `weapon.seeker_active`, `weapon.pitbull`; `weapon.track_lost` once per shot, with the range and why (decoyed, the seeker lost it, the target is gone, or it could not hold the target); `weapon.decoyed` for each missile that followed chaff or a flare, an AI aircraft's or the player's own, with the roll, its threshold, and the missile's susceptibility and the device's effectiveness behind it (a roll that failed shows in the missile's guidance tree); `weapon.outcome` (hit, missed, spoofed, jammed) for every shot the debrief ledger closes, with why (the decoy and its roll, the jammer, or a track lost earlier) and, for a guided miss, how close it came |
| Combat | `combat.hit` from each aircraft's hit points, with the attacker, damage, hit points after and the region hit (plus damaged systems for the player); `combat.destroyed` with the killer; `combat.ground_impact`; `combat.countermeasure` for every chaff cartridge and flare that leaves an aircraft, the player's and the AI's, with how many of that kind it has left and the aircraft's exact position, velocity and attitude as it left, which the viewer flies the device again from ([below](#chaff-and-flares)); `combat.countermeasures_cleared` when a range reset removes them all |
| Aircraft | `aircraft.crashed` (flying into the ground or a structure, or a destroyed aircraft's wreck coming down or exploding), `aircraft.ejected` with the hazard an AI pilot left for or your own ejection, `aircraft.pilot_killed`, `aircraft.took_off`, `aircraft.landed`, `aircraft.flameout`, `aircraft.fuel_out` |
| Flight | `flight.departure` (mode changes), `flight.stall` and `flight.spin` on and off; `flight.effect` when a flight-model effect starts or stops, with what it applied and why ([below](#flight-model-effects)); `flight.g_limit` when the stick reaches its stop, with the G the envelope offers, the limit applied, the G delivered and what set the limit; `flight.structural_failure` with the section, the G and why |
| AI | `ai.activity` (with how long the old activity lasted), `ai.target` (with priority and score), `ai.weapon_phase` (with the store and the weapon service's words), `ai.airfield_phase`, each with its reason ([below](#reasons-for-ai-decisions)); `ai.defense` when a missile defense starts, changes maneuver, releases chaff or flares, or ends, and when a launch warning arrives or is dropped; `ai.fallback` the first time each aircraft uses each fitted stand-in rule; `ai.ejection` when the ejection check finds a hazard, the pilot ejects, a go-around replaces an ejection, or the hazard passes |
| Communication | Every entry of the [communication journal](#communication-journal) and of the AI message journal, with trigger, rolls, outcome and reason ([below](#communication-events)); and `comms.hud` for every cockpit message line: shown, a repeat that moved the line on screen to the bottom with a fresh timer, or pushed off the screen by newer lines |
| Audio | `audio.effect` (impacts, explosions, and each chaff cartridge's and flare's release sound, marked `own` when the player's own aircraft released it), `audio.release` (weapon release sounds), `audio.tone` (the seeker tone, its loudness and whether its weapon aims at the surface), `audio.stall_warning`, `audio.ejection` (warnings, seat, parachute, a wingman ejecting), `audio.device` (gear, flaps, hook, brake) and `audio.music` (every input of the situation music, the score they ask for, and why) |
| Player and system | `player.command` (combat commands and trigger releases), `player.bookmark`, `system.pause`, `system.resume`, `system.time_scale`, `system.cheat`, `system.restart` (first in a recording that follows a restart), `system.end`, `system.gap`, and `system.note` when a tick held more than the format stores or a journal overflowed |
| Display trees | `ai.thought` for every AI aircraft, `flight.telemetry` for every aircraft that flies, `weapon.guidance` for every guided missile ([below](#display-trees)) |

Every second a frame carries a checksum of all aircraft's exact state,
which `--recording-diff` uses. Every gun round is its own launch and
outcome, as the debrief counts them, so the summary's shot table lists a
burst round by round.

The attacker on `combat.hit` comes from the debrief ledger's last shooter,
and its projectile from the same tick's shot outcomes; two hits on one
aircraft in one tick can be credited to the later shooter. A headless probe
runs no weather, crew voice, music or cockpit messages, and its header says
so. What the records themselves cannot explain is listed with the
[AI thinking record](#ai-thinking-record), the
[telemetry record](FLIGHT-MODEL.md#telemetry-record) and the
[communication journal](#not-visible-yet).

### Chaff and flares

Combat notes every chaff cartridge and flare as it leaves an aircraft, the
player's own and the AI's, and the player's own decoy rolls, in two
write-only lists the recorder drains (`State::take_device_notes`,
`State::take_decoy_rolls`); the AI's rolls come from the bridge as before.
Each release becomes one `combat.countermeasure` entry holding the
releasing aircraft, the kind, its number among the flight's releases (which
chooses the device's look: its flicker, sideways throw, smoke and strips)
and the aircraft's position, velocity and right, up and forward vectors
exactly as they were. The player's releases between ticks land on the tick
on screen, like the key press that made them; the AI's on the tick whose AI
step released them. How many of that kind the aircraft has left comes from
combat for the player and from the AI's dispensers, counted back over its
releases in the same tick, for AI aircraft. A range reset that clears every
device is its own entry. The golden fingerprint
`combat/player-countermeasures` was taken before these lists existed, and
they leave it unchanged.

### Display trees

A display tree is what a debug panel shows, as numbered lines: a label, a
value with its unit, and a note that says why. The recorder builds three
kinds from the simulation's write-only records, and the debug panels will
build the same trees from a live flight. The builders are pure functions in
`tore-app/src/replay/trees.rs`; the layouts and rates are agent decisions
(2026-09-26).

| Channel | Subject | Recorded | And at once when |
| --- | --- | --- | --- |
| `ai.thought` | Each AI aircraft | 10 times a second while alive | Its activity, target, weapon phase, motion branch or maneuver changes |
| `flight.telemetry` | You, and each AI aircraft | You 30 times a second, AI 5 times, while alive | An effect starts or stops |
| `weapon.guidance` | Each guided missile | 10 times a second, and at launch | A decoy roll is made against it |

Each aircraft and missile keeps its own phase within the rate, so the work
spreads over the ticks. Numbers are rounded to what a panel shows (tenths of
a nautical mile, whole degrees and knots, tens of feet for heights and
pounds for thrust and drag); notes use clock times rather than running
counts, so an unchanged line costs nothing in the file. Measurements (angle
of attack, sideslip, Mach, dynamic pressure, height above ground) are
labelled as measurements and never given as a cause.

The **AI thinking** tree, top level first. Lines appear only when the
record has something to say; the controller's lines appear only when its
decision ran this tick, otherwise "Motion" says why not (an airfield
sequence, destroyed, a training target).

| Line | Value | Note and children |
| --- | --- | --- |
| Aircraft | Label | Aircraft, skill and where it came from, side, wing and place |
| Mission | Role | Stance; holding formation, must rejoin; each remembered attack, acted on or ignored and why |
| Activity | Activity (`Activity`) | Why it last changed; `Since` the clock time it began |
| Target | Aircraft id (`Target`) | Its name; `Chosen by` (mission ranking, kept, nearest first, weapons held) and why; `Lost`; `Score` with its priority; `Runner-up` with its score, name and priority; the ranking's rules; who attacked it or its charge; `Not eligible`, up to three observed aircraft that could not be chosen and why |
| Geometry | Range, nm | `Off nose` (ahead or behind, facing or not), `Aspect`, `Closure`, `Height` above or below |
| Weapon | The chosen store and station | Target class; `Phase` (and the one before); `Fire?` with the first reason it did not fire and its numbers, for example "outside max range (6.1 nm > 5.8 nm)"; `Service`, the weapon service's words with its next deadline as a clock time; each store against this target, grouped, with its verdict and failing checks |
| Defense | The missile | Its weapon and shooter; `Range`, `Maneuver` (notch, jink or not yet, with heading and pitch), `Countermeasures`, `Time to impact`, `Maneuver time`, `Margin`, `Why now` |
| Motion | Branch (tactics, formation, missile defense, rejoining, search, ordered) | What the branch is doing; `Slot distance` and `Formation speed` in formation; `Choice` with every draw of the choice (for example "roll 37 < 50: best attack"), `Chosen at`, `Path`, `Situation` with the quadrant's percentages, any fitted rule, last-ditch candidate, engagement pitch and pursuit offsets; `Next choice`; `Rolls` made this tick outside a choice |
| Steering | | `Heading`, `Pitch`, `Bank` and `Speed` delivered, each with `Asked`, what the maneuver asked for; `G` delivered with its `Limit`; `Terrain floor` |
| Controls | | Pitch, roll, rudder, throttle, afterburner |
| Fuel | Pounds | State; `Endurance`, `Time home`, `Heading home` |
| Airfield | Phase | The gates (turn, runway, wing landed, leader landing); parking slot, a landing begun, why it left the sequence, a go-around |
| Ejection | Watching, ejected or go-around | The hazard and seconds to impact; the phase that guarded it; whether it was a catastrophe |
| Fitted rules | Names | Every stand-in rule used this tick |
| Recent | Count | The last four decision changes, each labelled with its clock time, with what changed and why |

The **telemetry** tree: `Aircraft` (label; aircraft and which flight-model
path moved it), `Altitude` above sea level, `AGL` (measured), `Air data`
(measurements: `TAS`, `Mach`, `AoA`, `Sideslip`, `q`), `Load` (`G`
delivered, `Asked`, `G limit` with what set it), `Rates` (roll, pitch, yaw),
`Thrust` (with `Lapse`, `Power available`, `Throttle`, `Afterburner`),
`Drag` (airframe, fuel and stores, pull, gear, flaps, airbrake, slip, and
damage when there is any), `Stall speed` (with `Authority`), `Fuel`,
`Contact` on the ground, and `Effects applied`, one line per effect the
step applied: its label, the factor, limit or state it applied, and a note
saying why ([effects](#flight-model-effects)). The labels `TAS`, `Mach`,
`AoA`, `Sideslip`, `AGL`, `G` and `G limit` are the ones the exports read.

The **guidance** tree: `Weapon` (name; guidance kind), `Shooter`, `Target`,
`Mode` (cued or boresight), `Seeker` (status; acquired, searching or not yet
enabled) with `Quality` and `Tracking`, `Time of flight`, `Speed`,
`Range to target`, `Closest approach` so far and when, `Intercept in`, and
`Decoy roll`, the latest roll against it and whether it was decoyed or
resisted.

### Reasons for AI decisions

Each decision change is recorded on the tick it happened, with a thought
tree, and with a sentence built from the records: for an activity, the
missile defense and its reasons, the tactic chosen, the store firing at
whom, why there is no firing solution yet, holding formation until
released, the search, the rejoin, the fuel state, or the airfield sequence;
for a target, the mission ranking with priority, score and the runner-up,
the nearest-first ranking, a target that left, or weapons held; for a
weapon phase, the weapon service's words. When the aircraft heard something
from another aircraft on the same tick (an attack report, a leader's
release to free selection, a wing order, a launch warning), the reason ends
with "after" and what it heard. Spacing and wing-control orders move the
slot, not the decision, so they are not cited.

### Flight-model effects

`flight.effect` turns each effect of the [telemetry
record](FLIGHT-MODEL.md#telemetry-record) into an "on" event when it starts
and an "off" event when it stops, compared by `std::mem::discriminant` (and
the device, for held devices). An effect that is gone for less than a
quarter of a second (30 ticks) and comes back is the same episode, so
flicker makes no events. Effects of one moment (a touchdown, a lift-off, a
blast kick, the spin direction rule, a spin's end, an autopilot release, a
building rebound) have an "on" event marked `momentary` and no "off". Every
aircraft's first recorded tick lists the effects already in force. An AI
aircraft's blast kick lands before its own flight step, so its record shows
the rotation it caused (`Blast jolt`), not the kick.

### Communication events

| Journal entry | Event |
| --- | --- |
| Radio calls, wingman replies, AI chatter | `comms.radio` |
| Crew remarks and coaching checks | `comms.crew` |
| Tower lines and replies, and tower speech cut when the runway under your landing clearance is destroyed | `comms.tower`; the cut is `cancelled` because the runway is unusable |
| AI text lines not shown yet (queued, held, replaced) | `comms.hud`; a line the HUD showed is recorded once, by the HUD |
| Player orders | `comms.order`, then one `comms.delivery` per addressed wingman with its answer (applied, rejected or skipped) and why; an order that went out on the radio has the `radio` route, one the game refused before it went out has none |
| Music inputs | `audio.music` with `from`, `to`, why, and every input (`succeeded`, `ejected`, `launching`, `air_target`, `hit_recently`, `danger`, `home`, `deck`) as true or false |
| Attack evidence (AI message journal) | `comms.report` when queued, then one `comms.delivery` per recipient: delivered, ignored with why, or expired after 2 s without news |
| A neutral leader releasing itself to free selection | `comms.order` with the attack that triggered it |
| Wing orders between AI aircraft | `comms.request`, answered by one `comms.delivery` per recipient when there are several |
| Launch warnings | `ai.defense` when they arrive (the reaction and devices scheduled) or are dropped (and why) |

Each carries its `trigger`, the `rolls` it used, its `outcome` and
`reason`, how long it waited, whether you heard it, its audience, its
`route` (`radio`, `tower` or `direct`: how the cockpit plays it), and a
`message` number shared by every entry about the same line or message.
Radio call numbers are the channel's own; AI messages count from 2^32. An
escort's changed priority is not an event; it shows in the escort's thought
tree.

A line has one entry for each thing that happened to it, so it is
delivered, and heard, exactly once: the entry whose `outcome` is
`delivered` (`tore_replay::vocab::heard` says which). Its queued entry
comes before, and an `interrupted` one after, when your wing order voice
cut it off. The replay's sound, subtitles and Tacview messages use only
that entry. Two tower triggers are fixed names a replay acts on
(`tore_replay::vocab::trigger`): `player request` on the tower's answer to
your own request, and `landing clearance cancelled` on the cut; other
triggers are plain English.

The seeker tone (`audio.tone`) is recorded when it starts, stops or
changes, and again when its loudness (`strength`, 0 to 1, as the mixer
takes it) moves by 0.05 or more from the last entry, so a tone that swells
with the seeker's signal makes a few entries rather than one a tick. Its
name comes from `tore_replay::vocab::tone`, and `surface` says whether the
weapon aims at surface targets, which changes the sound of an infrared
lock. The music's inputs are journaled only when the flight has sound; a
headless probe has none.

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

The headless AI probes, three minutes each with every reason event and
display tree, measured on 2026-09-26: 5 aircraft take 320 to 500 KB a
minute, and 7 to 9 aircraft 630 to 760 KB a minute, so a 10-minute mission
with 9 aircraft is about 7.6 MB. The reasons and trees add 100 to 220 KB a
minute for 5 aircraft and 260 to 380 KB for 7 to 9, most of it each tree's
full copy at the start of every one-second chunk.

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
used, shots, hits, kills, chaff and flares released, final state and time in each AI activity; a table
of every shot (launch geometry, time of flight, peak speed, closest approach
to the intended target, outcome and why); the communication transcript
with triggers, outcomes and reasons, including the music's changes; a
timeline of key events (G-limit hits, chaff and flare releases and decoys among them; AI decisions
and effect changes stay in the log and the bookmarks); each bookmark with
the events of the ten seconds around it and every aircraft's state at that
moment; and the anomaly flags.

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
| Call dropped, call suppressed | A call is dropped, or a spoken call (radio, crew, tower) is suppressed by a cooldown, limit or radio silence. The HUD's rate limit for AI lines, a cockpit message pushed off the screen by newer lines, and the AI's rules for which radio events become calls are routine and not flagged |
| Long wait | A call waits in a queue more than 3 seconds |
| Repeated call | The same speaker makes the same call 3 times within 10 seconds, each saying counted once (not its queued or cut-off entry) |
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
| Flares and chaff | `Misc+Decoy+Flare`, `Misc+Decoy+Chaff`, with `Parent` set to the releasing aircraft, where they left it, removed when a flare burns out (30 s) or chaff drifts away (20 s); an older recording's flare and chaff effects the same way without a parent |
| Ejected pilots | `Ground+Light+Human+Air+Parachutist`, with `Parent` set to the aircraft |

Object ids are hexadecimal and never zero: aircraft, projectiles, decoys and
parachutes each have their own range. Objects are removed with `-id` when
they leave the recording (a wreck when it is gone).

Events: `Destroyed` for kills, `Message` for each radio, crew or tower line
the player heard, once, when it was delivered, `Bookmark` for the player's
bookmarks, `TakenOff` and `Landed`, and `Debug` (shown with Tacview's
`/Debug:on`) for AI decisions with their reasons, orders and answers, comms
entries with an outcome or reason, flight-model effects starting and
stopping, and G-limit hits. Commas in text are escaped as Tacview requires;
line breaks become spaces.

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
hits, chaff and flares, shot outcomes, AI decisions, comms, flight events,
player and system events), the counts and the first event that differs or, when the events
match, the first difference in timing.

The checksum is FNV-1a 64 over every aircraft's exact state in id order,
computed by the app from live values once a second. States are compared
after decoding with tolerances of one and a half steps, so two recordings of
the same flight never differ just because their keyframes fall on different
ticks.

## Replays screen

**Replays** on the main menu's top bar, after Multi, opens the Replays
screen: every recording in the replays folder, newest first, the selected
one's details, and buttons to watch, keep, delete and export it. It is drawn
like the Controls screen, with the imported font, colours, title strip,
list rows and footer buttons. The entry and the screen are an opinionated
addition requested by John on 2026-09-26; their layout, wording and
behaviour are agent decisions (2026-09-26). The retail "Replay Last
Mission" button is unrelated and stays disabled.

### The list

| Column | Shows |
| --- | --- |
| Markers | A padlock when the recording is kept; a warning triangle when it did not finish (the game crashed or was closed while recording) or cannot be read |
| Started (UTC) | The date and time the flight started, in UTC, from the file name |
| Theater | The theater's name |
| Aircraft | The player's aircraft by its exact name, for example F/A-18D Hornet |
| Length | Minutes and seconds, with hours from an hour; `?` for an unfinished recording until its details are read |
| Result | Success or Failure for a Quick Mission; Ended, Quit or Restarted for Free Flight; Incomplete or Unreadable |

A second flight started in the same minute (`-2`, `-3` and so on) counts
as the newer one. Listing reads only each file's header, seek index and
footer. The list's title gives the number of recordings and their total
size.

### Details

The panel on the right describes the selected recording: its start, the
kind of mission, the theater, the weather and the local start time, the
player's aircraft, the wings and aircraft on each side, the length, the
result with how the flight ended and the pilot's fate, the player's kills,
the bookmarks with their times into the recording, the size and file name,
whether it finished normally, and whether it is kept. Wings, aircraft
counts, bookmarks and an unfinished recording's length need the whole file,
so a background thread reads them; the panel shows "Reading..." until they
arrive. The foot of the panel names the folder, where the exports go too.

### Buttons

| Button | Does |
| --- | --- |
| Watch | Opens the recording in the [viewer](#viewer); leaving the viewer comes back to this screen, with the list read again |
| Keep | Marks the recording kept, or not; its box is ticked while kept. Auto-delete never removes a kept recording. Saved at once in `replays-v1.conf` |
| Delete | Asks first, then deletes the file. A kept recording can be deleted too, with a warning. Exports made from it stay |
| Tacview | Writes `NAME.txt.acmi` beside the recording, the file `--recording-acmi` writes, and shows its path |
| Debug log | Writes `summary.txt` and `log.jsonl` into a `NAME-log` folder beside the recording, as `--recording-log` does, and shows its path |
| Auto-delete | Opens the auto-delete settings |
| Back | Returns to the main menu |

With no recordings, only Auto-delete and Back are available. Exports run on
a background thread, one at a time, so the menu keeps responding: the status
line shows "Writing ..." with the seconds so far, then the path written, or
why it failed. A recording being exported cannot be deleted until the
export finishes. Closing the screen lets a running export finish; the
session log records the result.

### Auto-delete settings

A panel over the list, with the [auto-delete](#auto-delete) rules:

- **Auto-delete:** On or Off.
- **Rule:** Keep a number, or Delete by age.
- **Keep the last:** 5, 10, 20, 50 or 100 recordings.
- **Delete older than:** 7, 14, 30 or 90 days.

Choosing a number also chooses its rule. A value written into the settings
file by hand that is not one of these shows as an extra choice. Every change
is saved at once. The panel says what the rule does and how many recordings
the next cleanup would delete, but deletes nothing itself: cleanup runs when
a flight ends and when this screen opens, and the status line then says how
many recordings it removed. Kept recordings, `.partial` files written in the
last ten minutes and files that are not recordings are never touched.

### Keys and mouse

| Input | Action |
| --- | --- |
| Up / Down | Previous or next recording; Down from the last goes to the buttons, Up from the buttons back to the list |
| PageUp / PageDown, Home / End | A page up or down, the first or the last recording |
| Enter, or a double-click | Watch the selected recording; on a button, press it |
| Delete or Backspace | Delete the selected recording, after the confirmation |
| Tab / Shift+Tab | Through the list and the available buttons |
| Left / Right | Between the buttons, and between Delete and Cancel; in the settings panel, the previous or next choice |
| Mouse wheel | Scroll the list three rows a notch; in the settings panel, move between rows |
| Esc | Close the settings panel or the confirmation, otherwise back to the main menu |

A held key repeats movement only, never Enter or Delete. Controller menu
buttons act as the arrow keys, Enter and Esc. `--snapshot-state replays`,
`replays-settings` and `replays-delete` draw the screen over a synthetic
list, for checks without recordings.

## Viewer

The viewer plays a recording back from any viewpoint. It draws the same
picture live flight drew, through the same drawing helpers and renderer
calls, rebuilt from the recording alone. Its interface, keys and layout are
agent design decisions (2026-09-26); the numbers below labelled fitted are
agent choices too.

### Opening a recording

- From the Replays screen: select a recording and press Watch (or
  double-click it). A recording that cannot be opened stays on the Replays
  screen, with the reason on its status line.
- From the command line: `tore-app --watch-replay FILE`.
- Esc leaves the viewer and returns to the Replays screen, with the list read
  again, including when the viewer was started from the command line. The
  game's own world and aircraft come back as they were.

The viewer builds the recorded world (map, weather choice, time of day,
wind and cloud deck) from the header, so a replay looks the same whatever
the viewer's own settings are. It loads the recorded player's aircraft and a
model for every other aircraft type the flight drew, and every weapon shape
the recording names.

### Playing

Playback starts from the beginning at normal speed. The playhead can sit
between ticks, so slow motion is smooth, and every picture depends only on
where the playhead is: playing backwards shows exactly what playing forwards
showed at the same moment.

- **Speeds:** 1/8x, 0.25x, 0.5x, 0.75x, 1x, 2x, 4x, 8x and 16x, forwards or
  backwards. Play starts at 1x, fast forward at 2x; pressing reverse, fast
  forward, J or L again doubles the speed up to 16x. From slow motion they
  go back to their starting speed.
- **Steps and jumps:** one tick at a time while paused, 5 seconds or 30
  seconds either way, the start, the end, and the previous or next timeline
  marker. Playing on from either end starts over from the other.
- **Timeline markers:** launches (yellow), kills (red), orders (blue) and
  bookmarks (green), from the recording's events.

### Transport bar

Along the bottom of the view: start, step back, reverse, pause, play, fast
forward, step forward and end; the speed; the time and the recording's
length; the camera, which a click steps through the flight views and the
two drone modes; the selected aircraft, which a click moves to the next one;
and Hide. Above the buttons is the timeline: click it to jump, drag along it
to scrub. The lit button shows what playback is doing. The bar uses the
Controls screen's colours, font and button style in the 640x480 interface
layer; the layer's top half is pinned to the top of the view and its bottom
half to the bottom, so the bar sits on the bottom edge of any window.

### Keys and mouse

The viewer's keys are built in, listed in the
[controls master list](CONTROLS.md#built-in-controls-outside-the-tables).
They follow the video-editor convention for J, K and L. F11 is not a view,
as in flight.

| Input | Action |
| --- | --- |
| Space | Play or pause |
| J / K / L | Play backwards / pause / play forwards; again for twice the speed |
| Up / Down | Next faster or slower speed in the same direction |
| Left / Right | 5 seconds back or forward; one tick while paused; with Shift, 30 seconds |
| Home / End | Start or end |
| PageUp / PageDown | Previous or next marker |
| Tab / Shift+Tab | Next or previous aircraft |
| F1 to F10, F12 | The flight views, on the selected aircraft |
| Backquote | Drone following the selected aircraft, then flying free, then back |
| W A S D, E / Q | Drone: move along the view and sideways, climb / descend; Shift is four times faster |
| Mouse wheel | Scrolls a panel or menu under the pointer; otherwise drone speed, or zoom in a flight view |
| Right-drag | Look around, or turn the drone |
| N / T / C | Name labels / mission timer / [Comms panel](#the-comms-panel) |
| I / F / G | [AI thinking / telemetry](#debug-panels) of the selected aircraft / guidance of its newest missile in flight; again to close |
| M / X | The [right-click menu](#the-right-click-menu) on the selected aircraft / close every panel |
| Right-click | The right-click menu on the aircraft or missile under the pointer |
| R / Shift+R | Trails on or off / next trail length |
| H | Hide or show the whole interface and the pointer |
| P | Save the view without the interface as a PNG |
| Esc | Show the interface if hidden, otherwise leave |

### Cameras

- **Flight views on any aircraft.** Tab picks the aircraft; F1 to F12 give
  the same views as in flight, from that aircraft: front, back and up sit at
  the aircraft and hide it (there is no cockpit in a replay), and track,
  threat, wing, target, fly-by and missile views work from it. The target is
  that aircraft's own target when the recording has its AI target changes,
  otherwise the player's designated target, which the recording notes with
  every command the player gives. A view that cannot be shown (no target, no
  wingman, no missile) says why and shows the aircraft from outside; once
  the aircraft has left the recording (a wreck that exploded), the camera
  stays where it was. The viewer opens in the external view of the player.
- **Drone.** Backquote switches to a drone that follows the selected
  aircraft at a fixed offset in world axes, so the aircraft stays where it
  was framed while the drone travels with it; Backquote again lets it fly
  free where it is, and again returns to the flight view. The drone starts
  where the camera was or, when that is far away, 250 feet behind, 90 feet
  to the right of and 60 feet above the aircraft: off its path, so it does
  not fly through the flares and smoke the aircraft leaves. W A S D move along
  the view and sideways, E and Q climb and descend (as in the terrain
  viewer), Shift is four times faster, and the wheel sets the speed from 20
  to 5,000 feet per second (250 at first, 25% a notch). The drone stays 10
  feet above the ground and moves in real time, so shots can be framed while
  playback is paused. Speed range and offsets are fitted.
- Right-drag looks around, at the mouse-look sensitivity set on the
  Controls screen.

### What is drawn

Everything live flight draws outside the cockpit, rebuilt for the tick
under the playhead:

- Aircraft, fixtures, weapons, tracers, debris and effects from the
  recorded frames, blended between ticks the way live flight blends its
  last two ticks. Frames are decoded a second at a time from the nearest
  keyframe; the eight most recently used seconds stay decoded, so playing
  either way reads each second once.
- Ejected pilots, with the imported ejection art.
- Buildings and airport objects, minus those the recording shows destroyed
  by that tick.
- Smoke and contrails rebuilt from their release ticks with the
  simulation's lifetimes, rise and caps; effects from their start ticks.
- The player's wing vapor, rebuilt by stepping the vapor history over the
  last 250 ticks of recorded poses from one of live flight's own commit
  ticks, so it matches live flight exactly once two seconds of history
  exist. Recordings keep attitudes, not turn rates, so the roll rate that
  shortens the vapor is worked out from the attitudes a tick apart
  (fitted).
- The weather, re-stepped one tick at a time as live flight steps it, with
  a snapshot every second (the environment's own state every ten seconds)
  so a seek is at most a second of stepping. Live flight steps the weather
  from the view camera; the viewer steps it from the recorded player's
  forward view, so each tick's weather is the same however it is watched
  (fitted: only the sun-whitening smoothing can differ from what the pilot
  saw). The palette is then resolved for the viewer camera's height. A
  headless probe never steps the weather, so its recordings keep the launch
  sky.

There is no cockpit, HUD, instrument panel or mirror in a replay.

### Interface parts

- **Name labels** (N, on at first): each aircraft's label over it in its
  side's colour (friendly blue, enemy red, neutral green, grey when
  unknown), drawn at the view's full resolution with a dark shadow; the
  selected aircraft's is in brackets. Aircraft over 100 nautical miles away,
  wrecks on the ground and the aircraft the camera sits in have none.
- **Mission timer** (T, on at first): mission time as `mm:ss.t` and the tick.
- **Subtitles:** radio, tower and crew lines the player heard, each once
  from its delivery, and the cockpit messages the HUD showed or queued, for
  four seconds from their tick, newest lowest, up to three.
- **Comms panel** (C): every recorded comms and audio entry up to the
  playhead, each line once with its final outcome, filtered by kind and
  aircraft; see [the Comms panel](#the-comms-panel). While it is open the
  subtitles are not drawn, since it lists the same lines.
- **Flight path trails** (R, off at first): each aircraft's and guided
  weapon's path over the last 30 seconds (Shift+R: 10, 30, 60, 120 or 300)
  as a thin line in its side's colour, a weapon's paler than its owner's.
  They are drawn in the 3D view, so terrain and aircraft hide them, from
  path samples ten times a second plus where the aircraft is drawn now.
  Colours and lengths are fitted.
- **Hide UI** (H or the Hide button): hides the bar, labels, subtitles,
  timer, debug panels, the right-click menu and the pointer; trails stay as
  chosen, and playback and camera keys keep working. Esc brings the
  interface back.
- **Screenshots** (P): the 3D view without any interface, at the view's
  size (up to 1920x1080), saved as
  `screenshots/<recording>-tick<tick>.png` under the app data folder. The
  PNG writer is built in: uncompressed image data in stored deflate blocks,
  so a full-HD picture is about 6 MB.

Trails, fallen buildings and the weather need the whole recording read
once. A background pass does that when the viewer opens, a second at a
time; until it has reached a moment, trails there are shorter and the sky
waits at the last moment it has reached.

### Sound

John asked on 2026-09-26 to hear the communication in a replay: the
voices, tones and effects played back at normal speed. At exactly 1x
forwards the viewer plays what the player heard, through the same sound
calls live flight makes, so it sounds as the flight did. How it is
scheduled is an agent design (2026-09-26); the code is
`tore-app/src/replay/sound.rs`.

- **Radio, crew and tower lines** the player heard, each once and in
  recorded order, queued behind the line before as in flight: radio calls,
  crew remarks, tower lines, the tower's reply to the player's own request
  (which replaces waiting tower speech), and recordings played straight
  into the cockpit, such as the death scream. Each plays the way its
  recorded route plays in flight: the radio queue, the tower's own queue,
  or straight in. The recording lists everything that happened to a line,
  so only its delivery speaks: an entry recorded as not heard, or as
  queued, held back, dropped or cut off, stays silent. Cockpit messages
  have no voice.
- **The player's wing orders:** every order that went out on the radio
  plays its voice, which cuts off waiting wingman speech as in flight. An
  order the wing refused has no voice but still cuts the speech off, as in
  flight; one the game refused before it went out (no AI wing, no airport
  to land at) does nothing. Orders between AI aircraft are silent.
- **Cockpit sounds:** the seeker tone at its recorded loudness, the stall
  warning, gear, flap, hook and air brake sounds, the ejection warnings,
  seat and parachute, and a friendly wingman ejecting. They are the
  player's cockpit, whichever aircraft the camera follows. Queued tower
  speech stops when the player's aircraft is lost, and when the runway
  under the player's landing clearance is destroyed, as in flight.
- **Traveling sound:** impacts, explosions and the player's weapon
  releases, aircraft and missiles passing the camera, and sonic booms,
  through the flight's [distance, delay and stereo model](audio.md#traveling-sound)
  with the viewer's camera as the listener. The model steps once for every
  recorded tick played, so sound takes as long to arrive as in flight. A
  camera inside the player's cockpit hears the player's releases at once,
  as the cockpit does. A cut to another view, another aircraft or the drone
  starts the pass detector afresh, so a cut never sounds like something
  flying past.
- **Engine:** the engine and afterburner loops of the aircraft the camera
  follows, from its recorded engine, afterburner and throttle, and its
  start or stop sound when its engine lights or stops while it is watched.
  Following another aircraft swaps the loops without cutting off speech.

What stops it:

- **Pause** freezes everything playing, a line mid-word included, and
  playing on resumes it. Playback pauses itself at the end, so sound
  freezes there too.
- **Any other speed, reverse, dragging the timeline, or a jump** of the
  playhead (the arrows, a marker, Home, End, a click on the timeline, a
  step while paused) cancels waiting speech and silences tones, loops and
  traveling sound. Back at 1x forwards, sound starts afresh from the
  playhead: the seeker tone, stall warning and engine sounding at that
  moment come back at once, and when the playhead sits exactly on a tick,
  that tick's cues play too.
- **Leaving the viewer** stops everything.

Fitted details, agent decisions (2026-09-26):

- The seeker tone's loudness follows the seeker's signal quality or the
  estimated hit chance in flight. The recording keeps it in steps of 0.05
  (see [Communication events](#communication-events)), so the replay's
  tone can be up to 0.05 from the flight's between steps. A recording made
  before the loudness was kept names only the tone, and the replay uses
  the live strength at 50 percent: radar lock 0.7, radar search 0.425,
  infrared lock 0.575 and infrared search 0.2875, times the seeker volume.
- An infrared lock on a surface weapon plays a different tone. The
  recording says whether the weapon aims at the surface; in a recording
  made before it did, the replay takes the lock to be of the same kind as
  the search tone before it.
- The engine loop plays while the recorded engine runs and the aircraft
  is neither destroyed nor abandoned. AI aircraft are recorded as they are
  drawn, engine always running and afterburner never lit, so a watched AI
  aircraft hums at its recorded throttle.
- Only missiles with a reviewed profile pass the camera audibly, as in
  flight.

Not in a replay:

- **Music.** The recording keeps every change in the inputs that choose a
  score (`audio.music`, recorded when the flight had sound), but the
  replay does not hand them to the music yet, so a replay has no music.
- Sounds that leave no record: the mixer's own choices, such as speech
  dropped from a full queue, and whatever a headless probe does not run
  (crew voice, music and cockpit messages).

Scheduling is checked without listening. The tests play a synthetic
recording in frames of every length, at every speed and through jumps and
pauses. `TORE_REPLAY_SOUND_LOG=1` writes every cue the viewer schedules,
with its tick, to the session log (traveling sound only on ticks with an
impact, explosion or release, and loops only when they change), and
`TORE_REPLAY_SOUND_FILE=FILE cargo test --locked -p tore-app replay::sound -- --ignored --nocapture`
prints the same for a whole recording played at 1x, without a display.
Nobody has listened to a replay yet.

### Captures and timing

For checking the viewer without a keyboard:

```sh
tore-app --watch-replay FILE --capture-replay OUT.ppm --replay-tick N \
    [--flight-view 0..11] [--replay-aircraft ID] [--replay-drone] \
    [--replay-ui labels,timer,trails,comms,subtitles] [--replay-clean]
```

The capture waits for the background pass, draws the frame at tick `N`
with the interface as chosen, writes a PPM like `--capture-flight` and
exits. A path ending in `.png` instead saves the 3D view without the
interface through the same code and PNG writer as P. `--replay-clean`
starts with the interface hidden, as H hides it.
`--replay-speed S` starts playing at a ladder speed, negative for reverse,
and with `TORE_PERF_FRAMES=N` the viewer reports frame timings like live
flight. `--replay-panels thought,telemetry,guidance,comms,menu` opens
[debug panels](#debug-panels), or the right-click menu, on the selected
aircraft for the capture.

Measured on the development Mac (Apple M3, 1440x1080 view, release build,
other work running on the machine) with a synthetic ten-minute recording of
17 aircraft, trails and labels on: 33 ms a frame at 16x forwards, 36 ms at
16x backwards and 39 ms at 1x, against 38 ms for live free flight measured
at the same time. The GPU dominates: rebuilding the moment took 6 to 7 ms of
each frame, most of it the same aircraft drawing live flight does, and the
interface about 1 ms. While the weather snapshots are first being built,
about 1,200 ticks a frame, frames take about 2 ms more.

### Known limits

- No cockpit, HUD or instruments and no music yet, and the keys cannot be
  rebound. Replay sound has not been checked by ear.
- Checked by eye with synthetic recordings and headless AI probe
  recordings; a recording of a flight flown by hand has not been watched
  yet.
- Gun rounds have no trails.

## Debug panels

Right-click an aircraft or a missile, in a replay or in live flight, to see
why it does what it does: an AI aircraft's thinking, any aircraft's
flight-model telemetry, a missile's guidance, and every radio call, order
and sound with its outcome and reason. This is an **opinionated addition
requested by John on 2026-09-26**. The panels' layout, keys, colours and
wording, the menu's items and the pick distance are agent decisions
(2026-09-26).

### Opening them

- **In a replay:** right-click an aircraft, a missile or a name label: press
  and release the right button without moving more than 4 pixels. Dragging
  with it still looks around. Keys act on the selected aircraft: I its AI
  thinking, F its telemetry, G the guidance of its newest missile in flight
  (each again to close), C the Comms panel, M the right-click menu, X closes
  every panel.
- **In live flight:** Escape, then Pref, then **Debug panels?** (off by
  default, saved with the other flight preferences). While it is on, the
  mission timer shows at the top of the view and a right-click without
  dragging opens the same menu; a right-drag is still mouse look when mouse
  look is on (Controls, Mouse tab). With mouse look off and the right button
  bound to an action, the button keeps its binding and the menu does not
  open from the mouse. Letter keys stay flight keys, so the panels open from
  the menu. The flight carries on while the menu is open: the menu's keys
  (the arrows, Tab, Home, End, PageUp, PageDown, Enter, Space and Esc) work
  it until it closes, and every other key still flies. Opening the Escape
  menu closes it.

### The right-click menu

The menu picks through the camera that drew the frame: the aircraft or
missile drawn nearest the pointer, within 14 interface pixels scaled with
the view (about 32 pixels on a 1080-line view), or the aircraft whose name
label is under the pointer. It is the same on any window shape, windowed,
fullscreen or letterboxed. On a panel it is the panel's aircraft or
missile.

| Right-click on | Items |
| --- | --- |
| An aircraft | Follow (chase view): F10 on it. Cockpit view: the front view from it, the player's own cockpit in live flight. Drone here (replay): the follow drone beside it. AI thinking (aircraft the AI flies). Telemetry. Comms for this aircraft. Name labels. Flight path trails (replay) |
| A missile | Guidance. Drone here (replay): a free drone beside it. Go to the shooter. Name labels. Flight path trails (replay) |
| Empty space | Every aircraft in the picture, grouped by side and wing, to jump to: a replay selects it, live flight puts the chase view on it. Then the switches |

Up, Down and Tab (Shift+Tab back), Home, End, PageUp and PageDown move;
Enter, Space or Right choose; Esc or Left close. The pointer moves the
highlight, a press and release on the same item chooses it, a press outside
closes the menu, and the wheel scrolls a long list.

### The panels

Up to two panels show at once, one on each side, plus the Comms panel along
the bottom. A new panel takes a free side, or the side whose unpinned panel
is older; with both pinned it says so and opens nothing. Each has **Pin**
and a close button (x) in its title. An unpinned AI thinking or Telemetry
panel follows the selected aircraft (in live flight, the aircraft the
camera is on); a pinned one stays on its aircraft; a Guidance panel always
stays on its missile. The mouse wheel over a panel scrolls it.

A tree panel draws one display tree as an indented list: each label, its
value with its unit in a column beside it, and the value's "because" line
beneath in grey. Numbers read to the precision a pilot would use: whole
feet, pounds and knots with thousands separators, tenths of a mile, degree
and percent, hundredths of a G, a second and a ratio (`x0.84`). Aircraft
named in a tree read as their labels. The line under the title says when
the sample was taken; in a replay it is the latest one at or before the
playhead, and one older than 2 seconds (an aircraft that has gone) shows in
amber.

| Panel | Tree | Shows |
| --- | --- | --- |
| AI thinking | `ai.thought` | What an AI aircraft considered: mission, activity, target and why, geometry, weapon and whether it may fire, defense, motion, steering, controls and fuel. See the [AI thinking record](#ai-thinking-record) |
| Telemetry | `flight.telemetry` | Air data, load, power, drag, and each effect the flight model applied with its because line |
| Guidance | `weapon.guidance` | A guided weapon's seeker and steering |

### The Comms panel

Every comms and audio entry up to the playhead, newest at the bottom, whole
rows only. Entries that share a message number make one row, shown once
with its latest outcome: a line queued and then delivered, dropped or cut
off, or an order, request or report and each recipient's answer. A row
sits where its newest entry is, so a line that waited in the queue appears
when it was said, and before that moment it reads as queued. Each row
gives the time, the kind (RADIO, CREW, TOWER, HUD, ORDER, REQUEST, REPORT,
ANSWER, TONE, STALL, MUSIC, EFFECT, RELEASE, EJECT, DEVICE), who said what
to whom, and the outcome in brackets: green when it went out or was acted
on, amber while it waits or when some recipients took it and some did not,
red when it was held back, dropped, refused or cut off, grey when there was
nothing to act on (a check that found nothing to say, or a line said on a
radio the player does not hear). Beneath come the reasons, the trigger,
the rolls, the wait, the line's earlier states with their times, anything
else recorded with it, and one line per recipient with its answer and
why. The same sound repeated within half a second, such as a gun burst's
release sounds, is one row with its count. An entry without a message
number, such as a cockpit message or a tone, is a row of its own.

The chips filter by kind: **Radio**; **Orders** (orders, requests, reports
and each recipient's answer); **Tower**; **Crew** (crew remarks and cockpit
messages); **Tones** (seeker tones, warnings, music and sound effects). The
aircraft chip steps through the aircraft, keeping the entries it sent,
received or is named in; "Comms for this aircraft" sets it. The filters are
kept when the panel closes. The list follows the playhead in either
direction at any speed; the wheel scrolls back, a note says so, and wheeling
down to the newest entry follows again.

### Where the data comes from

- **A replay** reads trees and entries from the recording at the playhead.
  Trees are decoded one chunk (a second) at a time and the last six chunks
  stay decoded, so playing either way decodes each second once. Everything
  shown depends only on the playhead, so playing backwards shows what
  playing forwards showed.
- **Live flight** shows what the mission recording writes, as it writes it:
  while the panels show, the recorder hands them each display tree it
  builds and every comms and audio entry (the newest 512). The trees come
  from the same builders, fed with the same records at the same moments,
  including the history a tree reads (when an activity began and why, the
  recent changes, a missile's closest approach), so a live panel shows
  exactly what a replay of the flight will show, a tenth of a second or so
  behind for an AI aircraft's thinking. Names come from the roster a
  recording registers. With recording off (`TORE_RECORD_MISSIONS=0`, and
  captures, which do not record unless it is `1`) the panels say so and
  stay empty.
- A recording made before display trees were recorded has none, and its
  panels say that nothing was recorded.

The panels only read. Behaviour probes give identical output with the
panels' code in place, and nothing a panel or the menu does reaches the
simulation: the menu moves the camera and opens panels.

### Captures

`--replay-panels thought,telemetry,guidance,comms,menu` opens panels, or the
menu, on the selected aircraft for `--capture-replay` with a `.ppm` path (a
`.png` capture is the clean view, without any interface). For flight,
`--debug-panels` turns the Pref row on for one run and `--flight-panels`
with the same list also opens them for `--capture-flight`: AI thinking and
the menu on the first aircraft the AI flies, telemetry on the player,
guidance on the newest missile in flight, and the Comms panel. Captures do
not record, so they show trees and entries only with
`TORE_RECORD_MISSIONS=1`, and a capture's single frame comes before the
first tick has been recorded.

### Known limits

- The panels sit in the 640x480 interface layer, centred and scaled with the
  view, so on a wide window the sides of the view stay free.
- Live flight shows no drone or trails, and the panels open from the menu
  only.
- Radio calls in recordings made before the communication journal was
  recorded name AI speakers by their words, not their aircraft, so the
  aircraft filter misses them.

### AI thinking record

Every AI aircraft writes down why it did what it did, every tick, so the AI
thinking panel, recordings and debug logs can give real reasons with real
numbers. The record is **write-only**: no decision ever reads it. It holds
copies of values the AI worked out anyway, plus explanations recomputed from
the same rules. The golden behaviour fingerprints prove the AI flies exactly
as it did before the record existed, including one test that reads every
record and drains the journal after every tick of two full synthetic
missions. What the record contains is an agent decision. The code lives in
`tore_sim::ai::thought`.

There are four parts.

**The controller record** (`Controller::trace()`) is what one aircraft's
decision loop considered on its latest tick. A paused or repeated tick leaves
it as it was.

| Part | What it says |
| --- | --- |
| Path | Whether the decision ran, or the aircraft was already destroyed. |
| Events | Hits, targets that disappeared, and every missile warning: the delay before the aircraft notices it, the tick it takes effect, and on arrival the countermeasure roll, the devices released, the reaction and whether it made a radio call. Then the most urgent reason and whether it restarted the aircraft's plan or let it carry on. |
| Fuel | Fuel, endurance, time to reach home, the fuel state and whether the aircraft is heading home. |
| Target | The previous target and how the new one was chosen: weapons held, the mission's choice (with why it was refused, if it was), kept because it is inside 20,000 ft, or ranked by distance and penalties with the winning score. |
| Geometry | Range, horizontal range and the angles to the target. |
| Weapons | Every carried store with the first rule it breaks: switched off, needs radar that is off, needs a sensor track, wrong kind of target, empty, or outside its envelope. Outside the envelope, every reason: too close, too far, too far off the nose, outside its zone. Usable stores carry their hit chance and score. Then the store chosen, whether the lock holds and which condition failed, exactly what the weapon service was told, its phase before and after, its outcome and its next deadline. |
| Motion | Which branch flew the aircraft: missile defense, rejoining a charge or patrol, searching along a bearing, an ordered approach or maneuver, investigating an old contact, formation (slot point, aim, speed, bank), carrying on with the current maneuver, not yet due for a new choice, or a new choice. Also the quarter-second clock and when the next choice is due. |
| Maneuver | For a new choice: going home, a missile reaction, no target, target straight above or below, evasion or the ordinary approach; the tactical situation, the quadrant (ahead or behind, facing or not) and its experience percentages, the choice, any fitted rule that stood in for an unknown one, the last-ditch move and the motion request. |
| Resolve | Where the pitch came from (asked for, or the fitted engagement pitch with its inputs), the speed, the pursuit offsets, steering point and regulated speed, and how the maneuver ends. |

Beside it, `Controller::draws()` lists every random draw of the tick: what
the draw decided (for example "best attack" or "countermeasure roll"), the
source line, the value, the threshold it was compared with and whether it
passed. `last_batch()`, `weapon_phase()`, `weapon_deadline()`,
`active_maneuver()` and `search_contact()` show the current state.

**The actor record** (`AiActor::trace()`) is what the mission decided around
the controller on its latest tick.

| Part | What it says |
| --- | --- |
| Path | Destroyed, a training target, a takeoff or landing sequence, or the ordinary decision. |
| Airfield gates | Turn to go, runway free, earlier wing members down, a free parking slot, and whether a joining wingman's leader is landing. |
| Ejection | The hazard found and seconds to impact, whether a takeoff or landing phase judged it, whether it was a catastrophe or earned a go-around, and whether the pilot ejected. |
| Airfield | Whether a landing started, why the aircraft left a sequence (its leader stopped landing, or a missile threat), and what the sequence saw and commanded. |
| Dropped warnings | Missile warnings thrown away while taking off or landing, or by a training target. |
| Forgotten attacks | Remembered attacks dropped after 240 ticks without news, or because the attacker is gone. |
| Targets and stores | What the aircraft's own sensors allowed it to target, its stores' view of the target, and which aircraft the stores were aimed at. |
| Engagement | Whether it is holding formation, the role and stance in force, each remembered attack and why any is ignored, the choice, and a ranking of up to 16 aircraft: why any cannot be a target, its priority, its distance and penalties, and its score. |
| Rejoin and search | An escort flying back inside its leash or a patrol returning to its region, a bearing to search along, and an old contact to investigate. |
| Bingo | A bingo landing ordered this tick. |
| Controls | The maneuver flown, the terrain floor, the steering adapter's whole output (the controls, its fallbacks and the attitude it asked for) and the attitude and speed the flight model delivered. |

`AiActor::route_draws()` lists the draws of the private route home. In the
app, `AiWings::decoy_rolls()` lists each missile's roll against a released
decoy in the latest step, with its draw and threshold (the bridge clears
its decoy generator's draw log at the start of each step, which never
touches the generator's state), `AiWings::decoy_draws()` the draws
themselves, and `AiWings::station_weapon()` the weapon on a station.

**Missile defense** (`AiActor::defense_decision()`) now also explains itself:
the maneuver the evidence calls for (notch or jink) and its heading and
pitch even when it is not flown yet, whether a dive is safe, and each reason
behind maneuvering or releasing chaff and flares now.

**The message journal** (`AiMission::take_journal()`, or
`AiWings::take_ai_journal()` in the app) lists the messages between
aircraft. The host drains it once per tick; if it is never drained it keeps
the latest 4,096 entries and counts the rest. Each entry has the tick, the
sender, the content and every recipient's outcome with its reason.

| Message | Outcomes |
| --- | --- |
| Attack evidence, from the aircraft that saw the attack to itself, its wing leader and its escorts | Queued for the next tick, delivered, ignored (with why: a report about someone else, an unknown reporter, a recipient that left, holding formation since a recall, or a projectile already known at the recall) and forgotten after 240 ticks. A refresh of evidence already held is not a new message. |
| A neutral leader releasing its wing after a perceived attack | The attack that triggered it and the leader's own outcome. |
| Wing orders over the wing channel (break, approach, spacing, formation, wing control, target assignment, land) | Each recipient's outcome: applied, rejected with the reason, accepted without motion, or motion installed. |
| An escort's target or priority changing | Journaled once per change. |
| Missile warnings, sent by the launcher | Due at a tick, received with the reaction, or dropped with the reason. |

What cannot be explained yet:

- The record explains this game's rules, not the original's. Where the
  original's rule is unknown, the record names the fitted rule that stood in
  for it (see [behavior provenance](behavior-provenance.md)).
- The weapon lock is a simple host test (target ahead, not behind terrain),
  so a lock failure can say only which of those failed.
- The formation guidance explains itself only through its phase and trace;
  its internal plan is not recorded.
- The ejection monitor's own once-a-second roll is inside the ejection
  code, so the record says the pilot ejected but not the roll.
- Player orders, radio calls and cockpit voices belong to the
  [communication journal](#communication-journal), not to this record.
- The flight model's reasons are a separate record; this one stops at the
  controls the flight model received.

## Communication journal

Every radio call, wingman reply, crew remark, tower line, AI text line and
player order is written to a journal as it happens: when, who said it to
whom, the words and recordings, what triggered it, any random roll it used
with its threshold, and what became of it, with the reason. Lines that were
never said are kept too, with the rule that held them back. The journal is
**write-only**: no rule reads it and writing it never draws a random number,
so what is said and when is the same with or without it. Tests run the same
scripted flight twice, draining the journal every tick and never, and hear
the same lines at the same ticks with the same rolls left over. The shape of
the records is an agent decision (2026-09-26).

The code is `tore-app/src/comms/journal.rs`. The mission recorder drains
it every tick and records each entry as an event
([communication events](#communication-events)); a flight that is not
recorded never drains it, and it simply stays within its bound.

### What an entry holds

| Field | Meaning |
| --- | --- |
| Time | Simulation seconds on the producer's clock; the recorder stamps its own tick |
| Call number | Shared by every entry about one line: queued, then delivered, dropped, cancelled or cut off |
| Source | `RADIO`, `REPLY` (a wingman answering the player's order), `CREW`, `TOWER`, `HUD` (the AI's text lines), `ORDER`, `CHATTER` (an AI radio event before it becomes a call) or `MUSIC` |
| Speaker and audience | The aircraft id (0 is the player), the name as printed (`Red two`, `RIO`, `YOU`), and who it was addressed to: the cockpit, the flight leader, the flight, the player, the airport frequency, or the wingmen an order addressed |
| Words | The text, the recording stems, the route (radio, airport or direct) and whether radio silence may drop it |
| Trigger | A typed cause with its numbers, for example "infrared missile release at aircraft 3", "hit by aircraft 5 (aircraft, gun rounds)", "fuel state bingo", "situation Defensive, target 5000 ft: break" |
| Rolls | Each draw in the order made: "roll 37 < 50: Fox call", "roll 12 < 40: Splash one with the type name", "roll 13 mod 8 = 5: hit call", including draws a held call still used up |
| Outcome and reason | See the next table |

| Outcome | Meaning |
| --- | --- |
| `queued` | Waiting in the channel until it is due, or in the tower's own queue until it expires |
| `delivered` | Printed and played, with the seconds it waited; a tower notice also carries when it entered the tower's queue. An AI text line: shown on the HUD |
| `dropped` | Radio silence dropped routine chatter when it was sent, or a full queue pushed out its oldest line |
| `suppressed` | A rule held it back, with the time left on a cooldown or limit |
| `unheard` | Said, but the player's radio does not receive it: another flight, an enemy flight, the player is down, or the speaker has no radio identity |
| `replaced` | A newer notice about the same thing took its place, or an unread AI text line was overwritten |
| `expired` | A tower notice older than 15 seconds |
| `cancelled` | Taken off a queue: the tower answered the player's request, the player's aircraft was lost, the runway became unusable, the player left the approach, the wingman went down, or the aircraft entered an airfield sequence or left formation |
| `interrupted` | Delivered, then cut off by the player's wing order voice |
| `answered` | An order's answers, one per addressed wingman, and who replied |
| `rejected` | An order nobody received, and why |
| `silent` | A crew coaching check that found nothing to say, with the rule and the rolls it drew |
| `noted` | A state change: the crew's comment gate, or the music's inputs |

Shared names follow `tore_replay::vocab::outcome`; `unheard`, `interrupted`,
`answered`, `silent` and `noted` are the journal's own.

### Reasons, by producer

- **The channel.** Radio silence, the 64-call queue, the wait before
  delivery, airport calls cancelled, and lines cut off by a wing order
  voice.
- **Radio calls.** The listener rule; a release with no target; the 4 s bomb
  and gun cooldowns; unguided hits limited to one per shooter every 8 s and
  one every 4 s overall; "I'm hit" from gun rounds once per aircraft every
  8 s; ground kills after a bomb kill (4 s); friendly fire beyond 52,800 ft
  and its 6 s cooldown. Rolls: the Fox chance under 50, "Splash one" at 40
  or more, contact size words, and every variant.
- **Crew.** Changes in whether the crew may comment (aircraft lost, the
  eject warning, radio silence, not in free flight, or why nobody can coach
  a single-seat player: no wingman, wingman down, a different target, or
  beyond 15,000 ft). Each coaching check: the situation, the previous one,
  the range, the rule that chose the line or why none did, the next check
  time and the rolls. G strain and the -1 G crossing count, fuel states,
  and missile warnings, including a missile held back by the shared 6 s
  limit, which is then never called.
- **Tower.** Each trigger (runway free or occupied, airborne, climb-out,
  landing clearance, wind, landing score, welcome, a wingman's airfield
  phase or go-around), the 15 s lifetime, the 24-notice queue, and
  notices coalesced by aircraft and topic.
- **AI chatter.** Why a new target makes no contact report: only the first
  two aircraft of a flight report, the 15 s cooldown, never the same target
  twice, the 20 s block after accepting an attack order, or a target that is
  not a living airborne aircraft. Events pushed out of the 64-event queue.
- **Player orders.** Every addressed wingman's answer: applied (with or
  without motion), rejected with the receiver's reason, rejected because its
  sensors cannot see the target, or skipped (bugged out, flown by a human,
  already landed, taking off or landing, no base); the silent side orders
  with their outcomes; a refusal before anyone received it (no wingmen,
  all bugged out, no valid hostile target, no airport selected); and which
  wingman replied and with what, or why nobody did.
- **AI text lines.** Formation reports queued, held by the 10 s limit per
  aircraft, replaced, pushed out of the 16-report queue, cancelled, and
  shown; the activity line held by its 2 s limit, overwritten unread, and
  shown.
- **Situation music.** When an input changes: the score the inputs ask for
  and why each input is on (the designated enemy inside or beyond
  40,000 ft, a hit within 30 s, an AI aircraft aiming at the player within
  4 s, missiles guided at the player, success, home, deck, takeoff,
  ejection).

### Repeats and bounds

A gun fires one release per round, so a rule is listed once per window:
once per cooldown or limit for each aircraft, and at most every 4 s for a
rule without one. Crew gates are listed when they change; the 3 s channel
hold is not listed as a gate, because every delivered line shows it. At
most 1,024 entries wait between drains; a host that never drains keeps the
newest and counts the rest as lost.

### Not visible yet

- **The mixer's own decisions** inside the audio device: speech dropped
  because its queue is full or sound effects are off or paused, when a
  delivered line actually starts after the lines ahead of it, and which
  situation score really plays (its lockout, the once-per-flight scores,
  the Valkyries toggle and failed-load retries). The journal records what
  the inputs ask for.
- **The order voice without audio.** With no audio device nothing is cut
  off, so no line is marked `interrupted`.
- **Headless AI probes** run no crew voice, music or HUD delivery.

### Draining and host hooks

- Drain with `Comms::take_journal` once a tick. `radio_calls::step` moves
  the wing's journal (`AiWings::take_journal`) into the channel's each
  tick, so one call gets everything except the music's entries, which come
  in `flight_music::Step::journal`.
- `Entry::describe` prints one plain-English line; `Cause`, `Reason` and
  `Outcome` print their own text for event fields.
- Host hooks, all in place in `main.rs`:
  `Comms::cut_off(now, Reason::OrderVoice)` where the order voice
  interrupts wing speech, `Comms::cancel_airport` and `Entry::tower_reply`
  for tower replies, `Entry::clearance_cancelled` where a destroyed runway
  under the landing clearance cuts tower speech, `Entry::order_refused` for
  orders refused before delivery (no AI wing, no landing site, an error),
  and `Step::radio_calls` for the mission result calls with their trigger.
  Every order the wing received is journaled with the `radio` route: the
  host plays its voice, even an empty one.
- The recorder drains it with `Recorder::drain_comms` after the tick's
  radio is delivered (live flight and `--record-mission` probes alike),
  notes entries its bound threw away, and takes the music's entries with
  `Recorder::comms`.

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
| `--watch-replay FILE` | Opens the [viewer](#viewer) on a recording (this one needs the game media and a display); with `--capture-replay OUT.ppm --replay-tick N` it writes one frame and exits, see [captures](#captures-and-timing) |
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
  `begin`, `note` and the other noting methods, `drain_comms`, `end`,
  `finish`), `replay/library.rs` owns the folder and auto-delete (`list`,
  `plan`, `cleanup`, `order_key`), `replay/screen.rs` is the Replays screen,
  and `replay/cli.rs` the command line, whose `log` and `acmi` the screen's
  export buttons call on a background thread. `Tick::journal` hands `begin`
  the AI message journal the host drained for the tick.
- `replay/recorder/why.rs` turns the AI's and the flight model's records
  into reason events and display trees, with its rates, triggers and
  debouncing; `replay/recorder/journal.rs` turns the two journals into
  events. `replay/trees.rs` holds the pure tree builders (`ai_thought`,
  `flight_telemetry`, `weapon_guidance`) and the shared wording
  (`effect_line`, `draw_text`, `service_text` and the labels), for a live
  panel as much as for the recorder. The recorder reads only through
  shared references; draining the two write-only journals is its one
  change, and a test flies the same synthetic mission with and without
  recording and finds the same flight, decision and weapon state.
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
- The viewer lives in `tore-app/src/replay/`: `viewer.rs` (the screen:
  loading, cameras, keys, drawing), `host.rs` (the `Screen::Replay`
  plumbing in the app), `panels.rs` (the debug panels and the chunk cache
  of recorded trees), `context_menu.rs` (the right-click menu, picking and
  the click-or-drag rule), `live.rs` (the panels and menu in live flight), `playback.rs` (any tick's picture, smoke and wing
  vapor), `clock.rs` (playhead, speeds, steps, markers), `tracks.rs` (the
  background pass: trail samples, building hit points, the player's path
  for the weather), `weather.rs` (weather snapshots), `sound.rs` (which
  recorded sounds play and when, as plain-data cues), `drone.rs`,
  `trails.rs`, `overlay.rs` (the interface and its pointer handling) and
  `png.rs`. Tests use a synthetic recording (`replay/fixture.rs`); a longer
  demonstration recording over the imported Ukraine map, for looking at the
  viewer by eye, is written by
  `TORE_REPLAY_DEMO=FILE cargo test --locked -p tore-app replay::demo -- --ignored`
  (`TORE_REPLAY_DEMO_MINUTES` and `TORE_REPLAY_DEMO_AIRCRAFT` size it).
- Tests use synthetic recordings only; golden outputs live in
  `crates/tore-replay/tests/golden` and are rewritten with
  `TORE_UPDATE_GOLDEN=1 cargo test --locked -p tore-replay`.
