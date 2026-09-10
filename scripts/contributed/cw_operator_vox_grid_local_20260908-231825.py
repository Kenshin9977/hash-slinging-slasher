"""Generate the Cold War operator voice grid from the local alias table."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ALIAS = ROOT / "all_names" / "blkopscw" / "sound_alias.txt"
known = set()
speakers = set()
suffixes = set()
counts = {}
for raw in ALIAS.read_text(encoding="utf-8", errors="ignore").splitlines():
    parts = raw.split(",", 1)
    if len(parts) != 2:
        continue
    name = parts[1].strip().lower()
    known.add(name)
    match = re.match(r"^vox_([a-z0-9]+)_(.+)$", name)
    if match:
        speaker, suffix = match.groups()
        speakers.add(speaker)
        suffixes.add(suffix)
        counts[speaker] = counts.get(speaker, 0) + 1
speakers = {speaker for speaker in speakers if counts[speaker] >= 10}
candidates = sorted(
    f"vox_{speaker}_{suffix}"
    for speaker in speakers
    for suffix in suffixes
    if f"vox_{speaker}_{suffix}" not in known
)
print(f"{len(speakers):,} speakers x {len(suffixes):,} suffixes -> {len(candidates):,} candidates", file=sys.stderr)
for candidate in candidates:
    print(candidate)
