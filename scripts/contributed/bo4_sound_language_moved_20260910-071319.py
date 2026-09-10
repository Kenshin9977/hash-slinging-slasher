#!/usr/bin/env python3
"""Black Ops 4 voice-over, out of the per-language tables, which put the language at the other end.

    python contrib/bo4_sound_language_moved.py | confirm_list - --no-fold \
        --script contrib/bo4_sound_language_moved.py

`cod-name-db` carries twelve `fnv1a_<language>_xsounds.csv`, about 130,000 rows each, and they are
spelled for the games that put the language in the *ending*:

    vox/scripted/mpl/anbo/vox_anbo_ctf_enemy_dropped_00.rn75.pc.en.snd

Black Ops 4 puts it in the *path*, and swaps to its own ending:

    en\\vox\\scripted\\mpl\\anbo\\vox_anbo_ctf_enemy_dropped_00.sn100.pc.snd

`en` is the single commonest root in every Black Ops 4 sound name recovered so far, ahead of
`zmb`, `wpn` and `amb`, so this is not a marginal shape. Moving the language from one end to the
other and trying the four Black Ops 4 endings reached 256 ids that nothing else here had.

Two things worth knowing before spending a night on the idea again:

  - **The twelve files are one file.** Strip the ending and they hold the same paths; only the
    ending differed. Everything found comes from `en`, because the ids in the snapshot were
    captured from an English install. The other eleven add nothing and this stops after the first.
  - **It is spent.** Every language prefix was tried against every path, and the four endings
    against all of it. What did not match here will not match on a second run.
"""
import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import settings  # noqa: E402

BS = chr(92)
SUFFIXES = (".ll100.pc.snd", ".ln100.pc.snd", ".sn100.pc.snd", ".sl100.pc.snd")
# Black Ops 4 writes German `ge`, not `de`, the same way Black Ops 3 did.
LANGUAGES = ("en", "fr", "ge", "es", "it", "ru", "ja", "ko", "pl", "pt", "zh", "br", "me", "cz", "")
TAILS = (re.compile(r"\.[a-z]{2}\d+\.pc\.[a-z_]+\.snd$"),
         re.compile(r"\.[a-z]{2,3}\.\d+\.\d+\.[a-z_]+$"),
         re.compile(r"\.[a-z]{2}\d+\.pc\.snd$"))


def strip(name):
    for tail in TAILS:
        cut = tail.sub("", name)
        if cut != name:
            return cut
    return name


def main():
    csv = settings.tables_csv()
    stems = set()
    for path in sorted(glob.glob(os.path.join(csv, "*xsounds*.csv"))):
        with open(path, encoding="utf-8", errors="replace") as handle:
            for line in handle:
                if "," in line:
                    stems.add(strip(line.rstrip("\n").split(",", 1)[1].lower()).replace("/", BS))

    seen = set()
    for stem in sorted(stems):
        for language in LANGUAGES:
            base = (language + BS + stem) if language else stem
            for suffix in SUFFIXES:
                candidate = base + suffix
                if candidate not in seen:
                    seen.add(candidate)
                    print(candidate)

    print("%d paths, %d candidates" % (len(stems), len(seen)), file=sys.stderr)


if __name__ == "__main__":
    main()
