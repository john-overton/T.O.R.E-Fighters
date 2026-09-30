#!/usr/bin/env python3
"""Battery test runner for T.O.R.E-Fighters.

Runs scenarios (one `tore-app` invocation each) in parallel, checks the output
for problems and writes one results folder per run. See docs/testing/README.md.

    python3 tools/battery.py --list
    python3 tools/battery.py --lane ai --jobs 8
    python3 tools/battery.py --scenario 'ai-fight-*' --keep-going

Every scenario gets its own copy of an imported data folder (a copy-on-write
copy where the filesystem supports it), so runs cannot disturb each other or
John's real profile. Scenarios that open a window go through tools/agent-run.sh
and at most `--windows` run at once.
"""
from __future__ import annotations

import argparse
import concurrent.futures as futures
import dataclasses
import fnmatch
import importlib
import json
import os
import re
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Callable, Optional

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

LANES = ("menus", "flight", "ai", "replay")

# Output that means something is wrong in any scenario, unless it opts out.
GENERIC_BAD = [
    (re.compile(r"panicked at"), "panic"),
    (re.compile(r"\bNaN\b"), "NaN in output"),
    (re.compile(r"(?<![A-Za-z])-?inf(?![A-Za-z])"), "infinity in output"),
    (re.compile(r"thread '.*' has overflowed its stack"), "stack overflow"),
    (re.compile(r"(?i)\bfatal\b.*error"), "fatal error"),
]


@dataclasses.dataclass
class Scenario:
    """One run of the app plus what a healthy run looks like."""

    name: str
    lane: str
    args: list[str]
    timeout: float = 300.0
    window: bool = False  # opens a window: wrap in agent-run.sh, limit concurrency
    expect: list[str] = dataclasses.field(default_factory=list)  # regexes that must appear
    forbid: list[str] = dataclasses.field(default_factory=list)  # regexes that must not
    allow_generic: list[str] = dataclasses.field(default_factory=list)  # generic labels to tolerate
    check: Optional[Callable[[str], list[str]]] = None  # extra problems from the output
    env: dict[str, str] = dataclasses.field(default_factory=dict)
    expect_exit: int = 0
    notes: str = ""
    # Files (relative to the scenario's work folder) the run should produce.
    outputs: list[str] = dataclasses.field(default_factory=list)
    # Follow-up commands run after the main one, in the same environment (for
    # example reading a recording back). Their output is appended to the log
    # and to what `expect`, `forbid` and `check` see.
    then: list["Step"] = dataclasses.field(default_factory=list)
    # Looks at the work folder (and the joined output) once everything has run.
    check_work: Optional[Callable[[Path, str], list[str]]] = None
    # A defect found and left for someone else: the scenario still runs, its
    # failure is reported as known instead of failing the run, and it fails
    # once it starts passing so the marker gets removed.
    known_failure: str = ""


@dataclasses.dataclass
class Step:
    """A follow-up command. `app` steps run the game binary, others run as given."""

    args: list[str]
    app: bool = True
    expect_exit: Optional[int] = 0  # None accepts any exit code
    window: bool = False
    timeout: float = 120.0


@dataclasses.dataclass
class Result:
    name: str
    lane: str
    ok: bool
    seconds: float
    exit_code: Optional[int]
    problems: list[str]
    command: list[str]
    log: str


def load_scenarios() -> list[Scenario]:
    found: list[Scenario] = []
    pkg = ROOT / "tools" / "battery_scenarios"
    for path in sorted(pkg.glob("*.py")):
        if path.name.startswith("_"):
            continue
        module = importlib.import_module(f"battery_scenarios.{path.stem}")
        found.extend(module.scenarios())
    names = [s.name for s in found]
    dupes = {n for n in names if names.count(n) > 1}
    if dupes:
        raise SystemExit(f"duplicate scenario names: {sorted(dupes)}")
    for s in found:
        if s.lane not in LANES:
            raise SystemExit(f"{s.name}: unknown lane {s.lane!r}")
    return found


def clone_profile(source: Path, dest: Path) -> None:
    if dest.exists():
        shutil.rmtree(dest)
    dest.parent.mkdir(parents=True, exist_ok=True)
    # A reflink copy is near free on Linux; elsewhere (no GNU cp, or no cp at all) copy the tree.
    try:
        copied = subprocess.run(["cp", "-a", "--reflink=auto", str(source), str(dest)]).returncode == 0
    except OSError:
        copied = False
    if not copied:
        shutil.rmtree(dest, ignore_errors=True)
        shutil.copytree(source, dest)


def judge(s: Scenario, output: str, code: Optional[int], timed_out: bool, work: Path) -> list[str]:
    problems: list[str] = []
    if timed_out:
        problems.append(f"timed out after {s.timeout:.0f}s")
    elif code != s.expect_exit:
        problems.append(f"exit code {code}, expected {s.expect_exit}")
    for pattern, label in GENERIC_BAD:
        if label in s.allow_generic:
            continue
        m = pattern.search(output)
        if m:
            line = output[max(0, output.rfind("\n", 0, m.start()) + 1) : output.find("\n", m.end())]
            problems.append(f"{label}: {line.strip()[:200]}")
    for pattern in s.expect:
        if not re.search(pattern, output, re.M):
            problems.append(f"missing expected output /{pattern}/")
    for pattern in s.forbid:
        m = re.search(pattern, output, re.M)
        if m:
            problems.append(f"forbidden output /{pattern}/: {m.group(0)[:200]}")
    for rel in s.outputs:
        f = work / rel
        if not f.exists() or f.stat().st_size == 0:
            problems.append(f"expected file not written: {rel}")
    if s.check:
        problems.extend(s.check(output))
    if s.check_work:
        problems.extend(s.check_work(work, output))
    return problems


def run_steps(
    s: Scenario, opts: argparse.Namespace, env: dict, work: Path, output: str, step_problems: list[str],
    window_slots: Optional[threading.Semaphore] = None,
) -> str:
    """Runs a scenario's follow-up commands, appending their output; returns the joined output."""
    for i, step in enumerate(s.then):
        step_cmd = [a.replace("{work}", str(work)) for a in step.args]
        if step.app:
            step_cmd = [opts.bin, *step_cmd]
        if step.window:
            step_cmd = [str(ROOT / "tools" / "agent-run.sh"), *step_cmd]
        # A window step counts against the window limit unless the scenario already holds a slot.
        slot = window_slots if (step.window and not s.window and window_slots) else None
        if slot:
            slot.acquire()
        try:
            done = subprocess.run(
                step_cmd, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                errors="replace", timeout=step.timeout * opts.timeout_scale, start_new_session=True,
            )
            step_out, step_code = done.stdout, done.returncode
        except subprocess.TimeoutExpired as e:
            step_out = (e.stdout.decode(errors="replace") if isinstance(e.stdout, bytes) else (e.stdout or ""))
            step_code = None
            step_problems.append(f"step {i + 1} timed out")
        except OSError as e:
            step_out, step_code = f"could not run the step: {e}\n", None
            step_problems.append(f"step {i + 1} could not run: {e}")
        finally:
            if slot:
                slot.release()
        output += f"\n$ then {i + 1}: {' '.join(step_cmd)}\n{step_out}"
        if step_code is not None and step.expect_exit is not None and step_code != step.expect_exit:
            step_problems.append(f"step {i + 1} exit code {step_code}, expected {step.expect_exit}")
    return output


def run_one(s: Scenario, opts: argparse.Namespace, run_dir: Path, window_slots: threading.Semaphore) -> Result:
    work = run_dir / "work" / s.name
    work.mkdir(parents=True, exist_ok=True)
    data = work / "data"
    clone_profile(Path(opts.profile), data)
    resolved = [a.replace("{work}", str(work)) for a in s.args]
    cmd = [opts.bin, *resolved]
    if s.window:
        cmd = [str(ROOT / "tools" / "agent-run.sh"), *cmd]
    env = dict(os.environ)
    env.update({"TORE_DATA_DIR": str(data), "TORE_NO_ERROR_DIALOG": "1", "RUST_BACKTRACE": "1"})
    env.update({k: v.replace("{work}", str(work)) for k, v in s.env.items()})
    timeout = s.timeout * opts.timeout_scale
    started = time.time()
    if s.window:
        window_slots.acquire()
    try:
        proc = subprocess.Popen(
            cmd, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace",
            start_new_session=True,
        )
        timed_out = False
        try:
            output, _ = proc.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(proc.pid, 15)
            except ProcessLookupError:
                pass
            try:
                output, _ = proc.communicate(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(proc.pid, 9)
                output, _ = proc.communicate()
        step_problems: list[str] = []
        output = run_steps(s, opts, env, work, output, step_problems, window_slots)
    finally:
        if s.window:
            window_slots.release()
    seconds = time.time() - started
    log = run_dir / "logs" / f"{s.name}.log"
    log.parent.mkdir(parents=True, exist_ok=True)
    log.write_text(f"$ {' '.join(cmd)}\n\n{output}")
    problems = judge(s, output, None if timed_out else proc.returncode, timed_out, work) + step_problems
    if s.known_failure:
        if problems:
            problems = [f"known failure ({s.known_failure}): {p}" for p in problems[:1]]
            passed = True
        else:
            problems = [f"known failure now passes, remove known_failure ({s.known_failure})"]
            passed = False
    else:
        passed = not problems
    if not opts.keep_data:
        shutil.rmtree(data, ignore_errors=True)
    return Result(s.name, s.lane, passed, seconds, None if timed_out else proc.returncode, problems, cmd, str(log.relative_to(run_dir)))


def write_summary(run_dir: Path, results: list[Result], started: float) -> None:
    (run_dir / "results.json").write_text(json.dumps([dataclasses.asdict(r) for r in results], indent=1))
    failed = [r for r in results if not r.ok]
    lines = [
        f"# Battery run {run_dir.name}",
        "",
        f"{len(results)} scenarios, {len(results) - len(failed)} passed, {len(failed)} failed, "
        f"{time.time() - started:.0f}s wall clock.",
        "",
    ]
    if failed:
        lines += ["## Failures", ""]
        for r in failed:
            lines.append(f"- `{r.name}` ({r.lane}, {r.seconds:.1f}s): {'; '.join(r.problems)}  (log: `{r.log}`)")
        lines.append("")
    lines += ["## All scenarios", "", "| Scenario | Lane | Result | Seconds |", "| --- | --- | --- | --- |"]
    for r in sorted(results, key=lambda r: (r.lane, r.name)):
        lines.append(f"| `{r.name}` | {r.lane} | {'pass' if r.ok else 'FAIL'} | {r.seconds:.1f} |")
    (run_dir / "summary.md").write_text("\n".join(lines) + "\n")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--list", action="store_true", help="list scenarios and exit")
    ap.add_argument("--lane", action="append", choices=LANES, help="run only this lane (repeatable)")
    ap.add_argument("--scenario", action="append", help="glob on scenario names (repeatable)")
    ap.add_argument("--jobs", type=int, default=6, help="scenarios at once (default 6)")
    ap.add_argument("--windows", type=int, default=3, help="windowed scenarios at once (default 3)")
    ap.add_argument("--bin", default=str(ROOT / "target" / "debug" / "tore-app"))
    ap.add_argument("--profile", default=str(ROOT / ".local" / "bugbash-data"), help="imported data folder to clone per scenario")
    ap.add_argument("--out", default=str(ROOT / ".local" / "battery"), help="folder that receives one subfolder per run")
    ap.add_argument("--timeout-scale", type=float, default=1.0)
    ap.add_argument("--keep-data", action="store_true", help="keep each scenario's data folder (replays, logs)")
    ap.add_argument("--tag", default="", help="suffix for the run folder name")
    opts = ap.parse_args(argv)

    scenarios = load_scenarios()
    if opts.lane:
        scenarios = [s for s in scenarios if s.lane in opts.lane]
    if opts.scenario:
        scenarios = [s for s in scenarios if any(fnmatch.fnmatch(s.name, g) for g in opts.scenario)]
    if opts.list:
        for s in scenarios:
            print(f"{s.lane:8} {'window' if s.window else '      '} {s.name}")
        print(f"{len(scenarios)} scenarios")
        return 0
    if not scenarios:
        print("no scenarios selected", file=sys.stderr)
        return 2
    if not Path(opts.bin).exists():
        print(f"binary not found: {opts.bin} (build it first)", file=sys.stderr)
        return 2
    if not (Path(opts.profile) / "media-source.txt").exists():
        print(f"{opts.profile} is not an imported data folder; import once with --import ... --import-only", file=sys.stderr)
        return 2

    stamp = time.strftime("%Y%m%d-%H%M%S") + (f"-{opts.tag}" if opts.tag else "")
    run_dir = Path(opts.out) / stamp
    run_dir.mkdir(parents=True, exist_ok=True)
    started = time.time()
    slots = threading.Semaphore(opts.windows)
    results: list[Result] = []
    with futures.ThreadPoolExecutor(max_workers=opts.jobs) as pool:
        pending = {pool.submit(run_one, s, opts, run_dir, slots): s for s in scenarios}
        for fut in futures.as_completed(pending):
            r = fut.result()
            results.append(r)
            print(f"{'pass' if r.ok else 'FAIL'} {r.name} ({r.seconds:.1f}s)", flush=True)
            for p in r.problems:
                print(f"     {p}", flush=True)
            write_summary(run_dir, results, started)
    write_summary(run_dir, results, started)
    failed = sum(1 for r in results if not r.ok)
    print(f"\n{len(results) - failed}/{len(results)} passed. Results: {run_dir}/summary.md")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
