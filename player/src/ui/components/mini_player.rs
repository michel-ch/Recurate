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
        if ui.button(icon).clicked() {
            app.playback.play_pause();
        }
        if ui.button("⏭").on_hover_text("Next").clicked() {
            app.playback.next();
        }

        ui.separator();
        let mut frac = state.progress();
        let resp = ui.add(
            egui::Slider::new(&mut frac, 0.0..=1.0)
                .show_value(false)
                .clamp_to_range(true),
        );
        if resp.drag_stopped() || resp.lost_focus() {
            app.playback.seek_fraction(frac);
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

fn format_position(current_ms: u64, duration_ms: u64) -> String {
    fn fmt(ms: u64) -> String {
        let s = ms / 1000;
        format!("{}:{:02}", s / 60, s % 60)
    }
    format!("{} / {}", fmt(current_ms), fmt(duration_ms))
}
