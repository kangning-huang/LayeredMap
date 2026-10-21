#!/usr/bin/env python3
"""Build assets/zh/ts-characters.txt, the traditional -> simplified character table that place search
folds Chinese text with (crates/lm-core/src/zh_fold.rs), from OpenCC's TSCharacters.txt.

    curl -LO https://raw.githubusercontent.com/BYVoid/OpenCC/<commit>/data/dictionary/TSCharacters.txt
    python3 tools/lm/build_ts_table.py TSCharacters.txt <commit>

One line per character pair, "traditional<TAB>simplified", sorted by code point. A traditional
character with several simplified candidates keeps OpenCC's first; pairs that are not one
character to one character, and identities, are left out. Data from OpenCC (Open Chinese Convert),
https://github.com/BYVoid/OpenCC, Apache License 2.0. Standard library only.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, '..', '..', 'assets', 'zh', 'ts-characters.txt')


def main(src, commit):
    pairs = {}
    for line in open(src, encoding='utf-8'):
        if not line.strip() or line.startswith('#'):
            continue
        key, _, vals = line.rstrip('\n').partition('\t')
        first = vals.split(' ')[0] if vals else ''
        if len(key) == 1 and len(first) == 1 and key != first:
            pairs.setdefault(key, first)
    rows = sorted(pairs.items(), key=lambda kv: ord(kv[0]))
    with open(OUT, 'w', encoding='utf-8', newline='\n') as f:
        f.write(f'# Traditional -> simplified Chinese characters, {len(rows)} pairs, from OpenCC TSCharacters.txt\n')
        f.write(f'# (github.com/BYVoid/OpenCC, commit {commit}; Apache License 2.0). Made by tools/lm/build_ts_table.py.\n')
        for t, s in rows:
            f.write(f'{t}\t{s}\n')
    print(f'wrote {len(rows)} pairs to {os.path.normpath(OUT)}')


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else 'master')
