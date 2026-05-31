use macroquad::prelude::*;

#[derive(Debug, Clone, Default)]
pub struct Debugger {
    // Gizmos
    points: Vec<(Vec2, Color)>,
    lines: Vec<(Vec2, Vec2, Color, u32)>,
    rects: Vec<(Rect, Color)>,
    rings: Vec<(Vec2, f32)>,
    text: Vec<String>,
}

impl Debugger {
    pub fn new() -> Self {
        Debugger {
            ..Default::default()
        }
    }

    pub fn draw_point(&mut self, point: Vec2, colour: Color) {
        self.points.push((point, colour));
    }

    pub fn draw_line(&mut self, start: Vec2, end: Vec2, colour: Color) {
        self.lines.push((start, end, colour, 1));
    }

    pub fn draw_sticky_line(&mut self, start: Vec2, end: Vec2, colour: Color, lifetime: u32) {
        self.lines.push((start, end, colour, lifetime));
    }

    pub fn draw_rect(&mut self, rect: Rect, colour: Color) {
        self.rects.push((rect, colour));
    }

    pub fn draw_ring(&mut self, centre: Vec2, radius: f32) {
        self.rings.push((centre, radius));
    }

    pub fn draw_text(&mut self, text: String) {
        self.text.push(text);
    }

    pub fn reset(&mut self) {
        self.points.clear();
        self.rects.clear();
        self.rings.clear();
        self.text.clear();

        self.lines.retain_mut(|(_, _, _, lifetime)| {
            *lifetime -= 1;
            *lifetime > 0
        });
    }

    pub fn render_debug(&self) {
        for (Vec2 { x, y }, colour) in self.points.iter().copied() {
            draw_circle(x, y, 2.0, colour);
        }

        for (start, end, colour, _) in self.lines.iter().copied() {
            draw_line(start.x, start.y, end.x, end.y, 2.0, colour);
        }

        for (rect, colour) in self.rects.iter().copied() {
            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, colour);
        }

        for (centre, radius) in self.rings.iter().copied() {
            draw_circle_lines(centre.x, centre.y, radius, 2.0, GREEN);
        }

        for (n, line) in self.text.iter().enumerate() {
            draw_text(
                &line,
                0.,
                screen_height() - (n + 1) as f32 * 16.,
                16.,
                WHITE,
            );
        }
    }
}
