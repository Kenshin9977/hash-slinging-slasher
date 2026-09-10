"""Emit asset-shaped literals embedded in this repository's own code and plans.

This is deliberately separate from table recombination: scripts and plans often preserve
literal names copied from tools or game-facing configuration, while findings and generated
tables are excluded so the input is not a closed loop.
"""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SCAN = (ROOT / "scripts", ROOT / "contrib", ROOT / "plans")
EXTENSIONS = {".py", ".rs", ".toml", ".md", ".txt", ".json", ".csv"}
TOKEN = re.compile(r"(?<![A-Za-z0-9])([A-Za-z][A-Za-z0-9_./\\-]{7,180})(?![A-Za-z0-9])")
NOISE = {
    "something_else", "confirm_plan", "confirm_list", "derive_closure",
    "scripts_contributed", "black_ops", "cold_war", "sound_asset", "sound_alias",
}


def plausible(value):
    value = value.lower().replace("\\", "/").strip("./-")
    if value in NOISE or value.startswith(("http/", "https/", "python/", "target/")):
        return False
    if "_" not in value and "/" not in value:
        return False
    if sum(ch.isalpha() for ch in value) < 4:
        return False
    return not any(part in {"contrib", "scripts", "findings", "submissions"}
                   for part in value.split("/"))


def main():
    names = set()
    files = 0
    for base in SCAN:
        for path in base.rglob("*"):
            if (not path.is_file() or path.suffix.lower() not in EXTENSIONS
                    or path.stat().st_size > 2_000_000):
                continue
            files += 1
            try:
                text = path.read_text(encoding="utf-8", errors="ignore")
            except OSError:
                continue
            for match in TOKEN.finditer(text):
                value = match.group(1)
                if plausible(value):
                    names.add(value.lower().replace("\\", "/"))
    print(f"{files:,} repository files -> {len(names):,} asset-shaped literals", file=__import__("sys").stderr)
    for name in sorted(names):
        print(name)


if __name__ == "__main__":
    main()
