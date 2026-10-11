# AI strike behaviour

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Specification, 2026-10-11, for the AI strike round (slice ST-S). Status:
**specified, partly built**: the ground picture ([What the AI
knows](#what-the-ai-knows), and the threat inputs of [Perceived threat and
available speed](#perceived-threat-and-available-speed)) is built (ST-P); no
AI attacks the ground yet. John asked for AI that attacks the ground and
authorised the work for this round (answers of 2026-10-11, folded in below). The
slices that build it are listed in [Source notes](#source-notes).

For a player: AI aircraft learn to bomb. A strike wing flies to its target,
picks a pass, drops, gets out, comes back until the target is dead or the
stores are gone, then goes home. Your wingmen strike on your orders. A pod
carrier can light the target for you or for a laser bomb carrier. If you give
your plane back mid-attack, the AI can finish the attack with your settings.
Fighters, attack jets, heavy bombers, helicopters and the AC-130 all do it,
each in a way that suits it.

Today the AI carries bombs and Mavericks and never uses them ([AI
behaviour](ai.md#b14-surface-attack-and-egress) records what the retail
scripts say and what is open). This spec adds the missing behaviour on top of
the player's own weapon systems in [air-to-ground](air-to-ground.md), which the
AI now shares.

## How to read the labels

Every rule below carries one label, as [AGENTS.md](../../AGENTS.md) defines
them.

| Label | Meaning |
| --- | --- |
| **retail** | Established from the retail scripts, records or executable ([AI behaviour](ai.md), [AI formats](../formats/ai.md)) |
| **spec-derived** | Taken from another spec in this directory |
| **fitted** | Chosen by an agent because the original's value is not known; the rule and constants are written down |
| **opinionated (John, date)** | A design John asked for or approved |
| **opinionated (agent)** | A design choice an agent made, never attributed to John |
| **unknown** | Insufficient evidence; see [Unknowns](#unknowns) |

## What retail does, and what changes

Retail FA's AI does strike, simply. A script picks one of four passes by dice
(straight run, pop-up, fast-high, dive bomb), the weapon service releases
whenever its check passes, and each AI bomb is re-aimed to land within 100 ft
of its target ([B14](ai.md#b14-surface-attack-and-egress), [B42](ai.md),
[bomb notes](../formats/weapons.md)). It has no buddy lasing and no AI lasing for
players. Its Quick Mission already sends friendly bomber-class wings at ground
targets ([Who strikes](#who-strikes)).

John's answers change the shape of it. All of these are opinionated (John,
2026-10-11):

| Retail | This spec |
| --- | --- |
| AI bombs are moved to land within 100 ft of the target | **No re-aim.** The AI flies the pass through control inputs and releases from the same delivery and guided solutions a player uses. Skill sets how closely it releases ([Skill](#skill-and-release-precision)) |
| The pass is a random choice | **No dice.** The pass is a fixed function of perceived threat and available speed, limited by bombing class and weapon ([Pass choice](#pass-choice)) |
| A heavy bomber flies a high pass at 10,000 ft or more | Heavies bomb level from a minimum of 5,000 ft AGL, or the mission's airborne height if higher, raised by perceived threat ([Heavies](#heavy-bombers)) |
| No buddy lasing | The AI lases for its wing and for you, and drops on your spot ([Lasing](#lasing)) |
| Orders reach air targets only | Wingmen strike on the flight lead's orders, ground targets included ([Orders](#orders-from-the-flight-lead)) |

Randomness is kept as low as it can be (John, decision 13). Every choice below
is a function of the situation. The only dice left in a strike are the ones the
player's weapons already have: a bomb's up to 3 degree scatter in heading and
pitch ([bomb motion](air-to-ground.md#bomb-motion), retail), and the existing
seeded draws for hit rolls and countermeasures.

## Who strikes

| Rule | Label |
| --- | --- |
| Every AI family can strike: fighters, attack jets, heavies, helicopters and the AC-130 | opinionated (John, 2026-10-11) |
| The behaviour is side neutral. Any side's wing with the Strike role strikes | opinionated (John, 2026-10-11) |
| Quick Mission gives each wing a role, **CAP** (today's behaviour, the default) or **Strike** (offered only when a ground target is set). A Strike wing flies to the objective, strikes, goes home. Its loadout is the loadout page; a Strike wing with no surface stores is told so | opinionated (John, 2026-10-11) |
| **Retail's rule, recorded beside it** ([ST-R](../formats/ai.md#strike-maneuvers-steering-frame-and-orbit)). Retail Quick Mission has no role setting. When a ground target is set, every *friendly* bomber-class wing (A-4E, A-6, F-117, B-52, AC-130U, AH-64, Mi-24, Su-24, Tornado) is given ground classes (ship, tank, vehicle, structure) to attack and defend and fighters to evade, and attacks what it sees within about 26,400 ft. Fighter-class wings (A-10, A-7, AV-8B, Su-25 included) stay air only. No wing is ever given SAMs or AAA, so retail has no SEAD. Enemy wings never strike. How the wings reach the area is open. T.O.R.E's per-wing CAP or Strike role is a departure from this, and the default is CAP for every wing; see [Confirm with John](#confirm-with-john) item 9 | retail (rule), opinionated (John, 2026-10-11) for the departure |
| Redfor can strike but Quick Mission gives Redfor no air-to-ground objectives, so no Redfor wing is tasked in Quick Mission. Imported missions can task any side later | opinionated (John, 2026-10-11) |
| PvP: a Blue Strike wing goes after Redfor's protected objects ([objectives](surface-defenses.md#objectives-scoring-and-debrief)); Redfor wings stay CAP and defend | opinionated (agent) |
| The player's own wingmen follow the player and strike on orders only. They never wander off to bomb on their own, even on loose control | opinionated (John, 2026-10-11) |
| An AI lead that inherits a wing whose human was striking continues that target, then the objective, before any air search (extends [the lost human leader rule](ai.md)) | opinionated (agent) |

Reactions while striking use the Mission Creator's vocabulary (retail manual
p. 169): ground targets **attack**, aircraft **defend**, SAMs and missiles
**evade**.

| Reaction | Rule | Label |
| --- | --- | --- |
| Attack | The striker works its target until it dies or no usable store is left, then the next target, then home | retail (waypoint completion, [B48](ai.md#b48-routes-fuel-and-recovery)) and manual |
| Defend | It engages an aircraft only if that aircraft has fired at the wing or locked it, or is inside 5 nm and facing it (the [B02](ai.md) test). Otherwise it presses on | fitted |
| Evade | Existing [B47](ai.md#b47-threat-warnings-countermeasures-and-reason-priority) reactions and countermeasures, unchanged. With a ground target retail evades by climbing on the same course to at least 20,000 ft | retail |
| Out of surface stores | A wing with missiles left and loose control may take up free air engagement | opinionated (John, 2026-10-11) |

## One cockpit, two pilots

The ground-attack state a human uses is one record per aircraft, the **strike
cockpit**: the target (a ground object, a sight pin point, or none), the
selected station and armed state, the delivery mode (CCIP or LVL), the seeker
mode (CUED or BORE), the sight (track, pin, look, zoom) and the laser with its
hand setting. It holds what the kept cockpit of a given-back plane already holds, made the
single home of the state.

- A human edits it with keys, as today.
- The AI edits it from its strike controller (target, station, laser on or
  off) and leaves everything else alone.
- One release function serves every launcher, human or AI: packs, pod rounds,
  scatter, guided flights, laser spots. AI rows no longer build a plain
  projectile.
- Guided flights find their launcher by aircraft, not by "the player", so an AI
  laser bomb, JDAM or AT-2 guides exactly as a player's does.

All opinionated (John, 2026-10-11: "one cockpit, two pilots" and
[takeover](#takeover-and-handback)); the record layout is an agent design.

## What the AI knows

| Rule | Label |
| --- | --- |
| Briefed targets (the objective's template objects, flagged for Redfor in Quick Mission) are known from the start. This is the briefing, not a sensor | fitted (retail missions name a target object) |
| Anything else must be seen. A strike actor builds the same ground lists as a player (A-G radar, FLIR, eye) at the retail 6 s rebuild, from its own aircraft | spec-derived ([air-to-ground](air-to-ground.md#ground-contacts-and-channels)), rebuild cadence retail |
| Air-to-air actors are unchanged: surface objects enter only a Strike actor's picture, so CAP wings behave as before | opinionated (agent) |
| A wing tasked as SEAD (a Wild Weasel assignment, retail manual p. 169) has the objective's air defences in its briefed set | retail manual, opinionated (agent) for the set |
| Target order: nearest first, at most two attackers on one object (the [B41](ai.md#b41-target-retention-eligibility-and-ranking) allowance) | spec-derived |
| After the briefed targets are gone, a striker with stores left attacks hostile ground objects it has seen within 10 nm of the objective, then goes home | fitted |
| The briefing is Quick Mission's destroy targets (flagged 0x80) of another side than the striker's, with their centre as the objective point. Redfor's are its own, so a Redfor striker is briefed on nothing (the capability is the same on both sides) | opinionated (John, 2026-10-11) for the sides; centre opinionated (agent) |
| A briefed or ordered object's place and whether it stands are known (retail's preferred-target test reads them directly). Its sensors decide only whether it is *perceived* | fitted |
| A SEAD striker is one carrying an anti-radiation store; its briefing adds the hostile armed units and radars within 5 nm of the objective point (the distance Quick Mission's Red defends within) | opinionated (agent), radius fitted |
| An ordered ground target (a flight lead's order, a taken-over designation) is known like a briefed one and outranks the briefing; it gives even a CAP wingman a ground picture while it stands | retail (the ordered object is the only candidate) |
| The A-G radar list (radar on and working) and the FLIR list follow the player's page rules (class, listing flag, visibility distance, range box, volume, signature, weather, clear line). The AI reads both, and its eye, at once, without switching its scope | spec-derived; reading all at once opinionated (agent) |
| The eye sees a standing, listable object on the ground inside the visual sensor's volume and range, with a clear line, and no farther than its own visibility distance (`maxVisDist`) when the type gives one; at most 50, nearest first | fitted |
| Between rebuilds, listed contacts follow their objects every quarter second, as the player's pages do | spec-derived |
| Neutral scenery never enters the picture (it would fill the 50-entry lists ahead of sided objects) | opinionated (agent) |
| A perceived object stays known at its last perceived place until it is destroyed or leaves the world | fitted |
| Every briefed target fully attacked (two others on each): the striker waits for one to free up, rather than taking a third place or an opportunity target | opinionated (agent) |

Built (ST-P): the picture is `tore_sim::ai::awareness::ground::GroundPicture`,
one per striker (`AiActor::ground_picture`), refreshed by
`AiMission::observe_ground`; the selector is
`tore_sim::ai::targeting::select_strike_target`. The bridge
(`tore_world::ai_wings::ground`) briefs strikers and describes the ground
objects. The role is `engagement::StrikeRole`, CAP by default; the Quick Mission
setting that chooses Strike comes with the orders and roles slice.

## Perceived threat and available speed

The two inputs to every tactical choice. They are read from what the AI
actually has: its sensors, its RWR and its warnings. An object it has not seen
does not count. Both are opinionated (John, 2026-10-11, "base tactics around
perceived threat and available speed"); the levels and numbers are agent
choices (opinionated, agent) unless marked.

**Perceived threat** is the highest level that applies. A defence "covers" a
point when that point lies within the defence's own maximum engagement range
(its weapon record) of it.

| Level | Name | Applies when |
| --- | --- | --- |
| 0 | Clear | None of the below |
| 1 | Air | A hostile fighter is in the picture within 30 nm, or an air-to-air warning sounded in the last 10 s |
| 2 | Guns | A known AAA site covers the target point or the planned release point |
| 3 | Missiles | A known SAM site with a working radar or seeker covers the target point or the planned release point, or a surface-to-air missile warning sounded in the last 10 s |

**Available speed (energy)** is **Good** when the aircraft's speed is at least
its corner speed at the current altitude, loaded (the corner query of
[B05](ai.md)), and **Low** otherwise. The **dive gate** is the retail dive
condition with the dice removed: altitude AGL and horizontal distance to the
target both at least max(10,000 ft, twice the turn radius) ([B14](ai.md#b14-surface-attack-and-egress),
retail).

**Retail's threat geometry**, the only one it has ([ST-R](../formats/ai.md#strike-maneuvers-steering-frame-and-orbit); it reads no threat rings or weapon ranges): a ship threat is ignored beyond 40,000 ft and any other beyond 30 statute miles; between 20,000 ft and 30 miles it is ignored unless the aircraft is within 100 degrees of the threat's nose; inside 20,000 ft it is always evaded (if its class is in the evade set). Speed enters retail's pass choice only through turn radius. These bands (20,000 ft, 30 miles, 100 degrees) are the inputs the levels above are built from, not the levels themselves: the levels stay opinionated (agent), and the Air level's 30 nm is close to retail's 30 statute miles (26 nm).

The picture is rebuilt at the 6 s cadence. Inputs are read when a pass is
chosen and again before each re-attack, never in the middle of a pass, so the
choice cannot flicker.

How the inputs are built (ST-P, `awareness::ground::ThreatPicture`):

| Input | Rule | Label |
| --- | --- | --- |
| Known defences | Every briefed defence that stands and every perceived armed unit or radar, of any side, with its missile reach (the farthest launch zone of its missiles), its gun reach (the farthest launch zone of its guns), whether it is a radar and whether it can guide (its battery's radar stands, or it guides by itself) | spec-derived; reach from the launch zones fitted |
| Covers | A point lies within the reach as a straight-line distance from the site; a missile site covers only while it can guide | spec-derived |
| Fighters | The hostile fighters of the striker's current air picture within 30 nm, nearest first, with the angle off each one's nose | spec-derived |
| Retail geometry | Each known defence and fighter answers retail's ignore-or-evade test; the angle off the nose is measured in the horizontal | retail bands; horizontal angle fitted |
| Warnings | The caller supplies whether an air-to-air or surface-to-air warning sounded in the last 10 s | spec-derived |

## Pass choice

The pass is chosen once per attack run, 10 nm from the target and again before
each re-attack. The four retail passes keep their retail shapes
([B14](ai.md#b14-surface-attack-and-egress), retail); only the choice is new.

| Pass | Shape | Label |
| --- | --- | --- |
| Straight run | Fly at the target until 2,000 ft horizontal, then exit | retail |
| Pop-up | Approach 500 ft above the target until within 10,000 ft, pull up 25 degrees, then point at the target until 1,500 ft or it is no longer ahead. Aim is the target's own position (0 to 359 ft right of it in retail, see below) | retail |
| Fast-high | Approach 5,000 ft above, straight on at maximum speed after 2,000 ft | retail |
| Dive bomb | Entry only at the dive gate. Approach at a point H = max(2R, AGL at entry) above the target at corner speed until horizontal distance is under 1.5R. Roll-in: hold the bearing to the target at that moment, level, bank 180 (inverted), corner speed, until under R. Pull to 90 degrees nose down (flown at the aircraft's `maxDive`, -80 on every fighter and attack jet) and hold until AGL is under max(2R, 2,000 ft). Roll out on the roll-in heading plus 180 at corner speed, a split-S pull-through to level. Then climb 25 degrees for 10 s, then the exit run. R is the turn radius, V squared over 43.6 G ft | retail ([ST-R](../formats/ai.md#strike-maneuvers-steering-frame-and-orbit)) |
| Level run | Straight and level at or above the release floor, with the LVL cue, for bombs dropped from altitude | opinionated (agent); the heavies' retail high pass is the model |
| Orbit | Left-hand pylon turn around the target | retail for the family, numbers [fitted](#ac-130) |

**The choice** for fighters and attack jets with unguided bombs, first row that
applies:

| Order | Condition | Pass |
| --- | --- | --- |
| 1 | Energy Low and threat Guns or Missiles | **Regain**: do not enter; fly the exit leg (climb 10 degrees at maximum speed) until energy is Good, then choose again |
| 2 | Threat Missiles | Pop-up (a low approach under the radar, a short climb, release, down) |
| 3 | The dive gate holds | Dive bomb |
| 4 | Otherwise | Straight run |

**Other weapons and a kept delivery mode**, applied before the table:

| Case | Pass |
| --- | --- |
| Seeker missiles and HARM (stand-off) | Fast-high when energy is Good, else straight run; the approach ends at the store's release range. Retail's guided release rule is the model: a guided round only inside 10,000 ft or half the weapon's maximum range, one guided round in flight at a time (retail, [weapons](../formats/weapons.md#ai-release-against-ground-targets)) |
| Rockets | Straight run with a shallow dive; pop-up under threat Missiles |
| Guns (strafing) | Straight run; attack jets strafe after their bombs and rockets are gone |
| Laser bombs, JDAMs | Level run (or a shallow run) to the weapon's reach |
| Taken over with LVL kept | Level run whatever the table says |
| Taken over with CCIP kept | The table's pass, flown as CCIP |

**Retail quirks the pass copies and does not copy.** Retail's pop-up pull (`homeAngle 0 25 0 corner 4`, helicopter 30 degrees for 6 s) is written with the speed in an ignored slot, so it requests *minimum* speed during the pull and bleeds speed for 4 s (6 s). The final pop-up leg aims 0 to 359 ft right of the target because a bearing was used as feet. **Recommendation, opinionated (agent): do not copy either.** The pull holds corner speed, or the speed the pass choice needs, and the final leg aims at the target, because John asked that tactics follow perceived threat and available speed (decision 3), and a pass that throws away its own speed under threat Missiles contradicts that. The pass shape (offsets, angles, durations) is retail's. Say so to John if he would rather have the retail bleed (item 10).

**The steering frame** (retail, same source): a pass offset is added to the target's position (in feet, x to the target's right, z ahead, rotated by the *target's* heading; y straight up from the target's height) and the aircraft flies straight at that point. A "low approach" is therefore a straight line to the point, level only because pitch requests within 3 degrees snap to level while the point is 3,000 ft or more away. Pitch is clamped to the aircraft's climb and dive limits. The AI takes this frame for the offset passes; its pilot flies the line through control inputs ([Skill](#skill-and-release-precision)).

Release floor, a minimum altitude AGL for dive and level releases, taken from
the retail manual's advice (3,000 ft lightly defended, 5,000 ft or more
defended; advice, not enforced by retail) and made a rule here: **3,000 ft**
under threat Clear or Air, **5,000 ft** under Guns or Missiles. Opinionated
(agent). Pop-up releases are below it by design and are the Missiles answer.

The bombing class limits the choice (next section). Every pass is also
available to the headless driver by name, so a test can force each one; that
changes nothing in play.

## Bombing class

Each aircraft's class comes from its imported record, so a new aircraft needs
no code (John, 2026-10-11: "maybe we go by weight so we don't have to keep
programming it in"; thresholds accepted by John, decision 19). The test runs in
this order. Fields are in [the aircraft table](../formats/fa-aircraft.csv).

| Order | Class | Test on the record | Passes allowed |
| --- | --- | --- | --- |
| 1 | Helicopter | Lift kind helicopter or tiltrotor (retail `no_lift`) | Pop-up and stand-off fire; never dive |
| 2 | Gunship | The record gives it the linked-gun gunsight ([AC-130 guns](ac130-linked-guns.md)) | Orbit |
| 3 | Heavy | Maximum takeoff weight 150,000 lb or more, or positive G limit 3 or less | Level run only; stand-off missiles from level flight; no dive, pop-up, rockets or strafe |
| 4 | Attack | Top speed 700 kt or less | Retail fighter passes, plus strafe and rocket re-attacks after the bombs, and a lower ingress |
| 5 | Fighter | Everything else | The four retail passes and the level run |

- **Retail check.** Over the 145 retail aircraft records the rule agrees with
  retail's own AI script binding for 142. The three differences are the B-1B
  (retail binds the fighter script; the rule makes it a Heavy, as John wanted),
  the Aurora (rule: Fighter; carries no ground stores) and the Blimp (rule:
  Helicopter by lift; no stores). A test pins this census.
  Class thresholds are **opinionated (John, 2026-10-11)**, the order and the
  top-speed test **opinionated (agent)**; retail has only the script binding
  (retail: fighter, bomber, helicopter and the AC-130's own).
- **The current import.** Helicopter: AH-64, Mi-24, V-22, CH-47. Gunship:
  AC-130U. Heavy: C-130, E-3, Il-76, E-2C, B747, A310 (none carries ground
  stores today). Attack: A-4E, Su-25, AV-8B, A-7, A-10, MiG-17F. Fighter: every
  other imported aircraft, the F/A-18D included. The F-22 uses its bay as in
  [weapons](air-to-ground.md#weapon-classes).
- **Override.** An optional per-aircraft override in the aircraft's T.O.R.E
  metadata, empty for every current aircraft, for the day the rule is wrong for
  one (the F-117, which retail flies at 5,000 ft level, is the likely first).
  Opinionated (agent).
- **The player's default.** The class sets only the **default** bomb mode: a
  Heavy selects LVL whatever its attitude (retail heavies have no CCIP); every
  other class keeps the [H1 rule](air-to-ground.md#status-word-and-mode-choice).
  Both modes stay available to the player on every aircraft. Opinionated (John,
  2026-10-11); no imported aircraft is a Heavy with bombs today.

## Skill and release precision

**The principle (opinionated, John, 2026-10-11):** the AI flies the pass
through control inputs and releases from the shared solutions. Top skill
releases on the solution. The lowest skill deviates by up to 30%. Higher skill,
higher precision.

**The definition (opinionated, agent; confirm the exact form with John).** It
is deterministic: no random draw.

- The skill factor `s` comes from the aircraft's experience level
  ([AI experience](ai-experience.md)): **Ace 0, Experienced 0.10, Average 0.20,
  Novice 0.30**.
- Every delivery has an **error budget** `B`, a length:
  - Unguided bombs and rockets: `B = max(1,000 ft, slant range to the target / 8)`,
    the retail release tolerance (retail, `0x4c3163`; retail used it as the
    pass or fail gate, here it is the length the skill fraction is taken of).
  - Guided bombs and seeker missiles: `B` is the **reach**, the ground range
    window between the weapon's minimum and maximum reach at the release altitude
    and speed ([guided reach](air-to-ground.md#guided-bomb-release-range-indicator)).
- The AI aims at a point shifted from the target by `s x B`. For unguided
  weapons the shift is along the ground track, **short** of the target on the
  first pass at it and **long** on the second, alternating on each re-attack (a
  low-skill pilot walks his bombs over the target). For guided weapons the shift
  is in release timing: the AI releases `s x B` of ground range **earlier** than
  an Ace would (the player's T bar is the far edge of the reach less 2/5 of the
  reach, so even a Novice at 0.30 stays inside it).
- The AI releases when its predicted mean impact (CCIP, the LVL cue) or its
  reach test is within **5% of `B`** of that aim point (fitted), flying the pipper
  there with its stick and rudder. The bomb then scatters as a player's does.

**Worked numbers** (arithmetic of this spec, not a measurement; unguided shifts
are short of the target on the first pass and long on the second; the scatter is
the retail 3 degrees in heading and pitch, 0.0524 of the range):

| Case | B | Ace | Experienced | Average | Novice | Scatter alone |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Mk 82 dive: 30 degrees, 6,000 ft slant, release at 3,000 ft AGL | max(1,000, 750) = 1,000 ft | 0 | 100 ft | 200 ft | 300 ft | about 310 ft across the track |
| Mk 82 level: 6,000 ft AGL, 450 kt (760 ft/s), 19.4 s fall, 14,700 ft travel, 15,900 ft slant | max(1,000, 1,990) = 1,990 ft | 0 | 199 ft | 398 ft | 597 ft | about 770 ft across the track |
| GBU-10 (illustration: reach 3.0 nmi long, 450 kt) | 3.0 nmi | releases at the bar, far edge less 1.2 nmi | 0.3 nmi earlier | 0.6 nmi earlier | 0.9 nmi earlier (0.3 nmi inside the far edge, about 7 s sooner) | none: the bomb steers |

Reading: a Novice Mk 82 in the dive is aimed 300 ft off, the size of the slick
bomb's 300 ft fuze radius ([records](air-to-ground.md#weapon-records)), so its
single bombs often land outside it; an Ace's mean impact is on the target. In
level flight the player's own scatter already spreads bombs hundreds of feet,
and the skill shift is of the same order. A guided bomb is accurate for every
skill because it steers; the cost of low skill is margin, and a lost spot or a
missed gimbal is what loses the weapon.

## Weapons and releases

| Store | Pass | Releases when | Budget `B` |
| --- | --- | --- | --- |
| Unguided bombs, packs | Table above | CCIP mean impact, or the LVL cue, on the aim point (a pack leaves in one instant, [packs](air-to-ground.md#release-modes-and-packs)) | Tolerance |
| Rockets | Straight run, shallow dive, or pop-up | Rocket impact on the aim point, at the record's cadence | Tolerance |
| Guns | Straight run | The existing gun employment ([AI guns](ai-gun-employment.md)), unchanged | none |
| Laser bombs, laser missiles | Level or shallow run; then an offset turn holding the target inside the designator's gimbal until impact (fitted) | Target inside the reach and a spot will exist (own designator, or a same-side spot); with none, the store is not chosen | Reach |
| JDAM | Level run | Target point stored, inside the reach | Reach |
| Mavericks, AS-14, AS-30 | Fast-high or straight | Seeker locked on the target inside zone 1 ([lock flow](air-to-ground.md#the-seekers-and-the-lock-flow)) | Reach |
| Radar AGMs (AS-7, AS-15, AGM-84A, AM-39, AS-16) | Straight run | Radar track at release, held through the flight (the AI support rule, [B45](ai.md#b45-seeker-visibility-launch-eligibility-and-ammunition)) | Reach |
| HARM | Fast-high; fire at maximum range, exit | A HARM candidate inside the zone ([HARM cue](air-to-ground.md#missiles-and-the-harm-boresight-cue)) | Reach |
| AT-2 (Mi-24) | Helicopter pop-up; hold the line until impact | Target inside the launcher's cone with a clear line | Reach |

Store choice is retail's ([B45](ai.md#b45-seeker-visibility-launch-eligibility-and-ammunition)):
bombs, rockets and ground missiles only against the surface; air-to-air
missiles only against aircraft; guns against both. Label: retail, spec-derived.
The delivery modes, solutions and guided flights are the player's, so this
spec adds none (spec-derived).

## Heavy bombers

| Rule | Label |
| --- | --- |
| Heavies bomb level only. They never dive, pop up, fire rockets or strafe | retail (the bomber script flies a high pass), opinionated (John, 2026-10-11) for the class test |
| Minimum release altitude: **max(5,000 ft AGL, the mission's airborne height)** | opinionated (John, 2026-10-11); retail uses max(altitude, 10,000 ft) |
| Raised by perceived threat: Clear and Air +0, Guns +5,000 ft, Missiles +10,000 ft, never above 90% of the aircraft's ceiling | opinionated (John, 2026-10-11) for "raised by threat"; the amounts are fitted |
| "The mission's airborne height" is the Quick Mission altitude setting ([Quick Mission](quick-mission-menu.md)); for an imported mission, the wing's start altitude. Read as AGL over the target area | fitted; confirm with John |
| Release is the weapon record's own burst (rounds and spacing from the record), centred on the aim point as a stick. There is no ripple setting | retail (burst fields), fitted (centring) |
| Exit climbs toward 50,000 ft (never above the ceiling), then turns back | retail |
| Stand-off missiles are fired from level flight at the reach | opinionated (agent) |

## Egress, re-attack and going home

After release the aircraft exits and decides again. All fitted from the retail
exit unless marked.

- **Exit** (retail, [ST-R](../formats/ai.md#strike-maneuvers-steering-frame-and-orbit)).
  Inside 10,000 ft horizontal, jink: four legs of 3 s at maximum speed, heading
  the current heading plus or minus 10 degrees, climbing 10 to 14 degrees, a
  weave of 10 degrees to one side, repeated while still inside 10,000 ft.
  Beyond 10,000 ft: hold heading, climb 10 degrees at maximum speed. At
  max(30,000 ft, twice the turn radius) away, turn back level at corner speed
  and choose again. Retail draws the weave's side and heading by random; T.O.R.E
  uses the existing seeded channel or a fixed side (John, decision 13).
- **Re-attack.** At the exit distance the aircraft runs the choice again with
  fresh threat and energy. Target alive and a usable store: attack. Target
  dead: the next target. No target left, or no usable store: home (the existing
  return to base, [B48](ai.md#b48-routes-fuel-and-recovery)).
- **Defend.** A missile warning or an attacking fighter hands control to the
  existing B47 reactions. The pass is abandoned, countermeasures and breaks run
  as today, and the striker resumes at ingress when the threat is gone and the
  picture says safe (below). Damage or low fuel sends it home.
- **Safe** (also used by takeover): no warning in the last 10 s, no known SAM
  covering the target, and no hostile fighter inside 15 nm. Fitted.

```mermaid
stateDiagram-v2
    [*] --> Ingress: strike tasked
    Ingress --> PassChoice: 10 nm from the target
    PassChoice --> Regain: energy low, guns or missiles threat
    Regain --> PassChoice: energy good
    PassChoice --> Attack: pass chosen from threat and speed
    Attack --> Release: solution on the aim point
    Release --> Guide: laser or command weapon in flight
    Release --> Egress: weapon away
    Guide --> Egress: impact or weapon lost
    Attack --> Egress: pass ends with no release
    Egress --> ReAttack: past the exit distance
    ReAttack --> PassChoice: target alive, store left
    ReAttack --> NextTarget: target dead
    ReAttack --> Home: no usable store
    NextTarget --> Ingress: target left
    NextTarget --> Home: none left
    Ingress --> Defend: threatened
    Attack --> Defend: threatened
    Defend --> Ingress: threat gone, safe
    Defend --> Home: damaged or bingo
    Home --> [*]
```

## SEAD

HARM carriers in a Strike wing take the objective's radars first: SAM radars,
then AAA with radar, then the rest of the briefed set. They fly the fast-high
pass and fire at the HARM's maximum range, then exit. A HARM already flying
keeps homing while the object stays in the missile's 45 degree cone, even if
the radar shuts down ([air-to-ground edge cases](air-to-ground.md#edge-cases)).
The retail Wild Weasel idea ("attack enemy air defenses", manual p. 169) is the
source; the order of radars and the fire range are opinionated (John,
2026-10-11, decision 15) with the details agent. HARM eligibility for the AI is
[unknown](#unknowns).

## Helicopters

Retail gives helicopters one ground pass: a pop-up with a 1,500 ft offset, a 30
degree pull, point down until 1,500 ft, then exit to 20,000 ft
([B20](ai.md#b20-other-aircraft-families), retail). The class (above) keeps
them to it and to stand-off fire. The AH-64 fires rockets in the pop-up and
the Mi-24 fires AT-2s, holding the line to the target until impact. Rockets and
the AT-2 use the player's release and guided rules. Under threat Missiles a
helicopter stays at the weapon's release range rather than closing to 1,500
ft (fitted).

## AC-130

Retail circles a target on a left pylon turn (`circle 0 4000 2000 35`, at a
speed halfway between corner and maximum, re-issued every 5 s; [B20](ai.md#b20-other-aircraft-families),
[formats](../formats/ai.md#strike-maneuvers-steering-frame-and-orbit)). Its
numbers are script constants, not derived from the guns: radius **4,000 ft**
horizontal about the moving target, **2,000 ft** above it, heading to the
centre plus 90 degrees (45 to 135 as it converges), and a requested bank of 35
degrees that the AC-130's `maxBank` of **30 degrees** caps (it banks 20 to 30).
On that orbit the target is 4,472 ft slant, 26.6 degrees down and abeam left,
about a third of the 13,000 ft gun zone, inside every mount arc (retail). The arcs ST-R
confirmed, as half-widths about the left-facing neutral: C_25 60/60, C_40 45/45,
C_105 25 in heading and 45 in pitch. John asked
that the gunship "know range and circle targets appropriately based on where
guns can hit" (opinionated, John, 2026-10-11). The orbit is therefore chosen
from the guns:

| Rule | Value | Label |
| --- | --- | --- |
| Gun reach used | The shortest range of the guns it will fire. All three installed guns have **13,000 ft** | spec-derived ([linked guns](ac130-linked-guns.md)) |
| Slant range to the target | 80% of the reach: **10,400 ft** | fitted |
| Depression angle | **35 degrees**, inside the 105 mm's elevation limit of 45 degrees and the 25 and 40 mm's | fitted on the shipped arcs |
| Orbit height and radius | `h = slant x sin 35 = 5,965 ft` above the target, `r = slant x cos 35 = 8,520 ft` | fitted |
| Minimum radius | If the aircraft's turn radius at the orbit speed and at most `maxBank` (about 6,200 ft at 200 kt and 30 degrees of bank) exceeds `r`, raise `r` to it and `h` with it, keeping 35 degrees | fitted |
| Direction | Left-hand, guns toward the target (the guns' arcs are left side only) | retail direction, spec-derived arcs |
| Bank limit | The orbit never asks for more bank than the aircraft's `maxBank` (30 degrees for the AC-130). When the radius for the planned speed and bank is larger than `r`, the radius grows (and the height with it, keeping 35 degrees); the bank does not. This replaces the 35 degree bank of the earlier draft | opinionated (agent); retail also caps bank at `maxBank` |
| Mount limits | The target stays inside the arcs above (C_25 60/60, C_40 45/45, C_105 25 heading, 45 pitch); 35 degrees of depression is inside every pitch limit | retail (arcs), fitted (orbit) |
| Retail orbit, for comparison | 4,000 ft radius, 2,000 ft high, slant 4,472 ft (34% of reach), fixed. T.O.R.E departs on purpose: John asked for the orbit from gun reach, which puts the aircraft farther out and higher | retail, opinionated (John, 2026-10-11) for the departure |
| Speed | Halfway between corner and maximum | retail |
| Firing | Gunship sight on the target; guns fire inside their arcs and the 13,000 ft range, as the player's | spec-derived |
| Threat | With a known SAM covering the target it does not ingress and holds off; under AAA it continues | opinionated (agent); confirm with John |

## Lasing

Retail has no buddy lasing and its AI lasing is implicit
([lasers](../formats/missiles.md)). The buddy rules come from
[air-to-ground](air-to-ground.md#buddy-lasing) (opinionated, John,
2026-10-10); the AI follows them (opinionated, John, 2026-10-11).

- **AI lases its own target.** An AI with a designator (built in, or a Pave
  pod) lases while its laser weapon is selected or in flight: the player's
  automatic rule, through the same laser, with the same gimbal (60 degrees
  search and 45 degrees track built in; hemisphere for a pod).
- **Its spot is an ordinary spot.** Players' laser weapons can home on it, and
  the target page shows BUDDY with the AI's callsign.
- **AI drops on a buddy spot.** An AI laser-bomb carrier with no designator
  chooses a laser store only when a same-side spot is on its target: a player
  lasing it, or a wingmate.
- **"Lase my target"** is a new wingman order (a free Alt key, chosen by the
  slice and listed in [controls](../CONTROLS.md)). A wingman with a designator
  flies an offset orbit of radius max(3 nm, 1.2 turn radii) above the release
  floor and lases the target you designated, until it dies, the order changes or
  the wingman is threatened (threat Missiles). Opinionated (John, 2026-10-11);
  orbit numbers fitted.
- **A designator lases for its wing.** A Strike wing with one designator
  aircraft and laser-bomb carriers lets the designator lase the lead's target.
  Fitted.
- **Order changes.** If the lasing wingman leaves, a laser weapon guiding on its
  spot goes blind, as for a player who stops lasing.

## Orders from the flight lead

Actions are given by the flight lead (John, 2026-10-11): the player for the
player's wingmen, an AI lead for an AI wing. A Strike wing's AI lead gives the
same orders to its wingmen by the target rules above (nearest briefed target,
two to an object, designator carriers lase).

| Key | Order | Label |
| --- | --- | --- |
| Alt+E | Engage my target. Accepts a ground designation, else the sight pin. The wingman attacks with its best surface store and answers "Engaging" | opinionated (John, 2026-10-11) on retail keys |
| Alt+R | Engage from formation, the same with the ground target | opinionated (John, 2026-10-11) |
| Alt+F | Engage the IR/laser target. Retail: the contact you picked with `/` and `\` while IR/Laser advanced targeting is on, air or ground; with no hostile contact the wingmen attack whatever they find. T.O.R.E matches this (decision 7). Taking the target from your sight track, pin or laser spot, as well as the picked contact, is an addition | retail (meaning), opinionated (John, 2026-10-11) on the key, opinionated (agent) for the sight, pin and spot sources |
| Lase my target | New; see [Lasing](#lasing) | opinionated (John, 2026-10-11) |
| Alt+W | Unchanged: attack on contact. No target-class orders now. Retail's Alt+W is the class order ("Attack SAMs", attack all of the target's class); keeping attack on contact here is a departure | opinionated (John, 2026-10-11); the departure from retail is opinionated (agent) |
| Others | Unchanged. A wingman with no usable surface store refuses with the reason shown ("no usable surface store") | spec-derived |

Alt+F: the retail manual calls Alt+F "attack on contact" and the executable's
key table calls it "engage the IR/laser target" ([keyboard](keyboard.md)).
Research (ST-R) settled it: both are true. Alt+F sends the IR/laser contact you
picked, and when there is no hostile contact it sends the free order, which is
attack on contact. T.O.R.E follows it. A ground target is accepted by retail's
receiver (Alt+E, Alt+R and Alt+F alike), so ordering wingmen onto the ground
needs no new order, only the target source above. Retail Alt+W is the class
order; T.O.R.E keeps attack on contact there for now (item 11 below).

## Takeover and handback

When a human gives a plane back (drop, leave, away, kick), the AI takes the
strike cockpit as its own, and when the human takes it back, they find what the
AI did. All opinionated (John, 2026-10-11, decisions 10 to 12) unless marked.

- **It continues only if it thinks it is safe** (the [safe](#egress-re-attack-and-going-home)
  test). If not, it defends and egresses, then resumes at ingress when safe. The
  kept target, store, mode and laser are held meanwhile.
- **Target.** The kept ground designation, else the sight pin. A kept air
  target is as today.
- **Store.** The kept station if it is a usable surface store, else the AI's
  choice. Armed state kept.
- **Pass.** Kept LVL flies a level run; kept CCIP the [table's pass](#pass-choice).
  The AI never changes the mode.
- **Laser.** The hand setting stands: hand off stays off, the automatic rule
  runs. With a laser weapon in flight the AI holds the target inside the gimbal
  until impact. A radar AGM in flight keeps its support.
- **Pages.** The AI flies with its own pages and its radar on (the existing
  handback rule).
- **On return.** The cockpit as the AI left it: the designation follows the AI's
  current target (a dead target is replaced by the next), rounds are what is
  left, and the AI never changed the delivery mode or the laser hand setting.
- **First take of an AI striker.** The human gets the AI's target designated, its
  selected store, delivery mode and laser state.

## Online and replays

AI strike runs on the host like all AI. Strike state and the strike cockpit go
in the checkpoints so that standby, migration and resume carry a pass in
progress, a laser spot and a guiding bomb. Clients see AI spots as buddy spots
in the readout. New order codes and the Quick Mission role in the lobby are a
protocol change, made by the multiplayer slice. Replays keep format 3 and record
AI lasing, designations and releases as events, which also records the laser of
a given-back plane that is not recorded today; the viewer draws AI spots by
side and says "guided on Blue 2-1's laser". Opinionated (John, 2026-10-11,
decisions 17); wire and event details are agent.

Single-player output stays as it is unless a mission uses the Strike role, a
strike order or a takeover of a striking plane.

## Numbers

| Figure | Value | Label |
| --- | --- | --- |
| Skill factors (Ace, Experienced, Average, Novice) | 0, 0.10, 0.20, 0.30 of the error budget | opinionated (John, 2026-10-11, the 30%); steps agent |
| Error budget, unguided | max(1,000 ft, slant range / 8) | retail tolerance |
| Error budget, guided and seeker | the weapon's reach | opinionated (agent) |
| Release on the solution within | 5% of the budget | fitted |
| Scatter | up to 3 degrees in heading and pitch, as the player | retail |
| Threat levels | Clear, Air (fighter within 30 nm), Guns, Missiles | opinionated (agent) |
| Energy | Good at or above corner speed | opinionated (agent) |
| Dive gate | altitude and distance both at least max(10,000 ft, 2 turn radii) | retail |
| Pass choice point | 10 nm from the target | fitted |
| Release floor | 3,000 ft (Clear, Air), 5,000 ft (Guns, Missiles) | retail manual advice, opinionated (agent) |
| Heavy minimum release altitude | max(5,000 ft AGL, mission airborne height), +0 / +5,000 / +10,000 ft by threat, at most 90% of ceiling | opinionated (John, 2026-10-11); raises fitted |
| Heavy exit | toward 50,000 ft | retail |
| Exit | jink inside 10,000 ft (4 legs of 3 s, 10 to 14 degrees up, 10 degree weave), then climb 10 degrees to max(30,000 ft, 2 turn radii) | retail |
| Guided release | inside 10,000 ft or half the weapon's maximum range | retail |
| Striker defends against | an aircraft that fired, locked, or is inside 5 nm facing it | fitted |
| Safe | no warning in 10 s, no SAM covering the target, no fighter inside 15 nm | fitted |
| Bombing class | helicopter by lift, gunship by gunsight, Heavy at 150,000 lb or G limit 3 or less, Attack at 700 kt or less, else Fighter | opinionated (John, 2026-10-11) |
| AC-130 orbit | 10,400 ft slant, 35 degrees, 5,965 ft high, 8,520 ft radius, bank at most the aircraft's `maxBank` (radius grows instead) | fitted; bank cap opinionated (agent) |
| AC-130 retail orbit | 4,000 ft radius, 2,000 ft high, 4,472 ft slant, bank 20 to 30 (`maxBank` 30) | retail |
| Lase orbit | radius max(3 nm, 1.2 turn radii) | fitted |
| Ground picture | rebuilt every 6 s, contacts followed every 0.25 s | retail |
| SEAD briefing | hostile armed units and radars within 5 nm of the objective point | fitted |
| Attackers per object | two | spec-derived |

## Unknowns

| Unknown | Fallback | Next step |
| --- | --- | --- |
| Whether the AC-130's rounds hit from the orbit (aim 20 ft above the target, no drop; hit test `_PROJHitChance`) | The player's gun ballistics | Test in the AC-130 slice |
| How retail's Quick Mission strike wings reach the target area (placement) | Strike wings fly to the objective (Who strikes) | Read the wing placement code if John wants retail parity |
| The wing message sent when an order deadline passes (20 s) | None | Low priority |
| Retail's choice of jink side and heading | Seeded channel or a fixed side | None |

Closed by ST-R, no longer unknown: the steering frame, dive rollout, jink path,
AC-130 circle and mount arcs, how retail gives the AI a ground target, Alt+F,
Alt+E, Alt+R and Alt+W receiver behaviour, HARM and missile release gates, skill
(retail strikes ignore it), AI guided bombs with no designator (they glide at
-20 degrees below 293 ft/s) and retail's Quick Mission strike wings.
| Whether heavy burst spacing from the record gives a sensible stick on the first imported heavy | The record's spacing | Test when a heavy with bombs is imported |

## Confirm with John

These are agent choices that carry a decision of John's. Each has a default
and is open to change.

1. **The 30% definition.** Skill factor 0 / 0.10 / 0.20 / 0.30 of an error
   budget (the retail release tolerance for unguided weapons, the reach for
   guided ones), short then long on alternate passes for unguided weapons, early
   release for guided ones. Alternatives: scale the weapon's scatter instead, or
   take the fraction of the fuze radius.
2. **Skill source.** The aircraft's own experience level, the same four levels
   as air combat.
3. **Threat and energy levels, the release floor, and the order in the
   pass table**, especially "Missiles gives a pop-up" and "Guns gives a dive
   from above".
4. **Heavy minimum altitude.** "Mission airborne height" read as the Quick
   Mission altitude setting, as AGL, plus the threat raise of +5,000 and +10,000
   ft.
5. **AC-130 under a SAM** holds off instead of attacking, and the 35 degree,
   80%-of-reach orbit (with the bank held to `maxBank`, the radius growing
   instead).
6. **Alt+F** matches retail (the IR/laser contact, else attack on contact);
   the sight, pin and spot as extra sources, and attack on contact on Alt+W
   rather than retail's class order, are departures (see item 11).
7. **Safe** for takeover: no warning in 10 s, no covering SAM, no fighter
   inside 15 nm.
8. **Lase my target** orbit and key.
9. **Default role for bomber-class wings.** Retail Quick Mission sends a
   friendly bomber-class wing (A-4E, A-6, F-117, B-52, AC-130U, AH-64, Mi-24,
   Su-24, Tornado) at the ground target automatically when one is set, with
   fighter-class wings (A-10 and A-7 included) air only. Should a bomber-class
   wing default to Strike when a ground target is set, as retail does, instead
   of CAP? Default here: CAP for everyone, as John answered, with the player
   free to set Strike.
10. **Pop-up speed quirk.** Retail's pop-up pull requests minimum speed by an
    operand-order mistake. The spec recommends not copying it (tactics follow
    threat and speed). Say if the retail bleed is wanted.
11. **Alt+W.** Retail's Alt+W is "attack all of the target's class" (Attack
    SAMs, bandits, tanks). The spec keeps attack on contact there. Say if the
    class order should return, which would also give wingmen a SEAD command.

## Source notes

- Retail evidence: the AI scripts and records summarised in [AI
  behaviour](ai.md) and the [AI formats](../formats/ai.md) (B14, B20, B41, B42,
  B45, B47, B48), the weapon notes in [weapons](../formats/weapons.md), the
  retail manual text (pp. 120, 159, 160, 169, 170) and the aircraft table
  ([aircraft table](../formats/fa-aircraft.csv)). Build identity FA.EXE 1.02F.
- John's request and answers (2026-10-11) are in the AI strike planning notes
  outside the repository; the design behind every opinionated row is summarised
  here, not restated elsewhere.
- Related specs: [AI behaviour](ai.md), [air-to-ground](air-to-ground.md),
  [surface defenses](surface-defenses.md), [AI experience](ai-experience.md),
  [AI guns](ai-gun-employment.md), [AC-130 guns](ac130-linked-guns.md),
  [keyboard](keyboard.md).
- Slices that build it: ST-R research, ST-D driver, ST-Y bombing class, ST-P
  ground picture, ST-C strike cockpit, ST-T tactics, ST-L lasing, ST-O orders
  and roles, ST-K takeover, ST-F helicopters and AC-130, ST-N multiplayer,
  ST-RA replay, ST-B baseline, ST-A acceptance. The evidence will be a baseline
  at `docs/baselines/ai-strike.md` when the round is accepted.
