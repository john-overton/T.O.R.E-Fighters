"""Inventory local Fighters Anthology archives without extracting retail payloads."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct


def inventory(path):
    data = path.read_bytes()
    if len(data) < 7 or data[:5] != b"EALIB":
        raise ValueError(f"{path}: not an EALIB archive")
    count = struct.unpack_from("<H", data, 5)[0]
    directory_end = 7 + (count + 1) * 18
    if directory_end > len(data):
        raise ValueError(f"{path}: truncated directory")
    entries = []
    for i in range(count + 1):
        name, flag, offset = struct.unpack_from("<13sBI", data, 7 + i * 18)
        name = name.split(b"\0", 1)[0].decode("ascii")
        if i == count:
            if name or flag or offset != len(data):
                raise ValueError(f"{path}: invalid sentinel")
            break
        end = struct.unpack_from("<I", data, 7 + (i + 1) * 18 + 14)[0]
        if offset < directory_end or end < offset or end > len(data) or flag not in (0, 4):
            raise ValueError(f"{path}/{name}: invalid entry")
        if flag == 4 and end - offset < 6:
            raise ValueError(f"{path}/{name}: truncated compressed header")
        entries.append({
            "name": name, "flag": flag, "offset": offset, "stored_bytes": end - offset,
            "decoded_bytes": struct.unpack_from("<I", data, offset)[0] if flag == 4 else end - offset,
            "dcl_header": data[offset + 4:offset + 6].hex() if flag == 4 else None,
        })
    return {
        "archive": path.name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
        "entry_count": len(entries),
        "extensions": dict(sorted(Counter(Path(e["name"]).suffix.upper().lstrip(".") for e in entries).items())),
        "flags": dict(Counter(e["flag"] for e in entries)),
        "entries": entries,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, nargs="?", default=Path("gameassets/fighters-anthology"))
    parser.add_argument("--out", type=Path, default=Path(".local/exploration/inventory.json"))
    args = parser.parse_args()
    paths = sorted(p for p in args.source.iterdir() if p.is_file() and p.suffix.lower() == ".lib")
    if not paths:
        parser.error("No LIB archives found")
    report = [inventory(path) for path in paths]
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2) + "\n")
    for archive in report:
        print(f"{archive['archive']}: {archive['entry_count']} entries, {archive['bytes']} bytes")
        print("  " + ", ".join(f"{ext}: {count}" for ext, count in archive["extensions"].items()))
    print(f"Inventory with archive hashes and entry offsets: {args.out}")


if __name__ == "__main__":
    main()
