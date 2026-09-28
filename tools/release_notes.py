"""Turn docs/release/vX.Y.Z.md into a GitHub release title and description.

Uses only the Python standard library. The release workflow runs it when the
pushed tag has a notes file:

    python3 tools/release_notes.py docs/release/v0.1.1.md --title
    python3 tools/release_notes.py docs/release/v0.1.1.md --body --ref v0.1.1

The title is the first line without its `# `. The body drops that line, because
GitHub already shows the title, and makes relative links absolute: a release
description has no folder of its own, so `../REPLAYS.md` would otherwise break.
"""

import argparse
from pathlib import Path
import re
import sys
from urllib.parse import urljoin, urlsplit

REPOSITORY = "john-overton/T.O.R.E-Fighters"

# An inline Markdown link or image, `[label](target)` or `![alt](target "title")`,
# split into the image mark, the label and the target.
LINK = re.compile(r"(!?)(\[[^\]]*\]\()([^)\s]+)")


def title(text):
    first = text.split("\n", 1)[0]
    if not first.startswith("# "):
        raise ValueError("release notes must start with a '# vX.Y.Z: Name' title")
    return first[2:].strip()


def absolute(target, base):
    if target.startswith("#") or urlsplit(target).scheme:
        return target
    return urljoin(base, target)


def body(text, ref, repository=REPOSITORY):
    # Links open the file on GitHub; images need its raw bytes.
    pages = f"https://github.com/{repository}/blob/{ref}/docs/release/"
    files = f"https://github.com/{repository}/raw/{ref}/docs/release/"

    def rewrite(match):
        image, label, target = match.groups()
        return image + label + absolute(target, files if image else pages)

    rest = text.split("\n", 1)[1] if "\n" in text else ""
    return LINK.sub(rewrite, rest).lstrip("\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("notes", type=Path, help="the release notes file")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--title", action="store_true", help="print the release title")
    mode.add_argument("--body", action="store_true", help="print the release description")
    parser.add_argument("--ref", default="main", help="tag or branch that relative links point at")
    arguments = parser.parse_args()

    text = arguments.notes.read_text(encoding="utf-8")
    sys.stdout.write(title(text) + "\n" if arguments.title else body(text, arguments.ref))
    return 0


if __name__ == "__main__":
    sys.exit(main())
