"""BO4 reflection-probe images, wide grid: every probe id H known so far (published, plus the ones a
2^32 GPU sweep returned, one of them only reachable with volume -1), over volume -1..20 and state
-1..3. The first batch stopped at volume 4; published names go up to volume 18."""
H = ['14b73a34', '159b88cd', '182b90df', '18f8cd80', '196d969', '197298c4', '20e3b3', '22b27768', '24bab719', '281c46d', '2ad64101', '2c5edac6', '321af73b', '34230f66', '38034f1d', '3b298023', '3e934827', '4162703a', '44c1c796', '456b3986', '4768e824', '48306dbf', '49a36b1e', '4d87190', '4eac9ccf', '527ef4e2', '5464880a', '591e02e1', '5d869720', '62d31013', '66cd321e', '67e0ab65', '736cab5f', '74af6bb6', '78c358d6', '7adfc38e', '7e1b282f', '8996aed8', '8ce7f3e7', '924092fc', '93d2bf2e', '9dc66648', 'a1d9ed2a', 'a47e1566', 'a6060e6f', 'a657c255', 'a754f9f9', 'a7adec20', 'a8aab037', 'a9812e27', 'b371190c', 'b4bf4583', 'b5009b4f', 'b9195314', 'b9485d5b', 'bb068168', 'bfdce88c', 'c016f344', 'c0b81dd8', 'c53a059d', 'c63435b4', 'c6d3e98e', 'c8433041', 'ca766509', 'cd1642e9', 'd4a6913f', 'd52c41ba', 'd6853678', 'd7d7c72c', 'e54b9893', 'ec5b0b30', 'ee40ac2', 'f48b95d9', 'f52de995', 'f788ac97', 'fc30dbbf', 'fef3cbc4', 'ff32e91']
for h in H:
    for v in range(-1, 21):
        for s in range(-1, 4):
            base = "volume%d_state%d_reflection_probes_%s" % (v, s, h)
            for pre in ("mc/mtl_", "mc/", "i_"):
                print(pre + base)
            for i in range(0, 3000):
                print("%s_%d" % (base, i))
