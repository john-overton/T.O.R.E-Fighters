"""Recorded acceptance cases for the shared fitted bomb model."""
import re

from battery import Scenario


def guided_problems(output):
    problems = []
    if not re.search(r"a2g-run: release t=.* mode Guiding", output):
        problems.append("guided bomb did not leave with guidance")
    measured = re.search(r"flight-measured id=\d+ predicted=([\d.]+) actual=([\d.]+) error-ticks=([\d.-]+) arrival-kt=([\d.]+)", output)
    if measured is None:
        problems.append("missing precise release TTI measurement")
    elif abs(float(measured[3])) > 2:
        problems.append(f"release TTI differs by {measured[3]} ticks, limit 2")
    if "GBU10.JT" in output and measured and float(measured[4]) <= 300:
        problems.append("GBU-10 arrival speed is not clearly above 165 kt")
    impact = re.search(r"a2g-run: impact .*point-miss ([\d.]+)", output)
    if impact is None or float(impact[1]) > 60:
        problems.append("guided impact misses its stored point by more than 60 ft")
    return problems


def scenarios():
    cases = [
        ("gbu12-level", "laser", "f16c", "GBU12", "20000", "0", ["--release", "bar", "--station", "2"]),
        ("gbu38-level", "jdam", "f16c", "GBU38", "12000", "0", ["--release", "bar", "--station", "2"]),
        ("gbu10-level", "laser", "f16c", "GBU10", "20000", "0", ["--release", "bar", "--station", "2"]),
        ("jdam-max", "jdam", "f16c", "GBU30", "12000", "0", ["--release", "max", "--station", "2"]),
        ("mk82-dive", "ccip", "a4e", "MK82", "5000", "30", []),
        ("mk82air-low", "release", "a4e", "MK82AIR", "500", "0", []),
    ]
    return [Scenario(
        name=f"a2g-bomb-flight-{name}", lane="flight",
        args=["--a2g-run", "TVIET", "QTTRUCK", "--attack", attack,
              "--aircraft", aircraft, "--load", store, "--count", "1",
              "--altitude", altitude, "--speed", "450", "--dive", dive,
              "--from", "8" if attack in ("laser", "jdam") else "0.1",
              "--seconds", "150", "--record", "{work}/flight.tore-replay",
              "--verify-render", *extra],
        outputs=["flight.tore-replay"], timeout=300,
        check=guided_problems if attack in ("laser", "jdam") else None,
        forbid=[r"a2g-run: impact .*unseen"],
        expect=[r"a2g-run: impact", r"a2g-run: done",
                *([r"a2g-run: impact .*target-hp 0"] if name in ("gbu12-level", "gbu38-level") else [])],
        notes="Headless recording and CPU replay-transform verification. Flight-model numerical assertions are in tore-sim.",
    ) for name, attack, aircraft, store, altitude, dive, extra in cases]
