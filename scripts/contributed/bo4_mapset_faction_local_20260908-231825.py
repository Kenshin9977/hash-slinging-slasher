"""Generate BO4 map-set/faction substitutions from the local findings corpus."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HEADS = {"p7", "p8", "p9"}
FACTIONS = ("mp", "zm", "wz", "cp", "sp")
seen = set()
for path in (ROOT / "findings" / "blkops04").glob("*.txt"):
    if path.stem not in {"xmodel", "xanim", "image", "material"}:
        continue
    for raw in path.read_text(encoding="utf-8", errors="replace").splitlines():
        name = raw.partition(",")[2].strip().lower()
        bits = name.split("_")
        if len(bits) < 3 or bits[0] not in HEADS:
            continue
        for head in HEADS:
            for faction in FACTIONS:
                candidate = "_".join((head, faction, *bits[2:]))
                if candidate != name:
                    seen.add(candidate)
print(f"{len(seen):,} BO4 map-set/faction candidates", file=__import__("sys").stderr)
for candidate in sorted(seen):
    print(candidate)
