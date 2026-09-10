"""Swap first and last basename tokens in locally known sound-file names."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    game = sys.argv[1].lower()
    source = ROOT / "all_names" / game / "sound_asset.txt"
    known = set()
    for line in source.read_text(encoding="utf-8", errors="replace").splitlines():
        _, sep, name = line.partition(",")
        if sep and name.strip():
            known.add(name.strip().lower().replace("/", "\\"))
    output = set()
    for name in known:
        cut = name.rfind("\\") + 1
        head, base = name[:cut], name[cut:]
        core, sep, tail = base.partition(".")
        tokens = core.split("_")
        if not sep or len(tokens) < 2 or not tokens[0] or not tokens[-1]:
            continue
        tokens[0], tokens[-1] = tokens[-1], tokens[0]
        candidate = head + "_".join(tokens) + "." + tail
        if candidate not in known:
            output.add(candidate)
    for candidate in sorted(output):
        print(candidate)
    print(f"{game}: {len(known):,} local seeds, {len(output):,} outer-token candidates", file=sys.stderr)


if __name__ == "__main__":
    main()
