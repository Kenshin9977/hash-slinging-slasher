"""Build the existing material-to-image derivation as a plan over explicit new material seeds.

Run: python contrib/material_delta_plan.py findings/GAME/run_ID/material.txt --plan plans/material-delta.txt
Reads confirmed material files and typed image/material tables and confirmations. Writes
stems, observed one-to-three-token endings and a compiled-engine plan. Reusable after new
material discoveries. Mirrors stems_of/OPENINGS in src/bin/images_from_materials.rs.
"""
from pathlib import Path
import argparse
import json
import sys
ROOT=Path(__file__).resolve().parent
while not (ROOT/'scripts'/'snapshot.py').is_file() and ROOT!=ROOT.parent: ROOT=ROOT.parent
sys.path.insert(0,str(ROOT/'scripts'))
import snapshot

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('sources',nargs='+')
    ap.add_argument('--plan',required=True)
    args=ap.parse_args()
    seeds=set()
    for source in args.sources:
        for line in Path(source).read_text(encoding='utf-8').splitlines():
            _,sep,name=line.partition(',')
            if sep and name.strip(): seeds.add(name.strip().lower())
    stems=set()
    for name in seeds:
        stems.add(name)
        rest=name.rsplit('/',1)[-1]
        stems.add(rest)
        for _ in range(2):
            match=next((p for p in ['i_mtl_','mtl_mtl_','mtl_','i_c_','i_','c_']
                        if rest.startswith(p) and len(rest)-len(p)>=4),None)
            if match is None: break
            rest=rest[len(match):]
            stems.add(rest)
    endings=set()
    for kind,table in [('image','fnv1a_ximages'),('material','fnv1a_xmaterials')]:
        for name in snapshot.table_names(table)+snapshot.confirmed_names(kind):
            parts=name.lower().split('_')
            for depth in range(1,min(3,len(parts)-1)+1):
                endings.add('_'+'_'.join(parts[-depth:]))
    plan=Path(args.plan)
    plan.parent.mkdir(parents=True,exist_ok=True)
    stemfile=plan.with_suffix('.stems.txt')
    endfile=plan.with_suffix('.ends.txt')
    stemfile.write_text('\n'.join(sorted(stems))+'\n',encoding='utf-8')
    endfile.write_text('\n'.join(sorted(endings))+'\n',encoding='utf-8')
    text=['label: image counterparts of newly confirmed paired-change materials',
          'describe: existing material-to-image derivation bounded to explicit newly confirmed material seeds',
          'begin: i_','begin: i_mtl_','begin: mtl_','begin: c_','begin: i_c_',
          'stem: @'+stemfile.as_posix(),'end: @'+endfile.as_posix(),'bare: yes','fold: yes']
    plan.write_text('\n'.join(text)+'\n',encoding='utf-8')
    print(json.dumps(dict(seeds=len(seeds),stems=len(stems),endings=len(endings))))
if __name__=='__main__': main()
