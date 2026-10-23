#!/usr/bin/env python3
"""Drives a windowed T.O.R.E run with key presses sent through Hyprland.

    _replay_drive.py --data DIR [--bin PATH (default $TORE_BATTERY_BIN)] [--await-recording] [--cwd DIR] [--keys "wait 3;ctrl+P;Escape"] [--random SEED:COUNT:SET] -- GAME ARGS

The game is started through tools/agent-run.sh on a spare workspace. Keys go to
that one window by its process id (found by the unique data folder in its
environment), so nothing else on the desktop sees them. A window on a hidden
workspace loses focus at the first key, which pauses a flight, so callers set
TORE_PERF_ACTIVE=1 with a large TORE_PERF_FRAMES to keep it running. Named keys
(Insert, Delete, Space, F1 to F12, arrows) and Ctrl combinations reach the game;
plain letters and digits were seen to have no effect, so a script cannot rely on
them for gameplay.
The keys start once the window is mapped (plus DRIVE_SETTLE seconds, default 4), however
long a loaded machine takes to open it. The run ends with Alt+F4, the game's own exit. Exit status is the game's;
"driver:" lines say what was sent. Outside Hyprland the driver does nothing
and exits 0 with a note.
"""
from __future__ import annotations

import json
import os
import random
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
# Longest wait for the game's window to appear before the keys go out anyway (seconds).
WINDOW_WAIT = 90.0

# Key names as xkb knows them. Alt+F4 and Ctrl+Q leave the run, so random sets omit them.
FLIGHT_KEYS = (
    [f"F{n}" for n in range(1, 13)]
    + [f"shift+{d}" for d in "0123456789"]
    + list("123456789")
    + ["Up", "Down", "Left", "Right", "Prior", "Next", "End", "Home", "z", "x", "g", "b", "h", "o", "v", "m", "i", "r", "t", "y", "comma", "period", "bracketleft", "bracketright",
       "shift+Up", "shift+Down", "shift+Left", "shift+Right", "BackSpace", "ctrl+B", "ctrl+P", "Escape", "Return", "space", "Tab", "shift+m", "shift+n", "shift+l",
       "ctrl+Tab", "ctrl+1", "ctrl+2", "ctrl+3", "backslash", "semicolon", "l", "k", "j", "Insert", "Delete", "shift+e", "shift+k", "ctrl+shift+i", "shift+y", "shift+t", "alt+F1", "ctrl+F2", "alt+F4x"]
)
FLIGHT_KEYS = [k for k in FLIGHT_KEYS if k != "alt+F4x"]
REPLAY_KEYS = (
    ["space", "j", "k", "l", "Up", "Down", "Left", "Right", "shift+Left", "shift+Right", "Home", "End", "Prior", "Next", "Tab", "shift+Tab"]
    + [f"F{n}" for n in range(1, 13)] + ["alt+F1", "alt+F10", "ctrl+F10", "ctrl+F12", "0", "shift+0", "grave", "plus", "minus", "equal"]
    + list("wasdeqnticgmxrh") + ["shift+r", "shift+o", "o", "p", "Escape", "Return", "ctrl+P", "ctrl+B", "comma", "period", "bracketleft", "bracketright"]
    + list("123456789")
)
SETS = {"flight": FLIGHT_KEYS, "replay": REPLAY_KEYS}


def find_pid(marker: str) -> int | None:
    for p in os.listdir("/proc"):
        if not p.isdigit():
            continue
        try:
            env = Path(f"/proc/{p}/environ").read_bytes()
            argv = Path(f"/proc/{p}/cmdline").read_bytes().split(b"\0")
        except OSError:
            continue
        if marker.encode() in env and argv[0].endswith(b"tore-app"):
            return int(p)
    return None


def has_window(pid: int) -> bool:
    """True once the game's own window is mapped; key presses before that are dropped ("window not found")."""
    try:
        reply = subprocess.run(["hyprctl", "clients", "-j"], capture_output=True, text=True, timeout=10)
        return any(c.get("pid") == pid and c.get("mapped", True) for c in json.loads(reply.stdout))
    except (OSError, ValueError, subprocess.SubprocessError):
        return False


def cpu_share(pid: int, seconds: float = 1.5) -> str:
    """The game's processor use over a moment: a busy loop or a stall shows here."""
    def ticks() -> float | None:
        try:
            fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
            return float(fields[11]) + float(fields[12])
        except (OSError, IndexError, ValueError):
            return None

    before = ticks()
    time.sleep(seconds)
    after = ticks()
    if before is None or after is None:
        return "no process information"
    return f"{100 * (after - before) / os.sysconf('SC_CLK_TCK') / seconds:.0f}% of one core"


def send(pid: int, spec: str) -> str:
    parts = spec.split("+")
    key, mods = parts[-1], " ".join(m.upper() for m in parts[:-1])
    reply = subprocess.run(
        ["hyprctl", "dispatch", f'hl.dsp.send_shortcut({{mods="{mods}", key="{key}", window="pid:{pid}"}})'],
        capture_output=True, text=True,
    )
    return (reply.stdout + reply.stderr).strip()


def main(argv: list[str]) -> int:
    if "--" not in argv:
        print(__doc__)
        return 2
    opts, game = argv[: argv.index("--")], argv[argv.index("--") + 1 :]
    data = opts[opts.index("--data") + 1]
    script = opts[opts.index("--keys") + 1] if "--keys" in opts else ""
    steps = [s.strip() for s in script.split(";") if s.strip()]
    if "--random" in opts:
        seed, count, name = opts[opts.index("--random") + 1].split(":")
        rng = random.Random(int(seed))
        for _ in range(int(count)):
            steps.append(rng.choice(SETS[name]))
            if rng.random() < 0.3:
                steps.append("wait 0.4")
    if not os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"):
        print("driver: not running under Hyprland, nothing sent")
        return 0
    env = dict(os.environ, TORE_DATA_DIR=data, TORE_NO_ERROR_DIALOG="1", RUST_BACKTRACE="1")
    # The battery's binary under test (TORE_BATTERY_BIN, set by tools/battery.py), else the checkout's debug build:
    # the driver once always ran target/debug/tore-app, so a release battery's windowed flights ran a slow and
    # possibly stale debug build (or none, in a checkout that never built one).
    default = os.environ.get("TORE_BATTERY_BIN") or str(ROOT / "target" / "debug" / "tore-app")
    binary = opts[opts.index("--bin") + 1] if "--bin" in opts else default
    cwd = opts[opts.index("--cwd") + 1] if "--cwd" in opts else ROOT
    proc = subprocess.Popen(
        [str(ROOT / "tools" / "agent-run.sh"), binary, *game], cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace",
    )
    pid = None
    for _ in range(150):
        pid = find_pid(data)
        if pid or proc.poll() is not None:
            break
        time.sleep(0.2)
    if not pid:
        out, _ = proc.communicate(timeout=60)
        print(out[-2000:])
        print("driver: the game exited or never started")
        return proc.returncode if proc.returncode else 3
    print(f"driver: game pid {pid}")
    # A loaded machine can take well over the settle time to open the window (a debug build took 8 s or more
    # on 2026-10-13), and keys sent before it exists are dropped; wait for the window itself first.
    waited = 0.0
    while proc.poll() is None and not has_window(pid) and waited < WINDOW_WAIT:
        time.sleep(0.25)
        waited += 0.25
    print(f"driver: window {'up' if waited < WINDOW_WAIT else 'still missing'} after {waited:.1f} s")
    # --await-recording: a recorded flight's keys wait for the flight itself (its recording's `.partial` file in
    # the data folder's replays/), not only the window. A Quick Mission can take ten seconds or more past the
    # window to build its scene, and keys sent meanwhile reach the loading screen, not the cockpit.
    if "--await-recording" in opts:
        began = 0.0
        while proc.poll() is None and not list(Path(data, "replays").glob("*.partial")) and began < WINDOW_WAIT:
            time.sleep(0.25)
            began += 0.25
        print(f"driver: flight {'recording' if began < WINDOW_WAIT else 'still not recording'} after {began:.1f} s more")
    time.sleep(float(os.environ.get("DRIVE_SETTLE", "4")))
    alive = True
    for step in steps:
        if proc.poll() is not None:
            alive = False
            print(f"driver: the game exited before '{step}'")
            break
        if step.startswith("wait "):
            time.sleep(float(step[5:]))
            continue
        reply = send(pid, step)
        print(f"driver: sent {step} -> {reply}")
        time.sleep(0.25)
    if alive and proc.poll() is None:
        print(f"driver: before Alt+F4 the game shows {cpu_share(pid)}")
        print(f"driver: {send(pid, 'alt+F4')}")
    try:
        out, _ = proc.communicate(timeout=40)
    except subprocess.TimeoutExpired:
        subprocess.run(["kill", str(pid)])
        try:
            out, _ = proc.communicate(timeout=15)
        except subprocess.TimeoutExpired:
            proc.kill()
            out, _ = proc.communicate()
        print("driver: the game did not exit on Alt+F4 and was stopped")
        print(out[-3000:])
        return 4
    print(out[-3000:])
    return proc.returncode


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
