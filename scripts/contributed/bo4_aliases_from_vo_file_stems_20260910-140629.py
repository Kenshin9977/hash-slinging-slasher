"""BO4 sound_alias: the reverse of bo4_vo_files_from_aliases.py - a recovered VO/SFX file stem
minus its trailing _N is (often) the alias name. Prints the stems of every known/recovered BO4
sound file, with and without the variant index."""
import os, re, sys
here = os.path.dirname(os.path.abspath(__file__)); root = os.path.dirname(here)
B = chr(92); seen = set()
srcs = [os.path.join(root, "all_names", "blkops04", "sound_asset.txt"), os.path.join(here, "bo4_mitm2_names_20260910.txt")]
srcs += [os.path.join(root, "findings", "blkops04", d, "sound_asset.txt") for d in os.listdir(os.path.join(root, "findings", "blkops04")) if d.startswith("run_")]
for fn in srcs:
    if not os.path.exists(fn): continue
    for line in open(fn, encoding="utf-8", errors="replace"):
        line = line.strip()
        n = line.split(",", 1)[1] if "," in line else line
        if B not in n: continue
        stem = n.rpartition(B)[2].partition(".")[0]
        for c in (stem, re.sub(r"_\d+$", "", stem)):
            if c not in seen:
                seen.add(c); print(c)
