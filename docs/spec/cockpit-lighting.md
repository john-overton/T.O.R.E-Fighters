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
the prior noon appearance with flood off. Ambient and flood then light only
non-emissive surfaces. HUD copies on camera pages retain their
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

John requested ambient darkening on 2026-10-21 after reviewing the first night
captures. Cockpit surfaces and instrument bezels now use the world's continuous
sun direction, its -6 to +2 degree sky/day blend, its moon light vector and its
dense-weather sunlight transmission. These are the same inputs used by terrain
and aircraft in [surface lighting](surface-lighting.md). There is no independent
cockpit clock or night toggle. Dense bands use the same 600-foot optical path
and density/visibility thresholds; their record parameters interpolate with the
world's fractional weather weights.

The conversion to a flat painted cockpit is **fitted**, an agent decision:
`day * (0.28 + 0.90 * max(sun.y,0) * visible_disc * transmission) /
(0.28 + 0.90 * sin(25 degrees)) + night * (0.065 + 0.055 * max(moon.y,0))`,
clamped to 0..1. `night` is the existing world night factor and `day = 1-night`.
These are display-channel multipliers. A clear sun above 25 degrees leaves ambient at 1, preserving
the previous default noon image exactly; sunlight below that fades the art, with only 6.5 to 12
percent remaining at night depending on moon height. Cloud cover reduces direct
light, as it does on world surfaces. The values are not recovered retail lighting.

Flood has Off and levels 1..5, default 3. Its level strengths are
0, 0.18, 0.36, 0.60, 0.82 and 1.0. Multiply by
`(0.03 + 0.97 * (1-ambient)^2) * (1 + 0.5 * (1-ambient))`, so daylight
at ambient 1 retains the previous default noon appearance, while flood becomes the main surface light at night. Two soft
pools remain at normalized art positions (0.16,0.72) and (0.84,0.72), with
horizontal radius 0.32 and vertical radius 0.55 and Gaussian falloff. Bezels
receive 65 percent of this strength.

Apply ambient to the original surface colour, then add red flood reflection
from that original colour, so the reflection does not disappear with the sun:
`red = original_red * ambient + (22 + 0.35 * original_red) * flood`.
Green and blue are `original * ambient * (1 - 0.12 * flood)` and
`original * ambient * (1 - 0.16 * flood)`. Instrument brightness still scales
the result. Preserve alpha and replace mirrors with their real world view after
lighting. Nothing lights the canopy opening or changes the HUD glass symbols.

Display contents and their programmed indicators stay self-lit at the selected
instrument brightness, independently of ambient and flood. HUD brightness and
colour remain independent too. **Agent choice:** generated bezel titles, numbers,
button legends and pressed-button marks have a subtle backlight. Each channel
has a floor of `original * 0.18 * (1-ambient)` before instrument brightness.
It fades out continuously by day, so noon pixels do not change. Preserve the
original legend colours and their light/dark contrast. Only explicitly drawn
UI legends get this treatment; no colour heuristic guesses lamp locations in
retail art. Decorative pixels and button faces remain painted surfaces. Live
lamps drawn inside instrument displays remain emissive along with those displays.

Every change shows a brief HUD notice. Preferences save HUD brightness, colour,
instrument level and flood level between flights and launches. Old files load
with green, instruments 4 and flood 3. No simulation, network or replay format
changes are needed.
