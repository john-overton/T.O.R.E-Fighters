"""Lane: flight. The AI strike driver (AI strike round, slice ST-D;
`.local/ai-a2g/plan.md`, docs/testing/lane-flight.md "AI strike driver").

`--a2g-run THEATER STEM --pilot ai` lets the AI fly the striker and prints
`ai-strike:` lines (PROFILE, RELEASE, IMPACT miss_ft, KILL, EGRESS, LOST) from
what the simulation reports. Two kinds of scenario use it:

* Driver scenarios, which pass today and keep passing as the strike behaviour
  lands: they check that the driver runs, that its report is consistent with
  itself (the summary counts the lines, a release comes before its impact),
  that a human-flown run is handed to the AI, and that a wing order goes out.
  `ai-strike-driver`, `ai-strike-handback`, `ai-strike-order`.
* Behaviour scenarios, one per weapon class (slice ST-T, the strike tactics):
  `ai-strike-bombs`, `ai-strike-rockets`, `ai-strike-maverick`,
  `ai-strike-jdam`, `ai-sead-harm`, `ai-strike-defended`. Each run names the
  pass it flew, releases, and ends the round within its skill's limits; the
  unguided ones also print the pass's drawn aim offset on the RELEASE line,
  which must lie inside the release circle when the draw says so and in the
  ring R to 2R otherwise.

`parse_runs` reads the output; the check functions are tested in
tools/test_battery_ai_strike.py.
"""
from __future__ import annotations

import dataclasses
import math
import re

from battery import Scenario, Step
from battery_scenarios.a2g_missiles import STRAIGHT_FLUSH

LINE = re.compile(r"^ai-strike: (\S+)(?: (.*))?$", re.M)
FIELD = re.compile(r'(\w+)=("[^"]*"|\S+)')

# John's release circle (2026-10-11, docs/spec/ai-strike.md#skill-and-release-precision), by skill
# 0 (Novice), 1 (Average), 2 (Experienced), 3 (Ace): 85, 60, 35 and 10 m, in feet (279, 197, 115
# and 33).
CIRCLE_M = {0: 85.0, 1: 60.0, 2: 35.0, 3: 10.0}
CIRCLE_FT = {skill: metres / 0.3048 for skill, metres in CIRCLE_M.items()}
# The release gate (`strike.rs` GATE_SHARE and GATE_FLOOR_FT): the cue may be this far from the aim
# point at the release, so the bomb's mean impact may sit this far outside the drawn aim.
GATE_FT = {skill: max(0.1 * r, 10.0) for skill, r in CIRCLE_FT.items()}
# The bomb's own scatter, on top of the aim offset: half what it was at 3 degrees (800 ft), as John
# halved the scatter to 1.5 degrees. The worst of the specced cases is the level Mk 82 (about 385
# ft across the track); the dive is about 155 ft.
SCATTER_FT = 400.0
# Horizontal miss, in feet, a bomb pass may end within. The seeded aim offset is inside the circle
# 90% of passes and inside the ring R to 2R on the other 10%: a run whose RELEASE lines all say
# `inside=yes` is held to the circle plus the gate plus the scatter, any other to the ring's outer
# edge plus them.
MISS_FT_MAX = {skill: 2 * r + GATE_FT[skill] + SCATTER_FT for skill, r in CIRCLE_FT.items()}
MISS_FT_INSIDE = {skill: r + GATE_FT[skill] + SCATTER_FT for skill, r in CIRCLE_FT.items()}
# The release floor under threat Clear (`strike.rs` FLOOR_CLEAR_FT).
FLOOR_CLEAR_FT = 3000.0
# The driver rounds the aim offset to whole feet.
AIM_SLACK_FT = 1.0
# A rocket or a guided weapon is judged by the target, not a miss distance.
SKILLS = (0, 1, 2, 3)
EVENT_KINDS = ("RELEASE", "IMPACT", "KILL", "EGRESS", "LOST")


@dataclasses.dataclass
class Run:
    """The `ai-strike:` lines of one driver run, in order, as (kind, fields)."""

    lines: list[tuple[str, dict[str, str]]]

    def of(self, kind: str) -> list[dict[str, str]]:
        return [f for k, f in self.lines if k == kind]

    def first(self, kind: str) -> dict[str, str] | None:
        found = self.of(kind)
        return found[0] if found else None

    @property
    def setup(self) -> dict[str, str]:
        return self.first("setup") or {}

    @property
    def summary(self) -> dict[str, str]:
        return self.first("summary") or {}

    @property
    def skill(self) -> int:
        return int(self.setup.get("skill", "-1"))

    def times(self, kind: str) -> list[float]:
        return [float(f["t"]) for f in self.of(kind) if "t" in f]


def parse_line(kind: str, rest: str | None) -> dict[str, str]:
    """`key=value` fields; a quoted value keeps its spaces and loses its quotes."""
    fields = {}
    for key, value in FIELD.findall(rest or ""):
        fields[key] = value[1:-1] if value.startswith('"') else value
    return fields


def parse_runs(output: str) -> list[Run]:
    """The runs in `output`: each ends at its `ai-strike: done`."""
    runs: list[Run] = []
    current: list[tuple[str, dict[str, str]]] = []
    for m in LINE.finditer(output):
        kind, rest = m.group(1), m.group(2)
        if kind == "done":
            runs.append(Run(current))
            current = []
        elif kind == "no":
            current.append(("no-release", {}))
        else:
            current.append((kind, parse_line(kind, rest)))
    if current:
        runs.append(Run(current))
    return runs


def number(value: str | None) -> float:
    try:
        return float(value) if value is not None else math.nan
    except ValueError:
        return math.nan


def consistency_problems(run: Run, label: str) -> list[str]:
    """What must hold of any run, whatever the AI does."""
    problems: list[str] = []
    kinds = [k for k, _ in run.lines]
    for once in ("setup", "start", "summary"):
        if kinds.count(once) != 1:
            problems.append(f"{label}: {kinds.count(once)} {once} lines, want 1")
    if "setup" in kinds and "PROFILE" not in kinds:
        problems.append(f"{label}: no PROFILE line")
    s = run.summary
    if not s:
        return problems
    counts = {
        "releases": len(run.of("RELEASE")),
        "impacts": len(run.of("IMPACT")),
        "kills": len(run.of("KILL")),
    }
    for key, count in counts.items():
        if s.get(key) != str(count):
            problems.append(f"{label}: summary says {key}={s.get(key)}, the lines show {count}")
    for key, kind in (("egress", "EGRESS"), ("lost", "LOST")):
        want = "yes" if run.of(kind) else "no"
        if s.get(key) != want:
            problems.append(f"{label}: summary says {key}={s.get(key)}, the lines show {want}")
    if counts["releases"] == 0 and "no-release" not in kinds:
        problems.append(f"{label}: no release, but the run does not say so")
    if counts["releases"] and "no-release" in kinds:
        problems.append(f"{label}: released, yet says no release")
    if counts["impacts"] > counts["releases"]:
        problems.append(f"{label}: {counts['impacts']} impacts for {counts['releases']} releases")
    releases, impacts = run.times("RELEASE"), run.times("IMPACT")
    if impacts and (not releases or min(impacts) < min(releases)):
        problems.append(f"{label}: an impact before the first release")
    egress = run.times("EGRESS")
    if egress and (not releases or min(egress) < min(releases)):
        problems.append(f"{label}: egress before the first release")
    if len(run.of("LOST")) > 1:
        problems.append(f"{label}: lost more than once")
    stamps = [float(f["t"]) for k, f in run.lines if k in EVENT_KINDS and "t" in f]
    if stamps != sorted(stamps):
        problems.append(f"{label}: events out of order in time: {stamps}")
    return problems


def driver_problems(output: str, want: int, forced: list[str | None]) -> list[str]:
    """`ai-strike-driver`: `want` runs, each consistent, each echoing the profile it was asked
    to fly and a PROFILE line that is either the forced one or an honest `none`."""
    runs = parse_runs(output)
    if len(runs) != want:
        return [f"{len(runs)} runs finished, want {want}"]
    problems: list[str] = []
    for i, (run, profile) in enumerate(zip(runs, forced)):
        label = f"run {i + 1}"
        problems += consistency_problems(run, label)
        if run.setup.get("profile") != (profile or "none"):
            problems.append(f"{label}: setup says profile={run.setup.get('profile')}, asked {profile}")
        named = run.first("PROFILE") or {}
        if named.get("requested") != (profile or "none"):
            problems.append(f"{label}: PROFILE requested={named.get('requested')}, asked {profile}")
        if named.get("name") not in ("none", profile):
            problems.append(f"{label}: PROFILE name={named.get('name')}, want {profile} or none")
        if run.setup.get("hp", "0") == "0":
            problems.append(f"{label}: the target has no hit points")
    return problems


def handback_problems(output: str) -> list[str]:
    """`ai-strike-handback`: run 1 holds the trigger in LVL, so a bomb leaves the human before the
    handoff; run 2 hands back with the bomb selected and nothing released."""
    runs = parse_runs(output)
    if len(runs) != 2:
        return [f"{len(runs)} runs finished, want 2"]
    problems: list[str] = []
    for i, run in enumerate(runs):
        label = f"run {i + 1}"
        problems += consistency_problems(run, label)
        back = run.first("HANDBACK")
        if back is None:
            problems.append(f"{label}: no HANDBACK line")
            continue
        if back.get("mode") != "LVL":
            problems.append(f"{label}: handed back in {back.get('mode')}, asked LVL")
        if back.get("selected") in (None, "0"):
            problems.append(f"{label}: handed back with station {back.get('selected')} selected, want the bomb's")
        if run.of("DESIGNATE") == [] or run.first("DESIGNATE").get("target") == "none":
            problems.append(f"{label}: the human designated nothing")
        after = [t for t in run.times("ACTIVITY") if t >= float(back["t"])]
        if not after:
            problems.append(f"{label}: no AI activity at or after the handback")
    first = runs[0]
    released = first.times("RELEASE")
    back = first.first("HANDBACK")
    if not released:
        problems.append("run 1: the human released nothing before the handback")
    elif back and released[0] >= float(back["t"]):
        problems.append("run 1: the first release came after the handback")
    for impact in first.of("IMPACT")[:1]:
        miss = number(impact.get("miss_ft"))
        if not miss <= 1500:
            problems.append(f"run 1: the human's bomb ended {impact.get('miss_ft')} ft from the target")
    if runs[1].of("RELEASE") and runs[1].times("RELEASE")[0] < float(runs[1].first("HANDBACK")["t"]):
        problems.append("run 2: the human released, but the trigger was never pressed")
    return problems


def order_problems(output: str) -> list[str]:
    """`ai-strike-order`: the seat designates, gives the order, and the world answers it."""
    runs = parse_runs(output)
    if len(runs) != 1:
        return [f"{len(runs)} runs finished, want 1"]
    run = runs[0]
    problems = consistency_problems(run, "run 1")
    order = run.first("ORDER")
    if order is None:
        return problems + ["no ORDER line"]
    if order.get("designated") in (None, "none"):
        problems.append("the order went out with nothing designated")
    reply = run.first("ORDER-REPLY")
    if reply is None:
        problems.append("the world gave no reply to the order")
    elif reply.get("result") not in ("given", "refused", "failed"):
        problems.append(f"order reply {reply.get('result')}")
    elif not reply.get("message"):
        problems.append("the order's reply has no words")
    if run.setup.get("order") in (None, "none"):
        problems.append("setup does not name the order")
    return problems


def strike_problems(output: str, runs_wanted: list[dict]) -> list[str]:
    """The behaviour scenarios: for each run (a dict with `label`, `profile`, `skill`, and
    optional `miss` (bomb miss limit) or `weapon_hit`), the AI named the pass it flew, released,
    ended the round near the target, left, and was not lost."""
    runs = parse_runs(output)
    if len(runs) != len(runs_wanted):
        return [f"{len(runs)} runs finished, want {len(runs_wanted)}"]
    problems: list[str] = []
    for run, want in zip(runs, runs_wanted):
        label = want["label"]
        problems += consistency_problems(run, label)
        # The pass the AI reports flying: the last PROFILE line (the first is the driver's echo).
        named = (run.of("PROFILE") or [{}])[-1]
        profile = want.get("profile")
        if profile and named.get("name") != profile:
            problems.append(f"{label}: flew PROFILE {named.get('name')}, asked {profile}")
        if not run.of("RELEASE"):
            problems.append(f"{label}: no RELEASE")
        if not run.of("IMPACT"):
            problems.append(f"{label}: no IMPACT")
        limit = want.get("miss")
        if limit is not None and want.get("skill") in CIRCLE_FT:
            problems += aim_problems(run, label, want["skill"])
            if run.of("RELEASE") and all(r.get("inside") == "yes" for r in run.of("RELEASE")):
                limit = min(limit, MISS_FT_INSIDE[want["skill"]])
            # A rocket's or gun's own spread in place of the bomb's scatter.
            limit += want.get("spread", SCATTER_FT) - SCATTER_FT
        for impact in run.of("IMPACT") if limit is not None else []:
            miss = number(impact.get("miss_ft"))
            if not miss <= limit:
                problems.append(f"{label}: impact {impact.get('miss_ft')} ft from the target, want at most {limit:.0f}")
        problems += maneuver_problems(run, label, want)
        lost_allowed = run.of("LOST") and want.get("may_lose")
        if (limit is not None and run.of("RELEASE") and not run.of("EGRESS") and not lost_allowed
                and not want.get("no_egress")):
            problems.append(f"{label}: no EGRESS after the release")
        if run.of("LOST") and not want.get("may_lose"):
            problems.append(f"{label}: the striker was lost ({run.first('LOST')})")
        if want.get("defends") and not any(a.get("name") == "Defending" for a in run.of("ACTIVITY")):
            problems.append(f"{label}: never defended against the defences")
        if want.get("hit") and not _target_hurt(run):
            problems.append(f"{label}: the target is unhurt ({run.summary.get('target_hp')})")
    return problems


def maneuver_problems(run: Run, label: str, want: dict) -> list[str]:
    """The ST-T2 maneuver checks a run may ask for: `min_dive` (every RELEASE in a dive at least
    this steep, degrees), `max_alt` / `min_alt` (every RELEASE's height above the ground),
    `first_pass` (the first RELEASE on pass 1), `moving` (True: the RELEASE lays a lead on the
    target; False: none) and `bursts` (at least this many gun bursts)."""
    problems: list[str] = []
    releases = run.of("RELEASE")
    for release in releases:
        dive, alt = number(release.get("dive_deg")), number(release.get("alt_ft"))
        if "min_dive" in want and not dive >= want["min_dive"]:
            problems.append(f"{label}: released in a {release.get('dive_deg')} degree dive, want at least {want['min_dive']}")
        if "max_alt" in want and not alt <= want["max_alt"]:
            problems.append(f"{label}: released at {release.get('alt_ft')} ft, want at most {want['max_alt']}")
        if "min_alt" in want and not alt >= want["min_alt"]:
            problems.append(f"{label}: released at {release.get('alt_ft')} ft, want at least {want['min_alt']}")
        lead = number(release.get("lead_ft"))
        if want.get("moving") is True and not lead > 0:
            problems.append(f"{label}: no lead on the moving target ({release.get('lead_ft')})")
        if want.get("moving") is False and not lead == 0:
            problems.append(f"{label}: a lead of {release.get('lead_ft')} ft on a standing target")
    if want.get("first_pass") and releases and releases[0].get("pass") != "1":
        problems.append(f"{label}: first release on pass {releases[0].get('pass')}, want pass 1")
    if "bursts" in want and len([r for r in releases if "burst" in r]) < want["bursts"]:
        problems.append(f"{label}: {len(releases)} gun bursts, want at least {want['bursts']}")
    return problems


def aim_problems(run: Run, label: str, skill: int) -> list[str]:
    """Each RELEASE's drawn aim offset against the skill's circle R: inside R when the draw says
    `inside=yes`, in the ring R to 2R when it says `no` (docs/spec/ai-strike.md)."""
    problems: list[str] = []
    r = CIRCLE_FT[skill]
    for release in run.of("RELEASE"):
        aim = number(release.get("aim_ft"))
        inside = release.get("inside")
        if inside == "yes" and not aim <= r + AIM_SLACK_FT:
            problems.append(f"{label}: aim {release.get('aim_ft')} ft is outside the {r:.0f} ft circle it was drawn in")
        elif inside == "no" and not r - AIM_SLACK_FT <= aim <= 2 * r + AIM_SLACK_FT:
            problems.append(f"{label}: aim {release.get('aim_ft')} ft is outside the ring {r:.0f} to {2 * r:.0f} ft")
        elif inside not in ("yes", "no"):
            problems.append(f"{label}: a RELEASE without its aim draw")
    return problems


def _target_hurt(run: Run) -> bool:
    now, _, start = run.summary.get("target_hp", "").partition("/")
    return now == "gone" or (now.lstrip("-").isdigit() and start.isdigit() and int(now) < int(start))


def _runs(skills_and_profiles, miss=True, **extra) -> list[dict]:
    wanted = []
    for skill, profile in skills_and_profiles:
        wanted.append(
            dict(label=f"skill {skill} {profile or 'free'}", profile=profile, skill=skill,
                 miss=MISS_FT_MAX[skill] if miss else None, **extra)
        )
    return wanted


# The plan's bomb coverage: each pass forced once at the middle skill, and every skill on the dive.
BOMB_RUNS = [(2, p) for p in ("straight", "popup", "fasthigh", "dive", "level")] + [(s, "dive") for s in (0, 1, 3)]


# ST-T2: a rocket's spread and a gun burst's (the middle of the rounds that reach the ground) on
# top of the aim, in place of the bomb's scatter: fitted from the driver runs (rockets 90 to 125 ft
# from a 90 ft aim at skill 2; bursts 85 to 120 ft from 85 to 102 ft aims).
ROCKET_SPREAD_FT = 250.0
GUN_SPREAD_FT = 150.0
# The A-10's strafing target: the column's lighter vehicle (150 hit points; the tanks have 200).
STRAFE_UNIT = "0x50000003"
# Static and moving targets for the lead comparison: the Ukraine column (moving, about 50 ft/s)
# and the Ukraine factory (standing).
LEAD_STEMS = (("QUCOL", True), ("QUFACT", False))


def pilot_run(theater: str, stem: str, *rest: str) -> list[str]:
    return ["--a2g-run", theater, stem, "--pilot", "ai", *rest]


def scenarios() -> list[Scenario]:
    qucol = ["--aircraft", "a4e", "--load", "MK82", "--seconds", "60"]
    smoke = pilot_run("UKR", "QUCOL", *qucol)
    hand = ["--aircraft", "a4e", "--load", "MK82", "--handback", "30", "--mode", "lvl", "--altitude", "5000",
            "--seconds", "80"]
    bombs = ["--aircraft", "a4e", "--load", "MK82", "--seconds", "120"]

    def forced(skill: int, profile: str, *rest: str, theater: str = "UKR", stem: str = "QUCOL") -> list[str]:
        return pilot_run(theater, stem, *rest, "--skill", str(skill), "--profile", profile)

    # The column, not the TVIET truck park: the park's radar-laid guns (26,200 and 44,250 ft
    # reach) shoot an A-4E down within 25 s at 5,000 ft whatever it flies.
    bomb_args = [forced(s, p, *bombs) for s, p in BOMB_RUNS]
    # The straight rocket run at two skills (the pop-up and dive have their own ST-T2 scenarios).
    # Not the Ace: on a short straight run its 10 m circle is rarely met before the run ends.
    rocket_runs = [(1, "straight"), (2, "straight")]
    rockets = ["--aircraft", "a4e", "--load", "LAU61", "--seconds", "120"]
    maverick = ["--aircraft", "a10", "--load", "AGM65G", "--seconds", "120"]
    jdam = ["--aircraft", "f16c", "--load", "GBU30", "--station", "2", "--count", "2", "--seconds", "120"]
    a4e_bombs = ["--aircraft", "a4e", "--load", "MK82"]

    def run_list(entries):
        """(args, want) pairs to (first args, then steps, wants)."""
        return entries[0][0], [Step(a, timeout=400) for a, _ in entries[1:]], [w for _, w in entries]

    def want(skill, profile, **extra):
        label = extra.pop("label", f"skill {skill} {profile}")
        return dict(label=label, profile=profile, skill=skill, miss=MISS_FT_MAX[skill], **extra)

    steep = [(forced(s, "dive", *a4e_bombs, "--seconds", "150"),
              want(s, "dive", min_dive=45, min_alt=FLOOR_CLEAR_FT, moving=True)) for s in SKILLS]
    strafe = [(pilot_run("UKR", "QUCOL", "--unit", STRAFE_UNIT, "--aircraft", "a10", "--load", "GAU8",
                         "--seconds", "90", "--skill", str(s), "--profile", "strafe"),
               want(s, "strafe", spread=GUN_SPREAD_FT, bursts=1, no_egress=True, may_lose=True,
                    max_alt=4000)) for s in (1, 2)]
    rockets_popup = [(forced(2, "popup", *rockets), want(2, "popup", spread=ROCKET_SPREAD_FT, no_egress=True))]
    rockets_dive = [(forced(2, "dive", *rockets), want(2, "dive", spread=ROCKET_SPREAD_FT, no_egress=True))]
    snakeye = [(forced(s, "laydown", "--aircraft", "a4e", "--load", "MK82HD", "--seconds", "120"),
                want(s, "laydown", max_alt=1000, moving=True)) for s in (1, 2)]
    heavy = [(pilot_run("UKR", "QUFACT", "--aircraft", "c130", "--load", "MK82", "--station", "0", "--count", "8",
                        "--mission-altitude", "10000", "--altitude", "7000", "--speed", "250", "--from", "8",
                        "--seconds", "360", "--skill", "2", "--profile", "level"),
              want(2, "level", min_alt=9500, moving=False, label="heavy level from the 10,000 ft mission height"))]
    offsets = [(forced(2, p, *a4e_bombs, "--offset", str(off), "--seconds", "150"),
                want(2, p, first_pass=True, label=f"{p} {off} ft off the line"))
               for p in ("level", "dive") for off in (1000, 3000)]

    def lead_runs(profile):
        return [(pilot_run("UKR", stem, *a4e_bombs, "--seconds", "240", "--skill", str(s), "--profile", profile),
                 want(s, profile, moving=moving, label=f"skill {s} {profile} {stem} {'moving' if moving else 'static'}"))
                for stem, moving in LEAD_STEMS for s in SKILLS]

    maneuvers = [
        ("ai-strike-dive-steep", steep, 900,
         "An A-4E's Mk 82 dive at every skill: rolled inverted onto a dive steeper than 45 degrees (retail's "
         "roll-in), released above the 3,000 ft floor with a lead on the moving column, within the circle."),
        ("ai-strike-strafe", strafe, 600,
         "An A-10's GAU-8 gun run on the column's lighter vehicle at skills 1 and 2: shallow dive, each burst "
         "on its own seeded aim inside the circle (or the ring), the bursts' middle within the circle plus "
         "the gun's spread, break-off low."),
        ("ai-strike-rockets-popup", rockets_popup, 600,
         "An A-4E's LAU-61 pop-up at skill 2: the pull, the roll-in onto the column, rockets on the cue."),
        ("ai-strike-rockets-dive", rockets_dive, 600,
         "An A-4E's LAU-61 dive at skill 2 from 6,000 ft, rockets on the cue."),
        ("ai-strike-snakeye", snakeye, 600,
         "An A-4E's Mk 82 Snakeye laydown at skills 1 and 2: down to the low level height that clears the "
         "fuze and the fragments (under 1,000 ft), released level with a lead on the column."),
        ("ai-strike-heavy-level", heavy, 900,
         "A C-130 with a bay of Mk 82 on the factory: its release floor is the 10,000 ft Quick Mission "
         "altitude, not the 5,000 ft base."),
        ("ai-strike-offset", offsets, 900,
         "Level and dive Mk 82 passes starting 1,000 and 3,000 ft off the attack line: the sideways law lays "
         "the track on the aim and releases on the first pass."),
        ("ai-strike-lead-dive", lead_runs("dive"), 1200,
         "John's comparison: the same A-4E Mk 82 dive on the moving column and the standing factory at every "
         "skill; the moving target is led, the standing one not, both within the circle."),
        ("ai-strike-lead-level", lead_runs("level"), 1200,
         "The same comparison for the level run."),
    ]
    extra = []
    for name, entries, timeout, notes in maneuvers:
        first, steps, wants = run_list(entries)
        extra.append(Scenario(name=name, lane="flight", args=first, timeout=timeout, then=steps,
                              check=lambda out, wants=wants: strike_problems(out, wants), notes=notes))
    return extra + [
        Scenario(
            name="ai-strike-driver", lane="flight",
            args=smoke + ["--skill", "2", "--profile", "dive"], timeout=300,
            then=[
                Step(smoke + ["--skill", "0", "--profile", "popup"], timeout=300),
                Step(smoke + ["--skill", "3"], timeout=300),
            ],
            check=lambda out: driver_problems(out, 3, ["dive", "popup", None]),
            notes="The AI flies an A-4E Mk 82 pass at QUCOL and the driver reports it, at three skills, "
                  "with and without a forced profile; passes before and after the strike behaviour lands.",
        ),
        Scenario(
            name="ai-strike-handback", lane="flight",
            args=pilot_run("UKR", "QUCOL", *hand, "--trigger"), timeout=300,
            then=[Step(pilot_run("UKR", "QUCOL", "--aircraft", "a4e", "--load", "MK82", "--handback", "6",
                                 "--mode", "lvl", "--altitude", "5000", "--seconds", "30"), timeout=300)],
            check=handback_problems,
            notes="A seat designates the column, selects the Mk 82 in LVL and holds the trigger (a bomb is "
                  "released and reported), then gives the aircraft to the AI; and a second run handed back "
                  "before the release. Both report the handoff and what the AI does after it.",
        ),
        Scenario(
            name="ai-strike-order", lane="flight",
            args=pilot_run("UKR", "QUCOL", "--aircraft", "a4e", "--load", "MK82", "--order", "engage-my-target",
                           "--order-at", "3", "--seconds", "30"),
            timeout=300,
            check=order_problems,
            notes="A seat leads a two-ship wing, designates the column and gives the wingman Engage my "
                  "target; the driver reports the order and the world's reply.",
        ),
        Scenario(
            name="ai-strike-bombs", lane="flight",
            args=bomb_args[0], timeout=900,
            then=[Step(a, timeout=300) for a in bomb_args[1:]],
            check=lambda out: strike_problems(out, _runs(BOMB_RUNS, hit=False)),
            notes="An A-4E's Mk 82 on the Ukraine armored column: each pass (straight, pop-up, fast-high, "
                  "dive, level) forced once at skill 2 and skills 0, 1 and 3 on the dive. Each run names its "
                  "pass, releases with its drawn aim inside the release circle (or the ring R to 2R when the "
                  "draw says so), ends the bomb within the circle plus the gate plus 400 ft of scatter (the ring's edge plus "
                  "it for a ring draw), leaves and is not lost.",
        ),
        Scenario(
            name="ai-strike-rockets", lane="flight",
            args=forced(*rocket_runs[0], *rockets), timeout=600,
            then=[Step(forced(*r, *rockets), timeout=300) for r in rocket_runs[1:]],
            check=lambda out: strike_problems(out, _runs(rocket_runs, hit=False)),
            notes="An A-4E's LAU-61 pods on the column, straight runs at skills 1 and 2: a burst at the "
                  "record's cadence with the drawn aim inside the circle, each rocket within the circle "
                  "plus the gate plus 400 ft, and the striker leaves.",
        ),
        Scenario(
            name="ai-strike-maverick", lane="flight",
            args=pilot_run("UKR", "QUCOL", *maverick, "--skill", "2", "--profile", "fasthigh"),
            timeout=600,
            check=lambda out: strike_problems(out, _runs([(2, "fasthigh")], miss=False, hit=True)),
            notes="An A-10's Maverick on the Ukraine armored column, fast-high: fired on its lock and "
                  "the tank is hurt.",
        ),
        Scenario(
            name="ai-strike-jdam", lane="flight",
            args=pilot_run("UKR", "QUFACT", *jdam, "--skill", "2", "--profile", "level"),
            timeout=600,
            check=lambda out: strike_problems(out, _runs([(2, "level")], miss=False, hit=True)),
            notes="An F-16C's GBU-30 JDAM on the Ukraine factory, level: the stored point is the target "
                  "(no skill offset for a smart weapon) and the target is hurt. A static target: a JDAM "
                  "flies to the point stored at release, so a moving column would be missed.",
        ),
        Scenario(
            name="ai-sead-harm", lane="flight",
            args=pilot_run("VLA", "QVRDRI", "--surface-seed", "4", "--defenses", "3", "3", "--unit",
                           STRAIGHT_FLUSH, "--aircraft", "f4g", "--load", "AGM88", "--from", "12",
                           "--altitude", "15000", "--seconds", "120", "--skill", "2", "--profile", "fasthigh"),
            timeout=600,
            check=lambda out: strike_problems(out, _runs([(2, "fasthigh")], miss=False, hit=True)),
            notes="An F-4G's HARM on the heavily defended site's radar, fast-high: fired inside the AI's "
                  "100,000 ft HARM range while the radar emits, and the radar is hurt.",
        ),
        Scenario(
            name="ai-strike-defended", lane="flight",
            args=pilot_run("UKR", "QUCOL", "--aircraft", "f18", "--load", "MK82", "--defenses", "2", "2",
                           "--seconds", "180", "--skill", "2"),
            timeout=600,
            check=lambda out: strike_problems(out, _runs([(2, None)], hit=False, may_lose=True, defends=True)),
            notes="An F/A-18D's Mk 82 on the defended column, its pass chosen by threat (SAMs cover the "
                  "column: a pop-up): releases inside its circle and defends against the defences. The "
                  "column's own guns may still bring it down at the pop-up's low altitude, so a loss is "
                  "allowed; the run checks the release and the defence.",
        ),
    ]
