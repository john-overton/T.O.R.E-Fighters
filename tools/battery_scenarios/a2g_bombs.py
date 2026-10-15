"""Lane: flight. Bomb and rocket motion (docs/formats/weapons.md, "Bomb motion";
docs/spec/air-to-ground.md), through `--a2g-run`'s `release` attack.

`a2g-bomb-motion` drops through the real trigger over the Ukraine armored
column `~QUCOL`:

* an A-4E's Mk 82 from a level pass, 5,000 ft at 450 kt, and an F-4B's Mk 82
  from a 30 degree dive: the sight's solution obeys the bomb equation (fall
  `(vz + sqrt(vz^2 + 64 h)) / 32` seconds, throw the horizontal speed times the
  fall), every bomb bursts on its own path within 2 ft and inside the retail
  scatter's reach of the mean point, and its scatter stays within 1.5 degrees (John halved retail's 3, 2026-10-11);
* an A-4E's Mk 82 AIR 7-pack: one press releases all seven in one tick for one
  round, staggered x 0, -2, +2, -4, +4, -6, +6 ft and y 0, -2, -6, -12, -20,
  -30, -42 ft, at 66 percent of the aircraft's speed;
* an A-4E's Snakeye (Mk 82 HD) from the same level pass: 66 percent of the
  speed and the same fall, so a throw 66 percent of the slick bomb's.

`a2g-rocket-pod` fires an A-4E's two LAU-61 pods: 38 rockets, one a press,
four a second held (a quarter second apart on the game's quarter-second clock), all 38
when held on; and a pair
of LAU-10 pods (8 rockets) that fire once a press however long it is held.
"""
import json
import math
from pathlib import Path
import re

from battery import Scenario, Step

SOLUTION = re.compile(
    r"^a2g-run: solution t=\S+ height (\S+) speed (\S+) pitch (\S+) fall (\S+) throw (\S+) "
    r"mean-impact \S+ \S+ \S+ target-miss (\S+)$",
    re.M,
)
BOMB = re.compile(
    r"^a2g-run: bomb n=(\d+) id=\d+ offset-right (\S+) offset-up (\S+) origin-error (\S+) speed (\S+) "
    r"scatter-heading (\S+) scatter-pitch (\S+)$",
    re.M,
)
IMPACT = re.compile(
    r"^a2g-run: impact n=(\d+) at \S+ \S+ \S+ path-miss (\S+) mean-miss (\S+) bound (\S+) within (\w+)$", re.M
)
FIRED = re.compile(r"^a2g-run: fired t=(\S+) projectiles (\d+) rounds (\d+)->(\d+)$", re.M)
SUMMARY = re.compile(
    r"^a2g-run: trigger presses (\d+) hold (\S+) fired (\d+) bombs (\d+) rounds-start (\d+) rounds-end (\d+)$", re.M
)

PACK_X = [0, -2, 2, -4, 4, -6, 6]
PACK_Y = [0, -2, -6, -12, -20, -30, -42]


def runs(output: str) -> list[str]:
    """The output of each `--a2g-run`, in order."""
    return [p for p in re.split(r"^a2g-run: done$", output, flags=re.M) if "a2g-run:" in p]


def _solution(run: str, label: str, problems: list[str]):
    m = SOLUTION.search(run)
    if m is None:
        problems.append(f"{label}: no solution")
        return None
    height, speed, pitch, fall, throw = (float(m.group(i)) for i in range(1, 6))
    vz = speed * math.sin(math.radians(pitch))
    vh = speed * math.cos(math.radians(pitch))
    expected = (vz + math.sqrt(vz * vz + 64 * height)) / 32
    if abs(fall - expected) > 0.01:
        problems.append(f"{label}: fall {fall} s, the bomb equation gives {expected:.3f} s")
    if abs(throw - vh * expected) > 2:
        problems.append(f"{label}: throw {throw} ft, the bomb equation gives {vh * expected:.1f} ft")
    return dict(height=height, speed=speed, pitch=pitch, fall=fall, throw=throw)


def _bombs(run: str, label: str, count: int, problems: list[str]) -> list[re.Match]:
    bombs = BOMB.findall(run)
    if len(bombs) != count:
        problems.append(f"{label}: {len(bombs)} bombs released, want {count}")
    for b in bombs:
        if abs(float(b[5])) > 1.5005 or abs(float(b[6])) > 1.5005:
            problems.append(f"{label}: bomb {b[0]} scattered {b[5]} {b[6]} degrees, more than 1.5")
        if float(b[3]) > 0.01:
            problems.append(f"{label}: bomb {b[0]} left {b[3]} ft from its station")
    impacts = IMPACT.findall(run)
    if len(impacts) != count:
        problems.append(f"{label}: {len(impacts)} bursts seen, want {count}")
    for i in impacts:
        if float(i[1]) > 2:
            problems.append(f"{label}: bomb {i[0]} burst {i[1]} ft off its own path")
        if i[4] != "yes":
            problems.append(f"{label}: bomb {i[0]} burst {i[2]} ft from the mean point, beyond the scatter's {i[3]} ft")
    if re.search(r"^a2g-run: impact n=\d+ unseen$", run, re.M):
        problems.append(f"{label}: a burst was not seen")
    return bombs


def bomb_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 4:
        return [f"{len(parts)} runs finished, want 4"]
    problems: list[str] = []
    level = _solution(parts[0], "A-4E level Mk 82", problems)
    _bombs(parts[0], "A-4E level Mk 82", 1, problems)
    if not re.search(r"^a2g-run: fired t=\S+ projectiles 1 rounds 4->3$", parts[0], re.M):
        problems.append("A-4E level Mk 82: want one bomb for one round")
    if level and abs(level["height"] - 5000) > 30:
        problems.append(f"A-4E level Mk 82: released {level['height']} ft above the ground, want about 5,000")

    dive = _solution(parts[1], "F-4B dive Mk 82", problems)
    _bombs(parts[1], "F-4B dive Mk 82", 1, problems)
    if dive and abs(dive["pitch"] + 30) > 0.5:
        problems.append(f"F-4B dive Mk 82: released at {dive['pitch']} degrees, want a 30 degree dive")

    pack = _solution(parts[2], "A-4E Mk 82 AIR", problems)
    bombs = _bombs(parts[2], "A-4E Mk 82 AIR", 7, problems)
    if not re.search(r"^a2g-run: fired t=\S+ projectiles 7 rounds 1->0$", parts[2], re.M):
        problems.append("A-4E Mk 82 AIR: want all seven in one tick for one round")
    for b in bombs:
        n = int(b[0])
        if n < 7 and (abs(float(b[1]) - PACK_X[n]) > 0.01 or abs(float(b[2]) - PACK_Y[n]) > 0.01):
            problems.append(f"A-4E Mk 82 AIR: bomb {n} offset {b[1]} {b[2]}, want {PACK_X[n]} {PACK_Y[n]}")
    if pack and level and abs(pack["speed"] - math.floor(level["speed"] * 0.66)) > 1.01:
        problems.append(f"A-4E Mk 82 AIR: released at {pack['speed']} ft/s, want 66 percent of {level['speed']}")

    snakeye = _solution(parts[3], "A-4E Snakeye", problems)
    _bombs(parts[3], "A-4E Snakeye", 1, problems)
    if snakeye and level:
        if abs(snakeye["speed"] - math.floor(level["speed"] * 0.66)) > 1.01:
            problems.append(f"A-4E Snakeye: released at {snakeye['speed']} ft/s, want 66 percent of {level['speed']}")
        if abs(snakeye["fall"] - level["fall"]) > 0.05:
            problems.append(f"A-4E Snakeye: falls {snakeye['fall']} s, the slick bomb {level['fall']} s; want the same")
        ratio = snakeye["throw"] / level["throw"]
        if abs(ratio - snakeye["speed"] / level["speed"]) > 0.01 or ratio > 0.7:
            problems.append(f"A-4E Snakeye: throw {snakeye['throw']} ft is {ratio:.3f} of the slick bomb's, want about 0.66")
    return problems


def _summary(run: str, label: str, problems: list[str]):
    m = SUMMARY.search(run)
    if m is None:
        problems.append(f"{label}: no trigger summary")
        return None
    return dict(fired=int(m.group(3)), start=int(m.group(5)), end=int(m.group(6)))


def pod_problems(output: str) -> list[str]:
    parts = runs(output)
    if len(parts) != 4:
        return [f"{len(parts)} runs finished, want 4"]
    problems: list[str] = []
    taps = _summary(parts[0], "LAU-61 taps", problems)
    if taps and (taps["start"], taps["fired"], taps["end"]) != (38, 3, 35):
        problems.append(f"LAU-61 taps: {taps}, want 38 rockets in two pods, one a press")
    if any(f[1] != "1" for f in FIRED.findall(parts[0])):
        problems.append("LAU-61 taps: a press fired more than one rocket")

    held = _summary(parts[1], "LAU-61 held 1 s", problems)
    times = [float(f[0]) for f in FIRED.findall(parts[1])]
    if held and not 4 <= held["fired"] <= 5:
        problems.append(f"LAU-61 held 1 s: fired {held['fired']}, want 4 a second")
    # The repeat runs on the game's quarter-second clock, so the second rocket
    # comes at the next quarter after the press and the rest a quarter apart.
    gaps = [b - a for a, b in zip(times, times[1:])]
    if not gaps or not 0 < gaps[0] <= 0.26 or any(abs(g - 0.25) > 0.01 for g in gaps[1:]):
        problems.append(f"LAU-61 held 1 s: rockets at {times}, want a quarter second apart")

    empty = _summary(parts[2], "LAU-61 held on", problems)
    if empty and (empty["fired"], empty["end"]) != (38, 0):
        problems.append(f"LAU-61 held on: {empty}, want all 38 and the pods empty")

    lau10 = _summary(parts[3], "LAU-10 held 2 s", problems)
    if lau10 and (lau10["start"], lau10["fired"], lau10["end"]) != (8, 1, 7):
        problems.append(f"LAU-10 held 2 s: {lau10}, want 8 rockets and one fired: no repeat")
    return problems


STRAFE = re.compile(
    r"^a2g-run: strafe gun (\d+)->(\d+) fired (\d+) tracers (\d+) store (\d+)->(\d+) selected (\d+) "
    r"\(key down on (\d+)\) bombs-or-store-rounds-fired (\d+)$",
    re.M,
)


def strafe_problems(output: str) -> list[str]:
    """Tab fires the gun with a bomb selected: rounds leave and the ammunition falls, the
    selection does not move, the bomb stays on its pylon, and the trigger was never pressed."""
    parts = runs(output)
    if len(parts) != 1:
        return [f"{len(parts)} runs finished, want 1"]
    m = STRAFE.search(parts[0])
    if m is None:
        return ["no strafe summary (Tab never went down?)"]
    start, end, fired, tracers, store_start, store_end, selected, down_on, store_fired = map(int, m.groups())
    problems: list[str] = []
    if fired < 20:
        problems.append(f"Tab fired {fired} gun rounds in two seconds, want a burst")
    if start - end != fired:
        problems.append(f"gun ammunition fell {start - end} for {fired} rounds")
    if not fired // 4 <= tracers <= fired // 2 + 1:
        problems.append(f"{tracers} tracers in {fired} rounds, want about one in three")
    if selected != down_on:
        problems.append(f"selection moved from {down_on} to {selected}")
    if store_start != store_end or store_fired:
        problems.append(f"the selected store was used: rounds {store_start}->{store_end}, {store_fired} in the air")
    if "a2g-run: no key" in parts[0]:
        problems.append("the store was never selected")
    return problems


def jettison_recording_problems(work: Path, output: str) -> list[str]:
    rows = [json.loads(line) for line in (work / "log/log.jsonl").read_text().splitlines()]
    sounds = [row.get("fields", {}).get("sound") for row in rows
              if row.get("kind") == "audio.effect"]
    problems = []
    if sounds.count("blast39") != 2:
        problems.append(f"jettison replay has {sounds.count('blast39')} release sounds, expected 2")
    thuds = sum(sound in ("blast15", "blast16") for sound in sounds)
    if thuds != 5:
        problems.append(f"jettison replay has {thuds} thuds, expected 5")
    if any(sound not in ("blast39", "blast15", "blast16") for sound in sounds):
        problems.append(f"unexpected jettison sounds: {sounds}")
    launches = [row for row in rows if row.get("kind") == "weapon.launch"]
    if len(launches) != 5 or any(row.get("fields", {}).get("class") != "other" for row in launches):
        problems.append("jettison replay must contain five inert, non-weapon bodies")
    if "verify-render: PASS ticks=3601 missing=0 differing=0" not in output:
        problems.append("jettison replay scene verification did not pass")
    return problems


def scenarios() -> list[Scenario]:
    run = ["--a2g-run", "UKR", "QUCOL", "--attack", "release", "--from", "1"]
    return [
        Scenario(
            name="a2g-jettison", lane="flight",
            args=["--a2g-run", "UKR", "QUCOL", "--attack", "jettison",
                  "--aircraft", "f16c", "--load", "MK82", "--altitude", "5000",
                  "--speed", "300", "--seconds", "30", "--defenses", "0", "0",
                  "--record", "{work}/jettison.tore-replay", "--verify-render"],
            timeout=300,
            expect=[r"jettison: PASS released [1-9]\d* .* no-explosion"],
            then=[Step(["--recording-info", "{work}/jettison.tore-replay"]),
                  Step(["--recording-log", "{work}/jettison.tore-replay", "--out", "{work}/log", "--rate", "5"])],
            check_work=jettison_recording_problems,
            notes="Tanks then selected bombs at 5,000 ft, inert fall, small craters, recorded sounds and replay picture verification.",
        ),
        Scenario(
            name="a2g-bomb-motion", lane="flight",
            args=run + ["--aircraft", "a4e", "--load", "MK82", "--altitude", "5000", "--speed", "450"],
            timeout=300,
            then=[
                Step(run + ["--aircraft", "f4b", "--load", "MK82", "--altitude", "3000", "--speed", "450",
                            "--dive", "30"], timeout=300),
                Step(run + ["--aircraft", "a4e", "--load", "MK82AIR", "--count", "1", "--altitude", "3000",
                            "--speed", "450"], timeout=300),
                Step(run + ["--aircraft", "a4e", "--load", "MK82HD", "--count", "2", "--altitude", "5000",
                            "--speed", "450"], timeout=300),
            ],
            check=bomb_problems,
            notes="Mk 82 level and dive, the Mk 82 AIR 7-pack and the Snakeye; a few seconds a run in debug.",
        ),
        Scenario(
            name="a2g-rocket-pod", lane="flight",
            args=run + ["--aircraft", "a4e", "--load", "LAU61", "--altitude", "1500", "--dive", "10",
                        "--speed", "400", "--repeat", "3", "--gap", "0.5"],
            timeout=300,
            then=[
                Step(run + ["--aircraft", "a4e", "--load", "LAU61", "--altitude", "1500", "--dive", "10",
                            "--speed", "400", "--hold", "1"], timeout=300),
                Step(run + ["--aircraft", "a4e", "--load", "LAU61", "--altitude", "1500", "--dive", "10",
                            "--speed", "400", "--hold", "12"], timeout=300),
                Step(run + ["--aircraft", "a4e", "--load", "LAU10", "--count", "2", "--altitude", "1500",
                            "--dive", "10", "--speed", "400", "--hold", "2"], timeout=300),
            ],
            check=pod_problems,
            notes="Two LAU-61 pods tapped, held a second and held empty; two LAU-10 pods held.",
        ),
        Scenario(
            name="a2g-rocket-smoke", lane="flight",
            args=run + ["--aircraft", "a4e", "--load", "LAU61", "--altitude", "1500", "--dive", "10",
                        "--speed", "400", "--hold", "1"],
            timeout=300,
            expect=[r"^a2g-run: motors lit [1-9]\d* burnt-out-in-flight \d+ missile-smoke max-puffs [1-9]\d*$"],
            notes="Bug B6 (slice BF): a LAU-61 salvo's rockets, which have no seeker, smoke while their motors burn.",
        ),
        Scenario(
            name="a2g-gun-key", lane="flight",
            args=["--a2g-run", "UKR", "QUCOL", "--attack", "strafe", "--from", "1", "--aircraft", "a4e",
                  "--load", "MK82", "--altitude", "2000", "--dive", "10", "--speed", "400",
                  "--seconds", "20", "--hold", "2"],
            timeout=300,
            check=strafe_problems,
            notes="Tab held two seconds with an Mk 82 selected: the gun fires, the bomb does not (FX-T).",
        ),
    ]
