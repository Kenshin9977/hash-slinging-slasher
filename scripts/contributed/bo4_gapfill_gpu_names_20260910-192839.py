r"""Names found by bo4_gapfill_gpu.cu (3090): for every zombies VO prefix
en\vox\scripted\zmb\<dir>\vox_<maptok>_<speaker>_ (players plr_N per crew + NPC/announcer tokens),
<t1>[_<t2>]_<v>.<sn100|ln100> over the 9,618-token vocabulary (game names + sound aliases + the
user's BO4 map guides). 64 prefixes x 92M token pairs x 34 suffixes in 32 s."""
import os
here = os.path.dirname(os.path.abspath(__file__))
for fn in sorted(f for f in os.listdir(here) if f.startswith("bo4_gapfill_gpu_") and f.endswith(".txt")):
    for l in open(os.path.join(here, fn), encoding="utf-8"):
        if l.strip(): print(l.strip())
