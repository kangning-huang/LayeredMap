"""Offline token estimates (the API's own counts are the real measure).

Uses tiktoken's o200k_base when it is available, else about 3.6 characters per token for
English JSON (CJK text counts one token per character).
"""

from __future__ import annotations

import re

_enc = None
try:  # pragma: no cover - depends on the environment
    import tiktoken

    _enc = tiktoken.get_encoding("o200k_base")
except Exception:  # noqa: BLE001
    _enc = None


def count_tokens(text: str) -> int:
    if _enc is not None:
        return len(_enc.encode(text))
    cjk = len(re.findall(r"[　-鿿＀-￯]", text))
    return round((len(text) - cjk) / 3.6) + cjk


def method() -> str:
    return "tiktoken o200k_base" if _enc is not None else "chars/3.6 estimate"
