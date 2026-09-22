"""Change repeated identifiers together using substitutions witnessed in real sibling names.

Run: python contrib/coordinated_identifiers.py --out logs/coordinated
Reads typed community tables and confirmed submissions; writes separate visual and sound
candidate files and a JSON measurement report. Reusable after corpus updates. Unlike a
single-slot substitution, all complete occurrences of an identifier change atomically.
Rules require two distinct sibling templates as evidence. Measurements are written by the run.
"""
from pathlib import Path
import argparse
from collections import Counter, defaultdict
import itertools
import json
import re
import sys

ROOT = Path(__file__).resolve().parent
while not (ROOT / 'scripts' / 'snapshot.py').is_file() and ROOT != ROOT.parent:
    ROOT = ROOT.parent
sys.path.insert(0, str(ROOT / 'scripts'))
import snapshot

TABLES = {'xmodel': 'fnv1a_xmodels', 'material': 'fnv1a_xmaterials',
          'image': 'fnv1a_ximages', 'xanim': 'fnv1a_xanims',
          'sound_asset': 'fnv1a_xsounds', 'sound_alias': 'fnv1a_soundbanks_aliases'}

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out', required=True)
    args = ap.parse_args()
    prefix = Path(args.out)
    prefix.parent.mkdir(parents=True, exist_ok=True)
    report = {}
    outputs = {k: open(str(prefix) + '.' + k + '.txt', 'w', encoding='utf-8', newline='\n')
               for k in ('visual', 'sound')}
    for kind, table in TABLES.items():
        known = {n.strip().lower() for n in itertools.chain(snapshot.table_names(table),
                  snapshot.confirmed_names(kind)) if n.strip()}
        groups = defaultdict(set)
        rows = []
        for name in sorted(known):
            parts = re.split(r'([^a-z0-9]+)', name)
            repeated = [t for t, count in Counter(parts[::2]).items()
                        if count >= 2 and len(t) >= 2 and not t.isdigit()]
            for token in repeated:
                template = tuple('' if i % 2 == 0 and p == token else p
                                 for i, p in enumerate(parts))
                groups[template].add(token)
                rows.append((name, parts, token))
        support = Counter()
        controls = 0
        for values in groups.values():
            if 2 <= len(values) <= 100:
                controls += len(values)
                support.update(itertools.combinations(sorted(values), 2))
        rules = defaultdict(set)
        for (a, b), count in support.items():
            if count >= 2:
                rules[a].add(b)
                rules[b].add(a)
        emitted = set()
        rebuilt = 0
        for name, parts, token in rows:
            for other in sorted(rules.get(token, ())):
                candidate = ''.join(other if i % 2 == 0 and p == token else p
                                    for i, p in enumerate(parts))
                if candidate in known:
                    rebuilt += 1
                else:
                    emitted.add(candidate)
        target = outputs['sound' if kind.startswith('sound') else 'visual']
        for candidate in sorted(emitted):
            target.write(candidate + '\n')
        report[kind] = dict(seeds=len(known), repeated_rows=len(rows),
                            sibling_controls=controls, supported_rules=sum(map(len, rules.values())),
                            rebuilt_known=rebuilt, candidates=len(emitted))
        print(kind, json.dumps(report[kind]), file=sys.stderr, flush=True)
    for handle in outputs.values():
        handle.close()
    Path(str(prefix) + '.json').write_text(json.dumps(report, indent=2), encoding='utf-8')

if __name__ == '__main__':
    main()
