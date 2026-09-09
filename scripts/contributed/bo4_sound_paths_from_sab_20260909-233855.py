#!/usr/bin/env python3
"""Black Ops 4 sound files, out of the folder trees Black Ops 2 and 3 already published.

    python contrib/bo4_sound_paths_from_sab.py | confirm_list - --no-fold \
        --script contrib/bo4_sound_paths_from_sab.py

`sound_asset` is the largest unnamed ground in either game -- 70,878 of 79,263 -- and the reason
is not the hash. It is that a Black Ops 4 sound name is a *path*, and no vocabulary reaches a
folder nobody has ever seen. `zmb\\ai\\hallion\\single_step\\` cannot be composed out of words; it
has to come from somewhere.

It comes from the older games. `bo2_sab.csv` and `bo3_sab.csv` publish 400,815 sound paths, and
the three games share both content and layout -- the same `amb\\alarms\\car\\dist\\`, the same
`wpn\\<class>\\<variant>\\plr\\`. What differs is spelling, and only in three ways:

  - a wrapper on the front: `raw\\`, `devraw\\english\\sound\\`, or a language segment `en\\`, `ru\\`
  - the separator: those tables store forward slashes, Black Ops 4 hashes backslashes
  - the ending: `.SN65.pc.snd`, `.SN85.pc.snd` and so on, against Black Ops 4's four

So: strip the wrapper by walking in until a segment is a root Black Ops 4 actually uses, swap the
separators, and try the four endings. Measured on this: 400,815 published paths reached 3,620 ids
as whole names -- and their *folders*, offered to the vocabulary the pool already knows, reached
1,734 more. The folders are worth more than the filenames, which is the whole point of the method.

The roots are read from names already confirmed rather than hardcoded, so this widens on its own
as the pool fills in. Ends its own reach honestly: it is spent when a round adds nothing, which
happened here after five.
"""
import collections
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import settings  # noqa: E402

BS = chr(92)
# Black Ops 4 keeps four, and every id in the pool ends in one of them.
SUFFIXES = (".ll100.pc.snd", ".ln100.pc.snd", ".sn100.pc.snd", ".sl100.pc.snd")
TAIL = re.compile(r"\.[a-z]{2}\d+\.pc\.(all\.)?snd$")
TOKEN = re.compile(r"^[a-z0-9][a-z0-9]{0,17}$")


def rows(path):
    if not os.path.exists(path):
        return
    with open(path, encoding="utf-8", errors="replace") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if "," in line:
                yield line.split(",", 1)[1].lower()


def known_bo4_sounds():
    """Every Black Ops 4 sound name this project has already confirmed."""
    out = set()
    path = os.path.join(ROOT, "all_names", "blkops04", "sound_asset.txt")
    for name in rows(path):
        if BS in name:
            out.add(name)
    return out


def main():
    csv = settings.tables_csv()
    known = known_bo4_sounds()

    # The roots are whatever confirmed names actually start with - `zmb`, `wpn`, `amb`, `uin`...
    # Reading them rather than listing them means a root nobody has seen yet joins the method the
    # moment one name under it is confirmed.
    roots = {n.split(BS)[0] for n in known if BS in n}
    if not roots:
        sys.exit("no confirmed Black Ops 4 sound names to read roots from")

    published = set()
    for name in ("bo2_sab.csv", "bo3_sab.csv", "fnv1a_xsounds.csv"):
        for value in rows(os.path.join(csv, name)):
            published.add(TAIL.sub("", value).replace("/", BS))

    # Walk in from the left until a segment is a root this game uses, which drops `raw\`,
    # `devraw\english\sound\` and the language segments in one rule rather than three.
    aligned = set()
    for path in published:
        parts = path.split(BS)
        for index, part in enumerate(parts):
            if part in roots:
                aligned.add(BS.join(parts[index:]))
                break

    seen = set()

    # The whole published paths, respelled. Bounded and small enough to print: 400k paths against
    # four endings, which `confirm_list` eats in seconds.
    for stem in sorted(aligned):
        for suffix in SUFFIXES:
            candidate = stem + suffix
            if candidate not in seen:
                seen.add(candidate)
                print(candidate)

    # The folders are the half that pays, and they are a cross product -- folder x word x ending.
    # Printing that from Python would be three billion strings at 2.6M a second; the engine
    # multiplies it in place. So write the three lists and let a plan do it.
    vocabulary = collections.Counter()
    for name in list(known) + list(aligned):
        for token in TAIL.sub("", name).replace(BS, "_").split("_"):
            if TOKEN.match(token):
                vocabulary[token] += 1

    folders = set()
    for name in list(known) + list(aligned):
        parts = name.split(BS)
        for depth in range(1, len(parts)):
            folder = BS.join(parts[:depth]) + BS
            if folder.count(BS) <= 5:
                folders.add(folder)

    words = [word for word, _ in vocabulary.most_common(3000)]
    endings = [suffix for suffix in SUFFIXES]
    endings += ["_%02d%s" % (index, suffix) for suffix in SUFFIXES for index in range(20)]

    here = os.path.dirname(os.path.abspath(__file__))
    for name, values in (("bo4_sound_folders.txt", sorted(folders)),
                         ("bo4_sound_words.txt", words),
                         ("bo4_sound_endings.txt", endings)):
        with open(os.path.join(here, name), "w", encoding="utf-8", newline="\n") as handle:
            handle.write("\n".join(values) + "\n")

    print("%d roots, %d published paths, %d aligned (%d printed), %d folders x %d words x %d "
          "endings = %.2e for the plan"
          % (len(roots), len(published), len(aligned), len(seen), len(folders), len(words),
             len(endings), len(folders) * len(words) * len(endings)), file=sys.stderr)


if __name__ == "__main__":
    main()
