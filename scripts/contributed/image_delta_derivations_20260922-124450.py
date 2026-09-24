"""Apply existing channel/material derivations only to explicitly supplied new image names.

Run: python contrib/image_delta_derivations.py findings/GAME/run_ID/image.txt --out logs/image-delta.txt
Reads confirmed image files; uses channel vocabulary, material core extraction and all twelve
directories from the existing library. Writes a bounded candidate file and count. Reusable
after any image discovery; not a search scheduler or a new naming convention.
"""
from pathlib import Path
import argparse
import json
import sys
ROOT=Path(__file__).resolve().parent
while not (ROOT/'scripts'/'snapshot.py').is_file() and ROOT!=ROOT.parent: ROOT=ROOT.parent
sys.path.insert(0,str(ROOT/'scripts'))
import image_channels
import materials_from_images

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('sources',nargs='+')
    ap.add_argument('--out',required=True)
    args=ap.parse_args()
    seeds=set()
    for source in args.sources:
        for line in Path(source).read_text(encoding='utf-8').splitlines():
            key,sep,name=line.partition(',')
            if sep and name.strip(): seeds.add(name.strip().lower().replace('\\','/'))
    out=set()
    for name in seeds:
        head,sep,tail=name.rpartition('_')
        if head and (tail in image_channels.CHANNELS or len(tail)<=2):
            out.add(head)
            out.update(head+'_'+c for c in image_channels.CHANNELS)
        core=materials_from_images.core_of(name)
        for directory in materials_from_images.DIRECTORIES:
            out.add(directory+core)
            out.add(directory+'mtl_'+core)
    out-=seeds
    with open(args.out,'w',encoding='utf-8',newline='\n') as handle:
        for name in sorted(out): handle.write(name+'\n')
    print(json.dumps(dict(seeds=len(seeds),candidates=len(out))))
if __name__=='__main__': main()
