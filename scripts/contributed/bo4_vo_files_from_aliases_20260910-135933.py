"""BO4 sound_asset: VO files derived from known sound_alias names.

The community-measured 0.7% alias/file-stem overlap holds for SFX (bare alias names vs deep paths)
but not for scripted VO: a dialogue alias `vox_<map>_<...>` is stored as
    en\vox\scripted\<mode>\<map>\<alias>_<N>.sn100.pc.snd
i.e. the alias itself plus a variant index, in a per-map dir named by the alias's 2nd token.
Modes tried: zmb, mpl, cp, wz. Variants: "", _0.._15, _00.._31 (known VO peaks at _0.._5, max
seen 12). Endings: sn100 (97%) and ln100. Each candidate is computed from one alias, not a cross
product; ~3M candidates. First naive pass gave 10,320 new pocket-170 names in 30 s.
"""
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
variants = [""] + ["_%d" % i for i in range(16)] + ["_%02d" % i for i in range(32)]
tails = [".sn100.pc.snd", ".ln100.pc.snd"]
modes = ["zmb", "mpl", "cp", "wz"]
n = 0
for a in sorted(aliases):
    parts = a.split("_")
    if len(parts) < 3:
        continue
    m = parts[1] if parts[0] == "vox" else parts[parts.index("vox") + 1] if "vox" in parts[:-1] else None
    if not m:
        continue
    for mode in modes:
        d = B.join(["en", "vox", "scripted", mode, m]) + B
        for v in variants:
            for t in tails:
                sys.stdout.write(d + a + v + t + "\n"); n += 1
print(f"{len(aliases)} aliases -> {n} candidates", file=sys.stderr)
