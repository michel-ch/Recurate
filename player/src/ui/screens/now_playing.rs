use crate::domain::Screen;
use crate::ui::App;

pub fn draw(ui: &mut egui::Ui, app: &mut App) {
    ui.horizontal(|ui| {
        if ui.button("← Back").clicked() {
            app.navigate(Screen::AllSongs);
        }
        ui.heading("Now Playing");
    });
    ui.separator();

    let state = app.playback.state_snapshot();
    let song = match state.current_song.as_ref() {
        Some(s) => s,
        None => {
            ui.centered_and_justified(|ui| {
                ui.label("No song playing");
            });
            return;
        }
    };

    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.label(egui::RichText::new(&song.title).heading());
        ui.label(egui::RichText::new(&song.artist).size(16.0));
        ui.label(egui::RichText::new(&song.album).weak());
        ui.add_space(20.0);

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

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("⏮ Prev").clicked() {
                        app.playback.previous();
                    }
                    let icon = if state.is_playing { "⏸ Pause" } else { "▶ Play" };
                    if ui.button(icon).clicked() {
                        app.playback.play_pause();
                    }
                    if ui.button("Next ⏭").clicked() {
                        app.playback.next();
                    }
                    ui.separator();
                    if ui.button("Shuffle").clicked() {
                        let s = !state.shuffle_enabled;
                        app.playback.set_shuffle(s);
                    }
                    if ui.button(format!("Repeat: {:?}", state.repeat_mode)).clicked() {
                        app.playback.cycle_repeat();
                    }
                });
            });
        });
    });
}

fn format_position(current_ms: u64, duration_ms: u64) -> String {
    fn fmt(ms: u64) -> String {
        let s = ms / 1000;
        format!("{}:{:02}", s / 60, s % 60)
    }
    format!("{} / {}", fmt(current_ms), fmt(duration_ms))
}
