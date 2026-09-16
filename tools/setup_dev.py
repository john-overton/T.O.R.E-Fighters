"""Fresh-clone setup. Uses only the Python standard library.

Git hooks live in `.git/hooks/`, which is not version controlled, so a fresh
clone has none. This points Git at the committed `.githooks/` directory instead,
which installs the pre-push checks, and reports whether the toolchain the checks
need is present.

Run once per clone:

    python3 tools/setup_dev.py
"""

import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys


HOOKS_DIRECTORY = ".githooks"
REQUIRED_HOOKS = ("pre-push",)


def repository_root():
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    return Path(result.stdout.strip())


def install_hooks(root):
    """Point Git at the committed hooks directory and make the hooks runnable."""
    missing = [name for name in REQUIRED_HOOKS if not (root / HOOKS_DIRECTORY / name).is_file()]
    if missing:
        print(f"  !  {HOOKS_DIRECTORY}/ is missing: {', '.join(missing)}", file=sys.stderr)
        return False

    current = subprocess.run(
        ["git", "config", "core.hooksPath"], cwd=root, capture_output=True, text=True
    ).stdout.strip()
    if current == HOOKS_DIRECTORY:
        print(f"  ok core.hooksPath already set to {HOOKS_DIRECTORY}")
    else:
        subprocess.run(
            ["git", "config", "core.hooksPath", HOOKS_DIRECTORY], cwd=root, check=True
        )
        print(f"  ok core.hooksPath set to {HOOKS_DIRECTORY}")

    # Windows ignores the executable bit; everywhere else Git needs it.
    if os.name != "nt":
        for name in REQUIRED_HOOKS:
            path = root / HOOKS_DIRECTORY / name
            mode = path.stat().st_mode
            path.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        print(f"  ok {', '.join(REQUIRED_HOOKS)} executable")
    return True


def report_toolchain():
    """Name anything the pre-push checks need that is not installed."""
    complete = True
    for tool, hint in (
        ("cargo", "install rustup, then run `rustup show` in the repository"),
        ("git", "install Git"),
    ):
        if shutil.which(tool):
            print(f"  ok {tool}")
        else:
            print(f"  !  {tool} not on PATH: {hint}", file=sys.stderr)
            complete = False
    print(f"  ok python {sys.version.split()[0]}")
    return complete


def main():
    root = repository_root()
    if root is None:
        print("Not inside a Git repository.", file=sys.stderr)
        return 1

    print("Installing hooks:")
    hooks_ready = install_hooks(root)
    print("Checking the toolchain:")
    toolchain_ready = report_toolchain()

    if not hooks_ready:
        return 1
    print()
    print("Done. `git push` now runs the checks first and aborts if any fail.")
    print("Bypass a single push with `git push --no-verify`.")
    if not toolchain_ready:
        print("Install the missing tools above before the hook can run.")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
