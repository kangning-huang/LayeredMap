#!/usr/bin/env python3
"""Drive the iOS Simulator with real mouse and keyboard events.

`simctl` can install, launch and open a URL, but it cannot tap, and the chooser is a thing you
tap. System Events' `click at` is refused (-25204), so this posts CGEvents directly through
ctypes — no pyobjc needed.

Coordinates are the *device's* own points (402 x 874 on an iPhone 16 Pro), which is what a
screenshot gives you after dividing by 3. The window bezel is measured once, not guessed.

    python3 scripts/drive-simulator.py tap 76 135
    python3 scripts/drive-simulator.py type "forest loss"
    python3 scripts/drive-simulator.py shot /tmp/x.png

Needs Terminal (or whatever is running this) to hold macOS Accessibility permission, and the
Simulator window to be on screen — these are real events through the window server, not an
automation API.
"""
import ctypes
import ctypes.util
import os
import re
import subprocess
import sys
import time

cg = ctypes.cdll.LoadLibrary(ctypes.util.find_library("CoreGraphics"))


class CGPoint(ctypes.Structure):
    _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]


cg.CGEventCreateMouseEvent.restype = ctypes.c_void_p
cg.CGEventCreateMouseEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint32, CGPoint, ctypes.c_uint32]
cg.CGEventCreateKeyboardEvent.restype = ctypes.c_void_p
cg.CGEventCreateKeyboardEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint16, ctypes.c_bool]
cg.CGEventPost.argtypes = [ctypes.c_uint32, ctypes.c_void_p]
cg.CGEventKeyboardSetUnicodeString.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_void_p]
cg.CFRelease.argtypes = [ctypes.c_void_p]

MOVE, DOWN, UP = 5, 1, 2
HID = 0

# The device screen inside the Simulator window. Both are points; the bezel is the rounded
# surround the window draws around the screen itself.
BEZEL_X, BEZEL_Y = 27, 70


def window_origin():
    out = subprocess.run(
        ["osascript", "-e",
         'tell application "System Events" to tell process "Simulator" '
         'to get position of window 1'],
        capture_output=True, text=True).stdout.strip()
    x, y = (int(v) for v in out.split(", "))
    return x, y


def focus():
    subprocess.run(["osascript", "-e", 'tell application "Simulator" to activate'],
                   capture_output=True)
    time.sleep(0.25)


def tap(x, y, hold=0.06):
    wx, wy = window_origin()
    p = CGPoint(wx + BEZEL_X + x, wy + BEZEL_Y + y)
    for kind in (MOVE, DOWN):
        e = cg.CGEventCreateMouseEvent(None, kind, p, 0)
        cg.CGEventPost(HID, e)
        cg.CFRelease(e)
        time.sleep(0.04)
    time.sleep(hold)
    e = cg.CGEventCreateMouseEvent(None, UP, p, 0)
    cg.CGEventPost(HID, e)
    cg.CFRelease(e)


def swipe(x1, y1, x2, y2, steps=18):
    wx, wy = window_origin()
    start = CGPoint(wx + BEZEL_X + x1, wy + BEZEL_Y + y1)
    e = cg.CGEventCreateMouseEvent(None, MOVE, start, 0)
    cg.CGEventPost(HID, e); cg.CFRelease(e)
    e = cg.CGEventCreateMouseEvent(None, DOWN, start, 0)
    cg.CGEventPost(HID, e); cg.CFRelease(e)
    for i in range(1, steps + 1):
        p = CGPoint(wx + BEZEL_X + x1 + (x2 - x1) * i / steps,
                    wy + BEZEL_Y + y1 + (y2 - y1) * i / steps)
        e = cg.CGEventCreateMouseEvent(None, 6, p, 0)  # left-drag
        cg.CGEventPost(HID, e); cg.CFRelease(e)
        time.sleep(0.012)
    end = CGPoint(wx + BEZEL_X + x2, wy + BEZEL_Y + y2)
    e = cg.CGEventCreateMouseEvent(None, UP, end, 0)
    cg.CGEventPost(HID, e); cg.CFRelease(e)


def type_text(text):
    # Not CGEventKeyboardSetUnicodeString: the Simulator forwards the raw *keycode* to the guest
    # and ignores the unicode override, so every character arrived as keycode 0 and "forest loss"
    # typed as "aaaaaaaaaaa". System Events' keystroke picks the right keycodes, and unlike its
    # `click at` (which is refused with -25204) it works here.
    escaped = text.replace("\\", "\\\\").replace('"', '\\"')
    subprocess.run(["osascript",
                    "-e", 'tell application "Simulator" to activate',
                    "-e", "delay 0.2",
                    "-e", f'tell application "System Events" to keystroke "{escaped}"'],
                   capture_output=True)


def device():
    """The booted simulator, or $LM_SIM when more than one is running."""
    if os.environ.get("LM_SIM"):
        return os.environ["LM_SIM"]
    out = subprocess.run(["xcrun", "simctl", "list", "devices", "booted"],
                         capture_output=True, text=True).stdout
    found = re.findall(r"\(([0-9A-F-]{36})\)", out)
    if not found:
        raise SystemExit("no booted simulator")
    return found[0]


def shot(path):
    subprocess.run(["xcrun", "simctl", "io", device(), "screenshot", path],
                   capture_output=True)


if __name__ == "__main__":
    cmd = sys.argv[1]
    focus()
    if cmd == "tap":
        tap(float(sys.argv[2]), float(sys.argv[3]))
    elif cmd == "swipe":
        swipe(*(float(v) for v in sys.argv[2:6]))
    elif cmd == "type":
        type_text(sys.argv[2])
    elif cmd == "shot":
        shot(sys.argv[2])
    print("ok")
