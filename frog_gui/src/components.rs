use egui::{DragValue, WidgetText};

pub trait UiExt {
    fn unit_edit(
        self,
        label: impl Into<WidgetText>,
        value: &mut impl egui::emath::Numeric,
        units: &str,
    ) -> egui::InnerResponse<egui::Response>;
    fn numeric_edit(
        self,
        label: impl Into<WidgetText>,
        value: &mut impl egui::emath::Numeric,
    ) -> egui::InnerResponse<egui::Response>;
}

impl UiExt for &mut egui::Ui {
    fn numeric_edit(
        self,
        label: impl Into<WidgetText>,
        value: &mut impl egui::emath::Numeric,
    ) -> egui::InnerResponse<egui::Response> {
        self.horizontal(|ui| {
            ui.label(label);
            ui.add(DragValue::new(value))
        })
    }

    fn unit_edit(
        self,
        label: impl Into<WidgetText>,
        value: &mut impl egui::emath::Numeric,
        units: &str,
    ) -> egui::InnerResponse<egui::Response> {
        self.horizontal(|ui| {
            ui.label(label);
            ui.add(DragValue::new(value).suffix(units))
        })
    }
}
