"""Write the Data page's card illustrations: web/lm/img/data/<slug>.svg (320x180, 16:9).

    python3 -I tools/lm/site/data_art.py            # rewrite web/lm/img/data/*.svg
    python3 -I tools/lm/site/data_art.py OUTDIR     # write them somewhere else

Thirty-four explanatory plates in the manner of journal scientific illustration (cut-away block
diagrams, sections, globes), drawn from one shared kit in dsart/ (see dsart/kit.py for the visual
system: palette, line weights, the oblique projection and the globe). Each plate is a light,
self-contained card; data.css dims it in dark mode, so it follows the site's theme switch as
well as the system setting. The globes use the bundled Natural Earth countries, cities and
airports in assets/examples. Standard library only, and deterministic.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))   # -I drops the script's folder from sys.path

from dsart import registry  # noqa: E402

OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[3] / "web" / "lm" / "img" / "data"


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
