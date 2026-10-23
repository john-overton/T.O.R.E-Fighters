# Replay display records

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. Format 4, requested by John on 2026-10-21, records the
[HUD and sensor behaviour](../spec/target-window.md#sensor-view-in-a-replay)
at 24 Hz. The existing bounded event grammar is unchanged. Live mission and
network-capture writers request version 4; the reader still accepts 1, 2 and 3.
Legacy writers can still select 2 or 3. Unknown event fields remain skippable.

`aircraft.hud` and `aircraft.sight` use the same subject aircraft and object
(target) identities as before. Human cockpits emit on the first observation
and at the first observation in each subsequent five-tick interval. The host
observes every tick, giving samples on ticks divisible by five after the
first. Existing non-cockpit records remain change-driven. No simulation,
random-number, input or network-wire behaviour changes.

## Added fields

| Event | Fields | Meaning |
| --- | --- | --- |
| `aircraft.hud` | `target_x`, `target_y`, `target_z` | Displayed target position in world feet, including ground targets. Absent when no target is displayed. |
| `aircraft.hud` | `velocity_x`, `velocity_y`, `velocity_z` | Displayed target velocity in feet/second. Present with the target position. |
| `aircraft.sight` | `display_24hz` | Boolean true identifies the new full sight sample. Absent means legacy sparse data. |
| `aircraft.sight` | `gimbal_limit` | Boolean, live limit indicator. |
| `aircraft.sight` | `impactN_x`, `impactN_y`, `impactN_z`, `impactN_range`, `impactN_spent` | N is 0, 1 or 2, for 25, 40 or 105 mm. The exact displayed impact position and muzzle range in feet, plus a Boolean spent flag. An absent group means no pipper for that gun. Target-motion correction and firing-phase ballistics are already included. |
| `aircraft.sight` | `mark_x`, `mark_y`, `mark_z` | Live pinned or tracked mark in feet. Absent for free slew. The existing `x_ft/y_ft/z_ft` remains the aim point. |
| `aircraft.sight` | `radar_dots` | Numeric word array, carried by the existing `Ids` value encoding. Four u32 words per contact: IEEE-754 f32 X/Y/Z world feet, then symbol in bits 0..7 (0 dot, 1 cross, 2 square) and selected in bit 8. |

Pipper, target, aim and angle values retain the event encoder's f64 precision.
Only radar glyph positions use f32; at one million feet the maximum rounding
error is 0.03125 ft, far below a radar pixel. The list is bounded to 128 contacts
(512 words, below the value limit of 1024). The live readout currently caps its
ground list at 50. A reader ignores incomplete tuples, non-finite coordinates
and unknown symbols. It filters radar glyphs to the recorded console range.
A full three-gun sight fits below the 64-field event bound. Names and status
strings use the existing per-chunk string table; no artwork is embedded.

## Playback

New pippers use their stored impact points and the live ring/range-arc rules.
An absent impact in a new sample stays absent. Only legacy samples recompute
an impact from the old gun trains. The displayed target position is used even
when the replay scene has no aircraft pose for that ground object. Target names
come from the recording's existing object identities. Discrete symbols hold;
only camera pointing interpolates between adjacent full samples, per the spec.
Seeking reads the last sample at or before the requested tick, with no mutable
interpolation history, so backward seeks and repeated stills agree.
