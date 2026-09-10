"""Substitute tokens using alternatives observed in the same local BO4 SAB directory."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "contrib" / "bo4_source_sound_paths_20260829.candidates.txt"


def main():
    by_dir = {}
    rows = []
    for raw in LEDGER.read_text(encoding="utf-8", errors="ignore").splitlines():
        name = raw.strip().lower().replace("/", "\\")
        cut = name.rfind("\\")
        if cut < 0:
            continue
        base = name[cut + 1:]
        dot = base.find(".")
        if dot <= 0:
            continue
        parts = base[:dot].split("_")
        if len(parts) < 3:
            continue
        directory, tail = name[:cut], base[dot:]
        rows.append((directory, parts, tail))
        by_dir.setdefault(directory, set()).update(parts[1:-1])
    out = set()
    for directory, parts, tail in rows:
        for index in range(1, len(parts) - 1):
            for token in by_dir[directory]:
                if token != parts[index]:
                    out.add(directory + "\\" + "_".join(parts[:index] + [token] + parts[index + 1:]) + tail)
    print(f"{len(rows):,} source paths, {len(out):,} contextual-token candidates", file=sys.stderr)
    print("\n".join(sorted(out)))


if __name__ == "__main__":
    main()
