use crate::domain::Song;

pub struct RowAction {
    pub clicked: bool,
    pub remove_clicked: bool,
}

pub fn draw(ui: &mut egui::Ui, index: usize, song: &Song, is_current: bool) -> RowAction {
    draw_with_options(ui, index, song, is_current, RowOptions::default())
}

#[derive(Clone, Copy, Default)]
pub struct RowOptions {
    pub show_remove: bool,
    pub remove_hover: Option<&'static str>,
}

pub fn draw_with_options(
    ui: &mut egui::Ui,
    index: usize,
    song: &Song,
    is_current: bool,
    opts: RowOptions,
) -> RowAction {
    let bg = if is_current {
        Some(ui.style().visuals.selection.bg_fill)
    } else {
        None
    };
    let mut remove_clicked = false;
    let mut play_clicked = false;
    // Allocate the row rect with click sensing first so the entire bar is
    // clickable (including gaps between widgets), and so each row's width is
    // pinned to the parent's available_width — without that pin, the
    // ScrollArea widened each frame and the right-aligned cluster staircased
    // off the right edge. An explicit ▶ button covers cases where label
    // hit-testing intercepts the row click on a given platform.
    let row_w = ui.available_width();
    let row_h = 20.0;
    let (row_rect, response) =
        ui.allocate_exact_size(egui::vec2(row_w, row_h), egui::Sense::click());
    let mut child = ui.child_ui(
        row_rect,
        egui::Layout::left_to_right(egui::Align::Center),
        None,
    );
    {
        let ui = &mut child;
        if let Some(c) = bg {
            ui.painter().rect_filled(row_rect, 4.0, c);
        }
        ui.add_sized([28.0, row_h], egui::Label::new(format!("{:>3}", index + 1)));
        if ui
            .small_button("▶")
            .on_hover_text("Play")
            .clicked()
        {
            play_clicked = true;
        }
        ui.add_space(4.0);
        let inner_w = ui.available_width();
        let remove_reserve = if opts.show_remove { 28.0 } else { 0.0 };
        let right_reserve = (220.0_f32 + remove_reserve).min(inner_w * 0.6);
        let title_w = (inner_w - right_reserve - 8.0).max(60.0);
        ui.add_sized(
            [title_w, row_h],
            egui::Label::new(egui::RichText::new(&song.title).strong()).truncate(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if opts.show_remove {
                let btn = ui.small_button("✕");
                let btn = match opts.remove_hover {
                    Some(text) => btn.on_hover_text(text),
                    None => btn,
                };
                if btn.clicked() {
                    remove_clicked = true;
                }
                ui.add_space(4.0);
            }
            ui.label(song.formatted_duration());
            ui.separator();
            ui.add(egui::Label::new(&song.artist).truncate());
        });
    }
    RowAction {
        clicked: play_clicked || response.clicked(),
        remove_clicked,
    }
}
