"""Dialogue files from the subtitle keys of the localize tables.

A subtitle is the localize key `subtitle_<title>_<mode>_<family>_<group>/<alias>` (the asset-name
hash, so an alias confirms its own key). The key gives the title and the English text of the alias;
the file is `<title>/<mode>/<directory>/<stem>.<tail>` with the stem either the alias or the alias
and the text squeezed to 20 letters and digits. Every directory known under that title and mode is
tried, spelled with `/` and with `.`.

    python contrib/subtitle_key_files.py <keys.tsv> <names.tsv>... | confirm_list - --game <TAG> --script contrib/subtitle_key_files.py
keys.tsv: <subtitle key> <tab> <English text>;  names.tsv: <kind> <tab> <id hex> <tab> <name>.
"""
import collections
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import importlib.util
from pathlib import Path
_spec = importlib.util.spec_from_file_location("dialogue_lines", Path(__file__).with_name("dialogue_lines_20261011-215517.py"))
_dialogue = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_dialogue)
squeeze = _dialogue.squeeze

BS = chr(92)


def main():
    keys, tables = sys.argv[1], sys.argv[2:]
    dirs = collections.defaultdict(set)
    tails = set()
    for path in tables:
        for row in open(path, encoding='utf-8'):
            what, _, name = row.rstrip('\n').split('\t', 2)
            name = name.replace(BS, '/')
            if what != 'sound_asset' or 'dx_' not in name or not name.endswith(('english', 'english_safe')):
                continue
            i = name.index('dx_')
            d = name[:i].replace('.', '/').strip('/').split('/')
            tail = name[i:].partition('.')[2]
            tails.add(tail)
            fmt, _, rest = tail.partition('.')
            if len(fmt) == 3:
                tails.add(fmt[:2] + '.' + rest)
            if len(d) == 3:
                dirs[(d[0], d[1])].add(d[2])
    out = sys.stdout
    for row in open(keys, encoding='utf-8'):
        key, _, text = row.rstrip('\n').partition('\t')
        pre, _, alias = key.partition('/')
        p = pre.split('_')
        if len(p) < 4:
            continue
        stems = {alias, alias + '_' + squeeze(text)}
        for d in dirs.get((p[1], p[2]), ()):
            for stem in stems:
                for tail in tails:
                    name = '%s/%s/%s/%s.%s' % (p[1], p[2], d, stem, tail)
                    out.write(name + '\n')
                    out.write(name.replace('/', '.') + '\n')


if __name__ == '__main__':
    main()
