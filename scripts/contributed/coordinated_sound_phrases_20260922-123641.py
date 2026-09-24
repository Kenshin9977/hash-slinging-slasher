"""Preserve directory/basename agreement while replacing repeated multiword sound identifiers.

Run: python contrib/coordinated_sound_phrases.py --out logs/sound-phrases.txt
Reads sound-file community tables and confirmed names. Writes candidates and a JSON
measurement beside the output. Reusable after new names arrive. Learns replacements
from at least two exact sibling templates, allowing phrase lengths to differ. The
single-token/single-token case belongs to coordinated_identifiers and is excluded.
"""
from pathlib import Path
from collections import Counter, defaultdict
import argparse
import itertools
import json
import re
import sys

ROOT = Path(__file__).resolve().parent
while not (ROOT / 'scripts' / 'snapshot.py').is_file() and ROOT != ROOT.parent:
    ROOT = ROOT.parent
sys.path.insert(0, str(ROOT / 'scripts'))
import snapshot

def frames(name):
    words = list(re.finditer('[a-z0-9]+', name))
    counts = Counter()
    for i, word in enumerate(words):
        for width in range(1, 4):
            if i + width > len(words):
                break
            phrase = name[word.start():words[i + width - 1].end()]
            if re.fullmatch('[a-z0-9]+(?:_[a-z0-9]+)*', phrase) and len(phrase) >= 2 and not phrase.isdigit():
                counts[phrase] += 1
    for phrase, count in counts.items():
        if count >= 2:
            pattern = r'(?<![a-z0-9])' + re.escape(phrase) + r'(?![a-z0-9])'
            template, changed = re.subn(pattern, '\x00', name)
            if changed >= 2:
                yield phrase, template

def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out', required=True)
    args = ap.parse_args()
    known = {n.lower().strip() for n in itertools.chain(snapshot.table_names('fnv1a_xsounds'),
             snapshot.confirmed_names('sound_asset')) if n.strip()}
    groups = defaultdict(set)
    for name in sorted(known):
        for phrase, template in frames(name):
            groups[template].add(phrase)
    support = Counter()
    for values in groups.values():
        if 2 <= len(values) <= 100:
            for a, b in itertools.combinations(sorted(values), 2):
                if '_' in a or '_' in b:
                    support[a, b] += 1
    rules = defaultdict(set)
    for (a, b), count in support.items():
        if count >= 2:
            rules[a].add(b)
            rules[b].add(a)
    out = set()
    controls = 0
    for template, phrases in groups.items():
        for phrase in phrases:
            for other in rules.get(phrase, ()):
                candidate = template.replace('\x00', other)
                if candidate in known:
                    controls += 1
                else:
                    out.add(candidate)
    output = Path(args.out)
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open('w', encoding='utf-8', newline='\n') as handle:
        for candidate in sorted(out):
            handle.write(candidate + '\n')
    report = dict(seeds=len(known), templates=len(groups), rules=sum(map(len, rules.values())),
                  positive_controls=controls, candidates=len(out))
    Path(str(output) + '.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report), flush=True)

if __name__ == '__main__':
    main()
