use eframe::egui::{self, Color32};

pub const INK: Color32 = Color32::from_rgb(26, 40, 52);
pub const MUTED: Color32 = Color32::from_rgb(74, 96, 113);
pub const ACCENT: Color32 = Color32::from_rgb(19, 91, 100);
pub const LINE: Color32 = Color32::from_rgb(192, 207, 214);
pub const ERROR: Color32 = Color32::from_rgb(153, 34, 38);

pub fn install(context: &egui::Context) {
    let mut style = egui::Style::default();
    style.spacing.item_spacing = egui::vec2(8.0, 7.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = Color32::from_rgb(245, 248, 250);
    style.visuals.faint_bg_color = Color32::from_rgb(234, 241, 245);
    style.visuals.selection.bg_fill = Color32::from_rgb(178, 222, 220);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    style.visuals.error_fg_color = ERROR;
    context.set_theme(egui::Theme::Light);
    context.set_global_style(style);
}
