//! Settings: library paths (with a folder picker), renumber, equalizer,
//! Replacer credentials and appearance, as a two-column card grid with a
//! footer that counts unsaved changes.
//!
//! Library paths always show the full absolute path, wrapped rather than
//! scrolled, so the whole path is visible. Picking or editing a path saves
//! immediately (as before); the rescan still happens when you leave the page
//! (`App::rescan_if_paths_changed`), not per keystroke.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{vec2, Align, Layout, RichText, Ui};
use parking_lot::Mutex;

use crate::domain::Screen;
use crate::engine::eq::BAND_FREQS_HZ;
use crate::settings::{Settings, ThemeMode};
use crate::ui::theme;
use crate::ui::widgets;
use crate::ui::App;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Root {
    Source,
    Destination,
}

/// A folder dialog running on its own thread. `result` becomes
/// `Some(Some(path))` when a folder was chosen, `Some(None)` on cancel.
struct FolderPick {
    target: Root,
    result: Arc<Mutex<Option<Option<PathBuf>>>>,
}

/// Settings-screen state that must outlive a frame.
pub struct SettingsUi {
    /// Settings as last loaded or saved (TOML); unsaved changes diff against it.
    saved: String,
    picker: Option<FolderPick>,
    show_api_key: bool,
}

impl SettingsUi {
    pub fn new(settings: &Settings) -> Self {
        Self {
            saved: to_toml(settings),
            picker: None,
            show_api_key: false,
        }
    }
}

const FOOTER_H: f32 = 56.0;

pub fn draw(ui: &mut Ui, app: &mut App) {
    poll_pick(app);

    let config = Settings::config_path()
        .map(|p| short_config_path(&p))
        .unwrap_or_default();
    widgets::page_header(ui, "Settings", Some(&config), |ui| theme_picker(ui, app));

    egui::ScrollArea::vertical()
        .max_height((ui.available_height() - FOOTER_H).max(80.0))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = theme::SPACE_4;
            ui.columns(2, |cols| {
                for col in cols.iter_mut() {
                    col.spacing_mut().item_spacing.x = theme::SPACE_2;
                }
                library_paths_card(&mut cols[0], app);
                cols[0].add_space(theme::SPACE_4);
                renumber_card(&mut cols[0], app);
                equalizer_card(&mut cols[1], app);
                cols[1].add_space(theme::SPACE_4);
                replacer_card(&mut cols[1], app);
            });
        });

    footer(ui, app);
}

/// The Equalizer screen (Settings → "Open as screen").
pub fn draw_equalizer(ui: &mut Ui, app: &mut App) {
    widgets::page_header(
        ui,
        "Equalizer",
        Some("10-band equalizer and bass boost · saved with the other settings"),
        |_| {},
    );
    egui::ScrollArea::vertical()
        .max_height((ui.available_height() - FOOTER_H).max(80.0))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(560.0));
            card(ui, "Equalizer", |ui| {
                eq_enable_toggle(ui, app);
                ui.add_space(theme::SPACE_3);
                equalizer_controls(ui, app);
            });
        });
    footer(ui, app);
}

// ---------------------------------------------------------------------------
// Cards
// ---------------------------------------------------------------------------

fn card(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        // `columns()` hands out justified layouts, which would stretch
        // buttons and justify wrapped captions; cards lay out left-aligned.
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            widgets::section_label(ui, title);
            ui.add_space(theme::SPACE_2);
            add(ui);
        });
    });
}

fn library_paths_card(ui: &mut Ui, app: &mut App) {
    card(ui, "Library paths", |ui| {
        path_row(
            ui,
            app,
            Root::Source,
            "Source root",
            "Read-only catalogue. The Replacer reads filenames here and never changes them.",
            r"e.g. C:\Users\you\Music\music_original",
        );
        ui.add_space(theme::SPACE_4);
        path_row(
            ui,
            app,
            Root::Destination,
            "Destination root",
            "Downloads land here, and the player scans this folder for playback.",
            r"e.g. C:\Users\you\Music\music",
        );
        ui.add_space(theme::SPACE_4);
        if widgets::secondary_button_sm(ui, true, "↻ Rescan source + destination").clicked() {
            rescan(app);
        }
        widgets::caption(ui, "Path changes also rescan automatically when you leave this page.");
    });
}

fn path_row(
    ui: &mut Ui,
    app: &mut App,
    root: Root,
    label: &str,
    caption: &str,
    hint: &str,
) {
    let pal = widgets::p(ui);
    ui.label(widgets::strong(label, theme::TEXT_BODY).color(pal.ink));
    widgets::caption(ui, caption);
    ui.add_space(theme::SPACE_1);

    let id = egui::Id::new(("settings_library_path", root));
    let stored = get_root(app, root);
    // Unfocused: the full absolute path. While typing: exactly what's typed,
    // so the text doesn't jump under the cursor.
    let focused = ui.memory(|m| m.has_focus(id));
    let mut text = if focused { stored.clone() } else { full_path(&stored) };

    let picking = app.settings_ui.picker.is_some();
    let picking_this = app
        .settings_ui
        .picker
        .as_ref()
        .is_some_and(|p| p.target == root);

    ui.horizontal_top(|ui| {
        let browse_w = 92.0;
        let field_w = ui.available_width() - browse_w - ui.spacing().item_spacing.x;
        let resp = widgets::path_input(ui, id, &mut text, hint, field_w);
        if resp.changed() {
            set_root(app, root, text.clone());
        }
        if resp.lost_focus() {
            set_root(app, root, full_path(&text));
            save(app);
        }
        let button = if picking_this { "Choosing…" } else { "Browse…" };
        if widgets::secondary_button_sm(ui, !picking, button)
            .on_hover_text("Choose the folder in File Explorer")
            .clicked()
        {
            start_pick(ui.ctx(), app, root, &full_path(&text));
        }
    });
}

fn renumber_card(ui: &mut Ui, app: &mut App) {
    card(ui, "Renumber", |ui| {
        let pal = widgets::p(ui);
        let mut enabled = app.settings.read().renumber.enabled;
        if widgets::toggle_labeled(ui, &mut enabled, "Renumber after delete").changed() {
            app.settings.write().renumber.enabled = enabled;
        }
        ui.add_space(theme::SPACE_2);
        ui.horizontal(|ui| {
            ui.label("Prefix threshold");
            let mut threshold = app.settings.read().renumber.threshold;
            if widgets::slider(ui, &mut threshold, 0.0..=1.0, 160.0).changed() {
                app.settings.write().renumber.threshold = threshold;
            }
            ui.label(widgets::mono(format!("{threshold:.2}"), theme::TEXT_CAPTION).color(pal.ink_2));
        });
        widgets::caption(
            ui,
            "Folders where fewer than this share of files carry a number prefix are left alone.",
        );
    });
}

fn equalizer_card(ui: &mut Ui, app: &mut App) {
    card(ui, "Equalizer", |ui| {
        ui.horizontal(|ui| {
            eq_enable_toggle(ui, app);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widgets::text_link(ui, "Open as screen ↗").clicked() {
                    app.navigate(Screen::Equalizer);
                }
            });
        });
        ui.add_space(theme::SPACE_3);
        equalizer_controls(ui, app);
    });
}

fn eq_enable_toggle(ui: &mut Ui, app: &mut App) {
    let mut enabled = app.settings.read().equalizer.enabled;
    if widgets::toggle_labeled(ui, &mut enabled, "Enable").changed() {
        app.settings.write().equalizer.enabled = enabled;
    }
}

fn equalizer_controls(ui: &mut Ui, app: &mut App) {
    let pal = widgets::p(ui);
    let mut bands = app.settings.read().equalizer.bands;
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme::SPACE_1;
        for (i, &freq) in BAND_FREQS_HZ.iter().enumerate() {
            ui.allocate_ui_with_layout(vec2(28.0, 140.0), Layout::top_down(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = theme::SPACE_1;
                if widgets::vslider_centered(ui, &mut bands[i], -12.0..=12.0, 112.0)
                    .on_hover_text(format!("{} Hz: {:+.1} dB", format_hz(freq), bands[i]))
                    .changed()
                {
                    changed = true;
                }
                ui.label(widgets::mono(format_hz(freq), 10.0).color(pal.ink_3));
            });
        }
    });
    if changed {
        app.settings.write().equalizer.bands = bands;
    }
    ui.label(widgets::mono("−12 dB … +12 dB", 10.0).color(pal.ink_3));

    ui.add_space(theme::SPACE_2);
    ui.horizontal(|ui| {
        ui.label("Bass boost");
        let mut bass = app.settings.read().equalizer.bass_boost;
        if widgets::slider(ui, &mut bass, 0.0..=12.0, 160.0).changed() {
            app.settings.write().equalizer.bass_boost = bass;
        }
        ui.label(widgets::mono(format!("+{bass:.1} dB"), theme::TEXT_CAPTION).color(pal.ink_2));
    });
}

fn replacer_card(ui: &mut Ui, app: &mut App) {
    card(ui, "Replacer", |ui| {
        let pal = widgets::p(ui);

        ui.label(widgets::strong("YouTube Data API v3 key", theme::TEXT_BODY).color(pal.ink));
        widgets::caption(ui, "Only for the API search backend. The YOUTUBE_API_KEY variable overrides it.");
        ui.add_space(theme::SPACE_1);
        ui.horizontal(|ui| {
            let mut key = app.settings.read().replacer.youtube_api_key.clone();
            let w = ui.available_width() - 28.0 - ui.spacing().item_spacing.x;
            let resp = if app.settings_ui.show_api_key {
                widgets::mono_input(ui, &mut key, "AIza…", w)
            } else {
                widgets::password_input(ui, &mut key, "AIza…", w)
            };
            if resp.changed() {
                app.settings.write().replacer.youtube_api_key = key;
            }
            let (glyph, hover) = if app.settings_ui.show_api_key {
                ("◉", "Hide key")
            } else {
                ("◎", "Show key")
            };
            if widgets::icon_button(ui, glyph, 28.0, pal.ink_2)
                .on_hover_text(hover)
                .clicked()
            {
                app.settings_ui.show_api_key = !app.settings_ui.show_api_key;
            }
        });

        ui.add_space(theme::SPACE_3);
        ui.label(widgets::strong("Cookies from browser", theme::TEXT_BODY).color(pal.ink));
        widgets::caption(
            ui,
            "chrome, firefox, edge, brave or opera. Use it when YouTube answers \
             \"Sign in to confirm you're not a bot\". Empty = off.",
        );
        ui.add_space(theme::SPACE_1);
        let mut browser = app.settings.read().replacer.cookies_browser.clone();
        let w = ui.available_width();
        if widgets::text_input(ui, &mut browser, "firefox", w).changed() {
            app.settings.write().replacer.cookies_browser = browser.clone();
        }
        if !browser.trim().is_empty() {
            ui.label(
                RichText::new(
                    "Close that browser before downloading: it locks its cookie \
                     database while it runs.",
                )
                .size(theme::TEXT_CAPTION)
                .color(pal.caution),
            );
        }
    });
}

fn theme_picker(ui: &mut Ui, app: &mut App) {
    let pal = widgets::p(ui);
    let current = app.settings.read().ui.theme;
    let name = |m: ThemeMode| match m {
        ThemeMode::System => "Match Windows",
        ThemeMode::Light => "Light",
        ThemeMode::Dark => "Dark",
    };
    let mut mode = current;
    widgets::dropdown(ui, "settings_theme", name(mode), 140.0, |ui| {
        for m in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
            ui.selectable_value(&mut mode, m, name(m));
        }
    });
    ui.label(RichText::new("Appearance").size(theme::TEXT_CAPTION).color(pal.ink_3));
    if mode != current {
        app.settings.write().ui.theme = mode;
    }
}

fn footer(ui: &mut Ui, app: &mut App) {
    let pal = widgets::p(ui);
    let pending = unsaved_changes(&app.settings_ui.saved, &to_toml(&app.settings.read()));
    ui.separator();
    ui.horizontal(|ui| {
        let (text, color) = match pending {
            0 => ("All changes saved".to_string(), pal.ink_3),
            1 => ("1 unsaved change".to_string(), pal.caution),
            n => (format!("{n} unsaved changes"), pal.caution),
        };
        ui.label(RichText::new(text).size(theme::TEXT_CAPTION).color(color));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::primary_button(ui, pending > 0, "Save settings").clicked() {
                save(app);
            }
            if widgets::secondary_button(ui, pending > 0, "Discard").clicked() {
                discard(app);
            }
        });
    });
}

// ---------------------------------------------------------------------------
// Behaviour
// ---------------------------------------------------------------------------

fn get_root(app: &App, root: Root) -> String {
    let s = app.settings.read();
    match root {
        Root::Source => s.scan.source_root.clone(),
        Root::Destination => s.scan.roots.first().cloned().unwrap_or_default(),
    }
}

fn set_root(app: &App, root: Root, value: String) {
    let mut s = app.settings.write();
    match root {
        Root::Source => s.scan.source_root = value,
        Root::Destination => {
            if s.scan.roots.is_empty() {
                s.scan.roots.push(value);
            } else {
                s.scan.roots[0] = value;
            }
        }
    }
}

/// Full absolute form of a typed or stored path (relative paths resolve
/// against the working directory, which is also how the scanner reads them).
/// Empty stays empty.
fn full_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    std::path::absolute(trimmed)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| trimmed.to_string())
}

fn start_pick(ctx: &egui::Context, app: &mut App, root: Root, current: &str) {
    let result: Arc<Mutex<Option<Option<PathBuf>>>> = Arc::new(Mutex::new(None));
    let slot = result.clone();
    let ctx = ctx.clone();
    let start = PathBuf::from(current);
    let title = match root {
        Root::Source => "Choose the source folder (read-only catalogue)",
        Root::Destination => "Choose the music folder the player scans",
    };
    // The dialog blocks its thread; running it off the UI thread keeps the
    // window painting while it's open.
    std::thread::spawn(move || {
        let mut dialog = rfd::FileDialog::new().set_title(title);
        if start.is_dir() {
            dialog = dialog.set_directory(&start);
        }
        let picked = dialog.pick_folder();
        *slot.lock() = Some(picked);
        ctx.request_repaint();
    });
    app.settings_ui.picker = Some(FolderPick { target: root, result });
}

fn poll_pick(app: &mut App) {
    let finished = app.settings_ui.picker.as_ref().and_then(|p| {
        let taken = p.result.lock().take();
        taken.map(|picked| (p.target, picked))
    });
    if let Some((root, picked)) = finished {
        app.settings_ui.picker = None;
        if let Some(path) = picked {
            set_root(app, root, full_path(&path.to_string_lossy()));
            save(app);
        }
    }
}

fn rescan(app: &mut App) {
    let library = app.library.clone();
    let source = app.source.clone();
    let dest_roots: Vec<PathBuf> = app
        .settings
        .read()
        .scan
        .roots
        .iter()
        .map(PathBuf::from)
        .collect();
    let source_root = app.settings.read().scan.source_root.clone();
    std::thread::spawn(move || {
        if let Err(e) = library.scan(&dest_roots) {
            tracing::error!("destination rescan failed: {e:#}");
        }
    });
    if !source_root.trim().is_empty() {
        let source_roots = vec![PathBuf::from(source_root)];
        std::thread::spawn(move || {
            if let Err(e) = source.scan(&source_roots) {
                tracing::error!("source rescan failed: {e:#}");
            }
        });
    }
}

fn save(app: &mut App) {
    let result = app.settings.read().save();
    match result {
        Ok(()) => app.settings_ui.saved = to_toml(&app.settings.read()),
        Err(e) => {
            tracing::warn!("save settings failed: {e:#}");
            app.toast_error(format!("Couldn't save settings: {e:#}"));
        }
    }
}

/// Put back the settings as last saved (volume included, so the engine
/// follows the slider).
fn discard(app: &mut App) {
    match toml::from_str::<Settings>(&app.settings_ui.saved) {
        Ok(saved) => {
            let volume = saved.playback.volume;
            *app.settings.write() = saved;
            app.volume = volume;
            app.playback.set_volume(volume);
        }
        Err(e) => app.toast_error(format!("Couldn't restore saved settings: {e:#}")),
    }
}

fn to_toml(settings: &Settings) -> String {
    toml::to_string(settings).unwrap_or_default()
}

/// Number of individual values that differ between two serialized settings.
fn unsaved_changes(saved: &str, current: &str) -> usize {
    match (saved.parse::<toml::Table>(), current.parse::<toml::Table>()) {
        (Ok(a), Ok(b)) => count_diff(&toml::Value::Table(a), &toml::Value::Table(b)),
        _ => usize::from(saved != current),
    }
}

fn count_diff(a: &toml::Value, b: &toml::Value) -> usize {
    match (a, b) {
        (toml::Value::Table(x), toml::Value::Table(y)) => {
            let keys: BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            keys.into_iter()
                .map(|k| match (x.get(k), y.get(k)) {
                    (Some(va), Some(vb)) => count_diff(va, vb),
                    _ => 1,
                })
                .sum()
        }
        _ => usize::from(a != b),
    }
}

/// `C:\Users\me\AppData\Roaming\…` → `%APPDATA%\…`.
fn short_config_path(p: &Path) -> String {
    let s = p.display().to_string();
    if let Ok(appdata) = std::env::var("APPDATA") {
        if let Some(rest) = s.strip_prefix(&appdata) {
            return format!("%APPDATA%{rest}");
        }
    }
    s
}

fn format_hz(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.0}k", hz / 1000.0)
    } else {
        format!("{:.0}", hz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_path_resolves_relative_against_cwd() {
        let cwd = std::env::current_dir().unwrap();
        let got = PathBuf::from(full_path("music_folder_for_test"));
        assert!(got.is_absolute());
        assert_eq!(got, cwd.join("music_folder_for_test"));
    }

    #[test]
    fn full_path_keeps_empty_and_trims() {
        assert_eq!(full_path(""), "");
        assert_eq!(full_path("   "), "");
        let abs = std::env::current_dir().unwrap().join("x");
        let padded = format!("  {}  ", abs.display());
        assert_eq!(PathBuf::from(full_path(&padded)), abs);
    }

    #[test]
    fn unsaved_changes_counts_individual_values() {
        let mut s = Settings::default();
        let saved = to_toml(&s);
        assert_eq!(unsaved_changes(&saved, &to_toml(&s)), 0);
        s.playback.volume = 0.9;
        s.renumber.enabled = !s.renumber.enabled;
        assert_eq!(unsaved_changes(&saved, &to_toml(&s)), 2);
        s.equalizer.bands[3] = 4.0; // the band array counts once
        s.equalizer.bands[4] = 4.0;
        assert_eq!(unsaved_changes(&saved, &to_toml(&s)), 3);
    }

    #[test]
    fn hz_labels() {
        assert_eq!(format_hz(62.0), "62");
        assert_eq!(format_hz(16000.0), "16k");
    }
}
