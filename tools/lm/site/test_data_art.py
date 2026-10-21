"""The Data page illustrations: one per card, the right size, no text, deterministic."""
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from dsart import registry  # noqa: E402

SITE = HERE.parents[2] / "web" / "lm"


def test_every_card_has_an_illustration_and_no_orphans():
    # the site now shows the nature-journal set (img/data-laws/); this v3 set stays as the alternative
    used = set(re.findall(r'img/data(?:-laws)?/([a-z-]+)\.svg', (SITE / "data.html").read_text(encoding="utf-8")))
    assert used == set(registry())
    assert len(used) == 34


def test_plates_are_small_textless_and_16_by_9():
    for slug, draw in registry().items():
        svg = draw()
        assert svg.startswith('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 180">'), slug
        assert "<text" not in svg, slug
        assert len(svg.encode()) < 60_000, slug


def test_generation_is_deterministic_and_committed():
    for slug, draw in registry().items():
        assert draw() == draw(), slug
        assert (SITE / "img" / "data" / f"{slug}.svg").read_text(encoding="utf-8") == draw(), f"{slug}: rerun data_art.py"
