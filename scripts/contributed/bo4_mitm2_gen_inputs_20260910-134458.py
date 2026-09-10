import collections, glob, os, sys
from load_ids import load_ids

BANK = sys.argv[1] if len(sys.argv) > 1 else "zm_office"
OUT = f"C:/tmp/bo4_vox/mitm_{BANK}"
os.makedirs(OUT, exist_ok=True)

def split_path(name):
    d, _, fn = name.rpartition("\\")
    stem, _, tail = fn.partition(".")
    return d, stem, tail

def core_and_nn(stem):
    parts = stem.rsplit("_", 1)
    if len(parts) == 2 and parts[1].isdigit():
        return tuple(parts[0].split("_")), parts[1]
    return tuple(stem.split("_")), None

pools = load_ids("C:/tmp/hash-slinging-slasher/snapshots/blkops04.ids")
p170 = pools[170]
known = {}
with open("C:/tmp/bo4_vox/known_bo4_files.txt", encoding="utf-8") as f:
    for line in f:
        h, n = line.rstrip("\n").split(",", 1)
        known[int(h, 16)] = n

# vocabulary: every '_'-separated token from every known BO4 file stem + dir segment + alias name
vocab = set()
for n in known.values():
    d, stem, tail = split_path(n)
    for seg in d.split("\\"):
        vocab.update(t for t in seg.split("_") if t)
    vocab.update(t for t in stem.split("_") if t)
with open("C:/tmp/hash-slinging-slasher/all_names/blkops04/sound_alias.txt", encoding="utf-8") as f:
    for line in f:
        line = line.strip()
        if "," in line:
            vocab.update(t for t in line.split(",", 1)[1].split("_") if t)
vocab.update(f"{i:02d}" for i in range(100))
vocab = sorted(t for t in vocab if len(t) <= 31)

hs = set()
for path in glob.glob(f"C:/tmp/bo4_vox/banks/{BANK}.*.hashes"):
    with open(path) as f:
        hs.update(int(l.strip(), 16) for l in f if l.strip())
hs &= p170
ks = {h: known[h] for h in hs if h in known}
us = sorted(h for h in hs if h not in known)

fam = collections.defaultdict(lambda: {"cores": set(), "nn": set()})
for n in ks.values():
    d, stem, tail = split_path(n)
    core, nn = core_and_nn(stem)
    fam[(d, tail)]["cores"].add(core)
    if nn is not None:
        fam[(d, tail)]["nn"].add(nn)

templates = set()
for (d, tail), info in fam.items():
    nns = sorted(info["nn"]) or [None]
    for core in info["cores"]:
        # free one slot (gap = 1 or 2 tokens, handled by the C++ side)
        for i in range(len(core)):
            pre = d + "\\" + ("_".join(core[:i]) + "_" if i > 0 else "")
            rest = "_".join(core[i + 1:])
            for nn in nns:
                suf = ("_" + rest if rest else "") + ("_" + nn if nn else "") + "." + tail
                templates.add((pre, suf))
        # free two adjacent slots (gap = 2 tokens replacing 2 known ones, or 1 replacing 2)
        for i in range(len(core) - 1):
            pre = d + "\\" + ("_".join(core[:i]) + "_" if i > 0 else "")
            rest = "_".join(core[i + 2:])
            for nn in nns:
                suf = ("_" + rest if rest else "") + ("_" + nn if nn else "") + "." + tail
                templates.add((pre, suf))
        # free the whole stem (dir + tail known, stem = 1 or 2 tokens [+ _NN])
        for nn in nns:
            templates.add((d + "\\", ("_" + nn if nn else "") + "." + tail))
        # free the NN slot too, with the stem kept: core_XX where XX any vocab token
        templates.add((d + "\\" + "_".join(core) + "_", "." + tail))

with open(f"{OUT}/templates.txt", "w", encoding="utf-8") as f:
    for pre, suf in sorted(templates):
        f.write(f"{pre}\t{suf}\n")
with open(f"{OUT}/vocab.txt", "w", encoding="utf-8") as f:
    f.write("\n".join(vocab) + "\n")
with open(f"{OUT}/targets.txt", "w") as f:
    for h in us:
        f.write(f"{h:016x}\n")
with open(f"{OUT}/known.txt", "w", encoding="utf-8") as f:
    for h, n in sorted(ks.items(), key=lambda kv: kv[1]):
        f.write(f"{h:016x},{n}\n")
print(f"bank {BANK}: in-bank {len(hs)} known {len(ks)} unknown {len(us)} families {len(fam)} templates {len(templates)} vocab {len(vocab)}")
