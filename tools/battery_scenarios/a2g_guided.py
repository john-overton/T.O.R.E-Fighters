"""Lane: flight. Guided air-to-ground weapons (docs/spec/air-to-ground.md, "Weapon
classes" 4, 5, 7 and 12, "The laser", "Buddy lasing" and "Guided bomb release range
indicator"), through `--a2g-run`'s `laser`, `jdam` and `at2` attacks. Every run
designates through the player's keys (the A-G page and T, else Easy targeting's
Ctrl+X), releases on the HUD's release range cue (IN RNG) and follows the weapon to
its end.

`a2g-laser-self`: an F-16C's Pave Spike lases a T-80 of the Ukraine armored column
`~QUCOL` (the automatic laser with the bomb selected) and a GBU-10 from 15,000 ft at
480 kt guides onto it and destroys it; again with Ctrl+L turning the laser off 20 s
after the release: the bomb goes blind, falls, and the tank survives.

`a2g-laser-buddy`: an Su-25 two miles behind lases by hand (Ctrl+L on its own seat)
and an F/A-18D, which has no designator, drops a GBU-10 that homes on the Su-25's
spot and destroys the tank.

`a2g-jdam-cloud`: in cloudy weather an F-16C's GBU-30 stores the designated object's
position at release (a standing gun of the Vietnam truck park `~QTTRUCK`) and flies
to it: the object is destroyed. The same at a `~QUCOL` tank, which drives on at
50 ft/s: the bomb lands on the stored point and the tank is gone from it.

`a2g-bar`: slice FX-H, the level T for guided bombs. A guided bomb in LVL holds the held
trigger for the T's bar, which stands two fifths in from the reach's far edge; the run
(`--release bar`) turns onto the designated object, holds the trigger from the
designation and leaves the release to combat. An F-16C's GBU-30 from 12,000 ft at 450 kt
(a standing gun of the truck park) and its GBU-10 on its own Pave Spike from 8,000 ft at
400 kt (a tank of the armored column) each release once, with the aim within 60 ft of the
bar, and hit within 60 ft.

`a2g-laser-missile`: an Su-25 lases with its built-in designator and its AS-14 homes
on its own spot and destroys a tank.

`a2g-at2`: a Mi-24's AT-2 is steered at the designated tank while the helicopter
holds it in its 45 degree cone and hits it; again with the helicopter turning away
2 s after the launch: the missile goes blind for good and misses.
"""
import re

from battery import Scenario, Step

RELEASE = re.compile(
    r"^a2g-run: release t=\S+ id=(\d+) kind (\w+) mode (\w+) spot-owner (\S+) height (\S+) speed (\S+) stored .*$",
    re.M,
)
SIGHT = re.compile(
    r"^a2g-run: release-sight t=\S+ kind (\w+) range (\S+) reach (\S+) in-reach (\w+) advisory (\S+) buddy (\w+) laser (\w+)(?: .*)?$",
    re.M,
)
IMPACT = re.compile(
    r"^a2g-run: impact t=\S+ id=(\d+) mode (\w+) at \S+ \S+ \S+ target-miss (\S+) point-miss (\S+) target-hp (-?\d+) flown (\S+)$",
    re.M,
)
MODE = re.compile(r"^a2g-run: mode t=\S+ id=\d+ (\w+) -> (\w+)$", re.M)
TARGET = re.compile(r"^a2g-run: target 0x[0-9a-f]+ hp (\d+)/(\d+)$", re.M)
KILL = re.compile(r"^a2g-run: kill 0x[0-9a-f]+ class \S+$", re.M)
FLIGHT = re.compile(r"^a2g-run: flight t=\S+ id=\d+ mode (\w+) height \S+ speed (\S+) tti (\S+)$", re.M)


def runs(output: str) -> list[str]:
    """The output of each `--a2g-run`, in order."""
    return [p for p in re.split(r"^a2g-run: done$", output, flags=re.M) if "a2g-run:" in p]


def _one(run: str, label: str, problems: list[str]):
    """The run's release, its sight at the release, its end and the target."""
    release = RELEASE.search(run)
    sight = SIGHT.search(run)
    impact = IMPACT.search(run)
    target = TARGET.search(run)
    if release is None:
        problems.append(f"{label}: nothing released")
    if sight is None:
        problems.append(f"{label}: no release sight")
    elif sight.group(4) != "yes":
        problems.append(f"{label}: released outside the reach ({sight.group(0)})")
    if impact is None:
        problems.append(f"{label}: the weapon's end was not seen")
    if target is None:
        problems.append(f"{label}: no target line")
    return release, sight, impact, target


def _killed(run: str, target) -> bool:
    return target is not None and int(target.group(1)) == 0 and KILL.search(run) is not None


def _countdown(run: str, label: str, problems: list[str]):
    """The time to impact counts down while the weapon guides."""
    tti = [float(m[2]) for m in FLIGHT.findall(run) if m[0] == "Guiding" and m[2] != "none"]
    if len(tti) < 3:
        problems.append(f"{label}: fewer than three guiding time-to-impact readings")
    elif any(b >= a for a, b in zip(tti, tti[1:])):
        problems.append(f"{label}: the time to impact did not count down: {tti}")


def laser_self_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 2:
        return [f"{len(parts)} runs finished, want 2"]
    problems: list[str] = []
    label = "F-16C GBU-10 own spot"
    release, sight, impact, target = _one(parts[0], label, problems)
    if release and (release.group(2), release.group(3), release.group(4)) != ("Laser", "Guiding", "0"):
        problems.append(f"{label}: released {release.group(0)}, want a laser bomb guiding on aircraft 0's spot")
    if release and abs(float(release.group(5)) - 15000) > 50:
        problems.append(f"{label}: released {release.group(5)} ft above the target, want 15,000")
    # Bombs inherit the full aircraft velocity: 480 kt, about 810 ft/s.
    if release and abs(float(release.group(6)) - 810.144) > 5:
        problems.append(f"{label}: left at {release.group(6)} ft/s, want the aircraft's 810.144")
    if sight and (sight.group(7), sight.group(6)) != ("Lasing", "no"):
        problems.append(f"{label}: the laser was {sight.group(7)} (buddy {sight.group(6)}) at release")
    if impact and (impact.group(2) != "Guiding" or float(impact.group(3)) > 40):
        problems.append(f"{label}: ended {impact.group(0)}, want guiding onto the tank")
    if not _killed(parts[0], target):
        problems.append(f"{label}: the tank was not destroyed")
    _countdown(parts[0], label, problems)

    label = "F-16C GBU-10, laser off"
    release, sight, impact, target = _one(parts[1], label, problems)
    if ("Guiding", "Blind") not in MODE.findall(parts[1]):
        problems.append(f"{label}: the bomb never went from guiding to blind")
    if not re.search(r"^a2g-run: laser-off t=", parts[1], re.M):
        problems.append(f"{label}: Ctrl+L was not pressed")
    if impact and (impact.group(2) != "Blind" or float(impact.group(3)) < 200):
        problems.append(f"{label}: ended {impact.group(0)}, want a blind fall that misses")
    if target and int(target.group(1)) != int(target.group(2)):
        problems.append(f"{label}: the tank was hurt by a blind bomb")
    return problems


def buddy_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 1:
        return [f"{len(parts)} runs finished, want 1"]
    problems: list[str] = []
    label = "F/A-18D GBU-10 on an Su-25's spot"
    run = parts[0]
    if not re.search(r"^a2g-run: buddy-lasing t=", run, re.M):
        problems.append(f"{label}: the Su-25 never lased")
    release, sight, impact, target = _one(run, label, problems)
    if release and (release.group(3), release.group(4)) != ("Guiding", "1"):
        problems.append(f"{label}: released {release.group(0)}, want guiding on aircraft 1's spot")
    if sight and (sight.group(6), sight.group(7)) != ("yes", "Off"):
        problems.append(f"{label}: at release the spot was buddy {sight.group(6)}, own laser {sight.group(7)}")
    if impact and float(impact.group(3)) > 40:
        problems.append(f"{label}: came down {impact.group(3)} ft from the tank")
    if not _killed(run, target):
        problems.append(f"{label}: the tank was not destroyed")
    return problems


def jdam_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 2:
        return [f"{len(parts)} runs finished, want 2"]
    problems: list[str] = []
    label = "GBU-30 in cloud on a standing object"
    release, sight, impact, target = _one(parts[0], label, problems)
    if release and (release.group(2), release.group(3)) != ("Coordinate", "Guiding"):
        problems.append(f"{label}: released {release.group(0)}, want a JDAM flying to its point")
    if impact and float(impact.group(4)) > 40:
        problems.append(f"{label}: came down {impact.group(4)} ft from the stored point")
    if not _killed(parts[0], target):
        problems.append(f"{label}: the object was not destroyed")
    _countdown(parts[0], label, problems)

    label = "GBU-30 at a moving tank"
    release, sight, impact, target = _one(parts[1], label, problems)
    if impact:
        if float(impact.group(4)) > 40:
            problems.append(f"{label}: came down {impact.group(4)} ft from the stored point")
        if float(impact.group(3)) < 500:
            problems.append(f"{label}: came down {impact.group(3)} ft from the moving tank; want it missed")
    if target and int(target.group(1)) != int(target.group(2)):
        problems.append(f"{label}: the moving tank was hit")
    return problems


BAR_SIGHT = re.compile(
    r"^a2g-run: release-sight t=\S+ kind (\w+) .* in-reach (\w+) .* bar (\S+) along (\S+) seconds \S+ steer (\S+)$",
    re.M,
)


def bar_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 2:
        return [f"{len(parts)} runs finished, want 2"]
    problems: list[str] = []
    for label, run, kind in (
        ("GBU-30 at the bar", parts[0], "Coordinate"),
        ("GBU-10 at the bar", parts[1], "Laser"),
    ):
        release, _, impact, target = _one(run, label, problems)
        if len(RELEASE.findall(run)) != 1:
            problems.append(f"{label}: {len(RELEASE.findall(run))} releases, want one")
        if release and (release.group(2), release.group(3)) != (kind, "Guiding"):
            problems.append(f"{label}: released {release.group(0)}")
        bar = BAR_SIGHT.search(run)
        if bar is None:
            problems.append(f"{label}: no release sight with the T")
        elif bar.group(4) in ("none", "") or not (-60 <= float(bar.group(4)) <= 60):
            problems.append(f"{label}: released {bar.group(4)} ft from the bar")
        elif abs(float(bar.group(5))) > 10:
            problems.append(f"{label}: released {bar.group(5)} degrees off the stem")
        if impact and float(impact.group(3)) > 60:
            problems.append(f"{label}: came down {impact.group(3)} ft from the target")
        if not _killed(run, target):
            problems.append(f"{label}: the target was not destroyed")
        _countdown(run, label, problems)
    return problems


def missile_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 1:
        return [f"{len(parts)} runs finished, want 1"]
    problems: list[str] = []
    label = "Su-25 AS-14 on its own spot"
    release, sight, impact, target = _one(parts[0], label, problems)
    if release and (release.group(2), release.group(3), release.group(4)) != ("Laser", "Guiding", "0"):
        problems.append(f"{label}: released {release.group(0)}, want guiding on its own spot")
    if impact and (impact.group(2) != "Guiding" or float(impact.group(3)) > 60):
        problems.append(f"{label}: ended {impact.group(0)}")
    if not _killed(parts[0], target):
        problems.append(f"{label}: the tank was not destroyed")
    return problems


def at2_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 2:
        return [f"{len(parts)} runs finished, want 2"]
    problems: list[str] = []
    label = "Mi-24 AT-2 held"
    release, sight, impact, target = _one(parts[0], label, problems)
    if release and (release.group(2), release.group(3)) != ("Command", "Guiding"):
        problems.append(f"{label}: released {release.group(0)}, want command guidance")
    if impact and (impact.group(2) != "Guiding" or float(impact.group(3)) > 60):
        problems.append(f"{label}: ended {impact.group(0)}, want guiding onto the tank")
    if target and int(target.group(1)) >= int(target.group(2)):
        problems.append(f"{label}: the tank was not hit")

    label = "Mi-24 AT-2, turning away"
    release, sight, impact, target = _one(parts[1], label, problems)
    if not re.search(r"^a2g-run: turn-away t=", parts[1], re.M):
        problems.append(f"{label}: the helicopter did not turn")
    if ("Guiding", "Blind") not in MODE.findall(parts[1]):
        problems.append(f"{label}: the missile never lost guidance")
    if ("Blind", "Guiding") in MODE.findall(parts[1]):
        problems.append(f"{label}: a lost missile guided again")
    if impact and float(impact.group(3)) < 200:
        problems.append(f"{label}: came down {impact.group(3)} ft from the tank")
    if target and int(target.group(1)) != int(target.group(2)):
        problems.append(f"{label}: the tank was hurt")
    return problems


def scenarios() -> list[Scenario]:
    def run(theater: str, stem: str, attack: str, *rest: str) -> list[str]:
        return ["--a2g-run", theater, stem, "--attack", attack, *rest]

    gbu10 = ["--load", "GBU10", "--station", "2", "--count", "2"]
    laser = run("UKR", "QUCOL", "laser", "--aircraft", "f16c", *gbu10, "--altitude", "15000",
                "--speed", "480", "--from", "6", "--seconds", "120")
    jdam = ["--attack", "jdam", "--aircraft", "f16c", "--load", "GBU30", "--station", "2", "--count", "2",
            "--altitude", "12000", "--speed", "450", "--from", "6", "--seconds", "120", "--condition", "cloudy"]
    at2 = run("UKR", "QUCOL", "at2", "--aircraft", "mi24", "--load", "AT2", "--altitude", "300",
              "--speed", "60", "--from", "2", "--seconds", "60")
    return [
        Scenario(
            name="a2g-laser-self", lane="flight", args=laser, timeout=300,
            then=[Step(laser + ["--laser-off", "10"], timeout=300)],
            check=laser_self_problems,
            notes="F-16C Pave Spike and a GBU-10 from 15,000 ft; again with the laser off mid-fall.",
        ),
        Scenario(
            name="a2g-laser-buddy", lane="flight",
            args=run("UKR", "QUCOL", "laser", "--aircraft", "f18", *gbu10, "--altitude", "12000",
                     "--speed", "450", "--from", "6", "--seconds", "120", "--buddy", "su25"),
            timeout=300,
            check=buddy_problems,
            notes="Two seats: an Su-25 lases, an F/A-18D with no designator drops a GBU-10 on the spot.",
        ),
        Scenario(
            name="a2g-jdam-cloud", lane="flight",
            args=["--a2g-run", "TVIET", "QTTRUCK", *jdam], timeout=300,
            then=[Step(["--a2g-run", "UKR", "QUCOL", *jdam], timeout=300)],
            check=jdam_problems,
            notes="GBU-30 in cloud on a standing gun, then at a tank that drives on.",
        ),
        Scenario(
            name="a2g-bar", lane="flight",
            args=["--a2g-run", "TVIET", "QTTRUCK", "--attack", "jdam", "--aircraft", "f16c", "--load", "GBU30",
                  "--station", "2", "--count", "2", "--altitude", "12000", "--speed", "450", "--from", "12",
                  "--seconds", "150", "--condition", "clear", "--release", "bar"],
            timeout=300,
            then=[Step(["--a2g-run", "UKR", "QUCOL", "--attack", "laser", "--aircraft", "f16c", "--load", "GBU10",
                        "--station", "2", "--count", "2", "--altitude", "8000", "--speed", "400", "--from", "9",
                        "--seconds", "150", "--release", "bar"], timeout=300)],
            check=bar_problems,
            notes="Guided bombs in LVL: a held trigger releases at the T's bar (JDAM, then Pave Spike laser bomb).",
        ),
        Scenario(
            name="a2g-laser-missile", lane="flight",
            args=run("UKR", "QUCOL", "laser", "--aircraft", "su25", "--load", "AS14", "--station", "2",
                     "--count", "2", "--altitude", "3000", "--speed", "400", "--from", "5", "--seconds", "90"),
            timeout=300,
            check=missile_problems,
            notes="Su-25 built-in laser and an AS-14 on its own spot.",
        ),
        Scenario(
            name="a2g-at2", lane="flight", args=at2, timeout=300,
            then=[Step(at2 + ["--turn-away", "2"], timeout=300)],
            check=at2_problems,
            notes="Mi-24 AT-2 command guidance; again turning away 2 s after the launch.",
        ),
    ]
