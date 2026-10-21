# Cockpit lighting controls

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, requested by John on 2026-10-21. HUD brightness uses
[the recovered palette rule](../formats/weather.md#hud-palette-consumer-continuation-2026-09-15).
The remaining presentation constants and chord behavior are agent decisions.

## HUD

Shift+[ and Shift+] retain 33 settings, -256 through +256 in steps of 16,
neutral zero. Apply the recovered rule to HUD ink only. The neutral green uses
the existing aircraft palette exactly. Ctrl+H cycles green, amber, white and
cyan, independently of instruments. Alternate base RGB values are (255,176,48),
(220,240,255) and (64,220,255); brightness retains the same black/white endpoints.
Colour selection is opinionated: the retail manual documents Dim HUD and
Brighten HUD, but no HUD colour key or menu item was found. PREF also exposes
colour directly and the instrument/flood commands in its Cockpit lighting
submenu, keeping every row clear of the bottom buttons. All HUD copies in live
flight share the ink, including the front-view instrument. Replay HUD cameras
use current local saved choices through the existing tape whitening and
softness; historical lighting changes are not recorded.

## Instruments and flood lights

Instrument levels 1..5 multiply screen contents, bezels and cockpit art by
0.25, 0.5, 0.75, 1.0 and 1.25, clipping channels at 255. Default is 4, exactly
the prior appearance with flood off. HUD copies on camera pages retain their
own brightness. Mirrors and the world outside the canopy are unaffected.

Ctrl+I selects instrument adjustment and Ctrl+Shift+F selects flood adjustment.
Retail Ctrl+F is Frame rate in the [keyboard spec](keyboard.md), so the agent
selected the nearest unused chord. Controls can still bind flood to Ctrl+F.
Ctrl+I has no conflict in the app catalog or reviewed retail keyboard spec.
Then - or + (also =) changes one step, with or without Ctrl still held.
The selection expires after four seconds without adjustment, or on another
key, menu or map. Each adjustment refreshes the four seconds. This is an
opinionated chord, not a requirement to hold a letter. Both selectors and
separate dim/bright commands are bindable in Controls, under View.

Flood has Off and levels 1..5, default 3. Two fitted soft pools originate at
normalized cockpit-art positions (0.16,0.72) and (0.84,0.72), with horizontal
radius 0.32 and vertical radius 0.55. Gaussian falloff lights covered art only.
The red reflected-light contribution preserves surface texture and alpha,
never covers glass, world pixels, mirrors or display contents. Bezels receive
a red contribution at 65 percent of the flood strength. On an 8-bit surface
colour, flood strength times falloff adds `22 + 0.35 * red` to red, and
multiplies green by `1 - 0.12 * strength` and blue by
`1 - 0.16 * strength`, before instrument brightness. Solar elevation scales intensity from full at night
to 3 percent in daylight, transitioning from sine elevation -0.05 to 0.35.
This is fitted illumination of flat retail artwork, not recovered retail lights.

Every change shows a brief HUD notice. Preferences save HUD brightness, colour,
instrument level and flood level between flights and launches. Old files load
with green, instruments 4 and flood 3. No simulation, network or replay format
changes are needed.
