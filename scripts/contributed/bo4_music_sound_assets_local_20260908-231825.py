#!/usr/bin/env python3
"""Generate BO4 music sound-file candidates from local vocabulary."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENCODINGS = (".ll100.pc.snd", ".ln100.pc.snd", ".sl100.pc.snd", ".sn100.pc.snd")


def lines(path):
    if not path.exists():
        return ()
    return path.read_text(encoding="utf-8", errors="replace").splitlines()


def generate():
    cores = set()
    for line in lines(ROOT / "all_names" / "blkops04" / "sound_alias.txt"):
        parts = line.strip().split(",")
        if len(parts) >= 2 and "mus" in parts[1].lower():
            cores.add(parts[1])
    for rel in (
        "scripts/contributed/bo3_cores_20260822-030351.txt",
        "scripts/contributed/bo1_cores_20260822-031028.txt",
    ):
        for line in lines(ROOT / rel):
            line = line.strip().lower()
            if "mus" in line:
                cores.add(line.split(".")[0].removesuffix("_l").removesuffix("_r"))
    for line in lines(ROOT / "cod-name-db" / "csv" / "bo2_sab.csv"):
        parts = line.strip().split(",")
        if len(parts) >= 2 and "mus" in parts[1].lower():
            cores.add(parts[1].strip().lower().replace("/", "\\").split(".")[0])

    seen = set()
    for raw in sorted(cores):
        raw = raw.strip().lower().replace("/", "\\")
        if not raw:
            continue
        bases = []
        if raw.startswith("mus\\"):
            bases.append(raw)
            if not raw.startswith("mus\\zmb\\"):
                bases.append("mus\\zmb\\" + raw[4:])
        else:
            for prefix in ("mus\\", "mus\\zmb\\", "mus\\mpl\\", "mus\\wz\\", "mus\\frontend\\"):
                bases.append(prefix + raw)
        for base in bases:
            for ending in ENCODINGS:
                candidate = base + ending
                if candidate not in seen:
                    seen.add(candidate)
                    print(candidate)


if __name__ == "__main__":
    generate()
