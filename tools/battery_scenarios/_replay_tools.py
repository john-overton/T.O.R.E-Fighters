"""Helper commands the replay lane's scenarios run as follow-up steps.

    python3 _replay_tools.py mangle KIND SRC DEST
    python3 _replay_tools.py ppm PATH [--expect-ui] [--min-colors N]

`mangle` writes a damaged copy of a recording. `ppm` prints statistics of a
captured frame and exits non-zero when it looks blank, black or malformed.
"""
from __future__ import annotations

import os
import sys


def mangle(kind: str, src: str, dest: str) -> None:
    data = bytearray(open(src, "rb").read())
    size = len(data)
    if kind == "empty":
        data = bytearray()
    elif kind == "head100":
        data = data[:100]
    elif kind == "trunc-third":
        data = data[: size // 3]
    elif kind == "trunc-half":
        data = data[: size // 2]
    elif kind == "no-tail":
        data = data[: size - 500]
    elif kind == "no-last-byte":
        data = data[: size - 1]
    elif kind == "random":
        data = bytearray(os.urandom(size // 2))
    elif kind == "text":
        data = bytearray(b"hello, this is not a recording\n")
    elif kind == "flip":
        for off in (size // 10, size // 3, size // 2, size * 3 // 4):
            data[off] ^= 0xFF
    elif kind == "bad-magic":
        data[0:8] = b"XXXXXXXX"
    elif kind == "bad-version":
        data[8] = 99
    elif kind == "zero-fill":
        for i in range(size // 3, size // 3 + 4096):
            data[i] = 0
    elif kind == "duplicate-tail":
        data = data + data[-2000:]
    else:
        raise SystemExit(f"unknown mangle kind {kind}")
    open(dest, "wb").write(data)
    print(f"mangled {kind}: {size} -> {len(data)} bytes")


def media(kind: str, dest: str) -> None:
    """Builds damaged Fighters Anthology media at `dest` from the local retail copy (copy-on-write)."""
    import shutil
    import subprocess

    root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    source = os.path.join(root, "gameassets", "fighters-anthology")

    def clone(name):
        os.makedirs(dest, exist_ok=True)
        subprocess.run(["cp", "-a", "--reflink=auto", os.path.join(source, name), os.path.join(dest, name)], check=True)

    if kind == "nonexistent":
        return
    if kind == "text-file":
        open(dest, "w").write("this is a text file, not a game folder\n")
        return
    if kind == "iso-file":
        open(dest, "wb").write(b"\0" * 4096)
        return
    if kind == "empty-dir":
        os.makedirs(dest, exist_ok=True)
        return
    if kind == "junk-files":
        os.makedirs(dest, exist_ok=True)
        for name in ("FA_1.LIB", "FA_2.LIB", "FA.EXE"):
            open(os.path.join(dest, name), "wb").write(b"junk" * 100)
        return
    for name in ("FA_1.LIB", "FA_2.LIB", "FA.EXE"):
        if (kind, name) in (("no-exe", "FA.EXE"), ("no-fa2", "FA_2.LIB")):
            continue
        clone(name)
    if kind == "trunc-fa1":
        os.truncate(os.path.join(dest, "FA_1.LIB"), 20_000_000)
    elif kind == "trunc-fa2":
        os.truncate(os.path.join(dest, "FA_2.LIB"), 1_000_000)
    elif kind == "empty-fa1":
        os.truncate(os.path.join(dest, "FA_1.LIB"), 0)
    elif kind == "exe-flip":
        with open(os.path.join(dest, "FA.EXE"), "r+b") as f:
            f.seek(100001)
            f.write(b"\xfe")
    elif kind not in ("no-exe", "no-fa2"):
        raise SystemExit(f"unknown media kind {kind}")


def newest_replay(data: str, dest: str) -> int:
    """Copies the newest finished recording in the data folder's replays/ to `dest`."""
    import glob
    import shutil

    found = sorted(glob.glob(os.path.join(data, "replays", "*.tore-replay")), key=os.path.getmtime)
    partial = glob.glob(os.path.join(data, "replays", "*.partial"))
    print(f"recordings: {len(found)} finished, {len(partial)} partial")
    if not found:
        print("no finished recording in replays/")
        return 1
    shutil.copy(found[-1], dest)
    print(f"copied {os.path.basename(found[-1])}")
    return 0


def kill_run(seconds: float, command: list[str]) -> int:
    """Runs `command` and kills it dead (SIGKILL) after `seconds`, as a crash or power cut would."""
    import subprocess
    import time

    proc = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(seconds)
    if proc.poll() is not None:
        print(f"the run had already finished with {proc.returncode}: use a longer run")
        return 1
    proc.kill()
    proc.wait()
    print(f"killed after {seconds} s")
    return 0


def seed_replays(data: str, source: str) -> None:
    """Fills the data folder's replays/ with dated copies of `source` and settings that keep five."""
    import shutil

    folder = os.path.join(data, "replays")
    os.makedirs(folder, exist_ok=True)
    for day in range(11, 19):
        shutil.copy(source, os.path.join(folder, f"2026-09-{day}_1200_UKR_F18.tore-replay"))
    with open(os.path.join(folder, "2026-09-10_1200_UKR_F18.tore-replay"), "wb") as f:
        f.write(b"not a recording, only named like one\n")
    with open(os.path.join(folder, "notes.txt"), "w") as f:
        f.write("mine\n")
    with open(os.path.join(data, "replays-v1.conf"), "w") as f:
        f.write("tore-replays 1\nauto-delete on\nrule keep-last\nkeep-last 5\nolder-than-days 30\nkeep 2026-09-11_1200_UKR_F18.tore-replay\n")
    print("seeded 8 recordings, one impostor and one note")


def list_replays(data: str) -> None:
    folder = os.path.join(data, "replays")
    print("REPLAYS", sorted(os.listdir(folder)))


def read_ppm(path: str):
    raw = open(path, "rb").read()
    if not raw.startswith(b"P6"):
        raise ValueError("not a P6 PPM")
    parts = raw.split(None, 4)
    width, height, maxval = int(parts[1]), int(parts[2]), int(parts[3])
    body = raw[len(raw) - width * height * 3 :]
    if maxval != 255 or len(body) != width * height * 3:
        raise ValueError("bad PPM body size")
    return width, height, body


def ppm_problems(path: str, expect_ui: bool = False, min_colors: int = 20) -> list[str]:
    """What is wrong with a captured frame, in words; empty when it looks like a real picture."""
    try:
        w, h, body = read_ppm(path)
    except (ValueError, OSError) as e:
        return [f"{path}: {e}"]
    problems = []
    sample = [body[i : i + 3] for i in range(0, len(body) - 2, 21)]
    colors = set(sample)
    dark = sum(1 for p in sample if p[0] + p[1] + p[2] < 24)
    if len(colors) < min_colors:
        problems.append(f"only {len(colors)} distinct colors: looks blank")
    if dark / len(sample) > 0.97:
        problems.append("frame is almost entirely black")
    if w < 320 or h < 240:
        problems.append("frame is tiny")
    if expect_ui:
        strip = body[(h - 60) * w * 3 : h * w * 3]
        if len(set(strip[i : i + 3] for i in range(0, len(strip) - 2, 21))) < 6:
            problems.append("no timeline strip at the bottom")
    return [f"{path}: {p}" for p in problems]


def ppm_stats(path: str, expect_ui: bool, min_colors: int) -> int:
    problems = ppm_problems(path, expect_ui, min_colors)
    for p in problems:
        print(f"PPM problem: {p}")
    if not problems:
        print(f"ppm ok: {path}")
    return 1 if problems else 0


if __name__ == "__main__":
    if len(sys.argv) >= 4 and sys.argv[1] == "seed":
        seed_replays(sys.argv[2], sys.argv[3])
    elif len(sys.argv) >= 3 and sys.argv[1] == "list":
        list_replays(sys.argv[2])
    elif len(sys.argv) >= 5 and sys.argv[1] == "killrun":
        sys.exit(kill_run(float(sys.argv[2]), sys.argv[3:]))
    elif len(sys.argv) >= 4 and sys.argv[1] == "newest":
        sys.exit(newest_replay(sys.argv[2], sys.argv[3]))
    elif len(sys.argv) >= 4 and sys.argv[1] == "media":
        media(sys.argv[2], sys.argv[3])
    elif len(sys.argv) >= 5 and sys.argv[1] == "mangle":
        mangle(*sys.argv[2:5])
    elif len(sys.argv) >= 3 and sys.argv[1] == "ppm":
        rest = sys.argv[3:]
        mc = int(rest[rest.index("--min-colors") + 1]) if "--min-colors" in rest else 20
        sys.exit(ppm_stats(sys.argv[2], "--expect-ui" in rest, mc))
    else:
        raise SystemExit(__doc__)
