"""Learn three linked attachment-texture field changes from independent texture cores.

Run: python contrib/attachment_triplet_plan.py --plan plans/attachment-triplets.txt
Reads real image names; removes observed channel suffixes before counting evidence so the
same texture's channels cannot supply multiple witnesses. Requires two distinct core frames
for a triplet substitution, with every field changed. Writes recombined cores, measured
channels, and a plan so Rust performs the channel cross product. Reusable after discoveries.
"""
from pathlib import Path
from collections import Counter,defaultdict
import argparse
import itertools
import json
import sys
ROOT=Path(__file__).resolve().parent
while not (ROOT/'scripts'/'snapshot.py').is_file() and ROOT!=ROOT.parent: ROOT=ROOT.parent
sys.path.insert(0,str(ROOT/'scripts'))
import snapshot
import image_channels

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--plan',required=True)
    args=ap.parse_args()
    cores=set()
    channels=set()
    for name in snapshot.table_names('fnv1a_ximages')+snapshot.confirmed_names('image'):
        name=name.strip().lower()
        if not name.startswith('i_attach_'): continue
        core,sep,channel=name.rpartition('_')
        if channel in image_channels.CHANNELS:
            cores.add(tuple(core.split('_')))
            channels.add('_'+channel)
    buckets=defaultdict(list)
    for row in cores:
        if 5<=len(row)<=12: buckets[len(row)].append(row)
    generated=set()
    rules_count=controls=0
    for length,rows in sorted(buckets.items()):
        for positions in itertools.combinations(range(2,length),3):
            groups=defaultdict(set)
            for row in rows:
                frame=tuple(v for i,v in enumerate(row) if i not in positions)
                groups[frame].add(tuple(row[i] for i in positions))
            support=Counter()
            for values in groups.values():
                if 2<=len(values)<=24:
                    for a,b in itertools.combinations(sorted(values),2):
                        if all(x!=y for x,y in zip(a,b)): support[a,b]+=1
            rules=defaultdict(set)
            for (a,b),count in support.items():
                if count>=2:
                    rules[a].add(b)
                    rules[b].add(a)
            rules_count+=sum(map(len,rules.values()))
            for row in rows:
                for values in rules.get(tuple(row[i] for i in positions),()):
                    changed=list(row)
                    for i,v in zip(positions,values): changed[i]=v
                    changed=tuple(changed)
                    if changed in cores: controls+=1
                    else: generated.add('_'.join(changed))
    plan=Path(args.plan)
    stemfile=plan.with_suffix('.stems.txt')
    endfile=plan.with_suffix('.ends.txt')
    stemfile.write_text('\n'.join(sorted(generated))+'\n',encoding='utf-8')
    endfile.write_text('\n'.join(sorted(channels))+'\n',encoding='utf-8')
    plan.write_text('\n'.join(['label: independent-core witnessed attachment triplets',
        'describe: three coupled field changes supported by two independent attachment texture cores',
        'stem: @'+stemfile.as_posix(),'end: @'+endfile.as_posix(),'bare: yes','fold: yes'])+'\n',encoding='utf-8')
    report=dict(known_cores=len(cores),rules=rules_count,controls=controls,
                new_cores=len(generated),channels=len(channels))
    Path(str(plan)+'.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report))
if __name__=='__main__': main()
