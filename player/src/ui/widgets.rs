//! Shared building blocks for every screen, drawn from the design tokens in
//! [`crate::ui::theme`]. Screens compose these instead of styling egui widgets
//! by hand, so colours, radii and type stay consistent and no screen needs a
//! raw colour.

use std::ops::RangeInclusive;

use egui::{
    vec2, Align, Align2, Color32, Frame, InnerResponse, Layout, Margin, Order, Pos2, Rect,
    Response, RichText, Rounding, Sense, Stroke, TextEdit, Ui, Vec2, WidgetText,
};

use crate::ui::theme::{self, Palette};

/// The palette currently installed.
pub fn p(ui: &Ui) -> Palette {
    theme::pal(ui.ctx())
}

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

/// Page title (serif `--h2`) with an optional caption underneath. `right` is
/// laid out right-to-left on the title row (filters, primary actions).
pub fn page_header(ui: &mut Ui, title: &str, caption: Option<&str>, right: impl FnOnce(&mut Ui)) {
    let pal = p(ui);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(title)
                    .font(theme::serif(theme::H2))
                    .color(pal.ink),
            );
            if let Some(c) = caption {
                ui.label(RichText::new(c).size(theme::TEXT_CAPTION).color(pal.ink_3));
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
    ui.add_space(theme::SPACE_3);
}

/// Serif panel title (`--h3`).
pub fn panel_title(ui: &mut Ui, text: &str) -> Response {
    let pal = p(ui);
    ui.label(RichText::new(text).font(theme::serif(theme::H3)).color(pal.ink))
}

/// Uppercase tracked section label (`ADD SONGS`, `LIBRARY PATHS`).
pub fn section_label(ui: &mut Ui, text: &str) -> Response {
    ui.label(section_label_text(ui, text))
}

pub fn section_label_text(ui: &Ui, text: &str) -> RichText {
    RichText::new(text.to_uppercase())
        .font(theme::semibold(theme::TEXT_MICRO))
        .extra_letter_spacing(0.66)
        .color(p(ui).ink_3)
}

/// 12px caption in `--ink-3`.
pub fn caption(ui: &mut Ui, text: impl Into<String>) -> Response {
    let pal = p(ui);
    ui.label(RichText::new(text.into()).size(theme::TEXT_CAPTION).color(pal.ink_3))
}

/// Secondary text (`--ink-2`) at body size.
pub fn secondary(ui: &Ui, text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(p(ui).ink_2)
}

/// Semibold (Inter 600) at `size`.
pub fn strong(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text.into()).font(theme::semibold(size))
}

/// Monospace data text (durations, paths, counters) at `size`.
pub fn mono(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text.into()).font(theme::mono(size))
}

/// Paint `text` into `cell` on one line, ellipsised to the cell width. No
/// widget is created, so the text never steals clicks from the row under it.
/// `align` is `Align::Min` (left), `Center` or `Max` (right).
pub fn paint_truncated(
    ui: &Ui,
    cell: Rect,
    text: &str,
    font: egui::FontId,
    color: Color32,
    align: Align,
) {
    if cell.width() <= 0.0 || !ui.is_rect_visible(cell) {
        return;
    }
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat::simple(font, color),
    );
    job.wrap = egui::text::TextWrapping {
        max_width: cell.width(),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = ui.fonts(|f| f.layout_job(job));
    let size = galley.size();
    let x = match align {
        Align::Min => cell.left(),
        Align::Center => cell.center().x - size.x / 2.0,
        Align::Max => cell.right() - size.x,
    };
    let y = cell.center().y - size.y / 2.0;
    ui.painter().galley(Pos2::new(x, y), galley, color);
}

/// A label squeezed into `width`, ellipsised, never wrapping.
pub fn truncated(ui: &mut Ui, text: impl Into<WidgetText>, width: f32) -> Response {
    let h = ui.spacing().interact_size.y;
    ui.add_sized([width.max(0.0), h], egui::Label::new(text).truncate())
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Primary,
    Secondary,
    Danger,
}

const H_TOOLBAR: f32 = 32.0;
const H_INLINE: f32 = 28.0;

fn styled_button(ui: &mut Ui, enabled: bool, text: &str, kind: Kind, height: f32) -> Response {
    let pal = p(ui);
    ui.scope(|ui| {
        let w = &mut ui.visuals_mut().widgets;
        let (rest, hover, press, ink) = match kind {
            Kind::Primary => (pal.accent_fill, pal.accent_hover, pal.accent_hover, pal.accent_ink),
            Kind::Danger => (pal.critical, pal.critical, pal.critical, pal.accent_ink),
            Kind::Secondary => (Color32::TRANSPARENT, pal.surface, pal.surface_sunk, pal.ink),
        };
        let border = |c: Color32| {
            if kind == Kind::Secondary {
                Stroke::new(1.0, c)
            } else {
                Stroke::NONE
            }
        };
        w.inactive.weak_bg_fill = rest;
        w.inactive.bg_stroke = border(pal.line);
        w.inactive.fg_stroke = Stroke::new(1.0, ink);
        w.hovered.weak_bg_fill = hover;
        w.hovered.bg_stroke = border(pal.line_strong);
        w.hovered.fg_stroke = Stroke::new(1.0, ink);
        w.active.weak_bg_fill = press;
        w.active.bg_stroke = border(pal.accent);
        w.active.fg_stroke = Stroke::new(1.0, ink);
        // Disabled buttons keep their shape; egui fades them.
        w.noninteractive.weak_bg_fill = rest;
        w.noninteractive.bg_fill = rest;
        w.noninteractive.bg_stroke = border(pal.line);
        w.noninteractive.fg_stroke = Stroke::new(1.0, ink);
        let label = if kind == Kind::Secondary {
            RichText::new(text).size(theme::TEXT_BODY)
        } else {
            strong(text, theme::TEXT_BODY)
        };
        ui.add_enabled(
            enabled,
            egui::Button::new(label)
                .min_size(vec2(0.0, height))
                .rounding(Rounding::same(theme::RADIUS_SM)),
        )
    })
    .inner
}

/// Terracotta filled button: the one thing the screen wants you to do.
pub fn primary_button(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    styled_button(ui, enabled, text, Kind::Primary, H_TOOLBAR)
}

pub fn primary_button_sm(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    styled_button(ui, enabled, text, Kind::Primary, H_INLINE)
}

/// Transparent button with a hairline border.
pub fn secondary_button(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    styled_button(ui, enabled, text, Kind::Secondary, H_TOOLBAR)
}

pub fn secondary_button_sm(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    styled_button(ui, enabled, text, Kind::Secondary, H_INLINE)
}

/// Brick-filled button. Only for the *armed* state of a two-click destructive
/// action (`⚠ Confirm delete N files?`).
pub fn danger_button(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    styled_button(ui, enabled, text, Kind::Danger, H_TOOLBAR)
}

/// Accent text link, underlined on hover.
pub fn text_link(ui: &mut Ui, text: &str) -> Response {
    let pal = p(ui);
    ui.add(egui::Link::new(RichText::new(text).color(pal.accent)))
}

/// Borderless square glyph button (transport icons, row `✕`). `color` is the
/// glyph colour at rest.
pub fn icon_button(ui: &mut Ui, glyph: &str, size: f32, color: Color32) -> Response {
    let pal = p(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    if ui.is_rect_visible(rect) {
        if resp.hovered() {
            ui.painter()
                .rect_filled(rect, Rounding::same(theme::RADIUS_SM), pal.surface_sunk);
        }
        let c = if resp.hovered() { pal.ink } else { color };
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            glyph,
            theme::sans((size * 0.55).max(12.0)),
            c,
        );
    }
    resp
}

/// Small hairline button for dense rows (`▲ ▼ ⇱ ⇲`), 24px tall.
pub fn mini_button(ui: &mut Ui, enabled: bool, glyph: &str) -> Response {
    let pal = p(ui);
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = vec2(theme::SPACE_1 + 2.0, 0.0);
        let w = &mut ui.visuals_mut().widgets;
        w.inactive.bg_stroke = Stroke::new(1.0, pal.line);
        w.noninteractive.bg_stroke = Stroke::new(1.0, pal.line);
        ui.add_enabled(
            enabled,
            egui::Button::new(RichText::new(glyph).size(theme::TEXT_MICRO).color(pal.ink_2))
                .min_size(vec2(24.0, 24.0))
                .rounding(Rounding::same(theme::RADIUS_SM)),
        )
    })
    .inner
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

fn input_frame(ui: &Ui, focused: bool) -> Frame {
    let pal = p(ui);
    Frame::none()
        .fill(pal.surface_sunk)
        .stroke(Stroke::new(1.0, if focused { pal.accent } else { pal.line_strong }))
        .rounding(Rounding::same(theme::RADIUS_SM))
        .inner_margin(Margin::symmetric(theme::SPACE_2, 6.0))
}

fn framed_edit(
    ui: &mut Ui,
    value: &mut String,
    hint: &str,
    width: f32,
    prefix: Option<&str>,
    monospace: bool,
    password: bool,
) -> Response {
    let pal = p(ui);
    let id = ui.make_persistent_id(("recurate_input", hint));
    let focused = ui.memory(|m| m.has_focus(id));
    input_frame(ui, focused)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme::SPACE_1 + 2.0;
                let mut inner_w = width - 2.0 * theme::SPACE_2;
                if let Some(glyph) = prefix {
                    let r = ui.label(RichText::new(glyph).color(pal.ink_3));
                    inner_w -= r.rect.width() + ui.spacing().item_spacing.x;
                }
                let mut edit = TextEdit::singleline(value)
                    .id(id)
                    .frame(false)
                    .hint_text(RichText::new(hint).color(pal.ink_3))
                    .desired_width(inner_w.max(40.0))
                    .password(password)
                    .margin(Margin::ZERO);
                if monospace {
                    edit = edit.font(theme::mono(theme::TEXT_CAPTION));
                }
                ui.add(edit)
            })
            .inner
        })
        .inner
}

/// Single-line text input on `--surface-sunk` with a hairline border.
pub fn text_input(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    framed_edit(ui, value, hint, width, None, false, false)
}

/// Same as [`text_input`] in JetBrains Mono (URLs, paths).
pub fn mono_input(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    framed_edit(ui, value, hint, width, None, true, false)
}

/// Masked input (API keys).
pub fn password_input(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    framed_edit(ui, value, hint, width, None, true, true)
}

/// Filter input with a leading `⌕`.
pub fn search_input(ui: &mut Ui, value: &mut String, hint: &str, width: f32) -> Response {
    framed_edit(ui, value, hint, width, Some("⌕"), false, false)
}

/// Multi-line monospace text area (paste boxes).
pub fn text_area(ui: &mut Ui, value: &mut String, hint: &str, rows: usize) -> Response {
    let pal = p(ui);
    let id = ui.make_persistent_id(("recurate_area", hint));
    let focused = ui.memory(|m| m.has_focus(id));
    input_frame(ui, focused)
        .show(ui, |ui| {
            ui.add(
                TextEdit::multiline(value)
                    .id(id)
                    .frame(false)
                    .font(theme::mono(theme::TEXT_CAPTION))
                    .hint_text(RichText::new(hint).color(pal.ink_3))
                    .desired_rows(rows)
                    .desired_width(f32::INFINITY)
                    .margin(Margin::ZERO),
            )
        })
        .inner
}

/// A `ComboBox` styled like an input (`--surface-sunk`, hairline border).
pub fn dropdown<R>(
    ui: &mut Ui,
    id_salt: &str,
    selected_text: impl Into<WidgetText>,
    width: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>> {
    let pal = p(ui);
    ui.scope(|ui| {
        let w = &mut ui.visuals_mut().widgets;
        w.inactive.weak_bg_fill = pal.surface_sunk;
        w.hovered.weak_bg_fill = pal.surface_sunk;
        w.active.weak_bg_fill = pal.surface_sunk;
        w.open.weak_bg_fill = pal.surface_sunk;
        ui.spacing_mut().interact_size.y = H_TOOLBAR;
        egui::ComboBox::from_id_source(id_salt)
            .selected_text(selected_text)
            .width(width)
            .show_ui(ui, add_contents)
    })
    .inner
}

// ---------------------------------------------------------------------------
// Surfaces
// ---------------------------------------------------------------------------

/// Card: `--surface`, hairline `--line`, `--radius`, generous padding.
pub fn card_frame(ui: &Ui) -> Frame {
    let pal = p(ui);
    Frame::none()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.line))
        .rounding(Rounding::same(theme::RADIUS))
        .inner_margin(Margin::same(theme::SPACE_4))
}

pub fn card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    card_frame(ui).show(ui, add_contents)
}

/// Flat cover-art placeholder: `--surface-sunk` with a hairline, or the
/// accent wash for the playing row.
pub fn art_tile(ui: &mut Ui, size: f32, tinted: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    paint_art_tile(ui, rect, tinted);
    resp
}

pub fn paint_art_tile(ui: &Ui, rect: Rect, tinted: bool) {
    let pal = p(ui);
    let radius = if rect.width() >= 96.0 {
        theme::RADIUS_LG
    } else {
        theme::RADIUS_SM
    };
    let (fill, stroke) = if tinted {
        (pal.accent_soft, pal.accent_soft_ink.gamma_multiply(0.35))
    } else {
        (pal.surface_sunk, pal.line)
    };
    ui.painter()
        .rect(rect, Rounding::same(radius), fill, Stroke::new(1.0, stroke));
    if rect.width() >= 40.0 {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "♫",
            theme::sans(rect.width() * 0.3),
            if tinted { pal.accent_soft_ink } else { pal.ink_3 },
        );
    }
}

// ---------------------------------------------------------------------------
// Pills, counters, stats
// ---------------------------------------------------------------------------

/// Rounded pill with explicit colours.
pub fn pill(ui: &mut Ui, text: impl Into<String>, fill: Color32, ink: Color32) -> Response {
    Frame::none()
        .fill(fill)
        .rounding(Rounding::same(999.0))
        .inner_margin(Margin::symmetric(theme::SPACE_2, 2.0))
        .show(ui, |ui| {
            ui.label(RichText::new(text.into()).size(theme::TEXT_MICRO).color(ink))
        })
        .inner
}

/// Tag / chip on the accent wash.
pub fn chip(ui: &mut Ui, text: impl Into<String>) -> Response {
    let pal = p(ui);
    pill(ui, text, pal.accent_soft, pal.accent_soft_ink)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Ok,
    Warn,
    Bad,
}

impl Status {
    pub fn color(self, pal: &Palette) -> Color32 {
        match self {
            Status::Ok => pal.positive,
            Status::Warn => pal.caution,
            Status::Bad => pal.critical,
        }
    }
}

/// `● yt-dlp ● ffmpeg ● API key` — each dot coloured by status and always
/// followed by its word, so state never rests on colour alone. `hover` is the
/// tooltip for the whole pill.
pub fn tool_pill(ui: &mut Ui, items: &[(&str, Status)], hover: &str) -> Response {
    let pal = p(ui);
    Frame::none()
        .fill(pal.accent_soft)
        .rounding(Rounding::same(999.0))
        .inner_margin(Margin::symmetric(theme::SPACE_3, 4.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme::SPACE_1;
                for (i, (name, status)) in items.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(theme::SPACE_2);
                    }
                    ui.label(mono("●", theme::TEXT_MICRO).color(status.color(&pal)));
                    let word = match status {
                        Status::Ok => name.to_string(),
                        Status::Warn => format!("{name} ?"),
                        Status::Bad => format!("{name} missing"),
                    };
                    ui.label(mono(word, theme::TEXT_MICRO).color(pal.accent_soft_ink));
                }
            })
        })
        .response
        .on_hover_text(hover)
}

/// `● N pending · ● N done · ● N failed` in mono 12.
pub fn batch_counters(ui: &mut Ui, pending: usize, done: usize, failed: usize) {
    let pal = p(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::SPACE_3;
        for (n, word, color) in [
            (pending, "pending", pal.accent),
            (done, "done", pal.positive),
            (failed, "failed", pal.critical),
        ] {
            ui.label(mono(format!("● {n} {word}"), theme::TEXT_CAPTION).color(color));
        }
    });
}

/// `⏸ Pause` / `▶ Resume` + `(N queued)`. Returns true when toggled.
pub fn worker_controls(ui: &mut Ui, paused: bool, queued: usize) -> bool {
    let pal = p(ui);
    let mut toggled = false;
    ui.horizontal(|ui| {
        let resp = if paused {
            ui.scope(|ui| {
                let w = &mut ui.visuals_mut().widgets;
                w.inactive.bg_stroke = Stroke::new(1.0, pal.accent);
                w.inactive.fg_stroke = Stroke::new(1.0, pal.accent);
                w.hovered.fg_stroke = Stroke::new(1.0, pal.accent);
                ui.add(
                    egui::Button::new(RichText::new("▶ Resume").size(theme::TEXT_CAPTION))
                        .min_size(vec2(0.0, H_INLINE)),
                )
            })
            .inner
        } else {
            secondary_button_sm(ui, true, "⏸ Pause")
        };
        if resp.clicked() {
            toggled = true;
        }
        ui.label(
            RichText::new(format!("({queued} queued)"))
                .size(theme::TEXT_CAPTION)
                .color(pal.ink_3),
        );
    });
    toggled
}

/// Big mono number over a small label.
pub fn stat(ui: &mut Ui, value: impl ToString, label: &str, color: Color32) {
    let pal = p(ui);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(mono(value.to_string(), 20.0).color(color));
        ui.label(RichText::new(label).size(theme::TEXT_CAPTION).color(pal.ink_3));
    });
}

/// Horizontal bar split into coloured segments proportional to their counts.
pub fn stacked_bar(ui: &mut Ui, width: f32, segments: &[(usize, Color32)]) -> Response {
    let pal = p(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 4.0), Sense::hover());
    let r = Rounding::same(2.0);
    ui.painter().rect_filled(rect, r, pal.surface_sunk);
    let total: usize = segments.iter().map(|(n, _)| *n).sum();
    if total > 0 {
        let mut x = rect.left();
        for (n, color) in segments {
            if *n == 0 {
                continue;
            }
            let w = rect.width() * (*n as f32 / total as f32);
            let seg = Rect::from_min_size(Pos2::new(x, rect.top()), vec2(w, rect.height()));
            ui.painter().rect_filled(seg, Rounding::ZERO, *color);
            x += w;
        }
    }
    resp
}

/// 4px progress bar (`--surface-sunk` track, `--accent` fill).
pub fn progress_bar(ui: &mut Ui, width: f32, frac: f32) -> Response {
    let pal = p(ui);
    stacked_bar(
        ui,
        width,
        &[
            ((frac.clamp(0.0, 1.0) * 1000.0) as usize, pal.accent),
            (((1.0 - frac.clamp(0.0, 1.0)) * 1000.0) as usize, pal.surface_sunk),
        ],
    )
}

// ---------------------------------------------------------------------------
// Sliders and toggles
// ---------------------------------------------------------------------------

/// Thin horizontal slider: 4px track, accent fill, 12px knob shown on hover.
/// The value only changes on drag or on a completed click, so callers can
/// treat `changed() && !dragged()` as a click-to-seek commit.
pub fn slider(ui: &mut Ui, value: &mut f32, range: RangeInclusive<f32>, width: f32) -> Response {
    slider_ex(ui, value, range, width, 4.0, 12.0, false)
}

pub fn slider_ex(
    ui: &mut Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    width: f32,
    track: f32,
    knob: f32,
    always_show_knob: bool,
) -> Response {
    let pal = p(ui);
    let (lo, hi) = (*range.start(), *range.end());
    let (rect, mut resp) =
        ui.allocate_exact_size(vec2(width, knob.max(16.0)), Sense::click_and_drag());
    let x0 = rect.left() + knob / 2.0;
    let x1 = rect.right() - knob / 2.0;
    if resp.dragged() || resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = ((pos.x - x0) / (x1 - x0).max(1.0)).clamp(0.0, 1.0);
            let nv = lo + t * (hi - lo);
            if (nv - *value).abs() > f32::EPSILON {
                *value = nv;
                resp.mark_changed();
            }
        }
    }
    if ui.is_rect_visible(rect) {
        let t = if hi > lo {
            ((*value - lo) / (hi - lo)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let cy = rect.center().y;
        let r = Rounding::same(track / 2.0);
        let rail = Rect::from_min_max(Pos2::new(x0, cy - track / 2.0), Pos2::new(x1, cy + track / 2.0));
        ui.painter().rect_filled(rail, r, pal.surface_sunk);
        let kx = x0 + t * (x1 - x0);
        let filled = Rect::from_min_max(rail.min, Pos2::new(kx, rail.max.y));
        ui.painter().rect_filled(filled, r, pal.accent);
        if always_show_knob || resp.hovered() || resp.dragged() {
            ui.painter()
                .circle_filled(Pos2::new(kx, cy), knob / 2.0, pal.accent);
        }
    }
    resp
}

/// Vertical slider filled from the centre of `range` (EQ bands).
pub fn vslider_centered(
    ui: &mut Ui,
    value: &mut f32,
    range: RangeInclusive<f32>,
    height: f32,
) -> Response {
    let pal = p(ui);
    let (lo, hi) = (*range.start(), *range.end());
    let knob = 12.0;
    let (rect, mut resp) = ui.allocate_exact_size(vec2(24.0, height), Sense::click_and_drag());
    let y_top = rect.top() + knob / 2.0;
    let y_bot = rect.bottom() - knob / 2.0;
    if resp.dragged() || resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = ((y_bot - pos.y) / (y_bot - y_top).max(1.0)).clamp(0.0, 1.0);
            let nv = lo + t * (hi - lo);
            if (nv - *value).abs() > f32::EPSILON {
                *value = nv;
                resp.mark_changed();
            }
        }
    }
    if ui.is_rect_visible(rect) {
        let cx = rect.center().x;
        let to_y = |v: f32| y_bot - ((v - lo) / (hi - lo)).clamp(0.0, 1.0) * (y_bot - y_top);
        let rail = Rect::from_min_max(Pos2::new(cx - 2.0, y_top), Pos2::new(cx + 2.0, y_bot));
        ui.painter().rect_filled(rail, Rounding::same(2.0), pal.surface_sunk);
        let mid = to_y((lo + hi) / 2.0);
        let vy = to_y(*value);
        let fill = Rect::from_min_max(
            Pos2::new(cx - 2.0, mid.min(vy)),
            Pos2::new(cx + 2.0, mid.max(vy)),
        );
        ui.painter().rect_filled(fill, Rounding::same(2.0), pal.accent);
        ui.painter().circle_filled(Pos2::new(cx, vy), knob / 2.0, pal.accent);
    }
    resp
}

/// 32×16 pill switch, `--accent-fill` when on.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let pal = p(ui);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(32.0, 16.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(resp.id, *on);
        let fill = if *on { pal.accent_fill } else { pal.surface_sunk };
        let stroke = if *on { pal.accent_fill } else { pal.line_strong };
        ui.painter()
            .rect(rect, Rounding::same(8.0), fill, Stroke::new(1.0, stroke));
        let x = egui::lerp((rect.left() + 8.0)..=(rect.right() - 8.0), t);
        let knob = if *on { pal.accent_ink } else { pal.ink_3 };
        ui.painter()
            .circle_filled(Pos2::new(x, rect.center().y), 6.0, knob);
    }
    resp
}

/// Toggle followed by its label; clicking either flips it.
pub fn toggle_labeled(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    ui.horizontal(|ui| {
        let mut resp = toggle(ui, on);
        let text = ui.add(egui::Label::new(label).sense(Sense::click()));
        if text.clicked() {
            *on = !*on;
            resp.mark_changed();
        }
        resp
    })
    .inner
}

// ---------------------------------------------------------------------------
// Modal
// ---------------------------------------------------------------------------

/// Centered modal over a scrim that swallows clicks to the page behind it.
/// Title in serif `--h3`, then `add_contents`. Returns its result.
pub fn modal<R>(
    ctx: &egui::Context,
    id: &str,
    title: &str,
    width: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let pal = theme::pal(ctx);
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new((id, "scrim")))
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .interactable(true)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            ui.painter().rect_filled(rect, Rounding::ZERO, pal.scrim);
        });
    egui::Area::new(egui::Id::new((id, "modal")))
        .order(Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            Frame::none()
                .fill(pal.surface)
                .stroke(Stroke::new(1.0, pal.line_strong))
                .rounding(Rounding::same(theme::RADIUS_LG))
                .inner_margin(Margin::same(theme::SPACE_5))
                .shadow(egui::Shadow {
                    offset: vec2(0.0, 1.0),
                    blur: 2.0,
                    spread: 0.0,
                    color: pal.shadow,
                })
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.label(
                        RichText::new(title)
                            .font(theme::serif(theme::H3))
                            .color(pal.ink),
                    );
                    ui.add_space(theme::SPACE_2);
                    add_contents(ui)
                })
                .inner
        })
        .inner
}
