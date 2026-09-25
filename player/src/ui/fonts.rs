//! Font setup.
//!
//! The design faces are bundled (SIL OFL, licences next to the files in
//! `assets/fonts/`): Inter for UI text, Inter SemiBold for emphasis,
//! Newsreader for page titles, JetBrains Mono for data. egui can't select a
//! weight from a variable font, so the Inter and Newsreader files are static
//! instances cut from the upstream variable fonts.
//!
//! Library filenames are full of CJK, Cyrillic and symbol characters the
//! design faces don't cover, so every family falls back to a Windows CJK font,
//! Segoe UI Symbol and Arial, then to egui's own emoji/icon fonts (which carry
//! the transport glyphs ⏮ ⏸ ⏭).

use std::path::Path;

use egui::{FontData, FontDefinitions, FontFamily};

use crate::ui::theme;

const INTER: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold.ttf");
const NEWSREADER: &[u8] = include_bytes!("../../assets/fonts/Newsreader-Regular.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf");

const WIN_CJK_CANDIDATES: &[(&str, u32)] = &[
    (r"C:\Windows\Fonts\msyh.ttc", 0),
    (r"C:\Windows\Fonts\msyhbd.ttc", 0),
    (r"C:\Windows\Fonts\simsun.ttc", 0),
    (r"C:\Windows\Fonts\simsun.ttf", 0),
    (r"C:\Windows\Fonts\YuGothM.ttc", 0),
    (r"C:\Windows\Fonts\malgun.ttf", 0),
    (r"C:\Windows\Fonts\arialuni.ttf", 0),
];

const WIN_LATIN_FALLBACKS: &[&str] = &[
    r"C:\Windows\Fonts\seguisym.ttf",
    r"C:\Windows\Fonts\arial.ttf",
];

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    // egui's defaults (Ubuntu-Light, Hack, emoji + icon fonts) stay as the
    // last fallbacks.
    let egui_proportional = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let egui_monospace = fonts
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    for (key, bytes) in [
        ("inter", INTER),
        ("inter-semibold", INTER_SEMIBOLD),
        ("newsreader", NEWSREADER),
        ("jetbrains-mono", JETBRAINS_MONO),
    ] {
        fonts
            .font_data
            .insert(key.to_string(), FontData::from_static(bytes));
    }

    let mut fallbacks: Vec<String> = Vec::new();
    for (path, index) in WIN_CJK_CANDIDATES {
        if let Some(name) = try_load(&mut fonts, path, *index, "cjk") {
            fallbacks.push(name);
            break;
        }
    }
    for path in WIN_LATIN_FALLBACKS {
        let stem = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("fallback");
        if let Some(name) = try_load(&mut fonts, path, 0, stem) {
            fallbacks.push(name);
        }
    }

    let chain = |first: &[&str], tail: &[String]| -> Vec<String> {
        let mut v: Vec<String> = first.iter().map(|s| s.to_string()).collect();
        for name in fallbacks.iter().chain(tail) {
            if !v.contains(name) {
                v.push(name.clone());
            }
        }
        v
    };

    fonts
        .families
        .insert(FontFamily::Proportional, chain(&["inter"], &egui_proportional));
    fonts.families.insert(
        FontFamily::Monospace,
        chain(&["jetbrains-mono"], &egui_monospace),
    );
    fonts.families.insert(
        FontFamily::Name(theme::SERIF.into()),
        chain(&["newsreader", "inter"], &egui_proportional),
    );
    fonts.families.insert(
        FontFamily::Name(theme::SEMIBOLD.into()),
        chain(&["inter-semibold"], &egui_proportional),
    );

    ctx.set_fonts(fonts);
}

fn try_load(
    fonts: &mut FontDefinitions,
    path: &str,
    index: u32,
    key: &str,
) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut data = FontData::from_owned(bytes);
    data.index = index;
    fonts.font_data.insert(key.to_string(), data);
    Some(key.to_string())
}
