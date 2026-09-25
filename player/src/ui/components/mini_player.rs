//! Persistent bottom player (72px): now-playing identity on the left,
//! transport and seek in the centre, volume and "Now Playing" on the right.

use egui::{vec2, Align, Align2, Layout, Rect, Sense, Ui};

use crate::domain::{PlaybackState, RepeatMode, Screen};
use crate::ui::theme;
use crate::ui::widgets;
use crate::ui::App;

const LEFT_W: f32 = 260.0;
const RIGHT_W: f32 = 220.0;

pub fn draw(ui: &mut Ui, app: &mut App) {
    let pal = widgets::p(ui);
    let state = app.playback.state_snapshot();
    let full = ui.available_rect_before_wrap();
    let gap = theme::SPACE_4;

    let left = Rect::from_min_size(full.min, vec2(LEFT_W, full.height()));
    let right = Rect::from_min_max(egui::pos2(full.right() - RIGHT_W, full.top()), full.max);
    let centre = Rect::from_min_max(
        egui::pos2(left.right() + gap, full.top()),
        egui::pos2(right.left() - gap, full.bottom()),
    );

    // ---- left: art + title ------------------------------------------------
    let mut l = ui.child_ui(left, Layout::left_to_right(Align::Center), None);
    match &state.current_song {
        Some(song) => {
            widgets::art_tile(&mut l, 44.0, false);
            let text_rect = Rect::from_min_max(
                egui::pos2(left.left() + 44.0 + theme::SPACE_3, left.top()),
                left.max,
            );
            let mid = text_rect.center().y;
            let folder = song
                .folder()
                .and_then(|f| f.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let sub = if folder.is_empty() {
                song.artist.clone()
            } else {
                format!("{} · {}", song.artist, folder)
            };
            widgets::paint_truncated(
                &l,
                Rect::from_min_max(egui::pos2(text_rect.left(), mid - 18.0), egui::pos2(text_rect.right(), mid)),
                &song.title,
                theme::semibold(theme::TEXT_BODY),
                pal.ink,
                Align::Min,
            );
            widgets::paint_truncated(
                &l,
                Rect::from_min_max(egui::pos2(text_rect.left(), mid), egui::pos2(text_rect.right(), mid + 18.0)),
                &sub,
                theme::sans(theme::TEXT_CAPTION),
                pal.ink_2,
                Align::Min,
            );
        }
        None => {
            l.label(egui::RichText::new("Nothing playing").color(pal.ink_3));
        }
    }

    // ---- centre: transport + seek ----------------------------------------
    let mut c = ui.child_ui(centre, Layout::top_down(Align::Center), None);
    c.add_space(theme::SPACE_2);
    c.horizontal(|ui| {
        // Centre the transport cluster: 5 controls, 16px gaps.
        let cluster = 24.0 * 4.0 + 32.0 + theme::SPACE_4 * 4.0;
        ui.add_space(((centre.width() - cluster) / 2.0).max(0.0));
        ui.spacing_mut().item_spacing.x = theme::SPACE_4;
        let shuffle_c = if state.shuffle_enabled { pal.accent } else { pal.ink_3 };
        if widgets::icon_button(ui, "⇄", 24.0, shuffle_c)
            .on_hover_text(if state.shuffle_enabled { "Shuffle on" } else { "Shuffle off" })
            .clicked()
        {
            app.playback.set_shuffle(!state.shuffle_enabled);
        }
        if widgets::icon_button(ui, "⏮", 24.0, pal.ink_2).on_hover_text("Previous").clicked() {
            app.playback.previous();
        }
        if play_circle(ui, 32.0, state.is_playing).clicked() {
            app.playback.play_pause();
        }
        if widgets::icon_button(ui, "⏭", 24.0, pal.ink_2).on_hover_text("Next").clicked() {
            app.playback.next();
        }
        let (glyph, color, hover) = repeat_look(state.repeat_mode, &pal);
        if widgets::icon_button(ui, glyph, 24.0, color).on_hover_text(hover).clicked() {
            app.playback.cycle_repeat();
        }
    });
    c.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::SPACE_2;
        let time_w = 44.0;
        let bar_w = (centre.width() - 2.0 * time_w - 2.0 * theme::SPACE_2).max(40.0);
        time_label(ui, state.current_position_ms, time_w, Align::Max);
        if let Some(target) = draw_seek_slider(ui, &state, bar_w, false) {
            app.playback.seek_fraction(target);
        }
        time_label(ui, state.duration_ms, time_w, Align::Min);
    });

    // ---- right: volume + Now Playing -------------------------------------
    let mut r = ui.child_ui(right, Layout::right_to_left(Align::Center), None);
    if widgets::secondary_button_sm(&mut r, true, "Now Playing ↗").clicked() {
        app.navigate(Screen::NowPlaying);
    }
    r.add_space(theme::SPACE_2);
    if widgets::slider(&mut r, &mut app.volume, 0.0..=1.0, 96.0)
        .on_hover_text(format!("Volume {:.0}%", app.volume * 100.0))
        .changed()
    {
        app.playback.set_volume(app.volume);
        app.settings.write().playback.volume = app.volume;
    }
    r.label(egui::RichText::new("🔈").color(pal.ink_2));

    ui.allocate_rect(full, Sense::hover());
}

/// Accent-filled round play/pause button.
pub fn play_circle(ui: &mut Ui, size: f32, playing: bool) -> egui::Response {
    let pal = widgets::p(ui);
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if resp.hovered() { pal.accent_hover } else { pal.accent_fill };
        ui.painter().circle_filled(rect.center(), size / 2.0, fill);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            if playing { "⏸" } else { "▶" },
            theme::sans(size * 0.45),
            pal.accent_ink,
        );
    }
    resp.on_hover_text(if playing { "Pause" } else { "Play" })
}

/// Repeat glyph, colour and tooltip for each mode.
pub fn repeat_look(mode: RepeatMode, pal: &theme::Palette) -> (&'static str, egui::Color32, &'static str) {
    match mode {
        RepeatMode::Off => ("↻", pal.ink_3, "Repeat off"),
        RepeatMode::All => ("↻", pal.accent, "Repeat all"),
        RepeatMode::One => ("↻¹", pal.accent, "Repeat one"),
    }
}

fn time_label(ui: &mut Ui, ms: u64, width: f32, align: Align) {
    let pal = widgets::p(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 16.0), Sense::hover());
    widgets::paint_truncated(ui, rect, &format_ms(ms), theme::mono(theme::TEXT_MICRO), pal.ink_3, align);
}

pub fn format_ms(ms: u64) -> String {
    let s = ms / 1000;
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

pub fn format_position(current_ms: u64, duration_ms: u64) -> String {
    format!("{} / {}", format_ms(current_ms), format_ms(duration_ms))
}

/// Seek bar. Returns `Some(target_fraction)` once the user finishes a drag or
/// clicks the track. The in-flight drag value is stashed in egui memory so
/// the released value survives the per-frame reset to the engine's progress
/// (otherwise the release frame would seek back to where playback already
/// was). `large` = Now Playing sizing (6px track, 16px knob always visible).
pub fn draw_seek_slider(
    ui: &mut Ui,
    state: &PlaybackState,
    width: f32,
    large: bool,
) -> Option<f32> {
    let mem_id = egui::Id::new("seek_slider_pending");
    let pending: Option<f32> = ui.memory(|m| m.data.get_temp(mem_id));
    let mut frac = pending.unwrap_or_else(|| state.progress());
    let resp = if large {
        widgets::slider_ex(ui, &mut frac, 0.0..=1.0, width, 6.0, 16.0, true)
    } else {
        widgets::slider(ui, &mut frac, 0.0..=1.0, width)
    };
    if resp.dragged() {
        ui.memory_mut(|m| m.data.insert_temp(mem_id, frac));
    }
    let commit = if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
        Some(frac)
    } else {
        None
    };
    if commit.is_some() {
        ui.memory_mut(|m| m.data.remove::<f32>(mem_id));
    }
    commit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_minutes_and_hours() {
        assert_eq!(format_ms(0), "0:00");
        assert_eq!(format_ms(83_000), "1:23");
        assert_eq!(format_ms(3_725_000), "1:02:05");
        assert_eq!(format_position(83_000, 225_000), "1:23 / 3:45");
    }
}
