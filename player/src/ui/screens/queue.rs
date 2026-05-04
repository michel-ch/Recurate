use crate::ui::components::song_row;
use crate::ui::App;

pub fn draw(ui: &mut egui::Ui, app: &mut App) {
    ui.heading("Queue");
    let queue = app.playback.queue_snapshot();
    let current = queue.current;

    if queue.items.is_empty() {
        ui.label("Queue is empty");
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut jump_to: Option<usize> = None;
        let mut remove_id: Option<i64> = None;
        for (i, song) in queue.items.iter().enumerate() {
            ui.horizontal(|ui| {
                let row = song_row::draw(ui, i, song, current == Some(i));
                if row.clicked {
                    jump_to = Some(i);
                }
                if ui.small_button("✕").on_hover_text("Remove from queue").clicked() {
                    remove_id = Some(song.id);
                }
            });
        }
        if let Some(idx) = jump_to {
            app.playback.jump_to(idx);
        }
        if let Some(id) = remove_id {
            app.playback.remove_from_queue(id);
        }
    });
}
