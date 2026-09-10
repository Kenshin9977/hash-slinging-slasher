"""BO4 zombies (Chaos story) player VO grid:
    en(bs)vox(bs)scripted(bs)zmb(bs)<map>(bs)vox_<map>_plr_<0-4>_<event>_<v>.sn100.pc.snd
map tokens found by brute force (zod = Voyage of Despair, red = Ancient Evil, tow = IX); the
event vocabulary (bo4_zm_chaos_vo_events.txt) came out of a meet-in-the-middle 1-2 token gap
fill inside each map's SAB bank (bo4_mitm2_gap_fill.cpp), and transfers across the three maps.
Also prints the raw MITM round-2 hits for red/tow (bo4_mitm2_round2_names_20260910.txt)."""
import os
here = os.path.dirname(os.path.abspath(__file__))
B = chr(92)
events = [l.strip() for l in open(os.path.join(here, "bo4_zm_chaos_vo_events.txt"), encoding="utf-8") if l.strip()]
for mp in ("zod", "red", "tow", "man", "mansion", "dotn"):
    for n in range(0, 5):
        for ev in events:
            for v in range(0, 16):
                for t in (".sn100.pc.snd", ".ln100.pc.snd"):
                    print(B.join(["en", "vox", "scripted", "zmb", mp, f"vox_{mp}_plr_{n}_{ev}_{v}{t}"]))
for l in open(os.path.join(here, "bo4_mitm2_round2_names_20260910.txt"), encoding="utf-8"):
    if l.strip(): print(l.strip())
