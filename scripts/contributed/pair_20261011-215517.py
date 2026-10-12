"""Two tokens of a known name replaced at once, each from the alphabet of its own slot.

One-token substitution cannot reach a name two steps from everything known. Every pair of slots
of every known name is crossed, each slot taking only the words its place calls for: those seen
after the same left neighbour or before the same right neighbour (digits folded), the `cap`
commonest. This is the template-with-slot-categories idea of the BO4 rounds (METHODS.md, method
10) with the category drawn from the slot's neighbours.
"""
import collections
import re

# Reference algorithm from the contributor's separate core/ engine.
# This repository does not ship that engine; this helper is not a standalone CLI.
try:
    from core import config, engines, games, names
except ImportError as error:
    raise ImportError("This reference helper requires the contributor's external core/ engine; use the repository's confirm_list or confirm_plan for confirmation.") from error

TOKEN = re.compile('[^_/.]+')


def shape(t):
    return re.sub(r'\d+', '#', t)


def run(tags, kind, cap=60, min_count=2, count_only=False, log=print, only=None):
    """only: a set of names; when given, just these known names are skeletons (a family worth a wider
    cap), the alphabets still come from every known name."""
    assert games.same_family(tags, kind), 'these games do not hash this kind alike: run them apart'
    skeletons = {}
    for t in tags:
        for name in names.known(t)[kind].values():
            if names.usable(name):
                skeletons.setdefault(name, TOKEN.findall(name))
    targets = names.targets(tags, kind)
    left, right = collections.defaultdict(collections.Counter), collections.defaultdict(collections.Counter)
    for toks in skeletons.values():
        sh = ['^'] + [shape(t) for t in toks] + ['$']
        for j, t in enumerate(toks):
            left[sh[j]][t] += 1
            right[sh[j + 2]][t] += 1
    words, alphabets, rows, total = {}, {}, [], 0
    for name, toks in skeletons.items():
        if only is not None and name not in only:
            continue
        if not 2 <= len(toks) <= 60:
            continue
        sh = ['^'] + [shape(t) for t in toks] + ['$']
        ids, sizes = [], []
        for j in range(len(toks)):
            key = (sh[j], sh[j + 2])
            if key not in alphabets:
                c = left[key[0]] + right[key[1]]
                chosen = [w for w, n in c.most_common(cap) if n >= min_count]
                alphabets[key] = (len(alphabets), [words.setdefault(w, len(words)) for w in chosen])
            ids.append(alphabets[key][0])
            sizes.append(len(alphabets[key][1]))
        total += (sum(sizes) ** 2 - sum(x * x for x in sizes)) // 2
        rows.append(name + '\t' + ','.join(map(str, ids)))
    log('pair %s: %d targets, %d skeletons, %d alphabets, %d words, %.3g candidates'
        % (kind, len(targets), len(rows), len(alphabets), len(words), total))
    if count_only:
        return {}
    f = [config.work('pair', '%s_%s.txt' % (kind, x)) for x in ('targets', 'words', 'alphabets', 'skeletons')]
    engines.write(f[0], ('%016x' % t for t in targets))
    engines.write(f[1], list(words))
    engines.write(f[2], (' '.join(map(str, a[1])) for a in sorted(alphabets.values())))
    engines.write(f[3], rows)
    return {name: i for i, name in engines.run('pair_slots', tags[0], kind, f) if name not in skeletons}
