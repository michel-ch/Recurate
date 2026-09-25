//! Design tokens (the design spec under `docs/`) mapped onto egui.
//!
//! Every colour a screen paints comes from [`Palette`]; screens never write a
//! raw `Color32::from_rgb`. [`apply`] installs the palette into egui's
//! `Visuals` plus the spacing, radii and text styles, and stores the palette in
//! context memory so [`pal`] can hand it to any widget. eframe resets
//! `Visuals` when the OS theme flips, so `App::update` calls [`ensure`] every
//! frame; it only re-applies when the installed palette doesn't match.

use egui::{
    Color32, FontFamily, FontId, Margin, Rounding, Shadow, Stroke, TextStyle, Vec2, Visuals,
};

use crate::settings::ThemeMode;

/// One palette of design tokens. Field names follow the CSS custom
/// properties in the design spec (`--ink-2` → `ink_2`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub paper: Color32,
    pub surface: Color32,
    pub surface_sunk: Color32,
    pub ink: Color32,
    pub ink_2: Color32,
    pub ink_3: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    pub accent: Color32,
    pub accent_fill: Color32,
    pub accent_hover: Color32,
    pub accent_ink: Color32,
    pub accent_soft: Color32,
    pub accent_soft_ink: Color32,
    pub positive: Color32,
    pub critical: Color32,
    /// Derived: "attention, not danger" (unapplied edits, missing API key).
    pub caution: Color32,
    /// Derived: wash for rows about to be deleted.
    pub critical_soft: Color32,
    /// Modal backdrop.
    pub scrim: Color32,
    pub shadow: Color32,
}

pub const LIGHT: Palette = Palette {
    dark: false,
    paper: Color32::from_rgb(0xF6, 0xF3, 0xEC),
    surface: Color32::from_rgb(0xEF, 0xEA, 0xDF),
    surface_sunk: Color32::from_rgb(0xE7, 0xE1, 0xD4),
    ink: Color32::from_rgb(0x33, 0x30, 0x2A),
    ink_2: Color32::from_rgb(0x6B, 0x63, 0x56),
    ink_3: Color32::from_rgb(0x97, 0x8C, 0x7B),
    line: Color32::from_rgb(0xE2, 0xDB, 0xCE),
    line_strong: Color32::from_rgb(0xD6, 0xCD, 0xBC),
    accent: Color32::from_rgb(0xB3, 0x50, 0x2F),
    accent_fill: Color32::from_rgb(0xA8, 0x49, 0x2B),
    accent_hover: Color32::from_rgb(0x8F, 0x3D, 0x22),
    accent_ink: Color32::from_rgb(0xFB, 0xF8, 0xF1),
    accent_soft: Color32::from_rgb(0xF1, 0xE3, 0xD8),
    accent_soft_ink: Color32::from_rgb(0x8A, 0x3C, 0x22),
    positive: Color32::from_rgb(0x5C, 0x6B, 0x3F),
    critical: Color32::from_rgb(0x9C, 0x3B, 0x2E),
    caution: Color32::from_rgb(0x7A, 0x5A, 0x1E),
    critical_soft: Color32::from_rgb(0xF2, 0xDF, 0xD9),
    // rgba(54,46,36,.35), premultiplied
    scrim: Color32::from_rgba_premultiplied(19, 16, 13, 89),
    // rgba(54,46,36,.06), premultiplied
    shadow: Color32::from_rgba_premultiplied(3, 3, 2, 15),
};

pub const DARK: Palette = Palette {
    dark: true,
    paper: Color32::from_rgb(0x22, 0x20, 0x1B),
    surface: Color32::from_rgb(0x2A, 0x28, 0x23),
    surface_sunk: Color32::from_rgb(0x1C, 0x1A, 0x16),
    ink: Color32::from_rgb(0xEA, 0xE4, 0xD6),
    ink_2: Color32::from_rgb(0xB3, 0xAB, 0x9A),
    ink_3: Color32::from_rgb(0x84, 0x7C, 0x6D),
    line: Color32::from_rgb(0x38, 0x35, 0x2D),
    line_strong: Color32::from_rgb(0x46, 0x42, 0x3A),
    accent: Color32::from_rgb(0xD9, 0x8A, 0x66),
    accent_fill: Color32::from_rgb(0xAE, 0x4F, 0x30),
    accent_hover: Color32::from_rgb(0x98, 0x42, 0x29),
    accent_ink: Color32::from_rgb(0xFB, 0xF6, 0xEE),
    // rgba(205,117,81,.15), premultiplied
    accent_soft: Color32::from_rgba_premultiplied(31, 18, 12, 38),
    accent_soft_ink: Color32::from_rgb(0xE7, 0xA9, 0x8A),
    positive: Color32::from_rgb(0x8A, 0x9A, 0x66),
    critical: Color32::from_rgb(0xD0, 0x7A, 0x66),
    caution: Color32::from_rgb(0xD9, 0xB2, 0x6A),
    // rgba(208,122,102,.15), premultiplied
    critical_soft: Color32::from_rgba_premultiplied(31, 18, 15, 38),
    scrim: Color32::from_rgba_premultiplied(0, 0, 0, 115),
    // rgba(0,0,0,.25), premultiplied
    shadow: Color32::from_rgba_premultiplied(0, 0, 0, 64),
};

// Spacing scale (8pt grid, fine control at 4).
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 24.0;
pub const SPACE_6: f32 = 32.0;
pub const SPACE_7: f32 = 48.0;

pub const RADIUS_SM: f32 = 6.0;
pub const RADIUS: f32 = 10.0;
pub const RADIUS_LG: f32 = 14.0;

/// `--w-app`: content never grows wider than this.
pub const W_APP: f32 = 1120.0;

// Type scale.
pub const H1: f32 = 34.0;
pub const H2: f32 = 26.0;
pub const H3: f32 = 21.0;
pub const TEXT_BODY: f32 = 13.0;
pub const TEXT_CAPTION: f32 = 12.0;
pub const TEXT_MICRO: f32 = 11.0;

/// Font family names registered by `ui::fonts`.
pub const SERIF: &str = "serif";
pub const SEMIBOLD: &str = "semibold";

pub fn serif(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SERIF.into()))
}

pub fn sans(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

fn palette_id() -> egui::Id {
    egui::Id::new("recurate_palette")
}

/// The palette currently installed. Cheap (`Copy`), call it anywhere.
pub fn pal(ctx: &egui::Context) -> Palette {
    ctx.data(|d| d.get_temp::<Palette>(palette_id()))
        .unwrap_or(LIGHT)
}

/// Resolve the user's preference against the OS theme eframe reports.
pub fn wants_dark(mode: ThemeMode, system: Option<eframe::Theme>) -> bool {
    match mode {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::System => matches!(system, Some(eframe::Theme::Dark)),
    }
}

/// Re-apply the palette if egui's visuals drifted from it (first frame, a
/// preference change, or eframe resetting visuals on an OS theme change).
pub fn ensure(ctx: &egui::Context, dark: bool) {
    let want = if dark { DARK } else { LIGHT };
    let installed = ctx.style().visuals.panel_fill;
    if installed != want.paper || pal(ctx).dark != dark {
        apply(ctx, want);
    }
}

pub fn apply(ctx: &egui::Context, p: Palette) {
    ctx.data_mut(|d| d.insert_temp(palette_id(), p));

    let mut style = (*ctx.style()).clone();

    // ---- visuals -----------------------------------------------------------
    let mut v = if p.dark { Visuals::dark() } else { Visuals::light() };
    v.dark_mode = p.dark;
    v.override_text_color = None;
    v.panel_fill = p.paper;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.line_strong);
    v.window_rounding = Rounding::same(RADIUS_LG);
    v.menu_rounding = Rounding::same(RADIUS);
    let shadow = Shadow {
        offset: Vec2::new(0.0, 1.0),
        blur: 2.0,
        spread: 0.0,
        color: p.shadow,
    };
    v.window_shadow = shadow;
    v.popup_shadow = shadow;
    v.extreme_bg_color = p.surface_sunk; // text edit background
    v.faint_bg_color = p.surface;
    v.code_bg_color = p.surface_sunk;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.caution;
    v.error_fg_color = p.critical;
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent_soft_ink);
    v.slider_trailing_fill = true;
    v.striped = false;
    v.indent_has_left_vline = false;
    v.button_frame = true;
    v.collapsing_header_frame = false;

    let rounding = Rounding::same(RADIUS_SM);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.ink);
    w.noninteractive.rounding = rounding;

    // Secondary button at rest: transparent with a hairline.
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
    w.inactive.bg_fill = p.surface_sunk; // checkbox / slider rail
    w.inactive.bg_stroke = Stroke::new(1.0, p.line_strong);
    w.inactive.fg_stroke = Stroke::new(1.0, p.ink);
    w.inactive.rounding = rounding;
    w.inactive.expansion = 0.0;

    w.hovered.weak_bg_fill = p.surface;
    w.hovered.bg_fill = p.surface;
    w.hovered.bg_stroke = Stroke::new(1.0, p.line_strong);
    w.hovered.fg_stroke = Stroke::new(1.0, p.ink);
    w.hovered.rounding = rounding;
    w.hovered.expansion = 0.0;

    w.active.weak_bg_fill = p.surface_sunk;
    w.active.bg_fill = p.surface_sunk;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.0, p.ink);
    w.active.rounding = rounding;
    w.active.expansion = 0.0;

    w.open.weak_bg_fill = p.surface;
    w.open.bg_fill = p.surface;
    w.open.bg_stroke = Stroke::new(1.0, p.line_strong);
    w.open.fg_stroke = Stroke::new(1.0, p.ink);
    w.open.rounding = rounding;
    w.open.expansion = 0.0;

    style.visuals = v;

    // ---- spacing -----------------------------------------------------------
    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(SPACE_2, SPACE_2);
    s.button_padding = Vec2::new(SPACE_3, SPACE_1);
    s.interact_size = Vec2::new(40.0, 28.0);
    s.window_margin = Margin::same(SPACE_5);
    s.menu_margin = Margin::same(SPACE_2);
    s.indent = SPACE_4;
    s.icon_width = 16.0;
    s.icon_width_inner = 8.0;
    s.icon_spacing = SPACE_2;
    s.combo_width = 160.0;
    s.text_edit_width = 240.0;

    // ---- motion ------------------------------------------------------------
    style.animation_time = 0.15; // --dur-fast

    // ---- type --------------------------------------------------------------
    style.text_styles = [
        (TextStyle::Small, sans(TEXT_CAPTION)),
        (TextStyle::Body, sans(TEXT_BODY)),
        (TextStyle::Button, sans(TEXT_BODY)),
        (TextStyle::Monospace, mono(TEXT_CAPTION)),
        (TextStyle::Heading, serif(H2)),
    ]
    .into();

    ctx.set_style(style);
}
