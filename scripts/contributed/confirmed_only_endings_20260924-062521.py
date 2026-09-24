"""All-boundary cores, crossed with endings that appear ONLY on names this project has itself
confirmed -- never on any published name. The mirror of --confirmed-only (which restricts the
CORE side); this restricts the ENDING side instead.

Rationale: every all-boundary run so far takes its ending vocabulary from the union of published
and confirmed names. An ending that shows up only among names *we* found is evidence the general
search's own vocabulary never had it -- so crossing it against the full (published+confirmed) core
list reaches candidates no committed list, and no prior all-boundary run using the union endings
list, was ever built from.
"""
import collections
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent
while not (ROOT / "scripts" / "snapshot.py").exists() and ROOT != ROOT.parent:
    ROOT = ROOT.parent
sys.path.insert(0, str(ROOT / "scripts"))
import snapshot

import argparse

parser = argparse.ArgumentParser()
parser.add_argument("--sound", action="store_true")
parser.add_argument("--segments", type=int, default=2)
parser.add_argument("--min-core", type=int, default=8)
parser.add_argument("--top", type=int, default=100000)
parser.add_argument("--confirmed-only-cores", action="store_true",
                     help="also restrict the core side to names this project alone confirmed, "
                          "for a purely self-referential cross (both sides never published)")
args = parser.parse_args()

SOUND_TABLES = ["fnv1a_xsounds", "fnv1a_xsounds_v2",
                "fnv1a_soundbanks_aliases", "fnv1a_soundbanks_aliases_v2"]
GENERAL_TABLES = ["fnv1a_xmaterials", "fnv1a_xmaterials_v2", "fnv1a_ximages", "fnv1a_ximages_v2",
                  "fnv1a_xmodels", "fnv1a_xanims", "fnv1a_xanims_v2"]

published = snapshot.table_names(*(SOUND_TABLES if args.sound else GENERAL_TABLES))
confirmed = snapshot.confirmed_names()
carried_path = ROOT / "data" / ("sound.suffixes.txt" if args.sound else "suffixes.txt")
carried = {line.strip() for line in carried_path.read_text(encoding="utf-8").splitlines() if line.strip()}


def ending_of(name, segments):
    pieces = name.split("_")
    if len(pieces) <= segments:
        return None
    return "_" + "_".join(pieces[-segments:])


published_endings = set()
for name in published:
    e = ending_of(name, args.segments)
    if e:
        published_endings.add(e)

confirmed_counted = collections.Counter()
for name in confirmed:
    e = ending_of(name, args.segments)
    if not e or e in carried or e in published_endings:
        continue
    if not args.sound and "." in e:
        continue
    confirmed_counted[e] += 1

endings = [e for e, _ in confirmed_counted.most_common(args.top)]


def all_boundary_cores(name, min_core, sound):
    seps = "_/" + chr(92) + "." if sound else "_"
    for i, ch in enumerate(name):
        if ch in seps and i >= min_core:
            yield name[:i]


core_source = confirmed if args.confirmed_only_cores else published + confirmed
published_cores = set()
if args.confirmed_only_cores:
    for name in published:
        published_cores.update(all_boundary_cores(name, args.min_core, args.sound))

cores = set()
for name in core_source:
    cores.update(all_boundary_cores(name, args.min_core, args.sound))
if args.confirmed_only_cores:
    cores -= published_cores

stem = "sound_" if args.sound else ""
tag = "_selfonly" if args.confirmed_only_cores else ""
ends_path = ROOT / "contrib" / f"confirmedonly_ends_{stem}{tag}.txt"
cores_path = ROOT / "contrib" / f"confirmedonly_ends_cores_{stem}{tag}.txt"
ends_path.write_text(chr(10).join(endings) + chr(10), encoding="utf-8")
cores_path.write_text(chr(10).join(sorted(cores)) + chr(10), encoding="utf-8")
print(f"{len(confirmed_counted)} endings appear only on confirmed names (not published, not carried), "
      f"heading {sum(confirmed_counted.values())} confirmed names", file=sys.stderr)
print(f"{len(endings)} endings x {len(cores)} cores -> {len(endings) * len(cores):,} candidates",
      file=sys.stderr)
print(f"wrote {ends_path.name} and {cores_path.name}", file=sys.stderr)
