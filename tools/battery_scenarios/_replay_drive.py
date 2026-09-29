#!/usr/bin/env python3
"""Drives a windowed T.O.R.E run with key presses sent through Hyprland.

    _replay_drive.py --data DIR [--bin PATH] [--keys "wait 3;ctrl+P;Escape"] [--random SEED:COUNT:SET] -- GAME ARGS

The game is started through tools/agent-run.sh on a spare workspace. Keys go to
that one window by its process id (found by the unique data folder in its
environment), so nothing else on the desktop sees them. A window on a hidden
workspace loses focus at the first key, which pauses a flight, so callers set
TORE_PERF_ACTIVE=1 with a large TORE_PERF_FRAMES to keep it running. Named keys
(Insert, Delete, Space, F1 to F12, arrows) and Ctrl combinations reach the game;
plain letters and digits were seen to have no effect, so a script cannot rely on
them for gameplay.
The run ends with Alt+F4, the game's own exit. Exit status is the game's;
"driver:" lines say what was sent. Outside Hyprland the driver does nothing
and exits 0 with a note.
"""
from __future__ import annotations

import os
import random
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

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
    binary = opts[opts.index("--bin") + 1] if "--bin" in opts else str(ROOT / "target" / "debug" / "tore-app")
    proc = subprocess.Popen(
        [str(ROOT / "tools" / "agent-run.sh"), binary, *game], cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace",
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
