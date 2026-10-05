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
in seconds times 2 pi times `(5 + 15 * throttle)` revolutions per second. The
5 and 20 rev/s endpoints are fitted visual choices, shared by propellers and
main rotors. Tail rotors use three times the main phase. V-22 left/right and
CH-47 front/rear use opposite directions. Spin is deterministic from fixed
120 Hz simulation ticks and remains unchanged when rendering frequency changes.
The current throttle sets phase directly, so power changes can shift blade phase.
Engine off or no fuel returns blades to their source pose immediately. Gradual
spool and stationary phase retention are unknown and not simulated by this
presentation rule.

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
geometry untouched. Runtime source rendering
still requires the combined aircraft-variety validation pass; synthetic tests do
not establish retail animation parity.
