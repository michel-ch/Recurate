use crate::domain::Song;

pub struct RowAction {
    pub clicked: bool,
}

pub fn draw(ui: &mut egui::Ui, index: usize, song: &Song, is_current: bool) -> RowAction {
    let mut clicked = false;
    let bg = if is_current {
        Some(ui.style().visuals.selection.bg_fill)
    } else {
        None
    };
    let response = ui.scope(|ui| {
        if let Some(c) = bg {
            ui.painter()
                .rect_filled(ui.available_rect_before_wrap(), 4.0, c);
        }
        ui.horizontal(|ui| {
            // Width budget so a long title cannot push the artist + duration
            // cluster off the right edge. The previous layout relied on
            // `Label::truncate()` alone, but in an unconstrained `horizontal`
            // the truncated title still consumed the full remaining width
            // (truncate caps the *rendered* text, not the *allocation*) and
            // the right-to-left cluster was placed at width 0, overflowing
            // past the visible area on rows with long titles.
            let row_h = 18.0;
            ui.add_sized([28.0, row_h], egui::Label::new(format!("{:>3}", index + 1)));
            let row_w = ui.available_width();
            let right_reserve = 220.0_f32.min(row_w * 0.5);
            let title_w = (row_w - right_reserve - 8.0).max(60.0);
            ui.add_sized(
                [title_w, row_h],
                egui::Label::new(egui::RichText::new(&song.title).strong()).truncate(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(song.formatted_duration());
                ui.separator();
                ui.add(egui::Label::new(&song.artist).truncate());
            });
        });
    });
    if response.response.interact(egui::Sense::click()).clicked() {
        clicked = true;
    }
    RowAction { clicked }
}
