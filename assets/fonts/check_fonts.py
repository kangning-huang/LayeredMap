import sys
from fontTools.ttLib import TTFont
for p in sys.argv[1:]:
    f = TTFont(p, lazy=True)
    n = f["name"]
    fmt = "glyf" if "glyf" in f else "CFF2" if "CFF2" in f else "CFF" if "CFF " in f else "?"
    cmap = f.getBestCmap(); hm = f["hmtx"]
    digits = [hm[cmap[ord(c)]][0] for c in "0123456789"]
    zh = all(ord(c) in cmap for c in "上海市徐汇区")
    sym = {c: ord(c) in cmap for c in "°²×−"}
    print(p.split("/")[-1], "|", n.getDebugName(1), "/", n.getDebugName(2), "| typo:", n.getDebugName(16), n.getDebugName(17),
          "| ver:", n.getDebugName(5), "| wght:", f["OS/2"].usWeightClass, "| glyphs:", f["maxp"].numGlyphs,
          "| outline:", fmt, "| variable:", "fvar" in f, "| zh:", zh, "| sym:", sym,
          "| digit adv:", sorted(set(digits)), "tabular" if len(set(digits)) == 1 else "PROPORTIONAL")
