# Jettisoned tanks and stores

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation specification. Opinionated request by John, 2026-10-15.

## Player behavior

Shift+K drops every installed external fuel tank, including wing, centreline
and any conformal tank represented by an installed GAS row. Remaining external
fuel leaves with the tanks. Internal fuel and weapon ammunition do not change.
Ctrl+Shift+K drops all physical stores on the selected external weapon station.
Internal weapons stay aboard. View + right stick press keeps selected-store
jettison outside the AC-130 console, where John's gun-lock binding still applies.
No tank gamepad gesture is assigned. Custom key bindings remain custom.

The stores disappear from their pylons immediately. Their shell mass, remaining
fuel and payload drag contribution leave the aircraft at the same tick. Fuel
already burned is not subtracted twice. The stores page follows remaining ammo.

Every dropped body starts at its drawn station with the aircraft's velocity
and a **fitted, agent-chosen 6 ft/s separation push** opposite the aircraft's up
axis. It follows the shared unguided `BombMotion`, currently gravity 32 ft/s²
and no drag. Its nose follows its velocity. It never arms, seeks, ignites a
motor, tests aircraft or ground-unit collision, inflicts damage or awards kills.
The bomb-physics slice owns later changes to the shared fall model.
Inert bodies share the existing 5,000-projectile limit. If that pool is full,
the loadout still empties but excess falling bodies are omitted. Keeping this
bound is a fitted agent decision.

Terrain contact uses the swept fall segment. Land gets the existing inert dirt
puff (type 15, scale 0) and a 2.5 ft wide pock-mark crater (size 0, mark scale 25) that lasts for the mission.
Water gets the small splash (type 17, scale 0), with no crater. These are fitted
agent choices. No blast flash, fire, shockwave or collateral damage is produced.
Tank and shapeless-store bodies reuse fitted pylon geometry and retain their
dimensions in snapshots and recordings.

## Evidence and sound

The local retail manual, `manual.txt` lines 3931 to 3943, says weapons and tanks
increase weight and drag, and identifies Shift+K for all air-to-ground ordnance
and Shift+J for external tanks. John's requested keys intentionally differ.
The manual does not specify separation speed, tumbling or impact effects.

[Sound research](../formats/sound.md#weapon-launch-and-bomb-release-sound)
identifies FA.EXE 1.02F store-jettison calls at 0x414ec4, 0x414f8a and 0x451ffc:
`&BOMB.11K`, level 255, full within 100 ft and silent at 7,000 ft. One release
emission plays per nonempty command. The sound never creates an explosion visual.

Land contact uses the existing retail small-ground-impact choices
`&BULLTS1.5K` and `&BULLTS4.5K`; water uses `&SPLASH3.11K`. Selecting these for
inert stores is fitted, not evidence of retail jettison impact behavior. A
specific retail dud/tank thud remains unknown. Next research: inspect the
retail dropped-store terrain-contact consumer, rather than infer from names.

## Replay, network and AI

Dropped objects, release and contact sounds, attached-store changes and craters
appear in recordings and on network clients. A checkpoint preserves their fall
and inert status. Older mission recordings retain their existing visual history
and remain readable.

Peers must agree on the [network conventions](../formats/net-protocol.md#inert-jettison-integration)
and [checkpoint layout](../formats/checkpoint.md#inert-stores). The lead assigns
the protocol bump.

AI jettison is opinionated behavior requested by John on 2026-10-15. The
thresholds below are agent decisions, not recovered retail numbers. AI wingmen
of humans and autonomous wings use the same rules on the authoritative host.

- Perceived missile defense or a defensive break drops all external tanks,
  including empty tanks. A missile defense means a maneuver or countermeasure
  response, not tracking a threat while waiting. A strike pilot in missile
  defense or an actual defensive break also drops the external air-to-ground load.
- A non-strike pilot pursuing, acquiring or attacking an aircraft within
  3 nautical miles drops tanks and external air-to-ground stores.
- A perceived attack on the aircraft itself drops that load when the observed
  attacker is within 3 nautical miles. Strike pilots instead use 4, 3, 2 and
  1 nautical miles for Novice, Average, Experienced and Ace. A nearby contact
  alone never makes a strike pilot abandon the load.
- Bombs, rocket pods, selectable designator pods and air-to-ground missiles are discarded. Guns and
  air-to-air missiles remain, as do internal weapons. No cruise, ground,
  ordinary strike or bingo-return trigger is added.

One nautical mile is 6076.11549 feet. Distances use perceived aircraft contacts,
not hidden enemy positions. No random draw is added. The decision uses the
current mission tick; the bridge applies it after that tick's ordinary weapon
releases, before the next flight step. An already released weapon stays armed.
Removing stores empties their AI inventory and configuration, so a sustained
trigger cannot repeat a drop or sound. No new checkpoint latch is needed.
Tank fuel and the mass/drag load update through the existing fuel and flight
payload interfaces. A dropped load can improve maneuvering and shorten fuel
endurance. The existing inert projectile path supplies recordings and clients.
Human-owned aircraft use the same combat path regardless of seat, including
an aircraft taken over from AI.
