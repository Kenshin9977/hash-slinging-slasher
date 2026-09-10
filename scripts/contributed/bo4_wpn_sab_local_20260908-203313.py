"""Probe local BO4 weapon SAB directories with observed action/tail vocabulary."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
ACTIONS = (
    "start", "stop", "loop", "fire", "shot", "lfe", "loop_lfe",
    "start_act", "start_ads", "stop_act", "stop_ads", "loop_ads",
    "fire_act", "fire_ads", "shot_act", "shot_ads", "tail", "tail_ext",
    "tail_int", "mech", "decay", "first", "last", "burst", "burst_fire",
)
TAILS = ("ln100.pc.snd", "ll100.pc.snd", "sn100.pc.snd", "sl100.pc.snd")


def main():
    source = ROOT / "scripts" / "contributed" / "bo4_snd_dirs_20260823-030223.txt"
    directories = {
        line.strip().lower().rstrip("\\")
        for line in source.read_text(encoding="utf-8", errors="replace").splitlines()
        if line.strip().lower().startswith("wpn\\")
        and len(line.strip().split("\\")) >= 3
        and line.strip().split("\\")[2]
    }
    output = set()
    for directory in directories:
        parts = directory.split("\\")
        weapon = parts[2]
        sub = "_".join(parts[3:])
        prefixes = {f"wpn_{weapon}", weapon}
        if sub:
            prefixes.update({f"wpn_{weapon}_{sub}", f"{weapon}_{sub}"})
        for prefix in prefixes:
            for action in ACTIONS:
                for tail in TAILS:
                    output.add(f"{directory}\\{prefix}_{action}.{tail}")
    for candidate in sorted(output):
        print(candidate)
    print(f"{len(directories):,} local weapon directories, {len(output):,} candidates", file=sys.stderr)


if __name__ == "__main__":
    main()
