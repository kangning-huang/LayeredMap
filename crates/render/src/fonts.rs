//! Font discovery: a good installed sans-serif when there is one (Segoe UI, SF, Inter, Noto Sans,
//! Open Sans, DejaVu Sans), else the Ubuntu Light face that ships with egui. Fonts are read at
//! run time and never bundled.
//!
//! Text the renderer draws (labels, legends, exported images and PDF pages) takes each character
//! from the first of these fonts that has it:
//! 1. the interface font ([`ui_fonts`]);
//! 2. fonts the embedding app registered with [`add_fallback`], in the order it added them;
//! 3. on the desktop, an installed CJK font ([`cjk_font`]: Simplified Chinese, else Japanese,
//!    else Korean), read only once a character needs it — not on the web, nor with
//!    `RUSTGIS_BUILTIN_FONTS` set.
//!
//! A character none of them has is drawn as the interface font's missing-glyph box, and map labels
//! drop the lines that have one ([`crate::text::covers`]).

use std::sync::{OnceLock, RwLock};

use ab_glyph::{FontArc, FontRef, FontVec};

/// Candidate files for (regular, semibold), in order of preference.
const CANDIDATES: &[(&str, &str)] = &[
    ("C:\\Windows\\Fonts\\segoeui.ttf", "C:\\Windows\\Fonts\\seguisb.ttf"),
    ("/System/Library/Fonts/SFNS.ttf", "/System/Library/Fonts/SFNS.ttf"),
    ("/System/Library/Fonts/Supplemental/Arial.ttf", "/System/Library/Fonts/Supplemental/Arial Bold.ttf"),
    ("/usr/share/fonts/TTF/Inter-Regular.ttf", "/usr/share/fonts/TTF/Inter-SemiBold.ttf"),
    ("/usr/share/fonts/inter/Inter-Regular.ttf", "/usr/share/fonts/inter/Inter-SemiBold.ttf"),
    ("/usr/share/fonts/noto/NotoSans-Regular.ttf", "/usr/share/fonts/noto/NotoSans-SemiBold.ttf"),
    ("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf", "/usr/share/fonts/truetype/noto/NotoSans-SemiBold.ttf"),
    ("/usr/share/fonts/google-noto/NotoSans-Regular.ttf", "/usr/share/fonts/google-noto/NotoSans-SemiBold.ttf"),
    ("/usr/share/fonts/TTF/OpenSans-Regular.ttf", "/usr/share/fonts/TTF/OpenSans-SemiBold.ttf"),
    ("/usr/share/fonts/truetype/open-sans/OpenSans-Regular.ttf", "/usr/share/fonts/truetype/open-sans/OpenSans-SemiBold.ttf"),
    ("/usr/share/fonts/TTF/DejaVuSans.ttf", "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf"),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
];

/// Largest font file accepted (32 MiB).
const MAX_FONT: u64 = 32 << 20;

pub struct UiFonts {
    pub regular: Vec<u8>,
    pub semibold: Vec<u8>,
    /// File the regular face came from, or "built-in".
    pub source: String,
}

fn read(path: &str) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > MAX_FONT {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    ab_glyph::FontRef::try_from_slice(&bytes).ok()?;
    Some(bytes)
}

/// The interface fonts, discovered once.
pub fn ui_fonts() -> &'static UiFonts {
    static F: OnceLock<UiFonts> = OnceLock::new();
    F.get_or_init(|| {
        if std::env::var_os("RUSTGIS_BUILTIN_FONTS").is_none() {
            for (r, b) in CANDIDATES {
                if let Some(regular) = read(r) {
                    let semibold = read(b).unwrap_or_else(|| regular.clone());
                    return UiFonts { regular, semibold, source: (*r).to_string() };
                }
            }
        }
        let b = epaint_default_fonts::UBUNTU_LIGHT.to_vec();
        UiFonts { regular: b.clone(), semibold: b, source: "built-in".into() }
    })
}

/// The most fallback fonts [`add_fallback`] keeps.
pub const MAX_FALLBACKS: usize = 8;

static FALLBACKS: RwLock<Vec<FontArc>> = RwLock::new(Vec::new());

/// Registers a fallback font for the text the renderer draws: face `index` of `bytes` (a TrueType
/// or OpenType font, CFF outlines included, or a collection). It is tried after the interface font
/// and the fallbacks added before it, before any installed CJK font (see the module notes). Call it
/// before rendering — an app bundling a CJK face, or the web app once it has downloaded one.
///
/// Returns `false`, keeping nothing, when the font can't be read or [`MAX_FALLBACKS`] are already
/// registered.
pub fn add_fallback(bytes: Vec<u8>, index: u32) -> bool {
    let Ok(font) = FontVec::try_from_vec_and_index(bytes, index) else { return false };
    let mut all = FALLBACKS.write().unwrap_or_else(|e| e.into_inner());
    if all.len() >= MAX_FALLBACKS {
        return false;
    }
    all.push(FontArc::new(font));
    true
}

/// The registered fallback fonts, in order.
pub(crate) fn fallbacks() -> Vec<FontArc> {
    FALLBACKS.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The installed CJK font text falls back on last, found once: `None` on the web, with
/// `RUSTGIS_BUILTIN_FONTS` set, or when none is installed.
pub(crate) fn system_cjk() -> Option<&'static FontArc> {
    static F: OnceLock<Option<FontArc>> = OnceLock::new();
    F.get_or_init(|| {
        if cfg!(target_arch = "wasm32") || std::env::var_os("RUSTGIS_BUILTIN_FONTS").is_some() {
            return None;
        }
        [CjkRegion::SimplifiedChinese, CjkRegion::Japanese, CjkRegion::Korean]
            .into_iter()
            .filter_map(cjk_font)
            .find_map(|(bytes, index)| FontRef::try_from_slice_and_index(bytes, *index).ok())
            .map(FontArc::new)
    })
    .as_ref()
}

/// Regional CJK glyph styles (the same code point is drawn differently by region).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CjkRegion {
    SimplifiedChinese,
    Japanese,
    Korean,
}

/// Installed CJK font candidates: (path, face index in a collection).
fn cjk_candidates(region: CjkRegion) -> &'static [(&'static str, u32)] {
    // Noto Sans CJK collections order their faces JP, KR, SC, TC, HK.
    match region {
        CjkRegion::SimplifiedChinese => &[
            ("C:\\Windows\\Fonts\\msyh.ttc", 0),
            ("/System/Library/Fonts/PingFang.ttc", 0),
            ("/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
            ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 2),
            ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 2),
            ("/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc", 2),
            ("/usr/share/fonts/noto-cjk/NotoSansSC-Regular.otf", 0),
            ("/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc", 0),
        ],
        CjkRegion::Japanese => &[
            ("C:\\Windows\\Fonts\\YuGothR.ttc", 0),
            ("C:\\Windows\\Fonts\\meiryo.ttc", 0),
            ("/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc", 0),
            ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 0),
            ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
            ("/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc", 0),
            ("/usr/share/fonts/noto-cjk/NotoSansJP-Regular.otf", 0),
        ],
        CjkRegion::Korean => &[
            ("C:\\Windows\\Fonts\\malgun.ttf", 0),
            ("/System/Library/Fonts/AppleSDGothicNeo.ttc", 0),
            ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 1),
            ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 1),
            ("/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc", 1),
            ("/usr/share/fonts/noto-cjk/NotoSansKR-Regular.otf", 0),
        ],
    }
}

/// An installed CJK font for `region` (bytes and face index), found once. `None` on the web or
/// when nothing suitable is installed.
pub fn cjk_font(region: CjkRegion) -> Option<&'static (Vec<u8>, u32)> {
    static FOUND: [OnceLock<Option<(Vec<u8>, u32)>>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let slot = match region {
        CjkRegion::SimplifiedChinese => &FOUND[0],
        CjkRegion::Japanese => &FOUND[1],
        CjkRegion::Korean => &FOUND[2],
    };
    slot.get_or_init(|| {
        cjk_candidates(region).iter().find_map(|(path, index)| {
            let meta = std::fs::metadata(path).ok()?;
            // CJK collections are large; accept up to 128 MiB.
            if meta.len() > 128 << 20 {
                return None;
            }
            std::fs::read(path).ok().filter(|b| valid_font(b, *index)).map(|b| (b, *index))
        })
    })
    .as_ref()
}

/// Whether `bytes` hold a font with face `index` that the font parser accepts. Fonts handed to the
/// UI are checked first: a corrupt file or an out-of-range face index must never reach it.
pub fn valid_font(bytes: &[u8], index: u32) -> bool {
    ab_glyph::FontRef::try_from_slice_and_index(bytes, index).is_ok()
}

/// Where the web app downloads a CJK face from (Noto Sans, SIL Open Font License).
pub fn cjk_font_url(region: CjkRegion) -> &'static str {
    match region {
        CjkRegion::SimplifiedChinese => "https://assets.rustgis.app/fonts/NotoSansSC-Regular.otf",
        CjkRegion::Japanese => "https://assets.rustgis.app/fonts/NotoSansJP-Regular.otf",
        CjkRegion::Korean => "https://assets.rustgis.app/fonts/NotoSansKR-Regular.otf",
    }
}
