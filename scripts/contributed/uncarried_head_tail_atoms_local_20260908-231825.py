"""Target omitted leading heads with locally measured omitted tail atoms."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import snapshot

TABLES = ("fnv1a_xmaterials", "fnv1a_xmaterials_v2", "fnv1a_ximages",
          "fnv1a_ximages_v2", "fnv1a_xanims", "fnv1a_xmodels",
          "fnv1a_soundbanks_aliases", "fnv1a_xsounds")
TAILS = ("futz", "kill", "nag", "hv", "exerts", "greet", "riot", "stab")


def main():
    known = set()
    for table in TABLES:
        known.update(n.strip().lower().replace("\\", "/") for n in snapshot.table_names(table))
    for pool in ("image", "material", "anim", "model", "sound_alias", "sound_asset"):
        known.update(n.strip().lower().replace("\\", "/") for n in snapshot.confirmed_names(pool))
    carried = {line.strip().lower() for line in (ROOT / "data" / "prefixes.txt").read_text().splitlines() if line.strip()}
    heads = set()
    for name in known:
        if "/" in name or "." in name or "_" not in name:
            continue
        head = name.split("_", 1)[0] + "_"
        if not any(head[:cut] in carried for cut in range(1, len(head) + 1)):
            heads.add(head)
    candidates = set()
    for name in known:
        if name.split("_", 1)[0] + "_" not in heads:
            continue
        base, sep, _ = name.rpartition("_")
        if not sep or not base:
            continue
        for tail in TAILS:
            candidate = base + "_" + tail
            if candidate not in known:
                candidates.add(candidate)
    print(f"{len(heads):,} omitted heads, {len(candidates):,} candidates", file=sys.stderr)
    for candidate in sorted(candidates):
        print(candidate)


if __name__ == "__main__":
    main()
