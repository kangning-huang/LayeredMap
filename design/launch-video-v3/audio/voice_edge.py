"""Render the v3.2 narration with Microsoft en-US-AndrewNeural through edge-tts (the owner's choice;
only the script text is sent to the TTS service). One mp3 per line in audio/vo_edge/<id>.mp3 and
its word boundaries in <id>.json (offset/duration in seconds), used for timing and the SRT.
The text is the caption column of audio/narration.txt, spelt normally ("Tohoku" without the macron).

    mamba run -n lm-video python audio/voice_edge.py [voice] [rate]
Fallback without the network: audio/voice.sh (macOS say).
"""
import asyncio
import json
import os
import sys

import edge_tts

HERE = os.path.dirname(os.path.abspath(__file__))
VOICE = sys.argv[1] if len(sys.argv) > 1 else 'en-US-AndrewNeural'
RATE = sys.argv[2] if len(sys.argv) > 2 else '+4%'
# VO_SCRIPT / VO_OUT pick the script and folder (v3.3: narration33.txt -> vo_edge33)
SCRIPT = os.environ.get('VO_SCRIPT', 'narration.txt')
OUT = os.path.join(HERE, os.environ.get('VO_OUT', 'vo_edge'))
SPOKEN = {'Tōhoku': 'Tohoku'}


def lines():
    for line in open(os.path.join(HERE, SCRIPT), encoding='utf-8'):
        if line.startswith('#') or '|' not in line:
            continue
        vid, _, cap = [p.strip() for p in line.split('|')]
        for a, b in SPOKEN.items():
            cap = cap.replace(a, b)
        yield vid, cap


async def one(vid, text):
    com = edge_tts.Communicate(text, VOICE, rate=RATE, boundary='WordBoundary')
    words = []
    with open(os.path.join(OUT, vid + '.mp3'), 'wb') as f:
        async for ch in com.stream():
            if ch['type'] == 'audio':
                f.write(ch['data'])
            elif ch['type'] == 'WordBoundary':
                words.append({'text': ch['text'], 'offset': ch['offset'] / 1e7, 'duration': ch['duration'] / 1e7})
    json.dump({'voice': VOICE, 'rate': RATE, 'text': text, 'words': words}, open(os.path.join(OUT, vid + '.json'), 'w'), indent=1)
    w = words[-1]
    print(f'{vid:9s} {w["offset"] + w["duration"]:5.2f} s  {text}')


async def main():
    os.makedirs(OUT, exist_ok=True)
    only = set(os.environ.get('VO_ONLY', '').split(',')) - {''}
    for vid, text in lines():
        if not only or vid in only:
            await one(vid, text)


asyncio.run(main())
