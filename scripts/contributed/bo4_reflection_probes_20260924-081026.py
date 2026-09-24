"""BO4 reflection-probe images (pocket 9): volume<v>_state<s>_reflection_probes_<H>_<i>.
<H> is a per-map id printed as bare hex (no leading zeros) that no string anywhere carries; the 38
already published ones reached nothing new, so every 32-bit value was swept on a GPU
(bo4_reflection_probes_sweep.cu, 5 s on a 3090, two published names found back as controls). The
38 new ids it returned are below; each map's full grid is then just an enumeration. Where these
images sit: by value inside the map's `lighting` asset (read with Greyhoundx's zone walker)."""
H = ['182b90df', '196d969', '20e3b3', '22b27768', '24bab719', '2c5edac6', '34230f66', '38034f1d', '3b298023', '49a36b1e', '4d87190', '4eac9ccf', '527ef4e2', '5464880a', '591e02e1', '5d869720', '62d31013', '66cd321e', '67e0ab65', '736cab5f', '78c358d6', '7adfc38e', '8996aed8', '8ce7f3e7', '93d2bf2e', 'a47e1566', 'a6060e6f', 'a657c255', 'a754f9f9', 'bfdce88c', 'c0b81dd8', 'c53a059d', 'c6d3e98e', 'd4a6913f', 'd52c41ba', 'd6853678', 'f52de995', 'fc30dbbf']
for h in H:
    for v in range(0, 4):
        for s in range(0, 4):
            base = "volume%d_state%d_reflection_probes_%s" % (v, s, h)
            for i in range(0, 3000):
                print("%s_%d" % (base, i))
