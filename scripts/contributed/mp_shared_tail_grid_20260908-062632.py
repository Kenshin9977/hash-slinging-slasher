"""Fill mp_ operator-skin cells using tails observed with more than one operator axis."""
import collections
import os
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
while ROOT != os.path.dirname(ROOT) and not os.path.isfile(os.path.join(ROOT, "scripts", "snapshot.py")):
    ROOT = os.path.dirname(ROOT)
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import snapshot

TABLES = ("fnv1a_xmaterials", "fnv1a_xmaterials_v2", "fnv1a_ximages", "fnv1a_ximages_v2",
          "fnv1a_xmodels", "fnv1a_xanims", "fnv1a_xanims_v2",
          "fnv1a_soundbanks_aliases", "fnv1a_soundbanks_aliases_v2")


def main():
    known = {n.strip().lower().replace("\\", "/") for n in snapshot.table_names(*TABLES)}
    known.update(n.strip().lower().replace("\\", "/") for n in snapshot.confirmed_names())
    known.discard("")
    axes = set()
    tails = collections.Counter()
    for name in known:
        if not name.startswith("mp_") or "/" in name or "." in name:
            continue
        parts = name.split("_", 2)
        if len(parts) == 3 and parts[1] and parts[2]:
            axes.add(parts[1])
            tails[parts[2]] += 1
    shared = {tail for tail, count in tails.items() if count > 1}
    out = sorted(f"mp_{axis}_{tail}" for axis in axes for tail in shared
                 if f"mp_{axis}_{tail}" not in known)
    for name in out:
        print(name)
    print("mp shared-tail grid: %d axes x %d shared tails = %d unseen cells" %
          (len(axes), len(shared), len(out)), file=sys.stderr)


if __name__ == "__main__":
    main()
