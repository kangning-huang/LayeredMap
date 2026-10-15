# Bundled fonts

Inter for Latin text and Noto Sans SC for Chinese, as static files that egui (ab_glyph) can load.
egui does not apply OpenType features, so Inter is rebuilt once with `tnum` (tabular figures) frozen in as the default.

| File | Source | Upstream version | License | SHA-256 | Size (bytes) | Modification |
|---|---|---|---|---|---|---|
| `Inter-Regular.ttf` | https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip → `extras/ttf/Inter-Regular.ttf` (release v4.1) | 4.001;git-9221beed3 | OFL-1.1 (`OFL-Inter.txt`) | `b572b937baec4e1b2fa485c7ad10ae273b4c094f48eba37cb0cb44e09cd9a5b1` | 411736 | tnum frozen with opentype-feature-freezer 1.32.2 (fontTools 4.67.0) |
| `Inter-SemiBold.ttf` | same zip → `extras/ttf/Inter-SemiBold.ttf` (release v4.1) | 4.001;git-9221beed3 | OFL-1.1 (`OFL-Inter.txt`) | `24251220c712f68de9424d9a0f6159072b2c98af7a2c0071216a2596f50ad0df` | 419840 | tnum frozen with opentype-feature-freezer 1.32.2 (fontTools 4.67.0) |
| `NotoSansSC-Regular.otf` | https://github.com/notofonts/noto-cjk/blob/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/SubsetOTF/SC/NotoSansSC-Regular.otf | 2.004 | OFL-1.1 (`OFL-NotoSansSC.txt`) | `faa6c9df652116dde789d351359f3d7e5d2285a2b2a1f04a2d7244df706d5ea9` | 8331336 | none |
| `NotoSansSC-Medium.otf` | https://github.com/notofonts/noto-cjk/blob/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/SubsetOTF/SC/NotoSansSC-Medium.otf | 2.004 | OFL-1.1 (`OFL-NotoSansSC.txt`) | `7633f5a016d4dd95e685a69633d818aabc4644c4b08e26bd35b1b30c45ed5dda` | 8345436 | none |
| `OFL-Inter.txt` | `LICENSE.txt` from the Inter v4.1 zip | — | — | `262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a` | 4380 | none |
| `OFL-NotoSansSC.txt` | https://github.com/google/fonts/blob/2894aab31764f10f29c421bdfd2340d3b382d384/ofl/notosanssc/OFL.txt | — | — | `1c05c68c34f9708415aada51f17e1b0092d2cea709bf4a94cd38114f9e73d7d9` | 4388 | none |

Inter-4.1.zip SHA-256: `9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e`.
Unmodified upstream Inter TTFs: Regular `40d692fce188e4471e2b3cba937be967878f631ad3ebbbdcd587687c7ebe0c82`, SemiBold `78a843fade9d4612a5567302fb595b56976eb5fcebf4fea5a5912d638bafcde3`.

## Notes

- **There is no SemiBold Noto Sans SC.** Upstream static Noto Sans SC (noto-cjk SubsetOTF and OTF) ships Thin, Light, DemiLight, Regular, Medium, Bold and Black, with no 600 weight. Google Fonts only ships the variable `NotoSansSC[wght].ttf`. The bold-ish CJK face is therefore **Medium (usWeightClass 500)**. It pairs with Inter SemiBold (600).
- The upstream noto-cjk `Sans/LICENSE` has no copyright line, so `OFL-NotoSansSC.txt` is the Google Fonts copy of the same OFL text. It starts with "Copyright 2014-2021 Adobe (http://www.adobe.com/), with Reserved Font Name 'Source'".
- Outline formats: Inter = TrueType (`glyf`); Noto Sans SC = CFF (`CFF `), not CFF2. All four fonts are static (no `fvar`), and ab_glyph/ttf-parser can read both formats.
- Family names did not change. `pyftfeatfreeze` renames only when you pass `-s`/`-S`/`-U`/`-R`, and none was used. The name tables match upstream: `Inter`, `Inter SemiBold`, `Noto Sans SC`, `Noto Sans SC Medium`.
- Digit advance widths (units per em 2048):
  - Upstream Inter Regular has 9 distinct widths from 833 to 1323. Frozen: all 1328.
  - Upstream Inter SemiBold has 9 distinct widths from 866 to 1364. Frozen: all 1325.
  - Noto Sans SC digits are already equal: 555 for Regular, 570 for Medium.
- Coverage was checked with `check_fonts.py`. Inter covers 0-9, °, ², × and − (U+2212). Noto Sans SC also covers 上海市徐汇区.

## Reproduce

```bash
mamba create -y -n lm-fonts python=3.12 fonttools
mamba run -n lm-fonts pip install opentype-feature-freezer==1.32.2

curl -LO https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip
unzip Inter-4.1.zip LICENSE.txt extras/ttf/Inter-Regular.ttf extras/ttf/Inter-SemiBold.ttf
for w in Regular SemiBold; do
  mamba run -n lm-fonts pyftfeatfreeze -f tnum extras/ttf/Inter-$w.ttf Inter-$w.ttf
done

C=f8d157532fbfaeda587e826d4cd5b21a49186f7c
for w in Regular Medium; do
  curl -LO https://raw.githubusercontent.com/notofonts/noto-cjk/$C/Sans/SubsetOTF/SC/NotoSansSC-$w.otf
done
curl -L -o OFL-NotoSansSC.txt https://raw.githubusercontent.com/google/fonts/2894aab31764f10f29c421bdfd2340d3b382d384/ofl/notosanssc/OFL.txt
cp LICENSE.txt OFL-Inter.txt

mamba run -n lm-fonts python -I check_fonts.py *.ttf *.otf
```

A re-frozen Inter may not hash identically, because fontTools rewrites `head.modified` on save. Check it with `check_fonts.py`, which should report the same digit widths.
