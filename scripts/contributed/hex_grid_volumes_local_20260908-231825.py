"""Probe hex-indexed GI volume and reflection-probe image names locally."""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import snapshot

SHAPE = re.compile(r"^volume(\d+)_state(\d+)_(.+)_([0-9a-f]{8})_([0-9a-f]{1,2})$")
names = set()
for table in ("fnv1a_ximages", "fnv1a_ximages_v2"):
    names.update(n.strip().lower().replace("\\", "/") for n in snapshot.table_names(table))
names.update(n.strip().lower().replace("\\", "/") for n in snapshot.confirmed_names("image"))
volumes, states, kinds, blobs = set(), set(), set(), set()
for name in names:
    match = SHAPE.match(name)
    if match:
        volumes.add(int(match.group(1)))
        states.add(int(match.group(2)))
        kinds.add(match.group(3))
        blobs.add(match.group(4))
if not blobs:
    raise SystemExit("no volume-family names in the local image corpus")
volumes = range(0, max(volumes) + 3)
states = range(0, max(states) + 2)
kinds |= {f"gi_xyz_texture_mip{i}" for i in range(6)}
indices = [f"{i:02x}" for i in range(256)] + [f"{i:x}" for i in range(16)]
candidates = sorted(
    f"volume{volume}_state{state}_{kind}_{blob}_{index}"
    for blob in blobs for volume in volumes for state in states for kind in kinds
    for index in indices
    if f"volume{volume}_state{state}_{kind}_{blob}_{index}" not in names
)
print(f"{len(blobs):,} blobs -> {len(candidates):,} hex-grid candidates", file=sys.stderr)
for candidate in candidates:
    print(candidate)
