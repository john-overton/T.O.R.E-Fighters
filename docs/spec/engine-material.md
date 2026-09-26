# Throttle-driven engine material

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Behavior

Opinionated presentation requested by John on 2026-09-16. Replace the existing
rear round engine faces of F/A-18D, Rafale C, X-31, F-14D, MiG-21,
MiG-23, MiG-29, Su-27 and Su-35 with his supplied artwork. The expansion
was requested by John on 2026-09-16. Reuse the existing pink-mask grading below.
A-4E and Su-25 have no afterburner; F-22 has rectangular outlets. All three
retain their original engine presentation. Flame geometry, flight forces and sound
are unchanged. This is not a claim about original FA artwork or behavior.

Keep metal pixels unchanged. The pink regions serve as the glow mask: normalized
magenta excess (min(red,blue)-green)/max(red,blue,0.001), smoothed over 0.15..0.5.
At zero throttle the mask is gray (35% of source red in each channel). From
0..65% throttle blend toward red (source red times 1,0.025,0.008); above 65%
blend toward a red-white endpoint. Its white core blends to RGB 1,0.92,0.86
using source-red smoothstep 0.45..0.95, retaining dark recesses and red edges.
Engine off or empty fuel selects gray. Active afterburner selects full glow.
Power follows live presented throttle; no additional lag is introduced.

Map the full image across each existing nozzle's right/up bounds, independently
for twin engines. Bilinear sampling softens the reduced artwork. The engine
material is self-lit and still receives atmospheric haze and cloud occlusion.
It does not emit light onto the aircraft or surrounding terrain.

## Afterburner glow

Opinionated presentation requested by John on 2026-09-26. It applies to the
player and to AI aircraft whenever the afterburner is actually lit: engine
running, fuel left, afterburner selected and throttle past the aircraft's
afterburner setting.

- **The flame is the light source.** Each engine's light sits in its flame,
  3 feet behind its outlet (the contrail attachment point). The 3 feet is an
  agent decision.
- An afterburning aircraft lights nearby surfaces at **an eighth of a flare's
  strength**, split evenly across its engines. John first asked for a quarter,
  then halved it the same day. Together its engines light a surface facing
  them as brightly as full sun at about 22 feet. Color, 1,500-foot reach and
  night boost follow the [flare light](countermeasures.md#flare-light) rules,
  and the lights share the 16 scene lights with flares.
- **The flame looks the same at night as by day** (John). Flame surfaces keep
  their daylight colors instead of the weather's night darkening; only
  distance haze still applies.
- There is no glare or halo behind an afterburner. John had the first version's
  small star glare removed.
- The light is steady and follows the aircraft as drawn on screen each frame.

## Assets

`assets/aircraft/engine-texture-full.png` preserves the supplied 1254 × 1254 image.
`engine-texture.png` is the 314 × 314 copy used for runtime preparation, a 75%
reduction in each dimension requested by John. Box resampling is an agent choice.
`engine-texture.rgba` contains the same reduced pixels in a bounded runtime
container: ASCII TORErgba, little-endian u32 width and height, then top-down RGBA8.
Maximum dimensions are 2048 per side, and the byte count must match exactly.
No artwork is embedded in executables and no PNG decoding dependency is added.

Run `python3 tools/prepare_engine_texture.py` after changing the full-size PNG.
Only this preparation step needs ImageMagick. Ship the assets directory beside
the executable, or set `TORE_ASSET_DIR` to its location. Development also searches
the current working directory and source checkout. Missing artwork logs a
message and keeps the previous nozzle appearance; malformed artwork is rejected.

For inspection, `--flight-throttle 0..1` sets initial throttle and freezes a
capture pose when used with `--capture-flight`. It does not override live input
after normal flight starts.
