"""Probe uncarried volume4/volume5 asset heads using known volume-family stems."""
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import snapshot

TABLES = ("fnv1a_xmodels", "fnv1a_xmaterials", "fnv1a_ximages", "fnv1a_xanims")
HEADS = ("volume4_", "volume5_")


def main():
    known = set()
    for table in TABLES:
        known.update(n.strip().lower().replace("\\", "/") for n in snapshot.table_names(table))
    known.update(n.strip().lower().replace("\\", "/") for n in snapshot.confirmed_names())
    endings = {
        line.strip().lower().replace("\\", "/")
        for line in open(os.path.join(ROOT, "data", "suffixes.txt"), encoding="utf-8")
        if line.strip()
    }
    out = set()
    for head in HEADS:
        stems = {name[len(head):] for name in known if name.startswith(head) and len(name) > len(head) + 3}
        for stem in stems:
            for ending in endings:
                candidate = head + stem + ending.lstrip("_")
                if candidate not in known:
                    out.add(candidate)
    print(f"{len(HEADS)} volume heads, {len(out):,} candidates", file=sys.stderr)
    for candidate in sorted(out):
        print(candidate)


if __name__ == "__main__":
    main()
