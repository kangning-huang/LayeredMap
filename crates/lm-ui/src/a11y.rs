//! Screen readers and the keyboard (WP5.6, WP6.3; Interaction specification › Keyboard).
//!
//! egui builds an AccessKit tree from its widgets, which VoiceOver (and later Narrator) reads.
//! This module fills its gaps:
//! - names for custom-drawn controls and icon-only buttons ([`name`], [`toggle`]): a Phosphor
//!   glyph is a private-use character, which a screen reader reads as nothing or as garbage;
//! - the map canvas as one focusable node described in words ([`map_description`]); searchable
//!   records (search, the table) and the selection summary are its alternative (WP6.3);
//! - polite live regions ([`live`]): the selection summary ([`selection_summary`]), notices and
//!   result cards;
//! - a visible focus ring for keyboard focus, hidden again after a pointer press (like the web's
//!   `:focus-visible`), and F6 between regions (toolbar → shelf → map → inspector → table).

use egui::accesskit::{Live, Role};
use egui::{Context, Id, Response, WidgetInfo, WidgetType};

use lm_core::selection::SelectionContext;

use crate::Shell;

/// How long a selection summary waits for the layer's rank index before it is read without the
/// rank.
const RANK_WAIT: f64 = 1.5;

/// The regions F6 moves between, in the Tab order of the spec (Keyboard › Moving between regions).
const REGIONS: [&str; 5] = ["lm-toolbar", "lm-shelf", "lm-map", "lm-inspector", "lm-table"];

/// Per-window accessibility state.
#[derive(Debug, Default)]
pub struct A11y {
    /// Focus came from the keyboard (Tab, arrows, F6), so the focus ring shows.
    pub(crate) keyboard: bool,
    /// The map canvas's widget, once drawn.
    pub(crate) map_id: Option<Id>,
    /// The selection last announced, and the one waiting to be (with when it changed).
    announced: Option<SelectionContext>,
    pending: Option<(SelectionContext, f64)>,
    /// The latest selection announcement, read by the polite live region.
    pub(crate) announcement: String,
}

/// Gives a control its accessible name and role (custom-drawn controls have neither, and an
/// icon button's text is a glyph).
pub(crate) fn name(resp: &Response, typ: WidgetType, label: impl Into<String>) {
    let (enabled, label) = (resp.enabled(), label.into());
    resp.widget_info(|| WidgetInfo::labeled(typ, enabled, label.clone()));
}

/// A button's accessible name.
pub(crate) fn button(resp: &Response, label: impl Into<String>) {
    name(resp, WidgetType::Button, label);
}

/// A control that is on or off (a layer's visibility, the chosen basemap).
pub(crate) fn toggle(resp: &Response, label: impl Into<String>, on: bool) {
    let (enabled, label) = (resp.enabled(), label.into());
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, on, label.clone()));
}

/// One of a set of choices (a basemap, a tab, a segment).
pub(crate) fn choice(resp: &Response, label: impl Into<String>, on: bool) {
    let (enabled, label) = (resp.enabled(), label.into());
    resp.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, enabled, on, label.clone()));
}

/// Names a control without touching its role or value (text fields, sliders).
pub(crate) fn label(resp: &Response, label: &str) {
    resp.ctx.accesskit_node_builder(resp.id, |n| n.set_label(label));
}

/// Text drawn with the painter, which the tree wouldn't otherwise have.
pub(crate) fn text(resp: &Response, text: &str) {
    let text = text.trim().to_string();
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text.clone()));
}

/// A longer description, read after the name (VoiceOver's hint).
pub(crate) fn describe(resp: &Response, text: &str) {
    resp.ctx.accesskit_node_builder(resp.id, |n| n.set_description(text));
}

/// How many widgets `ui`'s layer has so far this frame (see [`name_new_controls`]).
pub(crate) fn widget_mark(ui: &egui::Ui) -> usize {
    let layer = ui.layer_id();
    ui.ctx().viewport(|vp| vp.this_pass.widgets.get_layer(layer).count())
}

/// Names the controls made since `mark` that have no name of their own with `label` (a form
/// row's drawn label, a pop-up next to a caption).
pub(crate) fn name_new_controls(ui: &egui::Ui, mark: usize, label: &str) {
    let layer = ui.layer_id();
    let ids: Vec<Id> = ui.ctx().viewport(|vp| {
        let nodes = vp.this_pass.accesskit_state.as_ref().map(|s| &s.nodes);
        vp.this_pass
            .widgets
            .get_layer(layer)
            .skip(mark)
            .filter(|w| w.sense.is_focusable())
            .filter(|w| nodes.and_then(|n| n.get(&w.id)).is_some_and(|n| n.label().is_none_or(|l| without_glyphs(l).is_empty())))
            .map(|w| w.id)
            .collect()
    });
    for id in ids {
        ui.ctx().accesskit_node_builder(id, |n| n.set_label(label));
    }
}

/// Text without Phosphor's private-use glyphs, for names built from a button's own label.
pub fn without_glyphs(s: &str) -> String {
    s.chars().filter(|c| !is_glyph(*c)).collect::<String>().trim().to_string()
}

/// A private-use code point (Phosphor's glyphs live there).
pub fn is_glyph(c: char) -> bool {
    matches!(c as u32, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD)
}

/// A polite live region: VoiceOver reads `text` when it appears or changes, without moving focus.
pub(crate) fn live(ctx: &Context, id: Id, text: &str) {
    if text.is_empty() {
        return;
    }
    ctx.accesskit_node_builder(id, |n| {
        n.set_role(Role::Label);
        n.set_value(text);
        n.set_live(Live::Polite);
    });
}

/// Reads what drawn text says in words: units as a screen reader should speak them.
pub fn spoken(s: &str) -> String {
    let mut out = s.to_string();
    for (sym, words) in [
        ("°C", "degrees Celsius"),
        ("°F", "degrees Fahrenheit"),
        ("km²", "square kilometres"),
        ("m²", "square metres"),
        ("mi²", "square miles"),
        ("°", " degrees"),
        ("·", ","),
        ("−", "minus "),
    ] {
        out = out.replace(sym, words);
    }
    // "31.2  degrees" from a bare "°" after a space.
    while out.contains("  ") {
        out = out.replace("  ", " ");
    }
    out.replace(" ,", ",")
}

/// Before the frame's widgets: who has the keyboard, and F6.
pub(crate) fn begin(shell: &mut Shell, ctx: &Context) {
    let (keys, pointer, f6, back) = ctx.input(|i| {
        let keys = i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::Key {
                    key: egui::Key::Tab
                        | egui::Key::ArrowUp
                        | egui::Key::ArrowDown
                        | egui::Key::ArrowLeft
                        | egui::Key::ArrowRight
                        | egui::Key::F6,
                    pressed: true,
                    ..
                }
            )
        });
        let pointer = i.events.iter().any(|e| matches!(e, egui::Event::PointerButton { pressed: true, .. }));
        (keys, pointer, i.key_pressed(egui::Key::F6), i.modifiers.shift)
    });
    if keys {
        shell.a11y.keyboard = true;
    }
    if pointer {
        shell.a11y.keyboard = false;
    }
    if f6 {
        ctx.input_mut(|i| i.consume_key(i.modifiers, egui::Key::F6));
        next_region(shell, ctx, back);
    }
}

/// The first focusable widget of a region (as drawn in the last frame).
fn region_start(shell: &Shell, ctx: &Context, region: &str) -> Option<Id> {
    if region == "lm-map" {
        return shell.a11y.map_id;
    }
    let layer = egui::LayerId::new(egui::Order::Middle, Id::new(region));
    ctx.viewport(|vp| vp.prev_pass.widgets.get_layer(layer).find(|w| w.sense.is_focusable() && w.enabled).map(|w| w.id))
}

/// Which region holds the widget with focus.
fn region_of(shell: &Shell, ctx: &Context, id: Id) -> Option<usize> {
    if Some(id) == shell.a11y.map_id {
        return Some(2);
    }
    let layer = ctx.viewport(|vp| vp.prev_pass.widgets.get(id).map(|w| w.layer_id))?;
    REGIONS.iter().position(|r| layer == egui::LayerId::new(egui::Order::Middle, Id::new(*r)))
}

/// F6 (⇧F6 back): focus the first control of the next region that is showing.
fn next_region(shell: &mut Shell, ctx: &Context, back: bool) {
    let n = REGIONS.len();
    let here = ctx.memory(|m| m.focused()).and_then(|id| region_of(shell, ctx, id));
    for step in 1..=n {
        let i = match (here, back) {
            (Some(h), false) => (h + step) % n,
            (Some(h), true) => (h + n - step) % n,
            (None, false) => step - 1,
            (None, true) => n - step,
        };
        if let Some(id) = region_start(shell, ctx, REGIONS[i]) {
            ctx.memory_mut(|m| m.request_focus(id));
            shell.a11y.keyboard = true;
            return;
        }
    }
}

/// The map canvas: a name and description in words, arrows for panning while it has focus.
pub(crate) fn map_canvas(shell: &mut Shell, ctx: &Context, id: Id) {
    shell.a11y.map_id = Some(id);
    let (label, help) = map_description(shell);
    ctx.accesskit_node_builder(id, |n| {
        n.set_role(Role::Canvas);
        n.set_role_description("map");
        n.set_label(label);
        n.set_description(help);
    });
    // While the map has the keyboard, arrows pan it rather than moving focus; Tab still leaves.
    if ctx.memory(|m| m.has_focus(id)) {
        ctx.memory_mut(|m| {
            m.set_focus_lock_filter(id, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() })
        });
    }
}

/// The map in words: what it shows and where (the label), and how to reach its records (the
/// description, read after a pause).
pub fn map_description(shell: &Shell) -> (String, String) {
    let shown: Vec<&str> = shell.ws.doc.layers.iter().rev().filter(|l| l.visible).map(|l| l.display_name.as_str()).collect();
    let layers = match shown.len() {
        0 if shell.ws.doc.layers.is_empty() => "No data yet".to_string(),
        0 => "Every layer is hidden".to_string(),
        _ => format!("Showing {}", list(&shown)),
    };
    let place = match shell.ws.view_extent {
        Some(e) => extent_words(&e),
        None => String::new(),
    };
    let label = if place.is_empty() { format!("Map. {layers}.") } else { format!("Map. {layers}. {place}.") };
    let help = if shell.ws.doc.layers.is_empty() {
        "Open a file or an example to add data. Search places with Command-F.".to_string()
    } else {
        "Arrow keys pan, plus and minus zoom. The layer list and the data table (Command-T) list every record; \
         Command-F finds one."
            .to_string()
    };
    (label, help)
}

/// "districts, schools and roads".
fn list(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [a] => (*a).to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The view's extent in words: "About 60 km across, centred at 31.2° N, 121.5° E", or "The whole
/// world".
pub fn extent_words(e: &lm_core::model::Extent) -> String {
    let lat = ((e.min_y + e.max_y) / 2.0).clamp(-90.0, 90.0);
    let lon = (e.min_x + e.max_x) / 2.0;
    let km = (e.max_x - e.min_x).abs() * 111.32 * lat.to_radians().cos();
    if km > 15_000.0 || (e.max_x - e.min_x) >= 300.0 {
        return "The whole world".to_string();
    }
    let across = if km >= 10.0 {
        lm_core::model::group_thousands((km / if km >= 1000.0 { 100.0 } else { 10.0 }).round() as u64 * if km >= 1000.0 { 100 } else { 10 })
    } else {
        format!("{:.0}", km.max(1.0))
    };
    let ns = if lat >= 0.0 { "north" } else { "south" };
    let ew = if lon >= 0.0 { "east" } else { "west" };
    format!("About {across} kilometres across, centred at {:.1} degrees {ns}, {:.1} degrees {ew}", lat.abs(), lon.abs())
}

/// What a screen reader says when the selection changes, and whether it is complete (`false`
/// while the layer's rank index is still being built).
///
/// "District 7 selected. Heat 31.2 degrees Celsius, 3rd highest of 17." The value is the styled
/// field's when the layer is styled by one (insight's first field), else the first starred or
/// measure field.
pub fn selection_summary(shell: &Shell, sel: &SelectionContext) -> (String, bool) {
    if let Some(p) = &sel.pixel {
        return (pixel_summary(shell, p), true);
    }
    match sel.features.as_slice() {
        [] => match sel.active_layer.and_then(|id| shell.ws.doc.layer(id)) {
            Some(l) => (spoken(&format!("{} layer selected. {}.", l.display_name, l.profile.subtitle())), true),
            None => ("Selection cleared.".to_string(), true),
        },
        [f] => {
            let Some(l) = shell.ws.doc.layer(f.layer) else { return ("Selection cleared.".into(), true) };
            let Some(ds) = crate::inspect::dataset(shell, l.id) else { return (format!("{} selected.", l.display_name), false) };
            let title = lm_jobs::inspect::object_title(l, &ds, f.row);
            let Some(index) = shell.ws.layer_index(l.id) else { return (format!("{title} selected."), false) };
            let row = lm_jobs::inspect::insight_rows(l, &ds, &index, f.row, 1).into_iter().next();
            let detail = match row {
                Some(r) => match r.phrase {
                    Some(p) => format!(" {} {}, {p}.", r.field, r.value),
                    None => format!(" {} {}.", r.field, r.value),
                },
                None => String::new(),
            };
            (spoken(&format!("{title} selected.{detail}")), true)
        }
        many => {
            let same = many.iter().all(|f| f.layer == many[0].layer);
            let noun = match shell.ws.doc.layer(many[0].layer) {
                Some(l) if same => lm_jobs::inspect::noun_plural(l),
                _ => "objects".to_string(),
            };
            (format!("{} {noun} selected.", lm_core::model::group_thousands(many.len() as u64)), true)
        }
    }
}

fn pixel_summary(shell: &Shell, p: &lm_core::selection::PixelPick) -> String {
    let Some(l) = shell.ws.doc.layer(p.layer) else { return "Cell selected.".into() };
    let place = format!(
        "{:.2} degrees {}, {:.2} degrees {}",
        p.lat.abs(),
        if p.lat >= 0.0 { "north" } else { "south" },
        p.lon.abs(),
        if p.lon >= 0.0 { "east" } else { "west" }
    );
    let value = match (shell.ws.data(l.id).map(|d| d.data.clone()), p.value) {
        (Some(lm_jobs::import::LayerData::Raster(r)), Some(v)) => {
            let band = lm_jobs::inspect::style_band(&r, &l.style);
            let unit = l.profile.bands.get(band).and_then(|b| b.unit.value.clone());
            let (value, phrase) = lm_jobs::inspect::pixel_insight(&r, band, v, unit.as_deref());
            match phrase {
                Some(ph) => format!("{value}, {ph}"),
                None => value,
            }
        }
        (_, Some(v)) => lm_jobs::inspect::number_text(v),
        (_, None) => "No value".to_string(),
    };
    spoken(&format!("{} cell at {place} selected. {value}.", l.display_name))
}

/// After the frame's widgets: the selection announcement, and the focus ring.
pub(crate) fn end(shell: &mut Shell, ctx: &Context) {
    let now = ctx.input(|i| i.time);
    let sel = shell.ws.selection.clone();
    let changed = shell.a11y.pending.as_ref().map_or(shell.a11y.announced.as_ref(), |(s, _)| Some(s)) != Some(&sel);
    if changed {
        // The first frame's selection (an opened map) is not news.
        if shell.a11y.announced.is_none() && shell.a11y.pending.is_none() && sel.is_empty() {
            shell.a11y.announced = Some(sel.clone());
        } else {
            shell.a11y.pending = Some((sel.clone(), now));
        }
    }
    if let Some((s, since)) = shell.a11y.pending.clone() {
        let (text, complete) = selection_summary(shell, &s);
        if complete || now - since > RANK_WAIT {
            shell.a11y.announcement = text;
            shell.a11y.announced = Some(s);
            shell.a11y.pending = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    live(ctx, Id::new("lm-announce-selection"), &shell.a11y.announcement.clone());
    strip_glyphs(ctx);
    focus_ring(shell, ctx);
}

/// Icon glyphs out of the frame's tree: a name that mixes a glyph and words keeps the words
/// ("\u{e1a4}  Rename" → "Rename"), and text that is only glyphs (a decorative icon) is hidden.
/// Controls named by a glyph alone are left for [`name`]; the tests list any that remain.
fn strip_glyphs(ctx: &Context) {
    enum Fix {
        Label(Id, String),
        Value(Id, String),
        Hide(Id),
        Group(Id),
    }
    let fixes: Vec<Fix> = ctx.viewport(|vp| {
        let Some(state) = vp.this_pass.accesskit_state.as_ref() else { return Vec::new() };
        let mut out = Vec::new();
        // A pop-up, slider or value box without a name takes the caption just left of it on the
        // same line ("Statistic [Mean ▾]"), as VoiceOver would guess from the screen.
        // egui's own unnamed nodes: the hit area of a wrapped link's second line takes the link's
        // name; the background of a menu or window (drag to move, click to keep it open) is a
        // group, not a control.
        let by_node: std::collections::HashMap<egui::accesskit::NodeId, (Id, &egui::accesskit::Node)> =
            state.nodes.iter().map(|(id, n)| (id.accesskit_id(), (*id, n))).collect();
        for parent in state.nodes.values() {
            let mut before: Option<&egui::accesskit::Node> = None;
            for child in parent.children() {
                let Some((id, n)) = by_node.get(child).copied() else { continue };
                if n.role() == Role::Unknown
                    && n.label().is_none()
                    && (n.supports_action(egui::accesskit::Action::Click) || n.supports_action(egui::accesskit::Action::Focus))
                {
                    match before.filter(|b| matches!(b.role(), Role::Link | Role::Button)).and_then(|b| b.label()) {
                        Some(l) if n.children().is_empty() => out.push(Fix::Label(id, l.to_string())),
                        _ => out.push(Fix::Group(id)),
                    }
                }
                before = Some(n);
            }
        }
        let captions: Vec<(String, egui::accesskit::Rect)> = state
            .nodes
            .values()
            .filter_map(|n| {
                let text = match n.role() {
                    Role::Label => n.value(),
                    Role::Slider => n.label(),
                    _ => None,
                };
                text.map(without_glyphs).filter(|t| !t.is_empty()).zip(n.bounds())
            })
            .collect();
        for (id, n) in &state.nodes {
            if !matches!(n.role(), Role::ComboBox | Role::Slider | Role::SpinButton) || n.label().is_some_and(|l| !l.trim().is_empty()) {
                continue;
            }
            let Some(b) = n.bounds() else { continue };
            let left = captions
                .iter()
                .filter(|(_, c)| c.x1 <= b.x0 + 1.0 && b.x0 - c.x1 < 60.0 && c.y0 < b.y1 && b.y0 < c.y1)
                .max_by(|x, y| x.1.x1.total_cmp(&y.1.x1));
            if let Some((text, _)) = left {
                out.push(Fix::Label(*id, text.clone()));
            }
        }
        for (id, n) in &state.nodes {
            let label = n.label().filter(|l| l.chars().any(is_glyph));
            let value = n.value().filter(|v| v.chars().any(is_glyph));
            if let Some(l) = label {
                let words = without_glyphs(l);
                if !words.is_empty() {
                    out.push(Fix::Label(*id, words));
                }
            }
            if let Some(v) = value {
                let words = without_glyphs(v);
                match (words.is_empty(), n.role()) {
                    (true, Role::Label) => out.push(Fix::Hide(*id)),
                    (true, _) => {}
                    (false, _) => out.push(Fix::Value(*id, words)),
                }
            }
        }
        out
    });
    for f in fixes {
        match f {
            Fix::Label(id, l) => ctx.accesskit_node_builder(id, |n| n.set_label(l)),
            Fix::Value(id, v) => ctx.accesskit_node_builder(id, |n| n.set_value(v)),
            Fix::Hide(id) => ctx.accesskit_node_builder(id, |n| n.set_hidden()),
            Fix::Group(id) => ctx.accesskit_node_builder(id, |n| {
                n.set_role(Role::Group);
                n.clear_actions();
            }),
        };
    }
}

/// A 2 px accent ring around the control with keyboard focus (text fields show their own).
fn focus_ring(shell: &Shell, ctx: &Context) {
    if !shell.a11y.keyboard {
        return;
    }
    let Some(id) = ctx.memory(|m| m.focused()) else { return };
    if egui::TextEdit::load_state(ctx, id).is_some() {
        return;
    }
    let Some(r) = ctx.read_response(id) else { return };
    let rect = r.interact_rect;
    if !rect.is_positive() {
        return;
    }
    let t = shell.theme();
    let ring = if Some(id) == shell.a11y.map_id { rect.shrink(3.0) } else { rect.expand(2.0) };
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("lm-focus-ring")));
    painter.rect_stroke(
        ring,
        egui::CornerRadius::same(crate::theme::RADIUS_CONTROL + 2),
        egui::Stroke::new(2.0, t.accent),
        egui::StrokeKind::Outside,
    );
}

#[cfg(test)]
mod tests;
