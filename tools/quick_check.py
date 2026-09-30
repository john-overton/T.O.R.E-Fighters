#!/usr/bin/env python3
"""The per-change quick check: one command to run while you work (about five minutes).

    python3 tools/quick_check.py [--base REF] [--budget SECONDS]

It looks at what changed since REF (default: the merge base with `multiplayer`, or HEAD~1 when on it)
and runs, in order:

  1. formatting (`cargo fmt --all -- --check`);
  2. clippy and the tests for the crates the change touches and the crates that depend on them (the
     whole workspace when a shared crate's public API or a Cargo file changed);
  3. the Python tests, when anything under tools/ changed;
  4. the documentation check, when anything under docs/ changed;
  5. a build of the app, then the battery scenarios the change can affect, fitted to the budget
     (`tools/battery.py --changed`, see docs/testing/README.md);
  6. the quick single-player guard, when the local harness is present (`.local/mp-baseline/quick.sh`).

It prints each step's time and a total, and exits nonzero if any step failed. This is an iteration aid.
The merge requirement is still the check list in AGENTS.md, which the pre-push hook runs.
"""
from __future__ import annotations

import argparse
import dataclasses
import os
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional, Sequence

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

import battery_selection as selection  # noqa: E402

# A public item changing in one of these crates can break any crate that uses it.
API_LINE = re.compile(r"^[+-]\s*pub\s+(?!\((?:crate|super|in)\b)(?:unsafe\s+|async\s+|const\s+)*(?:fn|struct|enum|trait|type|const|static|mod|use|union)\b")
CARGO_FILES = ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml")


# --------------------------------------------------------------------------
# Which crates
# --------------------------------------------------------------------------


def workspace_crates(root: Path) -> dict[str, list[str]]:
    """Crate directory name -> the workspace crates it depends on (normal and dev dependencies)."""
    crates: dict[str, list[str]] = {}
    for manifest in sorted((root / "crates").glob("*/Cargo.toml")):
        deps = re.findall(r'^\s*([A-Za-z0-9_-]+)\s*=\s*\{[^}]*path\s*=\s*"\.\./([A-Za-z0-9_-]+)"', manifest.read_text(), re.M)
        crates[manifest.parent.name] = sorted({path_dir for _, path_dir in deps})
    return crates


def dependents_closure(crates: dict[str, list[str]], touched: set[str]) -> set[str]:
    """The touched crates and every crate that depends on them, directly or not."""
    result = set(touched)
    grew = True
    while grew:
        grew = False
        for name, deps in crates.items():
            if name not in result and any(d in result for d in deps):
                result.add(name)
                grew = True
    return result


def touched_crates(changed: Sequence[str], crates: dict[str, list[str]]) -> set[str]:
    found = set()
    for path in changed:
        m = re.match(r"crates/([^/]+)/", path)
        if m and m.group(1) in crates:
            found.add(m.group(1))
    return found


def public_api_changed(root: Path, base: str, head: Optional[str], crate: str) -> bool:
    """Heuristic: a `pub` item's line was added or removed in the crate's sources."""
    args = ["git", "diff", "-U0", base] + ([head] if head else []) + ["--", f"crates/{crate}/src"]
    out = subprocess.run(args, cwd=root, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, errors="replace").stdout
    return any(API_LINE.match(line) for line in out.splitlines() if not line.startswith(("+++", "---")))


@dataclasses.dataclass
class CargoScope:
    packages: list[str]
    workspace: bool
    reason: str


def cargo_scope(root: Path, changed: Sequence[str], base: str, head: Optional[str]) -> Optional[CargoScope]:
    """None when no Rust is involved. Otherwise the packages to lint and test, or the whole workspace."""
    crates = workspace_crates(root)
    touched = touched_crates(changed, crates)
    cargo_changed = [p for p in changed if p in CARGO_FILES or re.fullmatch(r"crates/[^/]+/Cargo\.toml", p)]
    if not touched and not cargo_changed:
        return None
    if cargo_changed:
        return CargoScope(sorted(crates), True, f"{cargo_changed[0]} changed")
    closure = dependents_closure(crates, touched)
    for crate in sorted(touched):
        has_dependents = any(crate in deps for deps in crates.values())
        if has_dependents and public_api_changed(root, base, head, crate):
            return CargoScope(sorted(crates), True, f"public API of {crate} changed")
    return CargoScope(sorted(closure), closure == set(crates), f"touched {', '.join(sorted(touched))}; with dependents")


# --------------------------------------------------------------------------
# Running steps
# --------------------------------------------------------------------------


@dataclasses.dataclass
class StepResult:
    name: str
    status: str  # ok, FAIL, skipped
    seconds: float
    detail: str = ""


class Runner:
    def __init__(self, log_dir: Path, fail_fast: bool):
        self.log_dir = log_dir
        self.fail_fast = fail_fast
        self.results: list[StepResult] = []

    def skip(self, name: str, why: str) -> None:
        self.results.append(StepResult(name, "skipped", 0.0, why))
        print(f"  skipped  {name}: {why}", flush=True)

    def run(self, name: str, cmd: Sequence[str], env: Optional[dict] = None, summary=None, echo=None) -> bool:
        print(f"  running  {name}: {' '.join(cmd)}", flush=True)
        started = time.time()
        log = self.log_dir / f"{re.sub(r'[^a-z0-9]+', '-', name.lower())}.log"
        try:
            done = subprocess.run(
                cmd, cwd=ROOT, env={**os.environ, **(env or {})}, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                text=True, errors="replace",
            )
            output, code = done.stdout, done.returncode
        except OSError as e:
            output, code = f"could not run: {e}\n", 127
        seconds = time.time() - started
        log.write_text(f"$ {' '.join(cmd)}\n\n{output}")
        ok = code == 0
        detail = (summary(output) if summary else "") or ""
        if echo and output:
            text = echo(output)
            if text:
                print(text, flush=True)
        if not ok:
            tail = "\n".join(output.splitlines()[-30:])
            print(f"  FAILED   {name} (exit {code}, {seconds:.0f}s); last lines, full log {log}:\n" + "\n".join("      " + l for l in tail.splitlines()), flush=True)
            detail = f"exit {code}; log {log}"
        else:
            print(f"  ok       {name} ({seconds:.0f}s){': ' + detail if detail else ''}", flush=True)
        self.results.append(StepResult(name, "ok" if ok else "FAIL", seconds, detail))
        if not ok and self.fail_fast:
            raise SystemExit(self.finish())
        return ok

    def finish(self) -> int:
        total = sum(r.seconds for r in self.results)
        print("\nQuick check summary")
        for r in self.results:
            print(f"  {r.status:8} {r.seconds:6.0f}s  {r.name}" + (f"  ({r.detail})" if r.detail else ""))
        failed = [r for r in self.results if r.status == "FAIL"]
        print(f"  {'':8} {total:6.0f}s  total")
        if failed:
            print(f"\nFAILED: {', '.join(r.name for r in failed)}. Logs are in {self.log_dir}")
            return 1
        print("\nQuick check passed. The AGENTS.md check list is still the merge requirement.")
        return 0


def last_line(output: str) -> str:
    lines = [l for l in output.strip().splitlines() if l.strip()]
    return lines[-1].strip()[:160] if lines else ""


def battery_summary(output: str) -> str:
    chosen = re.search(r"chosen (\d+) of (\d+) candidate scenarios \(([^)]*)\), estimated (\d+) s wall", output)
    passed = re.search(r"(\d+)/(\d+) passed", output)
    if passed and chosen:
        return f"{passed.group(0)} ({chosen.group(3)}, estimated {chosen.group(4)} s)"
    if "Nothing to run" in output:
        return "no scenario can be affected"
    return last_line(output)


def battery_echo(output: str) -> str:
    """The plan the battery printed, without the per-scenario progress lines."""
    plan: list[str] = []
    for line in output.splitlines():
        if re.match(r"^(pass|FAIL) ", line):
            break
        plan.append(line)
    return "\n".join("    " + l for l in plan if l.strip())


def guard_summary(output: str) -> str:
    m = re.findall(r"^SAME \d+, DIFFERENT \d+, MISSING \d+", output, re.M)
    return m[-1] if m else last_line(output)


# --------------------------------------------------------------------------
# Main
# --------------------------------------------------------------------------


def find_profile(explicit: Optional[str]) -> Optional[Path]:
    candidates = [explicit, os.environ.get("TORE_DATA_DIR"), str(ROOT / ".local" / "bugbash-data")]
    for c in candidates:
        if c and (Path(c) / "media-source.txt").exists():
            return Path(c).resolve()
    return None


def main(argv: Sequence[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base", metavar="REF", help="compare against REF (default: merge base with multiplayer, or HEAD~1 on it)")
    ap.add_argument("--head", metavar="REF", help="take the changed files from BASE..REF instead of the working tree (to try a past change on the current build)")
    ap.add_argument("--budget", type=float, default=selection.DEFAULT_BUDGET, help="wall-clock seconds for the battery selection (default %(default)s)")
    ap.add_argument("--jobs", type=int, default=None, help="parallel runs for the battery and the guard (default: half the cores, 4 to 12)")
    ap.add_argument("--profile", help="imported data folder for the battery and the guard (default: $TORE_DATA_DIR, then .local/bugbash-data)")
    ap.add_argument("--with-windows", choices=("auto", "yes", "no"), default="auto", help="windowed battery scenarios (auto: only when the change touches rendering or windowed input)")
    ap.add_argument("--no-battery", action="store_true", help="skip the battery selection")
    ap.add_argument("--no-guard", action="store_true", help="skip the single-player guard")
    ap.add_argument("--fail-fast", action="store_true", help="stop at the first failing step")
    ap.add_argument("--plan", action="store_true", help="print what would run and stop")
    opts = ap.parse_args(argv)
    jobs = opts.jobs or selection.default_jobs()

    base = opts.base or selection.default_base(ROOT)
    try:
        changed = selection.changed_files(ROOT, base, opts.head)
    except RuntimeError as e:
        print(f"quick_check: {e}", file=sys.stderr)
        return 2
    scope = cargo_scope(ROOT, changed, base, opts.head)
    tools_changed = any(p.startswith("tools/") or p.startswith(".githooks/") for p in changed)
    docs_changed = any(p.startswith("docs/") or p.startswith("README") for p in changed)
    rust_changed = scope is not None

    print(f"Quick check: {len(changed)} changed file(s) since {base}{' to ' + opts.head if opts.head else ''}")
    if scope:
        what = "the whole workspace" if scope.workspace else ", ".join(scope.packages)
        print(f"  Rust: {what} ({scope.reason})")
    else:
        print("  Rust: no crate changed")
    if opts.plan:
        print(f"  Python tests: {'yes' if tools_changed else 'no'}; docs check: {'yes' if docs_changed else 'no'}")
        return 0

    stamp = time.strftime("%Y%m%d-%H%M%S")
    local = ROOT / ".local"
    log_dir = (local if local.is_dir() else Path(os.environ.get("TMPDIR", "/tmp"))) / "quick-check" / stamp
    log_dir.mkdir(parents=True, exist_ok=True)
    runner = Runner(log_dir, opts.fail_fast)
    started = time.time()

    runner.run("fmt", ["cargo", "fmt", "--all", "--", "--check"])

    build_ok = True
    if scope:
        pkgs = [] if scope.workspace else [a for p in scope.packages for a in ("-p", p)]
        scope_args = ["--workspace"] if scope.workspace else pkgs
        runner.run("clippy", ["cargo", "clippy", *scope_args, "--all-targets", "--locked", "--", "-D", "warnings"])
        runner.run("rust tests", ["cargo", "test", *scope_args, "--locked"])
    else:
        runner.skip("clippy", "no Rust changed")
        runner.skip("rust tests", "no Rust changed")

    if tools_changed:
        runner.run("python tests", [sys.executable, "-m", "unittest", "discover", "-s", "tools", "-p", "test_*.py"])
    else:
        runner.skip("python tests", "nothing under tools/ changed")

    if docs_changed:
        runner.run("docs check", [sys.executable, "tools/check_docs.py"])
    else:
        runner.skip("docs check", "no documentation changed")

    profile = find_profile(opts.profile)
    want_battery = not opts.no_battery
    want_guard = not opts.no_guard and rust_changed
    guard_script = local / "mp-baseline" / "quick.sh"
    if want_battery or want_guard:
        if profile is None:
            if want_battery:
                runner.skip("battery selection", "no imported data profile (pass --profile, see docs/testing/README.md)")
            if want_guard:
                runner.skip("single-player guard", "no imported data profile")
            want_battery = want_guard = False
    if want_battery or want_guard:
        build_ok = runner.run("build", ["cargo", "build", "--locked", "-p", "tore-app"])
        if not build_ok:
            if want_battery:
                runner.skip("battery selection", "the build failed")
            if want_guard:
                runner.skip("single-player guard", "the build failed")
            want_battery = want_guard = False
    if want_battery:
        cmd = [
            sys.executable, "tools/battery.py", "--changed", base, "--budget", str(opts.budget), "--jobs", str(jobs),
            "--profile", str(profile), "--with-windows", opts.with_windows, "--no-unit-tests", "--tag", "quick",
        ]
        if opts.head:
            cmd += ["--head", opts.head]
        runner.run("battery selection", cmd, summary=battery_summary, echo=battery_echo)
    elif opts.no_battery:
        runner.skip("battery selection", "--no-battery")
    if want_guard:
        if guard_script.exists():
            out = log_dir / "guard"
            runner.run("single-player guard", [str(guard_script), str(out)], env={"TORE_DATA_DIR": str(profile), "JOBS": str(jobs)}, summary=guard_summary)
        else:
            runner.skip("single-player guard", "the local harness (.local/mp-baseline/quick.sh) is not present")
    elif opts.no_guard:
        runner.skip("single-player guard", "--no-guard")
    elif not rust_changed:
        runner.skip("single-player guard", "no Rust changed")

    print(f"\nWall clock {time.time() - started:.0f}s.")
    return runner.finish()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
