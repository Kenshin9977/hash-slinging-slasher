"""Fill numbered take-variant gaps that `families.py --gaps` cannot see.

A method, not a report. Pipe it into `confirm_list`.

    python contrib/sound_take_gaps_20260908.py | bin\\windows\\confirm_list.exe - \\
        --label "sound take-number gap filling" --script contrib/sound_take_gaps_20260908.py

## The gap this fills

`scripts/families.py` finds numbered families by matching the LAST run of digits in a whole
name (`NUMBERED = re.compile(r"^(.*?)(\\d+)([^0-9]*)$")`). Sound alias and sound file names are
commonly written

    vox/scripted/operators/zyna/vox_zyna_mtx_execute_rare_00.rn75.pc.en.snd
    vox/scripted/operators/zyna/vox_zyna_mtx_execute_rare_01.rn75.pc.en.snd
    ...

and the codec/quality tag (`rn75`, `pn75`, `lnn.75`, ...) carries its own trailing digits
*after* the real take number. `families.py`'s regex anchors on the last digit run in the
string, which is the codec tag's digits (`75`), not the take number (`00`) -- so it groups by
the wrong family entirely and the take-number axis is invisible to it.

Measured directly against the published sound tables plus this project's own confirmed and
merged names: 909,650 names carry a `_<NN>.<anything>` shape (a number immediately after an
underscore and immediately before a dot), across 214 distinct codec/language extension
combinations (`.rn75.pc.en.snd`, `.rn75.pc.fr.snd`, `.lnn.75.48000.all`, ...). Grouped by
`(everything before the number, its width, everything from the dot onward)`, 219,177 of
285,259 such families have two or more members already observed -- meaning a missing take is
common, not rare, and every one of them is invisible to the existing gap-filler.

## How it generates

Exactly `families.py --gaps`'s own algorithm (same `WIDEST` cap, same default margin, same
"a one-character stem is a coincidence, not a family" guard) with one change: the regex
requires the number to sit directly between an underscore and a dot (`_(\\d{2,3})\\.`), which is
where a take number lives, rather than matching the last digit run wherever it falls.

## What it reads and writes

Reads the community sound tables (`fnv1a_soundbanks_aliases[_v2]`, `fnv1a_xsounds[_v2]`, and
all twelve per-language `xsounds` tables) plus this machine's confirmed findings and the merged
submissions, all via `snapshot.py`. Writes candidate names to standard output, one per line;
sizing to standard error.

## Options

    --margin N   how far past each end of an observed run to go (default 4, matches families.py)
    --count      print how many candidates this would produce, and stop

## Reusable or one-off

Reusable. Re-measures from the corpus every time it runs.
"""
import collections
import os
import re
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
while ROOT != os.path.dirname(ROOT) and not os.path.isfile(os.path.join(ROOT, "scripts", "snapshot.py")):
    ROOT = os.path.dirname(ROOT)
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import snapshot

TABLES = ("fnv1a_soundbanks_aliases", "fnv1a_soundbanks_aliases_v2",
          "fnv1a_xsounds", "fnv1a_xsounds_v2") + tuple(
    "fnv1a_%s_xsounds" % language for language in (
        "english french german italian spanish americanspanish brazilianportugese "
        "russian polish japanese korean chinese"
    ).split()
)

# A take number sits between an underscore and a dot -- `_00.rn75...` -- which is what
# distinguishes it from a codec tag's own trailing digits (`rn75`'s `75` is preceded by a
# letter, not an underscore, so it never matches this).
NUMBERED = re.compile(r"^(.*_)(\d{2,3})(\..+)$")

# Same constants as families.py, so this stays a drop-in sibling of the existing method rather
# than a differently-tuned one.
WIDEST = 512


def families(names):
    found = collections.defaultdict(set)
    for name in names:
        match = NUMBERED.match(name)
        if not match:
            continue
        before, digits, after = match.groups()
        if len(before) < 3:
            continue
        found[(before, len(digits), after)].add(int(digits))
    return found


def gaps(found, margin):
    for (before, width, after), seen in found.items():
        low, high = min(seen), max(seen)
        if high - low > WIDEST:
            continue
        for number in range(max(0, low - margin), high + margin + 1):
            if number in seen:
                continue
            yield "%s%0*d%s" % (before, width, number, after)


def main(argv):
    margin = int(argv[argv.index("--margin") + 1]) if "--margin" in argv else 4
    counting = "--count" in argv

    print("reading known names", file=sys.stderr)
    known = {n.strip().lower().replace("\\", "/") for n in snapshot.table_names(*TABLES)}
    known.update(n.strip().lower().replace("\\", "/") for n in snapshot.confirmed_names())
    known.discard("")
    print("%d known names" % len(known), file=sys.stderr)

    found = families(known)
    multi = {k: v for k, v in found.items() if len(v) >= 2}
    print("%d take-number families, %d with two or more members" % (len(found), len(multi)),
          file=sys.stderr)

    produced = 0
    out = sys.stdout
    batch = []
    for candidate in gaps(multi, margin):
        if candidate in known:
            continue
        produced += 1
        if not counting:
            batch.append(candidate)
        if len(batch) >= 65536:
            out.write("\n".join(batch) + "\n")
            batch = []

    if batch and not counting:
        out.write("\n".join(batch) + "\n")

    print("%d candidates" % produced, file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv[1:])
