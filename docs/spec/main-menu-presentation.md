# Main-menu branding and button text

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. **Opinionated** presentation requested by John on
2026-09-23: show the project badge at the bottom left with the version beside
it, and lower button labels onto the raised button faces. Coordinates below
are agent choices on the existing 640 by 480 menu canvas.

The main-menu footer uses the supplied [project logo](../images/tore-fighters-logo.png)
in place of the written T.O.R.E name. It is 64 by 64 pixels at (14,408), with
`vX.Y.Z` at (86,433), vertically aligned beside it. The version retains the
bundled white 14 px font and its one-pixel dark shadow. Existing menu scaling
scales the complete footer. The badge has transparent corners; no opaque box
covers the background. Popups and notices can cover the footer.

The bitmap is the existing 64 px project icon, exported to straight-alpha
RGBA by `tools/package/build_icons.py` and compiled into the application.
It is project-owned art, not retail data. Its provenance is recorded in the
[asset manifest](../../crates/tore-app/assets/README.md#application-icon).
No new runtime image dependency or external file is required.

Main activity button labels move down two pixels, from y+4 to y+6 relative to
the button's draw origin, for both active and disabled buttons. Horizontal
alignment, original fonts and original button artwork stay in place. The
one-pixel movement with a pressed button remains shared by its label and art.

[Visual and input validation](../baselines/menu-ui-polish.md).

## Replays entry

**Opinionated** addition requested by John on 2026-09-26, like the version
badge: the top bar gains a fourth entry, **Replays**, after Multi. The retail
bar's three entries, ?, Pref and Multi, their dropdowns and positions are
unchanged. Replays uses the same MENUFONT label, centered by its visible
glyph bounds, and the same highlight. Its area is 66 by 19 pixels at y=38,
starting where Multi ends: x=180 on CHOOSEV, CHOOSEU and CHOOSEM, 174 on
CHOOSEAC and 289 on CHOOSE3, following each background's bar origin. It has
no dropdown: a click, or Enter with keyboard focus, opens the
[Replays screen](../REPLAYS.md#replays-screen) at once with the button
click sound. With another dropdown open, moving onto Replays keeps the menu
open with no list showing, and moving back reopens that dropdown. Tab
reaches it after Multi and before the activity buttons. The activity
buttons are unchanged, including the disabled Replay Last Mission, which in
the original means fly the last mission again.
