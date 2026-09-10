"""Build a Cold War uncarried-prefix plan from Black Ops 4 core vocabulary."""
from collections import Counter
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import seams
import snapshot

TABLES = ("fnv1a_xmaterials", "fnv1a_ximages", "fnv1a_xmodels", "fnv1a_xanims")


def reductions(name):
    name = name.strip().lower().replace("\\", "/")
    return {
        fn(name) for fn in dict(seams.REDUCTIONS).values()
        if len(fn(name)) >= 4
    }


def main():
    carried = {
        line.strip().lower()
        for line in (ROOT / "data" / "prefixes.txt").read_text(encoding="utf-8").splitlines()
        if line.strip()
    }
    cw = {name.strip().lower().replace("\\", "/") for name in snapshot.table_names(*TABLES) if name.strip()}
    bo4 = {name.strip().lower().replace("\\", "/") for name in snapshot.table_names(*TABLES) if name.strip()}
    counts = Counter()
    for name in cw:
        for index, char in enumerate(name):
            if char in "_/" and index < 40:
                counts[name[: index + 1]] += 1
    begins = [head for head, _ in sorted(
        ((head, count) for head, count in counts.items() if head not in carried),
        key=lambda item: (-item[1], item[0]),
    )[:200]]
    stems = sorted({core for name in bo4 for core in reductions(name)})[:100000]
    (ROOT / "contrib").mkdir(exist_ok=True)
    (ROOT / "contrib" / "cw_cross_title_begins.txt").write_text("\n".join(begins) + "\n", encoding="utf-8")
    (ROOT / "contrib" / "cw_cross_title_stems.txt").write_text("\n".join(stems) + "\n", encoding="utf-8")
    plan = ROOT / "plans" / "cw_uncarried_begins_cross_title.txt"
    plan.write_text(
        "label: Cold War uncarried beginnings over BO4 cores\n"
        "describe: Cold War leading cuts absent from the carried list over BO4-derived cores\n\n"
        "begin: @contrib/cw_cross_title_begins.txt\n"
        "stem: @contrib/cw_cross_title_stems.txt\n"
        "end: @data/suffixes.txt\n\n"
        "bare: no\nfold: yes\n",
        encoding="utf-8",
    )
    print(f"{len(begins)} beginnings, {len(stems)} cores, {len(begins) * len(stems) * 4629:,} candidates", file=sys.stderr)
    print(plan)


if __name__ == "__main__":
    main()
