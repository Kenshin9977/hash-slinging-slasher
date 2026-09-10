r"""BO4 zombies non-player speakers, same grid as the player VO:
    en\vox\scripted\zmb\<dir>\vox_<maptok>_<speaker>_<event>_<v>.sn100.pc.snd
Speaker tokens found by GPU brute force over [a-z0-9_]{2..5} (bo4_spkfind_gpu.cu, 0.2 s per
69M tokens on a 3090) with the events already known: rush = Rushmore, avog/marl/mcca/ncom
(Alpha Omega), herm = the hermit (Pablo), apot, sama = Samantha (Tag der Toten), butd (Dead of
the Night), prst (IX), avoa/prst/ward/zmba = announcers (common). Events = every event known
so far (all maps); the MITM gap fill grows each speaker's own event list afterwards."""
import os
here = os.path.dirname(os.path.abspath(__file__))
B = chr(92)
SPK = {("white", "whi"): ["rush", "avog", "marl", "mcca", "ncom"], ("orange", "oran"): ["herm", "apot", "sama"],
       ("man", "man"): ["butd"], ("tow", "tow"): ["prst"], ("common", "cmn"): ["avoa", "prst", "ward", "zmba", "rush"],
       ("zod", "zod"): ["prst", "ward", "rush"], ("red", "red"): ["prst", "ward"], ("bod", "bod"): ["ward", "brut", "rush"], ("fiv", "fiv"): ["rush", "ward"]}
ev = set()
for fn in ("bo4_zm_chaos_vo_events.txt", "bo4_zm_aether_vo_events.txt"):
    ev |= {l.strip() for l in open(os.path.join(here, fn), encoding="utf-8") if l.strip()}
for l in open(os.path.join(here, "bo4_zm_vox_csv_suffixes_cracked.txt"), encoding="utf-8"):
    ev.add(l.split(",")[1][4:])
for (d, tok), spks in SPK.items():
    for s in spks:
        for e in sorted(ev):
            for v in range(0, 16):
                for t in (".sn100.pc.snd", ".ln100.pc.snd"):
                    print(B.join(["en", "vox", "scripted", "zmb", d, f"vox_{tok}_{s}_{e}_{v}{t}"]))
