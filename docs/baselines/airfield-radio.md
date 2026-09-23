# Airfield radio, taxiway queue and ground visibility

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode after static research, 2026-09-23. John requested the
radio pass and chose a taxiway queue for Quick Mission wingmen. His reported
visibility case was Ukraine, Simferopol, three experienced F/A-18Ds, no other
wings, clear weather and the standard weapons load. Specifications:
[radio](../spec/airfield-radio.md), [queue](../spec/quick-mission-menu.md#player-ground-start),
[airport rendering](../spec/airports.md#integration-choices).

## Evidence and behavior

The installed FA.EXE matches the reviewed 1.02F SHA-256 in
[radio source notes](../formats/radio.md#airport-speech-review). The fresh static
trace and phrase-table reads confirm conventional-field takeoff clearance,
airborne/farewell, landing clearance/wind, grade and welcome. No original
executable code was run. Carrier-only branches remain separate. Reusing
human-facing tower recordings for AI status is explicitly fitted.

Quick Mission uses the last taxi-out legs for its queue, 200 ft between aircraft
and at least 250 ft from the player's takeoff spot. Both turn order and runway
occupancy gates apply. Parking starts still work and unusable taxiway layouts
retain the documented runway fallback. In the Simferopol three-Hornet probe,
wingmen lifted off at 46.7 and 95.7 simulated seconds. The first used to leave
parking and taxi for a liftoff around 138 seconds. These are host measurements,
not retail timing comparisons.

The headless radio trace includes startup clear-for-takeoff, each wingman's
hold, clearance, takeoff and airborne calls, the player's airborne/farewell,
returning to base, marshal, landing clearance, final, landing grade and taxiing
clear. A separate parked run emits startup clearance at 0 s and the two hold
reports at 3 and 6 s, without a spurious landing or repeated clearance.
Synthetic tests cover wind, grade, welcome, restart, cancellation, replacement
of stale status, expiry, channel spacing and radio silence.

## Ground visibility

The physics trace kept both taxiing aircraft 8 ft above terrain and the runway
at Simferopol. The GPU regression showed the pavement slope bias hiding half
of a synthetic aircraft at 1,500 ft range, despite the aircraft being above
its contact plane. An overhead extension also showed the constant texture bias hiding all four
reference aircraft pixels from 5,000 ft above. Both biases are removed.
Ordered equal-depth artwork preserves the airport layers. The regression
covers solid and textured pavement, grazing views and overhead views, and
ensures that geometry actually below the ground is still hidden.
The regression renders with the production airport and aircraft pipelines.

The low-cockpit screenshot then exposed terrain overlapping the runway.
Before/after captures at the same Simferopol spawn reproduce the green foreground
and show pavement after the correction. Terrain triangles are split at the
oriented airport boundary and recessed at most to one foot below its fixed
support plane, with boundary walls. Synthetic tests check rotated/sloped
footprints, texture-coordinate continuity, total area, unchanged outside
terrain and no raising of ground already below the plane. Source height queries
and aircraft contact are unchanged. The new captures and mesh inspection stay
local (`cockpit-before.png`, `cockpit-after.png`, `airport-mesh.bin`).

Distant moving views exposed a separate depth-precision failure. The new
regression uses a one-foot ground/pavement separation at real-world coordinate
magnitudes. Before the fix, a 0.625-foot camera shift at 10,000 ft range hid
all 1,810 runway pixels. The shared world projection now uses reversed
floating-point depth, keeping the existing near/far distances and physics.
[Technical mapping](../ARCHITECTURE.md#flight-presentation-and-measurement).
Tests compare every frame against a clearly separated reference at 10,000,
20,000 and 40,000 ft, with eight camera positions per distance and with both
1x and 4x samples. Matched real Simferopol captures use the same unmodified
assets; logs and images for this check are in `.local/airport-flicker/`.

The AI rendering path also forced gear and flaps retracted. It now takes
actual actor gear, flap, brake, hook, bay, exhaust and control-surface positions,
interpolated at render time. It never changes physics height. Synthetic device
interpolation tests and a real three-Hornet creator capture check that bridge.

## Checks and limits

All required Linux checks pass: formatting, locked workspace clippy/tests/build,
75 Python tests, source and both debug-binary asset guards, and documentation
headers. The workspace suite has 453 passing app tests and 786 passing simulation
tests. Five GPU-only tests are ignored in the ordinary suite. All five were run
explicitly: moving airport visibility, aircraft/pavement occlusion, geometry
shadows, smoke and glare.

Display smoke tests pass for the menu, a three-Hornet/no-enemy Simferopol creator
setup, an airborne mission and the compatibility fixture ground start. These
scripted creator runs use the default Average skill; John's screenshot selects
Experienced. The visibility regression is independent of AI skill. The
creator checks also pass restart comparison. The Simferopol capture has two
grounded wingmen and no airborne targets. A matching headless departure shows
both wingmen airborne by 95.7 seconds, with no reported ground hazards.

Scratch evidence stays in `.local/airfield-radio/`, including `tower.txt`,
`phrases.txt`, `queue-probe.log`, `flight-with-radio.log`, `startup-radio.log`,
`depth-before.log` and `depth-after.log`. Imported media is read from an isolated
profile under that directory. No retail bytes or generated captures belong in Git.

Run the targeted GPU regression with:

```sh
cargo test --locked -p tore-app gpu_airport_pavement -- --ignored --nocapture
```

For a matching airport and aircraft-count creator capture use `--theater UKR --ground-start 2 --aircraft f18
--probe-wing-size 3 --probe-wing-only --launch-quick-mission --flight-view 7
--capture-flight .local/airfield-radio/queue.ppm --smoke-test`, with `TORE_DATA_DIR`
pointing to an imported test profile. The ordinary airborne and fixture paths
are retained. The probe's scripted human can hit terrain on a long cruise;
it is not a validated player autopilot.

Unvalidated: running-retail comparison, manual listening to every cue, broad
crosswind/multi-wing contention, all airport layouts and Windows/macOS runtime
behavior. Taxi and marshal have text status, not guessed voice recordings.
Carrier catapult, hook and landing-officer cues require carrier functionality.
