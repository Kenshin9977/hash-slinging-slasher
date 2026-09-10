r"""BO4 zombies Aether-story player VO grid.

zm_audio.gsc builds a line as  <suffix> + "_plr_" + <character index> + "_" + <variant>, with
<suffix> = column 3 of gamedata/audio/zm/zm_<map>_vox.csv (hashed in the acts dump - the hash is
plain fnv1a-63 of e.g. "vox_power_switch", 221 of 650 cracked from the sound vocabulary, see
bo4_zm_vox_csv_suffixes_cracked.txt). The shipped *file* reorders that with a map token:
    en\vox\scripted\zmb\<tok>\vox_<tok>_plr_<n>_<event>_<v>.sn100.pc.snd
Chaos crew maps use plr_1-4 (zod/red/tow); the Aether crew is plr_5-8 (zm_office.gsc:
vox_plr_5_exert_pain .. plr_8), Primis/Ultimis/Victis push the index further. Map tokens found
by brute force: bod = Blood of the Dead (zm_escape). Events = that map's cracked suffixes + the
common table's events (shared with the Chaos maps, bo4_zm_chaos_vo_events.txt)."""
import os
here = os.path.dirname(os.path.abspath(__file__))
B = chr(92)
MAPS = {"bod": "zm_escape", "man": "zm_mansion"}   # token -> csv map name; man = Dead of the Night (plr_9-12)
PLR = list(range(0, 17))
common = {l.strip() for l in open(os.path.join(here, "bo4_zm_chaos_vo_events.txt"), encoding="utf-8") if l.strip()}
common |= {l.strip() for l in open(os.path.join(here, "bo4_zm_aether_vo_events.txt"), encoding="utf-8") if l.strip()}
common |= {"exert_pain", "power_switch", "magicbox", "pickup_generic", "kill_headshot", "oh_shit", "perk_generic"}
permap = {}
for l in open(os.path.join(here, "bo4_zm_vox_csv_suffixes_cracked.txt"), encoding="utf-8"):
    h, s, maps = l.strip().split(",")
    for mp in maps.split(";"): permap.setdefault(mp, set()).add(s[4:])
for tok, mp in MAPS.items():
    for e in sorted(common | permap.get(mp, set())):
        for n in PLR:
            for v in range(0, 16):
                for t in (".sn100.pc.snd", ".ln100.pc.snd"):
                    print(B.join(["en", "vox", "scripted", "zmb", tok, f"vox_{tok}_plr_{n}_{e}_{v}{t}"]))

for l in open(os.path.join(here, "bo4_mitm2_round3_names_20260910.txt"), encoding="utf-8"):
    if l.strip(): print(l.strip())
