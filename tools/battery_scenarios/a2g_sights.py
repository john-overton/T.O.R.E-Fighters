"""Lane: flight. The HUD's bomb and rocket sights (docs/spec/air-to-ground.md,
"The HUD in air-to-ground modes"), flown through `--a2g-run`'s `ccip`, `level`
and `rocket` attacks. Every release goes through the real trigger, decided by
the sight combat works out for the HUD.

`a2g-ccip-dive`: an F/A-18D's Mk 82 in a 30 degree dive on a truck of the
TVIET truck park, released when the CCIP pipper covers it: the truck is
destroyed. Every logged sight obeys the retail arc rule (glyph 0x2f inside 0.4
of the 20,000 ft zone-1 range, one step fewer each fourteenth of the rest, the
bare ring 0x20 from the maximum on; the range is retail's approximate
distance) and the arc never shrinks as the range closes. A second, longer and
shallower pass logs the arc closing from far out.

`a2g-level-t`: an F-4B's Mk 82 from a level pass at 5,000 ft and 450 kt over
the Ukraine column. The A-G page designates the target, the bomb starts in LVL
on selection, and the trigger is held from then on: exactly one bomb leaves,
on the tick the impact point reaches the target (within three ticks of
ground travel past it) with the cross-track miss inside the record's radius,
it bursts within the splash radius of the target, and OVERFLY shows after.

`a2g-rocket-ccip`: an A-4E's LAU-61 rockets in a 10 degree dive on a truck,
fired once the rocket pipper says IN RNG: they hit and destroy it, and a
rocket that bursts on the ground does so on its pipper.

`a2g-warnings`: an A-4E's Mk 82 released from a 30 degree dive at 300 ft
(LOW: it falls in under the fuze's one second, so it lands a dud, a puff of
dirt that destroys nothing; FRAG), from a level pass at 400 ft (FRAG only:
the aircraft holding its height is inside the 500 ft splash; the bomb bursts)
and at 1,500 ft (neither).
"""
import math
import re

from battery import Scenario, Step

SIGHT = re.compile(
    r"^a2g-run: (sight|release) t=(\S+) mode (\S+) rocket (\w+) arc (\d+) glyph 0x([0-9a-f]+) range (\S+) "
    r"max (\S+) in-range (\w+) slant \S+ fall (\S+) impact \S+ \S+ \S+ target-gap \S+ warnings (\S+)$",
    re.M,
)
IMPACT = re.compile(r"^a2g-run: impact n=(\d+) at \S+ \S+ \S+ (\w+) target-miss (\S+) pipper-miss (\S+)$", re.M)
LEVEL_RELEASE = re.compile(
    r"^a2g-run: level-release t=(\S+) along (\S+) cross (\S+) rel \S+ radius (\S+) rounds (\d+) count (\d+)$", re.M
)
LEVEL = re.compile(r"^a2g-run: level t=\S+ along (\S+) cross \S+ rel (\S+) steer \S+ marker (\S+) far (\w+)", re.M)
KNOT = 1.6878


def runs(output: str) -> list[str]:
    return [p for p in re.split(r"^a2g-run: done$", output, flags=re.M) if "a2g-run:" in p]


def arc_rule(distance: float, maximum: float) -> int:
    """The retail glyph rule (docs/formats/weapons.md, "Bomb sight")."""
    if maximum <= 0 or distance >= maximum:
        return 0
    near = 0.4 * maximum
    if distance <= near:
        return 15
    return max(1, min(14, 14 - math.floor(14 * (distance - near) / (maximum - near))))


def _sights(run: str, label: str, problems: list[str]) -> list[dict]:
    sights = []
    for m in SIGHT.finditer(run):
        kind, t, mode, rocket, arc, glyph, rng, mx, inr, fall, warnings = m.groups()
        s = dict(kind=kind, t=float(t), mode=mode, rocket=rocket == "yes", arc=int(arc), glyph=int(glyph, 16),
                 range=float(rng), max=float(mx), in_range=inr == "yes", fall=float(fall),
                 warnings=[] if warnings == "none" else warnings.split(","))
        want = arc_rule(s["range"], s["max"])
        if s["arc"] != want:
            problems.append(f"{label}: arc {s['arc']} at {s['range']:.0f} of {s['max']:.0f} ft, the rule gives {want}")
        if s["glyph"] != 0x20 + s["arc"]:
            problems.append(f"{label}: glyph {s['glyph']:#x} for arc {s['arc']}")
        if s["in_range"] != (s["range"] < s["max"]):
            problems.append(f"{label}: IN RNG {s['in_range']} at {s['range']:.0f} of {s['max']:.0f} ft")
        sights.append(s)
    for a, b in zip(sights, sights[1:]):
        if b["range"] < a["range"] and b["arc"] < a["arc"]:
            problems.append(f"{label}: the arc shrank from {a['arc']} to {b['arc']} as the range closed")
    if not sights:
        problems.append(f"{label}: no sight logged")
    return sights


def _release(sights: list[dict], label: str, problems: list[str]):
    released = [s for s in sights if s["kind"] == "release"]
    if len(released) != 1:
        problems.append(f"{label}: {len(released)} releases, want 1")
        return None
    return released[0]


def _killed(run: str) -> bool:
    m = re.search(r"^a2g-run: target (0x[0-9a-f]+) hp (-?\d+)/\d+$", run, re.M)
    return bool(m) and int(m.group(2)) <= 0 and re.search(rf"^a2g-run: kill {m.group(1)} ", run, re.M) is not None


def ccip_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 2:
        return [f"{len(parts)} runs finished, want 2"]
    problems: list[str] = []
    label = "F/A-18D Mk 82 30 degree dive"
    sights = _sights(parts[0], label, problems)
    release = _release(sights, label, problems)
    if release:
        if release["mode"] != "CCIP" or not release["in_range"]:
            problems.append(f"{label}: released in {release['mode']}, in range {release['in_range']}")
        if release["warnings"]:
            problems.append(f"{label}: released with {release['warnings']}")
    if not _killed(parts[0]):
        problems.append(f"{label}: the truck was not destroyed")
    far = _sights(parts[1], "F/A-18D Mk 82 from far", problems)
    arcs = {s["arc"] for s in far}
    if len(arcs) < 5:
        problems.append(f"F/A-18D Mk 82 from far: arcs {sorted(arcs)}, want the arc seen closing step by step")
    return problems


def level_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 1:
        return [f"{len(parts)} runs finished, want 1"]
    run = parts[0]
    problems: list[str] = []
    if not re.search(r"^a2g-run: designated 0x[0-9a-f]+ ", run, re.M):
        problems.append("no designation from the A-G page")
    if not re.search(r"^a2g-run: mode LVL on selection$", run, re.M):
        problems.append("the bomb did not start in LVL")
    levels = LEVEL.findall(run)
    if not levels:
        problems.append("no level solution logged")
    # The release cue closes: time to release falls and the marker rides down.
    before = [lv for lv in levels if float(lv[0]) > 0]
    for a, b in zip(before, before[1:]):
        if float(b[1]) >= float(a[1]) or float(b[2]) > float(a[2]):
            problems.append(f"the cue did not close: {a} then {b}")
            break
    releases = LEVEL_RELEASE.findall(run)
    if len(releases) != 1 or releases[0][5] != "1":
        problems.append(f"{len(releases)} releases {releases}, want one bomb")
    else:
        _, along, cross, radius, _, _ = releases[0]
        tick = 450 * KNOT / 120
        if not (-3 * tick < float(along) <= 0):
            problems.append(f"released {along} ft along, want within three ticks past the target")
        if abs(float(cross)) > float(radius):
            problems.append(f"released {cross} ft across, outside the {radius} ft radius")
    if not re.search(r"^a2g-run: trigger held released 1 ", run, re.M):
        problems.append("the held trigger did not release exactly once")
    if not re.search(r"^a2g-run: overfly t=", run, re.M):
        problems.append("no OVERFLY after the release")
    impacts = IMPACT.findall(run)
    m = re.search(r"^a2g-run: record \S+ fuze-arm \S+ s fuze-radius \S+ collateral (\d+)", run, re.M)
    splash = float(m.group(1)) if m else 0
    if len(impacts) != 1:
        problems.append(f"{len(impacts)} bursts seen, want 1")
    elif float(impacts[0][2]) > splash:
        problems.append(f"burst {impacts[0][2]} ft from the target, outside the {splash:.0f} ft splash")
    return problems


def rocket_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 1:
        return [f"{len(parts)} runs finished, want 1"]
    run = parts[0]
    problems: list[str] = []
    label = "A-4E LAU-61 10 degree dive"
    sights = _sights(run, label, problems)
    release = _release(sights, label, problems)
    if release and (not release["rocket"] or release["mode"] != "CCIP" or not release["in_range"]):
        problems.append(f"{label}: released {release}")
    impacts = IMPACT.findall(run)
    if not any(i[1] in ("hit", "destroyed") for i in impacts):
        problems.append(f"{label}: no rocket hit the truck: {impacts}")
    for n, kind, _, pipper in impacts:
        if kind == "ground" and pipper != "-" and float(pipper) > 1:
            problems.append(f"{label}: rocket {n} burst {pipper} ft from its pipper")
    if not _killed(run):
        problems.append(f"{label}: the truck was not destroyed")
    return problems


def warning_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 3:
        return [f"{len(parts)} runs finished, want 3"]
    problems: list[str] = []
    for part, label, want in zip(
        parts,
        ["300 ft 30 degree dive", "400 ft level", "1,500 ft level"],
        [["LOW", "FRAG"], ["FRAG"], []],
    ):
        sights = _sights(part, label, problems)
        release = _release(sights, label, problems)
        if release and release["warnings"] != want:
            problems.append(f"{label}: released with {release['warnings'] or 'none'}, want {want or 'none'}")
        if release and "LOW" in want and release["fall"] >= 1:
            problems.append(f"{label}: LOW with a {release['fall']} s fall, the fuze arms in 1 s")
        # LOW means a dud: the bomb lands before its fuze arms and does not go
        # off; an armed bomb bursts.
        kinds = [i[1] for i in IMPACT.findall(part)]
        if len(kinds) != 1:
            problems.append(f"{label}: {len(kinds)} bursts seen, want 1")
        elif ("LOW" in want) != (kinds[0] == "dud"):
            problems.append(f"{label}: the bomb landed as '{kinds[0]}' with warnings {want or 'none'}")
        if "LOW" in want and (_killed(part) or re.search(r"^a2g-run: kill ", part, re.M)):
            problems.append(f"{label}: a dud destroyed something")
    return problems


def scenarios() -> list[Scenario]:
    run = ["--a2g-run"]
    truck = ["TVIET", "QTTRUCK", "--target", "2"]
    return [
        Scenario(
            name="a2g-ccip-dive", lane="flight",
            args=run + truck + ["--aircraft", "f18", "--load", "MK82", "--station", "3", "--count", "4",
                                "--attack", "ccip", "--dive", "30", "--altitude", "2000", "--speed", "450",
                                "--from", "1"],
            timeout=300,
            then=[Step(run + truck + ["--aircraft", "f18", "--load", "MK82", "--station", "3", "--count", "4",
                                      "--attack", "ccip", "--dive", "20", "--altitude", "5000", "--speed", "450",
                                      "--from", "3"], timeout=300)],
            check=ccip_problems,
            notes="CCIP dive kill and the arc rule; the second pass logs the arc closing from far out.",
        ),
        Scenario(
            name="a2g-level-t", lane="flight",
            args=run + ["UKR", "QUCOL", "--aircraft", "f4b", "--load", "MK82", "--attack", "level",
                        "--altitude", "5000", "--speed", "450", "--from", "2"],
            timeout=300,
            check=level_problems,
            notes="A-G designation, LVL on selection, pickle and hold: one release at the cue, OVERFLY after.",
        ),
        Scenario(
            name="a2g-rocket-ccip", lane="flight",
            args=run + truck + ["--aircraft", "a4e", "--load", "LAU61", "--attack", "rocket", "--altitude", "1500",
                                "--dive", "10", "--speed", "400", "--from", "1", "--hold", "1"],
            timeout=300,
            check=rocket_problems,
            notes="Rocket pipper on a truck: hits and a kill; ground bursts on the pipper.",
        ),
        Scenario(
            name="a2g-warnings", lane="flight",
            args=run + truck + ["--aircraft", "a4e", "--load", "MK82", "--attack", "ccip", "--dive", "30",
                                "--altitude", "300", "--speed", "450", "--from", "1"],
            timeout=300,
            then=[
                Step(run + truck + ["--aircraft", "a4e", "--load", "MK82", "--attack", "ccip", "--dive", "0",
                                    "--altitude", "400", "--speed", "450", "--from", "1"], timeout=300),
                Step(run + truck + ["--aircraft", "a4e", "--load", "MK82", "--attack", "ccip", "--dive", "0",
                                    "--altitude", "1500", "--speed", "450", "--from", "1"], timeout=300),
            ],
            check=warning_problems,
            notes="LOW and FRAG from a low dive, FRAG from a low level pass, neither from 1,500 ft.",
        ),
    ]
