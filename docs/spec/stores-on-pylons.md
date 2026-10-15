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
| Which side | The mirror (left) when the whole releases left on the station after this one (rounds over `actualRoundsPerGame`) is odd; so the left side empties first and the right holds the odd store | retail parity rule; whether retail counts before or after the debit is unknown, the order is fitted |
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
| Pylon | A grey slab 0.28 ft thick, from the top of the store's body (its lugs, not its fin tips) or a rack's bar up into the model's underside (1 ft, see [pylon depth](#pylon-depth)), 0.02 ft into it (retail wings are single faces, so more would show on top). Its length is 0.45 store lengths, at least twice its depth and at most 0.7 store lengths (1.2 to 8 ft), cut short where the underside ends (a pylon does not run out ahead of a leading edge), and its top follows the underside from end to end. The underside is the lowest face of the clean model up to 12 ft above the lugs, straight above or up to 3 ft inboard |
| Rack | A bar 0.25 ft deep across the pylon's foot, reaching out to the centre lines of the stores beside it, 0.7 of the pylon's length (plus the racks one behind another) |
| Fin roll | Every weapon shape is rolled about its body's centre line so its fins stand in an X, 45 degrees from straight out and straight up and down, so no fin points into the pylon, the wing or body above, or the store beside it (John 2026-10-13: "ordnance needs to be rotated 45 degrees to make sure fins don't clip and are perpendicular to pylons and the fuselage as best as possible"). The roll is measured per retail shape, not assumed: the fin tips' clock about the body (the corners more than 1.6 body half widths out, their angles averaged over a quarter turn). A "+" shape (clock 0) turns 45 degrees; a shape already in an X (clock about 45) is not turned; a shape whose tips agree on no clock (agreement under 0.5 of 1) is not turned. One roll serves every mount (under a pylon, beside a bar, on a wingtip, on a rail), since an X is the same seen from below or from the side. The rolled shape is the one drawn everywhere the store is: on the pylon, in flight after release (so it does not turn at release), for the player, AI aircraft, network peers, replays and previews. See [fin roll by shape](#fin-roll-by-shape). Opinionated, John 2026-10-13; the measuring rule and one roll per shape are agent decisions |
| Wingtip | Nothing drawn under a wingtip store ([rules](#rules)) |
| Shoulder rail | The pylon the store rides beside, from its foot into the surface above (drawn again for the shoulder store, so it stands when the station under it was left empty), and an arm 0.15 ft deep jutting off the pylon's side, above the main store, out along the shoulder store's top to its centre line, 0.6 store lengths long (3 to 6 ft), drawn as pylon structure |
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
