# Aircraft damage appearance and combat smoke

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, requested by John on 2026-09-17. Reuse the user's FA damaged
aircraft shapes/textures and smoke artwork. Local source inspection establishes
resource availability and visible geometry, not original damage thresholds or
smoke scheduling. [Resource evidence](../formats/objects-and-shapes.md#combat-damage-and-smoke-resource-review).

Agent-selected fitted rules: show a damaged body once remaining hit points are
at most 50% of starting hit points. Choose one of the reviewed A or C bodies by
supported-aircraft roster index parity, and retain it through destruction. These
are alternate appearances, not alphabetical severity levels. Render destroyed
target bodies while airborne, using their existing ballistic motion. Do not add
control failures, AI or change damage amounts. The corresponding B/D piece detaches once at this transition. Use each shape's own texture; never apply intact animation address ranges
to a damaged shape. The original model scale is retained; variant scale parity
remains unverified.

Emit white missile smoke only during the movement model's powered interval,
including supported compatibility weapons. Guns emit none. Emit dark aircraft
smoke at or below the same 50% health threshold, while the target remains airborne.
Ownship emits while damaged and alive; residual puffs persist after destruction.
No smoke is emitted by an undamaged aircraft or a motor before ignition or after
burnout. Existing smoke continues to disperse after its source stops or disappears.

Smoke samples use fixed 120 Hz simulation time. Missile puffs emit each tick and
last 4 seconds; aircraft puffs emit every 2 ticks and last 8 seconds. Puff radii
start at 4/8 feet and grow by 6/8 feet per second for missile/aircraft smoke.
Puffs rise 2 feet per second, fade linearly, and have no gameplay sensor effect.
The oldest puff is discarded above a total 8,192-puff budget. Reset clears smoke.
Original 43-pixel smoke cells at x=0 (dark) and x=94 (pale), with
palette index 255 keyed transparent, are camera-facing, blended, depth-tested and ordered
back to front. Size, lifetime, placement and opacity are fitted, not retail parity.

Load Ordnance does not show the straight-flight dummy description. Validation
errors and useful loading feedback remain.


## Detached pieces and ground cleanup

Requested by John on 2026-09-17. A/B and C/D are fitted body/piece pairings.
A piece inherits the aircraft's complete velocity and orientation. Its starting
location is fitted from the largest bounding-box extent lost between the intact
and damaged body, aligning the fragment center to that missing region. This is
not an original attachment transform. Gravity is 32.174 feet/second squared;
a fitted world-axis tumble uses 0.8/0.5/0.2 radians/second. Rendering uses the
fragment's own original geometry and texture, without intact-aircraft animation.

The first swept terrain contact removes the piece immediately and creates one
15-foot, 0.375-second ground-hit animation from `GRDLRGA.PIC`. The animation uses
12 frames in a 3x4 grid of 80x63 source cells, keyed with palette index 255. Its
center is 6 feet above the contact so terrain does not hide it. This small use of
the original ground-explosion art is an agent-selected fit, not a recovered
bullet-impact mapping. There is no resting wreck part or collision obstacle.
Further hits cannot respawn the same piece. Reset clears detached pieces and
impact effects. A 256-piece visual budget bounds the simulation; pieces have no
AI, damage, radar return or independent weapon behavior. Original breakup choices,
trajectories and animation timing remain unverified.
