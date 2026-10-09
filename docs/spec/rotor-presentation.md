# Propeller, rotor and nacelle presentation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification, 2026-10-05, revised for the VTOL overhaul's
moving parts (slice P7b, 2026-10-08). These transforms are fitted agent
choices over reviewed user-owned source geometry. The rotorcraft's rotor
speed comes from the flight model (its rotor speed state); the original
game's own rotor speed and spool presentation are not claimed.
The player can see original propellers rotate on C-130, AC-130 and E-2C,
main/tail rotors on AH-64 and Mi-24, tandem rotors on CH-47, and rotating
proprotors plus converting nacelles on V-22.

## Propellers

With engine running and fuel remaining, a fixed-wing aircraft's propeller
phase is simulation time in seconds times 2 pi times 15 revolutions per
second. The 15 rev/s speed is a fitted visual choice (agent decision,
2026-10-08), like a governed propeller: throttle sets thrust, not blade speed.
It replaces an earlier `(5 + 15 * throttle)` rev/s rule, which multiplied the
whole elapsed time by the changing rate and so jumped many revolutions at
every throttle change. Spin is deterministic from fixed 120 Hz simulation
ticks and remains unchanged when rendering frequency changes, and
consecutive ticks differ by one fixed step. Engine off or no fuel returns the
blades to their source pose immediately.

## Rotor speed and blade angle

A rotorcraft's rotors turn at the simulated rotor speed: the drawn speed is
the flight's rotor speed (a share of 100 percent) times the type's nominal
speed from its rotor table: AH-64 289 rpm, Mi-24 240, CH-47 225, V-22 397. So
spool-up, droop, the V-22's 84 percent (333 rpm) in airplane mode and
autorotation all show, with the engine running or not, and a stopped rotor
rests where it stopped. The AH-64 and Mi-24 tail rotors turn at the speed
their tail rotor's tip speed and radius give (about 1,450 and 1,040 rpm at 100
percent).

The blade angle never jumps. The flight keeps the rotors' **turns**, the
rotor speed integrated over time in seconds at 100 percent, advanced once a
tick on every flight adapter (`Drive::rotor_turns`, exact-coded with the
flight state so the own aircraft's prediction and checkpoints keep it, and
read by nothing in the physics). The drawn angle is turns times the rotor's
nominal revolutions per second, folded into one revolution. Between ticks the
drawing blends the turns as it blends the attitude. The other drawn aircraft
carry their turns in the render snapshot: the host's AI aircraft from their
flights; a client's remote aircraft from the rotor speed in their entity
records, integrated exactly through the received states (straight lines
between ticks), the same whatever the frame rate; a replay from its recorded
rotor speed, summed frame by frame from the recording's start, so a tick shows
the same blade angle however the playhead reached it. A remote or replayed
blade angle is not the host's or the recorder's own; no player can compare
them, and every one turns at the right speed (agent decision, 2026-10-08).

Directions follow the flight: the AH-64's main rotor turns counter-clockwise
seen from above and the Mi-24's clockwise, as their rotor tables say; the
CH-47's front rotor counter-clockwise and its rear clockwise; the V-22's left
proprotor clockwise and its right counter-clockwise in the hover. (Before the
overhaul the Mi-24 and both CH-47 rotors were drawn the other way.)

## Nacelles

The V-22 nacelle angle is the flight's actual nacelle angle, 0 to 97.5
degrees (`State::nacelle_degrees`: actual conversion times the nacelle
travel): 0 is forward propeller thrust, 90 vertical rotor lift, 97.5 the aft
stop. It follows the actual conversion position rather than a local key or an
animation-only toggle. Source X is right, Y forward, Z up. The fitted hinge is
at X = +/-102, Y = 0, Z = 12 source units; each whole nacelle and its
propeller turns about X after propeller spin and disk tilt. No wing or
fuselage faces are included.

The CH-47 aft rotor uses its forward/reverse facing panels near Y=-45,Z=29.
The original front panels are already located near Y=67,Z=15 and rotate about
that mast. No rotor is translated to fabricate a second assembly.

The decoded rotor panels carry multiple coincident atlas image phases: three
on V22, AH64 and E2C, two for the Mi24 main rotor (three on its tail), four on
C130 and CH47, and two on each AC130 blade polygon. Drawing every phase overlays the original blur patterns into a bright
noisy disc. The fitted host presentation selects phase zero per facing panel
and rotates that original texture continuously. It preserves the source
cutout, colors and blade artwork, with no atlas recoloring or global shader
change. Original phase-selection sequencing remains unknown.

## Disk tilt

Each main rotor disk tilts from its shaft by the flight's simulated disk
tilt (`lift_controls.rotors[i].tilt`: cyclic, blowback, flapping and the
retreating blade stall's pitch-up, slice P2): rotor 0 is the main or front
rotor, rotor 1 the CH-47's rear rotor or the V-22's right proprotor. Apply
spin first, then the longitudinal tilt about source X (a forward tilt leans
the disk's lift forward), then the lateral tilt about source Y (a right tilt
leans it right), about the mast centre. The fuselage, mast and fixed landing
gear do not tilt. Preserve blade radii, panel dimensions, material and texture
coordinates. The drawn tilt is the simulated angle within fitted bounds that
keep the blades clear of the airframe:

| Rotor | Bound |
| --- | --- |
| AH-64, Mi-24 main rotor | 0.25 rad each way on each axis (the flight allows 0.45) |
| CH-47 | the part both disks share 0.03 rad longitudinal and 0.08 lateral; the part where they tilt opposite ways 0.01 and 0.02 |
| V-22 proprotors | 0.2 rad each way |

The CH-47's canted panels overlap in plan, so opposite lateral tilts (the
pedals) bring the overlapping edges together fastest; its bounds keep a
minimum clearance of 0.49 ft across every combination the animation probe
sweeps. Mi-24 and CH-47 spin about the normals of their original tilted panel
planes: Mi-24 [0,1,44], CH-47 aft [0,9,136], front [0,9,137]. Spinning about
source vertical instead made the canted panels wobble. All axes are
normalized before rotation. A V-22 proprotor tilts in its nacelle's frame
before the nacelle turns: with the nacelle at 0 degrees its shaft is source
Y, a forward tilt leans the disk down (about X) and a right tilt right (about
Z), as the flight's tiltrotor disk frame has it. These angles are
presentation bounds, not new aerodynamic forces. Combined tilts must remain
rigid and keep each mast centre fixed at every rotor phase.

The stick no longer tilts a disk directly: before the overhaul the disks
followed the smoothed stick at a fitted 0.10 rad (CH-47 0.03 pitch, 0.08 roll,
0.02 pedal). A replay records no disk tilt, so replayed disks stay level; the
multiplayer entity record carries it.

The original rotor art is a flat blade/blur image rather than separately
feathering blade meshes. Collective and AH-64/Mi-24 tail-rotor pedal changes
therefore have no independently reviewed blade-pitch geometry in this fit.
Their existing flight inputs and force response remain available. Do not report
those missing visual mechanisms as recovered behavior, or distort the whole
disk to imply individual blade feathering. Gear and other airframe devices
retain separate per-aircraft acceptance requirements.

## Geometry provenance

The shape build identities, face groups and pivots are recorded once in the
[geometry evidence](../formats/variety-rotors.md). Original art is loaded at
runtime; tests contain synthetic faces only. Geometry selection is source-derived;
hinge placement, spin rates, direction and phase selection are fitted. Full
original interpolation and rotor consumers remain unknown. The next research
step is review of the bounded shape interpolation operands for the two CH-47
image-phase consumers and V-22 engine assemblies, without executing original shape code.

## Validation

Synthetic checks preserve propeller hub positions and radius, leave unselected
faces unchanged, verify the V-22's nacelle at 0, 45, 87, 90 and 97.5 degrees
and its transformed normal, the rotor tables' speeds, a blade angle that turns
by exactly each tick's rotor speed through spool-up, droop and overspeed with
the engine off, the directions against the rotor tables, CH-47 counter-rotation
about its existing separate masts, disk tilt against the flight's tilt, the
tandem bounds, and the proprotor tilt in the nacelle frame. The animation
probe's `moving-parts.ppm` sheet draws the V-22 at 0, 45 and 90 degrees, the
AV-8 and Yak-141 nozzles at 0, 90 and 100, and each helicopter's disks level,
forward and right.
Phase-selection checks retain one original image per panel and leave unrelated
geometry untouched. Independent actual-source probes now check fixed mast
centers, rigid panels and spin-plane preservation. Combined tilt/phase
sweeps cover 8,400 helicopter poses, including direct tandem-panel separation
checks, driven through the disk tilt the drawing reads. The [animation audit](../baselines/aircraft-animations.md) records those
results and the remaining aircraft. GPU appearance and retail timing remain
unvalidated; these geometry tests do not establish retail animation parity.
