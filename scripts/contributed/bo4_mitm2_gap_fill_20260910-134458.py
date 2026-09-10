"""BO4 sound_asset: meet-in-the-middle 1-2 token gap fill inside families that already have a name.

Method (see bo4_mitm2_gap_fill.cpp, the actual search, and bo4_mitm2_gen_inputs.py, the per-bank
input builder):
  - Every SAB bank's name table (.sabl/.sabs, v21, 128-byte stride, 8-byte 63-bit id per slot) says
    which unnamed ids sit next to which named files. Templates come from the named ones only:
    free 1 slot, 2 adjacent slots, the whole stem, or the trailing _NN, keeping dir + ending.
  - Gap = tok1 or tok1_tok2, tokens = every '_'-token of every known BO4 file path + alias name +
    00..99 (3,462).  FNV-1a is invertible per byte, so: hash prefix+tok1(+_) forward, unhash the
    target through ending(+tok2) backward, match 64-bit states, then recompute + verify the 63-bit
    id. ~1e10-1e11 candidates/bank vs 2^63 -> ~0 coincidental matches; both bit-63 variants tried.
  - CPU only (OpenMP), whole game ~3.5h; 794 new names, 761 of them VO grids (zod player VO,
    zombies/MP announcers) - the slice a short-gap (<=8 char) lattice search cannot reach.
This wrapper prints the verified list so confirm_list can file it.
"""
import os
here = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(here, "bo4_mitm2_names_20260910.txt"), encoding="utf-8") as f:
    for line in f:
        line = line.strip()
        if "," in line:
            print(line.split(",", 1)[1])
