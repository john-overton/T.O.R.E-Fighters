# Stores on pylons

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification, 2026-10-12 (slice BP, bug B3). John reported that
bombs and missiles left from one fixed point ahead and to the right of the
view, and asked for them to drop from their hardpoints, with the pylons and
loaded stores drawn on the aircraft ("I'd love positions and drawn pylons").
Slice PY2 (2026-10-13) made every wing pylon 2 ft deep, put fuselage and
centreline stores on 4 inch rails and wingtip missiles on the tip itself, hung fuselage pairs one
behind another and moved the A-4E's Mk 82s to a centreline rack (John's
morning list, items 25 to 27). Slice PY3 (2026-10-13 evening, John's evening
list, items 7 and 9 and a follow-up) halved the pylons (1 ft wing pylons,
2 inch rails), replaced the deeper pylons beside nacelles with a sideways
move of the station, hung two stores on a pylon off the sides of a small
bar, rolled the weapon shapes so their fins stand in an X, and gave the
A-4E the real aircraft's three tanks. Slice F14 (2026-10-14, John's
annotated picture of the F-14 from below) moved the F-14's stores to the
real aircraft's stations: tanks under the intake nacelles, the Phoenixes
closer together in the tunnel with the front pair back where the tanks
hung, and the AIM-120 under the glove pylon beside the AIM-9 on its
shoulder rail ([the F-14](#the-f-14)).

## Even paired loads

**Opinionated, requested by John on 2026-10-25 (issue #6), expanded by his
review follow-up.** Every point that carries stores on both sides of the
aircraft loads complete pairs. This includes wings, wingtips, shoulder rails,
fuselage pairs, tunnel pairs and paired internal bays, even while bay doors
hide their stores. Use the resolved carrier's pairing and nonzero lateral
position, the same `Rack::two_sided` predicate as drawing and release.
Centreline and genuinely single-sided points keep their counts. Original
PT/JT records are not rewritten.

After existing compatibility and weight arithmetic, a positive odd maximum
above 1 rounds down (3 to 2, 5 to 4); a maximum of 1 becomes 2. Zero stays
zero. Quantity editing, including clicking a loaded station, steps by 2.
Moving between a pair and a single point also moves 2, or nothing if either
end cannot supply or accept a pair. The same conversion builds human and
AI default loads; AI tactics do not change. Existing singleton designator
and gun-pod equipment paths keep their separate rules.

A maximum of one becoming two is an explicit minimum-pair exception to the
old row weight class. Both stores' full weight, including tank fuel, counts
toward aircraft takeoff weight. This can require removing other stores or
fuel. All other odd maxima round down after the weight calculation.

**Saved preflight loads:** a saved single on a paired point becomes a pair
(1 to 2), as John requested. Larger odd quantities round down (3 to 2).
Clamp to the current maximum; zero stays zero. Layout maxima use the same
rule. A saved single is not silently removed to avoid overweight.

Quick Mission restores a structurally valid migrated load into the editor
even if overweight. The existing weight display shows the excess. Fly refuses
with `This plane is too heavy to take off. Remove stores or fuel.` until the
player changes stores or fuel. Invalid records still fall back to the normal
load. Non-interactive single-player mission construction also applies the
migration, then refuses an overweight start with that same error through
`Combat::with_loadout`. It does not drop the pair, reduce fuel, substitute a
default load or begin simulation. Network requests must already obey current
counts and aircraft weight limits; old odd requests are refused. An odd
count on a two-sided point is refused with `Paired stores must be loaded in
pairs.` (or `Paired tanks ...`); any other layout or quantity mismatch is
reported without attributing it to pairing.

In-flight ammunition, checkpoints and recorded store layouts are not
migrated: firing one store legitimately leaves an odd count. Each even load
starts with equal side counts and mass at mirrored positions. The existing
release rule remains left, right, left, right for the selected point, so the
heavier remaining side of that point fires next after its first shot.
Jettison removes actual remaining stores. The flight adapter still uses total
payload mass and total load-dependent drag; no lateral store-CG or per-side
drag torque model is introduced.

Retail has aggregate source-row capacities and an alternating per-row release
offset. John confirms it did not draw these pylons and loaded stores.
T.O.R.E's old right-extra layout independently applied to every row caused
the visible pile-up. Even preflight loads are a deliberate departure from
retail quantity editing, not a correction to the original records.
[Source-row research](../formats/aircraft-ordnance.md#paired-point-counts-and-physics)
owns that evidence and its limitations. The
[roster validation](../baselines/even-paired-loads.md) lists every changed
maximum and reports the default-load audit.

## What the player sees

- **Every store leaves from where it hangs.** A missile, bomb or rocket
  appears exactly where it was drawn a moment before, in the aircraft's
  frame: under its own pylon or rail, at its place on a rack, wingtip
  missiles on the tip, internal bay stores below the bay. Nothing jumps at
  release.
- **Wing pylons are 1 ft deep.** A wing store hangs with its top 1 ft
  under the wing above it.
- **Fuselage and centreline stores sit on short rails**, their tops about
  2 inches under the body, as launchers and conformal stations are.
- **A store beside a nacelle or a body's flank moves aside** (up to 2 ft)
  rather than hanging deeper, so no pylon is deeper than those above.
- **Two stores on one pylon hang beside it**, one each side of a small bar
  at the pylon's foot, as a wingtip missile sits on the tip, not under a
  wide beam.
- **Fins stand in an X.** Every weapon is turned about its long axis so its
  fins point 45 degrees away from the pylon, the wing, the body and the store
  beside it, and it keeps that turn after release.
- **Wingtip missiles sit on the tip.** No pylon or rail is drawn under the
  tip: the missile's centre line lies on the tip's edge, as an F-16's or
  F/A-18's tip AIM-9 does.
- **The F-14 carries its stores at its real stations**: its tanks under
  the intake nacelles, four Phoenixes close together in the tunnel between
  them (two ahead, two behind), and on each glove pylon an AIM-120 (or
  AIM-7 or AIM-54) under it with the AIM-9 on its shoulder rail beside it
  ([the F-14](#the-f-14)).
- **Fuselage pairs hang one behind another**, as the F-14's Phoenixes and
  the F-15's and F-4's Sparrows do, and the front pair leaves first.
- **Paired stations alternate.** Two AIM-9s on the wingtips leave left, then
  right; four Mavericks on a wing pair leave left, right, left, right. The
  player, the AI and every seat of a network game release the same way, so a
  remote player's missile appears at that player's pylon.
- **Pylons and loaded stores are drawn** on every aircraft that carries a
  combat loadout, in outside views and the flown aircraft's own views of
  itself, and they empty as stores leave. A pylon stays when its stores are
  gone. In the cockpit view the stores hide with the rest of the flown
  aircraft's model.
- The bomb's fall, sag and drop after release (the weapons pass behaviour) are
  unchanged; only the point they start from moves.

## Rules

| Rule | Value | Provenance |
| --- | --- | --- |
| Station position | The source hardpoint record's x (right) and z (forward), a third of a foot a unit, from the aircraft's reference point ([table](../formats/aircraft-ordnance.md#hardpoint-positions)); its height is the pylon's (below) | spec-derived (the height: opinionated, PY2) |
| Pylon depth | A wing store's top (its lugs, the top of its body) hangs 1 ft under the model's underside over its pylon; a fuselage or centreline store's 2 inches (0.17 ft) under it, on a launcher rail. Stores beside a rack's bar keep the same depth; a triple rack's third hangs under the bar. The underside is the lowest face of the clean model up to 12 ft above the record, straight above the pylon or, where nothing is straight above, up to 3 ft inboard. A station with nothing of the model above it keeps the record's height | opinionated, John 2026-10-13 (morning: 2 ft wing pylons and 4 inch rails; evening: "pylons need to be about half the size they are now"); the samples are agent decisions |
| Moved aside | A station whose store would cut through a part of the model beside it (a face within 0.6 ft either side of the pylon reaching more than 0.25 ft below the store's top: a nacelle, a gear pod, a body's flank) moves sideways in 0.25 ft steps, out before in at each distance, up to 2 ft, to the first place it clears with model straight above. A store that cannot clear within 2 ft hangs under the lowest surface across its body, as PY2 hung every store (no default load needs it). The move is part of the station's position, so it releases from there too. On the default loads: F/A-18D fuselage AIM-120s 0.25 ft in (3.08 ft out), Rafale fuselage Mavericks 0.25 ft in (3.08), AV-8 fuselage Mavericks 0.25 ft out (3.25), Su-25 B-13 pods 0.5 ft out (5.5), (the A-10's rows are placed by hand, [the A-10](#the-a-10)). The F-14's stations are placed by hand instead ([the F-14](#the-f-14)) | agent decision, 2026-10-13 (John: short pylons; a store inside the launching aircraft's hit box is not a reason to hang it deeper, slice SD) |
| Under the chord | A wing station with nothing of the model straight above it (nor up to 3 ft inboard) slides forward or aft, up to 12 ft, to under the wing's chord at its distance out, nearest the record: 2.4 ft behind the leading edge, so a launch rail about as long as an AA-11's pylon (4.3 ft) starts at the leading edge and the missile's nose reaches ahead of it, as on real underwing launchers; on a chord shorter than 4.8 ft, at its middle (John 2026-10-14: "center the rail to the leading edge"). A safety net: no default load of the 22 aircraft the stores preview draws needs it. It was first written for the Yak-141's outer row (retail records it 16.67 ft out at 0 ft forward, ahead of the swept wing, so its AA-11s hung in space), which now has its own stations ([the Yak-141](#the-yak-141)); it stays for any other row recorded off the wing | agent decision, 2026-10-14 (fitted) |
| Real stations (A-10) | The A-10's rows 6 and 7 hang on the wing at 10.75 and 14.75 ft out, not under the main gear pods ([the A-10](#the-a-10)) | opinionated, John 2026-10-14; the spans are read from his marks |
| Real stations (Su-27, Su-35) | The Flankers' tunnel, intake and wing rows hang at the real aircraft's stations, one missile a pylon, from the per-aircraft table ([the Su-27 and Su-35](#the-su-27-and-su-35)) | opinionated, John 2026-10-14 ("make them look realistic"); the positions are agent decisions read from the models |
| Real stations (Yak-141) | The Yak-141's two wing rows hang at its inner and outer underwing pylons, from the per-aircraft table, not the records' positions ([the Yak-141](#the-yak-141)) | opinionated, John 2026-10-14; the positions are agent decisions read from the model |
| Real stations (F-14) | The F-14's four weapon and tank records hang at the real aircraft's stations, from a per-aircraft table, not the records' positions: no move aside there ([the F-14](#the-f-14)) | opinionated, John 2026-10-14; the positions are agent decisions read from the model |
| Inboard rail | The Tornado's AIM-9 rail hangs on the swivelling pylon's inboard flank (facing the fuselage), 1.25 ft inboard of its centre line, drawn on a 3.0 ft arm; otherwise as the shoulder rail ([the Tornado](#the-tornado-aim-9-rail-on-the-inboard-flank-shorter-and-the-sweep-from-350-mph-tor-11-2026-11-12)) | opinionated, John 2026-11-12; the 3.0 ft is fitted |
| Shoulder rail | A store on a pylon's shoulder rail (the F-14's AIM-9) hangs 1.25 ft outboard of that pylon's centre line, its body 0.05 ft above the pylon's foot (the lugs, the top, of whatever store hangs under the pylon), and leaves from there: the rail juts off the pylon's side just above the main store, so the shoulder store's body is above the whole body of any store under the pylon, real loads or not (AIM-120, AIM-7, AIM-54, bombs, tanks, GBU-10). 1.25 ft keeps an AIM-9's fins (2.07 ft across at 45 degrees) clear of the pylon and of the fins of an AIM-120, AIM-7 or AIM-54 under it | opinionated, John 2026-10-14 ("the AIM-9 rail in real life is a jut off the main pylon slightly elevated above the stores on the main wing pylon"); the spacing and the 0.05 ft gap are agent decisions (fitted) |
| Wingtip | A wingtip store's centre line lies on the tip's edge: as far out as the model's outermost point beside the record (within 4 ft inboard and 0.3 ft outboard of it, 2.5 ft fore and aft, 3 ft up or down), at the middle height of that edge. No pylon or rail is drawn. With no model read, the record's position | opinionated, John 2026-10-13 ("the missile's centre line sits on the wingtip geometry, no gap"); the search box is an agent decision |
| Where the underside is read | When the loadout is built, from the aircraft shape the simulation already reads (`<ID>.SH`, at the debris scale): over the record, over a two-pylon wing's inboard and outboard pylons (1.4 ft either side), and 7 ft ahead of and behind the record; a tandem pylon's surface is interpolated between the record and the sample on its side. The heights travel with the station (`Rack::over`), into checkpoints and replays | agent decision |
| Released from | The point the store hangs at: its side, its pylon, its place on the pylon's rack and the pylon's height (`Rack::store_point`), for the player, the AI, every network seat and replays. The bomb sight predicts from the same point | opinionated, John 2026-10-13 ("a store never jumps at release") |
| Store size | Length, the height of its lugs above its origin (half its body diameter) and its width across its fins, from the [real sizes](#store-sizes) table by shape name; a pod's or tank's fitted body; 8 by 0.9 ft for a store not listed. The origin is taken as the store's middle | agent decision |
| Paired record | Hardpoint flag 0x4: the record's position is the right pylon, its mirror the left | spec-derived ([release state](../formats/weapons.md#release-state)) |
| Also paired | Every Wing and Wingtip record without flag 0x4 (the Mi-24's AT-2 rails) | agent decision, 2026-10-12 |
| A one-sided wing row by the centreline | A Wing record without flag 0x4 less than 3 ft out (the A-4E's Mk 82 row, 2.7 ft out, which retail releases from one side) is a centreline rack at x 0 ([A-4E](#the-a-4e)) | agent decision, 2026-10-13 |
| Fuselage pairs one behind another | A Fuselage record whose side holds two stores, or four or more, has two pylons on that side, one ahead of the other: their stores' middles are half a store length and 0.5 ft either side of the record's position, so they clear by 1 ft. The front pylon's store leaves first, so four leave left front, right front, left rear, right rear. Four or more a side: two side by side on each pylon's rack | opinionated, John 2026-10-13 ("fuselage missile pairs one behind another: fix"); the gap and the front-first order are agent decisions (the real aircraft's firing order is not published; front first keeps a rear missile from passing under a loaded front one) |
| Fuselage pair on the centreline | A paired Fuselage weapon record at x 0 holding two or more (the F-4E's and F-4G's four Sparrows) hangs 3 ft either side of the centreline, in the Sparrow wells | agent decision, 2026-10-13 (fitted) |
| Never paired | An internal gun, whatever its flags; a record on the centreline (nothing to mirror) | agent decision |
| Which side | The mirror (left) when the whole releases left on the station after this one (rounds over `actualRoundsPerGame`) is odd. Even wing loads start symmetric and release left then right; a partially spent point can hold one extra on the right | fitted release order based on the documented retail per-row alternation; no claim of a retail drawn-store split |
| Unlimited ammunition cheat | The count never moves, so the side follows the release's projectile number (alternates) | agent decision |
| Who | The player, the AI (strike stores, missiles and seeker missiles alike) and every network seat. Retail releases a distant AI aircraft from its centre; T.O.R.E releases every aircraft from its pylon | agent decision |
| Guns | Keep their muzzle (gun mount, AC-130 barrel) | unchanged |
| Bomb packs | Each bomb of a pack keeps its pack stagger, added to the pylon's side | unchanged |
| Twin rails on a wing | A Wing record whose side holds two stores (a row of four on a paired record) hangs them on one pylon at the record's position, on a twin-rail launcher: one each side of its bar ([drawing](#drawing-opinionated-john-2026-10-12)), as the real F/A-18's wing pylons carry two missiles each. The racks are mirrored left and right, so on both wings the outboard store leaves first: four Mavericks leave left outboard, right outboard, left inboard, right inboard. A side moved aside for a part beside it is checked 1.5 ft either side of the pylon (two stores side by side) rather than 0.6 ft. Four or more on a side (beyond retail's maxima) hang on two pylons 1.4 ft either side of the record, each with its rack. Three on a side hang on one pylon's triple ejector rack, and Fuselage, Centerline and Wingtip records keep one pylon a side | opinionated, John 2026-10-14 ("the F-18s need the dual pylons for the wings that take 4 weapons at a station", with a picture of a real F/A-18; on 2026-10-12, "four stores on a wing row: dual pylons on the wing", BP2 had read two pylons a wing); the 1.5 ft and the rule for other counts are agent decisions |
| Rockets from a two-pylon side | Leave from the pod that is firing (the outboard pod first) | agent decision |

**The bomb sight aims from the pylon the next store leaves** (John,
2026-10-12, slice BP3). The CCIP pipper, the level T and the rocket pipper
predict the release from the pylon of the selected station the next store
will leave, by the same rule the release uses (`Station::release_point`):
the side from the whole releases left on the station once that store has
gone, odd for the mirror (left), even for the record's own position (right),
and on a twin rail the side of the bar that store hangs from. So with four
Mk 82s on a wing row the pipper shows the left outboard bomb's fall, then
the right outboard, then the inboard ones. With unlimited ammunition the release's projectile number picks the
side, as the release does. The AI's release gate reads the same solution from
the pylon its next store leaves, so an AI bomb is judged from where it
drops (agent decision: the AI's next side is worked out from its weapon
service's rounds before the debit, which is what the release then debits).

| Sight | Predicts from | Provenance |
| --- | --- | --- |
| CCIP (bombs) | The next store's pylon, pack offset 0 | opinionated, John 2026-10-12 |
| Level T (bombs) | The CCIP's release point, so the same pylon | opinionated, John 2026-10-12 |
| Rocket pipper | The next rocket's pod | opinionated, John 2026-10-12 |
| AI release gate | The AI's next store's pylon | agent decision |
| Guided weapon sights, gun sights | Unchanged (the aircraft, the gun's muzzle) | unchanged |

## Drawing (opinionated, John 2026-10-12)

In a replay, carried weapons use their own shapes even if that weapon type
was never fired. The [replay stores contract](../REPLAYS.md#stores) describes
shape loading and how old recordings retain their station placement.

Retail draws no stores on aircraft (the A2G pass recorded this as decision 20,
"stores on pylons stay undrawn"); John reversed that decision on 2026-10-12.
Every number below is fitted (agent decision).

| Item | Rule |
| --- | --- |
| Stations drawn | External ones (Centerline, Fuselage, Wing, Wingtip) that held something at mission start; not guns or internal bays |
| Store look | The weapon's own shape (the same mesh it flies with after release) refitted to the real store's size ([store sizes](#store-sizes)). Rocket pods: a grey-green eight-sided pod of the real pod's size (LAU-61 5.5 by 1.38 ft, LAU-10 8.3 by 1.25, B-8M1 8.9 by 1.71, B-13L 11.7 by 1.35; any other 8.5 by 1.4). Fuel tanks: a light grey pod 13 ft by 2 ft at 2,000 lb of fuel, scaled by the cube root of its fuel (a 330 US gallon tank is 13.3 by 2.05 ft; the real one is about 12.9 by 2.1). A store with no shape: a plain body 8 ft by 0.9 ft |
| Stores on one pylon | One at the pylon point. Two hang off the sides of a small bar at the pylon's foot, one each side, their centre lines level with the bar's middle and at its ends, as a wingtip missile sits on the tip (John 2026-10-13: "the panel should be a small bar on the pylon and ordnance hangs to the side"): out from the pylon's middle by the pylon's half thickness (0.14 ft), a 0.1 ft gap and the store's body radius, or 0.36 of its width across the fins if more, so two sets of fins at 45 degrees clear each other (AIM-9 0.75 ft, Mk 82 0.69 ft, Maverick 0.85 ft). Three as a triple ejector rack: one each side of the bar and the third under it. Four as two pairs one behind another (a multiple ejector rack), the front pair first. More: triple racks one behind another, 1.05 store lengths apart. The layout keeps the loaded count's slots; the highest slot empties first. The simulation places them ([released from](#rules)) and the drawing hangs them there |
| Pylon | A grey slab 0.28 ft thick, from the top of the store's body (its lugs, not its fin tips) or a rack's bar up into the model's underside (1 ft, see [pylon depth](#pylon-depth)), 0.02 ft into it (retail wings are single faces, so more would show on top). Its length is 0.45 store lengths, at least twice its depth and at most 0.7 store lengths (1.2 to 8 ft), cut short where the underside ends (a pylon does not run out ahead of a leading edge), and its top follows the underside from end to end. The underside is the lowest face of the clean model up to 12 ft above the lugs, straight above or up to 3 ft inboard, preferring a nearby short attachment over a distant overhead surface ([Apache correction](#apache-stub-wing-pylon-attachment)) |
| Rack | A bar 0.25 ft deep across the pylon's foot, reaching out to the centre lines of the stores beside it, 0.7 of the pylon's length (plus the racks one behind another) |
| Fin roll | Every weapon shape is rolled about its body's centre line so its fins stand in an X, 45 degrees from straight out and straight up and down, so no fin points into the pylon, the wing or body above, or the store beside it (John 2026-10-13: "ordnance needs to be rotated 45 degrees to make sure fins don't clip and are perpendicular to pylons and the fuselage as best as possible"). The roll is measured per retail shape, not assumed: the fin tips' clock about the body (the corners more than 1.6 body half widths out, their angles averaged over a quarter turn). A "+" shape (clock 0) turns 45 degrees; a shape already in an X (clock about 45) is not turned; a shape whose tips agree on no clock (agreement under 0.5 of 1) is not turned. One roll serves every mount (under a pylon, beside a bar, on a wingtip, on a rail), since an X is the same seen from below or from the side. The rolled shape is the one drawn everywhere the store is: on the pylon, in flight after release (so it does not turn at release), for the player, AI aircraft, network peers, replays and previews. See [fin roll by shape](#fin-roll-by-shape). Opinionated, John 2026-10-13; the measuring rule and one roll per shape are agent decisions |
| Wingtip | Nothing drawn under a wingtip store ([rules](#rules)) |
| Shoulder rail | The pylon the store rides beside, from its foot into the surface above (drawn again for the shoulder store, so it stands when the station under it was left empty), and an arm 0.15 ft deep jutting off the pylon's side, above the main store, out along the shoulder store's top to its centre line, 0.6 store lengths long (3 to 6 ft), drawn as pylon structure. The F-15C's twin shoulder rails on one pylon (two Sidewinders, 1.75 ft either side of it) use the same arm, 3.5 ft wide across the pylon and 3 to 6 ft long, over both missiles. The 2026-10-20 attempts to narrow it to the F/A-18 bar's size, and then to half the width, were undone the same day: John moved the whole pylon inboard instead, below the paired pylon as on the real F-15 |
| Edge rail | A wing station's store with nothing of the model above it, beside a wing's edge less than 4 ft inboard, hangs on a rail from 0.3 ft inside the tip's edge out over the store, as deep as both the tip's edge and the store's upper half, 0.6 store lengths long (3 to 9 ft). With no tip beside it, a 0.35 ft rail |
| Colours | Pylons and racks grey (0.5, 0.52, 0.54), tanks light grey, pods grey-green; weapon shapes keep their palette colours |
| Lighting | Exactly the airframe's: every pylon, rack, fitted body and weapon face carries a palette entry (a fitted colour takes the aircraft palette's nearest entry) with its fog mode, so the weather's time-of-day palette darkens it at night and at dusk, fog and haze fall on it, and the same sun, moon and ambient light it, as they do the untextured faces of the aircraft it hangs on; a released weapon in flight is lit the same way. Before 2026-10-13 they carried fixed colours and showed bright at night (John's report, an A-10 at night). The stepped compatibility lighting (`TORE_WEATHER_SMOOTH=0`), which picks a light row per airframe face, is not applied to stores | opinionated, John 2026-10-13; the nearest-entry rule is an agent decision |

Where the stores come from:

- **Single player, the host, AI aircraft:** the render snapshot carries each
  aircraft's stations and their stores left ([`AircraftPose::stores`]).
- **Network clients (protocol 27, slice BP3, John 2026-10-12):** a client
  draws every aircraft's pylons and stores from its own copy of the
  aircraft's loadout (the session's copy of the mission, which every game
  builds the same), as the loadout built it until the host says otherwise.
  The host sends a **Stores** event for an aircraft when what it draws
  changes (a release that empties a whole store, a jettison, a rearm): the
  aircraft's drawn stations with the whole stores still on each. A player
  seated later, a resumed seat after a migration and an observer are told
  every aircraft not as built when they start. The client puts each told
  state at its tick, so a station empties when the picture reaches the
  release, the same moment the store appears as a weapon. The seat's own
  aircraft is still drawn from its cockpit readout, which is newer. A
  Stores event is about 14 bytes for a fighter's six stations and costs
  nothing while nothing changes ([the protocol](../formats/net-protocol.md#protocol-27-stores)).
- **Mission replays (slice BP3, John 2026-10-12):** a recording keeps each
  aircraft's drawn stations the first time it is recorded and their whole
  stores whenever one empties, as `stores.*` events; the viewer draws each
  aircraft's stores at the tick it shows, forward, backward and after a
  seek. A converted network capture and an observer's live recording keep
  them too. A recording made before them has none, so its aircraft carry no
  stores ([replays](../REPLAYS.md#stores)).

## Store sizes

John, 2026-10-12: "are the weapon renders from retail accurate? They seem big
and bulky. Check weapon renders to make sure they are realistic sizes."
Measured with `tore-app --stores-sizes` (every weapon shape and every
aircraft, as drawn) and `--stores-size-sheet` (silhouettes, retail under
refitted, on a one-foot grid):

- **The aircraft are drawn at their real size**, within about 10 percent:
  F/A-18D 56.0 ft long (real 56.1) and 36.3 ft across the tips (37.5), F-16C
  49.0 by 34.0 (49.4 by 32.7 with tip missiles), F-14 60.0 (62.75), A-10 48.7
  by 54.0 (53.3 by 57.5), Su-25 50.0 (50.9), MiG-29 56.0 (56.8), AV-8 50.0
  (46.3), A-4E 40.0 (40.1).
- **The retail weapon shapes are about the right length but 1.3 to 2 times
  too thick.** Shape coordinates are whole source units of a third of a foot,
  so the thinnest body a shape can draw is 0.67 ft across; an AIM-9 is
  0.42 ft. Bodies and fins are drawn larger still: a Mk 82's body 1.33 ft
  (real 0.9) with 2.0 ft fins (1.27), a Maverick's 1.33 ft (1.0) with 3.3 ft
  wings (2.36), an AIM-54's 2.0 ft (1.25). Lengths are within 10 percent,
  except the dispensers: CBU-87 and CBU-97 drawn 11 and 13 ft (real 7.7),
  Mk 20 10.7 ft (7.7), RBK-250 and RBK-500 11 ft (6.4 and 8.2); the FAB-250
  is drawn short (5.0 ft, real 6.4). There is no unit mix-up: missiles, bombs
  and aircraft all use the same third of a foot, and no shape header carries a
  different scale exponent.
- **The fitted pylon bodies were too long or too fat** for some pods (a
  LAU-61 drawn 8.5 ft, real 5.5).

**Refit (opinionated, John 2026-10-12; rule an agent decision).** Every
listed shape is refitted, on the pylon and in flight, so a store does not
change size when it leaves (the in-flight shapes are wrong by the same
measure). Each axis is mapped separately: the length scales about the
store's origin (its release point) to the real length; across, the body
(the second thinnest of nine cross-sections between a quarter and three
quarters of the length) scales to the real diameter and the fin tips to the
real span, linearly between, so a thin fin stays thin. A shape several
weapons share takes the size of the store it depicts. Unlisted shapes
(surface-launched missiles, the AT-12, the AAM-L) keep their retail size.

| Shape | Weapons | Length ft | Body ft | Span ft | Source |
| --- | --- | ---: | ---: | ---: | --- |
| `SIDEM.SH` | AIM-9M | 9.35 | 0.42 | 2.07 | US Navy fact file; AIM-9L/M 2.85 m, 127 mm, 0.63 m |
| `SIDEB.SH` | AIM-9B | 9.3 | 0.42 | 1.83 | AIM-9B 2.83 m, 127 mm, 0.56 m |
| `AM9X.SH` | AIM-9X | 9.9 | 0.42 | 1.47 | Raytheon; 3.02 m, 127 mm, 0.45 m |
| `AMRAM.SH` | AIM-120 | 12.0 | 0.58 | 1.75 | US Air Force fact sheet; 3.66 m, 178 mm, AIM-120A/B 0.53 m |
| `SM2.SH` | the fitted derived SM-2 (private alias of the AMRAAM shape) | 15.5 | 1.125 | 3.5 | Fitted, John 2026-11-06: RIM-66 Standard MR 4.72 m, 13.5 in, 1.08 m fin span; the AMRAAM shape keeps its own AIM-120 size |
| `SPAR.SH` | AIM-7, AIM-7E (and the SS-N-9 and Sea Sparrow records) | 12.0 | 0.67 | 3.3 | US Navy; 3.66 m, 203 mm, 1.0 m |
| `PHENX.SH` | AIM-54 | 13.0 | 1.25 | 3.0 | US Navy; 3.96 m, 380 mm, 0.91 m |
| `MICA.SH` | MICA | 10.2 | 0.53 | 1.83 | MBDA; 3.1 m, 160 mm, 0.56 m |
| `M55.SH` | R-550 Magic | 9.0 | 0.52 | 2.17 | Matra; 2.75 m, 157 mm, 0.66 m |
| `M53.SH` | Super 530D | 12.5 | 0.86 | 2.03 | Matra; 3.8 m, 263 mm, 0.62 m |
| `APHID.SH` | AA-8 (R-60) | 6.9 | 0.39 | 1.28 | Vympel; 2.09 m, 120 mm, 0.39 m |
| `ARCHR.SH`, `AA11B.SH` | AA-11 (R-73) | 9.5 | 0.56 | 1.67 | Vympel; 2.9 m, 170 mm, 0.51 m |
| `ALAMO.SH` | AA-10 (R-27) | 14.8 | 0.75 | 3.2 | Vympel R-27ET; 4.5 m, 230 mm, 0.97 m tail |
| `AMSKI.SH` | AA-12 (R-77) | 11.8 | 0.66 | 2.2 | Vympel; 3.6 m, 200 mm, 0.68 m grid fins |
| `ATOLL.SH` | AA-2 (K-13) | 9.2 | 0.42 | 1.73 | 2.8 m, 127 mm, 0.53 m |
| `ACRID.SH` | AA-6 (R-40) | 20.0 | 1.02 | 4.76 | about 6.1 m, 310 mm, 1.45 m |
| `AMOS.SH` | AA-9 (R-33) | 13.6 | 1.25 | 3.87 | Vympel; 4.15 m, 380 mm, 1.18 m |
| `STGR.SH` | FIM-92 | 5.0 | 0.23 | 0.3 | 1.52 m, 70 mm |
| `MAVRK.SH` | AGM-65A, AGM-65G | 8.2 | 1.0 | 2.36 | US Air Force fact sheet; 2.49 m, 305 mm, 0.72 m |
| `HARM.SH` | AGM-88 (and the AGM-45, 10 ft real, which retail draws with this shape) | 13.7 | 0.83 | 3.7 | US Navy; 4.17 m, 254 mm, 1.12 m |
| `HARPN.SH` | AGM-84A | 12.6 | 1.12 | 3.0 | US Navy; air-launched 3.84 m, 343 mm, 0.91 m |
| `SLAM.SH` | AGM-84E | 14.6 | 1.12 | 3.0 | US Navy; 4.45 m |
| `KERRY.SH` | AS-7 (Kh-23) | 11.6 | 0.9 | 2.58 | 3.53 m, 275 mm, 0.785 m |
| `KEDGE.SH` | AS-14 (Kh-29) | 12.8 | 1.25 | 3.6 | 3.9 m, 380 mm, 1.1 m |
| `KIKBK.SH` | AS-16 (Kh-15) | 15.7 | 1.49 | 3.0 | 4.78 m, 455 mm, 0.92 m |
| `AS12.SH` | AS.15TT | 7.1 | 0.62 | 1.85 | Aerospatiale; 2.16 m, 187 mm, 0.56 m |
| `AS30.SH` | AS.30L | 12.0 | 1.12 | 3.3 | 3.65 m, 342 mm, 1.0 m |
| `EXO.SH` | AM-39 Exocet | 15.4 | 1.15 | 3.6 | MBDA; 4.69 m, 348 mm, 1.1 m |
| `SWATR.SH` | AT-2 | 3.8 | 0.48 | 2.17 | 1.16 m, 148 mm, 0.66 m |
| `MK82.SH`, `MK82A.SH` | Mk 82, Mk 82 P3, Mk 82 AIR | 7.3 | 0.9 | 1.27 | US Navy; 2.22 m, 273 mm; fin span about 15 in (approximate) |
| `SNAKE.SH` | Mk 82 Snakeye | 7.4 | 0.9 | 1.27 carried, 4.0 released | as Mk 82; retail draws its retarding fins open, as they are after release; the open span is approximate |
| `MK84.SH` | Mk 84 | 10.8 | 1.5 | 2.1 | 3.28 m, 458 mm; fin span about 25 in (approximate) |
| `MK20.SH` | Mk 20 Rockeye | 7.7 | 1.1 | 1.45 | 2.34 m, 335 mm |
| `CB87.SH`, `CB89.SH` | CBU-87, CBU-97 | 7.7 | 1.3 | 1.9 | SUU-65 dispenser 2.34 m, 396 mm; fin span approximate |
| `PAVE.SH` | GBU-10 | 14.2 | 1.5 | 5.4 | 4.32 m, 1.65 m wings |
| `GB27.SH` | GBU-27 (and the GBU-29 and GBU-30 retail draws with it) | 13.9 | 1.2 | 5.4 | 4.24 m, BLU-109 370 mm |
| `GB28.SH` | GBU-28 | 19.2 | 1.21 | 2.2 | 5.84 m, 370 mm; fins approximate |
| `F250.SH`, `RBK2.SH` | FAB-250, RBK-250 | 6.4 | 1.07 | 1.35 | M-62 1.96 m, 325 mm |
| `F500.SH` | FAB-500 | 7.9 | 1.3 | 1.6 | M-62 2.4 m, 400 mm |
| `F1000.SH` | FAB-1000 | 9.4 | 2.07 | 2.6 | about 2.87 m, 630 mm |
| `RBK5.SH` | RBK-500 | 8.2 | 1.48 | 1.8 | 2.5 m, 450 mm |

Numbers are public data (manufacturers' and services' published figures, as
collected by Wikipedia and FAS Military Analysis Network), rounded; spans
marked approximate are estimates. Pod sizes are in the drawing table above.

## Fin roll by shape

Measured with `tore-app --stores-sizes` (the `fins` lines) on the retail
shapes:

| Fins | Shapes | Roll |
| --- | --- | --- |
| "+" (clock 0) | AIM-9 (`SIDEM`, `SIDEB`), AIM-120 (`AMRAM`), AIM-7 (`SPAR`), AIM-54 (`PHENX`), AA-2, AA-6, AA-8, AA-10, AA-11 (`ARCHR`), AA-9, Maverick (`MAVRK`), HARM, Harpoon, SLAM, AS-7, AS-14, AS-16, AT-2, Mk 82, Mk 84, GBU-10 (`PAVE`), FAB-250, FAB-1000, and the surface-launched missiles | 45 degrees |
| Already an X (clock 44 to 48) | AIM-9X (`AM9X`), MICA, R-550 (`M55`), AS.30L, AS.15TT (`AS12`), Exocet, CBU-87, CBU-97, Mk 20, GBU-27, GBU-28, RBK-250, RBK-500, Snakeye (fins drawn open in an X), FIM-92, AAM-L | none |
| No clear pattern | AA-11 (`AA11B`, 0.3), Super 530D (`M53`, 0.4), AA-12 (`AMSKI`, grid fins), the rocket shape; no fin tips found: FAB-500, Mk 82 AIR (`MK82A`) and a few surface shapes | none |

The unturned shapes with no clear pattern are the ones whose fins may still
touch a pylon or a neighbour; at the refitted real spans none showed a fin
through a pylon in the close-ups.

## Pylon depth

Before PY2 a store hung at its hardpoint record's height, so a pylon was as
deep as the gap from the record to the model: about 2 ft on the F/A-18D's
wing, 3.3 ft on the F-16C's and MiG-29's, 4 to 5 ft on the AV-8's and
Su-25's, and 7 ft under the F-14's body. Real pylons are 15 to 30 inches;
John asked for 2 ft everywhere, then the same day for short rails about
4 inches deep at fuselage and centreline stations and wingtip missiles on
the tip itself (2026-10-13). Every store now hangs that far under the
surface over it and leaves from there, so the release origins moved with
it: up by a few feet on most aircraft, by up to 4 ft under the F-14, and
wingtip missiles in to the tip's edge (1 ft on the F/A-18D and F-16C,
1.7 ft on the Rafale). In the evening John asked for pylons about half that
size (PY3): 1 ft on wings and 2 inches at fuselage and centreline rails, so
every store's release origin moved up again by 1 ft (wing) or 2 inches
(body), and more where PY2 had hung a store clear of a nacelle.

Measured with `tore-app --stores-preview` (the depth from each store's lugs
straight up to the drawn model, feet; default loads):

| Aircraft | Stations | Depth |
| --- | --- | --- |
| F/A-18D, F-16C, F-14, A-10, Su-25, MiG-29, AV-8, A-4E, F-15, F-4B, F-4E, F-4J, Rafale | Wing stations | 1.0 |
| F/A-18D, F-14, F-15, F-4B, F-4E, F-4J, AV-8, Su-25, Rafale | Fuselage stations (moved aside where noted in [the rules](#rules)) | 0.17 (0.02 to 0.15 where the body's underside slopes along the store) |
| F-4B, A-4E (Mk 82s loaded there) | Centreline pairs beside the bar | 0.17 |
| F/A-18D, F-16C, Rafale | Wingtip missiles | on the tip's edge (0.0 to 0.1) |

PY2's deeper pylons (Su-25 B-13 pods 3.2 ft, AV-8 fuselage Mavericks 3.7 ft,
F-14 front Phoenixes 3.4 ft) are gone: those stations moved aside instead
([moved aside](#rules)). The record's own fore and aft position is kept
everywhere, and its lateral position except the moves listed there and the
F-14's stations ([the F-14](#the-f-14)).

## The A-4E

Retail's default A-4E load is four Mk 82s on a one-sided wing row 2.7 ft
out, four 150-gallon tanks on the next wing row (5.7 ft out, paired) and two
LAU-61 pods on the outer wing row. The real A-4E has five stations: a
centreline one and two a wing, and carries its tanks on the centreline and
the inner wing pylons. With BP2's two pylons a wing, the four tanks (two a
wing) and the Mk 82s (paired, two a wing) crowded the same stretch of wing.

PY2 (agent decision, 2026-10-13) kept retail's quantities and moved the
Mk 82 row, which retail releases from one side next to the fuselage, to the
centreline: a rack of two pairs one behind another on a short rail under the
belly, the front pair first, right then left.

**Default load since PY3 (opinionated, John 2026-10-13: "A-4E: the real
three tanks (centreline + inner wings)").** A 150 gallon tank on the
centreline row and one on each inner wing pylon (5.7 ft out, one pylon a
wing), the LAU-61 pods on the outer row as before. External fuel is 2,970 lb
instead of retail's 3,960 (about 990 lb less, one tank). The centreline row
can carry a tank or the Mk 82s, not both, so the Mk 82s are no longer loaded
by default; they stay the row's weapon in the ordnance screen (four on the
centreline rack), as do the other stores each row accepts, and each row
keeps retail's capacity (the inner row can still take four tanks). Only what
starts loaded changes (`hardpoints::default_records`, applied by the live
configuration). The development driver `--a2g-run --load MK82` still loads
the four Mk 82s on the centreline row retail defaults them to, taking that
row's tank off.

## The F-14

John, 2026-10-14, on a picture of the F-14 from below: "default tank
pylons are actually under engine intake nacelles", "AIM-54s grouped closer
together and front missiles tucked backward where tanks are", "AIM-120
default pylon tucked under wing flaring next to AIM-9". Retail's records
put the tanks and all four Phoenixes 3 ft either side of the centreline,
the two tanks between the front and rear Phoenix pairs, and the AIM-120s
6 ft out, beside the nacelles rather than under the glove.

The real F-14's stations are the guide: 1A/1B and 8A/8B on the glove
pylons (an AIM-9 on the pylon's shoulder rail, an AIM-7, AIM-54 or AIM-120
under it), 2 and 7 under the intake nacelles (the tanks), 3 to 6 the
tunnel pallets between the nacelles (two ahead, two behind). T.O.R.E's F-14
model has its tunnel ceiling 2.7 ft under the reference out to 3 ft, the
nacelles from 3.5 to 6.5 ft out (their bottom ramps from 2.7 ft down at the
intake to 4 ft down from the reference point back), and the glove from
6.5 ft out about 0.6 ft up.

| Record (F14.PT row) | Retail position, ft out / forward | T.O.R.E station | How it hangs |
| --- | --- | --- | --- |
| 4, AIM-54 x4 (tunnel) | 3.0 / 8.0 (moved to 2.5 by PY3) | 1.25 / 1.5 | Tandem pairs on 2 inch rails, the tunnel pallets closed up (John 2026-10-14): 0.25 ft from the rear pair's noses to the front pair's tails (other aircraft's fuselage pairs keep 1 ft), front pair 1.6 to 14.6 ft forward (about where the tanks hung, its noses behind the intakes), rear pair -11.6 to 1.4 ft, its tails 0.6 ft past the tunnel's end (about -11 ft; was 2.5 ft past); 2.5 ft apart side by side, so the two pairs' fins clear by about 0.4 ft |
| 5, 250 gallon tanks x2 | 3.0 / 6.67 | 4.75 / 6.67 | Under the nacelles on 2 inch rails, under the lowest part of the intake ramp along the tank (6.5 ft either side of the record), so the tank does not cut into the ramp; the pylon is deeper at the front, where the ramp rises |
| 6, AIM-120 x2 (glove pylon) | 6.0 / 4.67 | 7.5 / 4.67 | 1 ft under the glove, just outboard of the nacelle |
| 7, AIM-9 x2 | 8.67 / 1.67 | 8.75 / 6.0 | On the glove pylon's shoulder rail ([rules](#rules)), its nose level with the AIM-120's |

Every load the ordnance screen allows on these rows hangs at the same
stations: the tunnel row's other stores (AIM-7s, bombs, AA-10s and the
rest) as tandem pairs or single stores 1.25 ft out, the glove row's under
the glove pylon, the AIM-9 row's (AIM-9B, AIM-9X, AA-2, AA-8, AA-11, R.550,
AT-12) on the shoulder rail. The release, the bomb sight and the AI fire
from those points, as everywhere ([released from](#rules)). Opinionated, John 2026-10-14;
the positions, the tandem gap and the shoulder rail's spacing are agent
decisions (fitted).

What still touches, measured with `tore-app --stores-preview` (its `clip`
lines: how far the deepest point of each drawn store reaches above the
model's lowest face straight below it), default load:

| Store | Before | After |
| --- | ---: | ---: |
| AIM-54 (tunnel) | 0.72 ft (into the nacelle beside it) | 0.27 ft (the tips of its upper fins into the tunnel ceiling) |
| AIM-120 (glove) | 0.21 ft (into the nacelle) | 0 |
| AIM-9 (shoulder) | 0 | 0.05 ft (since the rail sits above the main store: an upper tail fin tip into the glove) |
| Tanks | 0 (but inside the Phoenixes, same place) | 0 |

The Phoenix's fins are 3 ft across; turned to an X they reach 1.06 ft up
from its centre line, which on a 2 inch rail is 0.27 ft more than its body
radius and the rail together. Any finned missile on a 2 inch fuselage rail
does the same where its fins are wider than its body by more than the rail.
With bombs, tanks or a GBU-10 chosen for the glove pylon (beyond the real
F-14's loads) the shoulder AIM-9's body stays above them, though its lower
fins can touch a wide one. John accepts a little clipping (2026-10-14).

## The A-10

John, 2026-10-14, on a head-on picture: "the double pylons are attached to
the bottom of the gear/sensor things". Retail records row 6 (four
Mavericks, two a wing on a twin rail since nr-yak) 8.33 ft out, over the
main gear pod, so its pylon hung from the pod's underside, about 2 ft
below the other pylons. The real A-10's eleven stations are on the wings
and fuselage; the gear pods carry none (stations 3 and 9 are just outboard
of them, 2 and 10 between those and the outermost 1 and 11). From John's
marks:

| Record (A10.PT row) | Retail, ft out | T.O.R.E, ft out |
| --- | ---: | ---: |
| 5, twin rail inboard of the pod | 3.33 | unchanged (moved aside to 2.48) |
| 6, twin rail | 8.33 (moved aside to 7.83, under the pod) | 10.75, just outboard of the pod |
| 7, one a wing | 13.33 | 14.75 |
| 8, one a wing | 18.33 | unchanged |

Every pylon now hangs 1 ft under the wing; the `clip` lines read 0.
Opinionated, John 2026-10-14; the spans are agent decisions from his marks.

## The Su-27 and Su-35

John, 2026-10-14: the Su-27 and Su-35 should look realistic. The real
Flankers carry one missile a pylon: wingtip launchers, three underwing
pylons a wing (inner, middle, outer), a pylon under each intake duct and
two stations in the tunnel between the engines, one behind the other.
Retail's records (the same for both; SU35.PT is the canard Su-27M, the only
other Flanker in the game) put two AA-12s side by side on the centreline,
the next row 8 ft out beside the intake ducts, four wing missiles on one
row 17 ft out (two a wing, which nr-yak's rule hung on a twin rail), and
the wingtip rails.

| Record (row) | Retail position, ft out / forward | T.O.R.E stations | How |
| --- | --- | --- | --- |
| 5, centreline (Su-27 two AA-12s, Su-35 one AA-ML) | 0 / 4.0, side by side | The tunnel between the engines, centred 1 ft back | Two hang one behind the other on 2 inch rails, 0.25 ft apart (front 5 ft forward, rear 7 ft back); one hangs at the centre |
| 6, beside the intakes (AA-12s, AA-MLs) | 8.0 / 2.67 | Under each intake duct, 4.5 ft out, 2 ft forward | On 2 inch rails under the lowest part of the duct along the store, 6 ft either side (the Su-27's duct bottom slopes) |
| 7, wing (Su-27 four AA-11s, Su-35 four AA-12s) | 17.0 / -1.33 | The middle and outer underwing pylons, one missile each: Su-27 14 and 18 ft out, Su-35 13 and 17 ft out | A row with more stores than one real pylon a wing spreads to the neighbouring real pylon: two a wing are one on the middle pylon and one on the outer, the outer pair first; one a wing hangs on the outer pylon. Each pylon sits 0.75 ft further back for each foot out, along the swept leading edge; Su-27: middle 0.4 ft back, outer 3.4 ft back (rails about 1.5 ft behind the leading edge, clear of the model's drooped leading edge); Su-35: middle 0.6 ft forward, outer 2.4 ft back (rails at the leading edge) |
| 8, wingtip | 28.0 / -5.33 | Unchanged: on the tip's edge | |

The inner underwing pylons (about 10 ft out) have no retail row, so
nothing is drawn there. Every store the ordnance screen allows on these
rows hangs at the same stations. Clips measured with the stores preview's
`clip` lines, default loads:

| Store | Before | After |
| --- | ---: | ---: |
| Su-27 intake AA-12 | 1.30 ft (into the duct beside it) | 0 |
| Su-27 tunnel AA-12 | 0.54 ft | 0.54 ft (grid fins into the tunnel) |
| Su-27 wing AA-11 | 0.71 ft | 0.72 ft (the outer one's nose under the drooped leading edge) |
| Su-35 intake AA-ML | 0.38 ft | 0.38 ft |
| Su-35 wing AA-12 | 0 | 0 |

Opinionated, John 2026-10-14; positions are agent decisions (fitted).

## The Yak-141

John, 2026-10-14, with a picture of a Yak-141 from the side: "the wingtip
is actually a wing pylon, and both pylons are fully nestled under the
wing." Retail's records put the AA-10s 8.33 ft out at 0 ft forward and the
AA-11s 16.67 ft out at 0 ft forward. The model's wing is swept (its leading
edge about 0.8 ft forward at 7.5 ft out, 2 ft back at 11 ft out, 6.3 ft back
at 16.67 ft out; its trailing edge about 11 ft back; its tip about 18.5 ft
out), so the outer row was ahead of the wing and near the tip.

The real aircraft has two underwing pylons a wing, both well inboard under
the chord, the missiles' noses a little ahead of the leading edge. T.O.R.E
hangs the two rows there ([`station_move`](#source-notes)):

| Record (YAK141.PT row) | Retail position, ft out / forward | T.O.R.E station | Default store |
| --- | --- | --- | --- |
| 4, inner pylon | 8.33 / 0 | 7.5 / -2.8 | AA-10 (14.8 ft): rail 0.25 ft behind the leading edge, nose 4.6 ft ahead of it, tail 0.8 ft ahead of the trailing edge |
| 5, outer pylon | 16.67 / 0 | 11.0 / -4.4 | AA-11 (9.5 ft): rail 0.25 ft behind the leading edge, nose 2.35 ft ahead of it, tail 1.8 ft ahead of the trailing edge |

Both hang 1 ft under the wing, 3.5 ft apart. Every store the ordnance screen
allows on these rows (one a wing on each) hangs and leaves from the same
stations, centred where the default store is. Nothing of the default load
reaches into the airframe (the preview's `clip` lines read 0). Wide stores
on both rows of one wing (two GBU-10s, 5.4 ft wings) would touch wing tip
to wing tip. Opinionated, John 2026-10-14; the positions are agent decisions
(fitted).

## Performance

The stores join their aircraft's existing vertex batch (no extra draw call or
pipeline). A pylon is 48 vertices, a fitted pod or tank 144, a weapon shape its
own triangles; the model surfaces pylons meet are looked up once per point and
aircraft type. On a
16-aircraft formation each carrying six stations this is about 10,000 extra
vertices a frame.

## Source notes

- Hardpoint record layout and location names: [aircraft ordnance](../formats/aircraft-ordnance.md).
- Retail release position and the 0x4 mirror rule: [weapons formats](../formats/weapons.md#release-state)
  (FA.EXE 1.02F `HARDPos` `0x4532a0`, `HARDUnrotatedHardPos` `0x453220`).
- Code: `tore_sim::combat::hardpoints` (the rule, twin rails on a wing (`Rack::pylons`, `Rack::rack_offset`), tandem fuselage pairs,
  `station_move` and `placed_on` (the F-14's, Yak-141's and Flankers' stations, the shoulder rail, separate wing pylons),
  `Rack::store_point`, `slot_offset` and `shoulder` (the bar), `placed` and `Underside::nudge`
  (moving aside), the 1 ft pylon, `default_records` (the A-4E)), `tore-app/src/store_sizes.rs`
  (`fin_clock`, `carried_roll`, the fin roll in `variants`), `tore_sim::combat::store_sizes` (the sizes
  stores hang by), `Station::release_point`, `combat::live::cockpit`
  (`release_round`), `tore_world::ai_wings` (AI missiles),
  `tore_world::snapshot::store_poses`, `tore-app/src/pylons.rs` (drawing), `tore-app/src/store_sizes.rs` (real sizes), `tore_world::snapshot::pod_body`.
- Scenarios: `a2g-release-origin`, `net-a2g-stores` (a client sees another
  player's and an AI's stores empty) and `replay-a2g` (the stores recorded
  and replayed) in the battery.

## The F-15C

John, 2026-10-15: move the wing pylons aft and put the four fuselage
missiles on the lower outer intake corners, two in tandem on each side.
This layout is **opinionated**, requested by John. The exact coordinates
are **fitted**, agent decisions from the model and his supplied photograph,
not recovered original-game behaviour.

`F15.PT` is the roster's only F-15, the F-15C. No F-15E, ACTIVE or
conformal-tank station record exists in the supported roster.

| Source row | Before, feet out / forward | Fitted station, feet out / forward | Rule |
| --- | --- | --- | --- |
| 4, fuselage missiles | 8.33 / 0 | 5 / -1 | Lower outer intake corners; four missiles form two tandem pairs with the existing 1 ft nose-to-tail gap |
| 5, wing stores | 16.67 / 0 | 11.0 / 1.5 (16.67 / -5.5 until 2026-10-20) | Under the wing chord; four Sidewinders keep one small twin-rail bar per wing |

The duct bottom at 5 ft out is about 4.0 to 4.3 ft below the model's
reference point between 14 ft forward and 6 ft aft. The old source height
is only 3.33 ft below it, inside the duct. This row therefore reads the
underside starting 8 ft below the reference, rather than from that source
height. Its rails remain 2 inches deep; the upper fin tips may enter the
body slightly, as accepted for fuselage rails. The four missile release
origins are 5.5 ft forward and 7.5 ft aft, at 5 ft either side. The meshes
are not all centred on those origins: the AIM-120 extends from -2.67 to
9.33 ft relative to its origin, the Sparrow from -6 to 6 ft. Thus the
front AIM-120 spans 2.83 to 14.83 ft forward, the rear -10.17 to 1.83 ft;
the front Sparrow -0.5 to 11.5 ft, the rear -13.5 to -1.5 ft. Both keep
the existing 1 ft nose-to-tail gap. The front pair leaves first.

(Superseded on 2026-10-20: the pylon is now 11 ft out and forward, see
[the F-15C wing pylon](#the-f-15c-wing-pylon-moves-inboard-2026-10-20).)
At the 16.67 ft span the model chord runs approximately from 0 to 9 ft
aft. The release origin is 5.5 ft aft. The Sidewinder mesh's centre
is 1.21 ft forward of that origin, so the drawn pylon centre is 4.29 ft
aft, near mid-chord. Pylon depth remains 1 ft, the twin bar
and 45-degree fin roll remain unchanged, and tanks selected on this row
use the same aft position.

The source has no centreline carrier row. T.O.R.E now appends the
[derived tank stations](#derived-f-15c-and-f-16c-stations) requested by John.
The original missile rows retain their loading rules; their tanks still
replace weapons on those rows. The new wing tank row is independent.

The same table builds weapons, optional loads and tank positions. Flight,
network seats and newly recorded store layouts use those positions;
existing replays retain the positions saved in their recordings.

## The F-15C wing pylons, 2026-10-20

John, 2026-10-20: "there is a wing pylon that only accepts tanks; that should
be fixed (on the F-15)". The appended wing row (row 7, stations 2 and 8) took
only the 610 gallon tank (station mask 0x205), so the Load Ordnance page showed
its box empty and refused every weapon, although the real Eagle's wing pylon
carries air-to-air missiles on its launcher as well as the tank.
**Opinionated**, requested by John; the list is an agent decision.

| | Before | After |
| --- | --- | --- |
| Station mask | 0x205: tank | 0x2a5: tank, infrared and active radar missiles |
| Normal loading takes | One 610 (or 150, 250, 350, 500) gallon tank a pylon, two in all | The same tanks, or an AIM-120, AIM-9B, AIM-9M or AIM-9X a pylon (two in all, 90 class), never both on one row |
| Refused | Every weapon | AIM-7 and AIM-7E, AA-11, bombs, rocket pods, air-to-ground missiles and pods. Cheat loading keeps its unrestricted rule |

The row is exclusive like the F-22's inboard pair: choosing a tank empties the
missiles and choosing a missile empties the tank, through the same selector.
A missile hangs where the tank would, under the pylon, with the two Sidewinders
of row 5 on its shoulder rails above, so a full Eagle carries 4 fuselage
missiles, 4 shoulder Sidewinders and 2 more missiles or 2 tanks on the wing
pylons. The row gains a weapon slot (a fifth station after the centreline
bomb row), empty by default. A loadout saved before it has one station fewer
and still restores, the new station empty (`stations::later_weapon_rows`).
The station list, the checkpoint's station entries and a lobby loadout's
station count for the F-15C change with it; see the report for the protocol.

### The F-15C wing pylon moves inboard (2026-10-20)

John, 2026-10-20: "the wing pylons were below the paired pylon; the whole
thing needs to be moved inward, kind of like the real F-15". The wing pylon
(rows 5 and 7: shoulder Sidewinders, the tank or the AIM-120/AIM-9 under
it) was 16.67 ft out, at the wing's outer third. It is 11 ft out
(`F15_WING_OUT`, **fitted**, from John's marks on the model's 22.8 ft
semispan; the real Eagle's pylon is about 9 to 10 ft of 21 ft out), its top on the wing's underside there. John then asked for it forward,
"almost to the tip of the front of the wing": `F15_WING_FORWARD` puts row 5 1.5
ft ahead of the reference point (row 7's tank pylon 1.2 ft ahead of that, 2.7
ft). The model's wing at 11 ft out reaches 5.25 ft forward, so the pylon's
front (4.8 ft for the Sidewinder pylon, 5.2 ft for the tank's) stands just behind
the leading edge and the Sidewinders' noses (7.4 ft) a little ahead of it; the
tank's nose is about 5 ft ahead of the edge. Row 5 and 7 stand 7 ft further
forward than before (fitted). Both wings are symmetric.
Store release points move with it (stores leave from where they are
drawn); recorded replays keep the poses they recorded.

## The MiG-25

John, 2026-11-07: the MiG-25 (`M25.PT`) hangs its **decoded** hardpoint
positions under the standard pylon rules, not a real-aircraft station table, and
he accepted the AA-6's small upper-fin entry into the wing that the stores
preview reports, as for the F-14's Phoenix. The registration and the decoded
positions are spec-derived; the pylon rules are the existing opinionated ones
(2026-10-13). No other aircraft changed and no `station_move` entry was added.

The three carrier rows, from the [hardpoint table](../formats/aircraft-ordnance.md#hardpoint-positions):

| Record (M25.PT row) | Location | Decoded position, ft right / up / fwd | Count | T.O.R.E station |
| --- | --- | --- | ---: | --- |
| 3 | Wing | 12.67 / -3.67 / -5.00 | 2 (one a wing) | The record's position, 1 ft under the wing |
| 4 | Wing | 17.00 / -3.67 / -7.00 | 2 (one a wing) | The record's position, 1 ft under the wing |
| 5 | Centerline | 0 / -8.33 / -1.33 | 1 | The record's position, on a 2 inch rail under the belly |

Rows 0 to 2 are the fixed visual sensor, radar and jammer rows, so they carry no
drawn store.

No move-aside was needed. The preview measures a wing chord above both wing rows,
nothing hangs in space, the fins clear the neighbouring row (about 2.0 ft between
the AA-6's outer tip and the AA-2's inner tip in the default load; about 0.97 ft
between two AA-6s in a full AA-6 load), and the innermost store stands about 5 ft
outboard of the gear. The measured depths match [the pylon-depth
table](#pylon-depth): 1.00 ft on the wings and 0.17 ft (a rail) for the
centreline tank. The fin roll turns the AA-6's fins to an X like every store.

**Accepted clipping (John, 2026-11-07).** The AA-6 (`ACRID.SH`, 20 ft, 4.76 ft
fins) puts its upper fin tips inside the wing underside by 0.27 ft on row 3 and
0.30 ft on row 4 in a full AA-6 load, from `--stores-preview`'s `clip` lines. The
cause is the standard 1 ft wing pylon against the AA-6's wide fins, not the
decoded position: the fin half span at 45 degrees is 1.68 ft against a 0.51 ft
body radius, so the upper tips stand above the wing underside even straight under
the pylon. A real-station table would not remove it, because a real position
still hangs the same store 1 ft under the same wing. John accepted it, the same
0.27 ft the F-14's Phoenix shows ([the F-14](#the-f-14), "John accepts a little
clipping"), and it is one of the smallest values the preview reports across the
fleet. **Opinionated** (John's decision), for his review.

## The F-16A

John's standing rule (2026-11-07, carried into the F-16A in the M3 aircraft
plan) is the record's **decoded** hardpoint positions under the standard pylon
rules, with a stop and a frame report only if the stores preview shows clipping
beyond what the F-16C already flies. F16A-4 measured it: the F-16A hangs its
decoded positions, no real-station table was authored, and no `station_move`
entry was added. The registration and the decoded positions are spec-derived;
the 1 ft wing pylon, the 2 inch fuselage rail, the fin roll and the move-aside
behaviour are the existing opinionated rules (2026-10-13), applied unchanged.

The five carrier rows, from the [hardpoint
table](../formats/aircraft-ordnance.md#hardpoint-positions):

| Record (F16.PT row) | Location | Decoded position, ft right / up / fwd | Count | T.O.R.E station |
| --- | --- | --- | ---: | --- |
| 4 | Centerline | 0 / -6.67 / 0 | 1 | The record's position, on a 2 inch rail under the belly |
| 5 | Wing | 7.0 / -3.33 / -1.33 | 2 (one a wing) | The record's position, 1 ft under the wing |
| 6 | Wing | 10.67 / -3.33 / -2.33 | 2 (one a wing) | The record's position, 1 ft under the wing |
| 7 | Wing | 14.33 / -3.33 / -3.67 | 2 (one a wing) | The record's position, 1 ft under the wing |
| 8 | Wingtip | 18.0 / 0.67 / -4.67 | 2 (one a tip) | On the model's tip edge, fitted to 17 ft out |

Rows 0 to 3 are the fixed visual sensor, radar, jammer and built-in gun, so they
carry no drawn store. The four spans (7.0, 10.67, 14.33 and 17.0 ft) are the
same four stations the F-16C already flies (its rows 9, 6, 7 and 8), so the
F-16A's load sits where the shipped F-16C's does; the decoded fore/aft
positions are the record's own, not the F-16C's fitted aft shift.

Measured with `tore-app --stores-preview TORE_STORES_PREVIEW_CPU=1
TORE_STORES_PREVIEW_AUDIT=1 TORE_STORES_PREVIEW_FULL=1`, default load:

- the centreline `F150.GAS` tank hangs at 0 / -6.67 / 0 on its rail, clip 0.00;
- row 5 `AGM65G.JT` at 7.0 / -1.50 / -1.33, depth 1.00 ft, nose 1.89 ft behind
  the local leading edge, clip 0.00;
- row 6 `AIM120.JT` at 10.67 / -1.29 / -2.33, depth 1.00 ft, nose 4.75 ft ahead
  of the leading edge, clip 0.00;
- row 7 `AIM9M.JT` at 14.33 / -1.21 / -3.67, depth 1.00 ft, nose 2.97 ft ahead
  of the leading edge, clip 0.00;
- row 8 `AIM9M.JT` at 17.0 / 0.29 / -4.67 on the tip edge, clip 0.42 ft at
  [-16.56 0.73 -6.74].

**No clipping beyond the F-16C.** The F-16C's own stores preview reports 0.00 ft
on its wing rows and 0.42 ft at its wingtip AIM-9M, at the same place, because
the two records name the same tip row (flags `0x0085`, position 54 / 2 / -14)
and the same shape. A full load of the longest store on the inner rows (`AIM120`
on rows 5 and 6) still reads 0.00 ft on both. So the decoded positions ship and
the slice does not stop. The F-16A's row 6 AIM-120 nose overhangs 4.75 ft ahead
of the local leading edge against 3.08 ft for the F-16C's aft-shifted outer
AIM-120 pylon, but a nose ahead of the wing is not a clip; the record's position
is kept. The wingtip AIM-9M's 0.42 ft fin entry into the tip is the same value
the F-16C already ships and John reviewed on 2026-10-16.

## The MiG-31

Opinionated station table, John 2026-11-08. `MIG31.PT` has two editable carrier
rows and no tank or centreline row. Rows 0 to 3 are the fixed visual sensor,
radar, jammer and internal gun, so they carry no drawn store and no fixed sensor
or designator pod is added to the wing rows. The source rows, masks, capacities
and default quantities stay as decoded; only the drawn positions move.

| Record (MIG31.PT row) | Location | Decoded position, ft right / up / fwd | Count | Default | T.O.R.E station |
| --- | --- | --- | ---: | --- | --- |
| 4 | Wing | 8.33 / -3.33 / 0 | 4 | `AA9.JT` x4 | Two tandem pairs under the fuselage, 2.0 ft either side of the centreline, centred 1.5 ft forward; front pair ahead, 0.27 ft rails |
| 5 | Wingtip | 16.67 / -3.33 / 0 | 2 | `AA8.JT` x2 | 16.67 ft out on a standard 1 ft wing pylon under the drawn wing, outboard of the AA-9s |

Measured with `tore-app --stores-preview` (CPU, `TORE_STORES_PREVIEW_AUDIT=1`,
`TORE_STORES_PREVIEW_FULL=1`, `TORE_STORES_PREVIEW_CATALOG=1`,
`TORE_STORES_PREVIEW_AIRCRAFT=mig31`), default load:

- Row 4, the four `AA9.JT`: the rear pair at -5.42 ft forward and the front pair
  at 8.43 ft, each pair 2.0 ft either side of the centreline. The rear stores
  hang at -4.49 ft and the front at -4.75 ft, on 0.27 ft rails. The front pair
  is 13.85 ft ahead of the rear pair (the 13.6 ft store plus a 0.25 ft gap).
- Row 5, the two `AA8.JT`: 16.67 ft out at -0.74 ft, on a standard 1.00 ft wing
  pylon. The pylon height and proportions are the other reviewed fighters'.
- The audit's `clip` line reads `0.48 ft at [3.37 -3.12 -11.00]` for row 4: the
  rear pair's outer upper fin enters the belly skin under the fuselage. It does
  not reach the fuselage side or either intake duct. Row 5 reads 0.00 ft.
- The outermost fin at 3.37 ft is 2.3 ft inboard of the main gear at 5.67 ft,
  and the front store's nose at 16.45 ft is 9.2 ft behind the nose gear at
  25.67 ft, so no store touches the gear with it down.

**Why the record moves.** The shape is 22% wide of the real span (M31-2): its
wing reaches 27 ft, so the record's 16.67 ft Wingtip position is mid-wing, not a
tip. The four-store Wing row at 8.33 ft out also puts its inboard store's fin
2.76 ft inboard of a twin-rail pylon, inside the fuselage. The real aircraft
carries its R-33s in two tandem pairs under the fuselage and its R-60s on
underwing pylons, which is the layout adopted here.

For comparison, the reviewed default loads read: MiG-25 0.27 ft (AA-6, John
accepted 2026-11-07), F-16A and F-16C 0.42 ft, F-14 0.27, F-15 0.29, Su-27 0.72.
The MiG-31's 0.48 ft is under the 0.5 ft target and below the Su-27's 0.72;
before the table the same row read 3.63 ft and the AA-8 hung 3.59 ft below the
wing in space.

No fixed sensor, designator or jammer pod is placed on either row; rows 0 to 3
stay the fixed equipment the record has, and no pod row or fixed pod is added.
The row 4 decoded mask carries the pod bit, so the shared Load Ordnance rules
offer a Pave Spike on that real station, as they do on the F-16A; the
68-placement pod pin includes it. The decoded positions and counts are
spec-derived from `MIG31.PT`; the 1 ft wing pylon, the 2 inch body rail and the
fin roll are the existing opinionated rules of 2026-10-13, applied to the new
stations. The AA-9 rail is 0.27 ft rather than 0.17 ft, a fitted 0.1 ft drop so
the wide upper fins enter the belly only to 0.48 ft; the pylon still meets the
belly. The station table and the fitted drop are opinionated (John, 2026-11-08)
and fitted (agent decision, 2026-11-08) respectively.

## The Mirage 2000 (M2K-5, 2026-11-10)

Measurement only, no station table adopted. `M2000.PT` has five editable
carrier rows and four fixed or internal rows (visual sensor, radar, jammer and
the DEFA gun, rows 0 to 3). The shape is scaled per axis to real size
([aircraft shape scale](../formats/objects-and-shapes.md#aircraft-shape-scale-2026-11-09),
factors 1.3616 right, 1.2290 forward, 1.5053 up, fitted), and the decoded rows
scale with it (`placed_on`). They hang under the standard rules of 2026-10-13
(1 ft wing pylon, 2 inch fuselage and centreline rail, a store on the tip's
edge for a Wingtip row). No `station_move` exists for the Mirage 2000, and per
John's standing decision (2026-11-07) nothing was moved: the slice stops here
on clipping, with the options below for John. No fixed sensor or designator
pod was added (the record has no pod row).

| Row | Location | Decoded x / up / fwd, ft | Scaled x / up / fwd, ft | Default | Hang used | Hangs at (centre, ft) |
| ---: | --- | --- | --- | --- | --- | --- |
| 4 | Centerline | 0 / -5.0 / 0 | 0 / -7.53 / 0 | `F150.GAS` x1 | 2 inch rail under the belly (surface -2.54) | 0 / -3.50 / 0, tank 10.28 ft long, 1.58 ft wide |
| 5 | Fuselage, paired | 1.33 / -6.67 / 0 | 1.82 / -10.04 / 0 | `AS15.JT` x4 | two tandem 2 inch rails a side, surface -2.0 rear, -2.8 front | +-1.82 at -4.05 and +4.05 fwd, y -2.62 and -3.08 |
| 6 | Wing, paired | 3.67 / -6.67 / -3.33 | 4.99 / -10.04 / -4.10 | `R530.JT` x2 | 1 ft wing pylon under the wing (surface -2.01) | +-4.99 / -3.44 / -4.10 |
| 7 | Wing, paired | 6.67 / -6.67 / -6.67 | 9.08 / -10.04 / -8.19 | `R550.JT` x2 | 1 ft wing pylon (surface -2.01) | +-9.08 / -3.27 / -8.19 |
| 8 | Wingtip, paired | 11 / -1.33 / -9.67 | 14.98 / -2.01 / -11.88 | `R550.JT` x2 | tip rule: centre line on the tip's edge, no pylon | +-14.98 / -1.51 / -11.88 |

Measured with `tore-app --stores-preview` (CPU, audit, full, catalog flags, as
for the MiG-31), plus a local triangle-level check of every store against the
airframe, the gear (gear down, the M2K-4 rig, ground plane 6.02 ft under the
reference), the pylons and every other store (edge and face crossings, vertical
entry as the audit measures it, and surface distances sampled at 0.12 ft). The
check tool and its dumps are local only.

Default load (F150 tank, four AS-15, two R530, four R550):

| Station | Entry into the airframe | Edge crossings | Nearest neighbour | Gear down | Ground |
| --- | ---: | ---: | --- | --- | ---: |
| Tank, row 4 | 0.09 ft (nose top into the belly) | 12 | 0.23 ft to the rear AS-15 | 2.88 ft | 1.73 ft |
| AS-15, row 5, rear pair (-4.05 fwd) | 0.56 ft right, 0.75 ft left | 70, 34 | 1.00 ft to the front AS-15 | 0.52 ft to the main gear plate | 2.49 ft |
| AS-15, row 5, front pair (+4.05 fwd) | 0.67 ft right, 0.58 ft left | 78, 72 | 0.39 ft to the tank | 3.00 ft | 2.04 ft |
| R530, row 6 | none (0.42 ft clear) | 0 | 1.98 ft to the R550 outboard | 0.80 ft | 1.57 ft |
| R550, row 7 | none (0.18 ft clear) | 0 | clear | 3.0 ft | 2.08 ft |
| R550, row 8 (tip) | 1.21 ft (upper fins into the wing tip) | 12 | clear | 3.0 ft | 3.84 ft |

**Clipping, two cases for John** (stop-and-report rule of 2026-11-07; the
reviewed default loads read 0.27 ft for the MiG-25, 0.42 ft F-16A and F-16C,
0.27 ft F-14, 0.29 ft F-15, 0.72 ft Su-27 and 0.48 ft MiG-31):

1. **Wingtip R550 (row 8), 1.21 ft.** The decoded x (33 units, 14.98 ft scaled)
   is the shape's own tip, a delta tip with a one foot chord whose corner is at
   -11.88 ft forward. The standard tip rule centres the missile on that edge, so
   its upper fins stand inside the thin wing for 1.21 ft, above every value
   above. The decoded row and the shape agree on the position, so this is the
   rule meeting a very thin tip, not a mis-scaled row. The real Mirage 2000 has
   no wingtip rail (cited below), so no real station decides this row.
2. **Belly AS-15 (row 5), 0.56 to 0.75 ft.** The four missiles hang on 2 inch
   rails at +-1.82 ft, tandem 8.1 ft apart. Their fins and the wide body enter
   the belly and fuselage flank, inboard edge at 0.89 ft. This sits in the range
   of the Su-27 (0.72 ft), but it is clipping all the same and is listed for
   John's decision. The expected tank-versus-AS-15 clash of M2K-0 did **not**
   happen: the tank is 1.58 ft wide, so the AS-15's inboard edge clears it by
   0.23 ft.

The centreline tank's 0.09 ft entry, the two wing rows and every gap and
ground clearance above are clean. Nothing touches the ground or the gear.

Update (MF1-5b, 2026-11-11): John chose option (c) for row 5 (two stores moved out, here the
"clean slot" 3.24 ft) and option (b) for row 7 (a fitted tip offset); see
[the Mirage F1 stations, finished](#the-mirage-f1-stations-finished-mf1-5b-2026-11-11).

**Options for John (not applied):**

- Row 8: (a) accept 1.21 ft; (b) hang it on a standard 1 ft wing pylon at a
  fitted position inboard of the tip (the MiG-31's `Hang::Wing`, but where the
  wing has a chord to hang under, roughly 12 to 13 ft out), keeping the record's
  mask, quantity and default; (c) move it to the nearest real station position.
- Row 5: (a) accept 0.56 to 0.75 ft; (b) the MiG-31's fitted drop (rails 0.1 ft
  lower) to cut the entry; (c) move the pairs outboard, which brings the rear
  pair closer to the main gear (0.52 ft today); (d) a different pair spacing.

**Alternatives the Load Ordnance page offers**, each against the default
others (the live-fire list is `DEFA`, `R530`, `R550` and `AS15`, the tank
records are on the same rows). Rows 6 and 7 are clean for every one of
`AS15`, `R530`, `R550`, `F150` and `F250` (a tank hangs 1 ft under the wing,
0.88 ft from the main gear on row 6). Row 5 alternatives:

| Row 5 load | Entry into the airframe | Against the centreline tank | Gear down |
| --- | --- | --- | --- |
| R530 x4 | 0.62 to 0.91 ft | 0.45 ft | 0.35 ft (rear pair) |
| R550 x4 | 0.86 to 1.02 ft | 0.05 ft (almost touching) | 0.50 ft |
| F150 x4 | none | 0.28 ft | 0.57 ft |
| F250 x2 | 0.15 ft right | 0.10 ft | 0.86 ft |
| F350 x2 | 0.17 ft, 0.02 ft | **crosses the tank** (80 crossings) | 0.60 ft |
| F500 x2 | 0.19 ft, 0.08 ft | **crosses the tank** (96 crossings) | 0.34 ft |

The big tanks on row 5 are a Load Ordnance choice only (no default and not
live-fire); they cross the centreline tank when both are loaded, a clash that
needs its own decision if John wants it removed (for example a mask or capacity
rule on row 5 when row 4 holds a tank).

Real Mirage 2000 for plausibility only (the decoded rows decide): nine external
stations, with the Super 530 on the inboard wing pylons and underbelly and the
Magic on the outboard wing pylons (MILAVIA, Mirage 2000 specifications, and the
FAS DOD 101 Mirage 2000 page, both accessed 2026-11-10; forum posts disagree on
the fuselage and wing split, so no split is taken from them). The record has
eleven store positions (one centreline, four belly, four wing, two tip) against
nine real stations, so the wingtip row has no real counterpart; the placements
above are the record's own.

The row positions, masks, capacities and defaults are spec-derived from
`M2000.PT` (M2K-0, M2K-1); the per-axis scale is opinionated (John, 2026-11-09)
with fitted factors; the pylon and rail depths are the existing opinionated
rules of 2026-10-13. The measurement, the comparison and the stop are agent
findings, 2026-11-10. The ordnance audit pins did not move (no placement count
changed).

### The Mirage 2000 decisions (M2K-5, 2026-11-10)

John decided the two clipping cases and a belly rule on 2026-11-10 (opinionated,
John, 2026-11-10). The measurements above are kept as they were.

- **Belly pairs (row 5): the MiG-31's 0.1 ft rail drop, decoded positions kept.**
  `hardpoints::rail_drop` lowers the surface row 5's rails read by 0.1 ft, so
  its stores hang 0.1 ft lower (`placed_on`, one place, every store on the row
  including the tank records; the 0.1 ft is the MiG-31's agent-fitted value,
  see [the MiG-31](#the-mig-31)). John accepts the remaining entry. Measured
  after the drop (same triangle-level check as above; before the drop in
  brackets):

  | Row 5 load | Entry into the airframe | Against the centreline tank | Gear down |
  | --- | --- | --- | --- |
  | `AS15.JT` x4 (default) | 0.46 to 0.65 ft (0.56 to 0.75) | 0.26 ft (0.23) | 0.52 ft |
  | `R530.JT` x4 | 0.52 to 0.81 ft (0.62 to 0.91) | 0.42 ft (0.45) | 0.35 ft |
  | `R550.JT` x4 | 0.76 to 0.92 ft (0.86 to 1.02) | 0.02 ft, almost touching (0.06) | 0.50 ft |
  | `F150.GAS` x4 | none | 0.32 ft (0.28) | 0.57 ft |
  | `F250.GAS` x2 | 0.05 ft right, none left | 0.10 ft | 0.86 ft |

  The audit's own `clip` line for the default load reads 0.57 ft (0.67 before).
  The residual is recorded, not fixed further, as John asked.
- **No big tanks on the belly pairs.** `aircraft_stations::tank_allowed` refuses
  `F350.GAS` and `F500.GAS` on the Mirage 2000's row 5 in normal loading (they
  crossed the centreline tank); Cheat loading skips that check as it does for the
  other authored-tank masks, and the 150 and 250 gallon tanks stay. The tank
  placement pin in `ordnance_audit.rs` moves from 201 to 199, two placements,
  nothing else (4,076 weapon, 194 Cheat-only and 71 pod placements unchanged).
- **Wingtip R550 (row 8): not moved, stopped for John.** John asked for the
  nearest real station, and the real aircraft has two wing pylons a side (inner,
  outboard) and no wingtip rail (nine stations, five under the fuselage and two
  under each wing; the outboard wing pylons carry the Magic, the inboard ones
  the Super 530: archived Mirage 2000 article and the Ukrainian defence press
  numbering of the Mirage 2000-5, hardpoints 1 and 9 outer wing, 2 and 8 inner,
  searched 2026-11-10; no source gives the pylons' lateral offsets). The record's
  rows 6 and 7 already are those two real wing stations, so the nearest real
  station to row 8 is row 7's own spot (9.08 ft out, R550). Putting row 8 there
  coincides with row 7, so per the decision the slice stops. Measured options
  (the row hung on a standard 1 ft wing pylon with `Hang::Wing`, default
  load otherwise, each clean against the airframe): a pylon at 12.0 ft out,
  -9.6 ft forward, no entry, 0.78 ft from the row 7 R550 (two missiles abreast, frames
  `final-option-row8-pylon-x12-*`); at 13.0 ft out, -10.6 ft forward, no entry,
  about 1.75 ft from row 7 (bounding boxes). Options for John: (a) a fitted second outboard pylon at
  about 12 to 13 ft (the two above); (b) keep the tip rule (1.21 ft entry);
  (c) put it on row 7's own pylon (coincident stores, needs a twin-rack rule);
  (d) mask `R550.JT` off row 8 so the row carries nothing. The decoded row,
  mask and capacity are unchanged in every option.

#### No Mirage 2000 wingtip station (M2K-5, 2026-11-10, later the same day)

John decided (opinionated, John, 2026-11-10) that TORE carries **no store on
the Mirage 2000's wingtip row**, superseding the wingtip part of the decisions
above (those lines stay as written). Row 8 of `M2000.PT` (Wingtip, x 33, two
`R550.JT`, mask 0x85) is the Rafale template's wingtip rail carried into the
Mirage record; the real aircraft has nine store stations and no tip rail. The two
Magics ride the outer-wing pylons (row 7, unchanged: `R550.JT` x2, one a wing).

- **How.** `aircraft_stations::retire` drops that exact retail row, only while it
  is the last of nine and unedited, right after the PT is decoded and after the
  guarded additions of `append` (the same place and kind of rule: an opinionated
  station table applied to the decoded record, one function, a unit test). The
  decoder, the catalog CSV and the retail row in
  [the source review](../formats/aircraft-variety.md#mirage-hardpoints) are
  unchanged and stay documented as retail. Because the row is absent from
  TORE's record, every consumer agrees: the default load and its AI and saved
  copies, Load Ordnance, the audit, the stores drawing and the placements
  measured above. No station index moves (it was the last row).
- **Cheat loading.** It has no row 8 either, as for any station TORE's record does
  not have; the two Cheat-only novelty placements it had there are gone.
- **Counts.** `ordnance M2000.PT: 5 source rows, 0 optional weapon rows, 174
  compatible weapon placements, 7 tank placements; ... catalog 62 normal, 83
  Cheat`. Audit pins: weapon placements 4,076 to 4,066 (the ten compatible
  placements of the row) and Cheat-only 194 to 192; tank (199) and pod (71)
  placements and the roster are unchanged.
- **Default load.** Fixed gun, F150 centreline tank, four AS-15, two R530 and two
  R550 on the outer wing pylons; no tip missiles. The 1.21 ft tip clipping case
  and its options above no longer apply.

## The F-16C and fleet rail audit


John, 2026-10-15: put the F-16 underwing pylons under the wing, farther
aft, and review every aircraft's rails for obvious overhang. The F-16
changes are **opinionated**, requested by John; exact origins below are
**fitted**, agent decisions using the model chord and drawn weapon extents.
`F16C.PT` is the roster's only F-16 record.

| Source row | Real station grouping | Before origin, feet out / forward | After origin | Behaviour |
| --- | --- | --- | --- | --- |
| 5 | Centreline 5 | 0 / 0 | Unchanged | One tank, under the fuselage |
| 6 | Middle underwing 3/7 | 10.67 / -2.33 | 10.67 / -3 | One pylon per wing; paired stores retain the small twin bar |
| 7 | Outer underwing 2/8 | 14.33 / -3.67 | 14.33 / -7 | One pylon per wing, farther aft under the chord |
| 8 | Wingtip 1/9 | Source 18 / -4.67, fitted to the tip at 17 ft out | Unchanged | Stores sit on the model's tip launcher; AIM-9 or [AIM-120](#the-f-16c-wingtip-aim-120), one a tip |

The inner wet pair, real stations 4/6, has no source row. The
[derived station addition](#derived-f-15c-and-f-16c-stations) supplies it.
Tank selections on existing compatible rows retain their capacities and
replace the weapons on that row.

At 10.67 ft out the model wing runs from about 2.25 ft forward to 7 ft
aft. The Maverick's drawn centre is 0.34 ft ahead of its release origin,
so its pylon is about 2.66 ft aft, near the middle of that chord. At
14.33 ft out the chord runs from 0.75 to 7 ft aft. The AIM-120's drawn
centre is 3.33 ft ahead of its release origin, so the moved pylon lies
3.67 ft aft, near the middle. Its nose remains about 3.08 ft ahead of
the local leading edge: a 12 ft missile is longer than the 6.25 ft chord.
The pylon itself stays under the wing.

A geometry audit must measure **drawn mesh extents**, not assume that each
mesh is centred on the release origin. `--stores-preview` with
`TORE_STORES_PREVIEW_CPU=1 TORE_STORES_PREVIEW_AUDIT=1` prints each source
row, actual carried-store origins, local model chord, drawn hanger front
and drawn store front, without a GPU. It covers the selectable roster,
including internal/equipment-only records and optional weapon rows.
The quarter-foot chord samples are a screening tool. They do not establish
real aircraft carriage clearance, variable-sweep clearance or certification.
Long stores and tip launchers can extend ahead of a wing. Uncertain
real-world station mappings require references and a design decision,
not a blanket aft shift.

### The F-16C wingtip AIM-120

**Opinionated**, John, 2026-10-16: "On the F-16, the wingtip station
typically can have AIM-120s. Can we make sure that is enabled for the
wingtip?" The real F-16C's tip launchers (stations 1 and 9, LAU-129) carry
the AIM-120 as well as the AIM-9. The rule below is an **agent decision**.

Why it was refused: source row 8 is a wingtip station with mask 0x85
(infrared missiles only) and weight class 4 (400 lb, sized for two
190 lb AIM-9s). The AIM-120 is a 345 lb active-radar missile, so the
station mask turned it away, and a weight class of 4 would have allowed only
one. Raising the class would have changed the other infrared counts too
(a 230 lb AA-11 would go from one to two).

What changed: on the F-16C only, normal loading gives the AIM-120 on the
wingtip row the row's own count, **2**, which is one a tip, exactly what the
row holds for an AIM-9 today. The rule is
`aircraft::stations::extra_weapon` (read by `Loadout::capacity`, which every
caller shares: the Load Ordnance page and its drag and drop, the saved-load
and mission-text checks, the multiplayer lobby's loadout check and the AI
wingmen who copy their lead's loadout). It applies only to an F-16C record
whose guarded station rows were appended (so the reviewed source signature
matched, the same guard as the [derived stations](#derived-f-15c-and-f-16c-stations)).
Nothing else moves:

- every other store keeps the source mask and weight rule on that row (AIM-9
  two, AA-11 one, radar missiles other than the AIM-120 none, bombs none);
- the other F-16C rows, their order, station numbers and default load, and
  old saved loads, are unchanged (an old load simply has the AIM-9 there);
- Cheat loading already gave the row's raw capacity;
- the F/A-18D's tips stay AIM-9 only, as the real aircraft's.

A tip AIM-120 is placed by the same [wingtip rule](#rules) as the AIM-9: its
centre line on the tip's edge (17 ft out), no pylon. The 12 ft missile is
longer than the tip's 9 ft chord, so its nose stands about 3.2 ft ahead of
the tip's leading edge and its tail ends at the tip's trailing edge (the
outer-wing AIM-120 already shows about 3.1 ft ahead). The model's tip
overlaps the drawn missile by 0.9 ft at its worst vertex, against 0.4 ft
for the AIM-9; the audit's clip report shows both. Left and right alternate
as for any paired row, the missile leaves from the tip edge, and it flies the
AIM-120's own guided record.

Checked by `--validate-ordnance` (the F-16C tip block of `ordnance_audit.rs`:
capacity of every supported weapon, load, save, restore, two shots from the tip
edges), `--validate-wing-loadout` (wingmen and the lobby check) and a Load
Ordnance snapshot (`--snapshot-state ordnance-tip-amraam`).

Other aircraft with a wingtip row: the F/A-18D, Rafale C and X-31 tip rows have
the same infrared-only mask and stay as they are; the Su-27's tip row already
accepts the AIM-120 under the original mask, and the Su-35's is a fixed
station. These are listed for John, not changed.

### Mi-24 lower wing attachment

The fleet audit also found `MI24.PT` row 2 hanging its four AT-2s from the
rotor-height surface, at about +7.64 ft. Its source origin (8.33 ft out,
0 ft forward) is ahead of the stub wing, so the upward search sees the
rotor disk instead of triggering the generic off-wing fallback. The row
now sits 12 ft out and 5 ft aft, near the outer stub-wing ends, clear of
the model's fixed pods around 9.5 ft out. At that span the wing is about
1.3 ft below the reference. The existing twin bar hangs 1 ft below it;
its short AT-2 noses remain near the leading edge.

This is **fitted**, an agent correction under John's 2026-10-15 fleet
review request. The twin bar remains the game's simplified launcher;
the [US Army Mi-24 reference](https://odin.t2com.army.mil/WEG/Asset/Mi-24_%28Hind%29_Russian_Attack_Helicopter)
places the four AT-2s on two underwing rails. No rotor, flight controller,
weapon capacity or guidance behaviour changes.

### Apache stub-wing pylon attachment

**Fitted**, agent correction under John's 2026-10-21 pylon review request.
The renderer prefers a short attachment to the nearby wing over a distant
surface directly overhead. If its first surface is more than 2.5 ft above
the store lugs or rack bar, it checks up to 3 ft inboard in half-foot steps
and takes the first surface within that height. The 2.5 ft threshold is the
existing 1 ft wing depth plus the 1.5 ft surface-following tolerance. If
the mesh middle still sees only the distant surface, the hanger moves toward
the station origin in quarter-foot steps, up to 12 ft, stopping at the first
short attachment. This also handles the Apache's AT-12 (`SWNGR.SH`), whose
mesh middle is aft of the stub wing. The store itself never moves. When
there is no such nearby surface, the existing attachment remains. The rule
applies to every aircraft, including paired and empty pylons.

On `AH64.PT` row 6, the station sampler correctly finds the stub wing at
about -0.083 ft. Drawing starts higher, at the already placed missile, so
the rotor at about +6.33 ft enters the renderer's 12 ft upward search.
That directly overhead hit previously hid the nearby wing inboard, producing
a slab about 7.4 ft tall. The short attachment now meets the stub wing;
its top still sinks 0.02 ft into the model. Pylon length and surface-following
use the same selected surface. Store positions, release points, capacities,
weapon behaviour, simulation, protocol and replay formats are unchanged.

The supported default Apache load is two `LAU61.JT` rocket pods and two
`AIM9M.JT` missiles, plus its gun. Neither a Hellfire weapon nor an M299
four-rail launcher or M261 pod is supported by this loadout. This correction
does not relabel those weapons or add cosmetic missiles. Real Apache launcher
support remains separate work; these are fitted pylons for the game's
available stores, not a claim of exact real-aircraft carriage or retail parity.

The CPU stores preview checks all 47 selectable aircraft (46 until the Tornado IDS (Clean), TOR-13) with default and
maximum current-candidate weapon loads, retaining installed tanks. This
includes the Mi-24, CH-47, V-22, AV-8, Yak-141, A-10, Su-25, A-7, C-130,
AC-130, Il-76, E-2 and E-3. Aircraft with no external weapon load stay clean;
the roster has no AH-1. `render-ah64-stub-wing-pylons` and
`render-ah64-stub-wing-at12` check the actual Apache hanger vertices. Synthetic
tests cover paired and emptied racks with a rotor above a stub wing, an
off-centre store mesh, and a genuine high wing without a lower surface.

### Derived smaller guided bomb models

The private GB12.SH, GB16.SH and GB38.SH aliases reuse donor geometry with
separate length, body diameter and fin-span refits. Pylon placement and
released-store rendering use the same size table. See the canonical
[smaller guided bomb specification](missiles.md#smaller-guided-bomb-stores)
for dimensions and provenance.

## Derived F-15C and F-16C stations

**Opinionated**, John, 2026-10-15: add the missing F-15C centreline and
independent wing tank carriers and the F-16C inner wet pair. These are
new gameplay capabilities, not recovered original-game behaviour.
Exact positions, load limits, tank shell masses and tank shapes are
**fitted agent decisions**. Defaults remain unchanged; all added rows
start empty. Retail files are never edited.

| Aircraft / appended row (zero based) | Real stations | Capacity | Origin, feet right / forward |
| --- | --- | --- | --- |
| F15.PT / 6 | Centreline 5 | One tank or up to three bombs, 4,500 lb | 0 / -3 |
| F15.PT / 7 | Wing pylons 2 and 8 | Two tanks, or two AIM-120 or Sidewinders (one a pylon), 9,000 lb total ([wing pylon missiles](#the-f-15c-wing-pylons-2026-10-20)) | +/-11.0 / 2.7 (16.67 / -4.3 until 2026-10-20) |
| F16C.PT / 9 | Inner wet 4 and 6 | Two stores, 6,000 lb total | +/-7 / -1 |

The F-15 can retain its four fuselage AIM-7/AIM-120 and four wing
Sidewinders while carrying three `F610.GAS` tanks. The Sidewinders sit
above the shared pylon foot on its two side rails, each 1.75 ft from the
pylon centre to clear the tank and the rolled fins. The F-16 row accepts
two `F370.GAS` tanks or compatible bombs/missiles, including Mk 82 and
AGM-65G, with one store per wing. It uses the existing loading masks,
not a separate release path. Tank and weapon selections exclude each
other on the F-16 wet row. The [2026-10-16 addition](#optional-carriers-2026-10-16)
enables F-15 centreline bombs as requested gameplay, with fitted limits.

A 610 US gallon tank contains 4,026 lb fuel and a fitted 432 lb shell;
a 370 US gallon tank contains 2,442 lb fuel and a fitted 262 lb shell.
Fuel density is the existing 6.6 lb/gal class convention. Shell mass
scales the existing 350 gallon class's 248 lb shell. The normal selector
limits the new tank identities to the above carriers; Cheat retains its
existing unrestricted rules. Existing tank types keep their old rules.
Full F-15 additions total 12,078 lb fuel plus 1,296 lb shells. Full F-16
additions total 4,884 lb fuel plus 524 lb shells, independent of its
existing centreline tank.

Heights sample the model underside with 1 ft wing pylons and the existing
2 inch centreline rail. The F-15 centreline search starts 8 ft below the
reference and the F-16 wet pair 6 ft below. Tank bodies keep the existing
volume-scaled shape rule; no exact real tank contour is claimed.
Weight, external-store drag, fuel-first consumption, Shift+K tank
jettison, Ctrl+Shift+K selected weapon jettison, AI mission loadouts and
recorded store poses use the shared configuration. No AI decision rule
changes. Older saved loads retain their weapon indices and leave added
rows empty.

The parser appends only when every original station field matches the
reviewed F15.PT or F16C.PT signature. Edited rows suppress the addition.
The nine serialized fuel banks stay unchanged: F-16 source row 9 uses
bank 0, unused by its fixed visual sensor. Duplicate bank allocations
are rejected. Equipment damage still refers to source rows, so a visual
sensor hit does not empty the added tanks. This added row has no new
retail subsystem-damage roll. Peers must agree on derived resources,
row definitions, fuel-bank assignment and placement. No protocol number
or checkpoint layout changes in this slice.

Real-carriage context: the [USAF F-15 fact sheet](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104501/f-15-eagle/level/f-15-eagle/)
describes three external tanks; [Kunsan maintenance](https://www.kunsan.af.mil/News/Article/2440224/maintainers-construct-external-fuel-tanks/)
identifies 370 gallon F-16 wing tanks. The requested 610 gallon Eagle
class is used as John's requested size, not a measurement from the game.

### Source limitations behind the optional carriers

The 2026-10-15 roster screen compared the 36 supported retail identities'
source carrier rows with documented real tank carriage. The [2026-10-16 addition](#optional-carriers-2026-10-16) implements selected
candidates below. The table records the source limitations,
not evidence of original-game behaviour or a certification review.

| Aircraft | Source limitation | Real-carriage evidence and next step |
| --- | --- | --- |
| MIG29.PT, Fulcrum-C | Rows 5 to 7 are three wing pairs; no centreline carrier | The [German MiG-29 flight manual](https://jasonblair.net/wp-content/uploads/2015/06/MIG29-Flight-Manual-Pt-1.pdf) describes a 1,500 litre centreline tank. C-variant gun-clearance restrictions remain unknown; the requested gameplay addition is specified below. |
| A10.PT | Rows 5 to 8 are wing weapon groups; no centreline carrier and no tank permission bit on those rows | [USAF](https://www.127wg.ang.af.mil/Media/Articles/Article/865880/selfridge-angb-home-of-the-drop-tank/) confirms external tanks for long-distance flight. [Eglin](https://www.eglin.af.mil/News/Article-Display/Article/814477/40th-fts-expands-a-10-fuel-limitations-in-combat/) distinguishes the later 600 gallon combat-clearance work. Review period A-10A stations and ferry restrictions. |
| RAFALE.PT, Rafale C | Centreline row 4 is occupied by AAS38.SEE and retained as equipment; wing rows 6/7 lack tank permission | [Dassault](https://www.dassault-aviation.com/en/defense/rafale/adapt-and-deliver/) specifies five heavy/wet stations. The addition below retains the sensor and adds an aft centreline tank carrier; exact period tank contours remain unknown. |
| F22.PT | Only internal carrier rows 5 to 7; none has tank permission | [USAF](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104506/f-22-raptor/) specifies two external wing fuel tanks. The addition below fits the prototype model; The naval donor now shares this carrier under the [F-22 naval stores contract](#f-22-naval-stores). |

F-14, F/A-18D, A-4E and F-4 family source groups already represent the
centreline or independent tank positions relevant to this screen. A-7,
AV-8, MiG-17, MiG-21, MiG-23, F-104 and Su-25 have tank-capable carrier
rows, although class sizes and grouped weapon/tank tradeoffs still need
variant-specific review. Su-27/Su-35 and experimental/rotorcraft entries
were not declared deficient merely because their row counts differ from
real station counts. A grouped row can represent several physical
stations. No claim of exhaustive real-world certification coverage is made.

## Optional carriers, 2026-10-16

Implementation specification, **opinionated**, requested by John on 2026-10-16.
These additions are not claims about original-game behaviour or real aircraft
certification. Limits, coordinates and tank shells below are **fitted agent
choices**. A-10 and all default loads remain unchanged.

- F15.PT centreline row 6 accepts one 610 gallon tank or up to three bombs,
  within 4,500 lb total. The F-15C's source rows carry only air-to-air
  missiles, so the rack's list is its own: the station mask 0x3c1 (tank 0x200,
  unguided and optical 0x100, laser 0x80, SARH 0x40) filtered to bombs and
  guided bombs only (`Loadout::capacity`). Normal loading accepts Mk 82 (3),
  Mk 82 high drag, Mk 82 three-pack, Mk 20, CBU-87, CBU-89, FAB-250, FAB-500,
  RBK-250, RBK-500, GBU-12, GBU-16, GBU-38 and GBU-30 (3 each), Mk 84,
  GBU-10, GBU-10A, Paveway, Paveway III, GBU-29 and GBU-29/P (2, by weight),
  and FAB-1000, GBU-28, Mk 82 pack and Mk 82 AIR (1, by weight). Missiles,
  rocket pods and designator pods are refused; Boulder and Moose load only
  under Cheat. A three-position rack has two bombs beside its bar and one
  below, between the fuselage missiles.
- F22.PT row 8 is a paired inboard wing carrier: two physical pylons represented
  by one screen row, four AIM-120 or AIM-9 missiles (two per pylon), or two
  600 gallon tanks (one per pylon), within 9,000 lb total. Tank and missile
  selections exclude each other. No bomb or air-to-ground missile permission.
- MIG29.PT row 8 adds the missing centreline carrier for one 1,500 litre tank,
  within 3,000 lb. Rows 5, 6 and 7 already represent three pylons per wing.
  No duplicate wing pylons are added. Later-variant wing-tank certification is
  not assumed for the Fulcrum-C. Cannon operation is unchanged, an opinionated
  omission of variant-specific tank/gun restrictions pending C-model evidence.
- RAFALE.PT row 9 adds one centreline carrier for a 2,000 litre tank, within
  4,000 lb. This uses one additional loadout row and supplies useful fuel
  capacity while preserving the existing AAS38 sensor equipment and wing loads.
- F22N.PT now shares the bay and wing carriage below. Its original source
  rows were external fuselage carriers; this is an explicit gameplay override.

| Aircraft / appended row (zero based) | Real stations | Capacity | Origin, feet right / forward | Surface read upward from |
| --- | --- | --- | --- | --- |
| F15.PT / 6 | Centreline 5 | One tank or three bombs, 4,500 lb | 0 / -3 | 8 ft below the origin |
| F22.PT / 8 | Inboard underwing pair | Two tanks or four missiles, 9,000 lb total | +/-12 / -6 | 6 ft below the origin |
| MIG29.PT / 8 | Centreline, between the engines | One tank, 3,000 lb | 0 / -2 | 8 ft below the origin |
| RAFALE.PT / 9 | Centreline, behind the AAS-38 | One tank, 4,000 lb | 0 / -4 | 8 ft below the origin |

The new rows also accept the retail tank records that fit the weight class
(F150, F250 and F350 on all three; F500 on the F-22 and Rafale), as the
F-15C and F-16C rows do. Tank counts are capped at one per physical pylon.

Authored records are F600.GAS (600 US gal, 3,960 lb fuel, 425 lb shell),
M396.GAS (1,500 litres rounded to 396 US gal, 2,614 lb fuel, 281 lb shell),
and R528.GAS (2,000 litres rounded to 528 US gal, 3,485 lb fuel, 374 lb shell).
Fuel is rounded to whole pounds using 6.6 lb/US gal; shells scale 248/350 lb
per gallon. These are fitted game classes, not measured tank masses. Each is
added only to a pack containing its carrier; explicit pack records win.
Normal loading restricts each authored tank to its requested carrier row.

Every source row must match its reviewed signature before appending rows.
Old saved loads preserve existing indices and leave new rows empty. The
nine fuel banks remain unchanged: Rafale row 9 uses bank 0, occupied by fixed
sensor equipment rather than fuel. MiG-29 and F-22 row 8 use bank 8.
Fuel consumption, tank and selected-store jettison, AI mission loadouts,
network configuration and replay poses use the shared loadout paths. Peers
must agree on the new resources, compatibility, capacities and station poses.
Current compatibility is covered by the [protocol version rules](../formats/net-protocol.md#versions).

Real-carriage context: [USAF F-22 fact sheet](https://www.af.mil/About-Us/Fact-Sheets/Display/Article/104506/f-22-raptor/)
identifies two external wing tanks. The [German MiG-29 flight manual](https://jasonblair.net/wp-content/uploads/2015/06/MIG29-Flight-Manual-Pt-1.pdf)
describes a 1,500 litre centreline tank, but is not C-model certification.
[Dassault](https://www.dassault-aviation.com/en/defense/rafale/adapt-and-deliver/)
identifies 14 Rafale hardpoints, five suitable for heavy stores and tanks.
John's requested configuration defines the gameplay scope where period or
variant-specific evidence is incomplete.

## The Phantoms' gun pod, 2026-10-16

Implementation specification, **opinionated**, requested by John on 2026-10-16:
"There is a gun pod missing from the Phantom B, at least as an option to load.
I think the other variants had gun pods available to them as well." The records,
the real-aircraft basis and every choice below that John did not name are
**agent decisions**, for his review. They are not claims about original-game
behaviour.

**What the data holds.** The import has one gun pod record, `SUU16.JT`
(SUU-16 gun pod, 1,200 rounds, 1,702 lb). There is no Mk 4, SUU-23 or other
pod record. The Phantoms' source stations:

| Phantom | Built-in gun | Gun pod before | Gun pod now |
| --- | --- | --- | --- |
| F-4B (`F4B.PT`) | none | none | SUU-16 on the centreline, optional (below) |
| F-4J (`F4J.PT`) | none | SUU-16 on its own fixed row 3 (loaded by default; the page can take it off, but the row takes nothing else) | unchanged; now drawn and fired from its nose |
| F-4E (`F4E.PT`) | M61 | none | none (open question, below) |
| F-4G (`F4.PT`) | M61 | none | none (as the F-4E) |

**The F-4B's centreline.** The B's source row 3 is its centreline station, which
carries four AIM-9Bs by default and takes the other single-rack stores (bombs,
rocket pods). The SUU-16 now loads there, one pod in place of whatever the row
carries, through the normal Load Ordnance selector (it is listed on the
air-to-air page with the other weapons; the card reads "SUU-16 Gun Pod, 1
loaded (max 1)"). A real Phantom's centreline is one station, so the pod
replaces the row's stores rather than adding a second centreline row: the
default load (four AIM-9Bs, four Mk 82s on the fuselage pairs, four on the
wing pair and two 350 gallon tanks) is unchanged, and taking the pod off
restores the row's stores. Carrying it, the B weighs 53,416 lb, against 52,394 lb
without it (the pod's 1,702 lb replaces four 170 lb Sidewinders; the same
weight accounting as the F-4J's pod). It starts empty-of-pod; saved loadouts
and mission files from before carry the row's own weapon and load as they did.

- **One pod a station**, Cheat loading too (`Loadout::capacity`), and only on the
  B's row 3 or a row whose source record names the pod (the F-4J's): the pod
  is refused on every other station.
- **Ammunition and mass** follow the F-4J's fitted interpretation: 600 logical
  units for the record's 1,200 rounds (two rounds a unit), the 1,702 lb pod
  counted in the weight whether or not it holds rounds, until it is removed or
  jettisoned. A pod row with a missile on it carries no pod: the entry in the
  configuration (`GunPod`) counts only while the station's weapon is the pod
  (`Configuration::gun_pod`).
- **The Phantom B's stand-in.** The real US Navy pod was the Mk 4 (a 20 mm
  M61A1 pod, 750 rounds); the data has no Mk 4 record, so the SUU-16 (the
  same gun in a ram-air driven pod, 1,200 rounds) stands in, with its record's
  numbers. Agent decision, 2026-10-16.
- **Selecting and firing.** The pod is a gun (`AircraftId::mounts_gun`, the
  Guns only rule): GUN on the selection ring picks it, Tab fires it, with the
  aircraft's own gun first where it has one (none on the B or J). The F-4B has
  no installed gun, so `AircraftId::F4B.guns()` stays empty and
  `gun_pods()` names the pod it can carry.
- **Where the rounds start.** At the nose of the pod as it hangs, under the
  centreline: the drawn pod's middle (`Station::release_point`) and half the
  pod's 15.7 ft length ahead (`Station::muzzle`, `gun_mount`), for the player,
  the AI, the gun sight and the AI's ground-gun cue alike. The F-4J's pod
  fired from its record's position, 3.3 ft ahead of and 6.7 ft below the
  aircraft's reference point; it now fires from the same place as the B's,
  on the belly (agent decision).
- **Drawing.** The pod is a store on its pylon like a rocket pod: a fitted body
  15.7 ft by 1.8 ft (the real SUU-16/A is about 16 ft by 22 inches; fitted,
  `store_sizes::pod_body`), 2 inches under the body (a centreline rail),
  drawn on both Phantoms that carry it (the F-4J's pod was not drawn before) and
  in outside views, replays and on every network client from the same
  `Stores` event. It stays while it has no rounds and leaves only when it is
  jettisoned (Ctrl+Shift+K, with the selected store).
- **External stores.** The pod counts as one external store of 1,702 lb in the
  radar signature, as the F-4J's does; a pod row holding missiles counts the
  missiles.
- **AI wingmen** carrying the lead's flight loadout carry the pod, fire it as
  a gun (600 units, from its nose) and keep it through a restart. No AI default
  load changes: AI Phantoms keep their source stores.

Weight, the nose position and the F-4J's centreline are checked by the
`--validate-ordnance` pod audit ("gun pod F4B.PT: ..."), the wing check in
`--validate-wing-loadout` and the pod snapshot (`ordnance-gun-pod`).

**Limitation.** The F-4J's centreline row (source row 4, empty by default) does
not take the pod: the J already has its own. A J carrying its pod and a centreline
store on row 4 draws both on the belly, one through the other (the real
aircraft's centreline is one station). Not prevented; recorded here.

**Open question for John: the F-4E and F-4G.** Both are built with an M61. Real
F-4Es could in principle carry a centreline SUU-23/A, but the evidence in hand
is thin and a second gun on an aircraft that has one adds little, so neither
gets a pod. Adding one needs a new centreline row (the E's centreline row 4 is
a tank row with no unguided-store bits), appended and signature-guarded like
the rows above; the SUU-16 would stand in for the SUU-23 (there is no SUU-23
record).

**Wire.** Peers must agree on which rows take the pod and its capacity (the F-4B's
row 3, one pod) and on the `Stores` event listing a gun pod station with one
store while it hangs. A host and client that disagree refuse or misdraw a
Phantom carrying a pod. The lead assigns the protocol change.

## F-22 naval stores

Implementation specification, **opinionated**, requested by John on 2026-10-21.
F-22N uses the F-22A's three bay rows and optional wing pair. This is the
requested game configuration, not a claim of retail parity or certification.
The reviewed F22N.PT signature is checked before correcting rows 5 to 7 to
internal bays (location 3, flags 0x5e0, 0x5e0, 0x5e4) and appending row 8.
Edited source records keep their own definitions. Gun position, sensors,
flight characteristics, hook and exact model identity stay aircraft-specific.
The existing F/A-XX consumer inherits its F22N.PT donor's corrected carriage.

Default loads remain 750 gun rounds, two AIM-120, two AGM-65G and four AIM-9X;
the wing row starts empty. Row 8 offers four AIM-120 or AIM-9 missiles, or two
600 gallon tanks, with the same 9,000 lb limit and mutually exclusive selection
as the F-22A. Existing saved loads keep their row indices and new rows empty.
Load Ordnance lists gun, left main bay, right main bay, paired side bays and
wing pair in that order. Each identity keeps its own imported aircraft art.
AI loadout construction uses the same corrected carrier definitions; autonomous
flight and weapon-selection policy are unchanged.

The following geometry is **fitted, agent-authored**. Both reviewed shapes have
the central belly at 3 ft below the reference point. Their distinct belly and
intake polygons are cut and animated using their own materials. Main bays retain
the source lateral boundaries: left x -8/3 to -2/3 ft, right x 2/3 to 7/3 ft,
forward -8 to 17 ft (extended aft to fit tandem stores). Their recess depth is
2.5 ft to contain the default loads.
Side openings are x +/-3 to +/-4.9 ft and forward -6 to 46/3 ft, cut from
the sloping intake skins. Each side has one outward-opening door and a lined
recess roof at 0.4 ft up on F-22A, 0.6 ft on F-22N. This side-bay fit is not
recovered original animation.

| Carrier | Magazine centre x / floor up / release origin forward, feet |
| --- | --- |
| Left main, row 5 | -5/3 / -3 / 1.4 |
| Right main, row 6 | 1.5 / -3 / 1.4 |
| F-22A paired sides, row 7 | +/-3.9 / -1.2 / 1.7 |
| F-22N paired sides, row 7 | +/-3.9 / -1.05 / 1.7 |
| Wing pair, row 8 | +/-12 / own model underside minus the 1 ft pylon and store radius / -6 |

A bay magazine places two stores front to back, separated by their length
plus 0.25 ft. Their common height is the floor plus half the larger of the
fin span and body diameter, plus 0.08 ft. The forward store leaves first;
the side pair alternates left then right. Stores keep their positions as
the load empties. This fits the default/full candidate loads;
larger alternative weapons, including normally compatible guided bombs,
retain their eligibility but have not received enclosure-fit acceptance.
The ordinary compatibility masks and quantities stay unchanged.

Bay weapons are drawn inside, without exterior pylons, and leave from exactly
the same centres. Fully closed doors occlude their drawing, including through
masked pixels in the imported skin. Their counts and positions remain in
network snapshots and replay records while hidden. The [bay trigger timing](aircraft-animation.md#f-22-main-weapon-bays)
applies to bay weapons only. Wing missiles and the gun fire with doors closed
and do not request opening. Main and side doors share that existing timing.
Jettison still permits selected unfixed weapons, including bay stores, and
retains the existing inert falling-store rule. Internal bay stores do not add
external-store radar signature; wing stores and tanks do. The open-bay radar
term remains unchanged. Network clients and replays carry the same store
counts and magazine layout. Protocol 64 rejects incompatible live peers;
existing replay records retain their recorded layouts and still load.

## Designator, FLIR and jammer pod bodies (EQ-2, 2026-11-06)

The import keeps the designator, FLIR and jammer pods as fixed equipment
records: `PAVESPK.SEE` (fixed on the A-10 and F-16C, loadable on pod stations),
`AAS38.SEE` (the Rafale's centreline FLIR), `ALQ167.ECM` (the AV-8B's jammer)
and the others listed in the
[ordnance format](../formats/aircraft-ordnance.md#external-equipment-pods-name-no-shape-eq-2-2026-11-06).
These records name no shape, so retail has no pod body to draw.

A fixed pod and a designator pod loaded through Load Ordnance now draw as a
fitted body on their station, like a rocket pod or tank: an eight-sided tube
(`crates/tore-app/src/pylons.rs` `body`), hanging from the station's place
through the shared pylon rule. The size is fitted public pod dimensions
(`crates/tore-sim/src/combat/store_sizes.rs` `equipment_pod_body`): 12.3 by
1.1 ft for the Pave Spike, 13.2 by 1.2 ft for the AAS-38, 12.0 by 1.4 ft for
the ALQ-167, and `POD_BODY` otherwise. **Fitted**, agent-authored: the records
carry no geometry.

A fixed pod's source row is a placeholder position, not a real pylon: the
F-16C's Pave Spike is recorded at the reference height, inside the fuselage.
So a fixed pod hangs under the model's lowest underside across its own length,
which clears a rising belly, rather than at the record's height
(`Configuration::from_source`, `Underside::lowest`). The A-10's Pave Spike, the
Rafale's AAS-38 and the AV-8B's ALQ-167 already sit on or below their bellies,
so their place is unchanged; the F-16C's moves down to the belly.

A fixed pod is not a station. It is never loaded or jettisoned, its mass stays
in the aircraft's `fixed_external_equipment_lbs`, and it adds nothing to the
external-store radar signature. A designator pod loaded through the page keeps
its existing rules: one to an aircraft, in place of the station's weapon and a
tank on the same mount, jettisoned with the selected store. Every client draws
the same bodies, because both come from the configuration the aircraft is built
with, not from a per-client choice.

Guards: `a_fixed_pod_draws_a_fitted_body_and_an_internal_sensor_does_not`
(`crates/tore-world/src/content_tests.rs`),
`a_loaded_designator_pod_draws_a_fitted_body_on_its_station`
(`crates/tore-world/src/snapshot.rs`) and the retail
`crates/tore-world/src/pod_equipment_tests.rs`.

### Authored pod models (2026-11-07)

John asked on 2026-11-07 for three low-poly models in place of the fitted
eight-sided body: the Pave Spike (`PAVESPK.SEE`), the AAS-38 NITE Hawk
(`AAS38.SEE`) and the ALQ-167 (`ALQ167.ECM`). They match Fighters Anthology's
flat-shaded look: each stays under 100 triangles, with six or eight sided
cylinders.

| Record | Triangles | Look |
| --- | ---: | --- |
| `PAVESPK.SEE` | 92 | White, six sides: a straight tube of one diameter end to end, a blunt faceted head of the same diameter with a dark flat window on its face, a raised collar band under the pylon, a blunt tail |
| `AAS38.SEE` | 94 | Navy gray, eight sides: a rounded head with a dark window low on its front, a dark band joint behind it, a blunt tail |
| `ALQ167.ECM` | 88 | Navy gray, eight sides: blunt conical caps, a raised dark fairing strip along the top, dark antenna stubs under both ends, two dark L-shaped prongs at the front (out from the body, then forward), four small dark fins in an X near the tail |

The colours are sRGB white (1.0, 1.0, 1.0), Navy tactical light gray
(0.50, 0.53, 0.55, about FS 36375, between it and FS 36320) and near black
(0.13, 0.14, 0.15) for the details. Like the fitted bodies, every vertex takes
the nearest palette entry, so the models are lit as the airframe is. The
imported palettes hold pure white (255): an off-white 0.92 took a 235 grey
entry, pure white takes the 255 one. Under the belly the lighting still shades
the white pod light grey, as it shades a white airframe there. A model
fills the pod's size: its length runs along the station, its width and height
scale with the diameter, and its top meets the pylon as the fitted body's does.

The AAS-38's size is corrected to 6.0 by 1.08 ft, the AAS-38A/B's public
figures (Wikipedia, "AN/AAS-38": 6 ft by 13 in, read 2026-11-07). The 13.2 by
1.2 ft above was about twice its real length. The Pave Spike and ALQ-167 keep
their sizes.

Every other equipment pod (`PAVEKNF.SEE` loads as the Pave Spike, `ALQ72.ECM`,
`ALE40.ECM`), every rocket pod and every tank keeps the fitted body. A pod's
`StorePose::shape` carries `TORE-POD:` and its record, so the drawing can find
its model; the replay viewer never loads it as a shape.

**Opinionated** (John's request); the models are agent-authored, not retail
geometry, and are a **fitted** presentation: the records carry no shape. The
source is the Blender build script `assets/pods/build_pods.py`, which exports
the triangles into `crates/tore-app/src/pylons/pod_models.rs`
([how to rebuild](../../assets/pods/README.md)).

Guards: `authored_pod_models_stay_low_poly_and_inside_their_size`,
`only_the_three_authored_records_draw_their_model` and
`authored_pods_take_their_specified_colours` (`crates/tore-app/src/pylons.rs`).

### Fixed pods draw only on a real station (2026-11-07)

John's decision, 2026-11-07: a built-in (fixed) equipment pod is drawn only
when its decoded hardpoint row is a real station. A row whose point is a
placeholder draws nothing; no station is invented for it. This replaces the
2026-11-06 rule above that hung such a pod under the model's lowest underside,
which put the F-16C's Pave Spike far below the belly on a tall pylon block.

The rule: a row is a placeholder when its point lies inside the airframe, that
is, a face of the aircraft's own model lies straight under it within 12 ft
(`crates/tore-sim/src/combat/live.rs` `inside_airframe`). A station's point
hangs below the belly, with no face under it.

Evidence, from the decoded rows of every roster aircraft that carries an
external pod record (`PAVESPK.SEE`, `PAVEKNF.SEE`, `AAS38.SEE`, `ALQ167.ECM`,
`ALQ72.ECM`, `ALE40.ECM`): four aircraft, positions in source units.

| Aircraft | Pod row | Drawn | Why |
| --- | --- | --- | --- |
| A-10 | `PAVESPK.SEE`, location 1, flags `0x8`, [0, 0, 0], count 32767 | No | At the reference point, inside the fuselage; flagged like the internal sensors |
| F-16C | `PAVESPK.SEE`, location 1, flags `0x8`, [0, 0, 50] | No | Inside the fuselage, flagged like the internal sensors and ECM rows beside it |
| AV-8B | `ALQ167.ECM`, location 0, flags `0x601`, [0, 0, 0] | No | At the reference point, inside the fuselage, although its flags are those of a station |
| Rafale | `AAS38.SEE`, location 0, flags `0x601`, [0, -20, 0] | Yes | 20 units under the reference point, below the belly, flagged like the F-16C's centreline tank station; drawn exactly where it was |

The flags agree with the positions for three of the four rows: `0x8` marks the
internal sensor, ECM and gun rows, and `0x601` the F-16C's centreline tank
station. The AV-8B's row is the exception: station flags, but a reference-point
position. The position decides, so it draws nothing.

Only drawing changes. A pod not drawn keeps its mass accounting and its
sensor behaviour exactly as before: neither reads the list of drawn pods.
A designator pod loaded on a real station through Load Ordnance still draws,
with its authored model. With no model to test against, a row is drawn at its
record's point, as before.

Guards: `a_fixed_pod_row_inside_the_airframe_is_a_placeholder`
(`crates/tore-sim/src/combat/live.rs`) and the retail
`crates/tore-world/src/pod_equipment_tests.rs`, which checks each of the four
aircraft and that the Rafale's pod keeps its place.

## Batch 5 stations: Tornado outer wing, Mirage F1 centreline, F-5E defaults (B5-2, 2026-11-10)

**Opinionated (John, 2026-11-10, batch decision round), implemented by slice
B5-2.** Three station tables change, all in `aircraft_stations.rs` and all applied
to the decoded record only while every row involved is exactly the retail one, so
an edited record keeps its own stations. The decoder, the catalog CSV and the retail
rows in [the source reviews](../formats/aircraft-variety.md) stay documented as
retail. In every case **no station index moves, and no row's mask, count, weight
class or position changes**: a row is either dropped from the end of the table or
keeps its place and only its default store changes.

- **Tornado IDS: no outer wing row** (John: "remove end wing stations typically
  reserved for the pods"). `retire` drops row 8 of `TOR.PT` (Wing, x 16, y -22,
  z 4, mask 0x85, two `AIM9M.JT`, weight class 5), the outer wing pair; the real
  aircraft's outer wing pylons carry pods and not weapons. It is the last of nine
  rows, so rows 0 to 7 keep their indices (the same mechanism and guard as the
  Mirage 2000's wingtip row above: exact retail row, last of nine, one function,
  repeated application is a no-op). The inner wing pair (row 7: Wing, x 11, two
  `MK82P3.JT`, mask 0x505) stays, as do the centreline tank pair (row 5) and the
  shoulder pylons (row 6, mask 0x7e5, which still take the AIM-9M). What changes
  for the player: the default load has no wingtip-side Sidewinders (they stay
  loadable on the shoulder pylons), and the audit counts fall (below). The swivelling
  wing pylons are TOR-3's and TOR-5's.
- **Mirage F1: no default pod on the centreline.** Row 4 (Centerline, y -18, mask
  0x7e5, one store, weight class 22) keeps its station, mask, count and class, so
  it still accepts what it accepted; its default `ALQ167.ECM` is cleared. The
  record's American jammer pod is not in Jane's list of the F1's loads
  (`MF1.INF`), and a pod is never hung where the real aircraft has none. The row
  now starts empty and takes a 150 or 250 gallon tank through Load Ordnance
  (two more tank placements). `adjust_defaults` clears it by setting the row's
  store to none, exactly as the rows TORE appends to other aircraft start empty.
- **F-5E: closer to the real aircraft's default load.** The wing pair (row 5,
  Wing, x 22, mask 0x7e5, four stores, class 40) defaults to empty instead of two
  `AIM7.JT` (the real F-5E does not carry the Sparrow); it still accepts what it
  accepted, the Sparrow among them, in Load Ordnance. The `AIM9B.JT` wingtip
  default (row 6) and the gun and centreline tank stay. Its radar and jammer rows
  are covered in [aircraft records](aircraft-records.md#batch-5-record-corrections-tornado-ids-mirage-f1-f-5e-b5-2-2026-11-10):
  row 1 holds the F-104's `F104R.SEE` instead of the generic `F18R.SEE`, and row 2
  holds no `F18.ECM`. A fixed sensor row with no store is how an aircraft has none
  of that sensor: the sensor profile reads radar and jammer only from the rows that
  hold one.

**The F-5E radar.** The real set is the Emerson AN/APQ-159 (20 nm, `F5E.INF`;
Wikipedia "Northrop F-5"), and TORE has no record for it and invents none. The
closest reviewed record is the F-104's `F104R.SEE` (search 50 nm, track 25 nm,
look-down 50, the Basic radar preset): the same lightweight single-seat
fighter set. Options for John: `F4BR.SEE` (the same ranges, the F-4B and A-4E set),
`YAK141R.SEE` (40 and 25 nm, Transitional preset), or `F8R.SEE` (35 and 20 nm, the
closest in range but it has no reviewed preset, so it would never switch on). The
choice stays `F104R.SEE` until John says otherwise.

**The F-5E jammer.** The real F-5E has no internal jammer. `F18.ECM` is a bundle:
30 chaff and 30 flares plus a radar-deception jammer (chance 30) and an infrared
one. Clearing the row removes all of it, so the F-5E has no countermeasure
dispensers either. If John wants chaff and flares without a jammer, `F104.ECM`
(20 and 20, no deception, an Early-generation label) is the alternative record.

Update (B5-3, 2026-11-10, opinionated, John's round 2, item D2, "follow recs"):
the F-5E's row 2 now holds `F104.ECM` in place of the empty row B5-2 left: 20
chaff, 20 flares and no deception jammer, with the Early-generation jammer label
the F-104 and the Hawk already carry. The real F-5E has no internal jammer but
carries an AN/ALE-40 chaff and flare dispenser (John's round 2 note; the
AN/ALE-40 is the standard US dispenser, taking 30 chaff or 15 flare cartridges,
[FAS, AN/ALE-40](https://man.fas.org/dod-101/sys/ac/equip/an-ale-40.htm), accessed
2026-11-10; for the F-5 itself only secondary sources were found, a DCS F-5E
community thread quoting the type's technical order for two cartridges, so the
installation is cited second hand). TORE's record counts are not
the ALE-40's, because `F104.ECM` is the closest existing dispenser-only record,
not a model of the real cartridge count. The row keeps its place, mask, count and
position, so no station index moves; only the retail row (all seven rows
unchanged from retail except the B5-2 defaults) takes the change, and an edited
record keeps its own jammer row.

**Counts.** `ordnance TOR.PT: 4 source rows, 0 optional weapon rows, 85 compatible
weapon placements, 8 tank placements` (was 5, 0, 96, 8); `ordnance F5E.PT: 4 source
rows, 1 optional weapon rows, 71 compatible weapon placements, 5 tank placements`
(was 0 optional rows); `ordnance MF1.PT: 5 source rows, 2 optional weapon rows,
175 compatible weapon placements, 6 tank placements` (was 4 tank placements).
Audit pins: recovered stations 17 to 18, weapon placements 4,663 to 4,652, tank
placements 231 to 233, Cheat-only placements 222 to 220; the pod placements (80)
and the roster (45) are unchanged. See the [ordnance availability
baseline](../baselines/ordnance-availability.md#batch-5-record-and-station-corrections-b5-2-2026-11-10).

## The F-15E (F15E-5, 2026-11-10)

Implementation and review mode, slice F15E-5 of batch 5. **Stopped on clipping,
as John's standing rule (2026-11-07, batch rule B7) requires: no station table,
`station_move`, mask or pylon code was authored, and nothing changed in
behaviour.** The decoded rows hang under the standard pylon rules (no per-aircraft
entry in `station_move`, so `Hang::AsRecorded` for every row). Measurements are
the shape as drawn (350 faces, true size, no scale), the default load and every
store each row accepts, checked against the fuselage, the wings, the main and
nose gear (gear down and in transit) and the ground, and every store against
every other store and pylon. Evidence and scripts are local only
(`.local/m3-aircraft/checks/F15E-5/`, frames in
`.local/m3-aircraft/frames/F15E-5/`); the method is the Mirage 2000's
([M2K-5](#the-mirage-2000-m2k-5-2026-11-10)): the stores preview's aircraft-frame
triangles, then a triangle-level analysis, surface distances good to about 0.05 ft.

### Station table (default load, feet from the reference point)

| Row | Location, decoded right / up / forward | What it holds | Where it hangs | Result |
| ---: | --- | --- | --- | --- |
| 0 to 4 | Fuselage flag `0x8`, fixed sensors and jammer | `VIS340.SEE`, `F15R.SEE`, `F15.ECM`, `AV8IR.SEE`, `PAVESPK.SEE` | not stations; no store is drawn | see [the pods](#the-f-15e-pods) |
| 5 | Fuselage, 2.3 / -1.7 / 24.3 | the M61 (450 rounds, real 512) | internal gun, never drawn | not measured (no store) |
| 6 | Centerline, 0 / -8.7 / 0 | `F350.GAS` x1 (alternatives `F150.GAS`, `F250.GAS`) | 2 inch rail under the belly, body y -5.26 to -3.17, z -6.8 to 6.8 | clean: 0.17 ft to the belly, 1.29 ft to the gear, 3.74 ft to the ground |
| 7 | Fuselage pair `0x0505`, 6.3 / -8.7 / -1.7 | `MK82.JT` x12: six a side, two triple ejector racks one behind the other (centres z -5.8 and 2.5), the pair 5.6 and 7.0 ft out and the third under at 6.3 | 2 inch rails, tops 0.17 ft under the underside (the shelf and intake flank), racks 1.0 ft apart | see below |
| 8 | Wing `0x01c5`, 14.3 / -4.7 / -4.7 | `AIM120.JT` x4: two a side on a twin rail, 13.7 and 15.0 ft out | 1 ft wing pylon, bodies y -1.39 to -0.15 | clean: 0.55 to 0.64 ft from the wing, 3.0 ft from the gear, 7.6 ft from the ground |

Row 7 on the default load (12 Mk 82):

- **Airframe.** Ten of the twelve bombs are 0.17 to 0.25 ft clear (the third bomb
  under each rack 0.75 ft). The **aft inboard top bomb on each side enters the
  body by 0.06 ft (right) and 0.02 ft (left)**, with 32 and 26 fin and edge
  crossings at x 5.3 to 5.6 ft: the underside is read at the record's x and the
  intake flank slopes in aft of it. This is inside the 2 inch rail depth the
  rail rule already accepts (the reviewed fuselage values are 0.02 to 0.15 ft,
  the Su-27's 0.72 ft was accepted), but it is a measured entry.
- **Bomb to bomb.** On every rack the third (under) bomb's fin tips touch the
  inboard top bomb's: 0.005 ft apart, 20 crossings per rack, four racks. This is
  the generic three-on-a-rack layout ([stores on one pylon](#drawing-opinionated-john-2026-10-12)),
  not F-15E specific: the two top bombs sit 0.69 ft out of the bar and the third
  0.57 ft below it, 0.90 ft between centres against a 0.91 ft fin span. The other
  pairs clear (0.047 ft under-bomb to outboard bomb, 0.47 ft top pair, 1.0 ft tandem).
- **Gear down.** 0.32 ft from the main gear (the aft inboard bombs; the others
  1.0 to 3.0 ft), 3.3 to 3.9 ft from the ground. No crossing.
- **Gear in transit (finding).** With the default load, the main gear leg of
  each side **crosses the aft inboard top bomb** while retracting: at gear 0.4,
  28 crossings and 0.010 ft; 0.064 ft at 0.6; 0.327 ft at 0.8 and 0.319 ft down.
  The leg plates reach 5.7 ft from the centreline at gear 0.2 and 5.5 ft at 0.4,
  where the racks hang (5.2 to 7.5 ft). Gear up is clear. The captured frames are dark and
  not conclusive; the figures are from the triangles of the fitted gear
  ([F15E-4](../formats/aircraft-control-surfaces.md#f-15e-gear-speed-brake-and-plume-f15e-4-2026-11-10)).

Row 8: the two AIM-120s of a pair are 0.022 ft apart (bodies; their fins are at
45 degrees and do not cross). No clip.

### Alternatives each row accepts, measured

Row 6 takes only tanks (`F150`, `F250`, `F350`): all clean (0.17 ft to the belly,
gear 1.29 to 1.54 ft, ground 3.7 to 4.3 ft). Row 7 accepts 25 stores (counts are
the catalog capacity) and row 8 accepts 39. Entry is the deepest vertical entry
into the airframe; crossings are fin and edge crossings between neighbours of one
station.

| Row, store, count | Airframe entry (ft) | Neighbours and gear |
| --- | --- | --- |
| 7 `GBU29.JT` and `GBU29P.JT` x4 | **2.14** (aft inboard), 0.81 to 1.01 front | 0.02 ft from the gear |
| 7 `GBU30.JT` x8 | **1.85 to 1.93** (inboard), 0.53 to 0.70 | pairs of the racks cross (28 each), 0.01 ft from the gear |
| 7 `AGM88.JT` and `AGM45.JT` x10 | **0.95 to 1.02** (aft inboard), 0.73 to 0.77 | under-bomb crosses its neighbours 108 to 114 times, 0.03 ft from the gear |
| 7 `AT2.JT` x12 | 0.36 to 0.56 | 24 crossings a rack |
| 7 `GBU38.JT` x12 | 0.22 to 0.42 | 28 to 86 crossings a rack, 0.19 ft from the gear |
| 7 `CBU89.JT` x10 | 0.13 to 0.34 | 16 to 94 crossings a rack |
| 7 `CBU87.JT` x12 | 0.14 (aft inboard) | 110 to 114 crossings a rack |
| 7 `B13.JT` x6, `LAU61.JT` x12 | 0.05 to 0.08 (preview metric) | 64 crossings a rack, 0.04 to 0.05 ft from the gear |
| 7 `MK84.JT` x4, `FAB1000.JT` x2, `FAB500.JT` x6 | none | clean (FAB500 0.04 ft from the gear) |
| 7 the rest (`B8`, `FAB250`, `MK82HD`, `MK82P`, `MK82AIR`, `MK82P3`, `MK20`, `RBK250`, `RBK500`, `LAU10`) | 0.00 to 0.23 (preview metric only; the triangle analysis was not run for these) | not measured |
| 8 `AA6.JT` x2 (20 ft long) | crosses the right wing (28), 0.25 left | wing clip |
| 8 `GBU30.JT` x2 | 0.62 | |
| 8 `GBU12.JT`, `GBU16.JT`, `AS30.JT`, `AGM88.JT`/`AGM45.JT`, `AIM7.JT`/`AIM7E.JT` x2 | 0.19 to 0.28 (the missiles 0.06 or crossings 8 to 14 left) | marginal |
| 8 every other (`AIM-9M`, `AIM-9X`, `AIM-9B`, `AIM-120`, `AA-2`, `AA-8`, `AA-11`, `R550`, `PL-7`, `AGM-65`, `AGM-84`, `AS-15`, `AT-12`, bombs and rocket pods of the catalog) | 0.00 to 0.01 (preview metric; the triangle analysis ran for `AIM-9M`, `AA-11`, `AGM-65G` and the Mk 82, all clean) | clean where analysed |

The heaviest case (`F350` tank, row 7 `GBU30` x8, row 8 `GBU30` x2) is the row 7
and row 8 entries above and adds no tank contact (the tank is 4 ft from the
bombs). Pictures: the GBU-30 and the AGM-88 loads pile into the belly flank and
into each other (frames below).

### What the real aircraft has (plausibility only, cited)

The F-15E has a centreline station and two wing pylons, and conformal fuel
tanks with six tangential bomb stations each (three inboard, three outboard;
the inboard fore and aft stations take 2,000 lb stores, the outboard ones
1,000 lb), plus the two LANTIRN stations just aft of the intakes (navigation pod
on one, targeting pod on the other); the aircraft can carry three 610 gallon
tanks. Sources: the [F-15E Strike Eagle article](https://en.wikipedia.org/wiki/McDonnell_Douglas_F-15E_Strike_Eagle)
and [Air Power Australia's F-15E study](https://www.ausairpower.net/TE-F-15E-Strike-Eagle.html),
read 2026-11-10; one search, and no source for a 12 Mk 82 load was found.
Against the decoded rows: row 6 is a real station (the record's tank is 350
gallons, the real one 610); row 7's 6.3 ft out and 12 stores is a stand-in for the
conformal tank stations, which the shape does not hold
([F15E-2](../formats/aircraft-variety.md#f-15e-shape-damage-looks-and-decals-f15e-2-2026-11-10));
row 8 sits 14.3 ft out where the real pylon is about 9 to 10 ft of 21 (the
F-15C's was moved to 11 ft by John on 2026-10-20). The real wing pylon takes
tanks and bombs; the record's wing row takes no tank.

### The F-15E pods

John's 2026-11-10 decision (F-15E plan decision 3): **the Pave Spike row draws no
pod body; its sensor stays.** The 2026-11-07 rule
([fixed pods draw only on a real station](#fixed-pods-draw-only-on-a-real-station-2026-11-07))
already gives exactly this, so no code changed: row 4 (`PAVESPK.SEE`, flag `0x8`,
position 0 / 0 / 0) lies inside the airframe, so it is a placeholder and
`Configuration::fixed_pods` is empty for the F-15E. Its mass (425 lb) and its
designator behaviour are unchanged (neither reads the drawn list). Row 3
(`AV8IR.SEE`, weight 0) is an internal FLIR and never drew anything. The retail
guard `a_fixed_pod_record_names_no_shape_and_draws_a_fitted_body`
(`crates/tore-world/src/pod_equipment_tests.rs`) now includes the F-15E
(`PAVESPK.SEE` on `F15E.PT` draws nothing) and passes against the user's install.
No pod is hung on a LANTIRN position: the decoded rows have no intake station and
none was invented (B7). A designator pod loaded through Load Ordnance (rows 6 and
7 carry the pod bit `0x400`, so the retail equipment rule accepts one) still draws
its authored model on the centreline or the conformal-style station; neither is a
LANTIRN position, and Pave Spike is not a real F-15E store. That stays as the
standing rule for every aircraft unless John says otherwise.

### Options for John (none chosen)

1. **Default load as it stands** (accept): the 0.06 ft entry is inside the rail
   tolerance; the fin-tip touch of the rack's third bomb is the generic rack
   layout and affects every aircraft with three on a rack.
2. **Aft inboard bomb entry and the gear.** (a) The Mirage 2000 precedent
   (`rail_drop`, 0.1 ft) lowers the rails and clears the 0.06 ft entry but takes
   the bombs closer to the gear (0.32 to about 0.22 ft). (b) A fitted
   `station_move` for row 7 (for example 0.5 ft further out, 6.8 ft, or the aft
   rack forward) moves the bombs out of the leg's sweep and off the sloping flank,
   as the F-15C's wing pylon was moved. (c) Accept: the leg passes through the
   aft inboard bomb for part of a second while the gear moves.
3. **Alternatives that clip hard.** Row 7 `GBU29`/`GBU29P` x4 and `GBU30` x8 (about
   2 ft into the belly), `AGM88`/`AGM45` x10 (1.0 ft) and the weaker cases (`AT2`,
   `GBU38`, `CBU89`): refuse them on row 7 as the Mirage 2000's belly pairs refuse
   the 350 and 500 gallon tanks; or cap the count; or accept for large bombs (the
   real inboard stations take 2,000 lb stores but not in this fit). Row 8 `AA6`
   crosses the wing: refuse it or accept.
4. **Row 8 position.** 14.3 ft out as decoded, or about 11 ft like the F-15C
   (closer to the real 9 to 10 ft), a fitted `station_move`.
5. **Pods on rows 6 and 7:** leave the retail pod rule, or refuse a designator pod
   on row 7 (the conformal-style station); the real aircraft carries no Pave Spike.

### Frames and provenance

Frames (`.local/m3-aircraft/frames/F15E-5/`, png): `default-gear-{down,up}-{side,front,rear,below,quarter}`
(free flight, default load, zoom 3, small instrument layout, noon; the below look
is dark), `default-gear04-*` (gear at 0.4, the transit case), `raw-default/`
(stores preview, loaded views and close-ups), 3x crops `crop3x-cft-rack-aft-front`,
`crop3x-cft-bombs-side`, `crop3x-tank-centreline-front`, `crop3x-wing-aim120-front`,
`crop3x-gear04-*`, and the clipping cases `clip-crop3x-GBU30x8-belly-front`,
`clip-crop3x-GBU30x8-side`, `clip-crop3x-HARMx10-belly-front`,
`clip-crop3x-GBU38x12-side` (with `raw-gbu30/`, `raw-agm88/`, `raw-gbu38/`).

Provenance: rows, masks, counts and defaults **spec-derived** from `F15E.PT`;
the placement is the existing opinionated pylon rules; the measurements, the
transit finding, the comparison and the stop are agent findings of 2026-11-10;
the real-aircraft facts are cited from public pages for plausibility only; the pod
handling is John's decision of 2026-11-10 already satisfied by the 2026-11-07
rule. The audit pins do not move (no row, mask, count or default changed).

### The F-15E decisions (F15E-5, 2026-11-10, round 2)

John answered the options above on 2026-11-10 ("the F-15 do recs"). All three
station changes are **opinionated (John, 2026-11-10)**, each with a unit test;
the text above is the stop report as written and is not rewritten.

- **A1, rail drop.** The bomb pair (row 7) takes the Mirage 2000's 0.1 ft rail
  drop (`rail_drop`, `crates/tore-sim/src/combat/hardpoints.rs`; the decoded
  position is kept, only the surface the rails read drops). Test
  `the_f15e_bomb_pair_hangs_a_tenth_of_a_foot_lower_at_the_decoded_position`.
  Re-measured with the same triangle analysis: the default 12 Mk 82 now enter
  the airframe by **0.00 ft** (the aft inboard bombs were 0.06 and 0.02 ft in,
  now 0.049 ft clear); the lowest bomb stays 3.24 to 3.82 ft above the ground;
  the aft inboard bomb is still 0.32 ft from the main gear (the drop does not
  change the gear distance); the rack fin-tip touch (0.005 ft, 20 crossings a
  rack) is the generic rack layout and is unchanged; **the gear leg still
  crosses the aft inboard bomb while retracting** (28 crossings at gear 0.4,
  0.017 ft), which John's answer did not address and which stays an open item.
- **A2, large stores refused on row 7.** Normal loading (Cheat unchanged)
  refuses the stores that still enter the intake flank by more than about
  0.3 ft after the drop (`weapon_allowed` and `F15E_BELLY_REFUSED` in
  `crates/tore-formats/src/aircraft_stations.rs`, used by `Loadout::capacity`;
  test `the_f15e_refuses_the_large_stores_on_its_bomb_pair_and_the_aa6_on_its_wing_pair`).
  Measured entries after the drop, worst bomb of the row:

  | Store | Entry after the drop (ft) | Result |
  | --- | ---: | --- |
  | `GBU29.JT`, `GBU29P.JT` (x4) | 2.04 | refused |
  | `GBU30.JT` (x8) | 1.83 | refused |
  | `AGM88.JT`, `AGM45.JT` (x10) | 0.92 | refused |
  | `AT2.JT` (x12) | 0.46 | refused |
  | `GBU38.JT` (x12) | 0.32 | refused (just over the line) |
  | `CBU89.JT` (x10) | 0.24 | kept |
  | `MK20.JT`, `MK82HD.JT` | 0.13 | kept |
  | `RBK250.JT`, `CBU87.JT`, `RBK500.JT`, `B8.JT` | 0.01 to 0.07 | kept |
  | `MK82.JT`, `MK84.JT`, `FAB250/500/1000.JT`, `MK82P3.JT`, `MK82P.JT`, `MK82AIR.JT`, `B13.JT`, `LAU10.JT`, `LAU61.JT` | 0.00 | kept |

  So **seven** stores are refused on row 7: `GBU29.JT`, `GBU29P.JT`, `GBU30.JT`,
  `GBU38.JT`, `AGM88.JT`, `AGM45.JT` and `AT2.JT`. Several kept stores still have
  fin crossings on the racks (the generic three-on-a-rack layout, up to 116) and
  sit 0.02 to 0.05 ft from the main gear (`CBU87`, `CBU89`, `RBK`, `B8`, `LAU`);
  those are not part of John's line and are left as measured
  (`.local/m3-aircraft/checks/F15E-5/after/`).
- **A3, no AA-6 on the wing row.** Row 8 refuses `AA6.JT` in normal loading (the
  20 ft body crossed the wing). The wing row's other measured cases are kept
  (`GBU30` 0.62 ft, `GBU12`/`GBU16`/`AS30` about 0.2 ft, `AGM88`/`AGM45`/`AIM7`
  marginal): John asked only for the AA-6.
- **Counts.** The audit pin `WEAPON_PLACEMENTS` moves 4,652 to 4,644 (eight
  placements: seven on row 7, one on row 8; `F15E.PT` 65 to 57, catalog 51 to 48
  normal, 83 Cheat unchanged); tank, Cheat-only, pod, record, station and roster
  counts do not move. The audit now also checks that a refused store has capacity 0
  on its row. The `menus-ordnance-availability` battery line reads 4644. See the
  [ordnance availability baseline](../baselines/ordnance-availability.md#f-15e-stations-f15e-5-2026-11-10).
- **Flaps (E1).** Not a station change: the F-15E flaps deflect 30 degrees,
  documented in [F-15E control surfaces](../formats/aircraft-control-surfaces.md#f-15e-control-surfaces-f15e-3-2026-11-10).
- **Frames** (`.local/m3-aircraft/frames/F15E-5/`, prefix `final-`):
  `final-default-{below,side,quarter}` (default load, gear down),
  `final-crop3x-belly-rack-dropped-front` and `final-crop3x-belly-bombs-dropped-side`
  (the dropped belly row, 3x), `final-flaps{0,30}-{tailabove,rearlow}` and
  `final-crop3x-flaps0-vs-flaps30-*` (flaps neutral against 30 degrees; raw set
  `raw-final/`).

Still open (not decided): the gear leg crossing the aft inboard bombs in transit,
the wing row's position (14.3 ft out against the real 9 to 10 ft), and a designator
pod loadable on rows 6 and 7 through Load Ordnance.

## The Saab Gripen (GRI-5, 2026-11-10)

Measurement only, **stopped on clipping** as John's standing rule (2026-11-07,
confirmed for the batch 2026-11-10) requires: no station table, `station_move`,
pylon or mask was authored, and no count moved. `GRIPEN.PT` has nine decoded
rows: four fixed sensor rows (visual sensor, radar, jammer and the AAS-38 pod
row), the internal BK27 gun row and **four editable carrier rows**, the seven
store stations of the real aircraft (centreline, two wing pairs, the wingtip
pair; [source review](../formats/aircraft-variety.md#gripen-hardpoints)). The
shape is true size (no scale), so the decoded rows hang under the standard rules
of 2026-10-13 with the shape as it is. The check tool, the temporary dump hook
(removed again, `git status` shows only a test) and every dump are local.

| Row | Location | Decoded x / up / fwd, ft | Hangs at (centre, ft) | Default | Hang used |
| ---: | --- | --- | --- | --- | --- |
| 5 | Fuselage, centreline | 0 / -6.67 / 0 | 0 / -3.26 / 0 (tank 10.28 ft long, 1.58 ft wide, 2.30 ft under the reference) | `F150.GAS` x1 | 2 inch rail under the belly (surface -2.30 to -2.0) |
| 6 | Wing, paired | 5.00 / -5.00 / -4.67 | +-5.00 / -2.21 / -4.67 | `AGM65G.JT` x2 | 1 ft wing pylon (surface -0.66 to -0.83) |
| 7 | Wing, paired, 2 a side | 10.00 / -5.00 / -7.00 | +-9.37 and +-10.63 / -1.96 / -7.00 | `AIM120.JT` x4 | one pylon a side with a twin bar, 1 ft (surface -0.67) |
| 8 | Wingtip, paired | 14.67 / -0.67 / -8.67 | +-13.67 / -0.67 / -8.67 | `AIM9M.JT` x2 | tip rule: centre line on the tip's edge, no pylon |

Rows 0 to 3 and 4 draw nothing (sensors, the pod placeholder, the internal gun).

**Default load, against everything** (triangle level: every store against the
airframe gear up, the gear gear down, every other store and pylon, the ground
plane 5.33 ft under the reference; surface distances sampled, good to about
0.05 ft; the airframe is the neutral pose):

| Store | Airframe, gear up | Neighbours | Gear down | Ground |
| --- | --- | --- | --- | ---: |
| F150 tank, row 5 | clean, 0.16 ft from the belly | more than 1 ft from the Mavericks | **crosses the left main strut plate** (16 edge crossings, the plate's inner 0.25 ft inside the tank body), right strut 0.09 to 0.13 ft clear, wheels 0.82 ft | 1.28 ft |
| AGM-65 x2, row 6 | clean, 0.59 to 0.64 ft | clear | **cross the open main gear door** (36 right, 44 left crossings, door wall x 4.3 to 5.0 ft against a body x 4.17 to 5.83 ft) | 2.29 ft |
| AIM-120 x4, row 7 | clean, 0.67 ft | twin fins 0.02 ft apart (the standard twin bar, as the F/A-18) | clear (3 ft) | 2.76 ft |
| AIM-9M x2, row 8 | **0.15 ft** upper fins into the tip (34 and 15 crossings) | clear | clear | 3.93 ft |

Canards (neutral): every one of the 91 stores measured stays 2.56 ft or more from
the canard plates (nearest: AA-6 on row 6); a deflection of the 0.35 rad bound moves
a plate about 0.8 ft at most (estimate from the chord, not run), so no contact is
expected. Stores against stores: nothing closer than 1 ft except the twin bars
(AIM-120 pair 0.02 ft, the four-Sidewinder, four-Maverick and four-Mk 82 loads 0.03,
0.03 and 0.47 ft), and the eight whole-aircraft loads (all F150, all F500, all Mk 84,
all AA-9, all AIM-9M, all AGM-65G, all Mk 82, all AS-16) show no new contact. Nothing
touches the ground in any case (lowest: a GBU-10 on the centreline, 0.20 ft above it).

**Clipping, three cases for John (not fixed):**

1. **Inner wing row 6 against the open main gear door (every store, gear down only).**
   The row's decoded x (5.0 ft) is where the retail door wall's outer edge stands
   (the GRI-4 rig keeps the source pose, root plate x 3.0 to 4.3 ft, wall x 4.3 to
   5.0 ft). Every one of the 17 stores measured on row 6 crosses it (a slim AIM-9M
   only at its nose, 4 crossings; Mk 84 104, AGM-88 108). Gear up the row is clean.
2. **Default centreline tank against the left main strut plate (gear down).** The
   strut plates are zero-thickness sheets (in the plane 3.67 ft behind the reference)
   whose inner edges stand at 0.67 ft (right) and 0.33 ft (left, the source's one unit
   asymmetry) from the centreline; the default tank is 0.58 ft to a side at that plane
   (0.79 ft at its widest), so the left plate's inner 0.25 ft is inside it and the
   right plate clears it by 0.09 ft. Mirroring the left plate would clear the default
   tank by 0.09 ft. Wider stores cross both plates (F250 0.94 ft to a side, F350 and
   F500 1.18 ft, 56 crossings; the Mk 84 and the B-13 pod are slim and cross at their
   noses, 10 and 16), so this is not particular to the default tank.
3. **Wingtip AIM-9M, 0.15 ft** (and larger on other tip missiles, below). This is the
   standard tip rule meeting a thin tip; it is below every reviewed default load
   (0.27 ft MiG-25 to 0.72 ft Su-27), so it is recorded and not a stop on its own.

**Options for John (not applied).** Case 1: (a) accept (the door is a retail
zero-thickness plate, the contact exists only on the ground and in the landing
phase); (b) move row 6 outboard so the body clears the wall: an estimate from the
plates is about 1.2 ft (centre 6.2 ft out, fin half span 1.18 ft), which narrows the
gap to row 7's pylon (10.0 ft) to 3.8 ft (not run); (c) change the gear-down door
pose in the GRI-4 rig (a rig decision, the doors are fitted); (d) mask the heaviest
stores off row 6 (does not remove the default's contact). Case 2: (a) accept; (b)
mirror or widen the strut plates in the rig (fitted geometry; the retail wheels are
1.3 to 2.0 ft from the centreline; mirroring clears the default tank by 0.09 ft but
not F250 and larger); (c) keep only the 150 gal tank on row 5. **Nothing was moved:** no real
Gripen station is closer, the shape is retail.

**Alternatives the Load Ordnance page offers** (`TORE_STORES_PREVIEW_CATALOG`: 64
choices on each of rows 5, 6 and 7, 10 on row 8; each measured against the default
others: 61 single-row loads, representatives for the long, wide and fat shapes,
plus the eight whole-aircraft loads and the default):

| Row | Clean against the airframe (gear up) | Enters the airframe | Gear down | Ground |
| --- | --- | --- | --- | --- |
| 5 centreline | F250, F350, F500, Mk 82, Mk 84, FAB-1000, B-13, LAU-61 | AGM-65G 0.18 ft, AIM-120 0.13, AS-16 0.13, AIM-9M 0.33, AGM-84E 0.35, AA-9 0.37, AGM-88 0.66, GBU-10 0.60, AA-6 0.74, AM-39 0.97 (their pylon is a 2 inch rail on a belly that falls away) | 12 of 18 cross a strut plate (10 to 92 crossings); AIM-120 0.84 ft, AS-16 0.18, AIM-9M 0.21, LAU-61 0.98, FAB-1000 and Mk 82 0.04 ft clear | lowest 0.20 ft (GBU-10), F500 0.50 |
| 6 inner wing | tanks, Mk 82, Mk 84, FAB-1000, B-13, AIM-120, AIM-9M, AA-9, AS-16 | AA-6 0.16, AM-39 0.43, AS-30 0.30, AGM-88 0.12, GBU-10 0.09 | all 17 cross the door wall (case 1) | 0.96 (GBU-10) and up |
| 7 outer wing, twin | tanks, Mk 82, Mk 84, FAB-1000, B-13, AIM-9M, AGM-65G, AA-9, AS-16, GBU-10 | AA-6 0.17, AM-39 0.26, AS-30 0.09, AGM-88 0.07 | clear (3 ft) | 1.00 and up |
| 8 wingtip | none | AIM-9M, AIM-9B, AA-2 0.15; AA-11 0.20; AA-11B 0.28; AA-8 0.45; AT-12 0.71; AIM-9X 0.74; R-550 1.08; PL-7 1.33 (the fins of the larger shapes stand in the thin tip) | clear | 3.7 and up |

Twin bars on row 7: AIM-9M x4 0.03 ft, AGM-65G x4 0.03, AGM-88 x4 0.05, Mk 82 x4
0.47 ft between the two stores of a side (fins nearly touching on the first three).
The centreline row's alternative missiles are all masks the retail record admits;
none is a default and the real aircraft carries a tank, a bomb or a pod there, so
the entries are listed, not decided. The large store on row 6 or 7 never meets the
canards.

**The AAS-38 row draws no body (John, 2026-11-10).** Row 3 is flags `0x8` at the
reference point, the A-10 pattern, so under
[the 2026-11-07 rule](#fixed-pods-draw-only-on-a-real-station-2026-11-07) it is a
placeholder: nothing is drawn (stores preview and the triangle dump hold no pod
geometry; `frames/GRI-5/crop3x-aas38-row-no-body.png` looks from the starboard side
at the intake and finds none), while the sensor stays: the FLIR sight is read from
the row (`fixed_sight`), not from the drawn list. A guard was added: the existing
retail test `a_fixed_pod_record_names_no_shape_and_draws_a_fitted_body` now also
covers the Gripen (no drawn pod, the sight still resolves, the only drawn pod is
still the Rafale's). **Mass:** the loader's external-equipment sum skips rows
flagged `0x8`, so the row's 350 lb was never counted for the Gripen (the A-10's and
F-16C's pod rows are the same); that is the existing behaviour, unchanged here, and
the 2026-11-07 text above ("keeps its mass accounting") means it is as it was. If
John wants the 350 lb counted it is a GRI-6 change. The real FLIR sits under the starboard intake trunk (the game's own
encyclopedia text, `GRIPEN.INF`); no pod is hung there.

**Findings for John (nothing moved).**
- **Wingtip rail offset.** The record's x is 44 units (14.67 ft), the shape's tip
  edge 41 units (13.67 ft). The standard tip rule already centres the missile on
  the tip's edge (store centre 13.67 ft, fin tips 14.40 ft), so the 1 ft excess is
  absorbed and no move is needed; the 0.15 ft entry is the residual.
- **Gun side.** Row 4 is at x +4 units (+1.33 ft, starboard), 0.67 ft up, 12.3 ft
  forward; the fuselage section there spans x -2.4 to +2.7 ft, so the muzzle origin
  is 1.1 ft inside the starboard skin. The real BK27 is on the port side of the lower
  front fuselage (`GRIPEN.INF`). Options for GRI-6: keep the retail side (nothing is
  drawn; only the muzzle origin of the shots), mirror it to x -4, or move it to the
  skin. Not changed here.

**Real Gripen, for plausibility only** (the decoded rows decide): the in-game
encyclopedia lists seven store stations (two wingtip, two under each wing, one
centreline), which the record matches. Public pages count eight on the JAS 39C/D (two
wingtip, four underwing, one centreline, and two further fuselage stations under the
intake trunks; the Gripen NG demonstrator "also allowed for two additional hardpoints
to be added on the fuselage underside", Wikipedia, "Saab JAS 39 Gripen", accessed
2026-11-10; airforce-technology.com, Gripen project page, and a defencetalk
forum thread, same date: the sources disagree on seven or eight, none gives lateral
offsets). The record's wing pairs are the real inner and outer pylon pairs and the
wingtip pair the real AIM-9 rails; no designator pod station exists in the record.
No station was added.

Provenance: rows, masks, capacities, defaults spec-derived from `GRIPEN.PT` (GRI-0);
pylon and rail depths and the tip rule the opinionated rules of 2026-10-13; the AAS-38
handling John's decision of 2026-11-10 (opinionated) on the 2026-11-07 rule; the
measurement, the clipping classification and the options are agent findings,
2026-11-10; the estimates marked "not run" are agent estimates; real-aircraft station
counts from public pages (secondary, conflicting).

### Gripen station decisions (John, 2026-11-10)

Round 2 of the batch decisions (opinionated, John, 2026-11-10): the
Gripen station measurements above are accepted as they stand ("keep"); no

## The F-5E Tiger (F5E-5, 2026-11-10)

Measurement only, no station move, rail drop or mask adopted. `F5E.PT` has seven
rows: three fixed sensor rows (visual sensor, radar, and the jammer that B5-2
cleared), the internal M61 (560 rounds), the centreline tank row, one wing pair
and the wingtip pair. The shape is drawn 1:1 (no scale, John 2026-11-10), so the
decoded rows are used as recorded: feet are source units divided by three. They
hang under the standard rules of 2026-10-13 (1 ft wing pylon, 2 inch centreline
rail, a store centred on the tip's edge for a Wingtip row). No `station_move`
exists for the F-5E and none was added: per John's standing rule (2026-11-07)
the slice stops on clipping with the options below. **John kept the record's one
wing pair (decision 15) and the B5-2 empty wing default; no second pair and no
sensor or designator pod was added** (the record has no pod row, and the real F-5E
carries no such pod on a station).

| Row | Location | Decoded x / up / fwd, ft | Default | Hang used | Hangs at (centre, ft) |
| ---: | --- | --- | --- | --- | --- |
| 0 to 2 | fixed sensor, radar, jammer | 0 / 0 / 0 (jammer 0 / 0 / -16.7) | `VIS340.SEE`, `F104R.SEE`, none | nothing drawn | none |
| 3 | internal gun | 1.3 / -1.7 / 13.7 | `M61.JT` 560 rounds | internal, not drawn | none |
| 4 | Centerline | 0 / -4.67 / 0 | `F150.GAS` x1 | 2 inch rail under the belly (belly -1.67) | 0 / -2.62 / 0, tank 10.28 ft long, 1.58 ft wide |
| 5 | Wing, paired (4 stores, two a side) | 7.33 / -4.67 / 1.00 | empty (B5-2) | 1 ft pylon at 7.33 ft out, twin rail, wing underside -1.33 | stores 1.32 ft apart, centres +-6.67 and +-7.99 / -2.54 / 1.00 (an AIM-9B) |
| 6 | Wingtip, paired | 14.33 / -1.00 / -1.00 | `AIM9B.JT` x2 | tip rule: centre line on the tip's edge, no pylon | +-14.33 / -1.17 / 0.21 (missile 9.3 ft long) |

Measured with `tore-app --stores-preview` (CPU, audit and catalog flags), plus a
local triangle-level check of every store against the airframe, the gear (gear
down, the F5E-4 rig, the ground plane 6.0 ft under the reference), the pylons,
every other store and, for the wing row, the drooped flap (edge and face
crossings, the audit's vertical entry, and surface distances sampled at 0.12 ft,
good to about 0.05 ft). The check tools and their dumps are local only
(`.local/m3-aircraft/checks/F5E-5/`); a temporary dump hook in
`stores_preview.rs` was reverted and nothing in the tree changed but these docs.

The wing is a flat panel 0.33 ft thick (undersides at -1.33 ft, tops at -1.0 ft)
out to the tip at 14.33 ft, with a 0.67 ft wide, 4.33 ft long tip rail plate
under the outer end. The belly under the tank is at -1.67 ft.

**Default load (F150 tank on the centreline, AIM-9B on each tip):**

| Station | Entry into the airframe | Edge crossings | Nearest neighbour | Gear down | Ground |
| --- | ---: | ---: | --- | --- | ---: |
| Tank, row 4 | none (0.17 ft off the belly, the 2 inch rail) | 0 | clear | 3.0 ft or more | 2.58 ft |
| AIM-9B, row 6, right | **0.32 ft** (fins and body in the 0.33 ft thick tip) | 122 | clear | 3.0 ft or more | 4.19 ft |
| AIM-9B, row 6, left | **0.32 ft** | 148 | clear | 3.0 ft or more | 4.19 ft |

**Clipping, one case on the default load, for John (stop-and-report rule of
2026-11-07; not fixed):**

1. **Wingtip AIM-9B (row 6), 0.32 ft.** The missile's centre line sits on the
   tip's edge at mid-thickness of the 0.33 ft tip (y -1.17 between the wing's
   -1.0 and -1.33), so half its body and the upper and lower fins stand inside
   the thin wing tip and the drawn tip rail plate. It is the tip rule meeting a
   very thin tip, as on the Mirage 2000 (1.21 ft there). The value sits inside
   the range of the default loads already reviewed (0.27 ft F-14 and MiG-25, 0.29
   ft F-15, 0.42 ft F-16, 0.48 ft MiG-31, 0.72 ft Su-27) but is not zero. The
   record's tip position and the shape's tip agree (43 units is 14.33 ft, the
   wing's own edge), and the real F-5E does carry the AIM-9 on a wingtip rail at
   this place (cited below), so the row's position is real; only the missile's
   depth against a thin tip clips.
   Options (none applied): (a) accept 0.32 ft; (b) lower the tip rail (a rail
   drop of the kind the Mirage 2000's belly row uses, extended to Wingtip rows)
   about 0.36 ft to put the body under the slab (estimated from the 0.33 ft slab
   and the 0.2 ft body radius, not measured; the fins, 0.65 ft each way, would
   still cross unless the drop is about 0.8 ft, which would hang the missile
   visibly under the tip); (c) hang it on a 1 ft wing pylon inboard (MiG-31
   `Hang::Wing`), which moves the missile off the real rail position. The
   centreline tank is clean (0.17 ft is the 2 inch rail itself).

Nothing in the default load touches the gear, the ground, the flap or another
store. With the ailerons moved the outer end of each aileron strip (x 10.3 to
14.3 ft) turns through the lower part of the tip missile, which already overlaps
it; the flap strips end at x 10.3 ft and are clear (3.0 ft).

**Alternatives the Load Ordnance page offers** (record masks unchanged; the live
list is the same one the page prints). Row 4 takes only the `F150.GAS` tank. Row 6
takes ten small A2A records (`AA2`, `AA8`, `AA11`, `AA11B`, `AIM9B`, `AIM9M`,
`AIM9X`, `AT12`, `PL7`, `R550`). Row 5 takes 64 records: the 150, 250, 350 and 500
gallon tanks and 60 missiles, rockets and bombs; each was loaded to its capacity
(two a side, or one a side for the long and heavy ones) against the default tank
and tip missiles. Distances are to the airframe (wing, fuselage), the main gear sheet,
each other and the ground.

| Row 5 load | Result |
| --- | --- |
| Clean (no entry, no crossing, at least 0.1 ft from the wing): `AA2`, `AA8`, `AA9`, `AA11`, `AGM45`, `AGM65A/G`, `AGM84A/E`, `AGM88`, `AIM120`, `AIM9B`, `AIM9M`, `AS7`, `AS14`, `AS16`, `AT2`, `B8`, `B13`, `CBU87`, `F150`, `F250`, `F350`, `F500`, `FAB250/500/1000`, `GBU29`, `GBU29P`, `LAU10`, `LAU61`, `MK20`, `MK82` and variants, `MK84`, `PAVEWA3`, `RBK250/500` | Gear 0.38 ft (`B8`) to 1.95 ft, ground 0.37 ft (`GBU29` family, the lowest) to 3.1 ft; the two missiles of a pair 0.01 to 0.05 ft apart (touching, no crossing); every store is more than 1 ft from the centreline tank |
| Enter the wing | `AA6` 0.17 ft (26 crossings), `AM39` 0.23 ft (24), `AS30` 0.09 ft (20), `GBU10`, `GBU10A` and `PAVEWAY` 0.12 ft (12), `GBU12` 0.07 ft (16), `GBU16` 0.14 ft (28) |
| Fins cross the other store on the same pylon | `AA11B` 30, `AA12` 6, `AIM9X` 16, `AS15` 52, `AT12` 40, `CBU89` 16, `GBU30` 28, `GBU38` 28, `MICA` 16, `PL10` 16, `PL7` 63, `R530` 84, `R550` 59 (crossings per pair, no airframe entry) |
| Touch the main gear sheet (gear down) | `AIM7` and `AIM7E` 20 crossings (the Sparrow, the retail default of this row before B5-2), `GBU16` 28, `GBU30` 8; `GBU12` is 0.15 ft off it |

Row 6 alternatives against the tip (entry into the thin tip): `AIM9B` 0.32 ft (the
default), `AA2` 0.32, `AIM9M` 0.32, `AA11` 0.36, `AA11B` 0.17, `AA8` 0.62, `AIM9X`
0.90, `PL7` 1.17, `R550` 1.25 ft, and `AT12` no entry but 56 crossings. The
default missile is the least deep of its row. None comes near the gear or the
ground (3 ft or more from the gear, 3.8 ft or more above the ground).

**Flap droop (computed, F5E-3 hinge, 0.40 rad at full flap).** The wing-row stores
hang under the flap strip's chord (x 2.3 to 10.3 ft, hinge 1.3 to 1.5 ft forward
of the trailing edge). A flap at 100 percent comes 0.14 to 0.16 ft from an AIM-9B,
0.27 ft from an AIM-7, 0.42 to 0.70 ft from `MK82`, `MK84` and `F500`, and crosses
`R550`, `AGM45`, `GBU12` and `AA6` (28 to 46 crossings). A pylon's rear edge is
0.01 to 0.03 ft from the drooped flap. The flap droop is a fleet-fitted constant
(`FLAP_DROOP`); this is listed, not changed.

**Gear down and ground.** The rig's main gear sheets stand at 4.67 ft and 5.0 ft
out; the wing pylon at 7.33 ft (stores 5.9 to 8.6 ft out) clears them by 1.1 to
1.4 ft for an AIM-9B, and the centreline tank (0.79 ft each way) by more than 3 ft.
Parked on a runway (`--ground-start 2`) the default load stands on all three
wheels, the tank 2.58 ft above the ground, the tip missiles 4.19 ft. The lowest
alternative store is the `GBU29` family, 0.37 ft above the ground.

Real F-5E for plausibility only (the decoded rows decide): seven external stations,
two wingtip rails, four underwing pylons and one under the fuselage (Wikipedia,
"Northrop F-5", Specifications; "five external hardpoints and two wingtip rails",
aerospaceweb.org F-5 page, accessed 2026-11-10; no source gives the lateral
offsets). The record has three store positions (the centreline, one wing pair and
the tip pair), so two of the four real underwing stations have no row, which John
decided to keep (2026-11-10). The Sidewinder on the tip rail is the real F-5E's own weapon.

The row positions, masks, capacities and defaults are spec-derived from
`F5E.PT` (F5E-0, B5-1) and B5-2; the pylon and rail depths are the existing
opinionated rules of 2026-10-13; the keep-one-pair choice is John's (2026-11-10).
The measurement, the comparison, the flap check and the stop are agent findings,
2026-11-10. The ordnance audit pins did not move (no placement count changed).
Frames: `.local/m3-aircraft/frames/F5E-5/`.

### F-5E station decisions (John, 2026-11-10)

Round 2 of the batch decisions (opinionated, John, 2026-11-10): the
F-5E station measurements above are accepted as they stand ("keep"); no
station, mask, pylon or rail drop changes, and the clipping cases listed above
remain as recorded.

## The Mirage F1 (MF1-5, 2026-11-10)

Measurement only, no station table, rail drop, mask or pylon code adopted:
**the slice stops on clipping** (John's standing rule, B7), with the options
below for John. `MF1.PT` has four fixed or internal rows (visual sensor, radar,
the internal `AV8.ECM` suite and the DEFA gun, rows 0 to 3) and four external
rows. The shape is scaled per axis to real size ([aircraft shape
scale](../formats/objects-and-shapes.md#aircraft-shape-scale-2026-11-09), factors
1.0335 right, 1.0546 forward, 1.1804 up, fitted), and the decoded rows scale with
it (`placed_on`). The stores hang under the standard rules of 2026-10-13. No
`station_move` exists for the Mirage F1 and none was added. No sensor or
designator pod was added or kept (B5-2 already cleared the centreline's
`ALQ167.ECM` default).

| Row | Location | Decoded x / up / fwd, ft | Scaled x / up / fwd, ft | Default | Hang used | Hangs at (centre, ft) |
| ---: | --- | --- | --- | --- | --- | --- |
| 4 | Centerline | 0 / -6.0 / 1.7 | 0 / -6.33 / 1.97 | empty (B5-2) | 2 inch rail under the belly | one store; a `F250.GAS` centre at 0 / -3.9 / 1.97, 12.2 ft long |
| 5 | Wing, paired, four stores | 2 / -5.7 / 0 | 2.07 / -5.98 / 0 | `F250.GAS` x4 | twin-rail pylon at 2.07 ft, one store each side of the bar (1.18 ft off it) | tanks at 0.89 and 3.24 ft a side, y -3.8, 1.23 ft above the ground |
| 6 | Wing, paired | 7 / -3.3 / -1 | 7.23 / -3.52 / -1.18 | `R550.JT` x2 | 1 ft wing pylon under the wing | +-7.23 / -1.12 / -1.18 |
| 7 | Wingtip, paired | 13.7 / 1 / -3.7 | 14.12 / 1.05 / -4.33 | `R550.JT` x2 | tip rule: centre line on the shape's tip edge, no pylon | +-13.78 / 0.70 / -4.33 |

The default load is the fixed gun, **four `F250.GAS` under the belly**, two R550
under the wing pylons and two R550 at the tips (the Weapons panel reads
`250 DEFA`, `4 R-550`, `+ EXT 6600 LBS`). The count of four is the record's own
(row 5 holds four), so the default carries two tanks a side where the real F1
carries one (Jane's text in `MF1.INF`: two 1,200 L tanks on the inner wing
pylons, one 2,200 L tank on the underfuselage station).

Measured with `tore-app --stores-preview` (CPU, audit and catalog flags, then the
GPU frames) plus a triangle-level check of every store against the airframe
(gear up), the gear plates (gear down, the MF1-4 rig, ground plane 5.98 ft
under the reference), every pylon and every other store (edge and face
crossings, vertical entry as the audit measures it, surface distances sampled at
0.12 ft), as for the Mirage 2000. The default load and 71 more cases were run
(every record each row offers, row by row; the local scripts and their output
are not committed).

Default load:

| Station | Entry into the airframe | Crossings | Nearest neighbour | Gear down | Ground |
| --- | ---: | ---: | --- | --- | ---: |
| Tank, row 5, inner (x 0.89 ft) | none (0.13 ft clear) | 0 | **crosses the other side's inner tank (32 crossings)** | **crosses the main gear plate (24 right, 20 left crossings, touching)** | 1.23 ft |
| Tank, row 5, outer (x 3.24 ft) | none (0.86 ft clear) | 0 | 0.48 ft to the inner tank | 0.63 ft | 1.23 ft |
| R550, row 6, right | none (0.13 ft clear) | 0 | clear | 3.0 ft | 4.19 ft |
| R550, row 6, left | 0.05 ft (upper fin) | 20 | clear | 3.0 ft | 4.19 ft |
| R550, row 7, right tip | **1.48 ft (fins into the wing tip)** | 66 | clear | 3.0 ft | 6.01 ft |
| R550, row 7, left tip | **1.45 ft** | 56 | clear | 3.0 ft | 6.01 ft |

The audit's own `clip` lines read 1.48 ft (hardpoint 7) and 2.43 ft (hardpoint 5,
the inner tanks into the neighbours), against the 0.27 to 0.72 ft the reviewed
default loads read.

**Clipping, two cases for John** (not fixed):

1. **Belly tanks (row 5, four `F250.GAS`).** Four tanks hang on two twin-rail
   pylons at 2.07 ft each side of the centreline. A tank is 1.87 ft wide, so the
   two inner tanks (x 0.89 and -0.89 ft) overlap each other across the
   centreline by 0.05 ft each (32 crossings) and both stand in the main gear
   plates when the gear is down (touching, 0.01 ft). The two outer tanks are
   clean (0.63 ft off the gear, 0.86 ft off the belly). The frames show four tanks in
   a row under the belly with the wheels among them. The real F1 carries one tank
   a side. Not caused by the per-axis scale (the row is the record's own
   position); the Mirage F1's gear sheet (the retail shape, scaled) is narrow, so
   the inner pylon position meets it for any store.
2. **Wingtip R550 (row 7), 1.48 ft.** The tip rule centres the missile on the
   shape's tip edge, 0.70 ft up. The Magic's body and its upper fins stand inside
   the thin tip (66 and 56 crossings, 1.48 and 1.45 ft vertical entry). The
   position is the record's and the real F1 has wingtip rails (cited below), so
   this is the rule meeting a thin tip, not a mis-scaled row. It is the largest
   default-load entry of any reviewed aircraft (the F-5E's tip AIM-9B reads 0.32 ft
   and John accepted it).

The centreline row (empty by default), the wing pylon pair (row 6, the left R550
at 0.05 ft aside) and the ground clearances are clean.

**Options for John (not applied):**

- Row 5 (belly tanks): (a) keep the record's four (clipping stays); (b) default
  and capacity two (one a side), which fixes the tank-on-tank crossing but
  leaves the tank in the gear (16 crossings with `F250.GAS` x2 at 2.07 ft, 0.01 ft
  off the gear) and 0.21 ft from a centreline tank; (c) one tank a side moved
  outboard to the clean outer slot, about 3.24 ft (the measured clean spot: 0.63 ft
  off the gear, 0.86 ft off the belly; a `station_move` plus the capacity rule;
  no source gives the real lateral position, so the number is a measurement,
  not a citation); (d) the Mirage 2000's belly rail drop (0.1 ft) does not help
  here, the problem is sideways.
- Row 7 (wingtip): (a) accept 1.48 ft; (b) a fitted wingtip offset (outboard or
  down, no `rail_drop` exists for Wingtip rows; the size is not measured here);
  (c) a different carry rule for this row. It must not move to a wing pylon:
  the real F1 has tip rails.

**Alternatives the Load Ordnance page offers**, each with the other rows at the
default (one case ran four rows at once, so only the same row's metrics are
read). Row 4 takes one store: `F150.GAS` and `F250.GAS` (the 150 and 250 gallon
tanks; `F350.GAS` and `F500.GAS` are refused on the row by the existing mask) and
56 weapons (the mask is the Rafale template's). Of the 58 records, 22 are clean
against the airframe and gear (both tanks, the bombs, the racks and
`AA11B`, `AS7`, `B13`, `B8`, `AGM65A/G`, `GBU38`); 36 enter the belly by 0.05 to
0.71 ft (the missiles; `AM39` 0.71 ft) and 15 cross the gear (`AA6`, `AA9`, the
`GBU` pack, `AS14`, `AGM45/88`, `AIM7`). A centreline `F250.GAS` (12.2 ft long) alone
is 0.14 ft off the belly, 0.69 ft off the gear and 1.15 ft off the ground; with
the default four belly tanks it passes through them (148 and 156 crossings),
and with row 5 cut to two (one a side, x 2.07 ft) it clears them by 0.21 ft.

| Row 5 load (count) | Result |
| --- | --- |
| `F150.GAS` x4 / `F250.GAS` x4 (default) | tanks cross each other (32), inner tanks cross the gear (24, 20); outer clean |
| `F150.GAS` x2 / `F250.GAS` x2 | one tank a side at x 2.07 ft: crosses the gear (24, 24 for the 150 gallon; 16, 16 for the 250), 0.42 ft off the belly; 0.21 ft off a centreline `F250` |
| `F350.GAS` x2, `F500.GAS` x2 | cross the gear (16), 1.01 and 0.74 ft above the ground |
| 63 of 65 records | enter the belly (0.01 to 1.44 ft), cross the gear (58 records) or cross a neighbour; the only clean ones are the two ejector racks `MK82P3.JT` and `MK82P.JT` (two stores) |
| `R550.JT` x2 | 0.47 ft into the belly, crosses the gear (12, 14) |

Row 6 (the wing pair): 39 of the 47 records are clean (no entry, gear 3 ft away).
Eight enter the wing by 0.05 to 0.33 ft: `R550.JT` (left only, the default),
`PL7`, `PL10`, `AGM88`, `AGM45`, `AIM7`, `AIM7E` (0.06 to 0.15 ft) and `AS30` (0.33
ft). Row 7 (the wingtip rail, 10 records): all ten enter the tip, 0.50 ft
(`AIM9B`, `AIM9M`, `AA2`) to 1.48 ft (`R550.JT`); none touches the gear or the
ground.

**Real Mirage F1 for plausibility only** (the decoded rows decide): five external
hardpoints and two wingtip rails (Aerospaceweb, Mirage F1 page, accessed
2026-11-10; Jane's text in `MF1.INF`: centreline 2,100 kg, inner wing pylons 1,300
kg, outer wing pylons 550 kg, wingtip rails 150 kg for Magic or Sidewinder). The
record's one centreline row plus three mirrored pairs is the same seven
positions, so the station count agrees. The real centreline carries a 2,200 L tank
or a bomb or a pod, never a second store; **the centreline "takes two tanks" in
the B5-2 note meant two tank records (150 and 250 gallon), not two tanks at once:
the row holds one store.** No source gives the real pylons' lateral offsets, so
the measured clean slot above is a measurement of this shape, not a citation.

Provenance: rows, masks, capacities and defaults spec-derived from `MF1.PT`
(MF1-0, B5-1) with B5-2's opinionated empty centreline (John, 2026-11-10); the
per-axis scale opinionated (John, 2026-11-10) with fitted factors; pylon and rail
depths the existing opinionated rules of 2026-10-13; the measurement, the
comparison, the stop and the options agent findings, 2026-11-10; the real
aircraft's stations from public pages for plausibility only. The ordnance audit
pins did not move (no placement count changed).

## The Mirage F1 stations, finished (MF1-5b, 2026-11-11)

Update to [the Mirage F1 measurement above](#the-mirage-f1-mf1-5-2026-11-10),
which stays as the record of the retail layout and of the two clipping cases. John
answered both on 2026-11-11 (round 3 of the batch decisions, "follow recs"): item
M1, cap the belly row at two stores, one a side, and move it out to the measured
clean slot; item M2 (the coordinator's reading of "M1" as the F1 items), give the
wingtip Magic a fitted tip offset, outboard or down, the smallest that clears the
tip, never a wing pylon. Both are opinionated (John, 2026-11-11); every number
below is a fitted agent decision. The jammer (the record's `AV8.ECM`), the empty
centreline (row 4) and the per-axis scale are unchanged.

**What the code does.**

- **Capacity (M1).** `aircraft_stations::adjust_defaults` already clears the
  centreline pod when all eight source rows are exactly the retail ones; it now
  also sets row 5's `maxItems` from four to two (location Wing, paired, mask
  `0x7e5`, class 46, default `F250.GAS`; none of those change). The guard is the
  whole retail table, so an edited record keeps its own capacity, and repeating it
  is a no-op. The default `F250.GAS` count follows the capacity: the default load
  is two tanks. Every other row, mask and index is as decoded.
- **Position (M1).** `hardpoints::station_move` gives `(Mf1, 5)` a Wing station
  3.24 ft out and 0 ft forward on the standard 1 ft wing pylon (`Hang::AsRecorded`);
  the record's own position is 2.07 ft out. 3.24 ft is where the outer tank of the
  four hung and measured clean in MF1-5; no source gives the real lateral offset,
  so it is fitted. With two stores the row loads one a side (`sides` is 1 and 1),
  so the tank sits on the station itself, not 1.18 ft either side of a twin rail.
- **Tip offset (M2).** `hardpoints::tip_offset` adds 1.1 ft outboard (and 0 ft
  down) to the tip edge the tip rule reads, for the Mirage F1's row 7 only and only
  while it is the retail wingtip row (a Wingtip row of two stores). It is applied in
  `placed_on` to `Rack::tip`, which the drawing, the stores that leave the aircraft
  and the replay and session records already carry, so nothing else changed. There
  was no Wingtip `rail_drop`; this is the narrowest per-aircraft mechanism (one
  function and one constant, guarded to the record, like `rail_drop`). The stores
  stay on the tip rail: no pylon, no move to a wing station.

**Station table (scaled shape, ft).**

| Row | Location | Scaled right / up / fwd | Default | Hang | Result |
| ---: | --- | --- | --- | --- | --- |
| 0 to 3 | sensor, radar, `AV8.ECM`, gun | none drawn | `VIS340`, `F18R`, `AV8.ECM`, `DEFA` 250 | internal | nothing drawn, no pod (B7) |
| 4 | Centerline | 0 / -6.33 / 1.97 | empty (B5-2) | 2 inch rail | unchanged |
| 5 | Wing, capped at two | 3.24 / record height / 0 (was 2.07 and four stores) | `F250.GAS` x2 | 1 ft wing pylon, one a side | clean, see below |
| 6 | Wing, paired | 7.23 / -3.52 / -1.18 | `R550.JT` x2 | 1 ft wing pylon | unchanged (left 0.05 ft) |
| 7 | Wingtip, paired | tip edge + 1.1 ft out, 0.70 up, -4.33 | `R550.JT` x2 | tip rule, centre on the shifted edge | clean, see below |

**Default load, re-measured** (triangle-level check as in MF1-5 against every
pylon, store, the airframe with the gear stowed, the gear plates down and the ground):

| Station | Entry into the airframe | Gear down | Nearest neighbour | Ground |
| --- | ---: | --- | --- | ---: |
| Tank, row 5, right (x 2.30 to 4.18) | none, 0.06 ft off the fuselage flank | 1.25 ft | clear | 3.10 ft |
| Tank, row 5, left | none, 0.06 ft off the flank | 1.25 ft | clear | 3.10 ft |
| R550, row 6, right | none (0.13 ft clear) | 3.0 ft | clear | 4.19 ft |
| R550, row 6, left | 0.05 ft (upper fin, 20 crossings; as before, accepted) | 3.0 ft | clear | 4.19 ft |
| R550, row 7, right tip | none (0.48 ft off the tip) | 3.0 ft | clear | 6.01 ft |
| R550, row 7, left tip | none (0.24 ft off the tip) | 3.0 ft | clear | 6.01 ft |

The belly pair no longer crosses itself (was 32 crossings) or either main-gear
plate (was 24 and 20), and the whole default load shows nothing beyond the 0.05 ft
wing Magic that MF1-5 already accepted. The tank hangs higher than the old outer
tank: the standard pylon rule reads the wing root over x 3.24 ft and the tank's
top sits 1 ft under it, so its ground clearance is 3.10 ft (was 1.23 ft) and its
nearest approach to the stowed airframe is the fuselage flank, 0.06 ft, no crossing.
The audit's own `clip` lines read 1.48 ft (hardpoint 7) and 2.43 ft (hardpoint 5)
in MF1-5. Now hardpoint 7 reads 0.00 ft and hardpoint 6 0.00 ft; **hardpoint 5 reads
4.29 ft at [2.37, -1.58, 0.00], which is not an entry into the airframe**: the audit
looks straight down from each store vertex for the lowest face of the neutral
(gear down) shape, and under the tank's inner edge it finds the main-gear plate's
outer low corner (the plate runs to x 2.41 ft at y -5.98 ft; the face found is at
y -5.87 ft). The triangle-level check, which tests tank triangles against plate and
airframe triangles, finds 0 crossings, no entry and 1.25 ft between tank and plate,
and the frames show the wheel standing beside the tank. The line is therefore a
limit of the audit's straight-down metric, the same one that made the four-tank
load read 2.43 ft; it is not a clip, but it stays visible in the audit output.

**A centreline `F250.GAS` with the new pair.** The centreline tank is x -0.94 to
0.94 ft, the pair's inner edges 2.30 ft, so 1.36 ft clear (was 0.21 ft with the pair
at 2.07 ft, and 148 and 156 crossings with the old four). The tank is still 0.14 ft
off the belly and 0.69 ft off the gear alone; with the pair the three tanks are
clean (no entry, no gear crossing, no store pair under 0.1 ft or crossing). The real
three-tank ferry load now reads as one picture.

**Row 5 alternatives, re-measured at the capped count (65 records, one run each with
all four rows loaded, only the same row's metrics read).** The cap leaves the list as
it was (181 catalog choices, no record lost): every record that took four now takes
two, and the 19 records that already took two keep two (`MK84`, `FAB1000`, `MK82P3`,
`MK82P`, `MK82AIR`, `GBU10`, `GBU10A`, `PAVEWAY`, `PAVEWA3`, `GBU28`, `GBU29`,
`GBU29P`, `AS14`, `AGM84A`, `AGM84E`, `AM39`, `AS16`, `F350.GAS`, `F500.GAS`). The racks `MK82P3` and `MK82P` are two-store records
as before and are clean (the old log's odd `MK82P3` 3 to 2 reduction and the four
other odd-weight 3 to 2 reductions on row 5 are gone because the row's capacity is
now two; the `GBU12`, `GBU16` and `GBU38` row 5 capacity reads 2 where it read 4).
Results at 3.24 ft: **19 of 65 clean** (no airframe entry, no gear crossing: `AA8`,
`B13`, `B8`, `MK82`, `LAU61`, `MK84`, `FAB250`, `FAB500`, `MK82P3`, `MK82HD`,
`MK82P`, `MK82AIR`, `CBU87`, `MK20`, `RBK250`, `RBK500`, `LAU10`, `F150.GAS` and
`F250.GAS`). 46 enter the airframe: 13 by 0.3 ft or less, 33 by 0.37 to 3.39 ft (the
long missiles and the laser bombs, whose noses and fins lie against the fuselage
flank and the wing root beside the pylon). **None crosses the gear** (58 of 65 did
at 2.07 ft) and none touches the ground (lowest nose 1.68 ft up). Only in Load
Ordnance; the default does not use them. Rows 4, 6 and 7 are unchanged by this
slice except row 7's tip offset: row 4 still 22 of 58 clean (15 cross the gear),
row 6 39 of 47 clean.

**Row 7 (the ten tip records) with the 1.1 ft offset.** Before: all ten entered the
tip, 0.50 ft (`AIM9B`, `AIM9M`, `AA2`) to 1.48 ft (`R550`). After, with 0 crossings
and the clearance to the tip skin in feet (right / left wing):

| Record | Entry | Tip clearance |
| --- | ---: | --- |
| `R550.JT` (default) | none | 0.48 / 0.24 |
| `AA11.JT` | none | 0.78 / 0.58 |
| `AA11B.JT` | none | 0.39 / 0.27 |
| `AA2.JT` | none | 0.78 / 0.56 |
| `AA8.JT` | none | 0.79 / 0.66 |
| `AIM9X.JT` | none | 0.78 / 0.53 |
| `AIM9M.JT` | none | 0.78 / 0.53 |
| `AIM9B.JT` | none | 0.78 / 0.54 |
| `AT12.JT` | none | 0.75 / 0.58 |
| `PL7.JT` | 1.24 / 1.35 ft by the audit's straight-down metric, 0 crossings | 0.57 / 0.35 |

Nine of ten clear completely. `PL7` has fins wider than the Magic's: nothing
touches (0.35 ft off the skin) but its fin tops sit over the tip's edge, which the
vertical metric reads as entry; 1.4 ft outboard clears it too (0.75 / 0.55 ft), a
larger offset than the default needs. The left wing reads less clearance than the
right because the shape's own tip is 0.2 ft asymmetric.

**How the offset was chosen.** Sweeps with the temporary measurement build, `R550`
on row 7: outboard 0.4 ft still 1.45 ft entry and 46 crossings; 0.8 ft 0 crossings
but 1.38 ft by the vertical metric; 1.0 ft 1.17 / 1.44 ft; **1.1 ft none**; 1.2 ft
none (0.55 / 0.31 ft). Down instead of out: 0.8 ft 0.68 ft entry, 1.5 ft none but
0.055 ft from the tip (touching), 1.6 ft none (0.14 / 0.16 ft), 2.0 ft none. So the
smallest clean offset is 1.1 ft outboard (the Magic's fin half span is about 1.09
ft), against 1.6 ft down. The 3x crops show the missile standing off the tip with
the tip's thin rail shape beside it and no fin inside the wing.

**Audit pins did not move.** `--validate-ordnance`: `ordnance MF1.PT: 5 source rows,
2 optional weapon rows, 175 compatible weapon placements, 6 tank placements;
defaults and restart passed` and `45 aircraft, 18 recovered weapon stations, 4652
weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed`, 80
pods and 220 Cheat-only passed: placements count record and row pairs, not
quantities.

**The default load is lighter.** Two tanks carry 3,300 lb of fuel (`(+ EXT 3300
LBS)`), not 6,600 lb; with the gun, the two tanks' shells and the four Magics the
carried weight is 4,481 lb, a gross weight of 26,143 lb (87 percent of the 30,203 lb
maximum, was 29,839 lb and 99 percent). The scripted liftoff now reads 121.0 kt after
1,144 ft (349 m; was 135.4 kt after 504 m) and the scripted approach 127.6 kt (was
145.6 kt; published about 125 kt), landing at 118.7 kt and stopping in about 447 m.
The battery windows were refitted (see [variety flight](variety-flight.md)); the
other default-load numbers of the flight review (pull, sustained turn, top speed)
were not re-run and the lighter load only improves them.

**Not decided here.** The real Mirage F1's lateral inner pylon offset (no source),
whether 46 row 5 alternatives that enter the airframe at 3.24 ft should be refused
by a mask (they are only reachable through Load Ordnance; John's call), and a larger
tip offset for `PL7` (1.4 ft, not applied).

Update (MF1-6, 2026-11-11): a mask was applied on the coordinator's instruction,
following John's F-15E A2 0.3 ft line; see [the Mirage F1 refusals](#the-mirage-f1-refusals-mf1-6-2026-11-11).
`PL7` stays on the tip at 1.1 ft (no crossing).

Provenance: capacity and the move opinionated (John, 2026-11-11, round 3, M1 "follow
recs"); the 3.24 ft, 1.1 ft and the mechanisms fitted agent decisions; measurements
by the MF1-5 tools (`.local/m3-aircraft/checks/MF1-5b/`); the retail rows, masks and
defaults spec-derived from `MF1.PT` with B5-2's empty centreline (John, 2026-11-10);
the real inner pylon tanks and the wingtip rails from Jane's text in `MF1.INF` and
Aerospaceweb (plausibility only, no lateral offsets).

## The Mirage F1 refusals (MF1-6, 2026-11-11)

John answered the F-15E (A2, 2026-11-10) by refusing the large stores that enter
the belly by more than about 0.3 ft. For the F-5E he answered C2 "for the f-5 keep",
so the F-5E refused nothing and kept its wing entries. The coordinator asked for a
mask on the Mirage F1 once MF1-5b had put the belly pair at 3.24 ft and the tip
Magic 1.1 ft off the tip. The rows chosen, the 0.3 ft line and the lists below are
agent decisions that follow A2's 0.3 ft line on both rows (a first version of this
slice refused all seven row 6 entries on a reading of C2 that John's answer does not
support; corrected in place the same day, nothing was committed). They apply to normal loading only (Cheat loading skips the check,
as it skips every other refusal) and are in `aircraft_stations::weapon_allowed`
(read by `Loadout::capacity`, the audit and the empty-row selection).

**Row 5, the belly pair (33 refused).** Measured by MF1-5b at the 3.24 ft slot, the
alternatives that enter the airframe by more than 0.3 ft (ft entered, triangle
level): `AA11B` 0.43, `AA2` 0.37, `AA6` 2.80, `AA9` 1.39, `AIM9M` 0.73, `MICA` 0.65,
`R530` 1.44, `R550` 2.02, `PL7` 2.32, `PL10` 2.34, `AGM88` 2.35, `AIM7` 1.94,
`AIM7E` 1.94, `AIM9B` 0.48, `GBU12` 2.30, `GBU16` 2.37, `GBU10` 2.32, `GBU10A`
2.32, `PAVEWAY` 2.32, `PAVEWA3` 3.39, `GBU28` 2.61, `GBU29` 3.39, `GBU29P` 3.39,
`GBU30` 3.39, `AS14` 1.56, `AS30` 2.52, `AT12` 0.41, `AGM84E` 1.42, `AGM45` 2.35,
`AGM84A` 1.36, `AM39` 2.65, `AS16` 0.49, `AS15` 0.99. Kept: the 19 clean records
and the 13 that enter by 0.3 ft or less (`AA11` 0.23, `AA12` 0.21, `AS7` 0.29,
`AIM9X` 0.19, `AIM120` 0.21, `AGM65G` 0.20, `AGM65A` 0.20, `AT2` 0.26, `FAB1000`
0.08, `CBU89` 0.18, `GBU38` 0.13, and the `F350.GAS` 0.03 and `F500.GAS` 0.23
tanks, which the weapon check does not touch). No row 5 record crosses the main
gear at this slot, so no gear refusal arose.

**Row 6, the paired wing row (1 refused).** MF1-5 lists eight records that enter the
wing (0.05 to 0.33 ft). Only `AS30` (0.33 ft) is over the 0.3 ft line and is
refused. Kept: `PL7` 0.07, `PL10` 0.15, `AGM88` 0.15, `AGM45` 0.15, `AIM7` 0.06,
`AIM7E` 0.06, and `R550.JT`, 0.05 ft with 20 crossings on the left side only (the
right side is clean), which is the default of this row (and of the tip row): a
default store is never refused.

**Wingtip row.** Unchanged: nine of ten records clean at 1.1 ft; `PL7` has no
crossing (0.35 ft clearance, 1.24 and 1.35 ft by the vertical metric) and stays on
the tip.

**Centreline.** Unchanged and not asked for: 36 of its 58 records enter the belly
by 0.05 to 0.71 ft and 15 cross the gear (MF1-5, row 4).

**Effects.** `--validate-ordnance`: `ordnance MF1.PT: ... 141 compatible weapon
placements` (was 175), `catalog 60 normal` (was 62: `AS16.JT` and `GBU28.JT` were
only on the belly row), weapon placements 4,652 to 4,618. The Load Ordnance catalog
dropped from 181 to 147 choices (exactly these 34). An empty belly row remembers
`MK82.JT` now (it remembered `R550.JT`, which it refuses). Nothing else of the
default load, the stations or the clearances moved.

Provenance: the rule is an agent decision (2026-11-11) following John's F-15E A2
answer (2026-11-10), applied on both rows; the F-5E kept its wing entries (C2); the entries are MF1-5b's measurements
(`.local/m3-aircraft/checks/MF1-5b/`); the lists are `MF1_BELLY_REFUSED` and
`MF1_WING_REFUSED`.

## The Tornado IDS: swivelling wing pylons and a stop on clipping (TOR-5, 2026-11-10)

**Status: stopped on clipping, as John's standing rule requires.** The swivel is
built and tested; no station position, `station_move` or default was changed.
Every default store of the Tornado clips something, so John chooses the fix.
Numbers below are measured with `tore-app --stores-preview` (CPU, audit, with
`TORE_STORES_PREVIEW_SWEEP`, `TORE_STORES_PREVIEW_CLEARANCE`; see
[development](../DEVELOPMENT.md#headless-development)) and the frames in the
slice's local frame folder.

**The swivel rule (opinionated, John 2026-11-10, decision 19).** The real
Tornado's wing pylons swivel so the stores stay aligned with the airflow as the
wing sweeps; stores on them follow. Here:

- **Which rows swivel.** Only the inner wing pair (`TOR.PT` row 7, `Location::Wing`).
  The centreline tank pair (row 5) and the shoulder pylons (row 6) hang from the
  fuselage and stay fixed. Row 8 is retired. The row is chosen by
  `combat::swing::swivels(id, row)`.
- **Where the store goes.** The sweep is a pure function of the aircraft's speed and
  flaps (`combat::swing::wing_turn`: spread to 250 kt, linear to fully swept at
  600 kt, 25 to 67 degrees, flaps hold the wing out; the numbers are TOR-3's,
  moved from the drawing rig into the simulation unchanged. Update (TOR-11,
  2026-11-12): spread to 304 kt, John's 350 mph, the rest unchanged). A swivelling
  station's pylon is carried to where the wing's turn about its pivot puts it
  (`swing::swept`); the pylon swivels back by the same turn, so the store's heading
  stays along the fuselage at every sweep and only its position moves. The height
  does not change, because the wing panel is rigid.
- **One value, three consumers.** `Rack::turn` (a new `f32`, zero for every other
  station) holds the turn. `Rack::side_pylons` applies it, so the release origin
  (`Rack::store_point`, `Station::release_point`), the drawn store (`pylons::hung`)
  and the jettison points all agree. `Configuration::set_wing_turn` sets it on
  swivelling stations every tick from `tore-world` combat (`flight::State::speed` and
  `flaps`), so every peer works out the same value; it is derived, never carried by
  a checkpoint or a recording (`turn` is a `skip` field of the `Rack` checkpoint). The
  drawing sets the same value from the aircraft's own state (`pylons::swung`), so
  replays and remote aircraft draw it too. The pylon's underside is read from the
  model with the wing turned (`Airframe::underside_at`, a cached copy of the
  Tornado's shape with the two wing panels turned by `tornado_animation::swept_shape`).
- **Tests.** `combat::swing` (schedule, inverse, round trip, which rows),
  `live::tests::a_swivelling_wing_station_rides_the_swept_wing_and_a_fixed_row_stays`
  (a swivelling row lands on the swept wing at 25 and 67 degrees, the fixed row does
  not move), `pylons::tests::a_swivelling_station_is_drawn_where_the_swept_wing_carries_it`.

**Station table (default load: two `F250.GAS`, four `PAVEWAY.JT`, two `MK82P3.JT`;
feet right / up / forward of the reference point, right side; header scale 2/3).**

| Row | Location | Default | At 25 degrees (spread) | At 67 degrees | Swivels |
| ---: | --- | --- | --- | --- | --- |
| 5 | Wing, pair at x 0 | 250 gal x2 | tanks 1.18 either side of 0, up -5.94, forward 5.67, underside -4.0 | same | no |
| 6 | Fuselage, 4 | Paveway x4 | x 2.00, up -4.55 / -4.92, forward -2.93 and 12.27, underside -4.0 / -2.6 | same | no |
| 7 | Wing, 2 | Mk 82 x2 | x 3.67, up -4.89, forward 3.00, lugs 1.00 under the belly (-3.44) | right (7.03, -4.89, 3.76), left (-7.20, -4.89, 4.20), nothing above them | yes |

Gear down (contact -7.33 ft): lowest store points 0.46 ft (tank), 0.50 ft (rear
Paveway), 0.86 ft (front Paveway) and 1.97 ft (Mk 82) above the ground; the
Mk 82 store box contains gear-leg points at 25 degrees (the main legs stand at
about 3.3 ft out, row 7 is 3.67 ft out) and clears the legs by 1.9 ft at 67.
The all-heavy load (500 gal tank, four Mk 84 on row 6, two Mk 84 on row 7) comes
within 0.03 ft of the ground at the tank.

**Clipping, for John (nothing fixed).**

1. **Row 7 hangs from the fuselage side, not from the wing.** It is 3.67 ft out and
   3 ft forward; the movable panel's pivot is 6.3 ft out and 1 ft aft, and the model
   has no wing or glove ahead of y +1 unit (0.7 ft). At 25 degrees its Mk 82 enters
   the airframe by 2.61 ft (the `clip` report; larger alternatives: 2.18 to 2.83 ft,
   the GBU-30 1.27 ft; a Mk 84 reads 0.00). Carried by the swinging wing, the same
   station ends 7 ft out and 3.8 to 4.2 ft forward with **nothing above it** (the
   model's wing is swept away behind it), so the swivel draws a pylon standing in
   air and a bomb floating ahead of the swept wing (front frames). The swivel is
   correct for a pylon that is on the panel; row 7 as decoded is not.
2. **Row 6 Paveways enter the fuselage 0.96 ft** (hardpoint 6 `clip`), at x 2.00
   with their fins in the belly. Load Ordnance alternatives: GBU-12 0.90, AGM-88
   0.93, AS-30 2.12, GBU-28 0.33, AIM-9M 0.60; Mk 82 and Mk 84 read 0.00.
3. **Default load store against store.** The tank pair (centres 1.18 either side of
   x 0, radius about 0.9 ft) overlaps the Paveways beside them (x 2.0, radius about
   0.55 ft) along their whole length: bounding boxes overlap by 1.5 to 1.8 ft
   (Paveway and tank), 0.7 ft (Paveway and Mk 82). A single 350 or 500 gal tank at
   x 0 overlaps the Paveways by 0.9 to 1.1 ft; two 350 or 500 gal tanks on row 6
   overlap the centreline pair by 1.2 to 1.3 ft. A Mk 84 four on row 6 with the
   single 500 gal tank on row 5 is clear (the only default-compatible clean combination
   measured). The bounding-box gaps are conservative for thin bodies; the frames show
   the Paveway, tank and Mk 82 bunched and cutting each other under the belly.

**Options (agent analysis, John decides; nothing applied).**

- *Row 7 onto the panel.* A fitted station move (`station_move`) to a point
  on the movable panel outboard of the pivot, about 7 to 8 ft out (the real inner
  wing pylon is on the swinging panel, not the fuselage; the real position is not
  cited here: no dimensioned drawing is available), then the swivel is as built and
  its pylon meets the swept wing. Also needs its default (Mk 82 triple) re-fitted.
- *Row 7 as a fixed fuselage station.* Keep 3.67 ft and drop the swivel (the stores
  keep their station but contradict decision 19).
- *Rows 5 and 6.* Spread the Paveway pairs outboard (about x 2.9 ft with the pair of
  tanks at x 0), or take the MiG-31's 0.1 ft rail drop (0.96 ft remains), or hang the
  default tank pair as one 500 gal tank at x 0 (the real centreline store).
- *Accept* the measured entries with a note (the Su-27 precedent accepted 0.72 ft).

**Provenance.** Which rows swivel, the pylon rule and the swept-surface lookup are
opinionated (John 2026-11-10, decision 19, for the rotating pylons; the row choice
is an agent decision); the schedule, pivot and sweep range are TOR-3's fitted
values and the cited 25 to 67 degrees; the measurements are agent findings. The
real station numbers are not claimed. No audit pin moves.

## The Tornado IDS: inner wing pair on the wing, one centreline tank, a glove plate (TOR-5b, 2026-11-11)

Update (TOR-5b, 2026-11-11) to the [TOR-5 section](#the-tornado-ids-swivelling-wing-pylons-and-a-stop-on-clipping-tor-5-2026-11-10)
above (its text stands as the measurement it was): John answered the clipping stop on
2026-11-11 ("For the Tornado follow recs", answers T1 to T5, opinionated). Three of the
answers are applied and measured below; **the Paveway move (T2) could not be done as
asked** and is held for John with the measurements and frames. Numbers come from
`tore-app --stores-preview` (CPU, audit, clearance, `TORE_STORES_PREVIEW_SWEEP` 25, 35,
46, 55, 60 and 67 degrees, default load and 22 alternatives) and free-flight frames; both
are local only.

**T1, row 7 on the swinging panel (opinionated, John 2026-11-11; the position is fitted,
agent decision). Update (TOR-12, 2026-11-12): the numbers in this section are for the wing as
it was; after John's wing review the station is 7.96 ft out and 1.7 ft forward
([TOR-12](#the-tornados-wing-placed-pylons-moved-with-it-tor-12-2026-11-12)).** `station_move(Tornado, 7)` puts the inner wing pair at **9.5 ft out,
1.5 ft forward** (`TORNADO_WING_INNER`), a `Location::Wing` station on a standard 1 ft
wing pylon. The sweep pivot is 6.33 ft out, so the station is 3.2 ft outboard of it on the
movable panel, and the existing swivel (`combat::swing::swivels`, `Rack::turn`,
`side_pylons`) carries it: the pylon rides the wing and turns back by the wing's own turn,
so the store stays streamwise. Where the real inboard swivelling pylons are is not in any
source found. What is cited (read 2026-11-11): the Tornado IDS has seven store stations,
one centreline, two fuselage shoulders and **two swivelling pylons under each wing**
([Sir Viper](https://sirviper.com/index.php?page=fighters%2Ftornado%2Ftornado_ids),
[Dinger](https://dingeraviation.net/tornado/tornado.html)); the wing pivots sit in the
fixed inboard "nibs" ([Bomber Aircraft wiki](https://bomberaircraft.fandom.com/wiki/Panavia_Tornado_IDS));
the span is 45 ft 7.5 in spread and 28 ft 2.5 in swept
([Migflug](https://migflug.com/aircraft/panavia-tornado/)). No dimensioned drawing was
found, so the 9.5 / 1.5 ft pair is **fitted**: chosen from a grid of 8.5, 9.5 and
10.5 ft out by -1, 1.5 and 3 ft forward as the place where the default Mk 82 and the Mk 84
clear the airframe at every sweep and the largest alternative (the GBU-30) enters it
least. At 9.5 ft the panel's leading edge is 3.2 ft forward and its trailing edge 3.4 ft
aft, so a 7 ft store hangs with its nose about 1.8 ft ahead of the leading edge. Retail
records the row at 3.67 ft out and 3 ft forward, inside the fuselage side, with the
default Mk 82 entering the airframe 2.61 ft; that is gone.

**T3, one centreline tank (opinionated, John 2026-11-11).** The default load holds one
`F250.GAS` at x 0 instead of the crossing pair. It is a default rule, not a capacity
change: `aircraft_stations::default_counts` (guarded on the Tornado and the whole retail
row 5: Wing, flags 0x205, position 0 / -25 / 17, `F250.GAS`, two, weight class 40) lowers
the quantity that starts loaded to one, and `hardpoints::default_records` hands it to the
default loading as the A-4E's three tanks are. Row 5 keeps its capacity (two), place,
mask and weight class, so Load Ordnance still admits what it did and no audit pin moves.
External fuel is 1,650 lb (3,300 lb before).

**T4, the glove plate (opinionated, John 2026-11-11; the geometry is fitted).** See
[the Tornado control rig](../formats/aircraft-control-surfaces.md#tornado-glove-plate-tor-5b-2026-11-11).
It is stores-neutral: it lies above the wing, so no pylon reads it.

**T5, tailerons:** unchanged, 0.25 rad.

**Station table (default load: one `F250.GAS`, four `PAVEWAY.JT`, two `MK82P3.JT`; feet
right / up / forward, right side; header scale 2/3).**

| Row | Location | Default | At 25 degrees (spread) | At 67 degrees | Swivels |
| ---: | --- | --- | --- | --- | --- |
| 5 | Wing, x 0 | 250 gal x1 | tank at x 0, up -5.94, forward 5.67, underside -4.0 | same | no |
| 6 | Fuselage, 4 | Paveway x4 | x 2.00, up -4.55 / -4.92, forward -2.93 and 12.27 | same | no |
| 7 | Wing, 2 | Mk 82 x2 | x 9.50, up -1.45, forward 1.50, lugs 1.00 under the wing | right (10.36, -1.45, -1.26), left (-10.53, -1.45, -0.81) | yes |

At 67 degrees the left and right Mk 82 sit 0.45 ft apart fore and aft: the source's left
glove is one unit (0.67 ft) further out, so the left pivot is too (TOR-3), and the pair is
not an exact mirror. Row 7's clip is 0.00 at 25, 35, 46, 55, 60 and 67 degrees.

**Clearances, default load** (clip: deepest point of a store's shape above the airframe's
lowest face straight under it, in feet; the airframe is the clean shape with no gear, so
gear up and gear down read the same; "gear" lines are against the gear-down legs).

| Measure | Before TOR-5b (25 / 67) | After (25 and 67, every sweep) |
| --- | --- | --- |
| Row 7 Mk 82 against the airframe | 2.61 / 0.00 (and floating over nothing at 67) | **0.00** |
| Row 5 tank against the airframe | 0.00 | **0.00** |
| Row 6 Paveways against the airframe | 0.96 | **0.96** front pair, 0.55 rear pair (unchanged, see T2) |
| Paveway against tank (bounding boxes) | -1.84 (pair) | **-0.85** (single tank) |
| Paveway against Mk 82 | -0.70 | **none within 1 ft** |
| Gear down, Mk 82 against the legs | boxes held gear points | **4.4 ft (25), 5.3 and 5.5 ft (67)** |
| Gear down, lowest store above the ground | tank 0.46, Paveways 0.50 / 0.86, Mk 82 1.97 | tank **0.46**, Paveways **0.50 / 0.86**, Mk 82 **5.42** |

Load Ordnance alternatives (row by row, as in TOR-5): every Mk 82, Mk 84, GBU-38, AGM-45,
AGM-88, CBU-87, FAB-500, LAU-10 and B-8 on row 7 reads **0.00** at both sweeps; the
GBU-30 reads 0.00 at 25 and **1.10** at 67 degrees (its fins reach the airframe's lower
flank at the swept position; was 1.27 at the old position, 2.43 at 8.5 ft out and the
middle of the panel). The 500 gal tank with four Mk 84 on row 6 and two Mk 84 on row 7
(all-heavy) reads 0.00 everywhere and stands 0.03 ft into the ground with the gear down
(the tank, as before). On row 6: Mk 82, Mk 84 and two 350 or 500 gal tanks read 0.00; the
AIM-9M 0.60, GBU-28 0.33, AGM-88 0.93, GBU-12 0.90, Paveway 0.96 and AS-30 2.12.

**T2, row 6 Paveways, stopped (clearing is not possible by moving them outboard).** The
0.96 ft the TOR-5 report called "the Paveways entering the fuselage" is the **fin tips**,
not the bodies: the Paveway's wings span about 3.8 ft, and with the fins in an X the
upper inboard tip stands 1.9 ft inboard of and 1.9 ft above the store's centre line, which
is inside the belly (flat at -4.0 ft out to about 3 ft, then a flank to -2 ft). The
deepest tips are the front pair's at 12 ft forward (0.95 ft) and the rear pair's tails
at -2.9 ft forward (0.55 ft). Measured moves of the whole row (right side shown, both
mirror; forward kept at the record's 4.67 ft):

| Row 6 out (ft) | Worst store clip | Paveway against the tank | What changes |
| ---: | ---: | ---: | --- |
| 2.00 (record) | 0.96 (front pair 0.95, rear pair 0.55) | -0.85 | the default |
| 2.25 to 2.60 | 0.96 (same two) | -0.60 to -0.25 | tips still in the belly |
| 2.70 to 2.75 | **3.41** | -0.15 to 0.0 | the front pair's pylon reads the intake side wall ahead of it (-1.7 ft) and the pair hangs 2.1 ft higher, into the flank |
| 3.00 to 4.50 | 3.3 to 3.6 (rear pair 1.8 to 2.7 at 3.0, 0.78 at 3.5) | clear | both pairs in the flank |
| 4.75 to 5.00 | 4.9, 2.1 | clear | under the edge of the glove |
| 5.50 to 7.00 | 3.8 falling to 2.3 (rear pair); front pair 1.07 falling to 0.00 | clear | the stores hang from the wing glove, fins in the wing |

So the smallest outboard move that clears the tank (about 2.85 ft) is past the 2.70 ft
where the front pair climbs into the flank, and no outboard move gets the fin tips out of
the fuselage under 0.3 ft. Nothing was moved; row 6 is as TOR-5 left it. The fix does not
come from the row's lateral place. **Options for John** (agent analysis, frames
`opt-row6-*` in the slice's frame folder):

- *Accept* the fin-tip entries (0.55 and 0.96 ft; the bodies are clear; the Su-27's accepted
  0.72 ft is the precedent). The Paveway against the tank overlap (-0.85 ft, a fin tip
  into the tank's shoulder) stays too, or moves out to about 2.6 ft (-0.25 ft).
- *A rail drop* like the Mirage 2000's and MiG-31's 0.1 ft (each foot of drop takes a foot
  off every clip). It cannot clear: the front pair stands 0.50 ft above the ground gear
  down, so at most 0.5 ft is available (0.45 ft would remain at the front pair, the rear
  pair would clear), and the front pair would touch the ground.
- *A default of Mk 82 or Mk 84 on row 6* instead of the Paveway (both read 0.00 and clear
  the tank), through a default rule like T3; it takes the laser bombs off the default
  load (the Quick Mission and the laser-bomb scenarios use them).
- *A smaller Paveway shape*: the fins are the retail shape (`PAVE.SH`); a fitted smaller
  copy would be new art, which no slice has proposed.

**Provenance.** T1 to T5 are John's round-3 answers (opinionated, 2026-11-11); the
9.5 / 1.5 ft position, the choice of a default rule over a capacity change, the glove
plate's outline and the measurements are agent decisions or findings; the station counts
are from the cited public pages and no station offsets are claimed. The audit pins do not
move (45 aircraft, 18 stations, 4,652 weapon placements, 233 tank placements, 220
Cheat-only placements, 80 pod placements).

## The Tornado ordnance masks and the measured catalog (TOR-6, 2026-11-11)

Agent decision applying John's F-15E A2 precedent (2026-11-10: refuse the stores
that enter the airframe by more than about 0.3 ft), as the coordinator restated
it on 2026-11-11: refuse alternatives over 0.3 ft, keep anything at 0.3 ft or less
and anything that only crosses a neighbour store, never refuse the default
`PAVEWAY.JT`. The stations are the final ones from [TOR-5b](#the-tornado-ids-inner-wing-pair-on-the-wing-one-centreline-tank-a-glove-plate-tor-5b-2026-11-11)
(row 5 one centreline tank, row 6 shoulder Paveways unchanged with John's accepted
0.96 ft front and 0.55 ft rear fin-tip entry, row 7 on the swinging panel at 9.5 ft
out, row 8 retired).

**Measurement.** Every store the Load Ordnance catalog offered on rows 5, 6 and 7
(4, 65 and 23 stores) was hung alone on its row, at its largest count, with the
rest of the default load, in the CPU stores preview
(`.local/m3-aircraft/checks/TOR-6/matrix.sh`, 117 runs; row 7 at 25 and 67
degrees, the fixed rows 5 and 6 at 25) and clipped against the airframe without
gear. Entry is the deepest point of any triangle inside the skin. Summary:
`checks/TOR-6/summary.txt`.

**Row 6, the shoulder pair (fixed pylons, 2.00 ft out): 33 stores refused.**

| Store | Shape | Entry (ft) |
| --- | --- | ---: |
| `AT12.JT` | `SWNGR.SH` | 0.33 |
| `GBU28.JT` | `GB28.SH` | 0.33 |
| `AA12.JT` | `AMSKI.SH` | 0.35 |
| `AIM9X.JT` | `AM9X.SH` | 0.36 |
| `AIM120.JT` | `AMRAM.SH` | 0.37 |
| `AA11.JT` | `ARCHR.SH` | 0.39 |
| `AT2.JT` | `SWATR.SH` | 0.44 |
| `AGM84E.JT` | `SLAM.SH` | 0.46 |
| `AGM84A.JT` | `HARPN.SH` | 0.47 |
| `AA2.JT` | `ATOLL.SH` | 0.48 |
| `AS14.JT` | `KEDGE.SH` | 0.48 |
| `MICA.JT` | `MICA.SH` | 0.48 |
| `AIM9B.JT` | `SIDEB.SH` | 0.52 |
| `AA9.JT` | `AMOS.SH` | 0.58 |
| `AIM9M.JT` | `SIDEM.SH` | 0.60 |
| `AS15.JT` | `AS12.SH` | 0.62 |
| `R530.JT` | `M53.SH` | 0.62 |
| `AIM7.JT` | `SPAR.SH` | 0.75 |
| `AIM7E.JT` | `SPAR.SH` | 0.75 |
| `GBU12.JT` | `GB12.SH` | 0.90 |
| `R550.JT` | `M55.SH` | 0.90 |
| `AGM45.JT` | `HARM.SH` | 0.93 |
| `AGM88.JT` | `HARM.SH` | 0.93 |
| `PL10.JT` | `PL10.SH` | 0.93 |
| `PL7.JT` | `PL7.SH` | 0.93 |
| `GBU16.JT` | `GB16.SH` | 0.98 |
| `AA6.JT` | `ACRID.SH` | 1.01 |
| `AM39.JT` | `EXO.SH` | 1.17 |
| `GBU29.JT` | `GB27.SH` | 1.93 |
| `GBU29P.JT` | `GB27.SH` | 1.93 |
| `GBU30.JT` | `GB27.SH` | 1.93 |
| `PAVEWA3.JT` | `GB27.SH` | 1.93 |
| `AS30.JT` | `AS30.SH` | 2.12 |

Kept (entry in ft, at or under 0.3 ft or the default's own shape): `GBU10.JT` 0.96, `GBU10A.JT` 0.96, `PAVEWAY.JT` 0.96, `AS7.JT` 0.30, `GBU38.JT` 0.22, `AGM65A.JT` 0.17, `AGM65G.JT` 0.17, `AS16.JT` 0.15, `F250.GAS` 0.15, `B13.JT` 0.14, `CBU89.JT` 0.13, `AA8.JT` 0.12, `F150.GAS` 0.10, `AA11B.JT` 0.08, `B8.JT` 0.07, `MK82HD.JT` 0.06, `FAB500.JT` 0.05, `LAU10.JT` 0.05, `RBK500.JT` 0.05, `CBU87.JT` 0.04, `MK20.JT` 0.04, `MK82P.JT` 0.02, `RBK250.JT` 0.01, `F350.GAS` 0.00, `F500.GAS` 0.00, `FAB1000.JT` 0.00, `FAB250.JT` 0.00, `LAU61.JT` 0.00, `MK82.JT` 0.00, `MK82AIR.JT` 0.00, `MK82P3.JT` 0.00, `MK84.JT` 0.00.
`GBU10.JT` and `GBU10A.JT` hang the same `PAVE.SH` as the default `PAVEWAY.JT`
and read exactly its 0.96 ft; they stay with it (agent decision: John accepted
that shape's fin-tip entry on 2026-11-11, so refusing its siblings would refuse
the same geometry twice). `AS7.JT` reads 0.30 and stays by the "0.3 ft or less"
rule. Crossings of the neighbouring centreline tank or Paveways (down to
-1.64 ft for the 2,000 lb `GB27.SH` bombs) are not refusals.

**Row 7, the inner wing pair (swinging panel, 9.5 ft out): 3 stores refused.**
`GBU29.JT`, `GBU29P.JT` and `GBU30.JT` (`GB27.SH`) enter the airframe 1.10 ft at 67
degrees (0.00 at 25). The other 20 stores read 0.00 at both sweeps; none comes
within 2.1 ft of the gear legs.

**Row 5, the centreline: nothing refused.** `F150.GAS`, `F250.GAS`, `F350.GAS`,
`F500.GAS` read 0.00; the 500 gallon tank sits 0.03 ft into the ground with the
gear down (the all-heavy case TOR-5 found; under the 0.3 ft line).

**Gear.** The preview's gear gap is a bounding-box test; on row 6 it reads 0.00
for the default and nearly every store alike (a gear sample point lies inside the
store's box), so it cannot separate a refusal and none was made from it.
Row 7 never comes within 2 ft.

**Effect.** `weapon_allowed(Tornado, 6 | 7, ...)` in
`aircraft_stations::{TORNADO_SHOULDER_REFUSED, TORNADO_INNER_WING_REFUSED}`
returns false; `Loadout::capacity_before_store_pairs` gives those stores capacity
0 in normal loading (Cheat loading skips the check), so the Load Ordnance page,
saved loads and the lobby never offer them. The ordnance audit asserts that no
refused store has capacity, and its derived-bomb check (a laser-bomb variant such as `GBU12.JT` must keep
its donor's capacity) skips a row that refuses the variant by name.
Weapon placements 4,652 to 4,616 (36 fewer; `TOR.PT` 85 to 49), the Tornado's
Load Ordnance air-to-air page holds only `AA-11B` and `R-60M`. The refusal list
is a plain constant; John can narrow it.

The release points at both sweeps (`a2g-release-origin-tornado`): row 6 from
2.00 ft out, 12.27 then -2.93 ft forward, never moving; row 7 from 9.50 ft out
and 1.50 ft forward at 25 degrees, and swivelled with the panel at 67 degrees:
left -10.53 ft out and 0.81 ft aft, right 10.36 ft out and 1.26 ft aft.

## The Tornado AIM-9 side rails on the inboard swivelling pylons (TOR-6b, 2026-11-11)

**Opinionated (John, 2026-11-11, after TOR-6): "Lets do a custom side rail kind of
like the f-14 for this station and put it on the inner [wing] rail like the RL
Tornado."** The 0.3 ft refusal line ([TOR-6](#the-tornado-ordnance-masks-and-the-measured-catalog-tor-6-2026-11-11))
refused every AIM-9 on the shoulder row (fin tips 0.36 to 0.60 ft) and row 7's mask
takes no air-to-air missile, so the Tornado had no Sidewinder. It now carries one
AIM-9 on a side rail of each inboard swivelling wing pylon, at the same time as row
7's main store, as the real Tornado carries its AIM-9L on a rail beside the inboard
wing pylon (public pages: the GR1 and GR4 carry no more than one Sidewinder or
ASRAAM a wing, only on the inboard stub pylons: the War Thunder forum's "Panavia
Tornado (UK versions) - Technical data and discussion" thread, posts quoting the
manual, and the CombatAce "alternative Tornado Sidewinder stations" thread, both accessed
2026-11-11; forum posts, so plausibility only).

**Mechanism: the F-14's shoulder rail, on a new row.** The F-14's glove-pylon
Sidewinder is a source row (`F14.PT` row 7) that `station_move` hangs on its own
pylon's *shoulder rail* (`Location::Shoulder`, `Hang::Shoulder`; [the F-14](#the-f-14);
rules: [Shoulder rail](#rules)): 1.25 ft out from the pylon's centre line, its body 0.05
ft above the lugs of whatever hangs under the pylon, an arm drawn off the pylon's
side. The Tornado takes exactly that treatment. `aircraft_stations::side_rail` appends
one row at the end of the table, **row 8** (the index the retired outer wing pair held,
so no row index moves): Wing, mask 0x85 (infrared missiles), weight class 5, two
`AIM9M.JT` (one a side), the retail outer pair's own content restored; only its
position is the pylon's. `station_move(Tornado, 8)` puts it on row 7's pylon,
`Hang::Shoulder`, and `swing::swivels` lists row 8 with row 7, so the rail is carried
by the swinging wing with the pylon. Rows 0 to 7 are untouched; nothing in a format
(rows are a decoded list, not a fixed array) limits the count, and the
checkpoint's station records, replay conversion and `same_stores` follow the rows.

**One change in shared sim code (`Rack::side_pylons`, guarded to a shoulder rail with a
wing turn, which only the Tornado has).** A swivelling pylon is carried to where the
wing has swept it and swivels back to stay streamwise, so a rail on its side stays
beside it: the pylon's point is swept about the wing pivot and the 1.25 ft sideways
offset is added after, not turned with the wing. (Turning the whole point, as row
7's does, would have slid the rail 0.84 ft aft of its pylon at 67 degrees.) The
drawing's pylon and arm read the same point, so the rail and its pylon stay together.

| Quantity | Value |
| --- | --- |
| Position, right side (left mirrors) | the pylon of row 7 (9.50 ft out, 1.50 ft forward at 25 degrees) plus 1.25 ft outboard: **10.75 ft out, 1.50 ft forward**; AIM-9M origin 0.74 ft under the reference (the Mk 82 pack's 1.45 under) |
| At 67 degrees | right (11.61, -0.74, -1.26), left (-11.78, -0.74, -0.81): always 1.25 ft beside the pack, level with it fore and aft |
| Capacity, default | one a side, `AIM9M.JT` x2 (the retail default) |
| Mask | row 8's 0x85 minus four stores (below) |

**Which flank (fitted).** No source gives it. The public pages say only "inboard
Sidewinder stub pylons"; one modding page's note puts the missile "just to the inside of
the pylon", a second hand statement not taken as evidence. The F-14 rail sits outboard
of its pylon, and outboard keeps the missile away from the fuselage and the glove as the
wing sweeps in (measured: 0.00 ft into the airframe at 25 and 67 degrees; the inboard flank
was not measured), so the rail is on the **outboard** flank (agent decision, fitted).

**Mask and refusals (agent decision, the 0.3 ft line).** The mask admits 11 stores (the
F-14 precedent: its rail takes everything its infrared mask admits). Each was hung alone
on the rail, with the default Mk 82 pack, at 25 and 67 degrees in the CPU stores preview
(`.local/m3-aircraft/checks/TOR-6b/matrix.sh`, `catalog-row8.txt`). Entry into the airframe
(ft, same at both sweeps unless given): `AIM9B`, `AIM9M`, `AIM9X`, `AA2`, `AA8`, `AA11`,
`AA11B` **0.00**; `R550` 0.40; `AT12` 0.44; `PL7` 0.50 (0.83 at 67); `GBU12` 1.02. The
last four are refused in normal loading (`TORNADO_SIDE_RAIL_REFUSED`; a laser bomb and
an anti-tank missile are not rail missiles in any case), the seven others stay: AIM-9B,
AIM-9M, AIM-9X, AA-2, AA-8, AA-11, AA-11B. Cheat loading skips the check.

**Against the pylon's own store.** Bounding boxes (the preview's `pair` gap, which counts
a crossing of the boxes, not a cut through the mesh): the AIM-9M against the default Mk 82
pack **0.06 ft clear** (the AIM-9B 0.15, AA-2 0.19, AA-11 0.21, AA-8 0.34, AA-11B 0.06), at
both sweeps. Against the alternative row 7 stores the boxes cross by 0.04 ft clear (FAB-250)
to 0.79 ft (the HARM family: -0.79) because the missile's fins reach 1.03 ft from its centre
line and a bigger store's fins and body reach beyond 1.25 ft; as at TOR-6 a crossing of a
neighbour is not a refusal, and as on the F-14 ("its lower fins can touch a wide one",
John accepts a little clipping, 2026-10-14) the rail's body stays above the main store's
lugs whatever hangs there. Table: `checks/TOR-6b/matrix7/`.

**Airframe, gear.** Every kept AIM-9 reads **0.00 ft** entry into the airframe at 25 and
67 degrees; the gear-down legs are 5.5 ft (25 degrees) and 6.3 to 6.5 ft (67 degrees) from
the rail missile; its lowest point is 5.9 ft above the ground, gear down. The pack,
Paveways and the centreline tank are unchanged (0.00, 0.96 front pair, 0.00).

## The Tornado AIM-9 rail on the inboard flank, shorter, and the sweep from 350 mph (TOR-11, 2026-11-12)

**Opinionated (John, 2026-11-12): "I don't think for the tornado the wings should start
sweeping back until it's about 350 mph." and "The AIM-9 rail on the wing pylon is too long
and on the wrong side."** Update to the [TOR-5 swivel](#the-tornado-ids-swivelling-wing-pylons-and-a-stop-on-clipping-tor-5-2026-11-10)
and [TOR-6b side rail](#the-tornado-aim-9-side-rails-on-the-inboard-swivelling-pylons-tor-6b-2026-11-11)
sections above, which stay as written (their sweep schedule is the 2026-11-10 one and their
rail was on the outboard flank).

**Sweep start.** `swing::SWEEP_FROM_KNOTS` is 304 (350 mph, John), where it was a fitted 250;
`SWEEP_TO_KNOTS` stays 600. The wing is spread to 304 kt and fully swept (67 degrees) at 600 kt,
about 0.14 degrees a knot. The human ownship, the AI (`AiWings::carry_swing_wings`), the drawing,
replays and the animation probe all read the same function. 45 degrees is 445 kt (was 417 kt),
the scenarios' 200 kt and 650 kt are still the spread and fully swept poses.

**Which flank.** The rail is now on the pylon's **inboard** flank, facing the fuselage, which is
where the real Tornado's AIM-9 rails are (John's correction of the TOR-6b agent choice, which
had no source for either flank). A new `Location::InboardRail` (byte 0x84,
`INBOARD_RAIL_BYTE`) with `Hang::InboardRail` puts the missile 1.25 ft inboard of row 7's
pylon: 8.25 ft out, 1.50 ft forward, 0.74 ft under the reference at 25 degrees. The F-14's and
the F-15's shoulder rails are unchanged (`Location::Shoulder`, outboard); `Location::rail_out`
gives the signed offset, the one place the side lives, and `Rack::side_pylons`, the drawing
(`pylons::shoulder_rail`) and `station_move` read it. The offset is still added after the sweep,
so the rail stays 1.25 ft inboard of its pylon at every sweep, level with it fore and aft.

| Sweep | Row 7 pack, right / left (ft out, forward) | Row 8 AIM-9M, right / left (ft out, forward) |
| --- | --- | --- |
| 25 degrees (spread) | 9.50 / -9.50, 1.50 | 8.25 / -8.25, 1.50 |
| 45 degrees (445 kt) | 10.16, 0.27 / -10.20, 0.49 | 8.91, 0.27 / -8.95, 0.49 |
| 67 degrees | 10.36, -1.26 / -10.53, -0.81 | 9.11, -1.26 / -9.28, -0.81 |

Height 0.74 ft under the reference at every sweep (the pack's 1.45 ft); the rail is 0.53 ft
under the wing. Release and jettison use the same point (scenarios
`a2g-release-origin-tornado-rail`, `a2g-jettison-tornado-rail`).

**Length (fitted).** The drawn arm off the pylon, which is the rail the missile hangs from,
was the F-14's 0.6 of the missile, 5.61 ft on the 9.35 ft AIM-9M, longer than the 4.21 ft pylon
it hangs from (0.45 of the missile). It is now `INBOARD_RAIL_LENGTH` = **3.0 ft**, a third of the
missile, centred on its middle. Source: the real LAU-7 Sidewinder launcher is 110 in (9.2 ft) long
overall (Marvin Engineering LAU-7 data sheet, accessed 2026-11-12), nearly the missile's own
length, but that includes the launcher's forward housing and no public figure gives the span
of the rail that holds the missile's two launch shoes, so 3.0 ft is fitted (it is also the
F-14 arm's 3 ft floor). Drawing only: no clearance or release value reads it.

**Clearance (CPU stores preview, `.local/m3-aircraft/checks/TOR-11/`, default load, AIM-9M
on the rail).**

| Quantity | 25 degrees | 45 degrees | 67 degrees |
| --- | --- | --- | --- |
| Rail missile into the airframe (fuselage, glove, wing) | 0.00 ft | 0.00 | 0.00 |
| Gap to the row 7 Mk 82 pack (bounding boxes) | 0.06 ft | 0.06 | 0.06 |
| Gap to the row 6 Paveways (right side) | 3.63 ft | 4.30 | 4.49 |
| Gap to the centreline tank | 6.58 ft | 7.24 | 7.44 |
| Gear-down legs (right) | 3.09 ft | 3.71 | 3.90 |
| Lowest point above the ground, gear down | 5.86 ft | 5.86 | 5.86 |

The gear is the model's three gear pieces (two main legs, the nose leg); the model has no gear
door faces, so there is no door to measure. The pack, the Paveways and the tank are unchanged.
The side is symmetric about the pylon, so the pack-and-rail gaps equal TOR-6b's outboard ones:
the other six kept missiles (AIM-9B, AIM-9X, AA-2, AA-8, AA-11, AA-11B) read 0.00 ft into the
airframe at all three sweeps and 0.06 to 0.34 ft from the pack; the four refused ones read the same
entries as before on the inboard flank (R550 0.40, AT12 0.44, PL7 0.50, GBU12 1.02 ft, Cheat loading), so
the refusals, the 4,581 placements and the 222 Cheat-only placements do not move. Against the row 7
alternatives the boxes cross as in TOR-6b (FAB-250 0.04 ft clear to the HARM family -0.79 ft; Load
Ordnance only, not a refusal, John accepts a wide store touching the rail's missile). Table:
`checks/TOR-11/`.

**Not changed.** The mask, refusals, the capacity (one a side), `swing::swivels` (rows 7 and 8) and
the F-14 and F-15 shoulder rails.

## Two Tornado IDS versions and the Combat version outer wing pods (TOR-13, 2026-11-12)

**Opinionated (John, 2026-11-12), implemented by slice TOR-13** (the pods' models are TOR-13a,
[`assets/pods/`](../../assets/pods/README.md)). John: "lets go two versions to TORE, 1 is the
(Combat) which has the ECM and countermeasure pod on the outer wing pylons we removed earlier (not
configurable), and the second is (Clean) which doesn't have them on the outer wing pylons today but
doesn't have the jammer or chaff/flares." The pods are the ARI 23246 Sky Shadow ECM pod on the
**left** (port) pylon and the BOZ-107 chaff and flare dispenser on the **right** (starboard) one
(John's labelled photo; the Panavia GR4 page agrees).

### Identities

| | Tornado IDS (Combat) | Tornado IDS (Clean) |
| --- | --- | --- |
| Key (`--aircraft`, mission text, recordings) | `tornado` (as before; also `tor`, `tor.pt`, `tornado-combat`) | `tornadoc` (also `tornado-clean`) |
| `AircraftId` | `Tornado` (wire index 44, checkpoint code 44) | `TornadoClean` (wire index 45, checkpoint code 46) |
| Label | `Tornado IDS (Combat)` (was `Tornado IDS`) | `Tornado IDS (Clean)` |
| Record, shape, rig, flight model, cockpit, HUD, radar, guns, weapon rows and defaults | `TOR.PT` and its own | the same, unchanged |
| Outer wing pylon pods | Sky Shadow left, BOZ-107 right, fixed | none |
| Jammer, chaff, flares | the record's `MIG31.ECM`: jammer, 60 chaff, 60 flares | none |

`AircraftId::TornadoClean` follows the F/A-XX precedent: a runtime variant of its own identity,
never an alias (`AircraftId::source()` names the record it shares, as the F/A-XX's names the
F-22N's). It has its own selection key, label, checkpoint code and place in `SELECTABLE` (right after
the Combat version, so the Mirage F1's wire index moves from 45 to 46; both are unreleased, protocol
83), its own recordings key (`identity_key`), seat label, replay type name and Quick Mission list
entry (the list is sorted by label: Clean, then Combat). Every table the F/A-XX touches that the
Tornado shares was read once for the variant: the rig and animation (`variety_animation`,
`engine_material`, `hud_aperture`, the animation probe), the swing wing (`swing::swivels`,
`has_swivel`, `Airframe::swept_surface`, the vapor point), the station moves and refusals
(`hardpoints::station_move`, `aircraft_stations::weapon_allowed`, `default_counts`), the debris scale,
the variety flight model's identity, the live-fire permitted weapons and the AI roster
(`ai_wings` parses the record for the actor's own identity). `Aircraft::parse` never yields a
variant: every reader that flies or loads an identity takes the record through
`Aircraft::into_identity` or `parse_as`, which applies the variant's station table.

### The pods are no hardpoint row

The Combat pods are **not** source rows and not appended rows: they are `AuthoredPod`s
(`aircraft_stations::authored_pods`), station indices 9 (right) and 10 (left) past the nine source
rows. `Configuration::from_source` builds each as a `FixedPod` on the aircraft's configuration,
exactly as a retail fixed pod (EQ-2) is, so Load Ordnance (Normal and Cheat), saved loads, the lobby,
the AI's loads and the audit see nothing to offer or change: `--validate-ordnance` pins do not move
and a new audit step (`tornado_versions`) checks that both versions offer the same five rows, that no
offered row is a pod station, and that the pods' mass and jammer differences are exact. The pods
ride the swinging wing: `swing::swivels` lists stations 7 to 10 and `station_move` puts them on the
`Location::Wing` panel outboard of the inner pylon by the same rule as rows 7 and 8, so the sweep
carries them and the wing's own position (TOR-12) moves them. Each is a lone, unpaired store, the
right one at +x, the left at -x (`Rack::side_pylons` takes the wing to turn about from the point's
side, where it took it from the paired-side index).

**How a pod model is keyed when no retail record names it.** Retail equipment pods are drawn by their
record name after `TORE-POD:` in `StorePose::shape`. These two have no record, so their key is an
authored identifier, never a file name: `SKYSHADOW` and `BOZ107`, the same strings in
`aircraft_stations::{SKY_SHADOW, BOZ_107}`, `store_sizes::equipment_pod_body` (drawn size) and
`pod_models::MODELS` (the model). Nothing reads a file by that key.

### Numbers

| | Sky Shadow (left, row 10) | BOZ-107 (right, row 9) |
| --- | --- | --- |
| Length by diameter | 3.0 m by 0.42 m, 9.84 by 1.38 ft | 4.0 m by 0.48 m, 13.12 by 1.58 ft |
| Carried mass | 200 kg, 441 lb | 325 kg, 717 lb |
| Sources | Wikipedia, "Sky Shadow (radar jammer)": around 3 m and 200 kg; diameter fitted from John's photo | armedconflicts.com's database: about 4 m and 325 kg (secondary, approximate); diameter fitted |
| Model | 108 triangles, yellow triangle each side | 122 triangles, nine outlets on the aft face |
| Station | 16.0 ft out, 1.5 ft forward of the reference (fitted), `Hang::AsRecorded` from the wing's underside. Update (TOR-12, 2026-11-12): 14.46 ft out, 1.7 ft forward, the same place on the moved wing | the same, mirrored |

Mass and drag go through the existing path: the pods' 1,158 lb are in
`fixed_external_equipment_lbs` and so in the aircraft's carried mass (`payload_lbs`, the Loadout
total) and its `loadedDrag` law; TORE has no per-store drag model, so none is invented. The Combat
default load is 1,158 lb heavier than before; the Clean version is exactly the previous Tornado.
The position (16.0 ft out is 70 percent of the 22.8 ft semispan, 6.8 ft inboard of the spread tip,
6.5 ft outboard of the inner pylon) is the one fitted number: no source gives the outer pylons'
span, only that they carry pods and sit outboard of the inner pair.

### Whole-surface gate (`TORE_STORES_PREVIEW_PODCLEAR=1`, `TORE_STORES_PREVIEW_SWEEP`)

The preview samples each pod's drawn model and the airframe as drawn at that sweep, wing panel,
flap strips, tailerons, fin, glove plates and fuselage, a quarter foot apart both ways; the other
stores come from their drawn pylons and bodies; the gear is the rig's at gear 1. Gaps in feet,
smallest of the two pods (BOZ-107 first), default load; scripts and logs `.local/m3-aircraft/checks/TOR-13/`.

| Against | 25 degrees | 45 degrees | 67 degrees |
| --- | ---: | ---: | ---: |
| Wing panel (the pylon's own 1.0 ft) | 1.04 / 1.03 | 1.04 / 1.04 | 1.06 / 1.05 |
| Flap strips, flaps up | 1.19 / 1.17 | 1.19 / 1.22 | 1.48 / 1.97 |
| Flap strips, flaps down (wing held at 25) | 0.63 / 0.96 | n/a | n/a |
| Tailerons (neutral) | 8.83 / 9.86 | 7.01 / 8.21 | 3.98 / 5.41 |
| Fin | 18.1 / 19.0 | 16.9 / 17.7 | 15.1 / 15.7 |
| Glove plate | 5.93 / 5.40 | 6.46 / 6.08 | 6.33 / 6.43 |
| Fuselage | 8.73 / 8.15 | 9.00 / 8.41 | 8.05 / 7.64 |
| Row 7 store (Mk 82 pack) | 5.34 / 5.01 | 4.92 / 5.01 | 3.61 / 3.70 |
| Row 8 AIM-9 on its inboard rail | 5.63 / 5.69 | 5.24 / 5.34 | 4.15 / 4.28 |
| Gear legs, gear down | 10.61 / 10.71 | n/a | n/a |
| Ground, lowest point above the gear contact | 4.76 / 4.96 | | |

Alternative row 7 stores (MK 84, FAB-500, AGM-88, LAU-10, CBU-87, GBU-38, B-8, two each): the
closest is the AGM-88, 4.52 / 4.67 ft at 25 degrees and 3.07 / 3.02 ft at 67; none touches. Nothing
crosses at any sweep and no pod enters the wing, a flap strip, a taileron, the gear or the ground,
so no move was needed. Left unmeasured: the tailerons at deflection (they turn about a lateral
spindle at x 6, so their lateral distance to a pod at x 14 to 17 does not shrink), and the TOR-12
wing positions (the stations are relative to the wing).

### Chaff and flares from the BOZ-107

On the Combat version chaff and flares leave the centre of the BOZ-107's aft face
(`DISPENSER_EXIT`, model (0, -0.5, 0): on the pod's axis, half its length aft of the pod's centre),
heading aft with the aircraft's velocity as before (a flare still leaves as the usual mirrored pair
thrown to each side). `Configuration::dispenser_exit` works the point out from the pod's mount, its
size and the wing's present sweep (`rack.turn`, set every tick from the aircraft's speed and flaps
on the host and for each AI aircraft, the same on every peer); `countermeasure_release` bakes it into
the release's position (`Release::from_outlet`: the standard tail offset is undone, so the device
leaves at the outlet), which is what the wire, a recording and the energy decoys already read, so no
peer needs the sweep to place it. Numbers, aircraft frame (right, up, forward, feet): 25 degrees
(spread) 16.00, -1.79, -5.06; 45 degrees 16.27, -1.79, -8.52; 67 degrees 15.19, -1.79, -12.17. The
standard point (0, -2, -15) still serves every other aircraft and the Clean version (which has
nothing to release). `--countermeasure-preview` prints each device's offset two ticks after a
release; the battery scenarios `flight-countermeasures-tornado-boz` and `...-sweep67` check it. Update (TOR-14, 2026-11-13): the pod is centred on the wing's mid-chord, so the outlet is at 14.46, -1.67, -8.78 at 25 degrees, 13.34, -1.67, -12.28 at 450 kt (45.7 degrees) and 10.98, -1.67, -15.22 at 67 ([TOR-14](#the-tornados-pylons-centred-on-the-wing-one-size-at-every-sweep-tor-14-2026-11-13)).

### The Clean version, and what the record ties together

`MIG31.ECM` is one record: the deception jammer (radar and infrared chances), 60 chaff and 60 flares
come together, so Clean's jammer row holds no store (`stations::variant`, the way the F-5E's B5-2
interim row did) and the three go together. The sensor profile reads a jammer only from a row that
holds one; `Configuration::ecm` is `NONE`. **The RWR is not tied to it**: `TOR.PT` has no RWR
equipment row (`rwr_hardpoint` is `None` on both versions) and the warning receiver the scope shows
is the shared one, so it stays on the Clean version. The weapons window lists no `CHAFF` or `FLARE`
count for an aircraft with no dispenser at all (a `0 CHAFF` would say it ran out; this applies to
any aircraft whose record has none, not only the Clean Tornado), and the RWR window never lights
`JAM` for an aircraft with no jammer record, so the jammer key does nothing visible either (it never
did anything to a seeker: jamming is gated on the jammer profile).

### Guards

`the_two_tornado_versions_are_distinct_identities_on_one_record` (`tore-formats`),
`the_clean_tornado_loses_only_the_jammer_row_and_the_pods_are_the_combats` and
`only_the_combat_tornado_carries_the_authored_pods_with_real_sizes_and_masses`
(`aircraft_stations.rs`), `the_tornados_outer_pod_pylons_ride_their_own_wing_at_every_sweep`
(`hardpoints.rs`), `only_the_tornados_wing_pylons_and_the_outer_pod_pylons_swivel` (`swing.rs`),
`a_release_from_an_outlet_leaves_from_that_point` (`countermeasures.rs`),
`a_dispenser_pod_releases_the_devices_from_its_aft_face_and_follows_the_wing` (`live.rs`),
`the_clean_tornado_round_trips_at_code_46_and_shares_no_code` (`checkpoint_tests.rs`), the retail
`the_combat_tornado_carries_its_two_pods_and_the_clean_one_none` (`tore-world`), the pod model guards
`authored_pod_models_stay_low_poly_and_inside_their_size`, `only_the_five_authored_records_draw_their_model`
and `the_tornado_pods_take_their_authored_colours` (`pylons.rs`), and the audit's `tornado_versions`
step. **Palette (frames, `.local/m3-aircraft/frames/TOR-13/`):** the pods' Medium Sea Grey
(0.42, 0.47, 0.50) takes palette entry 76 (97, 113, 130), the BOZ-107's outlet colour (0.27, 0.29, 0.31)
entry 56 (73, 73, 73) and the Sky Shadow's yellow (1.0, 0.82, 0.0) entry 187 (247, 194, 40), an
orange-leaning yellow: the palette has a yellow, only slightly darker and warmer than asked.

## The Tornado's wing placed, pylons moved with it (TOR-12, 2026-11-12)

**Opinionated (John, 2026-11-12, from his Blender review of the wing; the stations' new numbers
are agent arithmetic on his).** Update to the
[TOR-5b inner wing pair](#the-tornado-ids-inner-wing-pair-on-the-wing-one-centreline-tank-a-glove-plate-tor-5b-2026-11-11),
the [TOR-6b side rail](#the-tornado-aim-9-side-rails-on-the-inboard-swivelling-pylons-tor-6b-2026-11-11),
the [TOR-11 inboard rail](#the-tornado-aim-9-rail-on-the-inboard-flank-shorter-and-the-sweep-from-350-mph-tor-11-2026-11-12)
and the [TOR-13 pods](#two-tornado-ids-versions-and-the-combat-version-outer-wing-pods-tor-13-2026-11-12):
the wing panel is 1.54 ft further in, 0.20 ft further forward and 0.1167 ft higher, its
root is 2.3483 ft further inboard, the left wing mirrors the right exactly and the sweep
pivot is (4.3333, -0.9007, 0.31) ft ([the wing placement](../formats/aircraft-control-surfaces.md#tornado-wing-placement-tor-12-2026-11-12)).
The pylons stay where they were **on the panel**, so they move with it: no new fitted
number is chosen, `TORNADO_WING_INNER` and `TORNADO_WING_OUTER` are the old values minus the
placement (`swing::WING_IN_FT`, `WING_FORWARD_FT`).

| Station | Before (spread) | Now (spread) | 67 degrees, both sides now |
| --- | --- | --- | --- |
| Row 7 Mk 82 pack, `station_move` | 9.50 ft out, 1.50 forward, up -1.45 | **7.96 out, 1.70 forward, up -1.33** (1.00 ft pylon, kept) | 8.77 out, 1.39 aft, up -1.33 (was right 10.36 / left -10.53, 1.26 / 0.81 aft) |
| Row 8 AIM-9M rail, inboard flank | 8.25 out, 1.50 forward, up -0.74 | **6.71 out, 1.70 forward, up -0.62** (still 1.25 ft inboard of row 7) | 7.52 out, 1.39 aft (was right 9.11 / left 9.28) |
| Row 9 BOZ-107 (right) | 16.0 out, 1.5 forward | **14.46 out, 1.70 forward** | 13.60 out, 5.74 aft |
| Row 10 Sky Shadow (left) | -16.0, 1.5 forward | **-14.46, 1.70 forward** | -13.60, 5.74 aft |
| Chaff and flare outlet (BOZ-107 aft face) | 16.00, -1.79, -5.06 | **14.46, -1.67, -4.86** (follows the pod) | 13.60, -1.67, -12.31 aft at 67 |

The left side is an exact mirror at every sweep (it was 0.45 ft out of mirror fore and aft at
67 degrees). Heights rise by the panel's 0.1167 ft: the simulation reads the retail shape for
the surface a pylon hangs from, so `placed_on` adds `swing::WING_UP_FT` to the surface samples
of the four swivelling rows (the drawing reads the placed shape, and the two now agree: the
pylon is 1.00 ft deep, 0.53 ft for the rail, as before; before the fix the preview read 1.12
and 0.65). The swivel and the sweep schedule are unchanged: the pylon turns back by the wing's
turn about the new pivot, so each store is streamwise at every sweep.

Update (TOR-14, 2026-11-13): the forward places and 67 degree places in this table are superseded: the pylons are centred on the panel's mid-chord, 7.96 ft out and 0.38 ft aft (rows 7 and 8, 6.71 ft out), 14.46 ft out and 2.22 ft aft (rows 9 and 10), see [TOR-14](#the-tornados-pylons-centred-on-the-wing-one-size-at-every-sweep-tor-14-2026-11-13).

**Clearance after the move** (CPU stores preview, default load, Combat version; scripts and
logs `.local/m3-aircraft/checks/TOR-12/`; gaps in feet, quarter-foot samples both ways, 0.00
is the pylon's own contact with the wing).

| Measure | 25 degrees | 45 degrees | 67 degrees |
| --- | ---: | ---: | ---: |
| Mk 82 pack and AIM-9M against the airframe (clip) | 0.00 / 0.00 | 0.00 / **0.11** | 0.00 / 0.00 |
| Pods BOZ-107 / Sky Shadow: wing panel | 1.04 / 1.05 | 1.05 / 1.05 | 1.06 / 1.09 |
| Pods: flap strips, flaps up | 1.19 / 1.19 | 1.19 / 1.34 | 1.49 / 2.18 |
| Pods: flap strips, flaps down (wing held at 25) | 0.63 / 1.12 | n/a | n/a |
| Pods: tailerons (neutral) | 7.71 / 8.74 | 5.80 / 6.83 | 2.62 / 3.65 |
| Pods: fin | 16.9 / 17.9 | 15.5 / 16.2 | 13.5 / 13.9 |
| Pods: fuselage (the new walls included) | 7.20 / 7.23 | 7.61 / 7.73 | 7.05 / 7.43 |
| Pods: Mk 82 pack | 5.34 / 5.46 | 4.92 / 5.01 | 3.61 / 3.70 |
| Pods: AIM-9 rail | 5.63 / 5.69 | 5.24 / 5.34 | 4.15 / 4.28 |
| Pods: gear legs, gear down | 9.09 / 9.19 | n/a | n/a |
| Pods: lowest point above the gear contact | 4.88 / 5.07 | 4.88 / 5.07 | 4.88 / 5.07 |
| Mk 82 pack: flap strip, flaps up / down | 0.91 / 0.48 | 1.01 | 1.13 |
| Mk 82 pack: taileron | 3.82 | 3.52 | 2.64 |
| Mk 82 pack: fuselage | 1.15 | 1.84 | 2.29 |
| AIM-9 rail pylon: fuselage | 0.06 | 0.01 | 0.12 |
| Mk 82 pack: gear legs, box to leg points | 2.97 | 3.62 | 3.75 |
| AIM-9M: gear legs | 1.85 | 2.38 | 2.49 |
| Lowest store above the ground, gear down | tank 0.46, Paveways 0.50 / 0.86, Mk 82 5.53, AIM-9M 5.98 | same | same |

(Before: the Mk 82 pack was 5.29 / 5.46 ft from the gear at 25 degrees and 5.3 to 5.5 at 67; the
pods' wing, strip and fuselage gaps were 1.04 / 1.03, 1.19 / 1.17 and 8.73 / 8.15 at 25.) The
pack is closer to the gear legs because it is 1.54 ft further in, but never within 1.8 ft.
Row 6 and the tank are unchanged (0.96 ft fin tips, accepted). The Mk 82, MK 84, GBU-38, CBU-87,
FAB-250, FAB-500, LAU-10 and B-8 on row 7 read 0.00 at 25 and 67 degrees; the AGM-88 reads 0.01 ft
at 25 degrees (was 0.00). The GBU-30 is refused on row 7 in normal loading, as since TOR-6.

**A finding for John (not guessed).** The AIM-9M rail's pylon top and its inner fin tips enter the
fuselage by **0.05 ft at 35 degrees, 0.11 ft at 40 and 45 degrees**, and 0.00 at 25, 30 and from 50
degrees (sweeps 25, 30, 35, 40, 45, 50, 55, 60, 65 and 67 measured). The rail is 6.71 ft out, and
the pylon is about 0.1 ft wide, so its inner edge reaches the fuselage's new outboard glove wall
(x 6.667 ft) while the wing sweeps the rail past the fuselage edge that item 8 moved forward
(4.84 ft). It is under the 0.3 ft refusal line and no catalog row changes, but it is an entry, so
nothing was moved to hide it. Options: leave it; or move the rail 0.15 ft outboard (its 1.25 ft
inboard offset to about 1.10 ft, a one-line `SHOULDER_OUT` change that also moves it nearer the
Mk 82 pack, whose gap is 0.06 ft now); or move the fuselage wall's forward edge back.

## The Tornado's pylons centred on the wing, one size at every sweep (TOR-14, 2026-11-13)

**Opinionated (John, 2026-11-13); the mid-chord definition, the fixed pylon size and the numbers
that follow from them are agent decisions, fitted.** John: "lets fix the pylons on the tornado, all
pylons need to be centered on the wing always point forward but have center position locked on the
wing as it sweeps back. We also need to fix pylon sizes because when wings are swept forward the out
pylons with the pods grow in size." Update to the
[TOR-5b inner wing pair](#the-tornado-ids-inner-wing-pair-on-the-wing-one-centreline-tank-a-glove-plate-tor-5b-2026-11-11),
the [TOR-6b side rail](#the-tornado-aim-9-side-rails-on-the-inboard-swivelling-pylons-tor-6b-2026-11-11),
the [TOR-13 pods](#two-tornado-ids-versions-and-the-combat-version-outer-wing-pods-tor-13-2026-11-12)
and [TOR-12](#the-tornados-wing-placed-pylons-moved-with-it-tor-12-2026-11-12): the numbers those
sections give for the stations' forward places, the 67 degree places, the pylon lengths and the
outlet are superseded here (their text is history). Both versions (the Clean one has rows 7 and 8).

### Why the pods' pylons grew

Not the wing's new height (`WING_UP_FT` is 0.117 ft). The drawn pylon was built from the wing under
it: 0.45 of the store's length (5.9 ft under the 13.1 ft BOZ-107, 3.3 ft under a Mk 82 pack, at least
twice its depth), centred on the store's *middle* rather than on the station, and trimmed fore and
aft to wherever the model's surface reached (`along_surface`), with its top found by searching up to
12 ft for a surface and, failing that, reaching aft toward the station for one
(`hanger_attachment`). The outer pylon stood 1.7 ft forward of the reference point, but with the wing
spread (25 degrees) the panel's leading edge there is only 0.51 ft forward: the pylon was 1.2 ft
**ahead of the wing**, so the middle had no surface over it, the slab kept its whole 5.9 ft at a flat
height and stood in the open ahead of the leading edge, a large grey slab with its top face showing
from above (John's 08-40 screenshot). As the wing swept, the panel moved under the pylon, the
trim cut the slab down to the surface over it and the pylon looked smaller (08-39). The inner pylons
sat on the wing at every sweep (the chord there is 6.4 ft), so they only changed by the trim.

### What changed

1. **Mid-chord.** `swing::mid_chord(span)` is the forward coordinate, wing spread, halfway between
   the placed right panel's leading edge and its trailing edge (the flap strip's outer edge, flaps up)
   along the streamwise line at `span` feet out. The edges are the drawn panel's own corners
   (`swing::WING_LEADING`, `WING_TRAILING`: retail corners (9, 7) to (31, -2) and (10, -4) to (30, -9)
   shape units through TOR-12's placement; `tornado_animation` has a test that its `wing_point`
   reproduces them, and the preview's `Pylon centre:` line reads the drawn panel's chord back at
   every sweep). The left panel mirrors. `TORNADO_WING_INNER` and `TORNADO_WING_OUTER` keep their
   spans (7.96 and 14.46 ft out, no span moved) and take `mid_chord` of them as their forward place.
2. **Locked to the wing.** Unchanged in kind: `Rack::side_pylons` carries the pylon's centre with
   `swing::swept` about the wing's pivot, the height unchanged, so it is the same point of the panel
   at every sweep and through the flaps' hold (the flaps hold the wing at 25 degrees; the strips
   droop about their own hinge, never moving the pylon). Row 8's rail keeps its place relative to
   row 7's pylon: the pylon is row 7's, the rail's store hangs 1.25 ft inboard of its centre line
   and that offset is not turned with the wing.
3. **Always forward.** Unchanged: the pylon is never turned, the wing's turn is undone by
   `pylon_swivel`, and the store and the pylon rotate about their own centre (the carried point),
   not about any other point. The drawn pylon is now centred on that point (it was centred on the
   store's middle, 1.2 ft away for an AIM-9M and -0.9 ft for a Mk 82).
4. **Constant size.** The drawn pylon of every swivelling row is `swing::PYLON_LENGTH_FT` long
   (3.3 ft, fitted: the Mk 82 pack's old length, 0.6 of the 5.45 ft chord at the pods), `PYLON_WIDTH`
   (0.28 ft) wide and from the stores' lugs up to the wing's underside, `PYLON_DEPTH` (1.0 ft) over
   the simulation's lugs, plus the 0.02 ft sink. It is no longer measured against the model, so it is
   the same at every sweep, for every store, and never reaches the wing's top face (the panel is
   0.67 ft thick). Rows 7 and 8 draw the same pylon. The sampling of the retail wing for the
   stations' height now reads the retail wing at the placed point's retail place (1.54 ft outboard and
   0.20 ft aft, TOR-12's placement); the wing is flat there, so the heights did not change.

| Station (feet, aircraft frame right / forward) | Before (TOR-12) 25 | Now 25 | Before 67 | Now 45 degrees | Now 67 |
| --- | --- | --- | --- | --- | --- |
| Row 7 Mk 82 pack's pylon centre | 7.96 / 1.70 | **7.96 / -0.38** | 8.77 / -1.39 | 7.92 / -1.65 | **7.38 / -2.94** |
| Row 8 AIM-9M rail (1.25 ft inboard) | 6.71 / 1.70 | **6.71 / -0.38** | 7.52 / -1.39 | 6.67 / -1.65 | **6.13 / -2.94** |
| Row 9 BOZ-107 pylon centre (right) | 14.46 / 1.70 | **14.46 / -2.22** | 13.60 / -5.74 | 13.40 / -5.60 | **10.98 / -8.66** |
| Row 10 Sky Shadow (left) | -14.46 / 1.70 | **-14.46 / -2.22** | -13.60 / -5.74 | -13.40 / -5.60 | **-10.98 / -8.66** |
| Chaff and flare outlet (BOZ-107 aft face), up -1.67 | 14.46 / -4.86 | **14.46 / -8.78** | 13.60 / -12.31 | 13.34 / -12.28 (450 kt, 45.7 degrees) | **10.98 / -15.22** |

Heights (up) are unchanged: row 7 -1.33, row 8 -0.62, the BOZ-107 -1.67, the Sky Shadow -1.57. In the
wing's own frame the centres are 7.96 ft out, 0.382 ft aft of the reference point and 14.46 ft out,
2.219 ft aft; the drawn chord there is 6.36 ft (2.80 forward to 3.56 aft) and 5.45 ft (0.51 forward
to 4.94 aft), and `Pylon centre:` reads the centres 0.002 and 0.004 ft off the middle at 25, 45 and
67 degrees. The old stations were 2.08 ft (inner) and 3.92 ft (outer) ahead of the middle. Release
and jettison origins, the AI's, and the chaff and flare outlet (`Configuration::dispenser_exit`,
`Release::from_outlet`) follow with no change of code, because they read the pylon's point.

**The store's own origin.** The pylon is centred on the station's point and the store's origin hangs
there too (the simulation takes a store's origin as its middle). A retail shape's origin is not always
its body's middle: the Mk 82's body is 0.87 ft aft of its origin and the AIM-9M's 1.21 ft ahead, so
those two hang off their pylon's centre by that much (before, the pylon followed the body). The pods
are centred exactly (their model's origin is the middle). An agent decision: the alternative, moving
each store by its own offset, needs the offset in the simulation, which the size table does not carry.

### Guards

`the_tornado_pylon_centres_stay_on_the_wing_and_point_forward_at_every_sweep` (`hardpoints.rs`: the
four rows, both sides, 25 to 67 degrees: the centre carried back to the spread pose is the same point,
that point is the mid-chord at its span, the heading is zero, the radius about the pivot is kept),
`the_mid_chord_is_halfway_between_the_placed_panels_edges` (`swing.rs`),
`the_stations_mid_chord_edges_are_the_placed_panels_corners` (`tornado_animation.rs`),
`a_swivelling_pylon_is_the_same_size_at_every_sweep` (`pylons.rs`: length, depth, centre, at six
sweeps, on a flat wing that ends well short of the pylon), and the preview's `Pylon centre:` lines on
the real wing.

### Clearance after the change

CPU stores preview, default load, Combat version (the Clean version's rows 7 and 8 read the same);
scripts and logs `.local/m3-aircraft/checks/TOR-14/` (`clear3.sh`, `default/`, `clean/`); gaps in feet,
quarter-foot samples both ways; 0.00 is a touch or a crossing (the gap tool cannot tell which; the
`Taileron entry:` lines say how deep a crossing goes). `TORE_STORES_PREVIEW_PODCLEAR=1` now also
prints the `Pylon centre:` and `Taileron entry:` lines, and its `Wing store clearance:` lines cover
the pods' pylons (rows 9 and 10).

| Measure | 25 degrees | 45 degrees | 67 degrees |
| --- | ---: | ---: | ---: |
| Mk 82 pack / AIM-9M against the airframe (clip) | 0.00 / 0.11 | 0.00 / 0.11 | 0.00 / 0.11 (the tool prints 1.44: see below) |
| Pods BOZ-107 / Sky Shadow: wing panel (the pylon's own depth) | 1.00 / 1.00 | 1.00 / 1.00 | 1.00 / 1.00 |
| Pods: flap strips, flaps up | 1.05 / 1.05 | 1.05 / 1.05 | 1.06 / 1.08 |
| Pods: flap strips, flaps down (wing held at 25) | 0.52 / 0.56 | n/a | n/a |
| Pods: tailerons (neutral) | 5.40 / 6.43 | 2.55 / 3.57 | **0.00 / 0.01** |
| Pods: fuselage (the new walls included) | 7.36 / 7.49 | 6.83 / 7.20 | 5.76 / 6.02 |
| Pods: Mk 82 pack | 5.32 / 5.42 | 4.26 / 4.36 | 2.36 / 2.48 |
| Pods: AIM-9 rail | 5.63 / 5.71 | 4.69 / 4.81 | 3.00 / 3.17 |
| Pods: gear legs, gear down | 9.22 / 9.38 | n/a | n/a |
| Pods: lowest point above the gear contact | 4.88 / 5.07 | 4.88 / 5.07 | 4.88 / 5.07 |
| Mk 82 pack: wing panel, flap strip (flaps up) | 0.00, 0.00 | 0.00, 0.02 | 0.00, 0.33 |
| Mk 82 pack: taileron, neutral | 2.59 | 1.82 | 0.72 |
| Mk 82 pack: fuselage | 1.16 | 1.71 | 2.11 |
| Mk 82 pack: gear legs | 3.14 | n/a | n/a |
| AIM-9 rail: taileron, neutral | 1.98 | 1.15 | 0.00 (0.03 ft deep) |
| AIM-9 rail: fuselage | 0.01 | 0.01 | 0.01 |
| AIM-9M: gear legs | 2.68 | n/a | n/a |

Taileron entry (the deepest a crossing goes inside the taileron's outline, neutral and at full
pitch up, pitch down, roll right and roll left; both sides): **BOZ-107 0.57 ft at 67 degrees** (0.56
to 0.58 in every pose), the Sky Shadow 0.07 ft (up to 0.11 deflected), the AIM-9M rail 0.03 ft, the
Mk 82 pack 0.00; every row 0.00 at 25 and 45 degrees.

**Findings, nothing moved to hide them (not guessed).**

1. **The BOZ-107's tail enters the right taileron by 0.57 ft at full sweep** (over the 0.3 ft line):
   with the pylon on the mid-chord, the pod's aft 4 ft (the pod is 13.1 ft long, its tail 6.56 ft
   aft of the pylon) is over the taileron's forward outer corner at 67 degrees (the pod spans 10.19
   to 11.76 ft out, the taileron's leading edge there is 13.3 ft aft). Before TOR-14 the pod was 3.9 ft
   further forward and cleared it by 2.62 ft. Moving the outer pylon outboard does not cure it
   within the panel: 15.5 ft out, 0.54 ft; 16.5 ft out, 0.35 ft; 17.5 ft out clears it by 0.17 ft
   (the Sky Shadow 0.22) but is 3.0 ft further out than the fitted span (the panel's tip is 20.5 ft
   out) with the chord down to 5.0 ft. Left for John; options: leave it; move the outer pylons to 17.5
   ft (their mid-chord there is 2.9 ft aft); or move them forward of the mid-chord, which is what
   John asked not to do. Update (TOR-14b, 2026-11-13): resolved by hanging the pods from rear lugs, see [TOR-14b](#the-tornados-pods-hang-from-rear-lugs-tor-14b-2026-11-13).
2. **The AIM-9M rail's 0.11 ft entry into the fuselage (TOR-12's open item) is now at every sweep.**
   Re-measured at 25, 30, 35, 40, 45, 50, 55, 60 and 65 degrees: **0.11 ft at each** (it was 0.00 to 0.11,
   at 35 to 45 only): with the pylon on the mid-chord the missile hangs alongside the glove box
   wall at every sweep, where before only part of it lay beside the wall at 25 degrees. The fuselage
   gap reads 0.01 ft throughout. The wall is unchanged, as asked. At 67 degrees the clip tool prints
   1.44 ft at the missile's fin tip: that is the tool's own limit, it takes the lowest face over the
   point and finds the taileron plate 1.4 ft *under* the missile (taileron entry 0.03 ft); the glove
   wall's entry there is the same 0.11 ft.
3. **The Mk 82 pack and the AIM-9M rail are closer to the tailerons at full sweep** (0.72 and 0.00 ft,
   neutral; the rail's fin grazes the plate by 0.03 ft; were 2.64 ft for the pack).
4. **Flaps down, wing held at 25 degrees:** the pylons are now 3.3 ft long, centred on the mid-chord,
   so the aft 0.6 ft of the pack's pylon and 0.4 ft of the pods' lie under the flap strip's forward part
   and the drooped strip (0.4 rad) lowers onto them by up to about 0.2 ft (agent arithmetic from the hinge
   line: 0.6 ft aft of it times the sine of 0.4 rad), a drawn-geometry overlap under 0.3 ft.

## The Tornado's pods hang from rear lugs (TOR-14b, 2026-11-13)

**Opinionated (John, 2026-11-13); the lug station is fitted (agent decision, no source gives it).**
John: "Both pods are technically mounted towards the rear in RL have the agent do the same for these
and avoid the taileromms". Update to the TOR-14 section above: the pylon is where it was (centred on the
wing's mid-chord, locked to the wing, streamwise, 3.3 ft long, 14.46 ft out), and the BOZ-107 and the
Sky Shadow now hang from it by **lugs a third of their length aft of their middle**, the middle of the
pod's rear third (`AuthoredPod::lug_aft`: 4.37 ft on the 13.12 ft BOZ-107, 3.28 ft on the 9.84 ft Sky
Shadow). The pod's middle therefore hangs that far ahead of the pylon's centre and its nose stands well
ahead of the wing's leading edge. No source gives the real lug station; "the rear third" is John's
direction and the middle of that third is the fitted number. The Clean version carries no pods
(`authored_pods` is the Combat version's), so it is unchanged.

**How.** `StoreSize::lug_aft` (zero for every other store) is the new per-store attach offset;
`slot_offset` puts a lone store's middle `lug_aft` ahead of its pylon's point, so `store_point`, the
drawing's `hung` centre, the clearance tools and every release read it with no other change. The
simulation's `Configuration::dispenser_exit` adds it (the BOZ-107's aft face is `lug_aft - length / 2`
from the pylon's point: 2.19 ft aft of it), and `StorePose::size` carries it for the drawing. The drawn
pylon is centred on the point, so it covers the pod's lug area (a 3.3 ft slab over the pod's rear third).
The Mk 82 pack and the AIM-9M are not touched (their origins still hang at the pylon's centre).

| Feet (aircraft frame right / forward) | 25 degrees | 45 degrees | 67 degrees |
| --- | --- | --- | --- |
| Pylon point, both pods (unchanged) | 14.46 / -2.22 | 13.40 / -5.60 | 10.98 / -8.66 |
| BOZ-107 middle (right) | 14.46 / 2.16 | 13.40 / -1.23 | **10.98 / -4.28** |
| BOZ-107 nose to aft face | 8.72 to -4.41 | 5.33 to -7.79 | 2.28 to -10.84 |
| Sky Shadow middle (left) | -14.46 / 1.06 | -13.40 / -2.32 | **-10.98 / -5.38** |
| Sky Shadow nose to aft face | 5.98 to -3.86 | 2.60 to -7.24 | -0.45 to -10.30 |
| Chaff and flare outlet (BOZ-107 aft face), up -1.67 | 14.46 / -4.41 | 13.40 / -7.79 | **10.98 / -10.84** |

(Before: the pods' middles were on the pylon points, the outlet 6.56 ft aft of them, 15.22 ft aft of the
reference at 67 degrees.)

**Clearance** (CPU stores preview, default load, Combat version; scripts and logs
`.local/m3-aircraft/checks/TOR-14b/`; gaps in feet, quarter-foot samples both ways, BOZ-107 / Sky Shadow;
the nose is the pod's forward quarter):

| Measure | 25 degrees | 45 degrees | 67 degrees |
| --- | ---: | ---: | ---: |
| Taileron, neutral | 7.98 / 8.36 | 5.13 / 5.51 | **1.38 / 1.77** |
| Taileron, full pitch up and down, roll right and left (closest of the four) | 7.98 / 8.36 | 5.13 / 5.51 | **1.37 / 1.75** |
| Taileron entry (deepest crossing), any pose | 0.00 / 0.00 | 0.00 / 0.00 | 0.00 / 0.00 (was 0.57 / 0.11) |
| Wing panel (the pylon's own 1.0 ft) | 1.05 / 1.03 | 1.04 / 1.02 | 1.00 / 1.00 |
| Nose against the wing panel | 5.81 / 3.82 | 4.26 / 2.71 | 1.94 / 1.37 |
| Flap strips, flaps up | 1.19 / 1.17 | 1.19 / 1.17 | 1.19 / 1.18 |
| Flap strips, flaps down (wing held at 25) | 0.69 / 0.88 | n/a | n/a |
| Fuselage and glove walls (intakes included) | 7.20 / 7.27 | 6.26 / 6.47 | 4.29 / 4.92 |
| Nose against the fuselage | 7.67 / 7.51 | 6.43 / 6.48 | 4.29 / 4.92 |
| Gear legs, gear down (nose 10.62 / 9.89) | 9.10 / 9.18 | n/a | n/a |
| Mk 82 pack | 5.38 / 5.45 | 4.33 / 4.44 | 2.43 / 2.52 |
| AIM-9 rail | 5.66 / 5.69 | 4.62 / 4.67 | 2.79 / 2.86 |
| Row 6 Paveways | 10.27 / 10.56 | 8.95 / 9.01 | 6.46 / 6.53 |
| Lowest point above the gear contact (the ground) | 4.88 / 5.07 | 4.88 / 5.07 | 4.88 / 5.07 |

Both pods clear both tailerons at 67 degrees, neutral and at full deflection, by more than five times the
0.25 ft asked for (1.37 ft at the least), so the lug station is not at the edge of what works; the 0.57 ft
entry of TOR-14 is gone. The wing's rows are as in TOR-14 (the Mk 82 pack 0.59 ft and the AIM-9M rail 0.00
ft, 0.03 ft deep, from the tailerons at 67 degrees; the rail's 0.11 ft entry into the glove wall at every
sweep): they were not moved. At 67 degrees the nose stands 1.9 and 1.4 ft under the panel's leading
edge. Guard: `the_pods_hang_from_rear_lugs_and_clear_the_tailerons_at_full_sweep` (`pod_clearance.rs`:
the lug offset, the mid-chord, and 0.25 ft from both tailerons' rig poses at 67 degrees), with
`StorePose::size` and the dispenser outlet pinned in `tornado_versions_tests.rs` and `live.rs`.
`TORE_STORES_PREVIEW_PODCLEAR=1` now also prints `Pod nose:` lines and the gap beside each `Taileron
entry:`.

## F-117 weapon bays (F117-5, 2026-11-15)

Implementation specification, **opinionated** (John, 2026-11-15, decision 25: two fitted
belly bays with two doors each, default two GBU-27; extra loads only where the weapons data
has them). Every number is **fitted**, agent-authored, F117-5. The door count is fitted: the
real count was not found. This is a game configuration, not a claim of original behaviour.
The research and the shape measurements are in
[F-117 weapon bays](../formats/aircraft-variety.md#f-117-weapon-bays-f117-5-2026-11-15).

**Stations and the machinery.** `F117.PT` rows 4 and 5 (the retail internal bay rows,
`0x1100`, one store each) are `Location::Magazine` stations like the F-22's
([F-22 naval stores](#f-22-naval-stores)): drawn inside, no pylons, released from where they
hang through doors that open for the shot. The per-aircraft data is a table,
`combat::bays::STATIONS` (aircraft, row, mount), read by `placed_on`; `OPENINGS` holds the
belly cut-out of the aircraft whose doors `bay_doors.rs` draws. `State::bay_available()` is
true for every aircraft with a row in `STATIONS`. The B-2 and B-52 slices add rows and their
own belly face lists; no other code changes.

| Carrier | Magazine centre x / floor up / release origin forward, feet |
| --- | --- |
| F-117 left bay, row 4 | -3.0 / -2.78 / 0.0 |
| F-117 right bay, row 5 | +3.0 / -2.78 / 0.0 |

The floor is the lowest fin tip (the magazine hangs the axis half the fin span above it,
plus 0.08 ft), so the GBU-27's axis is 1.0 ft above the belly plane and its fins hang 1.7 ft
below it. The F117-0 estimate of +/-2.55 ft is replaced by 3.0: two GBU-27 fin spans (5.4 ft)
need 5.4 ft of centre spacing less the keel.

**Bays and doors (feet).** Belly plane -1.478. Openings: x 0.2 to 5.8 each side, 18.0 long
(-9.0 to +9.0 forward). Two doors a bay, 2.8 ft wide, hinged at the inner and outer edges,
swinging 90 degrees down in the flight's 1.00 s bay travel (open they hang to -4.28). The
recess roof is 2.3 ft above the belly plane (+0.822), under a top skin that is at least
2.4 ft above it over the openings. A lined box (ceiling and four walls) closes the recess,
and a trapeze rail (0.4 ft wide, 0.6 ft deep, -5.5 to +3.5 forward) hangs from each
ceiling. Drawn only while the doors are off their seat.

**Release.** The doors open before the weapon leaves and close after, as the F-22's
([bay trigger timing](aircraft-animation.md#f-22-main-weapon-bays)): the press asks for
the doors, the weapon leaves once they are at 0.95 (116 ticks, 0.958 in the actual one-press
probe `flight-bay-fire-f117-row4` and `-row5`), the doors stay open 1 s after the release and
close 1 s later; a pilot-opened bay releases at once. Weapons leave from the bay's mount,
the same point they hang from. **The AI** releases through the shared strike release, which
does not wait for doors (`strike_readiness` reads no bay state, as for the F-22): an AI
F-117's bay doors stay drawn shut while its bombs leave the bay, as an AI F-22's do. A fix
(the AI holding its doors open for a release) is listed for John.

**What each bay admits.** One store a bay. Retail mask `0x1100` and weight class 20: 19
stores (`PAVEWA3`, the row's default, and `GBU29`, `GBU29P`, `GBU30`, `GBU38`, `MK82`,
`MK82HD`, `MK82P3`, `MK84`, `CBU87`, `CBU89`, `MK20`, `RBK250`, `RBK500`, `FAB250`, `FAB500`,
`AGM88`, `AGM45`, `AT2`); plus the four Paveway II bombs `GBU10`, `GBU10A`, `PAVEWAY` and
`GBU12` through `extra_weapon` (the row's own count, no weight cap); `GBU16` refused by
name. The default is two `PAVEWA3`. Real load: two 2,000 lb class bombs, 5,000 lb in all.

**Clearance, feet, doors open.** The default GBU-27 against the door planes (0.10), the
bay's aft and forward ends (1.06 and 3.04), the trapeze rail (0.10), the other store (0.60),
the roof (-1.40, upper tail fin tips) and the fuselage's top skin (-0.62, upper tail fin
tips); lowest fin tips 1.70 ft below the belly plane, inside the doors. The measured
alternatives clear the doors and the ends by 0.1 ft or more (HARM 0.13 ft past the forward
end), so no store is refused for fit. The GBU-27's 5.4 ft fin span is wider than the
fuselage is thick: the one remaining clip is the upper tail fin tips through the dorsal
skin by 0.62 ft. Options for John: accept; carry the GBU-27 with its tail fins at a
smaller span in a bay (a fitted pose); or a deeper bay roof under a lowered store (more
fin below the belly, more door sweep through the fins). The doors also pass the lower fin
tips during the 1 s swing.

**Signature.** The open-bay term (1.5 times at full travel) applies to every bay aircraft,
the F-22's values unchanged; it reaches only the path the F-22's term reaches today, the RCS
page (`scope.rs` `rcs()` reads `State::bay`), not the combat rows radars read: no gear,
flap or bay state was put on them (John has not answered that). Bay stores add nothing to
the external-store signature.

Provenance: stations, bays, doors, rail, timing and the 3.0 ft mount fitted (agent
2026-11-15); the 1 s bay travel, the 0.95 gate and the hold are the F-22's; the refusal of
`GBU16` and the Paveway II admission applied from John's decision 25.

## B-2 weapon bays (B2-5, 2026-11-15)

Implementation specification, **opinionated** (John, 2026-11-15, decision 21: the magazine
extended to a grid, the pair flag cleared, the belly cut with door leaves). Every number is
**fitted**, agent-authored, B2-5; the door count is fitted (two a bay) because no public source
gave one. The research and the measurements are in
[B-2 weapon bays](../formats/aircraft-variety.md#b-2-weapon-bays-b2-5-2026-11-15). Update to
[F-117 weapon bays](#f-117-weapon-bays-f117-5-2026-11-15): the same machinery, plus the grid.

**Stations.** `B2.PT` rows 5 to 8 (the retail internal bay rows, flags `0x1100` once the
pair flag is cleared, `aircraft_stations::adjust_defaults`) are `Location::Magazine` stations
through `bays::STATIONS`:

| Row | Role | Mount (right / floor up / forward), feet |
| --- | --- | --- |
| 5 | Mk 82 row, left bay | -5.333 / -3.667 / 6.667 |
| 6 | Mk 82 row, right bay | +4.667 / -3.667 / 6.667 |
| 7 | Paveway III row, left bay | -5.333 / -4.567 / 6.667 |
| 8 | Paveway III row, right bay | +4.667 / -4.567 / 6.667 |

A row is one whole bay. The Mk 82 row and the Paveway III row of a bay are **alternatives**
(`bays::ALTERNATIVES`): loading one empties the other in `Loadout::select`, `change` and
`transfer`, and `validate_for_editing` refuses both loaded.

**The grid** (`bays::GRIDS`, `hardpoints::grid_point`, `grid_cells`, `grid_capacity`; carried
in the rack's `tip` as [width, length] and its `over[0]` as the ceiling, fields a magazine does not
otherwise use). A row with a grid places store `slot` of `loaded` stores by the store's size
(`StoreSize`): pitch `p` = the fin span plus 0.03 ft, at least the body plus 0.1 ft, at most
1.3 ft; abreast = floor width / `p`; along = floor length / (length + 0.25 ft); tiers as the
load needs, so a tier holds abreast x along stores. The B-2's floor is 5.3 ft x 19.33 ft with the
ceiling 5.5 ft over the belly polygon's plane (-3.667 ft). A store's axis stands half its fin span
plus 0.08 ft over the floor (its lowest fin tip on it) and a tier higher by `p`. Capacity is
capped by `grid_capacity`: tiers while the top fin tip (0.08 ft plus the fin span over the mount,
plus `p` for each tier under it) stays under the ceiling. A Mk 82 (7.3 ft, 1.27 ft fins): 4 abreast,
2 long, 4 tiers = 32; a GBU-27 (13.9 ft, 5.4 ft fins): 4 abreast, 1 long, 1 tier = 4; a GBU-38 24; a
CBU-87 18; a Mk 84 8. `Loadout::capacity_before_store_pairs` applies the cap; the ordnance audit
caps its expected count the same way. A row with no grid keeps the F-22's single lane.

**Release order.** The grid fills in the order it releases, so a store's place does not move as
the others leave: release number `r` is slot `loaded - 1 - r`, and `r` runs the bottom tier first,
front to back, the stores of a row abreast outermost first alternating sides (columns 0, last, 1,
last but one), then the next tier up. A part load fills the bottom tier first. A store leaves from its
own place (`Rack::release_point` is `store_point`). **The bays alternate**: after a bay release
`Ownship::sister_bay` hands the selection to the other gridded bay station that holds the same
weapon on the other side of the aircraft and has stores left (`set_selection`, the trigger marked
held so one press is one bomb); no other aircraft has a gridded bay, so nothing else changes.

**Doors and cut.** Belly polygon `396d` (z -11 units, one 7-vertex face) has both openings cut
out of it (`bay_doors::belly_hole`) while the doors are off their seat; the two painted door panels
`3c84` and `3c65` (z -12 units) are cut into the two doors of each bay and turned down about their
hinges (`belly_doors`, the F-117's helper). `Opening` gained `centre` (the bays mirror about the
mesh axis x -0.333 ft, not 0) and `inner_depth` (a roof that rises from 4.6 ft over the outer wall to
9.6 ft over the inner, under the top skin); an equal rail end means no rail. The lining takes the
shape's own bay-plate grey. Timing, hold and closing are the F-22's, and the AI's doors stay shut
while its bombs leave, as for the F-22 and F-117.

**Clearances, feet, doors open** (`TORE_STORES_PREVIEW_BAYCLEAR=1`, scenario
`render-b2-bay-clearance`; positive is clear, negative is how far the drawn store reaches into the
part). The default 64 Mk 82, worst of each bay: door planes 1.60, ends 1.37 aft and 3.11 fore, roof
0.32 left and 0.21 right, top skin 0.50 and 0.16, nearest store 0.25. The 40 Mk 82 a bay of the real 80
reach about 1 ft through the roof and skin (not used). Four GBU-27 a bay: door planes -0.65, ends 1.72 and
3.71, roof -0.31, top skin 0.04 and -0.15, nearest store 0.10 (the fins are wider than the bay: left for
John).

**Refused for fit.** The Mk 82 rows refuse `GBU29.JT`, `GBU29P.JT`, `GBU30.JT` and `MK20.JT` (more than
0.3 ft through the roof; `aircraft_stations::B2_MK82_REFUSED`); the Paveway rows refuse nothing.
Provenance: stations, grid, doors, roof, order and the 0.9 ft lower Paveway trapeze fitted (agent,
2026-11-15); the 1 s travel, the 0.95 gate and the hold are the F-22's.

## B-52 bay, pylons and tail gun (B52-5, 2026-11-15)

Implementation specification, **opinionated** (John, 2026-11-15, bomber batch A decisions 7, 8,
9, 10 and 13) and **fitted**, agent-authored, B52-5. A game configuration, not a claim of
original behaviour. The measurements and the choices are in
[B-52 bay, pylons and tail gun](../formats/aircraft-variety.md#b-52-bay-pylons-and-tail-gun-b52-5-2026-11-15).
It reuses the F-117's bay machinery
([F-117 weapon bays](#f-117-weapon-bays-f117-5-2026-11-15)): `bays::STATIONS` rows read by
`placed_on`, `bay_doors.rs`, the F-22's door timing.

**Stations (feet right / up / forward).**

| Row | Station | Mount | Capacity, class |
| ---: | --- | --- | --- |
| 3 | bay, forward row, `Location::Magazine`, shared grid | 0 / -6.0 / +13.87 | 9, 7,000 lb |
| 5 | bay, middle row | 0 / -6.0 / +5.20 | 9, 7,000 lb |
| 6 | bay, aft row | 0 / -6.0 / -3.46 | 9, 7,000 lb |
| 7 | underwing pair, `Location::Beam`, one beam a wing, record `[52, 8, 31]` | +/-17.33 / 2.67 (record; the stores' lugs hang 1 ft under the wing) / +10.33 | 18, 17,000 lb |
| 4 | M61 tail gun | 0 / +2.0 / -92.0 | 1,242 rounds |

**Bay (feet).** Belly plane -6.0 (flat belly panel, z -9 units at 2/3 ft a unit); opening x
+/-3.0, 26.0 long (-8.67 to +17.33 forward); recess 6.0 deep (roof 0.0); two doors 3.0 wide
hinged on the outer edges, swinging 90 degrees down in the flight's 1.00 s bay travel, hanging
to -9.0 (0.3 ft above the gear contact line, -9.33). A lined box (ceiling and four walls) closes
the recess under an intact top skin (13.3 ft above the belly). Four rails hang 0.6 ft from the
ceiling over the lanes (x -1.95, -0.65, +0.65 and +1.95 for a 500 lb class store on the shared
grid; they were three at -1.42, 0 and +1.42 on B52-5's own grid) and eight posts stand from the
floor to the ceiling at the two inner row boundaries; the lining draws only while the doors are
off their seat. Code: the B-52 row of `bays::OPENINGS` (`centre` 0, `inner` 0, `outer` 3.0,
`inner_depth` equal to `depth`), `bay_doors::{centre_doors, centre_lining}` (the one-bay variants
of the shared door helper, taking the opening and the shape's feet a unit), `b52_animation.rs`.

**Grid (`bays::GRIDS`, the shared magazine grid of B2-5).** Folded onto the shared grid at the
batch A integration (2026-11-15): B52-5's own per-aircraft `GRIDS`, `BayGrid`, `CentreBay` and the
`if let` in `Rack::store_point` are deleted. The three B-52 rows are rows of the shared `GRIDS`
(floor width `B52_ROW_FLOOR_WIDTH` 6.0 ft, length `B52_ROW_LENGTH` 8.67 ft, ceiling the roof,
`B52_BELLY + B52_ROW_CEILING` = 0.0 ft up of the reference point, flat so `inner_depth` equals
`depth`), the carried grid comes from `Rack::grid()`, and the layout follows `grid_cells` and
`grid_point`: stores stand a span plus 0.03 ft apart (at most 1.3 ft, at least a diameter plus
0.1 ft), as many abreast and along as the floor holds and as many tiers as the ceiling allows
(`grid_capacity`). For a 500 lb class bomb (a Mk 82, 1.27 ft span, 7.3 ft long) that is **4 abreast,
1 long (an 8.67 ft slab), 4 tiers a row**, so a row holds up to 16 by room and 9 by its record (the
caps stay 9 stores and class 70): nine Mk 82 hang as 4, 4 and 1 on three tiers (B52-5's own grid
was 3 x 3 at a 1.42 ft pitch). The ripple order is B2-5's: the bottom tier first, front to back,
the stores of a row abreast outermost first, alternating sides (B52-5's was the middle lane first);
it still releases bottom tier first, so nothing falls through another store. The row's mount is
unchanged: its slab middle plus 0.87 ft, the `MK82.SH` family's origin offset (it spans -4.52 to
+2.78 ft about the point a store hangs and leaves from), so the drawn 500 lb bomb is centred in its
slab; mounts 13.87, 5.20 and -3.46 ft forward, belly plane -6.0 ft. **Effect of the fold on counts:**
the stores' caps are unchanged (9 and class 70 a row, 27 Mk 82), but the tighter 1.3 ft pitch
(B52-5's span plus 0.15 ft was 1.42 ft) lets two more store types touch their neighbours' fins,
so the admitted bomb types fall from five to three (below).

**What the bay admits.** Capacity 9 and class 70 a row (21,000 lb); 27 stores of 500 lb class, or
fewer of heavier ones by weight. Retail masks `0x1100` (row 3) and `0x1120` (rows 5 and 6). The
bay sweep (re-run on the shared grid, 2026-11-15, with the refusal list emptied and then
rebuilt; `.local/m3-aircraft/checks/bomber-merge2/bay-sweep-unrefused.txt`, the three 3 ft span
missiles on rows 5 and 6 in `bay-sweep-missiles.txt`) loaded each admitted store full in all three
rows (the 3 ft span missiles in the two rows that admit them). Worst clearances in feet over the
27 stores, doors open (0.05 ft is the line):

| Store (shape) | Side wall | Door leaf | Aft end | Fore end | Roof | Above belly plane | Rails and posts | Nearest store |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `MK82`, `MK82P3` (`MK82.SH`) | 0.60 | 0.65 | 0.68 | 0.68 | 2.24 | 0.25 | 0.58 | 0.39 |
| `FAB250` (`F250.SH`) | 0.57 | 0.64 | 1.79 | 0.48 | 2.17 | 0.28 | 0.38 | 0.35 |

Refused by name (`aircraft_stations::B52_BAY_REFUSED`, **22** stores, was 20 on B52-5's grid):
`AGM45`, `AGM84A`, `AGM84E`, `AGM88`, `AM39`, `AS16`, `AT2`, `CBU87`, `CBU89`, `FAB1000`,
`FAB500`, `GBU29`, `GBU29P`, `GBU30`, `GBU38`, `MK20`, `MK82AIR`, `MK82HD`, `MK82P`, `MK84`,
`RBK250`, `RBK500`. Measured at full load they leave under 0.05 ft to a wall, the roof, the
floor, a rail or post or a neighbour, or pass through one. **Two joined at the fold:** `GBU38` and
`RBK250` (their nearest-store gap fell from 0.15 ft to 0.00 ft at the 1.3 ft pitch, and `GBU38`'s
and `RBK250`'s side walls fell from 0.34 and 0.82 to 0.21 and 0.37 ft, still clear; the fins of
neighbours now touch). Worst cases: `MK84` fore end -1.10 ft and nearest store 0.00; the GBU-27
class aft end -2.74 ft and side wall -1.65 ft; `AGM84E`, 12.6 ft, is a pylon store; the HARM side
wall -0.26 ft; `AGM84A` (side wall -0.01 ft), `AM39` (-0.75 ft) and `AS16` (rails 0.00 ft) are the
3 ft span missiles of rows 5 and 6. The open door leaves hang against the side walls (3.0 ft out),
so the leaf figure is read below the belly plane.
**The real 8 heavy stores (Mk 84) are not carried.** Per-shape centring (each shape's own origin
offset in `store_sizes`) would admit the Snakeye, the MK20 and the cluster bombs; it is not
built (open for John).

**Pylons (`Location::Beam`).** The pair is an authored row (`B52.PT` row 7, appended while the
record is retail) at 17.33 ft out and 10.33 ft forward; the wing underside reads 3.8 to 4.0 ft
there. A wing's stores hang from one flat beam, three abreast (pitch the store's span plus 0.1
ft, or its diameter plus 0.3 ft, whichever is more), rows 1.05 store lengths apart centred on
the pylon's point, all level, lugs 1.0 ft under the surface (`hardpoints::beam_offset`); the left
wing mirrors. The highest slot, the front row's last, leaves first. The drawn pylon is the beam
(0.5 ft deep, as wide and long as the rows) and a slab up to the underside (`pylons.rs`).
Default: 12 `AGM84E` (`SLAM.SH`, 14.6 ft), six a wing, two rows of three 15.3 ft apart.
Refused (`B52_PYLON_REFUSED`): `AGM45`, `AGM88`, `GBU29`, `GBU29P`, `GBU30` (13.7 to 13.9 ft
stores whose rows reach the flap strip and the trailing edge, 0.00 to 0.01 ft at capacity).

**Pylon clearances, feet** (`TORE_STORES_PREVIEW_B52CLEAR=1`; every store at its capacity; the
station is the pylons and stores as drawn; the wing figure is the stores' bodies only, the pylon
meets the wing by design; ground is the lowest point above the gear contact line, -9.33 ft;
positive is clear; `.local/m3-aircraft/checks/B52-5/pylon-sweep.txt`):

| Load | Inboard pod | Flap strip up / down 15 deg | Fuselage | Gear down | Wing | Ground | Nearest store |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12 `AGM84E` (default) | 8.27 | 0.32 / 0.36 | 4.58 | 7.44 | 0.32 | 10.59 | 0.73 |
| 18 `MK82` | 10.63 | 0.53 / 0.57 | 6.62 | 10.38 | 0.89 | 11.30 | 0.36 |
| 18 `GBU38` | 9.81 | 0.37 / 0.42 | 5.96 | 9.52 | 0.47 | 10.93 | 0.10 |
| 18 `FAB250` | 10.57 | 0.59 / 0.62 | 6.56 | 10.45 | 0.93 | 11.20 | 0.32 |
| 8 `MK84` | 9.87 | 0.60 / 0.63 | 5.97 | 10.05 | 0.85 | 10.73 | 0.54 |
| 16 `GBU30` (refused) | 4.42 | 0.00 / 0.00 | 1.52 | 7.22 | 0.00 | 8.91 | 0.10 |

The full table (22 stores) is in the sweep file. No admitted store touches the inboard pod
(nearest 8.3 ft, the default Harpoons), the fuselage, the gear or the ground; the nearest to the flap strip is the 0.32 ft
of the default Harpoons' upper fins under the wing's trailing region with the flaps up (the 15
degree droop opens it to 0.36 ft). The pod faces are taken as the airframe faces 28 to 42 ft out
and under 3 ft up.

**Release.** Bay: the press asks for the doors; the weapon leaves once `State::bay` is at 0.95
(tick 116, 0.958, in `flight-bay-fire-b52-row3`, `-row5` and `-row6`), the doors stay 1 s and close
1 s later. A pilot-opened bay releases at once. Pylon: from the beam with the doors shut
(`flight-bay-fire-b52-pylon-row7`, an 18 Mk 82 load). Live fire: `flight-livefire-b52-slot1` to
`-slot5` (the gun 1,242 rounds, the 12 Harpoons, three never-loaded bay rows). **AI:** the AI's
release does not wait for doors and its bay doors stay drawn shut (the F-22's and the F-117's
precedent; `an_ai_bomb_leaves_a_b52_bay_grid_inside_the_centred_opening`).

**Signature.** `State::bay_available()` is true for the B-52 (rows in `bays::STATIONS`), and the
open-bay term (1.5 times at full travel) applies by itself; it reaches the RCS page only (as for
the F-22 and the F-117). Stores in the bay add nothing to the external-store signature; pylon
stores do (3 per store and 2 per 1,000 lb).

**Default load and weight.** 12 `AGM84E` on the pylons (16,588 lb with fittings), no bay stores.
`Loadout::total_lbs` counts every store that is not internal, bay stores included, so the default
tank, capped to the maximum take-off weight (488,000 lb), starts at 286,402 lb (it was 303,000 lb
with the Harpoons in the bay, which BMB-A2 counted as outside). A bay load needs fuel taken out
in the editor.

**Tail gun.** The M61 mount is the drawn tail cone, 92.0 ft aft and 2.0 ft up. Fire direction is
the aircraft's nose: aircraft hardpoint slew headings and limits are not read, so the retail
rear-facing arcs (heading 32,760, 30 degrees) do not apply, before or after.

Provenance: the opening length, the belly plane, the tail cone and the wing height are measured
from `B52.SH`; the 6 ft width, the 27-bomb and 12-Harpoon loads are cited public figures
(aircraftinformation.info, Wikipedia); everything else is fitted (agent, 2026-11-15).

## Update (SWING-1, 2026-11-15): the swing wing is a per-aircraft spec

The Tornado's swing wing (TOR-3, TOR-5, TOR-12, TOR-14) was a set of module constants in
`combat::swing`. SWING-1 turns it into data, `SwingSpec`, so the B-1B, Tu-160, F-111F and Tu-26
can use the same code. **The Tornado's behaviour is unchanged**: every value moved into
`swing::TORNADO` as it was, the module constants (`SWEEP_MIN`, `SWEEP_FROM_KNOTS`, `PIVOT_R`,
`WING_IN_FT`, `WING_LEADING`, `PYLON_LENGTH_FT` and the rest) read it, and the Tornado's tests
(swing, hardpoints, pylons, `tornado_animation`, pod clearance, replay) pass with their
assertions as they were.

**The spec.** `swing::spec(id) -> Option<&'static SwingSpec>` resolves through
`AircraftId::source()` (so Tornado IDS (Clean) shares the Tornado's) and is `None` for every
aircraft without a swing wing. A `SwingSpec` holds:

| Field | Meaning |
|---|---|
| `sweep_min`, `sweep_max` | The real leading-edge sweep range, degrees. |
| `rest_sweep` | The sweep the shape is drawn at, the rig's zero turn. Equal to `sweep_min` except for the Tu-26 (mesh at 18.4, real 20). |
| `feet_per_unit`, `axis_scale` | The header scale and a per-axis size fit (right, forward, up), `[1, 1, 1]` unless the aircraft's size is fitted per axis (F-111F). |
| `pivot_right`, `pivot_left` | The wing pivots, shape units (right, forward, up), vertical axis. |
| `law` | `SweepLaw::Linear { from_knots, to_knots }` or `SweepLaw::Detents(&[(knots, degrees)])`, smoothstep between points. |
| `gear_holds_spread` | Whether the gear, as well as the flaps, holds the wing spread (Tu-26). |
| `sweep_rate_dps` | The real actuator rate when a source gives one. None does; the drawn sweep is a pure function of speed and nothing moves at a rate. `None` throughout. |
| `placement` | The Tornado's in, up, forward and root-shift values (TOR-12); `Placement::NONE` for the others. |
| `swivel_rows`, `fixed_store_rows`, `max_sweep_with_fixed_stores` | Which source hardpoints swivel with the wing; which wing rows are fixed; the sweep limit while a fixed row carries a store. |
| `outline`, `pylon_length_ft` | The placed panel's edges (for `mid_chord`) and the fixed pylon length, where the aircraft has swivelling pylons. |

**The turn.** `SwingSpec::turn(speed_fps, flaps, gear)` is the wing's turn in radians from the
drawn pose: `min_turn + share * (sweep_max - sweep_min) * (1 - hold)`, where `share` is the
schedule's (0 spread, 1 fully swept), `hold` is the flaps (and the gear where
`gear_holds_spread`) and `min_turn = (sweep_min - rest_sweep)` is zero except for the Tu-26.
For the Tornado it is the arithmetic `wing_turn` always had, bit for bit.
`SwingSpec::turn_carrying` also applies `max_sweep_with_fixed_stores`. `speed_for_sweep`,
`pivot_feet`, `swept`, `unswept`, `swivels`, `has_swivel`, `pylon_length` and `mid_chord` are
methods too.

**What the callers use.** `swing::wing_turn_of(id, speed_fps, flaps, gear)` (0 for an aircraft
with no spec) and `Configuration::set_wing_flight(speed_fps, flaps, gear)` /
`Ownship::set_wing_flight`, which the host (`tore-world` combat) and the AI
(`AiWings::carry_swing_wings`) call every tick instead of computing the Tornado's turn;
`pylons::swung` resolves the id; `hardpoints::station_move` reads the placement (`in`,
`forward`, `up`) from the spec of the aircraft whose row swivels; `replay/convert.rs`'s sweep
test reads the schedule and the full turn from `swing::spec(AircraftId::Tornado)`. The
Tornado-flavoured free functions (`wing_turn(speed, flaps)`, `speed_for_sweep`, `pivot_feet`,
`swept`, `unswept`, `mid_chord`) stay as the Tornado's, for the Tornado's drawing rig and tests.

**The four new specs** (constants `B1B`, `TU160`, `F111F`, `TU26`; no `AircraftId` exists for
them yet, so `spec(id)` does not return them; `swing::ALL` lists all five). Schedules are
fitted or opinionated (the real wings are selected by the crew); ranges are cited.

| Spec | Sweep (rest) | Pivot (shape units, scale) | Schedule | Holds spread | Swivel |
|---|---|---|---|---|---|
| `TORNADO` | 25 to 67 (25) | (6.5, -1.351, 0.465), 2/3 ft | Linear 304 to 600 kt | Flaps | Rows 7 to 10 |
| `B1B` | 15 to 67.5 (15) | (10, 11), 4/3 ft: 13.33 ft out, 14.67 forward | Linear 280 to 520 kt | Flaps | None |
| `TU160` | 20 to 65 (20) | (4.8, 0), 8/3 ft: 12.8 ft out | Detents 20 to 250 kt, 35 from 320 to 520, 65 from 600, smoothstep between | Flaps | None |
| `F111F` | 16 to 72.5 (16) | (13, 2.5), 2/3 ft, x scaled 0.859: 7.4 ft out, 1.67 forward | Linear 250 to 650 kt | Flaps | Rows 5 and 6; fixed rows 7 and 8 hold the sweep to 26 degrees or less while they carry stores |
| `TU26` | 20 to 65 (18.4) | (14, 3), 4/3 ft: 18.67 ft out, 4 ft forward | Linear 300 to 560 kt | Flaps and gear | None |

The `F111F` spec also carries the panel outline (leading edge shape (13, 10) to (53, 0),
trailing edge at y -5) and the Tornado's 3.3 ft pylon length. No placement offsets: whether
the F-111F's 4.7 ft root seam gets a root shift or a glove plate is its rig slice's decision.

**Wiring an aircraft.** The identity slice adds the `AircraftId` and one arm to `swing::spec`
(the four arms are written there, commented out). The rig slice then draws the panels with
`spec.turn(state.speed, state.flaps, state.gear)` and `spec.pivot_feet`; a swivelling aircraft
(F-111F) also needs its rows' `station_move` entries centred on `spec.mid_chord`, and
`Rack::position` (which carries a swivelling station's point with `swing::swept`, the
Tornado's pivot) to take the aircraft's pivot. That is the F-111F slice's change; SWING-1
leaves `Rack` and its checkpoint layout alone.

Tests: `swing.rs` (`the_tornados_spec_is_the_modules_constants_and_resolves_for_both_versions`,
`every_spec_is_consistent_and_its_schedule_runs_from_spread_to_swept`, and one schedule test
each for the B-1B, Tu-160, Tu-26 and F-111F).
