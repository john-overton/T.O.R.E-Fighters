# Aircraft devices and exterior materials

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation contract, 2026-09-16. John requested an animation pass and
orange exterior F-22 canopy tint, with a clear cockpit view. The existing
F/A-18D, Rafale, F-14D, A-4E and X-31 rigs remain supported. This pass completes
initial moving-device presentation on the seven roster additions. No AI or
flight-force changes are authorized or needed.

## Fitted animation

All angles, timing and hinge choices below are agent fits, not measured original
schedules. Source part identities are in [shape notes](../formats/objects-and-shapes.md).
Use the actual imported polygons and preserve texture coordinates when splitting
panels across a hinge. Rotate normals with the polygons. Neutral controls and
fully deployed gear must retain source geometry exactly.

Retain 3-second gear/flap/brake travel. Gear uses aircraft-specific attachment paths below, hiding at full retraction
unless a retained exposed joint is explicitly documented. Wheel/shaft cards
remain rigid. Separately modeled fitted brace connectors may change length
while keeping their body roots and distal pin edges attached, as specified per
aircraft. Source brake panels rotate from a fitted
closed pose to the original deployed endpoint; no instant popping during travel.
The legacy defaults are overridden by the individual source-endpoint morphs
and correction sections below. Default trailing flaps droop up to 0.4 rad, outboard ailerons deflect up to
0.2 rad, pitch surfaces up to 0.3 rad and rudders up to 0.35 rad. Opposite wing
roll deflections use opposite signs. Su-35 canards use 0.25 rad pitch. Existing
rigs retain their documented values and applicable hooks; no hook is added to
an aircraft without a reviewed carrier hook capability.

Gear pivots use each shape's source units, in right/forward/up order. The
individual correction sections below define the current reviewed paths.

F-22N retains its own reviewed face addresses and the shared family behavior
described in the [attachment corrections](#f-22-family-attachment-corrections). Its bay belly uses
face 39dc in place of the F-22A's 3d03, a fitted agent choice since the belly
is remodelled. Its native hook stows about the fitted root hinge (0,-9,-9)
through 0.9 radians, per the [F/A-XX contract](fa-xx.md#retractable-hook).

Switched brake retraction angles multiply (1 - brake fraction). Their source
pose is the deployed endpoint. MiG-21 has an independently fitted ventral brake
strip at source y=-8..12, hinged at (0,12,-8) about x through 0.6 rad. MiG-23
has fitted aft side strips at y=-24..-16, hinged at (±4,-16,0) about vertical
through ±0.6 rad. These split their own source skin, retaining the surrounding
fuselage and UVs; no switched brake identity or original linkage is claimed.
F-14 brakes use the [current source-specific closure](#f-14d-attachment-corrections).

MiG-23 sweep and coupled flap attachments use the
[current correction contract](#mig-23-attachment-and-material-corrections).
Retain wingtip vapor attachment to the swept wing. This presentation does not
add a sweep-dependent flight law.

## Rafale attachment corrections

Agent-authored fit, 2026-10-05, for exact RAFALE.PT/RAF.SH. The old inward
main-gear fold crossed the wheel cards. Fold each complete main wheel/strut
rigidly aft through pi/2 around [+/-4,14.545455,-7], lateral axis. This fitted
pivot is the front endpoint of the painted brace attachment band, mapped from
source UV [239,128]. Both mains retain their own source X plane, so their card
separation stays 8 source units. Full deployment is exact; stowed cards fit
X=+/-4,Y=3.54..14.55,Z=-7.46..2.55. All 2,908 inspected opaque samples
fit neutral-body sections at stow. Five of 578 full-card grid samples lie
outside those sections, on transparent margins; a rectangle is not the painted
wheel volume.

Preserve the separate thick leading vertices of both flap skins at source
Y=-23 while moving their common trailing points under the existing fitted mix:
`0.40*flap - 0.30*elevator +/- 0.20*aileron` radians. This constrained skin
motion prevents the old leading-edge detachment. Recompute deformed normals
with the original face orientation. Existing canard, rudder, brake and exhaust
laws remain, with independent source-root and signed-motion checks.

The nose door keeps its actual diagonal source upper edge [-1,52,-6] to
[-1,68,-5], rather than rotating around a horizontal line one unit above its
rear root. Use its own [0,16,1] direction and the existing last-quarter door
schedule. The fitted nose-wheel fold remains about its source forward upper
corner [0,60,-6]. Zero gear selects the source closed belly and hides the open
gear/door branch. Original retraction mechanics and flap interpolation remain
unknown; nonzero source control branches use arithmetic outside the reviewed
static reader grammar.

## Nosewheel steering

John requested on 2026-10-02 that steering turn only the wheel and steerable
strut. Use the reviewed wheel/strut skins in each aircraft's own source model;
leave separately modeled braces and doors fixed during steering. Retraction
still moves the full gear group. Some source models combine wheel and strut in
one textured panel, so those move together. Steering angle comes from
[the sim's ground-speed rule](lateral-flight.md), not a renderer approximation.

## Double-sided panels

Fix dated 2026-09-23, for John's report that the F/A-18 speed brake showed its
underside art on top when deployed. Thin SH panels such as that brake, fins,
tails, doors and gear legs are two faces over the same vertices with opposite
stored normals. On the F/A-18 brake the textured face is the underside and the
top skin is a flat color. Original rendering culls by stored normal, so only
one side shows. Smooth rendering keeps every face so shadows do not depend on
the camera, and both sides then competed at equal depth: the underside, drawn
first, won from above and flickered in from below.

Agent decision: with smooth rendering, draw only the member of each such pair
whose stored normal faces the camera more; always keep exactly one, including
edge-on. Shadows are unchanged, because both members cover the same area.
Pairs are matched on the final animated faces, with vertices rounded to 1/64
source unit so clipped rudder pieces still pair. This applies to every
aircraft drawn through the shared exterior model path. Stepped rendering keeps
its full normal culling. Single-sided faces seen from behind still draw with
smooth rendering; that difference from the original's culling is unchanged.

The MiG-21 is the exception: smooth rendering culls its faces by stored normal
as stepped rendering does. Its upper and lower wing skins are divided into
different polygons, so exact twin matching cannot pair them and the underside
showed through the top skin. Their stored normals do oppose, so the normal test
separates them. This is an agent decision, a presentation fix for John's
2026-09-30 report, first made on a side branch; flight, control-surface state
and contact are unchanged. Shadows of the MiG-21 are now built from the faces
that face the camera, so a shadow caster seen only from behind may drop out of
that aircraft's shadow, a small departure from the camera-independent shadows
of the other aircraft. Source evidence:
[MiG-21 skin review](../formats/objects-and-shapes.md#mig-21-skin-review).

## F-22 main weapon bays

Add a main-bay presentation with 1-second travel and 90-degree
outward-opening doors. O, FA's bomb-bay key, toggles the bays open and shut, and
the input action `bay` can be rebound and recorded. Aircraft without the
reviewed F-22 bay rig ignore the command.

The bays stay shut until a weapon is released (**opinionated, requested by John
on 2026-09-28**). Pressing the trigger for a bay weapon whose shot is otherwise
ready opens the doors; the weapon leaves as soon as they are open, about one
second later, and the doors close 1 second after the release. One press
commits the shot, so the trigger need not be held. While the doors open the
HUD shows `OPENING BAY`. The press lapses, and the doors close, if the shot
stops being ready (disarm, a new selection, a lost target) or the doors are not
open within 3 seconds. Doors the pilot opened with O stay open, and a release
through them is immediate. The gun is not behind the doors. The 1-second hold
and 3-second lapse are **fitted**. Designating a target no longer opens the
bays. No weapon eligibility, mass, drag or damage rule changes.

Clip the imported belly panels over the two reviewed source bay rectangles,
retaining surrounding fuselage and the original material on moving doors. Each
bay has two doors, each half its width, hinged at the bay's outboard and inboard
edges and swinging 90 degrees down; fully open they hang where the source's
open-bay walls hang. Behind them is a recess 3 source units deep (**fitted,
agent choice 2026-09-28**). The source's open-bay pose, which the shape switches
on while the bays are open, lines it: each wall's bay-facing side becomes a
side of the recess and closes its half of the recess ends, and the F-22A's
textured bay interior becomes the recess ceiling. F-22N and F/A-XX have no
interior art (their texture repaints it), so their walls also roof the recess
in the walls' grey. Drawn unmodified, that pose read as a second, instant set of
open doors over a flat panel below the belly. This is a fitted main-bay
presentation, not recovered original door sequencing or side-bay parity.
`--flight-bay 0..1` provides an explicit inspection pose, rejected on other planes.

## F-22 canopy

Opinionated appearance requested by John: apply an amber/orange grade only to
reviewed exterior glazing faces. Preserve source texture shading and highlights.
Blend 75% toward amber RGB (0.95, 0.45, 0.08), scaled by 0.3 + 0.7 times source
luminance, retaining 25% of the sampled source color. Frame and surrounding
fuselage materials remain unchanged. Cockpit artwork, HUD and forward-view
world rendering stay clear. Do not alter any engine face on F-22.

John requested 75% opaque exterior glass on 2026-09-16. Keep the grade above
and alpha-composite the nearest glazing surface at opacity 0.75 over the
already rendered aircraft and world. This is actual 25% transparency, not a
reduction in orange tint strength. Agent choice: resolve nearest glass depth
before blending, so overlapping front/back glazing does not compound opacity.
No refraction or new cockpit geometry is introduced.

## Validation and unknowns

Test fixed hinge points, rigid gear lengths, paired surface direction, split
panel continuity, bay timing/reversal/interpolation, unsupported commands and
input roundtrip. Inspect closed/partial/open views and F-22 cockpit/exterior
views. Original schedules, exact mechanical linkage, side bays, original damage transitions, LOD
variants and retail comparison remain unknown. Damaged body rendering follows
the separate [damage and smoke specification](damage-smoke.md). Next research is source shape
control consumers and authored fits can ship without claiming original parity.

## F/A-18D attachment corrections

Agent fits, 2026-10-05, on exact F18.PT. Main assemblies remain rigid and fold
forward 140 degrees, left about [-8,-7,-6] along [1,-0.46,0], right about
[9,-7,-6] along [1,0.55,0]. Keep both complete main assemblies at zero gear:
tires fit inside the body while a small original upper-right joint remains
visible. Do not erase or distort the painted joint to claim total enclosure.
Nose gear closes forward 130 degrees around [0,55,-6], retaining steering.
Its separate brace fixes painted upper center [0,44,-5.5], while lower center
[0,54,-11] follows the nose gear. Use a noninverting telescoping centerline
fit with constant perpendicular thickness, not a shrinking wheel.

Main doors keep their own upper edges X=-3/+4,Z=-6. During the final quarter
of closure, their lower boundaries morph from X=-5/+6,Z=-15 to X=0.5,Z=-6.
This is an explicit contracting-door fit. Flaps retain each thick forward edge
and move shared trailing points by 0.52 radians times flap around their mean
seams: right [13,-7,3.5] along [26,-3,-2], left [-13,-8,3.5] along [25,2,2].
Recompute normals from the posed polygons, preserving their facing direction.

Existing stabilator, fin, brake, hook and flame travel remains. Validate their
actual shaft/cut and painted attachment points, including stabilator shaft
Y=-43,Z=0, brake front Y=-28,Z=5 and hook painted center [0,-37.5,-3].
Transparent image corners are not independent mechanical attachments.

## MiG-29 attachment corrections

Agent fits, 2026-10-05. Pitch moves only aft tail pieces behind Y=-41 through
-0.30 radians times elevator about Y=-41,Z=0. Forward triangles remain fixed;
remove unintended tail roll. Rudders use `Y=-32-0.20*(Z-1)` cuts and each
canted fin's own 3D cut line, through 0.35 radians times rudder. Inboard flaps
and caps morph to exact source down endpoints. Outer roll holds both thick
forward skins fixed, moving joined trailing points -0.20 radians times aileron
about outward axes [±21,-3,0] through [±36,-8,0.5].

Complete main shaft/wheel cards fold aft 90 degrees around [±17,1,0], retaining
at least 13 source units of signed lateral clearance from the centerline.
Nose cards fold aft 90 degrees around [0,47,-2]. Preserve source steering,
exact deployed geometry and hide all sixteen cards at zero. The established
upper/lower brake roots and angles remain unchanged: [0,-7,7], atan(9/7),
and [0,-8,-1], -atan(8/6), respectively. Flame scaling retains root Y=-41.

## Su-27 attachment corrections

Agent fits, 2026-10-05. Keep both tail inboard roots fixed; only distal points
carry -0.30 radians times elevator around [±21,-41,-6]. Rudders use
`Y=-43-0.25*(Z+5)` and their own canted cut axes, through 0.35 radians times
yaw. Flaperon leading edges retain Z=-1. Joined trailing points use
`Z=-1-4*flaps+4*side*aileron`, side=-1 left and +1 right, preserving source X/Y.
This reaches original signed flap endpoints, with fitted combined range -9..3.
Slats couple to flap fraction, retaining upper roots and morphing lower points
to exact asymmetric source endpoints. Original independent slat timing is unknown.

Main cards fold aft 90 degrees around [±23,-9,0]. Nose cards fold aft around
[0,60,-3]. Its separate brace fixes body edge Y=74 while its three-unit distal
edge follows the nose rigidly. No wheel card deforms. All eight gear faces
hide at zero and reproduce source deployment at one. Brake retains [0,40,11]
and atan(14/22); flame root remains Y=-60. The exact profile disables hooks.

## Su-35 attachment corrections

Agent fits, 2026-10-05. Retain the safe inward 90-degree main fold at
[±23,3,-1]. Nose gear now uses the actual front shaft edge [0,57,-1]; its
separate brace fixes Y=73 and keeps its four-unit distal edge rigid. Hide all
eighteen gear faces at zero. Preserve brake [0,23,8], atan(10/21), and exhaust.

Tail roots stay fixed while distal points carry -0.30 radians times elevator
around Y=-42,Z=-5. Canards retain their source roots and existing +0.25 radians
pitch sign. Rudders retain the valid Y=-30 cut and 0.35-radian yaw. Flaps morph
to exact own endpoints; split only the right upper source panel at X=49 to
isolate its flap. Preserve left X=-50 and other source asymmetries. Only outer
control skins carry -0.20-radian differential roll, retaining both thick
slanted forward edges. Broad forward wing skins stay fixed.

## MiG-21 attachment corrections

Agent fits, 2026-10-05. Retain rudder cut Y=-52 and 0.35-radian yaw, plus
flap hinge Y=-18,Z=-2 and 0.40-radian full droop. Outer roll uses its actual
slanted edges [±25,-15,-2] to [±38,-19,-2], with -0.20 radians times aileron
about outward axes. Tail points abs(X)<=6 stay fixed; own asymmetric distal
tips carry -0.30 radians times elevator around Y=-49.5,Z=-2. The fitted ventral
brake keeps its 0.60-radian law, retaining every thick front point at Y=12,
including both Z=-8 and -6. Surrounding belly pieces remain fixed.

Main cards retain inward 90-degree folds but use own shaft positions X=+21/-20,
Z=-1. Nose uses the actual sloped upper-edge midpoint [0,49.5,-7.5] for its aft
90-degree fold. All six cards stay rigid, reproduce deployment exactly and
hide at zero. Transparent card corners are distinguished from the painted
shaft. Source steering, afterburner capability and flame demand remain unchanged.

## Su-25 attachment and material corrections

Agent fits, 2026-10-05. Morph only the original rudder pair between its own
signed position and UV endpoints, using absolute demand for interpolation.
Host positive yaw selects the source -1 endpoint. Keep diagonal root
[0,-53,7] to [0,-60,27] and fixed fin skins unchanged. Flaps similarly morph
positions and UVs, retaining both thick forward edges and exact down art.
The valid pitch cut Y=-54,Z=3 and -0.30-radian law remain. Only outer rear wing
skins carry -0.20 radians times aileron around outward axes [±33,-2,-2] through
[±42,-8,3.5]; forward triangles and leading skin points stay fixed.

Main wheel/shaft cards fold aft 90 degrees around right [10,1,-9] and left
[-11,1,-9], preserving the source lateral offset and all card dimensions.
Separate untextured inner panels fold inward 90 degrees about their own top
edges X=±6,Z=-10. This panel assignment is fitted; original door semantics are
unknown. Nose cards fold aft 90 degrees around [0,41,-9]. Its separate brace
fixes body edge Y=51 while its two-unit distal edge follows the nose rigidly.
All eighteen gear faces hide at zero and reproduce source deployment at one.
Wingtip clamshell brakes retain front Y=-8,Z=1, closing by upper atan(5/6)
and lower -atan(4/6). No flame geometry or afterburner capability is added.

## F-22 family attachment corrections

Agent fits, 2026-10-05, preserving each exact donor. Inner flaps retain
0.4-radian full travel but use their actual diagonal mean seams: right
[16,-28,0] along [26,6,0.5], left [-17,-28,0] along [26,-6,-0.5]. Pin every
thick source front point while moving shared trailing vertices coherently.
Outer roll uses right [42,-22,0.5] along [17,-4,0.5] and left [-43,-22,0.5]
along [17,4,-0.5], through -side*0.2 radians times aileron. These are continuous
fits, not claims of original full-flap triangle interiors.

Tail motion retains -0.3 radians times elevator and opposed 0.1-radian roll,
now keeping every source body-root chord point fixed. Only distal points turn
around lateral axes at Y=-48,Z=1. Stock fins split at Z=6 and
`Y+35-0.25*Z=0`. Preserve both cuts and lower roots; trailing upper points move
laterally by `tan(0.35*rudder)*aft_distance*clamp((Z-6)/6,0,1)`. This constrained
deformation preserves each donor's cant and paired skins. F/A-XX retains its
opinionated fin removal, grey recolor and 0.4-flap midpoint with +/-0.6-radian
split leaves on these corrected seams. Positive yaw opens right leaves,
negative yaw opens left. The coordinate repair is an agent choice, not a new
user design request.

Complete main shaft/wheel cards close 90 degrees about left [-13,2.25,-5]
along [-12,-8,-1] and right [12,2.25,-6] along [-12,8,1]. These pivots lie on
painted upper braces. Nose shaft/wheel closes aft 175 degrees about [0,63,-9].
Preserve source steering and all card dimensions. Independent main doors close
side*90 degrees around their own edges X=-16/+18,Y=-5..23,Z=-3..-1. Nose door
closes -90 degrees around its own forward axis X=-2,Z=-8. Original door roots
already stand outside parts of the source fuselage; retain that source
clearance explicitly, rather than claim a flush seal. Hide gear only at zero.

Preserve brake front roots [0,-17,5] and [±3,-15,6]. Close rear points
together through 0.7*(1-brake) radians about the lateral axis at [0,-17,5],
keeping the shared center seam joined. This is constrained skin deformation,
not two independently rigid halves. Full extension matches source geometry.
Retain flame root Y=-48,
F-22N/F/A-XX 0.9-radian upward hook stow, bay belly clipping/doors/recess lining,
canopy treatment and all control/weapon timing. F-22 gains no hook. Full source
gear deployment still sets the existing 7 2/3-foot ground plane. GPU appearance
and recovered original mechanical timing require separate evidence.

## X-31 attachment corrections

Agent fits, 2026-10-05, exact F31.PT. Inner trailing panels and their closures
morph to the original flap down endpoint while retaining both thick front
edges at Y=-17,Z=-5/-6. Add -0.30-radian pitch and corrected opposed
0.20-radian roll about each mean front at Z=-5.5, retaining those front points.
Outer trailing skins now include both previously omitted left upper panels.
Use own outward axes from [±20,-17,-5.5] to [±31,-17,-6], with
-side*0.30*elevator-0.20*aileron radians. Flap-only demand leaves outer panels
static, as the bounded original branches do. This replaces the old whole-wing
flap coupling; independent control clearances remain explicit.

Canards retain +0.35-radian pitch at Y=54,Z=1. Rudder positions morph to their
own exact signed source endpoints, retaining the diagonal front from
[0,-35,8] to [0,-39,19]. Continuous travel and mixing remain fitted.

Complete main wheel/strut assemblies fold forward 130 degrees about [±4,-12,-5]
along [1,±0.55,0]. Nose wheel/strut folds forward 130 degrees around [0,33,-5].
Keep all painted wheel dimensions and deployed positions exact; hide only at
zero. The separate brace fixes painted upper center [0,23.5,-5], while lower
center [0,32.5,-9.5] follows the nose through a noninverting telescoping fit
with constant perpendicular thickness. Main untextured panels retain upper
edges X=0/1,Z=-5, closing +90/-90 degrees around Y. Nose door retains X=-2,Z=-5
and closes inward -90 degrees around Y.

Side brakes retain their own forward edges at X=±6,Y=-12,Z=-2..4, closing
-side*1.05 radians around Z. This corrects the old outward closing direction.
Keep the [existing prototype vector law](additional-aircraft.md#aircraft-and-flight):
actual auxiliary pitch/yaw rates, normalized by pi/2 radians per second and
clamped to +/-1, drive the source paddles and plume through +/-15 degrees.
No powered-lift lever alias is introduced. Full-exhaust combined geometry must
keep the plume root [0,-41,0], complete cross rigidity, paddle attachments and
fixed nozzle. The fitted plume-front envelope remains inside the projected
source nozzle aperture and within 3 source units of its plane at Y=-40.

## MiG-23 attachment and material corrections

Agent fits, 2026-10-05. Rudder positions interpolate to their exact own signed
source endpoints; keep the diagonal front [0,-24,5] to [0,-27,13] and fixed fin
unchanged. At zero demand use exact neutral UVs. At nonzero demand select the
original signed endpoint UVs directly. The positive endpoint reverses the front
UV order, so linear interpolation would collapse the texture halfway through.
This sign-based material switch is an authored timing choice.

Retain -0.30*elevator-side*0.10*aileron radians on the tails, now keeping inner
points abs(X)<=4 fixed about lateral pivots [±3.5,-26,1]. Corrected positive roll
lowers left and raises right. No separate wing aileron is invented. Flaps morph
positions and UVs to exact down endpoints before wing sweep. Retain actual
wing pivots [±9,4,3] and the existing fitted 0..40-degree sweep over 400..700
knots, multiplied by 1-flaps. The wing and its flaps must remain attached across
combined demands. Original sweep timing remains unknown.

Main wheel/shaft cards fold aft 90 degrees around [±3,-1,-3], retaining source
X coordinates and the entire front top line. Nose cards retain their aft
90-degree fold around [0,27,-3]. The separate brace pins both body endpoints
at Y=29 while its sqrt(5)-unit distal edge follows the nose rigidly. Hide all
eighteen gear faces only at zero; full deployment is exact.

Fitted side brakes retain the 0.60-radian demand and Y=-24..-16 bands, now
pinning every actual beveled forecut. Distal motion uses left midpoint
[-3.909091,-16,0.227273] along [0,0,2.272727] and right
[3.681818,-16,0.227273] along [-0.454545,0,2.272727], opening outward with
opposed signs. Source surrounding skins remain fixed. Flame demand retains
root Y=-29 and the aircraft's existing afterburner capability.

## F-14D attachment corrections

Agent fits, 2026-10-05, exact F14.PT at four-thirds foot per source unit.
Preserve the [existing compatibility repairs](additional-aircraft.md#fitted-exterior-behavior):
outlet/collar mirrors, seam triangles, right-tail front correction, left wing
alignment and lift, sweep/vapor mapping, plume width and donated hook artwork.
Those remain explicit repaired geometry, not unchanged original neutral art.

Replace the older 0.4-radian flap droop with interpolation to each original
negative branch. Only the inboard trailing point lowers from Z=1 to 0; fronts
and outboard trailing points remain fixed. Apply the existing wing alignment,
lift and sweep afterward. Paired skins share targets and preserve original UVs.
This deliberately reduces outer travel relative to the prior fit; source
endpoints are known but continuous timing remains authored. Tail pitch remains
-0.30 radians, with corrected -side*0.20-radian roll around Y=-10,Z=0. Retain
the existing 0.35-radian fin split from [side*4,-11,2] to [side*4,-13,9].

Complete main wheel/strut assemblies fold 75 degrees about [side*6,1,0] along
[1,side,0]. Nose wheel/strut folds forward 110 degrees around [0,17,-1].
Preserve wheel dimensions and deployment positions. The separate brace pins
[0,12,-1] while its lower center [0,16,-2.5] follows the wheel through a
noninverting telescoping centerline fit, with constant transverse thickness.
Its original top edge is collapsed to a point; do not invent a rectangle there.
The separate panel pins upper edge [0,17,-1] to [0,21,-1], closing 90 degrees
about Y. Preserve nosewheel steering and keep its brace connection attached.
Hide gear only at zero after the fitted stow.

Brakes retain diagonal hinges [side*2,-11,1] to [0,-12,2], now closing 75
degrees around [1,side*0.5,-side*0.5]. This replaces the earlier 45-degree fit
which left a source corner exposed at stow. Deployed source geometry remains
exact. The donated hook retains its 0.6-radian upward closure around [0,-5,-2];
flames retain repaired widths and length scaling from Y=-14. No flight forces,
control bindings or compatibility adapter selection change.
