use ::gfx;
use ::glutin;

use gfx::handle::{RenderTargetView, DepthStencilView};
use gfx::traits::{Factory, FactoryExt};
use gfx::{Encoder, PipelineState};
use gfx_device_gl as gl;
use gfx_window_glutin as gfx_glutin;

use ::cgmath::Vector2;

pub type ColorFormat = gfx::format::Rgba8;
pub type DepthFormat = gfx::format::DepthStencil;

const BLACK: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}

impl Mesh {
    pub fn new() -> Self {
        Mesh {
            vertices: vec![],
            indices: vec![],
        }
    }
    pub fn add_rect(&mut self, a0: Vector2<f32>, a1: Vector2<f32>, color: [f32; 4]) {
        let i0 = self.vertices.len() as u16;
        let vs = [[a0.x, a0.y], [a0.x, a1.y], [a1.x, a1.y], [a1.x, a0.y]];
        self.vertices.extend(vs.into_iter().map(|p| Vertex {
            pos: *p,
            color: color,
        }));
        self.indices.extend(&[i0, i0+1, i0+2, i0+2, i0+3, i0]);
    }

    pub fn add_fan<V>(&mut self, iter: V)
    where V: ::std::iter::IntoIterator<Item=Vertex> {
        let i0 = self.vertices.len() as u16;
        let mut vs = iter.into_iter();
        self.vertices.push(vs.next().unwrap());
        self.vertices.push(vs.next().unwrap());
        for (i, v) in vs.enumerate() {
            let i = i as u16 + 1;
            self.vertices.push(v);
            self.indices.extend(&[i0, i0+i, i0+i+1]);
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }
}

pub enum Object {
    Mesh(Mesh),
}

pub struct Scene {
    objs: Vec<Mesh>,
}

impl Scene {
    pub fn new() -> Self {
        Scene {
            objs: vec![],
        }
    }
    pub fn add_mesh(&mut self, mesh: Mesh) {
        self.objs.push(mesh)
    }

    pub fn clear(&mut self) {
        self.objs.clear()
    }
}

gfx_defines! {
    vertex Vertex {
        pos: [f32; 2] = "a_Pos",
        color: [f32; 4] = "a_Color",
    }

    pipeline pipe {
        screen: gfx::Global<[f32; 2]> = "i_Screen",
        vbuf: gfx::VertexBuffer<Vertex> = (),
        out: gfx::RenderTarget<ColorFormat> = "Target0",
    }
}

pub trait Render {
    fn render_fan<V>(&mut self, iter: V)
    where V: ::std::iter::IntoIterator<Item=Vertex>;

    fn render_rect(&mut self, a0: Vector2<f32>, a1: Vector2<f32>, color: [f32; 4]);
    fn clear(&mut self, color: [f32; 4]);
}

pub struct Renderer {
    factory: gl::Factory,
    encoder: Encoder<gl::Resources, gl::CommandBuffer>,
    out_color: RenderTargetView<gl::Resources, ColorFormat>,
    pso: PipelineState<gl::Resources, pipe::Meta>,
}

impl Renderer {
    pub fn new(
        mut factory: gl::Factory,
        encoder: Encoder<gl::Resources, gl::CommandBuffer>,
        out_color: RenderTargetView<gl::Resources, ColorFormat>
    ) -> Self {
        use gfx::state::{Rasterizer, MultiSample};

        let vs = factory.create_shader_vertex(
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/plain_150.glslv"))
        ).unwrap();
        let ps = factory.create_shader_pixel(
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/plain_150.glslf"))
        ).unwrap();

        let pso = factory.create_pipeline_state(
            &gfx::ShaderSet::Simple(vs, ps),
            gfx::Primitive::TriangleList,
            Rasterizer {
                samples: Some(MultiSample),
                ..Rasterizer::new_fill()
            },
            pipe::new(),
        ).expect("Failed to create a PSO");

        Renderer {
            factory, encoder, pso, out_color,
        }
    }
    pub fn render_scene(&mut self, scene: &Scene, screen_size: [f32; 2], device: &mut gl::Device) {
        for m in scene.objs.iter() {
            let (vbuf, sl) =
            self.factory.create_vertex_buffer_with_slice(&m.vertices, &*m.indices);

            let data = pipe::Data {
                screen: screen_size,
                vbuf,
                out: self.out_color.clone(),
            };

            self.encoder.draw(&sl, &self.pso, &data);
        }

        self.encoder.flush(device);
    }
    pub fn update_views(&mut self, window: &glutin::GlWindow, depth: &mut DepthStencilView<gl::Resources, DepthFormat>) {
        gfx_glutin::update_views(&window, &mut self.out_color, depth)
    }

    pub fn clear(&mut self, color: [f32; 4]) {
        self.encoder.clear(&mut self.out_color, color)
    }
}

pub struct OldRenderer {
    factory: gl::Factory,
    encoder: Encoder<gl::Resources, gl::CommandBuffer>,
    out_color: RenderTargetView<gl::Resources, ColorFormat>,
    pso: PipelineState<gl::Resources, pipe::Meta>,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
}


impl OldRenderer {
    pub fn new(
        mut factory: gl::Factory,
        encoder: Encoder<gl::Resources, gl::CommandBuffer>,
        out_color: RenderTargetView<gl::Resources, ColorFormat>
    ) -> Self {
        use gfx::state::{Rasterizer, MultiSample};

        let vs = factory.create_shader_vertex(
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/plain_150.glslv"))
        ).unwrap();
        let ps = factory.create_shader_pixel(
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/plain_150.glslf"))
        ).unwrap();

        let pso = factory.create_pipeline_state(
            &gfx::ShaderSet::Simple(vs, ps),
            gfx::Primitive::TriangleList,
            Rasterizer {
                samples: Some(MultiSample),
                ..Rasterizer::new_fill()
            },
            pipe::new(),
        ).expect("Failed to create a PSO");

        OldRenderer {
            factory, encoder, pso, out_color,
            vertices: vec![],
            indices: vec![],
        }
    }

    pub fn update_views(&mut self, window: &glutin::GlWindow, depth: &mut DepthStencilView<gl::Resources, DepthFormat>) {
        gfx_glutin::update_views(&window, &mut self.out_color, depth)
    }

    pub fn draw(&mut self, screen_size: [f32; 2], device: &mut gl::Device) {
        let (vbuf, sl) =
            self.factory.create_vertex_buffer_with_slice(&self.vertices, &*self.indices);
        let data = pipe::Data {
            screen: screen_size,
            vbuf,
            out: self.out_color.clone(),
        };

        self.encoder.draw(&sl, &self.pso, &data);
        self.encoder.flush(device);

        self.vertices.clear();
        self.indices.clear();
    }
}

impl Render for OldRenderer {
    fn render_fan<V>(&mut self, iter: V)
    where V: ::std::iter::IntoIterator<Item=Vertex> {
        let i0 = self.vertices.len() as u16;
        let mut vs = iter.into_iter();
        self.vertices.push(vs.next().unwrap());
        self.vertices.push(vs.next().unwrap());
        for (i, v) in vs.enumerate() {
            let i = i as u16 + 1;
            self.vertices.push(v);
            self.indices.extend(&[i0, i0+i, i0+i+1]);
        }
    }

    fn render_rect(&mut self, a0: Vector2<f32>, a1: Vector2<f32>, color: [f32; 4]) {
        let i0 = self.vertices.len() as u16;
        let vs = [[a0.x, a0.y], [a0.x, a1.y], [a1.x, a1.y], [a1.x, a0.y]];
        self.vertices.extend(vs.into_iter().map(|p| Vertex {
            pos: *p,
            color: color,
        }));
        self.indices.extend(&[i0, i0+1, i0+2, i0+2, i0+3, i0]);
    }

    fn clear(&mut self, color: [f32; 4]) {
        self.encoder.clear(&mut self.out_color, color)
    }
}
