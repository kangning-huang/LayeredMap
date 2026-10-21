"""The nature-journal illustration set: one per Data card, the right size, wordless, deterministic."""
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from dsart_laws import registry  # noqa: E402

SITE = HERE.parents[2] / "web" / "lm"


def test_same_slugs_as_the_data_page():
    used = set(re.findall(r'img/data/([a-z-]+)\.svg', (SITE / "data.html").read_text(encoding="utf-8")))
    assert used == set(registry())
    assert len(used) == 34


def test_pages_are_small_wordless_and_16_by_9():
    total = 0
    for slug, draw in registry().items():
        svg = draw()
        assert svg.startswith('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 180">'), slug
        assert "<text" not in svg and "<tspan" not in svg, slug
        assert svg.count("<filter ") <= 2, slug
        assert len(svg.encode()) < 60_000, slug
        total += len(svg.encode())
    assert total < 1_500_000


def test_generation_is_deterministic_and_committed():
    for slug, draw in registry().items():
        assert draw() == draw(), slug
        assert (SITE / "img" / "data-laws" / f"{slug}.svg").read_text(encoding="utf-8") == draw(), \
            f"{slug}: rerun data_art_laws.py"
