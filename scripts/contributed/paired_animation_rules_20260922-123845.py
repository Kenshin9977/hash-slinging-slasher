"""Transfer witnessed nonadjacent two-token changes between real animation names.

Run: python contrib/paired_animation_rules.py --out logs/paired-anims.txt
Reads typed animation tables and confirmed names. Writes candidates and JSON measurements.
Use --kind xmodel to measure the same relation in model naming separately.
Reusable. A rule changes two nonadjacent slots atomically, preserves their distance, and
must be witnessed in three otherwise identical sibling frames. Both tokens must change.
This transfers attested paired changes rather than crossing independent slot vocabularies.
"""
from pathlib import Path
from collections import Counter, defaultdict
import argparse
import itertools
import json
import sys

ROOT = Path(__file__).resolve().parent
while not (ROOT / 'scripts' / 'snapshot.py').is_file() and ROOT != ROOT.parent:
    ROOT = ROOT.parent
sys.path.insert(0, str(ROOT / 'scripts'))
import snapshot

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out', required=True)
    ap.add_argument('--kind', choices=['xanim','xmodel'], default='xanim')
    args = ap.parse_args()
    known = {n.strip().lower().replace('\\','/') for n in itertools.chain(
        snapshot.table_names('fnv1a_xanims' if args.kind=='xanim' else 'fnv1a_xmodels'),
        snapshot.confirmed_names(args.kind)) if n.strip()}
    rows = [tuple(n.split('_')) for n in sorted(known) if 4<=len(n.split('_'))<=18]
    out = set()
    report = []
    for gap in range(2,7):
        groups = defaultdict(set)
        for row in rows:
            for i in range(len(row)-gap):
                j=i+gap
                frame=(row[:i],row[i+1:j],row[j+1:])
                groups[frame].add((row[i],row[j]))
        support = Counter()
        for values in groups.values():
            if 2<=len(values)<=24:
                for a,b in itertools.combinations(sorted(values),2):
                    if a[0]!=b[0] and a[1]!=b[1]:
                        support[a,b]+=1
        rules=defaultdict(set)
        for (a,b),count in support.items():
            if count>=3:
                rules[a].add(b)
                rules[b].add(a)
        controls=0
        before=len(out)
        for row in rows:
            for i in range(len(row)-gap):
                j=i+gap
                for a,b in rules.get((row[i],row[j]),()):
                    changed=list(row)
                    changed[i],changed[j]=a,b
                    candidate='_'.join(changed)
                    if candidate in known:
                        controls+=1
                    else:
                        out.add(candidate)
        measurement=dict(gap=gap,rules=sum(map(len,rules.values())),
                         controls=controls,new_candidates=len(out)-before)
        report.append(measurement)
        print(json.dumps(measurement),flush=True)
    with open(args.out,'w',encoding='utf-8',newline='\n') as handle:
        for candidate in sorted(out): handle.write(candidate+'\n')
    Path(args.out+'.json').write_text(json.dumps(dict(seeds=len(known),candidates=len(out),gaps=report),indent=2),encoding='utf-8')

if __name__=='__main__': main()
