"""Expand only numeric SAB basename families already present in BO4 sound assets."""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
TAILS = ("ln100", "ll100", "sn100", "sl100", "rn75", "ln75", "rn100", "rr100")


def main():
    known = set()
    source = ROOT / "all_names" / "blkops04" / "sound_asset.txt"
    for line in source.read_text(encoding="utf-8", errors="replace").splitlines():
        _, sep, name = line.partition(",")
        if sep and name.strip().lower().endswith(".snd"):
            known.add(name.strip().lower().replace("/", "\\"))
    out = set()
    for name in known:
        stem = name.split(".", 1)[0]
        if not stem:
            continue
        match = re.match(r"^(.*)_([0-9]+)$", stem)
        if not match:
            continue
        prefix, digits = match.groups()
        width = len(digits)
        for number in range(35):
            for form in {str(number), f"{number:02d}", f"{number:0{width}d}"}:
                for encoding in TAILS:
                    out.add(f"{prefix}_{form}.{encoding}.pc.snd")
    for candidate in sorted(out):
        print(candidate)
    print(f"{len(known):,} known sound assets, {len(out):,} numeric-family candidates", file=sys.stderr)


if __name__ == "__main__":
    main()
