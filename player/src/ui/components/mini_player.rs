use crate::domain::Screen;
use crate::ui::App;

pub fn draw(ui: &mut egui::Ui, app: &mut App) {
    let state = app.playback.state_snapshot();
    ui.horizontal(|ui| {
        if let Some(song) = &state.current_song {
            ui.vertical(|ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(&song.title).strong()).truncate(),
                );
                ui.add(egui::Label::new(&song.artist).truncate());
            });
        } else {
            ui.label("Nothing playing");
        }

        ui.separator();

        if ui.button("⏮").on_hover_text("Previous").clicked() {
            app.playback.previous();
        }
        let icon = if state.is_playing { "⏸" } else { "▶" };
        let hover = if state.is_playing { "Pause" } else { "Play" };
        if ui.button(icon).on_hover_text(hover).clicked() {
            app.playback.play_pause();
        }
        if ui.button("⏭").on_hover_text("Next").clicked() {
            app.playback.next();
        }

        ui.separator();
        let frac_resp = draw_seek_slider(ui, &state);
        if let Some(target) = frac_resp {
            app.playback.seek_fraction(target);
        }
        ui.label(format_position(state.current_position_ms, state.duration_ms));

        ui.separator();
        if ui.add(egui::Slider::new(&mut app.volume, 0.0..=1.0).text("Vol")).changed() {
            app.playback.set_volume(app.volume);
            app.settings.write().playback.volume = app.volume;
        }

        ui.separator();
        if ui.button("Now Playing").clicked() {
            app.navigate(Screen::NowPlaying);
        }
    });
}

pub fn format_position(current_ms: u64, duration_ms: u64) -> String {
    fn fmt(ms: u64) -> String {
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
    format!("{} / {}", fmt(current_ms), fmt(duration_ms))
}

/// Returns Some(target_fraction) if the user finished interacting and a seek
/// should fire. Stashes the in-progress drag value in egui memory so the
/// final value survives the per-frame `frac = state.progress()` reset (the
/// drag_stopped frame would otherwise see the OLD progress and seek to it).
pub fn draw_seek_slider(ui: &mut egui::Ui, state: &crate::domain::PlaybackState) -> Option<f32> {
    let mem_id = egui::Id::new("seek_slider_pending");
    let pending: Option<f32> = ui.memory(|m| m.data.get_temp(mem_id));
    let mut frac = pending.unwrap_or_else(|| state.progress());
    let resp = ui.add(
        egui::Slider::new(&mut frac, 0.0..=1.0)
            .show_value(false)
            .clamp_to_range(true),
    );
    let mut commit: Option<f32> = None;
    if resp.dragged() || resp.changed() {
        ui.memory_mut(|m| m.data.insert_temp(mem_id, frac));
    }
    if resp.drag_stopped() || resp.lost_focus() {
        commit = Some(frac);
    } else if resp.changed() && !resp.dragged() {
        // Click on the track without a sustained drag.
        commit = Some(frac);
    }
    if commit.is_some() {
        ui.memory_mut(|m| m.data.remove::<f32>(mem_id));
    }
    commit
}
