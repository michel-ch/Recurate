use crate::domain::Screen;
use crate::ui::App;

pub fn draw(ui: &mut egui::Ui, app: &mut App) {
    ui.horizontal(|ui| {
        ui.heading("Recurate");
        ui.separator();
        nav_button(ui, app, "Songs", Screen::AllSongs);
        nav_button(ui, app, "Albums", Screen::AlbumsList);
        nav_button(ui, app, "Artists", Screen::ArtistsList);
        nav_button(ui, app, "Folders", Screen::Folders);
        nav_button(ui, app, "Queue", Screen::Queue);
        nav_button(ui, app, "Replacer", Screen::Replacer);
        nav_button(ui, app, "Missing", Screen::Missing);
        nav_button(ui, app, "Duplicates", Screen::Duplicates);
        ui.separator();
        if ui.button("Settings").clicked() {
            app.navigate(Screen::Settings);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let count = app.library.song_count();
            ui.label(format!("{count} songs"));
        });
    });
}

fn nav_button(ui: &mut egui::Ui, app: &mut App, label: &str, target: Screen) {
    let active = std::mem::discriminant(&app.screen) == std::mem::discriminant(&target);
    let response = ui.selectable_label(active, label);
    if response.clicked() {
        app.navigate(target);
    }
}
