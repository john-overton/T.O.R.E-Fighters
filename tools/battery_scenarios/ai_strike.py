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
* Behaviour scenarios, one per weapon class, which describe what the strike
  behaviour has to do and are marked as known failures until slice ST-T makes
  the AI take a ground target: `ai-strike-bombs`, `ai-strike-rockets`,
  `ai-strike-maverick`, `ai-strike-jdam`, `ai-sead-harm`, `ai-strike-defended`.
  When ST-T lands each one starts passing, the battery then fails it ("known
  failure now passes") and ST-T removes `known_failure` and tightens the
  numbers below.

`parse_runs` reads the output; the check functions are tested in
tools/test_battery_ai_strike.py.
"""
from __future__ import annotations

import dataclasses
import math
import re

from battery import Scenario, Step
from battery_scenarios.a2g_missiles import STRAIGHT_FLUSH

PENDING = "pending ST-T: the AI does not take a ground target yet (plan section 3.1)"

LINE = re.compile(r"^ai-strike: (\S+)(?: (.*))?$", re.M)
FIELD = re.compile(r'(\w+)=("[^"]*"|\S+)')

# John's release circle (2026-10-11, docs/spec/ai-strike.md#skill-and-release-precision), in
# feet, by skill 0 (Novice), 1 (Average), 2 (Experienced), 3 (Ace): 325, 250, 175 and 100 m.
CIRCLE_FT = {0: 1066.0, 1: 820.0, 2: 574.0, 3: 328.0}
# The bomb's own scatter, on top of the aim offset: the worst of the specced cases is the level
# Mk 82 (about 770 ft across the track); the dive is about 310 ft.
SCATTER_FT = 800.0
# Horizontal miss, in feet, a bomb pass may end within. The seeded aim offset is inside the circle
# 90% of passes and inside the ring R to 2R on the other 10%, and a run does not know which it
# drew, so the limit is the ring's outer edge plus the scatter. ST-T tightens it for runs whose
# reported aim offset is inside the circle (the driver prints no release aim offset yet).
MISS_FT_MAX = {skill: 2 * r + SCATTER_FT for skill, r in CIRCLE_FT.items()}
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
        named = run.first("PROFILE") or {}
        profile = want.get("profile")
        if profile and named.get("name") != profile:
            problems.append(f"{label}: flew PROFILE {named.get('name')}, asked {profile}")
        if not run.of("RELEASE"):
            problems.append(f"{label}: no RELEASE")
        if not run.of("IMPACT"):
            problems.append(f"{label}: no IMPACT")
        limit = want.get("miss")
        for impact in run.of("IMPACT") if limit is not None else []:
            miss = number(impact.get("miss_ft"))
            if not miss <= limit:
                problems.append(f"{label}: impact {impact.get('miss_ft')} ft from the target, want at most {limit:.0f}")
        if limit is not None and run.of("RELEASE") and not run.of("EGRESS"):
            problems.append(f"{label}: no EGRESS after the release")
        if run.of("LOST") and not want.get("may_lose"):
            problems.append(f"{label}: the striker was lost ({run.first('LOST')})")
        if want.get("hit") and not _target_hurt(run):
            problems.append(f"{label}: the target is unhurt ({run.summary.get('target_hp')})")
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


# The plan's bomb coverage: each pass forced once at the middle skill, and every skill on the
# pass the AI would pick (forced dive here as a stand-in until ST-T chooses by threat and energy).
BOMB_RUNS = [(2, p) for p in ("straight", "popup", "fasthigh", "dive", "level")] + [(s, "dive") for s in (0, 1, 3)]


def pilot_run(theater: str, stem: str, *rest: str) -> list[str]:
    return ["--a2g-run", theater, stem, "--pilot", "ai", *rest]


def scenarios() -> list[Scenario]:
    qucol = ["--aircraft", "a4e", "--load", "MK82", "--seconds", "60"]
    smoke = pilot_run("UKR", "QUCOL", *qucol)
    hand = ["--aircraft", "a4e", "--load", "MK82", "--handback", "30", "--mode", "lvl", "--altitude", "5000",
            "--seconds", "80"]
    bombs = ["--aircraft", "a4e", "--load", "MK82", "--seconds", "120"]

    def forced(skill: int, profile: str, *rest: str) -> list[str]:
        return pilot_run("TVIET", "QTTRUCK", *rest, "--skill", str(skill), "--profile", profile)

    bomb_args = [forced(s, p, *bombs) for s, p in BOMB_RUNS]
    rocket_runs = [(2, "straight"), (2, "popup")]
    rockets = ["--aircraft", "a4e", "--load", "LAU61", "--seconds", "120"]
    maverick = ["--aircraft", "a10", "--load", "AGM65G", "--seconds", "120"]
    jdam = ["--aircraft", "f16c", "--load", "GBU30", "--station", "2", "--count", "2", "--seconds", "120"]
    return [
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
            args=bomb_args[0], timeout=900, known_failure=PENDING,
            then=[Step(a, timeout=300) for a in bomb_args[1:]],
            check=lambda out: strike_problems(out, _runs(BOMB_RUNS, hit=False)),
            notes="An A-4E's Mk 82 on the TVIET truck park: each pass (straight, pop-up, fast-high, dive, "
                  "level) forced once at skill 2 and skills 0, 1 and 3 on the dive. Each run names its "
                  "pass, releases, ends the bomb within the skill's miss limit (twice the release circle plus 800 ft of scatter: 2,932, 2,440, 1,948 and 1,456 ft for skills 0 to 3), leaves and is not lost.",
        ),
        Scenario(
            name="ai-strike-rockets", lane="flight",
            args=forced(*rocket_runs[0], *rockets), timeout=600, known_failure=PENDING,
            then=[Step(forced(*r, *rockets), timeout=300) for r in rocket_runs[1:]],
            check=lambda out: strike_problems(out, _runs(rocket_runs, miss=False, hit=True)),
            notes="An A-4E's LAU-61 pods on the truck park: a straight run and a pop-up; the rockets hurt "
                  "the target and the striker leaves.",
        ),
        Scenario(
            name="ai-strike-maverick", lane="flight",
            args=pilot_run("UKR", "QUCOL", *maverick, "--skill", "2", "--profile", "fasthigh"),
            timeout=600, known_failure=PENDING,
            check=lambda out: strike_problems(out, _runs([(2, "fasthigh")], miss=False, hit=True)),
            notes="An A-10's Maverick on the Ukraine armored column, fast-high.",
        ),
        Scenario(
            name="ai-strike-jdam", lane="flight",
            args=pilot_run("TVIET", "QTTRUCK", *jdam, "--skill", "2", "--profile", "level"),
            timeout=600, known_failure=PENDING,
            check=lambda out: strike_problems(out, _runs([(2, "level")], miss=False, hit=True)),
            notes="An F-16C's GBU-30 JDAM on the truck park, level.",
        ),
        Scenario(
            name="ai-sead-harm", lane="flight",
            args=pilot_run("VLA", "QVRDRI", "--surface-seed", "4", "--defenses", "3", "3", "--unit",
                           STRAIGHT_FLUSH, "--aircraft", "f4g", "--load", "AGM88", "--from", "12",
                           "--altitude", "15000", "--seconds", "120", "--skill", "2", "--profile", "fasthigh"),
            timeout=600, known_failure=PENDING,
            check=lambda out: strike_problems(out, _runs([(2, "fasthigh")], miss=False, hit=True)),
            notes="An F-4G's HARM on the heavily defended site's radar, fast-high.",
        ),
        Scenario(
            name="ai-strike-defended", lane="flight",
            args=pilot_run("UKR", "QUCOL", "--aircraft", "f18", "--load", "MK82", "--defenses", "2", "2",
                           "--seconds", "180", "--skill", "2"),
            timeout=600, known_failure=PENDING,
            check=lambda out: strike_problems(out, _runs([(2, None)], hit=False)),
            notes="An F/A-18D's Mk 82 on the defended column: evades the defences, resumes, leaves.",
        ),
    ]
