"""File checkers for the replay lane: Tacview text, log.jsonl and summary text.

These read what `--recording-acmi` and `--recording-log` wrote and return a list
of problems (empty when the file is sane). They need only the standard library.
"""
from __future__ import annotations

import json
import math
import re
from pathlib import Path

OBJ = re.compile(r"^(-?)([0-9a-f]+)(?:,(.*))?$")


def _split_props(rest: str) -> list[str]:
    """Split on commas that are not escaped with a backslash."""
    return re.split(r"(?<!\\),", rest)


def check_acmi(text: str, *, need_objects: bool = True) -> list[str]:
    problems: list[str] = []
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    if len(lines) < 3 or lines[0] != "FileType=text/acmi/tacview" or not lines[1].startswith("FileVersion="):
        return ["ACMI header missing or wrong"]
    seen_time = False
    now = 0.0
    alive: dict[str, float] = {}
    ended: dict[str, float] = {}
    created: dict[str, float] = {}
    names: dict[str, str] = {}
    last_time = 0.0
    for n, line in enumerate(lines[2:], start=3):
        if not line:
            continue
        if re.search(r"\bnan\b|\binf\b", line, re.I) and "Event=" not in line:
            problems.append(f"line {n}: NaN or infinity: {line[:120]}")
            continue
        if line.startswith("#"):
            try:
                t = float(line[1:])
            except ValueError:
                problems.append(f"line {n}: bad time {line!r}")
                continue
            if not math.isfinite(t) or t < now - 1e-9:
                problems.append(f"line {n}: time went backwards or is not finite ({now} -> {line})")
            now = max(now, t)
            last_time = now
            seen_time = True
            continue
        m = OBJ.match(line)
        if not m:
            problems.append(f"line {n}: unparseable line {line[:120]!r}")
            continue
        removal, oid, rest = m.groups()
        if removal:
            if oid not in alive:
                problems.append(f"line {n}: removal of {oid}, which is not alive")
            else:
                ended[oid] = now
                del alive[oid]
            continue
        if oid == "0":
            # Global object: properties and events only.
            continue
        if not seen_time:
            # objects before the first time marker are not expected
            problems.append(f"line {n}: object {oid} before the first time marker")
        props = _split_props(rest or "")
        if oid in ended:
            problems.append(f"line {n}: object {oid} reappears after it was removed at t={ended[oid]}")
        if oid not in alive and oid not in ended:
            created[oid] = now
        alive[oid] = now
        for p in props:
            if p.startswith("T="):
                fields = p[2:].split("|")
                for f in fields:
                    if f == "":
                        continue
                    try:
                        v = float(f)
                    except ValueError:
                        problems.append(f"line {n}: object {oid} has non-numeric transform field {f!r}")
                        break
                    if not math.isfinite(v) or abs(v) > 1e9:
                        problems.append(f"line {n}: object {oid} has a wild transform value {v}")
                        break
                if len(fields) >= 3 and fields[2] != "":
                    try:
                        alt = float(fields[2])
                        if alt < -2000 or alt > 100000:
                            problems.append(f"line {n}: object {oid} altitude {alt} m is outside any sane range")
                    except ValueError:
                        pass
            elif p.startswith("Name="):
                names[oid] = p[5:]
    for oid, t0 in created.items():
        end = ended.get(oid, last_time)
        if end < t0:
            problems.append(f"object {oid} ends before it starts")
    if need_objects and not created:
        problems.append("ACMI has no objects")
    if need_objects and last_time <= 0:
        problems.append("ACMI has no time span")
    return problems


def check_jsonl(text: str) -> list[str]:
    problems: list[str] = []
    kinds: dict[str, int] = {}
    last_t = None
    ids: set[int] = set()
    count = 0
    header = footer = 0
    for n, line in enumerate(text.splitlines(), start=1):
        if not line.strip():
            problems.append(f"line {n}: blank line in log.jsonl")
            continue
        try:
            d = json.loads(line, parse_constant=lambda c: (_ for _ in ()).throw(ValueError(c)))
        except ValueError as e:
            problems.append(f"line {n}: invalid JSON ({str(e)[:60]})")
            continue
        count += 1
        kind = d.get("type")
        kinds[kind] = kinds.get(kind, 0) + 1
        if kind == "header":
            header += 1
            if count != 1:
                problems.append(f"line {n}: header is not the first line")
        if kind == "footer":
            footer += 1
        if kind == "aircraft":
            if d["id"] in ids:
                problems.append(f"line {n}: aircraft id {d['id']} listed twice")
            ids.add(d["id"])
        _walk_numbers(d, n, problems)
        if kind in ("event", "sample"):
            t = d.get("t", d.get("time"))
            if isinstance(t, (int, float)):
                if last_t is not None and t + 1e-9 < last_t:
                    problems.append(f"line {n}: time went backwards ({last_t} -> {t})")
                last_t = t
        if len(problems) > 25:
            problems.append("... more problems suppressed")
            break
    if header != 1:
        problems.append(f"expected one header line, found {header}")
    if footer > 1:
        problems.append(f"{footer} footer lines")
    if not kinds.get("aircraft"):
        problems.append("no aircraft lines")
    return problems


def _walk_numbers(v, n: int, problems: list[str]) -> None:
    if isinstance(v, float) and not math.isfinite(v):
        problems.append(f"line {n}: non-finite number")
    elif isinstance(v, dict):
        for x in v.values():
            _walk_numbers(x, n, problems)
    elif isinstance(v, list):
        for x in v:
            _walk_numbers(x, n, problems)


def info_vs_log(info: str, log: str) -> list[str]:
    """`--recording-info` counts events by kind; log.jsonl holds every event, so the counts must agree."""
    counted: dict[str, int] = {}
    for line in info.splitlines():
        m = re.match(r"^\s+(\d+) ([a-z_]+\.[a-z_]+)$", line)
        if m:
            counted[m.group(2)] = int(m.group(1))
    total = re.search(r"^Events\s+(\d+) in all", info, re.M)
    logged: dict[str, int] = {}
    for line in log.splitlines():
        d = json.loads(line)
        if d["type"] == "event":
            logged[d["kind"]] = logged.get(d["kind"], 0) + 1
    problems = []
    if total and int(total.group(1)) != sum(logged.values()):
        problems.append(f"info counts {total.group(1)} events, log.jsonl holds {sum(logged.values())}")
    for kind in sorted(set(counted) | set(logged)):
        if counted.get(kind, 0) != logged.get(kind, 0):
            problems.append(f"{kind}: info says {counted.get(kind, 0)}, log.jsonl holds {logged.get(kind, 0)}")
    return problems[:8]


def acmi_vs_log(acmi: str, log: str) -> list[str]:
    """Where the Tacview file and the log both hold an aircraft at one moment, the positions agree."""
    samples: dict[tuple[int, float], list[float]] = {}
    for line in log.splitlines():
        d = json.loads(line)
        if d["type"] == "sample":
            samples[(d["id"], round(d["t"], 3))] = d["pos_ft"]
    problems: list[str] = []
    now = 0.0
    compared = 0
    for line in acmi.split("\n"):
        if line.startswith("#"):
            try:
                now = float(line[1:])
            except ValueError:
                pass
            continue
        m = re.match(r"^(1[0-9a-f]{10}),T=([^,]*)", line)
        if not m:
            continue
        fields = m.group(2).split("|")
        pos = samples.get((int(m.group(1), 16) - 0x10000000000, round(now, 3)))
        if pos is None or len(fields) < 9:
            continue
        compared += 1
        for index, feet, name in ((2, pos[1], "altitude"), (6, pos[0], "east"), (7, pos[2], "north")):
            if fields[index] and abs(float(fields[index]) - feet * 0.3048) > 1.0:
                problems.append(f"Tacview {name} of object {m.group(1)} at {now}s is {fields[index]} m, the log says {feet * 0.3048:.1f} m")
        if len(problems) > 5:
            break
    if not compared:
        problems.append("no moment was in both the Tacview file and the log to compare")
    return problems


def jsonl_time_key(text: str) -> str:
    """Which key holds time in event and sample lines (for the doc and the tests)."""
    for line in text.splitlines():
        d = json.loads(line)
        if d.get("type") == "sample":
            return "t" if "t" in d else "time"
    return "t"


def check_summary(text: str, *, expect_aircraft: int | None = None) -> list[str]:
    problems: list[str] = []
    if not text.startswith("T.O.R.E mission summary"):
        problems.append("summary.txt does not start with the title")
    for section in ("Aircraft", "Shots", "Timeline"):
        if not re.search(rf"^{section}\n-+$", text, re.M):
            if section == "Shots" and "Shots\n" not in text:
                continue
            problems.append(f"summary.txt has no {section} section")
    if re.search(r"\bnan\b|\binf\b", text, re.I):
        problems.append("summary.txt contains NaN or infinity")
    if expect_aircraft is not None:
        got = len(re.findall(r"^\S.* \(.*, [A-Za-z0-9.]+;", text, re.M))
        if got != expect_aircraft:
            problems.append(f"summary.txt lists {got} aircraft, expected {expect_aircraft}")
    return problems


def file_problems(work: Path, rel: str, fn) -> list[str]:
    f = work / rel
    if not f.exists():
        return [f"{rel} was not written"]
    try:
        text = f.read_text(errors="strict")
    except UnicodeDecodeError:
        return [f"{rel} is not valid UTF-8"]
    return [f"{rel}: {p}" for p in fn(text)]
