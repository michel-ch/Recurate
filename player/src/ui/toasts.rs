//! In-app toast notifications.
//!
//! Failures previously vanished into `tracing::warn!` / `tracing::error!`
//! where the user never saw them — bulk-delete skips, refresh failures,
//! download errors. A toast surfaces the same message visually, auto-dismisses
//! on a level-dependent TTL, and can be dismissed manually. Keep additions
//! cheap: `app.toast_warn(format!("…"))` from anywhere with `&mut App`.

use std::time::{Duration, Instant};

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

    fn color(self) -> egui::Color32 {
        match self {
            ToastLevel::Info => egui::Color32::from_rgb(60, 110, 180),
            ToastLevel::Warn => egui::Color32::from_rgb(180, 130, 40),
            ToastLevel::Error => egui::Color32::from_rgb(180, 60, 60),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            ToastLevel::Info => "ℹ",
            ToastLevel::Warn => "⚠",
            ToastLevel::Error => "✖",
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

/// Render the toast stack as a floating area pinned to the top-right of the
/// screen. Returns the indices of toasts that the user dismissed; the caller
/// removes them from `toasts` after the closure returns (avoiding mutation
/// during iteration). Expired toasts are dropped here as well.
pub fn draw(ctx: &egui::Context, toasts: &mut Vec<Toast>) {
    let now = Instant::now();
    toasts.retain(|t| t.expires_at > now);
    if toasts.is_empty() {
        return;
    }

    let mut to_remove: Vec<usize> = Vec::new();
    egui::Area::new(egui::Id::new("toast_area"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-12.0, 56.0))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            ui.set_max_width(360.0);
            for (i, toast) in toasts.iter().enumerate() {
                let frame = egui::Frame::popup(ui.style())
                    .stroke(egui::Stroke::new(1.5, toast.level.color()))
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0));
                frame.show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.label(
                            egui::RichText::new(toast.level.icon())
                                .color(toast.level.color())
                                .strong(),
                        );
                        ui.add(egui::Label::new(&toast.message).wrap());
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::TOP),
                            |ui| {
                                if ui
                                    .small_button("✕")
                                    .on_hover_text("Dismiss")
                                    .clicked()
                                {
                                    to_remove.push(i);
                                }
                            },
                        );
                    });
                });
                ui.add_space(4.0);
            }
        });
    // Remove from highest index downward so earlier indices stay valid.
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
}
