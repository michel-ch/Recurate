use crate::ui::components::song_menu::{self, MenuAction};
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
        let mut menu: Option<(MenuAction, usize)> = None;
        for (i, song) in queue.items.iter().enumerate() {
            let row = song_row::draw_with_options(
                ui,
                i,
                song,
                current == Some(i),
                song_row::RowOptions {
                    show_remove: true,
                    remove_hover: Some("Remove from queue"),
                    ..Default::default()
                },
            );
            if row.clicked {
                jump_to = Some(i);
            }
            if row.remove_clicked {
                remove_id = Some(song.id);
            }
            if let Some(a) = row.menu {
                menu = Some((a, i));
            }
        }
        if let Some(idx) = jump_to {
            app.playback.jump_to(idx);
        }
        if let Some(id) = remove_id {
            app.playback.remove_from_queue(id);
        }
        if let Some((a, i)) = menu {
            song_menu::perform(app, a, &queue.items[i], &queue.items);
        }
    });
}
