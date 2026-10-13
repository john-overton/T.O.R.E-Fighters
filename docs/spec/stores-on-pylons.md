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

## What the player sees

- **Every store leaves from its own pylon.** A missile, bomb or rocket appears
  at the station that carried it, in the aircraft's frame: wing stations out
  on the wing, fuselage stations under the body, centreline stores on the
  centreline, wingtip missiles at the tip, internal bay stores below the bay.
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
| Station position | The source hardpoint record's x / y / z, a third of a foot a unit, right / up / forward of the aircraft's reference point ([table](../formats/aircraft-ordnance.md#hardpoint-positions)) | spec-derived |
| Paired record | Hardpoint flag 0x4: the record's position is the right pylon, its mirror the left | spec-derived ([release state](../formats/weapons.md#release-state)) |
| Also paired | Every Wing and Wingtip record without flag 0x4 (the A-4E's inner Mk 82 row, the Mi-24's AT-2 rails) | agent decision, 2026-10-12 |
| Never paired | An internal gun, whatever its flags; a record on the centreline (nothing to mirror) | agent decision |
| Which side | The mirror (left) when the whole releases left on the station after this one (rounds over `actualRoundsPerGame`) is odd; so the left side empties first and the right holds the odd store | retail parity rule; whether retail counts before or after the debit is unknown, the order is fitted |
| Unlimited ammunition cheat | The count never moves, so the side follows the release's projectile number (alternates) | agent decision |
| Who | The player, the AI (strike stores, missiles and seeker missiles alike) and every network seat. Retail releases a distant AI aircraft from its centre; T.O.R.E releases every aircraft from its pylon | agent decision |
| Guns | Keep their muzzle (gun mount, AC-130 barrel) | unchanged |
| Bomb packs | Each bomb of a pack keeps its pack stagger, added to the pylon's side | unchanged |

**The bomb sight aims from the pylon the next store leaves** (John,
2026-10-12, slice BP3). The CCIP pipper, the level T and the rocket pipper
predict the release from the side of the selected station the next store
will leave, by the same rule the release uses: the whole releases left on
the station once that store has gone, odd for the mirror (left), even for the
record's own position (right). So on a pair of wing Mk 82s the pipper first
shows the left bomb's fall, and after that bomb goes it moves to the right
bomb's. With unlimited ammunition the release's projectile number picks the
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

Retail draws no stores on aircraft (the A2G pass recorded this as decision 20,
"stores on pylons stay undrawn"); John reversed that decision on 2026-10-12.
Every number below is fitted (agent decision).

| Item | Rule |
| --- | --- |
| Stations drawn | External ones (Centerline, Fuselage, Wing, Wingtip) that held something at mission start; not guns or internal bays |
| Store look | The weapon's own shape (the same mesh it flies with after release). Rocket pods: a grey-green eight-sided pod 8.5 ft by 1.4 ft. Fuel tanks: a light grey pod 13 ft by 2 ft. A store with no shape: a plain body 8 ft by 0.9 ft |
| Stores on one side | One at the pylon point. Two side by side, 0.55 diameters either side. Three as a triple ejector rack: two shoulders and one 0.95 diameters below. More: such racks one behind another, 1.05 store lengths apart. The layout keeps the loaded count's slots; the highest slot empties first |
| Pylon | A grey slab 0.35 ft thick and 0.55 store lengths long (1.5 to 8 ft), from the top of the store up to the model's underside straight above the hardpoint, or up to 3 ft inboard of it (found from the clean model, up to 12 ft). With nothing above (a wingtip rail beside the wing) a 0.35 ft rail. A rack of several stores is as wide as the rack, at most 1.4 ft |
| Colours | Pylons and racks grey (0.5, 0.52, 0.54); weapon shapes keep their palette colours |

Where the stores come from:

- **Single player, the host, AI aircraft:** the render snapshot carries each
  aircraft's stations and their stores left ([`AircraftPose::stores`]).
- **Network clients (protocol 26, slice BP3, John 2026-10-12):** a client
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
  nothing while nothing changes ([the protocol](../formats/net-protocol.md#stores-protocol-26)).
- **Mission replays (slice BP3, John 2026-10-12):** a recording keeps each
  aircraft's drawn stations the first time it is recorded and their whole
  stores whenever one empties, as `stores.*` events; the viewer draws each
  aircraft's stores at the tick it shows, forward, backward and after a
  seek. A converted network capture and an observer's live recording keep
  them too. A recording made before them has none, so its aircraft carry no
  stores ([replays](../REPLAYS.md#stores)).

## Performance

The stores join their aircraft's existing vertex batch (no extra draw call or
pipeline). A pylon is 48 vertices, a fitted pod or tank 144, a weapon shape its
own triangles; the pylon heights are worked out once per aircraft type. On a
16-aircraft formation each carrying six stations this is about 10,000 extra
vertices a frame.

## Source notes

- Hardpoint record layout and location names: [aircraft ordnance](../formats/aircraft-ordnance.md).
- Retail release position and the 0x4 mirror rule: [weapons formats](../formats/weapons.md#release-state)
  (FA.EXE 1.02F `HARDPos` `0x4532a0`, `HARDUnrotatedHardPos` `0x453220`).
- Code: `tore_sim::combat::hardpoints` (the rule), `combat::live::cockpit`
  (`release_round`), `tore_world::ai_wings` (AI missiles),
  `tore_world::snapshot::store_poses`, `tore-app/src/pylons.rs` (drawing).
- Scenarios: `a2g-release-origin`, `net-stores` (a client sees another
  player's and an AI's stores empty) and `replay-stores` in the battery.
