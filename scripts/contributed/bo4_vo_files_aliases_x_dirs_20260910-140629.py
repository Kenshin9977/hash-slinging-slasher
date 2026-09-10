"""BO4 sound_asset: every known vox_ alias x every VO directory ever observed (+ the zombies map
dirs), variants "" / _0.._15 / _00.._31, endings sn100 + ln100. Widening of
bo4_vo_files_from_aliases.py, whose per-alias dir guess (2nd token) misses aliases named after a
character (vox_mist_*, vox_stuh_*, vox_brun_*...) that live in the map's dir, not the character's."""
import os, sys
here = os.path.dirname(os.path.abspath(__file__)); root = os.path.dirname(here)
B = chr(92)
aliases = set()
for line in open(os.path.join(root, "all_names", "blkops04", "sound_alias.txt"), encoding="utf-8"):
    line = line.strip()
    if "," in line:
        a = line.split(",", 1)[1]
        if a.startswith("vox_") or "_vox_" in a:
            aliases.add(a)
dirs = set()
srcs = [os.path.join(root, "all_names", "blkops04", "sound_asset.txt"), os.path.join(here, "bo4_mitm2_names_20260910.txt")]
fd = os.path.join(root, "findings", "blkops04")
srcs += [os.path.join(fd, d, "sound_asset.txt") for d in os.listdir(fd) if d.startswith("run_")]
for fn in srcs:
    if not os.path.exists(fn): continue
    for line in open(fn, encoding="utf-8", errors="replace"):
        n = line.strip().split(",", 1)[-1]
        if B + "vox" + B in n:
            dirs.add(n.rpartition(B)[0])
for m in ["zod", "zodt8", "escape", "office", "towers", "mansion", "red", "orange", "white", "common", "cmn", "zm_common", "blackout", "wz"]:
    for mode in ("zmb", "mpl", "wz", "cp"):
        dirs.add(B.join(["en", "vox", "scripted", mode, m]))
variants = [""] + ["_%d" % i for i in range(16)] + ["_%02d" % i for i in range(32)]
tails = [".sn100.pc.snd", ".ln100.pc.snd"]
print(f"{len(aliases)} aliases x {len(dirs)} dirs x {len(variants)} x {len(tails)}", file=sys.stderr)
w = sys.stdout.write
for d in sorted(dirs):
    for a in aliases:
        for v in variants:
            for t in tails:
                w(d + B + a + v + t + "\n")
