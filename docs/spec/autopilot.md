# Straight-flight and waypoint autopilot

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

## Player behavior

John requested the USNF-ATF modes on 2026-09-17, including waypoint plumbing
before routes exist. A toggles heading/altitude hold. Ctrl-A toggles waypoint
hold. Pressing the active mode again turns it off; switching modes preserves
the original heading and altitude capture. Re-engaging from off captures anew.
Waypoint mode continuously follows the selected horizontal target, retaining
captured altitude. With no target, or within 1 metre of it, it holds the captured
heading. No route selection, automatic sequencing or waypoint altitude is added.

Pitch, roll or rudder input strictly above 0.15 in magnitude disengages on the
same simulation tick. Throttle and equipment controls remain manual. Autopilot
disengages on ground contact or crash; new flights start off. It uses ordinary
control inputs at 120 Hz, without changing aircraft attitude or position directly.
The HUD shows two lines at the upper left beside the heading tape, using the
existing HUD font and color: `AUTO` above `HDG ALT` or `WP <number>`.
The number is the selected mission waypoint's number. With no target, the second
line reads `WP --`. No label appears when off. John requested this layout and
numbered waypoint label on 2026-09-17; the missing-target placeholder is an agent
choice.

## Numbers and provenance

The interaction and 30 degree commanded-bank / 20 m/s commanded-climb limits
are spec-derived from the reference checkout's prose. These are command limits,
not hard clamps on aircraft motion. Retail equivalence remains unknown.
The 1 metre arrival fallback is fitted. HUD line positions (211,133) and
(211,145) at 640×480 are fitted to the requested screenshot layout.

Controller tuning is fitted for this host: heading error requests bank at a
factor of 1.5; bank error requests roll rate with a 1 second time constant,
normalized by aircraft roll authority. Altitude error requests vertical speed
with a 10 second time constant, limited to 20 m/s. Vertical-speed error requests
vertical acceleration with a 3 second time constant. Bank compensation converts
that acceleration to normal load, using a minimum bank cosine of 0.5, then the
aircraft's available load envelope converts it to stick deflection. No autothrottle,
terrain avoidance, stall recovery or guaranteed hold outside the flight envelope
is implied. Mode state and target live in the renderer-independent simulation;
input tapes retain mode commands. Future navigation sets an optional world X/Z
target in feet with its waypoint number through `Autopilot::set_navigation_target`; invalid coordinates
clear the target. Target selection will need recording when navigation is added.

## Host acceptance

With the synthetic F/A-18D fixture at 10,000 ft, 450 knots and 70% throttle,
engage then disturb bank by 20 degrees and altitude by minus 100 ft. After
120 seconds, heading error must be below 3 degrees, altitude error below 100 ft,
and bank below 5 degrees, without a stall or crash. Apply this to heading hold
and a target at X/Z (100,000, 100,000) ft, in hybrid and legacy modes. These are
fitted host acceptance tolerances, not recovered retail performance numbers.
[Validation results](../baselines/autopilot.md) record the executed checks.

## Evidence and unknowns

Reference identity: USNF-ATF commit
`2d818054ff51db9f3353d0548dbd0e469b275a1a`,
`Docs/progress.md`, section “2026-09-09: compass direction, cloud silhouettes,
sun highlight and autopilot”, autopilot description and validation results.
That baseline reports a 30 second recovery to 178 degrees / 2866 m from an
upset at 180 degrees / 2914 m and six closed-loop hold tests. This is evidence
for the reference rebuild, not a retail measurement. Its engine is not reused.
Original FA gains, arrival radius and damage-related disengagement are unknown;
future research should recover the original autopilot behavior specification.
