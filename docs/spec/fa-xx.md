# F/A-XX concept variant

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode contract, 2026-09-18. John requested a duplicate of the
F-22 without vertical stabilizers and split flap rudder control. This is an
**opinionated** fictional aircraft, not a claim about real F/A-XX performance.

Select `faxx` independently of `f22`. Load and validate the user's F22.PT and
F22.SH, then create a separate runtime identity. Reuse F-22 cockpit, equipment,
stores, sounds, mass, thrust and flight response from [the roster spec](roster-aircraft.md).
Keep all three flight adapters and their defaults. The original F-22 is unchanged.

Agent choices: omit the imported vertical fin polygons from the intact exterior.
Duplicate each existing inboard flap skin at runtime into two leaves. Normal
flap deployment remains the midpoint, 0.4 radians at full extension. Rudder opens
the commanded side by plus/minus 0.6 radians at full input, proportional to input
magnitude, closing at neutral. Positive input opens the right flap; negative
opens the left. Other surfaces, bays and glass keep the F-22 animation contract.
Reuse the existing flap hinge at source (side * 17, -22, 0).

Yaw retains the F-22 control law and 0.1-second control response. The split drag
rudder is a visual representation of that yaw authority. Independent leaf drag,
added deceleration, roll coupling and loss of fin stability are not simulated.
This deliberately preserves the requested F-22-like handling. Those aerodynamic
changes would need a separate opinionated handling specification.

Damage reuses F-22 bodies with their remaining fin faces omitted. The selected
F22_C damaged body and F22_D fragment keep the existing damage behavior. Split
flap animation applies to the intact aircraft only.
No retail-derived meshes or textures are written into the repository.

## Retractable hook

John requested a hook hidden in normal flight on 2026-09-18. **Opinionated**
addition: H toggles a fully concealed hook, stowed at spawn, with the existing
3-second device travel and reversible motion. F-22 remains without hook controls.

John requested a more inset position using an annotated screenshot on
2026-09-18, then requested the same angle with the tip reaching the bottom
of the deployed wheels in level flight. The hinge remains at the inset position.

Agent-authored geometry: a centerline metal shank and wider terminal shoe,
using the imported gray palette. In source right/forward/up coordinates the
hinge is (0,-26,-5). The shank spans x=±0.45, y=-50.011495..-26, z=-5.45..-4.55;
the shoe spans x=±0.9, y=-51.011495..-47.011495, z=-6.3..-5. Rotate both rigidly downward
through 0.75 radians at full deployment. At the existing one-third-foot scale,
the shank is approximately 8.004 feet long. Its length is chosen so the lowest
shoe point reaches source z=-23, the deployed wheel-bottom plane: length =
(18 - 1.3*cos(0.75))/sin(0.75) - 1 source units.
Emit no hook faces at zero extension. Intermediate
poses emerge from beneath the aft fuselage. No extra bay doors are modeled.
The hook is omitted on damaged bodies, which retain their existing static rig.
This is device presentation and control, without new carrier arrestment physics.

Validation: [implementation baseline](../baselines/fa-xx.md).

Developer reuse: [source kit and original-game mod limits](../fa-xx-developer-kit.md).

## Original-game export status

The behavior above is implemented in T.O.R.E. An experimental original-format
separate F/A-XX definition and shape family are exported with discrete flap/rudder/hook poses. Its
[fitted export contract](fa-xx-export.md) records the differences, resource
requirements and unknown original-game operation. The
[packaging baseline](../baselines/fa-xx-packaging.md) records validation.
