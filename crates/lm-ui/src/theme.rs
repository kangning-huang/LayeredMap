//! Visual design system tokens as one `Theme` (WP1.1): colour, type, spacing, shape.
//! UI colour and data colour are separate systems; the accent never appears in a data palette.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

/// Light or dark: follows the system unless the View menu overrides it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ThemePref {
    #[default]
    System,
    Light,
    Dark,
}

/// The colour tokens of one theme (Visual design system › Color tokens).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub dark: bool,
    pub surface: Color32,
    pub surface_raised: Color32,
    pub border: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub accent: Color32,
    /// Text on an accent background.
    pub on_accent: Color32,
    pub accent_weak: Color32,
    pub assumption: Color32,
    pub warning: Color32,
    pub critical: Color32,
    pub success: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

impl Theme {
    pub const LIGHT: Theme = Theme {
        dark: false,
        surface: hex(0xFFFFFF),
        surface_raised: hex(0xF6F7F9),
        border: hex(0xE3E5E8),
        text_primary: hex(0x1B1D21),
        text_secondary: hex(0x5F6670),
        accent: hex(0x2563EB),
        on_accent: hex(0xFFFFFF),
        accent_weak: hex(0xDCE7FD),
        assumption: hex(0x6D4FD1),
        warning: hex(0xB45309),
        critical: hex(0xB91C1C),
        success: hex(0x15803D),
    };
    pub const DARK: Theme = Theme {
        dark: true,
        surface: hex(0x1E1F22),
        surface_raised: hex(0x2A2B2F),
        border: hex(0x3A3C41),
        text_primary: hex(0xF1F2F4),
        text_secondary: hex(0xA9AEB6),
        accent: hex(0x5B8DEF),
        on_accent: hex(0x0B1220),
        accent_weak: hex(0x24324F),
        assumption: hex(0xA78BFA),
        warning: hex(0xF59E0B),
        critical: hex(0xF87171),
        success: hex(0x4ADE80),
    };

    pub fn of(dark: bool) -> Theme {
        if dark { Theme::DARK } else { Theme::LIGHT }
    }
}

// Type, spacing, shape (Visual design system › Type, spacing, shape).
pub const CAPTION: f32 = 12.0;
pub const BODY: f32 = 14.0;
pub const HEADLINE: f32 = 16.0;
pub const TITLE: f32 = 20.0;
pub const BIG_NUMBER: f32 = 28.0;
pub const SPACE: [f32; 6] = [4.0, 8.0, 12.0, 16.0, 24.0, 32.0];
pub const PANEL_PADDING: f32 = 16.0;
pub const RADIUS_CONTROL: u8 = 6;
pub const RADIUS_CARD: u8 = 8;
pub const RADIUS_PANEL: u8 = 12;
pub const ICON_HIT: f32 = 36.0;
pub const ROW_HEIGHT: f32 = 32.0;
pub const TOOLBAR_HEIGHT: f32 = 52.0;
/// Panels sit this far from the window's edges.
pub const PANEL_MARGIN: f32 = 12.0;

/// An icon from the Phosphor font. Icons are always set in their own family: Inter has glyphs of
/// its own at some of the private-use code points Phosphor uses.
pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("phosphor".into()))
}

/// The semibold family (Inter SemiBold, Noto Sans SC Medium).
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Noto Sans SC Regular, shared by the interface and the map renderer's labels.
pub const NOTO_SC_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/NotoSansSC-Regular.otf");

/// Bundled fonts (WP0.4): Inter with tabular figures frozen in, Noto Sans SC for Chinese, and
/// the Phosphor icon font. System fonts are not used.
pub fn fonts() -> egui::FontDefinitions {
    let mut defs = egui::FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        defs.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
    };
    add("Inter", include_bytes!("../../../assets/fonts/Inter-Regular.ttf"));
    add("Inter SemiBold", include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf"));
    add("Noto Sans SC", NOTO_SC_REGULAR);
    add("Noto Sans SC Medium", include_bytes!("../../../assets/fonts/NotoSansSC-Medium.otf"));
    let regular: Vec<String> = vec!["Inter".into(), "Noto Sans SC".into()];
    let bold: Vec<String> = vec!["Inter SemiBold".into(), "Noto Sans SC Medium".into()];
    let mut prop = regular.clone();
    prop.extend(defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    defs.families.insert(FontFamily::Proportional, prop);
    let mut sb = bold;
    sb.extend(defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default());
    defs.families.insert(FontFamily::Name("semibold".into()), sb);
    egui_phosphor::add_to_fonts(&mut defs, egui_phosphor::Variant::Regular);
    // The icon family puts Phosphor first: the crate puts it second (after the text font), and
    // Inter has glyphs of its own at some of Phosphor's private-use code points.
    if let Some(list) = defs.families.get_mut(&FontFamily::Name("phosphor".into())) {
        list.retain(|f| f != "phosphor");
        list.insert(0, "phosphor".into());
    }
    // Icons in the semibold family too, so icon buttons can use it.
    if let Some(list) = defs.families.get_mut(&FontFamily::Name("semibold".into())) {
        list.push("phosphor".into());
    }
    defs
}

/// Applies the theme to egui's style: surfaces, text, accent, radii, spacing, sizes.
pub fn apply(ctx: &egui::Context, t: &Theme) {
    ctx.all_styles_mut(|s| {
        s.text_styles = [
            (TextStyle::Small, regular(CAPTION)),
            (TextStyle::Body, regular(BODY)),
            (TextStyle::Button, regular(BODY)),
            (TextStyle::Heading, semibold(TITLE)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();
        let v = &mut s.visuals;
        *v = if t.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        v.panel_fill = t.surface;
        v.window_fill = t.surface;
        v.extreme_bg_color = t.surface_raised;
        v.faint_bg_color = t.surface_raised;
        v.window_stroke = Stroke::new(1.0, t.border);
        v.window_corner_radius = CornerRadius::same(RADIUS_PANEL);
        v.menu_corner_radius = CornerRadius::same(RADIUS_CARD);
        v.hyperlink_color = t.accent;
        v.selection.bg_fill = t.accent_weak;
        v.selection.stroke = Stroke::new(1.5, t.accent);
        v.override_text_color = Some(t.text_primary);
        v.weak_text_color = Some(t.text_secondary);
        // Panels get one soft shadow in light; dark uses the border instead.
        v.window_shadow = if t.dark {
            egui::epaint::Shadow::NONE
        } else {
            egui::epaint::Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(31) }
        };
        v.popup_shadow = v.window_shadow;
        for (w, fill) in [
            (&mut v.widgets.noninteractive, t.surface),
            (&mut v.widgets.inactive, t.surface_raised),
            (&mut v.widgets.hovered, t.surface_raised),
            (&mut v.widgets.active, t.accent_weak),
            (&mut v.widgets.open, t.surface_raised),
        ] {
            w.bg_fill = fill;
            w.weak_bg_fill = fill;
            w.corner_radius = CornerRadius::same(RADIUS_CONTROL);
            w.fg_stroke = Stroke::new(1.0, t.text_primary);
        }
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.border);
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        // The focus ring is the accent.
        v.widgets.active.bg_stroke = Stroke::new(2.0, t.accent);
        s.spacing.item_spacing = egui::vec2(SPACE[1], SPACE[1]);
        s.spacing.button_padding = egui::vec2(SPACE[2], SPACE[0] + 2.0);
        s.spacing.interact_size.y = ROW_HEIGHT;
        s.spacing.window_margin = egui::Margin::same(PANEL_PADDING as i8);
    });
}

/// WCAG 2 contrast ratio between two opaque colours.
pub fn contrast(a: Color32, b: Color32) -> f64 {
    fn lum(c: Color32) -> f64 {
        let ch = |v: u8| {
            let s = f64::from(v) / 255.0;
            if s <= 0.039_28 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(c.r()) + 0.7152 * ch(c.g()) + 0.0722 * ch(c.b())
    }
    let (la, lb) = (lum(a), lum(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Gate 1: body and secondary text at least 4.5:1 against their surfaces; icons and focus
    // rings at least 3:1.
    #[test]
    fn every_token_passes_its_contrast_check() {
        for t in [Theme::LIGHT, Theme::DARK] {
            for surface in [t.surface, t.surface_raised] {
                assert!(contrast(t.text_primary, surface) >= 4.5, "primary on {surface:?} (dark {})", t.dark);
                assert!(contrast(t.text_secondary, surface) >= 4.5, "secondary on {surface:?} (dark {})", t.dark);
                assert!(contrast(t.accent, surface) >= 3.0, "accent/focus on {surface:?} (dark {})", t.dark);
                for status in [t.assumption, t.warning, t.critical, t.success] {
                    assert!(contrast(status, surface) >= 3.0, "{status:?} on {surface:?} (dark {})", t.dark);
                }
            }
            assert!(contrast(t.on_accent, t.accent) >= 4.5, "text on accent (dark {})", t.dark);
            assert!(contrast(t.text_primary, t.accent_weak) >= 4.5, "text on accent-weak (dark {})", t.dark);
        }
    }

    #[test]
    fn contrast_matches_known_values() {
        assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 1e-9);
        assert!((contrast(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 1e-9);
    }
}
