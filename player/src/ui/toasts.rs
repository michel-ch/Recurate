//! In-app toast notifications.
//!
//! Failures previously vanished into `tracing::warn!` / `tracing::error!`
//! where the user never saw them — bulk-delete skips, refresh failures,
//! download errors. A toast surfaces the same message visually, auto-dismisses
//! on a level-dependent TTL, and can be dismissed manually. Keep additions
//! cheap: `app.toast_warn(format!("…"))` from anywhere with `&mut App`.

use std::time::{Duration, Instant};

use egui::{vec2, Frame, Margin, Rect, RichText, Rounding, Stroke};

use crate::ui::theme::{self, Palette};
use crate::ui::widgets;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Warn,
    Error,
}

impl ToastLevel {
    fn ttl(self) -> Duration {
        match self {
            ToastLevel::Info => Duration::from_secs(4),
            ToastLevel::Warn => Duration::from_secs(6),
            ToastLevel::Error => Duration::from_secs(9),
        }
    }

    fn color(self, pal: &Palette) -> egui::Color32 {
        match self {
            ToastLevel::Info => pal.accent,
            ToastLevel::Warn => pal.caution,
            ToastLevel::Error => pal.critical,
        }
    }

    /// Leading word, so the level never rests on colour alone.
    fn word(self) -> &'static str {
        match self {
            ToastLevel::Info => "Info",
            ToastLevel::Warn => "Warning",
            ToastLevel::Error => "Error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub level: ToastLevel,
    pub message: String,
    pub expires_at: Instant,
}

impl Toast {
    pub fn new(level: ToastLevel, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
            expires_at: Instant::now() + level.ttl(),
        }
    }
}

/// Render the toast stack pinned to the top-right of the window. Expired
/// toasts are dropped here; dismissed ones are removed after the loop so
/// indices stay valid during iteration.
pub fn draw(ctx: &egui::Context, toasts: &mut Vec<Toast>) {
    let now = Instant::now();
    toasts.retain(|t| t.expires_at > now);
    if toasts.is_empty() {
        return;
    }
    let pal = theme::pal(ctx);

    let mut to_remove: Vec<usize> = Vec::new();
    egui::Area::new(egui::Id::new("toast_area"))
        .anchor(egui::Align2::RIGHT_TOP, vec2(-theme::SPACE_5, theme::SPACE_4))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_width(280.0);
            for (i, toast) in toasts.iter().enumerate() {
                let resp = Frame::none()
                    .fill(pal.surface)
                    .stroke(Stroke::new(1.0, pal.line_strong))
                    .rounding(Rounding::same(theme::RADIUS))
                    .inner_margin(Margin {
                        left: theme::SPACE_4,
                        right: theme::SPACE_2,
                        top: theme::SPACE_2,
                        bottom: theme::SPACE_2,
                    })
                    .show(ui, |ui| {
                        ui.set_width(280.0 - theme::SPACE_4 - theme::SPACE_2);
                        ui.horizontal_top(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(280.0 - 64.0);
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.label(
                                    widgets::strong(toast.level.word(), theme::TEXT_CAPTION)
                                        .color(toast.level.color(&pal)),
                                );
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&toast.message)
                                            .size(theme::TEXT_CAPTION)
                                            .color(pal.ink_2),
                                    )
                                    .wrap(),
                                );
                            });
                            if widgets::icon_button(ui, "✕", 24.0, pal.ink_3)
                                .on_hover_text("Dismiss")
                                .clicked()
                            {
                                to_remove.push(i);
                            }
                        });
                    })
                    .response;
                // 2px level edge on the left.
                let edge = Rect::from_min_size(
                    resp.rect.min + vec2(0.0, theme::SPACE_2),
                    vec2(2.0, resp.rect.height() - 2.0 * theme::SPACE_2),
                );
                ui.painter()
                    .rect_filled(edge, Rounding::same(1.0), toast.level.color(&pal));
                ui.add_space(theme::SPACE_1);
            }
        });
    to_remove.sort_unstable_by(|a, b| b.cmp(a));
    for i in to_remove {
        if i < toasts.len() {
            toasts.remove(i);
        }
    }

    // Toasts auto-dismiss on a TTL — without an active repaint request, the UI
    // wouldn't redraw until the user moves the mouse, leaving stale toasts on
    // screen well past `expires_at`.
    ctx.request_repaint_after(Duration::from_millis(250));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttl_orders_by_severity() {
        assert!(ToastLevel::Error.ttl() > ToastLevel::Warn.ttl());
        assert!(ToastLevel::Warn.ttl() > ToastLevel::Info.ttl());
    }

    #[test]
    fn toast_new_sets_expiry_in_future() {
        let t = Toast::new(ToastLevel::Info, "hi");
        assert!(t.expires_at > Instant::now());
    }

    #[test]
    fn every_level_has_a_word() {
        for l in [ToastLevel::Info, ToastLevel::Warn, ToastLevel::Error] {
            assert!(!l.word().is_empty());
        }
    }
}
