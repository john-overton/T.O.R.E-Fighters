#!/usr/bin/env python3
"""Repeat fixed performance workloads on a quiet machine and retain raw evidence.

Build first. Use the same profile content, arguments and repetitions before and
after. Windowed cases always use agent-run.sh. This tool never changes a profile,
builds code, stops other processes or downloads media.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parent.parent


def frame_summary(text: str, ticks: int, scale: int) -> dict:
    paused = re.search(r"paused frames: (\d+)", text)
    work = re.search(r"workload: (\d+) ticks \(target (\d+)\); designated estimate frames: (\d+); weapon: (\S+)", text)
    rate = re.search(r"([\d.]+)x real time \(asked for ([\d.]+)x\)", text)
    interval = re.search(r"frame interval: mean ([\d.]+) ms, p50 ([\d.]+), p95 ([\d.]+), p99 ([\d.]+), max ([\d.]+)", text)
    if not work or not rate or not interval or not paused:
        raise ValueError("missing completed frame measurement")
    actual, target, locked, weapon = work.groups()
    if int(actual) < ticks or int(target) != ticks or float(rate[2]) != scale:
        raise ValueError("the requested tick workload or time scale was not reached")
    if int(paused[1]) != 0:
        raise ValueError("the timed flight paused or faulted")
    if int(locked) == 0 or weapon != "AIM120.JT":
        raise ValueError("the lock case never produced a designated AIM-120 estimate")
    return {"ticks": int(actual), "locked_frames": int(locked), "weapon": weapon,
            "achieved_scale": float(rate[1]),
            **dict(zip(("frame_mean_ms", "frame_p50_ms", "frame_p95_ms", "frame_p99_ms", "frame_max_ms"),
                       map(float, interval.groups())))}


def foreign_games() -> list[str]:
    proc = Path("/proc")
    if not proc.is_dir():
        return []
    found = []
    for entry in proc.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            name = (entry / "exe").resolve(strict=True).name
        except (OSError, RuntimeError):
            continue
        if name in ("tore-app", "tore-server") or name.startswith("host_players-"):
            found.append(f"{entry.name}: {name}")
    return found


def quiet() -> None:
    found = foreign_games()
    if found:
        raise RuntimeError("another game or host measurement is running: " + ", ".join(found))


def host_summary(text: str) -> dict:
    found = re.search(r"host elapsed: ([\d.]+) ms a tick.*?ticking calls p50 ([\d.]+) ms, p95 ([\d.]+) ms, p99 ([\d.]+) ms, p99.9 ([\d.]+) ms, longest ([\d.]+) ms", text)
    if not found:
        raise ValueError("host benchmark did not report complete elapsed timings")
    return dict(zip(("host_mean_ms", "host_p50_ms", "host_p95_ms", "host_p99_ms", "host_p999_ms", "host_max_ms"),
                    map(float, found.groups())))


def run_case(command: list[str], env: dict[str, str]) -> subprocess.CompletedProcess:
    # Give agent-run.sh its normal termination trap before using a hard stop.
    # Killing only that wrapper on timeout would leave its Hyprland-launched
    # child running. These are exclusively processes started by this harness.
    process = subprocess.Popen(command, env=env, cwd=ROOT, text=True,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    try:
        output, _ = process.communicate(timeout=1200)
    except BaseException:
        process.terminate()
        try:
            process.communicate(timeout=15)
        except subprocess.TimeoutExpired:
            process.kill()
            process.communicate()
        raise
    return subprocess.CompletedProcess(command, process.returncode, output)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--profile", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--suite", choices=("headless", "host", "frames"), required=True)
    parser.add_argument("--host-benchmark", type=Path)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--ticks", type=int, default=2400)
    parser.add_argument("--seconds", type=int, default=60)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--scales", default="1,2,4,8")
    parser.add_argument("--missions", default="light,heavy")
    parser.add_argument("--audio", choices=("both", "on", "off"), default="both")
    parser.add_argument("--compare", type=Path)
    args = parser.parse_args()
    if args.repeat < 1 or args.ticks < 120 or args.seconds < 1:
        parser.error("positive repeat/seconds and at least 120 ticks are required")
    if not 0 <= args.workers <= 8:
        parser.error("--workers must be 0..8, matching the executor's supported sizes")
    binary, profile, output = args.binary.resolve(), args.profile.resolve(), args.output.resolve()
    if not binary.is_file() or not (profile / "media-source.txt").is_file():
        parser.error("a built binary and an isolated imported profile are required")
    if args.suite == "host" and (not args.host_benchmark or not args.host_benchmark.is_file()):
        parser.error("--host-benchmark must name the compiled host_players test executable")
    scales = [int(value) for value in args.scales.split(",")]
    missions = args.missions.split(",")
    if any(value not in (1, 2, 4, 8) for value in scales) or any(value not in ("light", "heavy") for value in missions):
        parser.error("scales must be 1,2,4,8 and missions light,heavy")
    output.mkdir(parents=True, exist_ok=False)
    env = {**os.environ, "TORE_DATA_DIR": str(profile), "TORE_NO_ERROR_DIALOG": "1",
           "TORE_WORKERS": str(args.workers), "TORE_LOG_DIR": str(output / "session-logs")}
    for key in list(env):
        if key.startswith("TORE_PERF_"):
            del env[key]
    cases = []
    if args.suite == "headless":
        for mission in missions:
            for ticks in sorted({args.ticks, max(args.ticks, 21600)}):
                cases.append((f"{mission}-{ticks}", [str(binary), "--ai-probe-ticks", str(ticks),
                              "--probe-fight", "2:2" if mission == "light" else "15:15",
                              "--separation", "5", "--probe-attack", "600:10", "--no-audio"], {}, None))
    elif args.suite == "host":
        for humans in (0, 15, 30):
            cases.append((f"host-{humans}", [str(args.host_benchmark.resolve()), "--ignored", "--nocapture", "--test-threads=1"],
                          {"TORE_MEASURE_BOTS": str(humans), "TORE_MEASURE_OPEN": "all", "TORE_MEASURE_SECONDS": str(args.seconds)}, None))
    else:
        for mission in missions:
            for scale in scales:
                for sound in ([False, True] if args.audio == "both" else [args.audio == "on"]):
                    script = output / f"scale-{scale}.script"
                    script.write_text("waittick 120\nkey ]\nkey t\n" + "key c\n" * (scale.bit_length() - 1))
                    command = [str(ROOT / "tools/agent-run.sh"), str(binary), "--launch-quick-mission",
                               "--probe-fight", "2:2" if mission == "light" else "15:15", "--separation", "5",
                               "--window-size", "1920x1080", "--input-script", str(script)]
                    if not sound:
                        command.append("--no-audio")
                    cases.append((f"{mission}-{scale}x-audio-{'on' if sound else 'off'}", command,
                                  {"TORE_PERF_TICKS": str(args.ticks), "TORE_PERF_ACTIVE": "1",
                                   "TORE_RECORD_MISSIONS": "1"}, scale))
    results = {"settings": {"suite": args.suite, "repeat": args.repeat, "ticks": args.ticks,
                            "seconds": args.seconds, "scales": scales, "missions": missions, "audio": args.audio},
               "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "workers": args.workers,
               "runs": {}}
    for name, command, extra, scale in cases:
        rows = []
        for repeat in range(args.repeat):
            quiet()
            print(f"{name} {repeat + 1}/{args.repeat}", flush=True)
            start = time.perf_counter()
            done = run_case(command, {**env, **extra})
            elapsed = time.perf_counter() - start
            (output / f"{name}-{repeat + 1}.log").write_text(done.stdout)
            if done.returncode:
                raise RuntimeError(f"{name}: exit {done.returncode}; see raw log")
            row = {"process_seconds": elapsed}
            if scale is not None:
                row.update(frame_summary(done.stdout, args.ticks, scale))
            elif args.suite == "host":
                row.update(host_summary(done.stdout))
            rows.append(row)
        results["runs"][name] = rows
        (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    if args.compare:
        before = json.loads(args.compare.read_text())
        if before["settings"] != results["settings"] or before["runs"].keys() != results["runs"].keys():
            raise ValueError("before and after workloads differ")
        metric = {"headless": "process_seconds", "host": "host_mean_ms", "frames": "frame_mean_ms"}[args.suite]
        for name, rows in results["runs"].items():
            old = statistics.median(row[metric] for row in before["runs"][name])
            new = statistics.median(row[metric] for row in rows)
            print(f"{name}: {metric} {old:.3f} -> {new:.3f}, {old / new:.2f}x", flush=True)


if __name__ == "__main__":
    main()
