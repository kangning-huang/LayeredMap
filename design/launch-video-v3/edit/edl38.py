"""NimbleMap launch film v3.8 (en and zh): v3.7 with a fast opening.

v3.7 spent its first 9.9 s on the folder of files ("your computer can't open these"), and the first
map appeared at 10.5 s. v3.8 opens on the map:

  0.00-2.00  cold open: the storm film's world view, all 4,999 tracks drawn, with its caption
             ("4,999 storm tracks"); v3.7 15.95-17.35 played at 0.7x (its slow drift)
  1.80-2.00  dissolve to the folder
  2.00-3.60  the folder, one quick beat: v3.7 7.5-9.9 at 1.5x (the file-type strip, then the .tif
             row is selected; the table camera is at rest at 9.9, so the change of speed doesn't show)
  3.60-      v3.7 from 9.9 on, SHIFT earlier: the drag, the files opening, the storms, and the rest of
             the film unchanged (picture, narration, captions, key clicks, music)

Only the first narration line is new (shorter); it starts at 0.30 s over the cold open. The sound is
v3.7's (audio/synth36.py on this timeline's VO, in v3.7 time) with its first SHIFT seconds removed
(audio/trim38.py): those are the folder's held pad before the first downbeat.
"""
from edl36 import *  # noqa: F401,F403
import edl36 as _e

COLD = 2.00                  # the cold open ends (v3.8 time)
COLD_SRC = (15.95, 17.35)    # v3.7 time it plays: the storm world view, fully drawn, slow drift
XF = 0.20                    # dissolve from the cold open into the folder, ending at COLD
FOLD_SRC = 7.50              # v3.7 time where the folder beat starts (the file-type strip is up)
RESUME_OLD = 9.90            # v3.7 time where normal speed resumes (as in v3.7.1-zh)
RESUME_AT = 3.60             # the same moment in v3.8
SHIFT = round(RESUME_OLD - RESUME_AT, 3)
DUR = round(_e.DUR - SHIFT, 3)
DUR_SOUND = _e.DUR           # audio/synth36.py makes v3.7's full length; trim38 cuts it
VO_FOLDER_AT = 0.30          # the new first line starts here (v3.8 time)

# for the sound, which is made on v3.7's clock and then trimmed: the first line moved so that it
# lands at VO_FOLDER_AT after the trim; a soft whoosh under the dissolve (v3.7 time)
VO = [(v, round(SHIFT + VO_FOLDER_AT, 3) if v == 'folder' else t0) for v, t0 in _e.VO]
T_COLD_CUT = SHIFT + COLD - XF


def cold_time(t):
    return COLD_SRC[0] + t * (COLD_SRC[1] - COLD_SRC[0]) / COLD


def folder_time(t):
    return FOLD_SRC + (t - COLD) * (RESUME_OLD - FOLD_SRC) / (RESUME_AT - COLD)


def old_time(t):
    """v3.7 time for v3.8 time t (the dissolve, COLD - XF .. COLD, blends cold_time and folder_time)."""
    if t < COLD:
        return cold_time(t)
    if t < RESUME_AT:
        return folder_time(t)
    return t + SHIFT


def mix(t):
    """Weight of the folder picture during the dissolve (0 = cold open only)."""
    if t < COLD - XF:
        return 0.0
    if t >= COLD:
        return 1.0
    u = (t - (COLD - XF)) / XF
    return u * u * (3 - 2 * u)
