"""Expand recording-take siblings only around newly confirmed sound files.

Run: python contrib/sound_seed_siblings.py findings/blkops04/run_ID/sound_asset.txt --out logs/siblings.txt
Reads explicit confirmed seed files and typed sound tables/submissions. Writes candidates
and a count report. Reusable, bounded derivation; does not schedule or confirm searches.
Only basename digits before the encoding extension are varied. Numeric spellings must
occur in real sound basenames, have the same width, and be at most 256.
"""
from pathlib import Path
from collections import defaultdict
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

def core_bounds(name):
    start = max(name.rfind('/'), name.rfind('\\')) + 1
    end = name.find('.', start)
    return start, len(name) if end < 0 else end

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('seeds', nargs='+')
    ap.add_argument('--out', required=True)
    args = ap.parse_args()
    known = set(n.lower().strip() for n in itertools.chain(snapshot.table_names('fnv1a_xsounds'),
                snapshot.confirmed_names('sound_asset')) if n.strip())
    vocabulary = defaultdict(set)
    for name in known:
        a, b = core_bounds(name)
        for m in re.finditer(r'\d{1,3}', name[a:b]):
            value = m.group()
            if int(value) <= 256:
                vocabulary[len(value)].add(value)
    seeds = set()
    for source in args.seeds:
        for line in Path(source).read_text(encoding='utf-8').splitlines():
            key, separator, name = line.partition(',')
            if separator and name.strip():
                seeds.add(name.strip().lower())
    out = set()
    for name in seeds:
        a, b = core_bounds(name)
        for m in re.finditer(r'\d+', name[a:b]):
            if len(m.group()) <= 3 and int(m.group()) <= 256:
                for value in vocabulary[len(m.group())]:
                    candidate = name[:a + m.start()] + value + name[a + m.end():]
                    if candidate not in known:
                        out.add(candidate)
    with open(args.out, 'w', encoding='utf-8', newline='\n') as handle:
        for candidate in sorted(out):
            handle.write(candidate + '\n')
    print(json.dumps(dict(seeds=len(seeds), candidates=len(out),
                          numeric_vocabulary={str(k):len(v) for k,v in vocabulary.items()})))

if __name__ == '__main__':
    main()
