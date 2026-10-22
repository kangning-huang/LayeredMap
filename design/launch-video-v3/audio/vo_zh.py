"""The v3.7-zh narration: the owner's Chinese lines (audio/narration37zh.txt, exactly as written),
read by Microsoft zh-CN-YunxiNeural through edge-tts at +6% (the owner's earlier video, vo_gen.py).
Only the script text goes to the TTS service. Each line starts at its v3.7 start time
(edit/edl36.py VO); a line that would run past the end of its beat is read again a little faster,
2% at a time up to +15%, and a line that still doesn't fit is reported (no words are cut).

    mamba run -n lm-video python audio/vo_zh.py          # writes audio/vo_edge37zh/<id>.mp3/.json, rates.json

The .json beside each mp3 holds its word boundaries (offset/duration, s), as audio/voice_edge.py
writes them; audio/synth36.py places and ducks under them (VO_OUT=vo_edge37zh).
"""
import asyncio
import json
import os
import sys

import edge_tts

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(os.path.dirname(HERE), 'edit'))
import edl36 as N  # noqa: E402

VOICE = 'zh-CN-YunxiNeural'
BASE, MAX_RATE, STEP = 6, 15, 2
OUT = os.path.join(HERE, 'vo_edge37zh')
# Where each line's beat ends (s): the next line's start, or the picture beat's end when that
# comes first (the drop at 39.87, the Pro window at 50.0, the end of the Pro section at 68.8).
BEAT_END = {'folder': 9.85, 'open': 14.70, 'storms_a': 19.60, 'storms_b': 23.20, 'storms_c': 27.95,
            'quakes_a': 32.05, 'quakes_b': 39.85, 'heat_a': 43.80, 'heat_b': 49.95, 'pro_a': 54.30,
            'pro_b': 59.50, 'pro_c': 68.80, 'end': 78.60}


def lines():
    for line in open(os.path.join(HERE, 'narration37zh.txt'), encoding='utf-8'):
        if line.startswith('#') or '|' not in line:
            continue
        vid, _, cap = [p.strip() for p in line.split('|')]
        yield vid, cap


async def say(vid, text, rate):
    com = edge_tts.Communicate(text, VOICE, rate=f'+{rate}%', boundary='WordBoundary')
    words, audio = [], bytearray()
    async for ch in com.stream():
        if ch['type'] == 'audio':
            audio += ch['data']
        elif ch['type'] == 'WordBoundary':
            words.append({'text': ch['text'], 'offset': ch['offset'] / 1e7, 'duration': ch['duration'] / 1e7})
    return bytes(audio), words


async def main():
    os.makedirs(OUT, exist_ok=True)
    starts = dict(N.VO)
    rates, report = {}, []
    for vid, text in lines():
        t0, end = starts[vid], BEAT_END[vid]
        rate = BASE
        while True:
            audio, words = await say(vid, text, rate)
            speech = words[-1]['offset'] + words[-1]['duration'] - words[0]['offset']
            fits = t0 + speech <= end
            if fits or rate >= MAX_RATE:
                break
            rate = min(MAX_RATE, rate + STEP)
        with open(os.path.join(OUT, vid + '.mp3'), 'wb') as f:
            f.write(audio)
        json.dump({'voice': VOICE, 'rate': f'+{rate}%', 'text': text, 'words': words},
                  open(os.path.join(OUT, vid + '.json'), 'w', encoding='utf-8'), indent=1, ensure_ascii=False)
        rates[vid] = rate
        line = (f'{vid:9s} +{rate:2d}%  {t0:6.2f} -> {t0 + speech:6.2f} s (beat ends {end:6.2f}; '
                f'{speech:4.2f} s)  {"ok" if fits else "DOES NOT FIT"}  {text}')
        report.append(line)
        print(line, flush=True)
    json.dump(rates, open(os.path.join(OUT, 'rates.json'), 'w'), indent=1)
    open(os.path.join(OUT, 'fit.txt'), 'w', encoding='utf-8').write('\n'.join(report) + '\n')


asyncio.run(main())
