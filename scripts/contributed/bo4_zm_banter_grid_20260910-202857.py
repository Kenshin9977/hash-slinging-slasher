r"""BO4 zombies character-to-character conversations ("banter").

zm_vo.gsc (function_2b7b1675) builds the alias as
    vox_<idx>_<category>_<chr1>_<chr2>_plr_<n>  (+ "_" + line)      category defaults to "banter"
with <chr> = the character's chrname (zm_characters.gsc revivevox: brun dieg scar shaw | demp niko
rich take | udem unik uric utak | mist marl russ stuh | brig butl guns psyc = plr_1..20 in that
order). The shipped file reorders it exactly like the other player lines:
    en\vox\scripted\zmb\<dir>\vox_<maptok>_plr_<n>_<idx>_<category>_<chr1>_<chr2>_<v>.sn100.pc.snd
Verified on white (demp/niko/rich, idx 1-4) and zod (brun/dieg/scar/shaw, idx 1-4)."""
B = chr(92)
crews = {"chaos": (["brun","dieg","scar","shaw"], [1,2,3,4]), "primis": (["demp","niko","rich","take"], [5,6,7,8]),
         "dotn": (["brig","butl","guns","psyc"], [9,10,11,12]), "ultimis": (["udem","unik","uric","utak"], [13,14,15,16]),
         "victis": (["mist","marl","russ","stuh"], [17,18,19,20])}
maps = {("zod","zod"): ["chaos"], ("red","red"): ["chaos"], ("tow","tow"): ["chaos"], ("bod","bod"): ["primis"], ("fiv","fiv"): ["primis","ultimis"],
        ("man","man"): ["dotn"], ("white","whi"): ["primis","ultimis"], ("orange","oran"): ["victis","primis","ultimis"], ("common","cmn"): list(crews)}
cats = ["banter", "convo", "conv", "story", "rcnv", "quest"]
seen = set()
for (d, t), cl in maps.items():
    chars = [c for k in cl for c in crews[k][0]]; plrs = [n for k in cl for n in crews[k][1]]
    for n in plrs:
        for idx in range(0, 31):
            for c1 in chars:
                for c2 in chars:
                    if c1 == c2: continue
                    for cat in cats:
                        for v in range(0, 16):
                            for tail in (".sn100.pc.snd", ".ln100.pc.snd"):
                                s = B.join(["en","vox","scripted","zmb",d,f"vox_{t}_plr_{n}_{idx}_{cat}_{c1}_{c2}_{v}{tail}"])
                                print(s)
