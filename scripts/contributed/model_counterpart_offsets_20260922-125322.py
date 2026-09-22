"""Transfer a component-label change together with an observed numeric offset in model names.

Run: python contrib/model_counterpart_offsets.py --out logs/model-offsets.txt
Reads real typed model tables and confirmations. Writes candidates and JSON measurements.
Reusable. Learns offsets -32..32 (excluding zero) from otherwise identical model siblings;
each label-change/offset/distance rule requires three distinct source integer values and
three sibling frames. Changes both fields, preserving the source number's spelling width.
This is an attested coupled numerical transformation, not an unobserved rectangular grid.
"""
from pathlib import Path
from collections import defaultdict
import argparse
import itertools
import json
import sys
ROOT=Path(__file__).resolve().parent
while not (ROOT/'scripts'/'snapshot.py').is_file() and ROOT!=ROOT.parent: ROOT=ROOT.parent
sys.path.insert(0,str(ROOT/'scripts'))
import snapshot

def number(t): return t.isdigit() and len(t)<=3 and int(t)<=256

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',required=True)
    ap.add_argument('--exclude',action='append',default=[])
    args=ap.parse_args()
    known={n.strip().lower().replace('\\','/') for n in itertools.chain(
        snapshot.table_names('fnv1a_xmodels'),snapshot.confirmed_names('xmodel')) if n.strip()}
    rows=[tuple(n.split('_')) for n in sorted(known) if 4<=len(n.split('_'))<=18]
    out=set()
    report=[]
    for gap in list(range(-8,0))+list(range(1,9)):
        groups=defaultdict(set)
        for row in rows:
            for j,t in enumerate(row):
                i=j-gap
                if not number(t) or not 0<=i<len(row) or row[i].isdigit(): continue
                frame=list(row)
                frame[i]='<label>'
                frame[j]='<number>'
                groups[tuple(frame)].add((row[i],int(t)))
        values=defaultdict(set)
        counts=defaultdict(int)
        for group in groups.values():
            if not 2<=len(group)<=32: continue
            frame_rules=set()
            for (a,n),(b,m) in itertools.permutations(group,2):
                delta=m-n
                if a!=b and 0<abs(delta)<=32:
                    key=(a,b,delta)
                    values[key].add(n)
                    frame_rules.add(key)
            for key in frame_rules: counts[key]+=1
        rules=defaultdict(set)
        for (a,b,delta),valueset in values.items():
            if len(valueset)>=3 and counts[a,b,delta]>=3: rules[a].add((b,delta))
        controls=0
        before=len(out)
        for row in rows:
            for j,t in enumerate(row):
                i=j-gap
                if not number(t) or not 0<=i<len(row): continue
                for b,delta in rules.get(row[i],()):
                    value=int(t)+delta
                    if not 0<=value<=256: continue
                    changed=list(row)
                    changed[i]=b
                    changed[j]=str(value).zfill(len(t))
                    candidate='_'.join(changed)
                    if candidate in known: controls+=1
                    else: out.add(candidate)
        item=dict(gap=gap,rules=sum(map(len,rules.values())),controls=controls,new_candidates=len(out)-before)
        report.append(item)
        print(json.dumps(item),flush=True)
    for source in args.exclude:
        with open(source,encoding='utf-8') as handle:
            for line in handle: out.discard(line.strip())
    with open(args.out,'w',encoding='utf-8',newline='\n') as handle:
        for candidate in sorted(out): handle.write(candidate+'\n')
    Path(args.out+'.json').write_text(json.dumps(dict(candidates=len(out),measurements=report),indent=2),encoding='utf-8')
if __name__=='__main__': main()
