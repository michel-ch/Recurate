//! The one song row used by Songs, Search, Albums, Artists, Folders and Queue.
//!
//! Grid: `# (32) · art (32) · title (1fr) · time (44) · artist (≤200) · [✕ 28]`,
//! 40px tall. The whole row is allocated as one click rect *before* anything
//! is drawn (constraint 14): that pins the row to the parent's width so the
//! ScrollArea can't widen frame by frame, and makes anywhere-on-row a play
//! click. Text is painted, not placed as widgets, so nothing on the row steals
//! the click except the optional `✕`.

use egui::{vec2, Align, Rect, Rounding, Sense, Stroke, Ui};

use crate::domain::Song;
use crate::ui::theme;
use crate::ui::widgets;

pub const ROW_H: f32 = 40.0;

pub struct RowAction {
    pub clicked: bool,
    pub remove_clicked: bool,
}

#[derive(Clone, Copy, Default)]
pub struct RowOptions {
    pub show_remove: bool,
    pub remove_hover: Option<&'static str>,
    /// Already-played queue rows: text in `--ink-2`, art faded.
    pub dim: bool,
}

pub fn draw(ui: &mut Ui, index: usize, song: &Song, is_current: bool) -> RowAction {
    draw_with_options(ui, index, song, is_current, RowOptions::default())
}

/// Column rectangles for a row (or the header) occupying `rect`.
struct Cells {
    num: Rect,
    art: Rect,
    title: Rect,
    time: Rect,
    artist: Rect,
    remove: Option<Rect>,
}

fn cells(rect: Rect, show_remove: bool) -> Cells {
    let gap = theme::SPACE_2;
    let inner = rect.shrink2(vec2(theme::SPACE_2, 0.0));
    let col = |x: f32, w: f32| Rect::from_min_size(egui::pos2(x, rect.top()), vec2(w.max(0.0), rect.height()));
    let num = col(inner.left(), 32.0);
    let art = col(num.right() + gap, 32.0);
    let remove_w = if show_remove { 28.0 + gap } else { 0.0 };
    // Trailing cluster (time + artist + ✕) never takes more than 60% of the row.
    let rest = inner.right() - (art.right() + gap);
    let artist_w = 200.0_f32.min(rest * 0.6 - 44.0 - gap - remove_w).max(60.0);
    let remove = show_remove.then(|| col(inner.right() - 28.0, 28.0));
    let artist_right = inner.right() - remove_w;
    let artist = col(artist_right - artist_w, artist_w);
    let time = col(artist.left() - gap - 44.0, 44.0);
    let title = col(art.right() + gap, time.left() - gap - (art.right() + gap));
    Cells { num, art, title, time, artist, remove }
}

/// `#  TITLE  TIME  ARTIST` column header with a hairline underneath.
pub fn header(ui: &mut Ui, show_remove: bool) {
    let pal = widgets::p(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let c = cells(rect, show_remove);
    let font = theme::semibold(theme::TEXT_MICRO);
    for (cell, text, align) in [
        (c.num, "#", Align::Max),
        (c.title, "TITLE", Align::Min),
        (c.time, "TIME", Align::Max),
        (c.artist, "ARTIST", Align::Min),
    ] {
        widgets::paint_truncated(ui, cell, text, font.clone(), pal.ink_3, align);
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, pal.line),
    );
}

pub fn draw_with_options(
    ui: &mut Ui,
    index: usize,
    song: &Song,
    is_current: bool,
    opts: RowOptions,
) -> RowAction {
    let pal = widgets::p(ui);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let c = cells(rect, opts.show_remove);
    let hovered = response.hovered();

    if ui.is_rect_visible(rect) {
        let bg = if is_current {
            Some(pal.accent_soft)
        } else if hovered {
            Some(pal.surface)
        } else {
            None
        };
        if let Some(fill) = bg {
            ui.painter()
                .rect_filled(rect, Rounding::same(theme::RADIUS_SM), fill);
        }

        let (title_c, meta_c) = if is_current {
            (pal.accent_soft_ink, pal.accent_soft_ink)
        } else if opts.dim {
            (pal.ink_2, pal.ink_3)
        } else {
            (pal.ink, pal.ink_2)
        };

        // `#` column shows ▶ when playing or hovered.
        if is_current || hovered {
            let color = if is_current { pal.accent } else { pal.ink_2 };
            widgets::paint_truncated(ui, c.num, "▶", theme::sans(theme::TEXT_CAPTION), color, Align::Max);
        } else {
            widgets::paint_truncated(
                ui,
                c.num,
                &(index + 1).to_string(),
                theme::mono(theme::TEXT_CAPTION),
                pal.ink_3,
                Align::Max,
            );
        }

        let art = Rect::from_center_size(c.art.center(), vec2(28.0, 28.0));
        widgets::paint_art_tile(ui, art, is_current);
        if opts.dim && !is_current {
            ui.painter()
                .rect_filled(art, Rounding::same(theme::RADIUS_SM), pal.paper.gamma_multiply(0.4));
        }

        let title_font = if is_current {
            theme::semibold(theme::TEXT_BODY)
        } else {
            theme::sans(theme::TEXT_BODY)
        };
        widgets::paint_truncated(ui, c.title, &song.title, title_font, title_c, Align::Min);
        widgets::paint_truncated(
            ui,
            c.time,
            &song.formatted_duration(),
            theme::mono(theme::TEXT_CAPTION),
            meta_c,
            Align::Max,
        );
        widgets::paint_truncated(
            ui,
            c.artist,
            &song.artist,
            theme::sans(theme::TEXT_BODY),
            meta_c,
            Align::Min,
        );
    }

    let mut remove_clicked = false;
    if let Some(cell) = c.remove {
        let mut child = ui.child_ui(cell, egui::Layout::left_to_right(Align::Center), None);
        let btn = widgets::icon_button(&mut child, "✕", 24.0, pal.ink_3);
        let btn = match opts.remove_hover {
            Some(text) => btn.on_hover_text(text),
            None => btn,
        };
        remove_clicked = btn.clicked();
    }

    RowAction {
        clicked: response.clicked() && !remove_clicked,
        remove_clicked,
    }
}
