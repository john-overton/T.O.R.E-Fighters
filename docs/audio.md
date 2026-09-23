# Audio behavior and acoustic model

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The [sound specification](spec/sound.md) establishes the original recording
identities. This guide describes the requested realism extensions and the
agent-fitted playback constants. All distances below are feet, all time is
simulation time, and the simulation remains fixed at 120 Hz. No audio output
or listener calculation changes flight, seekers, damage or AI decisions.

## Seeker growl

The air-to-air IR growl loops `&IR1.11K` continuously across lock changes.
Surface IR uses the reviewed radar-named search/lock pair without changing its
sensor requirements. Radar seekers keep that pair as well. IR volume uses the
same fitted percentage displayed by the HUD: with `p = percent / 100`, the
locked amplitude is `0.15 + 0.85*p`, and tracking is half that amplitude.
The audible floor preserves a search cue at zero percent. This floor and linear
curve are agent-fitted; the original establishes percentage scaling and the
half-volume tracking relationship. The default maximum remains 0.30, adjustable
with `TORE_SEEKER_VOLUME=0..1`. Amplitude reaches each target in 0.1 seconds.
Radar gain keeps the existing heat/quality mapping. Safe/empty/failed weapons,
loss of player life, pause and effects mute retain their existing gates.

John extended this to bore mode on 2026-09-23: an eligible IR return actively
tracked by the mounted seeker under the boresight produces growl without
cockpit designation, using that return's HUD
percentage. Tracking uses the same half-volume curve while the existing
0.25-second dwell completes. Lock gain requires the mounted seeker to have
locked that same return. A new candidate uses its own percentage and tracking
gain until acquired. An empty bore, a provisional return too weak to track,
or loss of the current track means silence. Terrain masking,
leaving the cone, safe/NAV, empty or failed stations, death and release remove
the old tone through the existing 0.1-second envelope. Aircraft radar can remain
off. A radar missile in boresight sounds its lock tone on its bore return without
designation (John, 2026-09-23). Every seeker is silent unless it is actively
tracking, and radar tones stop inside the weapon's minimum range.
This bore-audio availability is opinionated, requested by John; it does not
change selection, guidance or launch permission.

## Traveling sound

John requested realistic distance, fading and travel on 2026-09-23. The agent
chose spherical propagation at the existing atmospheric sound speed: 1,115
ft/s at sea level, declining to 967 ft/s at 36,000 ft. Each event retains its
emission position and expands a wavefront at the speed for the mean source and
listener altitude when emitted. The current listener must meet the wavefront
before playback begins. An explosion 11,150 ft away at sea level arrives after
10 seconds, including after its visual effect has expired. Moving toward it
shortens the delay; moving away extends it. Pause freezes propagation and PCM;
restart, leaving flight and effects-off clear pending and active spatial sound.

Amplitude follows inverse distance beyond a near-field reference, then a smooth
fade over the final 20 percent of the maximum range. It is zero at the maximum.
This models free-field pressure falloff with fitted audibility limits, without
claiming absolute sound-pressure calibration. During playback, distance and
stereo direction follow the listener relative to the retained emission position.
Gain changes use a 20 ms smoothing time constant to avoid steps when the listener
moves or turns. The treble cutoff is
`12000 / (1 + distance/3000)` Hz, clamped to 250..12000 Hz. One-shots have 5 ms
attack and 30 ms release ramps. The waveform's own envelope supplies the decay.

| Cue | Recording | Reference distance | Maximum distance | Peak gain |
| --- | --- | ---: | ---: | ---: |
| Impact | Existing `&EXPL3.5K` | 80 | 8,000 | 0.4 |
| Explosion | Existing `&EXPL12.5K` | 400 | 40,000 | 0.65 |
| Aircraft pass | `&AIRPASS.11K` | 200 | 2,000 | 0.5 |
| Missile pass | `&MPASS.5K` | 80 | 1,500 | 0.5 |
| Sonic boom | `&SNCBOOM.11K` | 500 | 13,000 | 0.8 |
| Player weapon release | Weapon's imported fire sound | 80 | 8,000 | 0.4 |

Impact/explosion positions come from the actual combat effect producer, never
a target's later location. Simultaneous equal recordings at different positions
remain separate. Cockpit weapon releases stay immediate; external releases use
travel. Cockpit avionics, radio, controls and engine loops stay local.
At most 256 waves wait in flight and 16 spatial voices play; overload discards
the oldest pending wave and replaces only a quieter active voice.

## Passing objects and booms

The listener follows the main view, including external camera position and look
direction. Secondary mirrors and panels never create additional listeners.
Aircraft and missiles are observed through presentation snapshots only. A swept
closest approach produces one pass recording inside its range, with at least
100 ft/s relative speed for aircraft or 200 ft/s for missiles. Aircraft already
supersonic use the boom instead of an ordinary pass. A source can pass again
after leaving 1.5 times its pass radius. Spawn, formation neighbors moving with
the listener, camera cuts and view switching do not synthesize pass events.
Passing recordings already contain a changing timbre; no extra Doppler shift
is claimed in this first model.

For other aircraft, a boom occurs when the listener crosses the trailing Mach
cone: along-track separation equals lateral distance times `sqrt(Mach^2-1)`.
This already represents shock arrival, so it receives no second travel delay.
Only outside-to-inside crossing fires, with rearming outside the cone. A
constant-velocity local cone is a fitted approximation during turns and speed
changes. The user's own aircraft also produces one boom on upward Mach 1
crossing in either external view, rearmed below Mach 0.98. This external-camera
cue is an opinionated presentation choice, not a claim that the pilot hears
his own sonic boom. Switching views while already supersonic does not trigger
it. Other aircraft can produce booms in cockpit or external view.

Physical basis: [NASA's speed-of-sound relation](https://www.grc.nasa.gov/www/k-12/BGP/sound.html)
and [continuous supersonic shock waves](https://www.nasa.gov/wp-content/uploads/2018/07/supersonic-student.pdf).
The model omits atmospheric refraction, wind transport, ground reflections,
terrain/building occlusion and calibrated pressure levels. These are known
approximations, not claims of full acoustic or retail parity.

[Validation and local listening previews](baselines/flight-sound.md).

## Pilot escape audio

Ejection schedules the imported pilot announcement, seat launch and chute-opening
recordings once at their respective transitions. Friendly AI pilots announce
their own escape. A cockpit danger warning is separate from manual confirmation.
After separation, the cockpit engine loop stops and the wreck remains a spatial
sound source. [Source clips and unresolved speaker routing](formats/ejection.md)
and [event rules](spec/ejection.md#audio-and-art) distinguish measured identities
from fitted assignments. Missing optional clips keep text and simulation working.

## Crew voice

The player's crew and first wingman speak through the shared radio channel:
each line prints `Speaker: 'text'` and queues its recordings after anything
already playing. The player's death scream plays directly, without text. What
is said and when is in [cockpit voice](spec/cockpit-voice.md#implementation-in-tore);
missing recordings are skipped and the text still shows.


## Airport and wing departure reports

The [airfield radio producer](spec/airfield-radio.md) observes player and
wingman airfield states. Runway starts receive a recorded takeoff clearance;
airborne/farewell, landing clearance/wind, landing grade and welcome use original
recordings. Taxi, hold, final and taxi-clear status fill gaps with text. Wing
reports identify the aircraft and share the radio channel at readable intervals.
Routine wing calls obey radio silence. Airport recordings have their own queue
ownership so a cancelled airport message cannot remove wing or crew speech.
[Static evidence](formats/radio.md#airport-speech-review),
[validation and carrier limits](baselines/airfield-radio.md).
