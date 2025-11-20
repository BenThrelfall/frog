use egui::style::{WidgetVisuals, Widgets};
use egui::{CornerRadius, Visuals};
use egui::{Color32, Stroke};

pub fn dark_visuals() -> Visuals {
    let back_background = Color32::from_hex("#333333").unwrap();
    let item_background = Color32::from_hex("#212121").unwrap();

    let main_red = Color32::from_hex("#9b0d0d").unwrap();
    let hover_red = Color32::from_hex("#af1818").unwrap();

    Visuals {
        window_fill: back_background,
        panel_fill: back_background,
        override_text_color: Some(Color32::WHITE),
        menu_corner_radius: CornerRadius::ZERO,
        window_corner_radius: CornerRadius::ZERO,
        widgets: Widgets {
            noninteractive: WidgetVisuals {
                bg_fill: item_background,
                weak_bg_fill: item_background,
                bg_stroke: Stroke::new(1.0, main_red),
                corner_radius: CornerRadius::ZERO,
                fg_stroke: Stroke::new(1.0, main_red),
                expansion: 0.0,
            },
            inactive: WidgetVisuals {
                bg_fill: main_red,
                weak_bg_fill: main_red,
                bg_stroke: Stroke::new(1.0, main_red),
                corner_radius: CornerRadius::ZERO,
                fg_stroke: Stroke::new(1.0, item_background),
                expansion: 0.0,
            },
            hovered: WidgetVisuals {
                bg_fill: hover_red,
                weak_bg_fill: hover_red,
                bg_stroke: Stroke::new(1.0, hover_red),
                corner_radius: CornerRadius::ZERO,
                fg_stroke: Stroke::new(1.0, item_background),
                expansion: 0.0,
            },
            active: WidgetVisuals {
                bg_fill: hover_red,
                weak_bg_fill: hover_red,
                bg_stroke: Stroke::new(1.0, hover_red),
                corner_radius: CornerRadius::ZERO,
                fg_stroke: Stroke::new(1.0, item_background),
                expansion: 0.0,
            },
            open: WidgetVisuals {
                bg_fill: item_background,
                weak_bg_fill: item_background,
                bg_stroke: Stroke::new(1.0, main_red),
                corner_radius: CornerRadius::ZERO,
                fg_stroke: Stroke::new(1.0, main_red),
                expansion: 0.0,
            },
        },
        ..Visuals::dark()
    }
}

