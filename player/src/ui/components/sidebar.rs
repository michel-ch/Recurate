//! Left navigation (200px). Replaces the old top tab strip and every
//! "← Back" button: every screen is one click away.
//!
//! Counts are read from state that is already cheap or cached — the queue
//! length, and the duplicate / missing views only if a screen already built
//! them. The sidebar never triggers a fingerprint pass or a missing scan.

use egui::{vec2, Align, Align2, Rect, Rounding, Sense, Ui};

use crate::domain::Screen;
use crate::ui::theme;
use crate::ui::widgets;
use crate::ui::App;

struct Item {
    glyph: &'static str,
    label: &'static str,
    target: Screen,
    count: Option<usize>,
    flag: bool,
}

pub fn draw(ui: &mut Ui, app: &mut App) {
    let pal = widgets::p(ui);
    ui.spacing_mut().item_spacing.y = 2.0;

    // Header: logo mark + wordmark.
    ui.horizontal(|ui| {
        ui.add_space(theme::SPACE_2);
        let (rect, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, Rounding::same(theme::RADIUS_SM), pal.accent_fill);
        ui.label(
            egui::RichText::new("Recurate")
                .font(theme::serif(theme::H3))
                .color(pal.ink),
        );
    });
    ui.add_space(theme::SPACE_4);

    let queue_len = app.playback.queue_len();
    let duplicate_extras = app
        .cached_duplicates
        .as_ref()
        .map(|(_, _, groups)| groups.iter().map(|g| g.songs.len().saturating_sub(1)).sum());
    let missing = app.cached_missing.as_ref().map(|m| m.missing.len());

    // Nav scrolls above the pinned Settings block, so short windows never
    // overlap the two.
    const BOTTOM_BLOCK: f32 = 64.0;
    egui::ScrollArea::vertical()
        .max_height((ui.available_height() - BOTTOM_BLOCK).max(0.0))
        .auto_shrink([false, true])
        .show(ui, |ui| nav_sections(ui, app, queue_len, duplicate_extras, missing));

    // Bottom block: Settings + library summary.
    let folders = app.library_folders().len();
    let songs = app.library.song_count();
    ui.with_layout(egui::Layout::bottom_up(Align::Min), |ui| {
        ui.horizontal(|ui| {
            ui.add_space(theme::SPACE_2);
            ui.label(
                egui::RichText::new(format!(
                    "{} songs · {} folders",
                    group_thousands(songs),
                    folders
                ))
                .size(theme::TEXT_MICRO)
                .color(pal.ink_3),
            );
        });
        ui.add_space(theme::SPACE_1);
        nav_item(
            ui,
            app,
            Item { glyph: "⚙", label: "Settings", target: Screen::Settings, count: None, flag: false },
        );
    });
}

fn nav_sections(
    ui: &mut Ui,
    app: &mut App,
    queue_len: usize,
    duplicate_extras: Option<usize>,
    missing: Option<usize>,
) {
    section(ui, "Library");
    for item in [
        Item { glyph: "♫", label: "Songs", target: Screen::AllSongs, count: None, flag: false },
        Item { glyph: "▤", label: "Albums", target: Screen::AlbumsList, count: None, flag: false },
        Item { glyph: "◎", label: "Artists", target: Screen::ArtistsList, count: None, flag: false },
        Item { glyph: "▣", label: "Folders", target: Screen::Folders, count: None, flag: false },
        Item {
            glyph: "≡",
            label: "Queue",
            target: Screen::Queue,
            count: (queue_len > 0).then_some(queue_len),
            flag: false,
        },
    ] {
        nav_item(ui, app, item);
    }

    ui.add_space(theme::SPACE_3);
    section(ui, "Curate");
    for item in [
        Item { glyph: "☷", label: "Playlists", target: Screen::Playlists, count: None, flag: false },
        Item { glyph: "⇄", label: "Replacer", target: Screen::Replacer, count: None, flag: false },
        Item { glyph: "⤓", label: "Import playlist", target: Screen::Playlist, count: None, flag: false },
        Item {
            glyph: "◫",
            label: "Duplicates",
            target: Screen::Duplicates,
            count: duplicate_extras.filter(|n| *n > 0),
            flag: duplicate_extras.unwrap_or(0) > 0,
        },
        Item {
            glyph: "◌",
            label: "Missing",
            target: Screen::Missing,
            count: missing.filter(|n| *n > 0),
            flag: false,
        },
    ] {
        nav_item(ui, app, item);
    }
}

fn section(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(theme::SPACE_2);
        ui.label(
            egui::RichText::new(text.to_uppercase())
                .font(theme::semibold(10.0))
                .extra_letter_spacing(0.8)
                .color(widgets::p(ui).ink_3),
        );
    });
    ui.add_space(theme::SPACE_1);
}

/// Which sidebar entry a screen belongs to (detail and sub-screens light up
/// their parent).
fn nav_parent(screen: &Screen) -> Screen {
    match screen {
        Screen::Library | Screen::Search => Screen::AllSongs,
        Screen::AlbumDetail(_) => Screen::AlbumsList,
        Screen::ArtistDetail(_) => Screen::ArtistsList,
        Screen::Equalizer => Screen::Settings,
        other => other.clone(),
    }
}

fn nav_item(ui: &mut Ui, app: &mut App, item: Item) {
    let pal = widgets::p(ui);
    let active = nav_parent(&app.screen) == item.target;
    let (rect, resp) =
        ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if active {
            Some(pal.accent_soft)
        } else if resp.hovered() {
            Some(pal.surface_sunk)
        } else {
            None
        };
        if let Some(c) = bg {
            ui.painter()
                .rect_filled(rect, Rounding::same(theme::RADIUS_SM), c);
        }
        let ink = if active { pal.accent_soft_ink } else { pal.ink_2 };
        let icon = Rect::from_min_size(rect.min + vec2(theme::SPACE_2, 0.0), vec2(16.0, rect.height()));
        ui.painter().text(
            icon.center(),
            Align2::CENTER_CENTER,
            item.glyph,
            theme::sans(13.0),
            ink,
        );
        let font = if active {
            theme::semibold(theme::TEXT_BODY)
        } else {
            theme::sans(theme::TEXT_BODY)
        };
        let label = Rect::from_min_max(
            egui::pos2(icon.right() + theme::SPACE_2, rect.top()),
            egui::pos2(rect.right() - 40.0, rect.bottom()),
        );
        widgets::paint_truncated(ui, label, item.label, font, ink, Align::Min);
        if let Some(n) = item.count {
            let count = Rect::from_min_max(
                egui::pos2(rect.right() - 40.0, rect.top()),
                egui::pos2(rect.right() - theme::SPACE_2, rect.bottom()),
            );
            let color = if item.flag { pal.accent } else { pal.ink_3 };
            widgets::paint_truncated(
                ui,
                count,
                &n.to_string(),
                theme::mono(theme::TEXT_MICRO),
                color,
                Align::Max,
            );
        }
    }
    // Clicking the parent of a detail screen (Albums while in an album)
    // returns to the list.
    if resp.clicked() && app.screen != item.target {
        app.navigate(item.target);
    }
}

/// `2457` → `2 457` (thin grouping, as in the design).
fn group_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('\u{202F}');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_thousands_with_narrow_space() {
        assert_eq!(group_thousands(7), "7");
        assert_eq!(group_thousands(2457), "2\u{202F}457");
        assert_eq!(group_thousands(1234567), "1\u{202F}234\u{202F}567");
    }

    #[test]
    fn detail_screens_light_up_their_parent() {
        assert_eq!(nav_parent(&Screen::AlbumDetail("x".into())), Screen::AlbumsList);
        assert_eq!(nav_parent(&Screen::Search), Screen::AllSongs);
        assert_eq!(nav_parent(&Screen::Equalizer), Screen::Settings);
        assert_eq!(nav_parent(&Screen::Replacer), Screen::Replacer);
    }
}
