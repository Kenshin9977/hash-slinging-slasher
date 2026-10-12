"""Materials named from the images they hold (the reverse of material_image_slots.py).

A material's derived images spell its name: `<base>_m0_v2`, `<base>_m1_v2`, `<base>_dmg[_v2]`,
`<base>_thermalmap`, `cm_<base>_pri`, `<base>_emissivitymap`. For every material that has no name but
holds a named image, the image name is cut by 0 to 4 trailing tokens, `cm_` is also read as `mtl_`,
`mtl_` is added or dropped, and each base is tried under every technique prefix known for materials.

    links file:  <material id hex> <tab> <image id hex> <tab> <slot>
    names file:  <kind> <tab> <id hex> <tab> <name>
    python contrib/material_from_images.py <links.tsv> <names.tsv> | confirm_list - --game <TAG> --script contrib/material_from_images.py
"""
import collections
import sys

NEWLINE = chr(10)
STAR = chr(42)


def main():
    links_path, names_path = sys.argv[1], sys.argv[2]
    mat, img = {}, {}
    for line in open(names_path, encoding='utf-8'):
        k, i, n = line.rstrip(NEWLINE).split('\t', 2)
        if k == 'material':
            mat[int(i, 16)] = n
        elif k == 'image':
            img[int(i, 16)] = n
    techs = collections.Counter()
    for n in mat.values():
        head, _, base = n.rpartition('/')
        if head and '/' not in head and not base.startswith(STAR):
            techs[head] += 1
    techs = [t for t, c in techs.items() if c >= 3]
    bases = set()
    for line in open(links_path):
        m, i, _ = line.split('\t')
        m, i = int(m, 16), int(i, 16)
        if m in mat or i not in img:
            continue
        n = img[i]
        if n.startswith('$') or '~' in n or '&' in n:
            continue
        t = n.split('_')
        for cut in range(0, min(4, len(t) - 1) + 1):
            b = '_'.join(t[:len(t) - cut])
            bases.add(b)
            if b.startswith('cm_'):
                bases.add('mtl_' + b[3:])
            if b.startswith('mtl_'):
                bases.add(b[4:])
            else:
                bases.add('mtl_' + b)
    out = sys.stdout
    for b in bases:
        for t in techs:
            out.write(t + '/' + b + NEWLINE)


if __name__ == '__main__':
    main()
