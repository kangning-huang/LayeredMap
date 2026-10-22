"""NimbleMap launch film v3.7.1-zh: v3.7-zh shortened (owner, 2026-10-21: the Chinese folder line
ends at 5.23 s and the next starts at 9.95 s, a silence the English line fills).

The folder section (v3.7 time 0-9.90, a slow push on the folder, then the .tif row is selected)
plays faster, uniformly, in 0-TRIM_AT; from 9.90 on everything is v3.7-zh shifted SHIFT earlier:
picture, narration, captions, key clicks and music (the cut is in the music's quiet folder pad,
before its first downbeat, so every sync point moves with its picture). The camera is at rest at
9.90 (between its 9.80 and 10.15 keys), so the change of speed doesn't show.
"""
from edl36 import *  # noqa: F401,F403
import edl36 as _e

TRIM_OLD = 9.90          # v3.7 time where normal speed resumes (just before the open line's lead)
TRIM_AT = 5.85           # the same moment in v3.7.1-zh: 0.62 s after the folder line ends (5.23)
SHIFT = round(TRIM_OLD - TRIM_AT, 3)
DUR = round(_e.DUR - SHIFT, 3)


def old_time(t):
    """v3.7-zh time for v3.7.1-zh time t."""
    return t * TRIM_OLD / TRIM_AT if t < TRIM_AT else t + SHIFT
