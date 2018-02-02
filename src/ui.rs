use ::palette::pixel::Srgb;
use ::cgmath::{ElementWise, Vector2};

use ::renderer::{Draw, Render};

#[derive(Debug, Clone, Copy)]
// Hard-coded Solarized theme
// This is wrong, but I'm lazy. Goodbye, corners! ✂
pub enum Style {
    Light,
    Dark,
}

fn u8_to_rgb(r: u8, g: u8, b: u8) -> [f32; 4] {
    let rgb: ::palette::Rgb = Srgb::new_u8(r, g, b).into();
    rgb.to_pixel()
}

impl Style {
    pub fn inverse(&self) -> Self {
        match *self {
            Style::Light => Style::Dark,
            Style::Dark => Style::Light,
        }
    }

    pub fn base0(&self) -> [f32; 4] {
        match *self {
            Style::Light => u8_to_rgb(0x83, 0x94, 0x96),
            Style::Dark => u8_to_rgb(0x65, 0x7b, 0x83),
        }
    }

    pub fn base1(&self) -> [f32; 4] {
        match *self {
            Style::Light => u8_to_rgb(0x93, 0xa1, 0xa1),
            Style::Dark => u8_to_rgb(0x58, 0x6e, 0x75),
        }
    }

    pub fn base2(&self) -> [f32; 4] {
        match *self {
            Style::Light => u8_to_rgb(0xee, 0xe8, 0xd5),
            Style::Dark => u8_to_rgb(0x07, 0x36, 0x42),
        }
    }

    pub fn base3(&self) -> [f32; 4] {
        match *self {
            Style::Light => u8_to_rgb(0xfd, 0xf6, 0xe3),
            Style::Dark => u8_to_rgb(0x00, 0x2b, 0x36),
        }
    }

    pub fn yellow(&self) -> [f32; 4] {
        u8_to_rgb(0xb5, 0x89, 0x00)
    }

    pub fn orange(&self) -> [f32; 4] {
        u8_to_rgb(0xcb, 0x4b, 0x16)
    }

    pub fn red(&self) -> [f32; 4] {
        u8_to_rgb(0xdc, 0x32, 0x2f)
    }

    pub fn magenta(&self) -> [f32; 4] {
        u8_to_rgb(0xd3, 0x36, 0x82)
    }

    pub fn violet(&self) -> [f32; 4] {
        u8_to_rgb(0x6c, 0x71, 0xc4)
    }

    pub fn blue(&self) -> [f32; 4] {
        u8_to_rgb(0x26, 0x8b, 0xd2)
    }

    pub fn cyan(&self) -> [f32; 4] {
        u8_to_rgb(0x2a, 0xa1, 0x98)
    }

    pub fn green(&self) -> [f32; 4] {
        u8_to_rgb(0x85, 0x99, 0x00)
    }
}

#[derive(Debug, Clone)]
pub struct Grid {
    pub size: Vector2<f32>,
    pub view: (Vector2<f32>, Vector2<f32>),
    pub beats: u8,
    pub style: Style,
    thin_width: f32,
    thick_width: f32,
}

impl Grid {
    pub fn new(size: Vector2<f32>, view: (Vector2<f32>, Vector2<f32>)) -> Self {
        Grid {
            size, view,
            style: Style::Dark,
            beats: 4,
            thin_width: 1.0,
            thick_width: 2.0,
        }
    }

    pub fn scale_vector(&self, position: Vector2<f32>) -> Vector2<f32> {
        let view_size = self.view.1 - self.view.0;

        position.div_element_wise(self.size).mul_element_wise(view_size)
    }
}

impl Draw for Grid {
    fn draw<R: Render>(&self, size: Vector2<f32>, renderer: &mut R) {
        let (v0, v1) = self.view;
        let v_size = v1 - v0;
        let aspect = size.div_element_wise(v_size);

        let scale = [0, 5, 10, 13, 18, 23, 28];

        let (y_first, y_last) = (
            v0.y.ceil() as i32,
            v1.y.floor() as i32
        );
        for line in y_first..(y_last + 1) {
            let pos = (line as f32 - v0.y) * aspect.y;
            let line_width =
                if line % 31 != 0 { self.thin_width }
                else { self.thick_width };
            let color =
                if line % 31 == 0 { self.style.base1() }
                else if scale.contains(&(line % 31)) { self.style.blue() }
                else { self.style.base2() };

            renderer.render_rect(
                Vector2::new(0.0, pos - 0.5 * line_width),
                Vector2::new(self.size.x, pos + 0.5 * line_width),
                color
            );
        }

        let beats = self.beats as f32;
        let (x_first, x_last) = (
            (v0.x * beats).ceil() as i32,
            (v1.x * beats).floor() as i32,
        );
        for line in x_first..(x_last) + 1 {
            let pos = (line as f32 / beats - v0.x) * aspect.x;
            let line_width =
                if line % self.beats as i32 != 0 { self.thin_width }
                else { self.thick_width };
            let color =
                if line % self.beats as i32 != 0 { self.style.base3() }
                else { self.style.base2() };

            renderer.render_rect(
                Vector2::new(pos - 0.5 * line_width, 0.0),
                Vector2::new(pos + 0.5 * line_width, self.size.y),
                color
            )
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlayBar {
    pub position: f32,
    pub style: Style,
}

impl Draw for PlayBar {
    fn draw<R: Render>(&self, size: Vector2<f32>, renderer: &mut R) {
        let width = 2.0;
        let color = self.style.violet();

        let pos = self.position * size.x;

        renderer.render_rect(
            Vector2::new(pos - 0.5 * width, 0.0),
            Vector2::new(pos + 0.5 * width, size.y),
            color
        );
    }
}

pub struct NoteView {
    pub notes: Vec<super::Note>,
    pub view: (Vector2<f32>, Vector2<f32>),
    pub measure_ticks: u16,
    pub style: Style,
}

impl Draw for NoteView {
    fn draw<R: Render>(&self, size: Vector2<f32>, renderer: &mut R) {
        use cgmath::ElementWise;

        let aspect = size.div_element_wise(self.view.1 - self.view.0);
        let brick_width = 1.4 * aspect.y;
        let color = self.style.orange();
        let border_color = self.style.base2();
        let border_width = 1.0;

        for note in &self.notes {
            let start = Vector2::new(
                note.time.0 as f32 / self.measure_ticks as f32,
                note.pitch as f32
            );
            let end = Vector2::new(
                note.time.1 as f32 / self.measure_ticks as f32,
                note.pitch as f32
            );

            let v0 = (start - self.view.0).mul_element_wise(aspect) - Vector2::new(0.0, brick_width / 2.0);
            let v1 = (end - self.view.0).mul_element_wise(aspect) + Vector2::new(0.0, brick_width / 2.0);

            //println!("{:?} : {:?}", v0, v1);

            let delta: Vector2<f32> = [border_width / 2.0; 2].into();

            renderer.render_rect(v0, v1, border_color);
            renderer.render_rect(v0 + delta, v1 - delta, color);
        }
    }
}
