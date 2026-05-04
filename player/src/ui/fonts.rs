use std::path::Path;

use egui::{FontData, FontDefinitions, FontFamily};

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

pub fn install_unicode_fallbacks(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut added: Vec<String> = Vec::new();

    for (path, index) in WIN_CJK_CANDIDATES {
        if let Some(name) = try_load(&mut fonts, path, *index, "cjk") {
            added.push(name);
            break;
        }
    }
    for path in WIN_LATIN_FALLBACKS {
        let stem = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("fallback");
        if let Some(name) = try_load(&mut fonts, path, 0, stem) {
            added.push(name);
        }
    }

    if added.is_empty() {
        return;
    }

    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let entry = fonts.families.entry(family).or_default();
        for name in &added {
            if !entry.contains(name) {
                entry.push(name.clone());
            }
        }
    }

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
