"""Subtitles for the square v3.8-zh: the same narration and timing as the 16:9 SRT, re-broken for a
1080-wide frame (at most ~16 characters a line, at most two lines), and the two longest lines split
into two cues at a word boundary measured from the voice (audio/vo_edge38zh/<line>.json).

    python path/subs_square.py <v3.8-zh .srt> <vo.txt> out.srt out.ass

The ASS is 1080x1080, Noto Sans SC Medium, ASS size 76 (about 54 px glyphs), white on a translucent dark box (BorderStyle 3), bottom centre with its
lowest line ending 150 px above the bottom (WeChat Channels / RedNote cover the bottom ~12%).
"""
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
VO = os.path.join(os.path.dirname(HERE), 'audio', 'vo_edge38zh')

# line id -> one or more cues (\N = line break). A cue after the first starts at the word that
# begins its text.
SQUARE = {
    'folder': ['网上找到的地图数据，\\N电脑根本打不开。'],
    'open': ['拖进凌波微图，\\N马上就能做出漂亮的地图。'],
    'storms_a': ['1980年以来近五千条气旋路径。\\N随意缩放拖动。'],
    'storms_b': ['搜索地点，比如纽约。'],
    'storms_c': ['也能搜你的数据：\\N输入Sandy，两个同名风暴都找到。'],
    'quakes_a': ['1900年以来，六级以上的地震。'],
    'quakes_b': ['飞到东京，点一下：\\N2011年东日本大地震，', '9.1级，\\N比99%的地震都强。'],
    'heat_a': ['打开2022年热浪，拖动时间轴，'],
    'heat_b': ['8月16日巴塞罗那：\\N热浪强度超过大部分地方。'],
    'pro_a': ['注册登录用Pro，大白话问数据。'],
    'pro_b': ['问它：有多少场五级风暴\\N经过东京100公里内？'],
    'pro_c': ['凌波微图在东京周围\\N画出100公里范围，', '数出穿过它的风暴路径：\\N共七条。'],
    'end': ['凌波微图。\\N探索地球的真实数据。免费。'],
}
PUNCT = re.compile(r'[\s，。：、？！,.:?!%\\N]')

SIZE, OUTLINE, MARGIN_V = 76, 9, 158
LINE_H = SIZE + 2 * OUTLINE      # a line's box height (libass: font size + padding on both sides)
HEAD = f"""[Script Info]
ScriptType: v4.00+
PlayResX: 1080
PlayResY: 1080
WrapStyle: 2
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: zh,Noto Sans SC Medium,{SIZE},&H00FFFFFF,&H00FFFFFF,&H1C101418,&H1C101418,0,0,0,0,100,100,1,0,3,{OUTLINE},0,2,60,60,{MARGIN_V},1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"""


def parse_srt(path):
    out = []
    for b in re.split(r'\n\s*\n', open(path, encoding='utf-8').read().strip()):
        lines = b.splitlines()
        a, z = [x.strip() for x in lines[1].split('-->')]
        out.append((sec(a), sec(z), ''.join(lines[2:])))
    return out


def sec(s):
    h, m, rest = s.replace(',', '.').split(':')
    return int(h) * 3600 + int(m) * 60 + float(rest)


def srt_ts(s):
    ms = int(round(s * 1000))
    return f'{ms // 3600000:02d}:{ms // 60000 % 60:02d}:{ms // 1000 % 60:02d},{ms % 1000:03d}'


def ass_ts(s):
    cs = int(round(s * 100))
    return f'{cs // 360000}:{cs // 6000 % 60:02d}:{cs // 100 % 60:02d}.{cs % 100:02d}'


def split_time(vid, start, prefix):
    """Film time where the word after `prefix` (text of the earlier cues) starts."""
    words = json.load(open(os.path.join(VO, vid + '.json'), encoding='utf-8'))['words']
    n = len(PUNCT.sub('', prefix))
    k = 0
    for w in words:
        if k >= n:
            return start + w['offset'] - words[0]['offset'] - 0.05
        k += len(PUNCT.sub('', w['text']))
    raise ValueError(vid)


def main(srt, vo_txt, out_srt, out_ass):
    cues = parse_srt(srt)
    ids = [line.split('\t')[0] for line in open(vo_txt) if line.strip()]
    assert len(ids) == len(cues), (len(ids), len(cues))
    rows = []
    for vid, (a, z, text) in zip(ids, cues):
        parts = SQUARE[vid]
        assert PUNCT.sub('', ''.join(parts)) == PUNCT.sub('', text), (vid, text)
        t0, done = a, ''
        for i, p in enumerate(parts):
            t1 = z if i == len(parts) - 1 else split_time(vid, a, done + p)
            rows.append((t0, t1, p))
            done += p
            t0 = t1 + 0.02
    with open(out_srt, 'w', encoding='utf-8') as f:
        for i, (a, z, p) in enumerate(rows):
            f.write(f'{i + 1}\n{srt_ts(a)} --> {srt_ts(z)}\n{p.replace(chr(92) + "N", chr(10))}\n\n')
    with open(out_ass, 'w', encoding='utf-8') as f:
        f.write(HEAD)
        for a, z, p in rows:
            # one event per line, stacked by exact margins, so the line boxes touch without overlapping
            for i, line in enumerate(reversed(p.split('\\N'))):
                f.write(f'Dialogue: 0,{ass_ts(a)},{ass_ts(z)},zh,,0,0,{MARGIN_V + i * LINE_H},,{line}\n')
    for a, z, p in rows:
        print(f'{a:6.2f} {z:6.2f} {p}')


if __name__ == '__main__':
    main(*sys.argv[1:5])
