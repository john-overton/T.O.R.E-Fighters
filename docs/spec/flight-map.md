# In-flight map

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification, 2026-09-21. This is an opinionated addition requested
by John, not a claim of retail behavior. Shift+M opens the map. All object
categories have right-side map toggles, without settings in the Escape menu.
Buildings default off, as requested by John on 2026-09-21. Known
locations and aircraft-detected signatures appear together. Unidentified
contacts use placeholders. Original map art and fonts are reused where available.

## Display and controls

Agent decisions: a north-up map follows the player, initially showing 100 nautical
miles across its 476-pixel-wide plotting area on a 640 by 480 reference canvas.
Plus/minus zoom between 25 and 800 nautical miles across in powers of two.
Arrow keys pan by one quarter of the displayed width. Home resumes following.
Shift+M or Escape closes the map. Flight continues at the existing 120 Hz rate;
opening the map does not pause, steer, designate or fire. Ordinary flight
controls remain available. Map clicks never operate the instruments underneath.
Heading, speed in knots, altitude in feet, a distance bar and player marker
remain visible. One nautical mile is 6,076 feet, matching the sensor component.
Crowded symbols move in 34-pixel steps up to 102 pixels with a leader to their
actual position; overlapping labels are suppressed. Theater edges are dark,
not repeated terrain. Markers outside the view are hidden.

## Knowledge and detection

All imported runways are known map locations. Other entities require a current
observation from the selected operating radar/IR channel or the visual sensor.
The map uses observed positions, never hidden live positions. Visual observation
identifies the entity; radar/IR alone gives an unknown aircraft or surface marker.
This identification rule is an agent decision. A broad surface placeholder covers
unclassified ground objects, vessels, AAA and SAMs rather than inventing a type.
Visually identified surface objects use their imported names where available.
Unknown contacts do not expose aircraft identity, object name or allegiance.
Lost contacts disappear at the next simulation step. Destroyed objects retain a
marker only while detected, marked DESTROYED.

## Category controls

The right rail has five buttons: Aircraft, Airfields, Buildings, Surface and
Emitters. All start on except Buildings. The player is always visible. Clicking
a button changes display only; it never changes detection or weapon support.
A press and release on the same button is required. Closing/reopening retains
the selections; restarting a flight restores the defaults.

Agent grouping decision: Buildings covers an explicit allowlist of imported
structural object resources, including city blocks, houses, hangars, barracks,
bunkers, factories, storage, towers, bridges and fuel structures. It also hides
unidentified returns belonging to those resources. It does not identify them to
the player. Runways use Airfields, while other ground objects, unclassified
surface contacts, vessels and defenses use Surface. SAM sites and radar
installations are excluded from the building allowlist. No category is inferred
from a damage-class number. This is an authored display grouping, not a recovered
retail filter contract. Surface subcategories remain grouped until their metadata
supports a narrower classification.

The rail occupies x=500..628 on the reference canvas. Five 120 by 30 buttons
start at (504,76), spaced 44 pixels apart. The plotting area is (12,30,476,408).

Surface detection is fitted: reuse the installed sensor's existing search
geometry, signature/range rules, terrain masking, power and damage gates, solely
for map observations. Surface observations confer no air-to-air designation or
weapon support. The airborne scope and its selection rules stay unchanged.
No SAM/AAA spawning, targeting or autonomous behavior is introduced. A mission
must actually contain an entity before it can be detected or plotted.
Bearing-only passive signals get an UNKNOWN EMITTER bearing ray, never an exact
position. The ray is a direction guide, not a measured range.

## Visual provenance and unknowns

The supplied screenshot is a visual reference for the gray frame, terrain,
labels, distance scale and original square symbols. It does not establish
retail detection, projection or timing rules. The map uses the loaded theater's
original briefing image. Its full raster maps
onto the T2 grid extents, with image north at the top and world positive Z north.
This registration is fitted, not a recovered retail projection. T2 cell colors
are the fallback, with water index 255 displayed as RGB 65,119,137.
MCICONS.PIC supplies player, runway and identified object symbols when imported.
Original aircraft fonts supply labels. Missing artwork has geometric fallbacks
so existing caches remain usable. Icon choices and crop coordinates are agent
presentation decisions following visual inspection of the sheet.

[Implementation validation](../baselines/flight-map.md).

Exact retail map projection, category classification, identification ranges and
contact persistence are unknown. Next research: inspect bounded map data and
manual descriptions only when those player-visible details are needed.
