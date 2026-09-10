"""Transfer locally stored BO2/BO3 SAB stems to BO4's unfolded sound-file tails."""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
TAILS = ("ln100.pc.snd", "ll100.pc.snd", "sn100.pc.snd", "sl100.pc.snd")
LANGS = {
    "english", "russian", "french", "german", "spanish", "italian",
    "japanese", "polish", "korean", "chinese", "en", "ru", "fr", "de",
    "es", "it", "ja", "ko", "zh", "pl", "cz", "ar",
}


def main():
    stems = set()
    for filename in ("bo2_sab.csv", "bo3_sab.csv"):
        path = ROOT / "cod-name-db" / "csv" / filename
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            _, sep, value = line.partition(",")
            if not sep:
                continue
            stem = value.strip().lower().replace("/", "\\").split(".", 1)[0]
            parts = stem.split("\\")
            if parts and parts[0] == "devraw":
                parts = parts[1:]
            if parts and parts[0] in LANGS:
                parts = parts[1:]
            if parts:
                stems.add("\\".join(parts))
    for stem in sorted(stems):
        for tail in TAILS:
            print(stem + "." + tail)
    print(f"{len(stems):,} local SAB stems, {len(stems) * len(TAILS):,} candidates", file=sys.stderr)


if __name__ == "__main__":
    main()
