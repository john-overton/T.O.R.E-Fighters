"""Helper commands the network-capture scenarios run as follow-up steps.

    python3 _replay_net_run.py capture WORK       a bot flies a dedicated server for 25 s and keeps its capture
    python3 _replay_net_run.py same A B           exits 0 when the two files are byte for byte the same
    python3 _replay_net_run.py cut SRC DEST PART  writes the first PART/100 of SRC to DEST (a capture cut short)

`capture` starts `tore-server` on a free UDP port with a small mission (two
Hornets against two Fulcrums, airborne), joins it with `tore-bot --capture`,
and stops the server it started. It builds the binaries beside
`tore-app` first (a build must match to join) and needs the imported data
folder in `TORE_DATA_DIR`, as every scenario has.
"""
from __future__ import annotations

import os
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BIN = ROOT / "target" / "debug"
CAPTURE_NAME = "2026-10-05_1500_NET_127001.tore-capture"

MISSION = """tore-mission 1
theater UKR
condition clear
start airborne 10000
separation-nm 5
preset free
guns-only no
wing friendly 1 F18.PT 2 experienced
wing enemy 1 MIG29.PT 2 experienced
cheats none
"""


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def capture(work: Path) -> int:
    data = os.environ["TORE_DATA_DIR"]
    # A build must match to join, so the server and the bot are built at the
    # commit the game under test was (a no-op when they already are).
    built = subprocess.run(
        ["cargo", "build", "--locked", "-p", "tore-app", "-p", "tore-server", "-p", "tore-session"],
        cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=900,
    )
    if built.returncode != 0:
        print("cargo build failed:\n" + built.stdout[-2000:])
        return 1
    for name in ("tore-server", "tore-bot"):
        if not (BIN / name).exists():
            print(f"{BIN / name} is missing: cargo build --locked -p tore-app -p tore-server -p tore-session")
            return 1
    work.mkdir(parents=True, exist_ok=True)
    (work / "mission.txt").write_text(MISSION)
    port = free_port()
    (work / "server.conf").write_text(
        f"name capture-scenario\nport {port}\nmission {work / 'mission.txt'}\nstart first-player\n"
        "status-interval 0\nempty-timeout 5\nafter-end quit\n"
    )
    replays = work / "replays"
    replays.mkdir(exist_ok=True)
    log = open(work / "server.log", "w")
    server = subprocess.Popen(
        [str(BIN / "tore-server"), "--config", str(work / "server.conf"), "--data-dir", data],
        stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
    )
    try:
        deadline = time.time() + 90
        while time.time() < deadline:
            if server.poll() is not None:
                print("the server stopped before it listened:\n" + (work / "server.log").read_text())
                return 1
            if "Waiting for players" in (work / "server.log").read_text():
                break
            time.sleep(0.5)
        else:
            print("the server never listened:\n" + (work / "server.log").read_text())
            return 1
        bot = subprocess.run(
            [
                str(BIN / "tore-bot"), "--connect", f"127.0.0.1:{port}", "--data-dir", data,
                "--seconds", "25", "--callsign", "Viper", "--capture", str(replays / CAPTURE_NAME),
            ],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=120,
        )
        print(bot.stdout[-1500:])
        if bot.returncode != 0:
            print(f"the bot exited {bot.returncode}")
            return 1
    finally:
        if server.poll() is None:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
        log.close()
    size = (replays / CAPTURE_NAME).stat().st_size
    print(f"capture written: {size} bytes")
    return 0 if size > 100_000 else 1


def same(a: Path, b: Path) -> int:
    first, second = a.read_bytes(), b.read_bytes()
    print(f"identical: {'yes' if first == second else 'NO'} ({len(first)} and {len(second)} bytes)")
    return 0 if first == second else 1


def cut(src: Path, dest: Path, part: int) -> int:
    data = src.read_bytes()
    dest.write_bytes(data[: len(data) * part // 100])
    print(f"cut {src.name} at {part} percent: {len(data)} -> {dest.stat().st_size} bytes")
    return 0


def main(argv: list[str]) -> int:
    if len(argv) >= 2 and argv[0] == "capture":
        return capture(Path(argv[1]))
    if len(argv) == 3 and argv[0] == "same":
        return same(Path(argv[1]), Path(argv[2]))
    if len(argv) == 4 and argv[0] == "cut":
        return cut(Path(argv[1]), Path(argv[2]), int(argv[3]))
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
