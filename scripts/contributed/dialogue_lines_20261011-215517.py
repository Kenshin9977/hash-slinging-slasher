"""Dialogue files from the subtitles: a line's file is its alias and the spoken text.

    <dir>/<alias>_<line>.<format>.<level>.48000.<language>                         every mode
    <dir>/dx_op_<operator>_<kind>_<speaker>_<event>_<line>[_<set>].<tail>           operators
        (alias dx_op_<kind>_<event>_<speaker>)

<line> is the English subtitle in lower case, letters and digits only, cut at 20 characters
(5,307 of the 6,469 lines of already named files are spelled that way from the game's own
localize texts). The texts come from the English localize zones (`eng_*.ff`, `ens_*.ff`), read
with GrayWolf's `disk-export --type localize` into `key,text` csv files.

Writes two plans, the directories spelled with `/` (Black Ops 6, Black Ops 7) and with `.`
(Modern Warfare II and III):
    begin   <dir>/<alias>_ for each directory the alias' family (its first three tokens) is known
            to use, or its mode when the family has none; operators: each known place of the speaker
    stem    every squeezed line
    end     every tail seen on a named dialogue file, operators also with each set ending

    python contrib/dialogue_lines.py <folder of localize csv> <names.tsv>...
    confirm_plan plans/dialogue_lines_slash.txt --game BLACKOP7
names.tsv: <kind> <tab> <id hex> <tab> <name>.
"""
import collections
import csv
import glob
import os
import re
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
while not os.path.isfile(os.path.join(ROOT, "scripts", "snapshot.py")):
    parent = os.path.dirname(ROOT)
    if parent == ROOT:
        raise SystemExit("Could not locate the repository root")
    ROOT = parent
BS = chr(92)
CODE = re.compile(r'\^[0-9a-zA-Z]|\[\{[^}]*\}\]|&&\d')


def squeeze(text):
    return re.sub(r'[^a-z0-9]', '', CODE.sub('', text).lower())[:20]


def main():
    folder, tables = sys.argv[1], sys.argv[2:]
    csv.field_size_limit(1 << 30)
    lines = set()
    for f in glob.glob(os.path.join(folder, '**', '*.csv'), recursive=True):
        with open(f, encoding='utf-8', errors='replace', newline='') as fh:
            for row in csv.reader(fh):
                if len(row) >= 2 and len(squeeze(row[1])) >= 3:
                    lines.add(squeeze(row[1]))
    aliases = set()
    family, mode = collections.defaultdict(set), collections.defaultdict(set)
    places = collections.defaultdict(set)
    tails, sets = set(), set()
    for path in tables:
        for row in open(path, encoding='utf-8'):
            what, _, name = row.rstrip('\n').split('\t', 2)
            name = name.replace(BS, '/')
            if what == 'sound_alias' and name.startswith('dx_'):
                aliases.add(name)
            elif what == 'sound_asset' and 'dx_' in name and name.endswith(('english', 'english_safe')):
                i = name.index('dx_')
                d = name[:i].replace('.', '/').rstrip('/')
                stem, _, tail = name[i:].partition('.')
                t = stem.split('_')
                tails.add(tail)
                fmt, _, rest = tail.partition('.')
                if len(fmt) == 3:
                    tails.add(fmt[:2] + '.' + rest)
                if len(t) < 4:
                    continue
                if t[1] == 'op' and len(t) >= 7:
                    places[t[4]].add((d, t[2]))
                    if re.fullmatch(r'o[a-z]{2}\d', t[-1]):
                        sets.add('_' + t[-1])
                else:
                    family['_'.join(t[:3])].add(d)
                    mode[t[1]].add(d)
    begins, op_begins = set(), set()
    for a in aliases:
        t = a.split('_')
        if len(t) == 5 and t[1] == 'op':
            for d, operator in places.get(t[4], ()):
                op_begins.add('%s/dx_op_%s_%s_%s_%s_' % (d, operator, t[2], t[4], t[3]))
        elif len(t) >= 3:
            for d in family.get('_'.join(t[:3])) or mode.get(t[1], ()):
                begins.add('%s/%s_' % (d, a))
    ends = sorted('.' + x for x in tails)
    op_ends = sorted(s + e for s in sets | {''} for e in ends)
    plans = os.path.join(ROOT, 'plans')
    os.makedirs(plans, exist_ok=True)

    def put(name, rows):
        with open(os.path.join(plans, name), 'w', encoding='utf-8', newline='\n') as fh:
            fh.write('\n'.join(rows) + '\n')

    put('dialogue_lines_stems.txt', sorted(lines))
    put('dialogue_lines_ends.txt', ends)
    put('dialogue_lines_op_ends.txt', op_ends)
    for spell, sep in (('slash', '/'), ('dot', '.')):
        put('dialogue_lines_begins_%s.txt' % spell, sorted(b.replace('/', sep) for b in begins))
        put('dialogue_lines_op_begins_%s.txt' % spell, sorted(b.replace('/', sep) for b in op_begins))
        put('dialogue_lines_%s.txt' % spell, [
            'label: dialogue files as their alias and the squeezed English subtitle',
            'begin: @plans/dialogue_lines_begins_%s.txt' % spell,
            'stem: @plans/dialogue_lines_stems.txt', 'end: @plans/dialogue_lines_ends.txt'])
        put('dialogue_lines_op_%s.txt' % spell, [
            'label: operator dialogue files as their grid and the squeezed English subtitle',
            'begin: @plans/dialogue_lines_op_begins_%s.txt' % spell,
            'stem: @plans/dialogue_lines_stems.txt', 'end: @plans/dialogue_lines_op_ends.txt'])
    print('%d lines, %d aliases; begins %d (x %d ends), operators %d (x %d ends)' % (
        len(lines), len(aliases), len(begins), len(ends), len(op_begins), len(op_ends)))
    print('candidates per spelling: %.3g and %.3g' % (len(begins) * len(lines) * len(ends),
                                                     len(op_begins) * len(lines) * len(op_ends)))


if __name__ == '__main__':
    main()
