"""Burned-in Chinese subtitles for v3.7-zh: the .srt as an ASS script at 1920x1080 (Noto Sans SC
from the repo, white with a dark outline, bottom centre, below the film's caption pills), for
ffmpeg's `ass` filter.

    python path/subs.py out/NimbleMap-launch-v3.7-zh-1080p.srt out/subs-zh.ass
    python path/subs.py out/NimbleMap-launch-v3.8-1080p.srt out/subs-en.ass en    # v3.8: English, in Inter
"""
import re
import sys

SIZE, MARGIN_V, OUTLINE = 46, 22, 3.2
HEAD = f"""[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
WrapStyle: 0
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: zh,{{FONT}},{{SIZE}},&H00FFFFFF,&H00FFFFFF,&H00271811,&H80000000,0,0,0,0,100,100,{{SPACING}},0,1,{OUTLINE},1.2,2,120,120,{MARGIN_V},1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"""


def ass_time(s):
    h, m, rest = s.split(':')
    sec, ms = rest.split(',')
    return f'{int(h)}:{m}:{sec}.{int(ms) // 10:02d}'


# the font, size and letter spacing per language (Inter SemiBold from assets/fonts for English)
STYLE = {'zh': ('Noto Sans SC Medium', SIZE, 1), 'en': ('Inter SemiBold', 44, 1.5)}


def main(src, dst, lang='zh'):
    font, size, spacing = STYLE[lang]
    head = HEAD.replace('{FONT}', font).replace('{SIZE}', str(size)).replace('{SPACING}', str(spacing))
    blocks = re.split(r'\n\s*\n', open(src, encoding='utf-8').read().strip())
    out = [head]
    for b in blocks:
        lines = b.splitlines()
        a, z = [x.strip() for x in lines[1].split('-->')]
        text = '\\N'.join(lines[2:])
        out.append(f'Dialogue: 0,{ass_time(a)},{ass_time(z)},zh,,0,0,0,,{text}\n')
    open(dst, 'w', encoding='utf-8').write(''.join(out))


if __name__ == '__main__':
    main(*sys.argv[1:4])
