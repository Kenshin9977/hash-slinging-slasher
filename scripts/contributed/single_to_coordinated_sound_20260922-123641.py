"""Transfer basename sibling substitutions to repeated path/filename identifiers.

Run: python contrib/single_to_coordinated_sound.py --out logs/single-coordinated.txt
Reads real sound-file tables and confirmed submissions. Writes candidates and JSON counts.
Reusable. Unlike coordinated_identifiers, evidence comes from single differing basename
tokens; application changes every matching complete token together. Requires three distinct
known sibling frames per substitution, excludes already tested coordinated candidates.
"""
from pathlib import Path
from collections import Counter, defaultdict
import argparse
import itertools
import json
import re
import sys

ROOT = Path(__file__).resolve().parent
while not (ROOT / 'scripts' / 'snapshot.py').is_file() and ROOT != ROOT.parent:
    ROOT = ROOT.parent
sys.path.insert(0, str(ROOT / 'scripts'))
import snapshot

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out', required=True)
    ap.add_argument('--exclude', action='append', default=[])
    args = ap.parse_args()
    known = {n.lower().strip() for n in itertools.chain(snapshot.table_names('fnv1a_xsounds'),
             snapshot.confirmed_names('sound_asset')) if n.strip()}
    rows = []
    repeated_vocab = set()
    for name in sorted(known):
        parts = re.split(r'([^a-z0-9]+)', name)
        tokens = {t for t,c in Counter(parts[::2]).items() if c>=2 and len(t)>=2 and not t.isdigit()}
        if tokens:
            rows.append((parts, tokens))
            repeated_vocab.update(tokens)
    frames = defaultdict(set)
    for name in sorted(known):
        cut = max(name.rfind('/'), name.rfind('\\')) + 1
        parts = re.split(r'([^a-z0-9]+)', name[cut:])
        for i in range(0, len(parts), 2):
            token = parts[i]
            if token in repeated_vocab:
                frame = (name[:cut], tuple(parts[:i]), tuple(parts[i+1:]))
                frames[frame].add(token)
    support = Counter()
    for values in frames.values():
        if 2 <= len(values) <= 64:
            support.update(itertools.combinations(sorted(values), 2))
    rules = defaultdict(set)
    for (a,b),count in support.items():
        if count>=3:
            rules[a].add(b)
            rules[b].add(a)
    excluded = set(known)
    for path in args.exclude:
        excluded.update(Path(path).read_text(encoding='utf-8').splitlines())
    out = set()
    controls = 0
    for parts,tokens in rows:
        for token in tokens:
            for other in rules.get(token, ()):
                candidate = ''.join(other if i%2==0 and p==token else p for i,p in enumerate(parts))
                if candidate in known:
                    controls += 1
                elif candidate not in excluded:
                    out.add(candidate)
    with open(args.out, 'w', encoding='utf-8', newline='\n') as handle:
        for candidate in sorted(out):
            handle.write(candidate+'\n')
    report = dict(seeds=len(known), sibling_frames=len(frames), rules=sum(map(len,rules.values())),
                  positive_controls=controls, novel_candidates=len(out))
    Path(args.out+'.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report))

if __name__ == '__main__':
    main()
