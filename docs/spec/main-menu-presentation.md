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
bar's three entries, ?, Pref and Multi, retain their horizontal positions and
existing dropdown actions. The taller CHOOSE3 layout below changes vertical placement. Replays uses the same imported MENUFONT glyphs and palette. The
original labels and Replays share the vertical placement of Multi, centered using Multi's
visible glyph bounds. The p and y descenders extend below that baseline
instead of lifting the whole word. Horizontal centering stays per label.
The normal, hover, keyboard-focus and pressed states keep this placement.
Hover and focus use the existing filled highlight, with no added underline.
Its area is 66 by 19 pixels at y=38, except in the taller CHOOSE3
project-link layout below. It starts where Multi ends: x=180 on CHOOSEV, CHOOSEU and CHOOSEM, 174 on
CHOOSEAC and 289 on CHOOSE3, following each background's bar origin. It has
no dropdown: a click, or Enter with keyboard focus, opens the
[Replays screen](../REPLAYS.md#replays-screen) at once with the button
click sound. With another dropdown open, moving onto Replays keeps the menu
open with no list showing, and moving back reopens that dropdown. Tab
reaches it after Multi and before the project links. The activity
buttons are unchanged, including the disabled Replay Last Mission, which in
the original means fly the last mission again.

The alignment correction was requested by John on 2026-10-21. Using Multi as
the common vertical reference is an agent implementation decision. On the
imported 1.02F MENUFONT at 640 by 480, the dark letter core has a 9-pixel cap
height (y=42..50), a 7-pixel x-height (y=44..50), and its last baseline row
at y=50. Replays' descenders reach y=52. Including the imported edge shading,
cap glyphs occupy y=41..52 and descenders reach y=54. The same font supplies
all weight, colour and shading; no glyph scaling, recolouring or composed
retail image is introduced. The visible Multi-to-Replays gap stays 8 pixels
on the base canvas. These are measurements of the imported font in this
renderer, not a claim of comparison with the original running game.

[Pixel measurements and state checks](../baselines/menu-replays-label.md).

## Multiplayer entries

The Multi menu's authored entries and Internet hint follow the
[multiplayer menu specification](multiplayer-menus.md#names-and-navigation).

## Updates and project feedback

**Opinionated**, requested by John on 2026-10-23. Choose Activity always shows
**Leave Feedback** at the right of the bar. It opens the project's GitHub
[issues page](https://github.com/john-overton/T.O.R.E-Fighters/issues), since
this repository has no issue templates. When a newer release is known,
**Update Available: vX.Y.Z** appears to its right and opens that release's page.
Both use the imported MENUFONT, Multi's baseline, and existing highlight and
click sounds. Mouse clicks, Tab or arrow focus plus Enter/Space, and controller
menu navigation/accept reach both. Pref adds **Check for updates: On/Off**.
The default is On. Turning it off hides the update item immediately and stops
future checks. A request already in progress may finish, without showing it.

Agent layout decisions: the rightmost item ends at x=632 on the 640-pixel
canvas; each label has 6 pixels of padding per side. Feedback precedes Update
Available in focus order, after Replays and before the activity buttons. The
left group retains its artwork origin. The blank bar texture extends to x=636.
On CHOOSE3 with an update, the bar grows from 19 to 38 pixels high: Leave/Feedback
and Update Available:/version wrap onto two lines. The other entries center
vertically and dropdowns start below the taller bar. This keeps labels off the
Jane's logo and above the activity panel. The other backgrounds keep one row.
At least 4 pixels separate the two groups. Exceptionally long prerelease labels
end in an ellipsis to fit; the
release page carries the full version. A result arriving during a click clears
the old hover/press/focus so a moved item cannot receive that click.

Agent network decisions: on first reaching Choose Activity during an ordinary
interactive run, one background worker reads the profile's cache and, if stale,
makes one anonymous HTTPS request to `api.github.com`. It sends no credentials,
player name, profile contents or hardware details. GitHub still receives the
normal network address and a fixed application User-Agent. The menu never waits
for the network. A check considers the first 100 published releases returned by
GitHub, includes prereleases, excludes drafts, and chooses the greatest semantic
version. Only strictly greater precedence than the running version is shown;
build metadata is ignored. Git-describe distances such as `0.2.0-3-gabc` use
`0.2.0`; a real prerelease before that suffix stays a prerelease.

The request has a 10-second total timeout, a 2 MiB response limit, no redirects
and no retries. A result, no update, failure or interrupted attempt is cached for
86,400 seconds in `update-cache-v1.json`; a future timestamp is invalid. The
cache stores the release, not a decision tied to a particular running version.
If the profile cannot save the attempt, networking is skipped. Only one attempt is allowed per
process, even if the player toggles the setting. This avoids repeat requests
when rate-limited. `updates-v1.conf` saves the preference independently from
flight preferences. Missing settings default on; unreadable or malformed
settings disable networking. Snapshot, smoke and scripted diagnostic runs make
no release requests. Preview states `update-available`, `update-focus` and
`updates-off` provide deterministic test pictures without contacting GitHub.

The browser uses the platform opener, with no elevated privileges. Only the
fixed project issues URL or a validated project release URL is accepted. This
change does not download, install or restart software. Installation remains the
player's normal platform workflow from the release page. Cross-platform browser
launches still need runtime checks on Windows and macOS.
