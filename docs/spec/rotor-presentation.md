# Propeller, rotor and nacelle presentation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification, 2026-10-05. These transforms are fitted agent
choices over reviewed user-owned source geometry. They establish working visible
animation and do not claim measured original rotor speed or spool behaviour.
The player can see original propellers rotate on C-130, AC-130 and E-2C,
main/tail rotors on AH-64 and Mi-24, tandem rotors on CH-47, and rotating
proprotors plus converting nacelles on V-22.

## Rotation and conversion

With engine running and fuel remaining, presentation phase is simulation time
in seconds times 2 pi times 15 revolutions per second. The 15 rev/s speed is a
fitted visual choice (agent decision, 2026-10-08), the same for propellers and
main rotors, like a governed rotor: throttle sets thrust, not blade speed. It
replaces an earlier `(5 + 15 * throttle)` rev/s rule, which multiplied the whole
elapsed time by the changing rate and so jumped many revolutions at every
throttle change. Tail rotors use three times the main phase. V-22 left/right and
CH-47 front/rear use opposite directions. Spin is deterministic from fixed
120 Hz simulation ticks and remains unchanged when rendering frequency changes,
and consecutive ticks differ by one fixed step. Engine off or no fuel returns
blades to their source pose immediately. Gradual spool and stationary phase
retention are unknown and not simulated by this presentation rule; a rotor speed
that follows power would need the phase kept as simulation state.

The V-22 nacelle angle equals actual simulated conversion times 90 degrees:
0 is forward propeller thrust, 1 is vertical rotor lift. It follows the actual
conversion position rather than a local key or an animation-only toggle.
Source X is right, Y forward, Z up. The fitted hinge is at X = +/-102, Y = 0,
Z = 12 source units; each whole nacelle and its propeller turns about X after
propeller spin. No wing or fuselage faces are included.

The CH-47 aft rotor uses its forward/reverse facing panels near Y=-45,Z=29.
The original front panels are already located near Y=67,Z=15 and rotate about
that mast with opposite phase. No rotor is translated to fabricate a second
assembly.

The decoded rotor panels carry multiple coincident atlas image phases: three
on V22, AH64 and E2C, two for the Mi24 main rotor (three on its tail), four on
C130 and CH47, and two on each AC130 blade polygon. Drawing every phase overlays the original blur patterns into a bright
noisy disc. The fitted host presentation selects phase zero per facing panel
and rotates that original texture continuously. It preserves the source
cutout, colors and blade artwork, with no atlas recoloring or global shader
change. Original phase-selection sequencing remains unknown.

## Helicopter cyclic presentation

Agent-authored fit, 2026-10-05, for the individual animation audit. The existing
main-rotor source panels follow cyclic pitch and roll while retaining their
reviewed mast centers. Apply spin first, then pitch about source X by
`0.10*elevator` radians, then lateral tilt about source Y by `0.10*aileron`.
Positive pitch tilts the disk's lift direction aft; positive roll tilts it right.
Both inputs use the same smoothed live control state as the aircraft surfaces,
not separate animation keys. Preserve blade radii, panel dimensions, material
and texture coordinates. The fuselage, mast and fixed landing gear do not tilt
with the rotor panel.

CH-47 uses smaller fitted limits because its canted panels overlap in plan:
`0.03*elevator` pitch and `0.08*aileron` lateral tilt. Add `0.02*rudder`
radians to the front disk and subtract it from the aft disk. This
expresses opposite rotor thrust directions without inventing a tail rotor.
Mi-24 and CH-47 spin about the normals of their original tilted panel planes:
Mi-24 [0,1,44], CH-47 aft [0,9,136], front [0,9,137]. Spinning about source
vertical instead made the canted panels wobble. All axes are normalized before
rotation. The smaller CH-47 control fit preserves separation through combined
inputs, including opposing yaw tilt. These angles are presentation fits, not new aerodynamic forces or recovered
retail actuator schedules. Combined controls must remain rigid and keep each
mast center fixed at every rotor phase.

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
faces unchanged, verify the V-22's 90-degree end position and transformed normal,
and rotate the CH-47 front rotor about its existing separate mast.
Phase-selection checks retain one original image per panel and leave unrelated
geometry untouched. Independent actual-source probes now check fixed mast
centers, rigid panels and spin-plane preservation. Combined control/phase
sweeps cover 8,400 helicopter poses, including direct tandem-panel separation
checks. The [animation audit](../baselines/aircraft-animations.md) records those
results and the remaining aircraft. GPU appearance and retail timing remain
unvalidated; these geometry tests do not establish retail animation parity.
