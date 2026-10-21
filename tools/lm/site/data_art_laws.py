"""Write the nature-journal set of Data page illustrations: web/lm/img/data-laws/<slug>.svg (320x180).

    python3 -I tools/lm/site/data_art_laws.py            # rewrite web/lm/img/data-laws/*.svg
    python3 -I tools/lm/site/data_art_laws.py OUTDIR     # write them somewhere else

An alternative to the plates in web/lm/img/data/ (written by data_art.py), offered for comparison:
the same 34 datasets drawn as pages from a field journal, in the manner of John Muir Laws's nature
journaling (pen first, then transparent watercolour; a main study with zoomed insets, leader lines,
arrows and scale bars). The cards appear in 11 languages, so no drawing contains words, letters or
digits. See dsart_laws/kit.py for the visual system. Standard library only, and deterministic.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))   # -I drops the script's folder from sys.path

from dsart_laws import registry  # noqa: E402

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[3] / "web" / "lm" / "img" / "data-laws"


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    arts = registry()
    total = 0
    for slug, draw in arts.items():
        text = draw()
        (OUT / f"{slug}.svg").write_text(text, encoding="utf-8")
        total += len(text.encode("utf-8"))
    print(f"{len(arts)} illustrations, {total / 1024:.0f} KB")


if __name__ == "__main__":
    main()
