"""Generate measured counterparts across observed volume-family prefixes."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
VOLUMES = ("volume4_", "volume5_", "volume14_", "volume15_")


def names():
    out = set()
    for game in ("blkops04", "blkopscw"):
        for path in (ROOT / "all_names" / game).glob("*.txt"):
            for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
                _, sep, name = line.partition(",")
                if sep and name.strip():
                    out.add(name.strip().lower().replace("\\", "/"))
    return out


def main():
    known = names()
    candidates = set()
    for name in known:
        directory, sep, base = name.rpartition("/")
        first, marker, rest = base.partition("_")
        if not marker or first not in {v[:-1] for v in VOLUMES}:
            continue
        for volume in VOLUMES:
            candidate = f"{directory}{sep}{volume}{rest}" if sep else f"{volume}{rest}"
            if candidate not in known:
                candidates.add(candidate)
    for candidate in sorted(candidates):
        print(candidate)
    print(f"{len(known):,} known names, {len(candidates):,} volume-family counterparts", file=sys.stderr)


if __name__ == "__main__":
    main()
