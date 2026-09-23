# Keyboard map update conventions

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

These conventions describe [tore-keyboard-map.html](tore-keyboard-map.html).
They preserve its content and visual language. Repository contributor rules
remain in [AGENTS.md](../AGENTS.md).

## Content and binding sources

John requested on 2026-09-23 that additions cover only normal flight, comms and
views, with no test commands. Flight includes ordinary weapons, sensors,
instruments and navigation used during a sortie.

- Exclude range fixtures, developer/test commands, damage injection, diagnostic
  toggles, startup command-line flags and AI tuning. Do not include a command
  simply because it appears in the complete input catalog. In particular,
  Shift+I incoming missiles, Shift+Y target jammer and backslash range reset
  do not belong on this map.
- Use [the input catalog](../crates/tore-app/src/input_catalog.rs) and
  [generated controls list](CONTROLS.md) for current default bindings. Check
  [input handling](../crates/tore-app/src/input.rs),
  [flight shortcuts](../crates/tore-app/src/flight_ui.rs) and
  [instrument controls](../crates/tore-app/src/instruments.rs) for context.
  The [input guide](INPUT.md) explains binding and modifier conventions.
- Show actual defaults. An action with no default key does not get an invented
  key. Profiles can remap controls; the map is not a display of the user's profile.
- Identify context or capability limits in the key label, tooltip or callout.
  Examples include F-22 weapon bays and Home while the live map is open.
  Instrument button letters are not automatically global keyboard bindings.
- Check relevant behaviour specs when describing effects or timing. For example,
  the [ejection spec](spec/ejection.md) governs confirmation and the
  [autopilot spec](spec/autopilot.md) governs waypoint fallback. Do not promise
  unsupported behaviour in a short label.

## Sheets and key labels

Retain the three sheets: **Fly & Fight**, **Comms**, and **Cockpit & View**.
Place an action on the sheet for its purpose. One physical key can appear on
several sheets with different commands. A dim key means there is no highlighted
command on that sheet, not that it has no binding anywhere in the game.

Use physical US keyboard positions and readable uppercase labels. Show the
base key once at the top, followed by a separate coloured row for each command.
Retain the existing badge conventions:

| Badge | Meaning |
| --- | --- |
| No badge | Base key, no modifier |
| S | Shift |
| C | Ctrl |
| A | Alt |
| CS | Ctrl and Shift together |
| AS | Alt and Shift together |
| S/C | Either Shift or Ctrl, not both required |

Keep modifiers on the command row they modify. Do not make Shift+E look like
unmodified E, or Alt+E look like the same action as Shift+E. Modifier badges
contain only modifiers; an aircraft or context label is ordinary command text.

Use short, unambiguous key labels and longer callouts or tooltips for details.
Ejection gets a visible two-press callout explaining release and the confirmation
interval. Holding a key is not a second press. Keep E's engine action visible.

## Colours and layout

Retain the existing palette and the legend on each sheet:

| Role | Colour |
| --- | --- |
| Flight; instrument selection/buttons; tower | Green `#7fd08a` |
| Systems; cockpit window toggles; formation | Cream `#efe7a6` |
| Weapons; wing orders | Coral `#ec7a6e` |
| Sensors/navigation; views; wing address | Blue `#7fb2e8` |
| HUD and map | Purple `#b8a2e8` |
| Approach-target orders; existing game/session controls | Orange `#f0a860` |

Preserve the 1920×1080 sheet canvas, key geometry, embedded VT323 font and badge
art. Match neighbouring row styles. Put explanations in callouts rather than
crowding a key or clipping text. Changing a callout also requires updating or
removing its leader line; it must never point to a different command's key.

Keep the S/C/A legend, remapping location and default-binding notice readable.
The on-screen export toolbar must not obscure them. Sheet navigation and export
controls are document controls, not new game bindings.

## Review and export checks

- Compare relevant normal defaults with the input catalog; check both new and
  existing entries for omissions or stale descriptions. Explicitly exclude test
  and range bindings during this comparison.
- Check all three sheets in a browser at 1920×1080 and at a smaller viewport.
  Verify every modified key, modifier badge, callout and leader. Text must not
  overflow, overlap unrelated controls or rely on a tooltip to explain
  ejection confirmation.
- Check tab/hash navigation and export-size selection. Confirm PNG output at
  1080p and 4K retains the new labels. Preserve ZIP, PDF and print support;
  exercise any export behaviour changed by the edit.
- Preserve embedded font/image bytes during text edits. Keep temporary captures
  in ignored `.local/`. Do not embed or commit new retail media.
- Update this file in place when John changes these conventions. Do not append a
  revision log. If a game binding changes, update the catalog and regenerate
  [CONTROLS.md](CONTROLS.md) using its documented command in the same change.
