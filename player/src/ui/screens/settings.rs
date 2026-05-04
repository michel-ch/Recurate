use crate::engine::eq::BAND_FREQS_HZ;
use crate::ui::App;

pub fn draw(ui: &mut egui::Ui, app: &mut App) {
    ui.heading("Settings");
    ui.separator();

    ui.collapsing("Library paths", |ui| {
        ui.label("Source root (Replacer reads filenames from here — never modified):");
        let mut src = app.settings.read().scan.source_root.clone();
        let src_resp = ui.add(
            egui::TextEdit::singleline(&mut src)
                .desired_width(420.0)
                .hint_text("e.g. C:\\Users\\you\\Music\\music_original"),
        );
        if src_resp.changed() {
            app.settings.write().scan.source_root = src;
        }
        if src_resp.lost_focus() {
            if let Err(e) = app.settings.read().save() {
                tracing::warn!("settings auto-save failed: {e:#}");
            }
        }

        ui.add_space(4.0);
        ui.label("Destination root (downloads land here, and the player scans this for playback):");
        let mut dest = app
            .settings
            .read()
            .scan
            .roots
            .first()
            .cloned()
            .unwrap_or_default();
        let dest_resp = ui.add(
            egui::TextEdit::singleline(&mut dest)
                .desired_width(420.0)
                .hint_text("e.g. C:\\Users\\you\\Music\\music"),
        );
        if dest_resp.changed() {
            let mut s = app.settings.write();
            if s.scan.roots.is_empty() {
                s.scan.roots.push(dest);
            } else {
                s.scan.roots[0] = dest;
            }
        }
        if dest_resp.lost_focus() {
            if let Err(e) = app.settings.read().save() {
                tracing::warn!("settings auto-save failed: {e:#}");
            }
        }

        ui.add_space(4.0);
        if ui.button("Rescan source + destination").clicked() {
            let library = app.library.clone();
            let source = app.source.clone();
            let dest_roots: Vec<std::path::PathBuf> = app
                .settings
                .read()
                .scan
                .roots
                .iter()
                .map(std::path::PathBuf::from)
                .collect();
            let source_root = app.settings.read().scan.source_root.clone();
            std::thread::spawn(move || {
                if let Err(e) = library.scan(&dest_roots) {
                    tracing::error!("destination rescan failed: {e:#}");
                }
            });
            if !source_root.trim().is_empty() {
                let source_roots = vec![std::path::PathBuf::from(source_root)];
                std::thread::spawn(move || {
                    if let Err(e) = source.scan(&source_roots) {
                        tracing::error!("source rescan failed: {e:#}");
                    }
                });
            }
        }
    });

    ui.collapsing("Renumber", |ui| {
        let mut enabled = app.settings.read().renumber.enabled;
        let mut threshold = app.settings.read().renumber.threshold;
        if ui.checkbox(&mut enabled, "Renumber after delete").changed() {
            app.settings.write().renumber.enabled = enabled;
        }
        if ui
            .add(egui::Slider::new(&mut threshold, 0.0..=1.0).text("Prefix threshold"))
            .changed()
        {
            app.settings.write().renumber.threshold = threshold;
        }
    });

    ui.collapsing("Equalizer", |ui| {
        draw_equalizer(ui, app);
    });

    ui.collapsing("Replacer", |ui| {
        let mut key = app.settings.read().replacer.youtube_api_key.clone();
        ui.label("YouTube Data API v3 key (used by the Replacer screen):");
        let resp = ui.add(
            egui::TextEdit::singleline(&mut key)
                .password(true)
                .desired_width(360.0)
                .hint_text("AIza…"),
        );
        if resp.changed() {
            app.settings.write().replacer.youtube_api_key = key;
        }
        ui.label(
            egui::RichText::new(
                "Stored in settings.toml. Env var YOUTUBE_API_KEY (if set) takes precedence.",
            )
            .weak(),
        );

        ui.add_space(6.0);
        let mut browser = app.settings.read().replacer.cookies_browser.clone();
        ui.label("Cookies-from-browser (for yt-dlp downloads):");
        let resp = ui.add(
            egui::TextEdit::singleline(&mut browser)
                .desired_width(360.0)
                .hint_text("chrome / firefox / edge / brave / opera (empty = off)"),
        );
        if resp.changed() {
            app.settings.write().replacer.cookies_browser = browser;
        }
        ui.label(
            egui::RichText::new(
                "Set this if YouTube replies \"Sign in to confirm you're not a bot\". \
                 yt-dlp will read cookies from the named browser profile.",
            )
            .weak(),
        );
    });

    ui.add_space(10.0);
    if ui.button("Save settings").clicked() {
        if let Err(e) = app.settings.read().save() {
            tracing::warn!("save settings failed: {e:#}");
        }
    }
}

pub fn draw_equalizer(ui: &mut egui::Ui, app: &mut App) {
    let mut enabled = app.settings.read().equalizer.enabled;
    if ui.checkbox(&mut enabled, "Enable equalizer").changed() {
        app.settings.write().equalizer.enabled = enabled;
    }
    let mut bands = app.settings.read().equalizer.bands;
    let mut changed = false;
    ui.horizontal(|ui| {
        for (i, &freq) in BAND_FREQS_HZ.iter().enumerate() {
            ui.vertical(|ui| {
                let resp = ui.add(
                    egui::Slider::new(&mut bands[i], -12.0..=12.0)
                        .vertical()
                        .show_value(false),
                );
                if resp.changed() {
                    changed = true;
                }
                ui.label(format_hz(freq));
            });
        }
    });
    if changed {
        app.settings.write().equalizer.bands = bands;
    }

    let mut bass = app.settings.read().equalizer.bass_boost;
    if ui
        .add(egui::Slider::new(&mut bass, 0.0..=12.0).text("Bass boost (dB)"))
        .changed()
    {
        app.settings.write().equalizer.bass_boost = bass;
    }
}

fn format_hz(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.0}k", hz / 1000.0)
    } else {
        format!("{:.0}", hz)
    }
}
