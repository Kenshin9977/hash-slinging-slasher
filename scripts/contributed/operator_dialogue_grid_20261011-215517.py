"""Operator dialogue files from the grid their names are built on.

An operator line is an alias `dx_op_<kind>_<event>_<speaker>` and its files are

    <title>/op/<directory>/dx_op_<operator>_<kind>_<speaker>_<event>_<line>[_<take>][_<set>].<format>.<level>.48000.<language>

where <line> is the spoken text squeezed to its first 20 letters. Operators share most of their
script, so a line one operator is known to say under an event is tried for every speaker that has
an alias for that event, in each directory that speaker is known to use, with every tail seen
(and its two-letter format spelling, `sn` for `snn`, which the older title uses).
Titles that spell the directory with dots (`iw9.op.ghost.dx_...`) get that spelling too.

    python contrib/operator_dialogue_grid.py <names.tsv>... | confirm_list - --game <TAG> --script contrib/operator_dialogue_grid.py
names.tsv: <kind> <tab> <id hex> <tab> <name>, known names of any of the four titles.
"""
import collections
import sys

BS = chr(92)


def main():
    places = collections.defaultdict(set)     # speaker -> (directory, operator)
    lines = collections.defaultdict(set)      # (kind, event) -> rest of the stem after the event
    tails = set()
    wanted = set()                            # (kind, event, speaker) of the aliases
    for path in sys.argv[1:]:
        for row in open(path, encoding='utf-8'):
            what, _, name = row.rstrip('\n').split('\t', 2)
            name = name.replace(BS, '/')
            if what == 'sound_alias' and name.startswith('dx_op_'):
                t = name.split('_')
                if len(t) == 5:
                    wanted.add((t[2], t[3], t[4]))
            elif what == 'sound_asset' and 'dx_op_' in name:
                if '/' not in name:
                    i = name.index('dx_op_')
                    name = name[:i].replace('.', '/') + name[i:]
                d, _, last = name.rpartition('/')
                stem, _, tail = last.partition('.')
                t = stem.split('_', 6)
                if len(t) == 7:
                    places[t[4]].add((d, t[2]))
                    lines[(t[3], t[5])].add(t[6])
                    tails.add(tail)
    for tail in list(tails):
        fmt, _, rest = tail.partition('.')
        if len(fmt) == 3 and fmt.endswith('n'):
            tails.add(fmt[:2] + '.' + rest)
    out = sys.stdout
    for kind, event, speaker in sorted(wanted):
        for d, operator in places.get(speaker, ()):
            head = '%s/dx_op_%s_%s_%s_%s_' % (d, operator, kind, speaker, event)
            for rest in lines.get((kind, event), ()):
                for tail in tails:
                    name = head + rest + '.' + tail
                    out.write(name + '\n')
                    out.write(name.replace('/', '.') + '\n')


if __name__ == '__main__':
    main()
