use std::collections::HashSet;

use macroquad::prelude::*;
use frogcore::{node_location::Point, units::Length};

use crate::Inspectable;

pub struct SceneData {
    pub camera: Camera2D,
    pub zoom_level: f32,
    pub drag_token: Option<(usize, Vec2)>,
    pub show_help_text: bool,
    pub panning: Option<Vec2>,
}

impl SceneData {
    pub fn new() -> SceneData {
        let camera = Camera2D {
            zoom: vec2(4. / screen_width(), 4. / screen_height()),
            //target: vec2(screen_width() / 2., screen_height() / 2.),
            ..Default::default()
        };

        SceneData {
            zoom_level: 4f32,
            camera,
            drag_token: None,
            panning: None,
            show_help_text: true,
        }
    }

    pub fn zoom_to_fit(&mut self, map: &Vec<Point>) {
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);

        for point in map.iter() {
            let x = point.x.metres() as f32;
            let y = point.y.metres() as f32;

            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }

        let x_factor = (screen_width()) / (max_x - min_x).max(10.);
        let y_factor = (screen_height()) / (max_y - min_y).max(10.);

        self.camera.target = vec2((max_x + min_x) / 2., (max_y + min_y) / 2.);
        self.zoom_level = x_factor.min(y_factor);
        self.zoom_level = self.zoom_level.clamp(0.1, 10.0);
        self.camera.zoom = vec2(
            self.zoom_level / screen_width(),
            self.zoom_level / screen_height(),
        );
    }

    pub fn camera_control(&mut self, scene_rect: Rect) {
        if !scene_rect.contains(mouse_position().into()) {
            return;
        }

        let mouse_pos = self.world_mouse_pos();
        let middle_click =
            is_mouse_button_down(MouseButton::Middle) || is_mouse_button_down(MouseButton::Right);

        // Zoom to Mouse
        let (_, scroll) = mouse_wheel();
        self.zoom_level *= scroll.tanh() * 0.25 + 1.0;

        self.zoom_level = self.zoom_level.clamp(0.1, 10.0);
        self.camera.zoom = vec2(
            self.zoom_level / screen_width(),
            self.zoom_level / screen_height(),
        );

        let delta = mouse_pos - self.world_mouse_pos();
        self.camera.target += delta;

        //Handling Panning
        match (self.panning, middle_click) {
            (None, true) => {
                self.panning = Some(mouse_pos);
            }
            (Some(origin), true) => {
                let delta = origin - mouse_pos;
                self.camera.target += delta;
            }
            (Some(_), false) => {
                self.panning = None;
            }
            (None, false) => (),
        }
    }

    pub fn select_interaction(
        &mut self,
        inspect_target: &mut Inspectable,
        map: &Vec<Point>,
        scene_rect: Rect,
    ) {
        if !scene_rect.contains(mouse_position().into()) {
            return;
        }

        let mouse_pos = self.world_mouse_pos();
        let left_click = is_mouse_button_pressed(MouseButton::Left);
        let node_size = self.node_size();

        if left_click {
            let clicked = map
                .iter()
                .enumerate()
                .find(|(_, x)| {
                    (mouse_pos - point_to_vec(**x)).length_squared() < node_size * node_size
                })
                .map(|(i, _)| i);

            if let Some(node_id) = clicked {
                *inspect_target = Inspectable::Node(node_id);
            }
        }
    }

    pub fn select_and_reposition_interaction(
        &mut self,
        inspect_target: &mut Inspectable,
        map: &mut Vec<Point>,
        scene_rect: Rect,
    ) {
        if !scene_rect.contains(mouse_position().into()) {
            return;
        }

        let mouse_pos = self.world_mouse_pos();
        let left_click = is_mouse_button_down(MouseButton::Left);
        let node_size = self.node_size();

        // Handle Dragging
        if let Some((key, offset)) = self.drag_token {
            let drag_point = mouse_pos + offset;

            // Continue Dragging
            if left_click {
                map[key] = Point {
                    x: Length::from_metres(drag_point.x as f64),
                    y: Length::from_metres(drag_point.y as f64),
                };
            }
            // End dragging
            else {
                self.drag_token = None;
            }
        }
        // Handle Clicking
        else if left_click {
            let clicked = map
                .iter()
                .enumerate()
                .find(|(_, x)| {
                    (mouse_pos - point_to_vec(**x)).length_squared() < node_size * node_size
                })
                .map(|(i, _)| i);

            if let Some(node_id) = clicked {
                *inspect_target = Inspectable::Node(node_id);
            }

            self.drag_token =
                clicked.map(|node_id| (node_id, point_to_vec(map[node_id]) - mouse_pos));
        }
    }

    pub fn scene_egui(&mut self, ui: &mut egui::Ui, can_drag: bool) {
        if self.show_help_text {
            egui::Frame::new()
                .outer_margin(5.0)
                .inner_margin(5.0)
                .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 128))
                .show(ui, |ui| {
                    if can_drag {
                        ui.label("Left click to select and drag to move nodes");
                    } else {
                        ui.label("Left click to select nodes");
                    }

                    ui.label("Middle or right click to pan");
                    ui.horizontal(|ui| {
                        ui.label("Scroll wheel to zoom");
                        ui.add_space(80.);
                        if ui.small_button("Hide").clicked() {
                            self.show_help_text = false;
                        }
                    });
                });
        }
    }

    pub fn render_grid(&self) {
        let grid_spacing = match self.zoom_level {
            ..0.9 => 1000.0,
            0.9.. => 100.0,
            _ => 1.0,
        };

        let grid_thickness = 4. / self.zoom_level;

        for x in -200..200 {
            let pos = x as f32 * grid_spacing;
            draw_line(
                pos,
                -10000000.,
                pos,
                10000000.,
                grid_thickness,
                WHITE.with_alpha(0.5),
            );
        }

        for y in -200..200 {
            let pos = y as f32 * grid_spacing;
            draw_line(
                -10000000.,
                pos,
                10000000.,
                pos,
                grid_thickness,
                WHITE.with_alpha(0.5),
            );
        }
    }

    pub fn render_scale_indicator(&self, ui: &mut egui::Ui, scene_rect: Rect) {
        let line_base_size = 2. / self.zoom_level;

        let ni = scene_rect.point() + scene_rect.size() - vec2(10., 20.);
        let Vec2 { x, y } = self.camera.screen_to_world(ni);

        draw_line(x - 1000., y, x, y, 10. * line_base_size, BLUE);
        draw_line(x - 100., y, x, y, 10. * line_base_size, RED);

        ui.painter().text(
            egui::Pos2::new(ni.x, ni.y - 10.),
            egui::Align2::RIGHT_BOTTOM,
            "  Red:  100m\nTotal: 1000m",
            egui::FontId::monospace(18.0),
            egui::Color32::WHITE,
        );
    }

    pub fn render_nodes(
        &self,
        inspect_target: &mut Inspectable,
        senders: Option<&HashSet<usize>>,
        map: &Vec<Point>,
        ui: &mut egui::Ui,
        scene_rect: Rect,
    ) {
        let node_size = self.node_size();
        for (i, point) in map.iter().enumerate() {
            let is_inspected = if let Inspectable::Node(id) = inspect_target {
                *id == i
            } else {
                false
            };

            let is_sending = senders.is_some_and(|x| x.contains(&i));

            let colour = match (is_inspected, is_sending) {
                (true, true) => YELLOW,
                (true, false) => Color::from_hex(0x90ee90),
                (false, true) => ORANGE,
                (false, false) => Color::from_hex(0xff8080),
            };

            let at_pos = vec2(point.x.metres() as f32, point.y.metres() as f32);

            draw_circle(at_pos.x, at_pos.y, node_size, colour);

            let screen_pos = self.camera.world_to_screen(at_pos);

            if scene_rect.contains(screen_pos) {
                ui.painter().text(
                    egui::Pos2::new(screen_pos.x, screen_pos.y),
                    egui::Align2::CENTER_CENTER,
                    i.to_string(),
                    egui::FontId::monospace(24.0),
                    egui::Color32::BLACK,
                );
            }
        }
    }

    fn world_mouse_pos(&self) -> Vec2 {
        self.camera.screen_to_world(mouse_position().into())
    }

    pub fn node_size(&self) -> f32 {
        50. / self.zoom_level
    }
}

pub fn point_to_vec(point: Point) -> Vec2 {
    vec2(point.x.metres() as f32, point.y.metres() as f32)
}
