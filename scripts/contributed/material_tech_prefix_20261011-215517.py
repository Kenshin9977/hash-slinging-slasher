"""Materials as `<technique>/<a name the game already writes somewhere>`.

A modern material is its technique prefix (`m`, `tm`, `elcq`, `i`, `tm7`, ...), a slash, and a base
that is very often the name of something else: a model, an image without its channel ending, a
string of a zone. This writes a plan crossing

    begin   every technique prefix seen on 3 known materials or more, with its slash
    stem    every identifier-like string given (zone strings, other types' names), and the known
            names of every type cut to their base and with 0 to 2 trailing tokens dropped

    python contrib/material_tech_prefix.py <names.tsv>... --strings <sorted unique strings file>
    confirm_plan plans/material_tech.txt --game <TAG>
names.tsv: <kind> <tab> <id hex> <tab> <name>.
"""
import collections
import os
import re
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
while not os.path.isfile(os.path.join(ROOT, "scripts", "snapshot.py")):
    parent = os.path.dirname(ROOT)
    if parent == ROOT:
        raise SystemExit("Could not locate the repository root")
    ROOT = parent
OK = re.compile(r'^[a-z0-9_]{4,90}$')


def main():
    args = sys.argv[1:]
    strings = None
    if '--strings' in args:
        i = args.index('--strings')
        strings = args[i + 1]
        del args[i:i + 2]
    prefixes, stems = collections.Counter(), set()
    for path in args:
        for line in open(path, encoding='utf-8'):
            kind, _, name = line.rstrip('\n').split('\t', 2)
            head, _, base = name.rpartition('/')
            if kind == 'material' and head and '/' not in head and not base.startswith(chr(42)):
                prefixes[head] += 1
            base = base.partition('.')[0]
            t = base.split('_')
            for k in range(0, min(2, len(t) - 2) + 1):
                s = '_'.join(t[:len(t) - k])
                if OK.match(s):
                    stems.add(s)
    os.makedirs(os.path.join(ROOT, 'plans'), exist_ok=True)
    begins = [p + '/' for p, c in prefixes.most_common() if c >= 3]
    with open(os.path.join(ROOT, 'plans', 'material_tech_begins.txt'), 'w', newline='\n') as fh:
        fh.write('\n'.join(begins) + '\n')
    n = 0
    with open(os.path.join(ROOT, 'plans', 'material_tech_stems.txt'), 'w', encoding='utf-8', newline='\n') as fh:
        for s in sorted(stems):
            fh.write(s + '\n')
            n += 1
        if strings:
            for line in open(strings, encoding='utf-8', errors='replace'):
                s = line.strip()
                if s not in stems and OK.match(s):
                    fh.write(s + '\n')
                    n += 1
    with open(os.path.join(ROOT, 'plans', 'material_tech.txt'), 'w', newline='\n') as fh:
        fh.write('label: materials as a technique prefix and a name the game writes elsewhere\n')
        fh.write('begin: @plans/material_tech_begins.txt\nstem: @plans/material_tech_stems.txt\n')
    print('%d prefixes, %d stems, %.3g candidates' % (len(begins), n, len(begins) * n))


if __name__ == '__main__':
    main()
