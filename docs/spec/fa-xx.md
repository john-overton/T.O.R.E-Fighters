# F/A-XX concept variant

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode contract, 2026-09-18, donor changed 2026-09-22. John
requested a duplicate of the F-22 without vertical stabilizers and split flap
rudder control. On 2026-09-22 he moved the donor from the F-22A to the retail
**F-22N**, which already carries a tail hook. This is an **opinionated**
fictional aircraft, not a claim about real F/A-XX performance.

Select `faxx` independently of `f22n`. Load and validate the user's F22N.PT and
F22N.SH, then create a separate runtime identity. Reuse the F-22N cockpit,
equipment, stores, sounds, mass, thrust and flight response from
[the roster spec](roster-aircraft.md); the F-22N shares the F-22A cockpit art,
radar, gun and fitted handling. Keep all three flight adapters and their
defaults. The original F-22A and F-22N are unchanged and both stay selectable.

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

Damage reuses F-22N bodies with their remaining fin faces omitted. The selected
F22N_C damaged body and F22N_D fragment keep the existing damage behavior. Split
flap animation applies to the intact aircraft only.
No retail-derived meshes or textures are written into the repository.

## Retractable hook

John requested a hook hidden in normal flight on 2026-09-18, and on 2026-09-22
chose the F-22N's own hook over the earlier agent-authored geometry. H toggles
the hook, stowed at spawn, with the existing 3-second device travel and
reversible motion. The F-22A remains without hook controls; the F-22N has the
same hook control as a stock capability.

The hook is the donor's `_PLhook` branch: two coplanar centerline faces forming
one blade whose root sits at source y=-11..-7, z=-9 and whose deployed tip
reaches z=-23, the deployed wheel-bottom plane. **Fitted** stowing, agent choice
2026-09-22: rotate the blade rigidly about the x axis through the root hinge
(0,-9,-9), raising it by 0.9 radians times (1 - extension). Full extension is
the unmodified source pose. Emit no hook faces at zero extension. Intermediate
poses emerge from beneath the aft fuselage. No extra bay doors are modeled.
The hook is omitted on damaged bodies, which retain their existing static rig.
This is device presentation and control, without new carrier arrestment physics.
The earlier authored shank-and-shoe geometry is retired.

Validation: [implementation baseline](../baselines/fa-xx.md).

Developer reuse: [source kit and original-game mod limits](../fa-xx-developer-kit.md).

## Original-game export status

The behavior above is implemented in T.O.R.E. An experimental original-format
separate F/A-XX definition and shape family are exported from the F-22N donors
with discrete flap/rudder poses and the donor's own hook. Its
[fitted export contract](fa-xx-export.md) records the differences, resource
requirements and unknown original-game operation. The
[packaging baseline](../baselines/fa-xx-packaging.md) records validation.
