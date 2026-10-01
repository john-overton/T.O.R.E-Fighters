# First Flight reel: engine reconnaissance

Research mode, 2026-09-30. Source: main `fb9c2ec` (Release 0.1.3), produced on
`reel/v0.1-promo`. This is a survey of the shipped Rust implementation, not new
research into original retail behavior. No simulation, flight model or AI changes
are authorized by this production. Main has app, sim, formats, replay, extraction,
input and diagnostics crates. There is no server crate on this revision.

## Aircraft

`AircraftId::SELECTABLE` in `crates/tore-formats/src/aircraft.rs` lists 14:
F/A-18D Hornet, Rafale C, F-14D Tomcat, A-4E Skyhawk, X-31 EFM,
MiG-29 Fulcrum-C, Su-27 Flanker-B, MiG-21 Fishbed, Su-25 Frogfoot-A,
MiG-23 Flogger-B, Su-35, F-22A Raptor, F-22N Raptor and F/A-XX.
All have app model, cockpit and flight paths. F/A-XX is an existing opinionated
variant using the F-22N donor, not another retail aircraft. F18.PT is F/A-18D;
RAFALE.PT is Rafale C. F14.PT is F-14D, so an F-14A nameplate would be false.
`Airframe::load` validates source geometry and selects the established rig.
See `docs/aircraft-import.md`, `docs/spec/roster-aircraft.md`,
`docs/spec/fa-xx.md`. All fourteen identities were subsequently rendered in this pass.

## Theaters and lighting

`tore-formats/src/theater.rs::THEATERS` lists 16 base maps: APA, BAL, CUB,
EGY, FRA, GRE, IRA, KURILE, LFA, NSK, PGU, SPA, TVIET, UKR, VLA and WTA.
Runtime-validated display names, in that order: Panama, The Baltics, Cuba,
Egypt, France, Greece, Iraq, Kuril Islands, Falkland Islands, North/South Korea,
Persian Gulf, Pakistan, North Vietnam, Ukraine, Vladivostok and Taiwan.
Names were read from the user's T2 data, not inferred from these codes.
The catalog also exposes 59 reviewed layout variants through developer flags.
`--validate-maps` constructs imported maps headlessly. Base maps are the
consumer-facing scope of the montage; variant layouts do not imply 59 new theaters.

`--weather-condition 0..5` chooses clear/cloudy/foggy/dawn/sunset/night.
`TORE_WEATHER_TIME=HH:MM` overrides launch time. `World::step_weather` and
`tore-sim::environment` own a fixed-tick clock, imported palette lighting,
sun/moon/stars and weather. Visual sampling blends lighting. The director may
sample a changed visual clock, without changing recorded aircraft dynamics.
Replay WeatherTrack supports seeking and snapshots. Probe recordings explicitly
omit weather stepping, so the director must declare and initialize visual time.

## Quick Mission formations

`QuickMission::wing_launches`, `MissionLayout::plan`,
`Combat::mission_aircraft` and `AiWings::build` form the production setup path.
`ai_wings::mission_spawns` uses existing formation slots, with up to five
aircraft per wing and three wings per side. AI runs the existing controllers,
wing orders, steering and flight dynamics. No authored aircraft trajectories
will stand in for AI. The existing `--ai-probe-ticks --record-mission` harness
uses those launch and simulation services without a display. `--probe-wing-size
4 --probe-wing-only` isolates a player plus three wingmen; `--probe-wing-order`
can issue existing orders. `--ground-start` and `--maneuver takeoff` use the
existing scripted pilot input harness for runway takeoff. That harness is fitted
test input, not a new flight model or a claim of an autonomous player pilot.

## Replay

See `docs/REPLAYS.md`, `crates/tore-replay/src/format.rs` and app `replay/`.
The dependency-free format begins TOREREPL, has versioned little-endian chunks,
checksums, frame/state data, spawns, events, decision trees, footer and index.
It records outcomes at 120 Hz, including AI and flight telemetry and comms.
Playback reconstructs snapshots rather than rerunning AI or depending on
input-replay determinism. `Clock` supports fractional ticks, reverse and seek.
`Viewer::frame` currently advances from wall time; the production director will
choose an explicit tick for each output frame, two ticks per 60 fps frame.
`Playback::picture`, weather, smoke, effects and device tracks are seekable.
`finish_tracks` completes the asynchronous scan before captures.

The camera is independent: twelve regular flight views plus free/anchored drone
views. The normal replay viewer omits cockpit/HUD artwork. For the reel, the
director feeds recorded attitude, speed, G and fuel into the existing HUD
and cockpit renderer. Recorded AI target selection supplies the target square.
The existing `gunsight::solve_observed` solver computes the pipper/range arc from
recorded target pose and the loaded gun's ballistics. Its visual-observation
path is described in `docs/spec/gunsight-targeting.md`; this is not a fabricated
radar lock. Weapon readiness and ammunition displays are omitted. The source cockpit
mirrors are rendered through the same scene pipeline. Repeatable output must be
checked on this host/GPU; cross-vendor floating point bit identity is not claimed.
Keep replays ignored alongside output: recordings include source-derived names,
world metadata and comms text, and do not need to be committed for a reproducible
recipe. No texture, shape, font or sampled audio is embedded by the recorder.
The director's `replay` view captures the actual viewer overlay at full
resolution, including transport, timeline, timer, object labels and subtitles.
Clock speed/pause drives its real control state, including the -4x rewind.

The second pass adds three presentation outputs to the director, all reading
the same recorded state. A HUD symbol layer redraws the cockpit pass with only
the symbols the HUD glass shows, clipped by the source aperture and hidden
behind the source art, over transparent black. A fixed-eye camera places the
eye in the anchor's heading frame at a chosen tick while the target follows the
anchor. Captures can run past a shot for sound only. Cockpit magnification uses
the engine's own camera zoom, which already scales the scene, the 640x480 HUD
raster and the cockpit art together about the forward datum; the director's FOV
now drives it for cockpit views.

## Renderer

`Renderer::scene_pixels` already renders into a GPU texture, copies to a mapped
readback buffer and returns RGBA. It accepts explicit dimensions. Its normal
constructor requires a window/surface, but `SimRenderer::new` needs only device,
queue, format and World. Therefore a surface-free wgpu device and the same scene
preparation/draw methods are the clean route. No desktop recording is needed.
Fixed 1920x1080 output and frame-count driven rendering can block for GPU and
encoder completion without losing or duplicating simulation steps. Keep 120 Hz
simulation unchanged and present the stored result at 60 fps.

## Audio

`Assets` imports sounds and MUS score scripts from the user's install. FA_1/FA_2
provide effects and speech; optional FA_4B/FA_4D provide music. `Audio::new`
decodes 5K/11K PCM through `tore-formats::pcm`. Aircraft profiles name engine,
afterburner and device clips. Weapons and explosion types select their source
clips; comms records retain speech references. The PCM music phrase scheduler
in `audio/music.rs` is device-independent and seeded, but normal startup seeds
it from wall time. `Mixer::frame` produces stereo samples independently of cpal.
Main had no complete offline exporter. The reel adds a fixed seed and
sample-count driven wrapper around the existing mixer and replay sound path. Replay music is
currently absent, so a separate retail score bed is an editorial addition.
Probe recordings omit crew voice and music, but include their actual wing/radio
journal events. Never label unrelated speech as a recorded outcome.

Offline captures now keep radio and crew speech on a separate stem, at exactly
the level the mixer would have added it, and log the sample at which each
recording starts. The offline mixer also drops finished one-shot voices after
each frame, as the device callback already does; before this, a long offline
capture stopped adding effects once eight had played. Scores are phrase
scripts: each phrase is a complete 20 to 60 second recording. `--reel-music`
renders every score from its start with the fixed seed, writes the phrase each
one chose and when, and exports every phrase any score can choose, so a bed can
be cut on its measured beat grid. Beat and key measurements are this
production's own analysis of the rendered PCM, not retail metadata.

## Cameras

Flight view IDs 0..11: front/cockpit, external, oblique, back, up, track, threat,
wing, player-to-target, target-to-player, fly-by and missile-to-target.
Reference modifiers choose player, target or last missile. Replay adds drone
movement and object selection. Director keyframes interpolate position, look
target and field of view (in zoom space) with monotone cubic or eased curves,
in recorded-aircraft relative coordinates. An optional aim places the look
target at a chosen screen position. This is presentation only.

## Production decisions and substitutions

These are agent editorial choices for the requested reel, not gameplay claims,
except where John's request is named with its date.

### Edit and timing

* The edit follows a 142.72 BPM beat grid, close to the requested 140 BPM
  feel. Every cut after the boot lands within one frame of a beat, counted from
  the launch downbeat at frame 90: launch 8 beats, formation 3 + 3, F-14D 5,
  Rafale C 5, Greece 5, North Vietnam 4, night 7, HUD to cockpit 12, replay
  2 + 1 + 1 + 4, then the end card at frame 1603 (26.72 s).
* John asked on 2026-09-30, after the first music pass, for a 7-second ending
  from the shoot-down and a longer end card. The film is now 32.0 s: everything
  before the end card is unchanged, and the end card holds for 5.3 s.
* The 15-second edit reuses the same shots: boot, the first four beats of the
  launch, the whole 302-frame HUD-to-cockpit shot, the whole replay sequence and
  a 205-frame end card. Its music is rebuilt: the same kick on its own replay
  cut, two bars, then the stinger, which rings out before 15 s.

### Shots

* Takeoff: the calm-wind F/A-18D departure recording is unchanged and holds the
  runway centerline exactly (checked on every run). The window moves later, to
  ticks 1180..1582, so the shot contains the rotation, the lift-off at tick 1301
  and the tower's real "Airborne" call. The visual clock is 07:00, with the sun
  on the horizon behind the shelters; the flight was recorded at 07:15 and its
  wind and motion are unchanged. The camera orbits from the front-left quarter
  to the rear-left quarter.
* Formation: the existing four-ship's Quick Mission delta slots are about 700 ft
  apart, so no single view shows all four larger than about 70 to 120 px. The
  formation is two beats-long views of the same recording, continuous in
  recorded time: a long lens from 3,300 ft to the left holds all four against
  the sky as the real Bug Out order arrives, then a close view of #4 as it rolls
  away. Both keep the formation's screen direction. No aircraft is moved.
* Aircraft showcase: John requested slower showcases on 2026-09-30. Two hero
  views of 2.1 s each keep the full fourteen-entry roster on screen. The F-14D
  and Rafale C are now recorded over Vladivostok's coast and Egypt's desert, new
  probes of the same unchanged kind, chosen for contrast; the Rafale C's retail
  scheme is green. Each orbits its aircraft with the aircraft held left of the
  roster panel by an aim key.
* Theater showcase: Greece and North Vietnam from about 18,000 to 22,000 ft
  above sea level, re-aimed so the coastlines clear the full sixteen-entry
  roster panel. The
  Greece island is never an airport view.
* Night: the same 4-versus-4 recording at a visual clock of 20:30, closer and
  lower, watching MiG-21 aircraft 6 and its wingmen release real flares and chaff
  (ticks 1595 and 1655 among them) as the F-22s' missiles approach. The shot
  ends before the first kill. Red two's "Fox two" and Red three's "Firing
  missile" are heard. Red four's "Fox two", queued behind them, would still be
  speaking at the cut and cannot finish before the kill, so the edit leaves it
  out rather than cut it or carry a missile call over the guns-only pursuit.
  The halation is raised for this shot.
* HUD to cockpit: John requested a full-screen HUD close-up on 2026-09-30 and,
  later that day, that the HUD camera look like an old TV recording with grain
  and chromatic aberration rather than green phosphor. The second pass is one
  continuous 302-frame capture of the F-22 wingman's cockpit: the engine zoom
  holds 2.5x for 76 frames, eases back to the normal cockpit by frame 151, and
  the videotape treatment fades over the same frames. Its parameters (blur,
  chroma delay, fringe, grain, band speed, dropout odds) are agent choices. The
  first gun burst (ticks 8326..8379) and its eight recorded hits (8449..8492)
  happen inside the close-up and the pullback; the second burst (8603..8633)
  follows in the full cockpit during the right bank. There is no gun kill.
* Replay: the separate missile-kill recording inside the actual replay viewer:
  a rear angle through the impact of the missile on aircraft 6 at tick 1885,
  pause on the impact (tick 1896, identical picture), the viewer's -4x rewind,
  then a front angle from tick 1887 as the fireball grows and "Red two:
  Beautiful!" is heard. The next call starts after the cut, so it is not heard.
* End card: John asked on 2026-09-30 for Windows, Linux and Apple marks because
  the game runs on all three. They are simple hand-drawn glyphs in the
  lettering colours, not the vendors' artwork. He then asked to drop the
  T.O.R.E. wordmark and have the badge flick in with cheesy sprite sparkles:
  the patch now spins in like a coin, flickers and flashes as it lands, takes a
  shine sweep, bursts sparkles on landing and on the music's final stinger, and
  twinkles on every beat between. The aircraft icon stays on the boot screen.
  The badge already reads T.O.R.E. FIGHTERS, so at John's request the separate
  FIGHTERS line is gone and the badge and lines are centred as one group. The
  other required lines are kept; they hold finished for about 4.5 s while the
  badge keeps sparkling.
* 4K: John asked for 4K exports on 2026-10-01. They are 2x Lanczos upscales of
  the lossless 1080p compositions with the tube curvature drawn at 4K, not
  native 4K engine captures.

### Picture treatment

* A restrained CRT finish on every frame: halation above a soft knee, a small
  black lift, three-line scanlines that thin on bright detail, a vignette and
  1.2 % grain, then curvature with slightly different per-gun curvature for a
  radial colour fringe. Brief horizontal sync wobbles mark some cuts; a short
  burst of static leads into the night. Lettering is a low-resolution bitmap
  of Liberation Mono with a soft phosphor halo.
* Portrait shots are reframed, not squeezed: a full-resolution 1080 square of
  each gameplay picture around its subject, 16:9 bands with the rosters for the
  showcases, and the replay's complete window. Lettering stays in the central
  square, and landscape lettering inside 4:3.

### Sound

* Music, picked by John on 2026-09-30 from loudness-matched exports of the
  game's score phrases: the bed is `XFI204CA`, a briefing track measured at
  138.34 BPM, moved 3.2 % faster onto the cut grid by a pitch-preserving
  rubberband stretch. Its pickup hit sounds under the end of the boot and its
  first downbeat lands on the launch. Its own breakdown (bars 12 and 13, with
  the bass dropping out) falls under the end of the cockpit and the replay's
  pause and rewind. On the cut to the fireball, where the full band would
  return, the music kicks into `AIR003` from the `M_SUCC.MUS` success score at
  its bar 1, plays three whole bars, then splices on the beat into the phrase's
  own final stinger, which rings out by 31.7 s. Both phrases are rendered by
  the game's own phrase player and matched in loudness to the first cut's
  approved bed. The splices, the stretch and the ending edit are editorial.
* Speech: each capture's real radio is mixed from its own stem. A recorded
  call is heard whole or not at all: it must start inside its shot and finish
  within 20 ms of the cut, or it is left out from its first sample. Shot
  windows were moved so that this drops only the one night call above. The
  final "Beautiful!" ends 11 ms after its cut. The bed ducks 12 dB under speech
  and the ending 6 dB, so the kick keeps its punch under "Beautiful!".
* The replay viewer delays sound with distance, as the game does: the missile
  blast reaches the rear camera about eight frames after its flash. The edit
  advances that shot's captured sound by six frames and lets the blast ring on
  over the paused frame from the capture's own following frames. The paused
  viewer itself is silent; this is an editorial audio choice.
* AI gun launch records contain weapon identity but no cockpit release sample
  event. For the watched AI cockpit only, the director plays that weapon's
  imported fire clip at the first recorded round; gaps over twelve ticks start
  a new burst cue. Both bursts' sounds start on their first recorded rounds.
* Boot Foley: the retail cockpit switch recording on power-on and button clicks
  as each boot line finishes. A short authored static burst leads into night.
* Mastering: a look-ahead limiter at -3.6 dBFS, then one measured linear gain
  to -14 LUFS. No dynamic loudness processing.

### Unchanged from the first pass

* The published renders used three renderer corrections for John's report
  that were left out when the tooling merged to main: MiG-21 source-normal face
  visibility, one reversed-depth ULP with zero slope bias on the textured
  airport detail pass, and an invariant terrain vertex position across matching
  GPU passes. They are planned with the multiplayer work. Contact geometry and
  simulation are unchanged.
* The pursuit uses the existing two-versus-one guns-only Quick Mission probe
  over Greece, with F-22s starting one nautical mile behind a novice MiG-21; the
  selected F-22 wingman has the MiG targeted from tick 2. The targeting box and
  gun pipper use recorded target observations and the existing ballistic solver.
  Weapon readiness and ammunition readouts are omitted.
* The Tomcat is the F-14D, `F18.PT` the F/A-18D and `RAFALE.PT` the Rafale C.
  F/A-XX is listed as a concept.
* v0.1 FIRST FLIGHT is the requested campaign title; footage is from main's
  0.1.3 implementation. "Multiplayer inbound" is the requested future tag, with
  no multiplayer footage or claim of a multiplayer implementation.
* Retail media is read from the user's install at render time. No retail
  bytes, recordings or renders are committed.

## Validation

Measured results, source build identity, comparison hashes, audio levels,
repository checks and the remaining platform and listening limits have one home
in the [production baseline](../../docs/baselines/reel-production.md).
