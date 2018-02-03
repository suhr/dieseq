#![allow(dead_code)]

#[macro_use] extern crate gfx;
extern crate gfx_device_gl;
extern crate gfx_window_glutin;
extern crate glutin;
extern crate cgmath;
extern crate palette;

extern crate portmidi;

use std::time::Instant;
use gfx::Device;
use gfx_window_glutin::init as gfx_init;
use cgmath::{Vector2, ElementWise};

use renderer::{ColorFormat, DepthFormat, Draw};

mod renderer;
mod ui;

fn duration_seconds(duration: ::std::time::Duration) -> f32 {
    let int = duration.as_secs() as f32;
    let frac = duration.subsec_nanos() as f32 * 1e-9;

    int + frac
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Note {
    pub channel: u16,
    pub time: (i16, i16),
    pub pitch: i16,
}

#[derive(Debug, Clone)]
struct Score {
    measure_ticks: u16,
    notes: Vec<Note>,
}

impl Score {
    pub fn new() -> Self {
        Score {
            measure_ticks: 16,
            notes: vec![],
        }
    }
}

#[derive(Debug, Clone)]
enum Msg {
    WindowEvent(glutin::WindowEvent),
    MouseWheel {
        position: Vector2<f32>,
        delta: (f32, f32),
    },
    LeftPressed {
        position: Vector2<f32>,
    },
    LeftReleased {
        position: Vector2<f32>,
    },
    LeftDrag {
        vector: Vector2<f32>,
    },
    RightDrag {
        vector: Vector2<f32>,
    },
    Time(std::time::Duration),
}

#[derive(Debug, Clone)]
struct Intent {
    mailbox: Vec<Msg>,
    screen_size: Vector2<f32>,
    mouse_pos: Vector2<f32>,
    lbutton_pressed: Option<std::time::Instant>,
    rbutton_pressed: Option<std::time::Instant>,
}

impl Intent {
    fn new() -> Self {
        Intent {
            mailbox: vec![],
            screen_size: [1024.0, 768.0].into(),
            mouse_pos: Vector2::new(0.0, 0.0),
            lbutton_pressed: None,
            rbutton_pressed: None,
        }
    }

    fn intent(&mut self, event: glutin::WindowEvent) {
        use glutin::WindowEvent::*;
        match event {
            MouseWheel {delta, ..} => {
                if let glutin::MouseScrollDelta::LineDelta(x, y) = delta {
                    self.mailbox.push(Msg::MouseWheel {
                        position: self.mouse_pos,
                        delta: (x, y),
                    })
                }
            },
            MouseInput { device_id, modifiers, button, state, ..} => {
                use glutin::{MouseButton as Mb, ElementState as Es};
                match (button, state) {
                    (Mb::Right, Es::Pressed) => {
                        self.rbutton_pressed = Some(std::time::Instant::now())
                    },
                    (Mb::Right, Es::Released) => {
                        self.rbutton_pressed = None
                    },
                    (Mb::Left, Es::Pressed) => {
                        self.lbutton_pressed = Some(std::time::Instant::now());
                        self.mailbox.push(Msg::LeftPressed {
                            position: self.mouse_pos,
                        })
                    },
                    (Mb::Left, Es::Released) => {
                        self.lbutton_pressed = None;
                        self.mailbox.push(Msg::LeftReleased {
                            position: self.mouse_pos,
                        })
                    },
                    _ => {},
                }

                self.mailbox.push(Msg::WindowEvent(
                    MouseInput { device_id, modifiers, button, state }
                ));

            },
            CursorMoved { position, ..} => {
                let position = Vector2::new(position.0 as f32, self.screen_size.y - position.1 as f32);

                if let Some(instant) = self.lbutton_pressed {
                    if instant.elapsed() >= std::time::Duration::from_millis(50) {
                        self.mailbox.push(Msg::LeftDrag {
                            vector: position - self.mouse_pos
                        });
                    }
                }
                if let Some(instant) = self.rbutton_pressed {
                    if instant.elapsed() >= std::time::Duration::from_millis(50) {
                        self.mailbox.push(Msg::RightDrag {
                            vector: position - self.mouse_pos
                        });
                    }
                }
                self.mouse_pos = position;
            },
            Resized(x, y) => {
                self.screen_size = [x as f32, y as f32].into();
                self.mailbox.push(Msg::WindowEvent(Resized(x, y)))
            },
            ev =>
                self.mailbox.push(Msg::WindowEvent(ev))
        }
    }

    fn messages(&mut self) -> Vec<Msg> {
        let mut mb = vec![];
        ::std::mem::swap(&mut mb, &mut self.mailbox);
        mb
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Brick {
    time: (f32, f32),
    pitch: f32,
}

impl From<Brick> for Note {
    fn from(brick: Brick) -> Self {
        let pitch = brick.pitch.round() as i16;

        let (t0, t1) = brick.time;
        let time =
            if t0 <= t1 {
                (t0.round() as i16, t1.round() as i16)
            } else {
                (t1.round() as i16, t0.round() as i16)
            };

        Note {
            channel: 0,
            time, pitch
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Idle,
    Drawing(Brick),
    Playing(f32, i16),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tool {
    Arrow,
    Pencil,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Command {
    NoteOn(Note),
    NoteOff(Note),
    Stop,
}

#[derive(Debug, Clone)]
struct Model {
    state: State,
    tool: Tool,
    grid: ui::Grid,
    play_pos: f32,
    score: Score,
    commands: Vec<Command>,
}

impl Model {
    fn new() -> Self {
        let score = Score::new();

        let grid = ui::Grid::new(
            Vector2::new(1024.0, 768.0),
            (Vector2::new(0.0, 31.0), Vector2::new(12.0, 155.0))
        );

        Model {
            state: State::Idle,
            tool: Tool::Pencil,
            grid,
            play_pos: 0.0,
            score,
            commands: vec![],
        }
    }
}

fn model(mut model: Model, msg: Msg) -> Model {
    use glutin::WindowEvent::*;
    use Msg::*;

    match msg {
        Msg::MouseWheel { position, delta: (_, y) } => {
            let start = model.grid.view.0.x;
            let end = model.grid.view.1.x;
            let y = -y;

            let k =
                if y < 0.0 || (end - start) <= model.grid.size.x / 16.0 {
                    (1.07_f32).powf(y)
                }
                else {
                    1.0
                };

            let split_ratio = position.x / model.grid.size.x;
            let split_point = start + (end - start) * split_ratio;

            model.grid.view.0.x = (start - split_point) * k + split_point;
            model.grid.view.1.x = (end - split_point) * k + split_point;

        },
        Msg::LeftPressed { position } if model.tool == Tool::Pencil => {
            let view_pos = model.grid.view.0 + model.grid.scale_vector(position);

            let time = view_pos.x * model.score.measure_ticks as f32;
            let pitch = view_pos.y;

            model.state = State::Drawing(Brick {
                time: (time, time),
                pitch,
            });
        },
        Msg::LeftReleased { .. } => {
            if let State::Drawing(brick) = model.state {
                if brick.time.0.round() != brick.time.1.round() {
                    model.score.notes.push(brick.into())
                }
                else if brick.time.0 == brick.time.1 {
                    let (time, pitch) = (brick.time.0, brick.pitch);

                    model.score.notes.retain(|n|
                        (n.pitch as f32 - pitch).abs() >= 0.5
                        || {
                            (n.time.0 as f32) > time
                            || (n.time.1 as f32) < time
                        }
                    )
                }

                model.state = State::Idle
            }
        },
        Msg::LeftDrag { vector } => {
            let shift = model.grid.scale_vector(vector);

            if let State::Drawing(brick) = model.state {
                let brick = Brick {
                    time: (brick.time.0, brick.time.1 + shift.x * model.score.measure_ticks as f32),
                    pitch: brick.pitch + shift.y,
                };

                model.state = State::Drawing(brick)
            }
        },
        Msg::RightDrag { vector } => {
            let shift = -model.grid.scale_vector(vector);

            let v0 = model.grid.view.0 + shift;
            let v1 = model.grid.view.1 + shift;

            if
                (shift.y > 0.0 && v1.y <= 31.0 * 8.0)
                || (shift.y < 0.0 && v0.y >= 0.0)
            {
                model.grid.view.0.y = v0.y;
                model.grid.view.1.y = v1.y;
            }

            model.grid.view.0.x = v0.x;
            model.grid.view.1.x = v1.x;
        },
        Msg::Time(t) => {
            if let State::Playing(_pos, mut ipos) = model.state {
                let pos = duration_seconds(t);
                let ticks = pos * model.score.measure_ticks as f32;

                if ticks as i16 > ipos {
                    ipos = ticks as i16;

                    for &n in &model.score.notes {
                        if n.time.0 == ipos {
                            model.commands.push(Command::NoteOn(n))
                        }

                        if n.time.1 == ipos {
                            model.commands.push(Command::NoteOff(n))
                        }
                    }
                }

                model.state = State::Playing(pos, ipos);
            }
        }
        WindowEvent(KeyboardInput { input, .. })
        if input.state == glutin::ElementState::Pressed => {
            match (input.scancode, model.state) {
                (0x39, State::Playing(_, _)) => {
                    model.commands.push(Command::Stop);

                    model.state = State::Idle
                },
                (0x39, _) =>
                    model.state = State::Playing(model.play_pos, model.play_pos.round() as i16),
                _ => {},
            }
        },
        WindowEvent(Resized(x, y)) =>
            model.grid.size = Vector2::new(x as f32, y as f32),
        _ => (),
    }

    model
}

fn draw(model: &Model, screen_size: [f32; 2], renderer: &mut renderer::Renderer) {
    use renderer::Render;
    renderer.clear(model.grid.style.base3());
    model.grid.draw(screen_size.into(), renderer);

    let mut notes = model.score.notes.clone();

    if let State::Drawing(brick) = model.state {
        notes.push(brick.into())
    }

    ui::NoteView {
        notes,
        measure_ticks: model.score.measure_ticks,
        style: model.grid.style,
        view: model.grid.view,
    }
    .draw(screen_size.into(), renderer);

    let play_pos =
        if let State::Playing(pos, _) = model.state { pos }
        else { model.play_pos };
    let play_pos = (play_pos - model.grid.view.0.x) / (model.grid.view.1.x - model.grid.view.0.x);
    ui::PlayBar {
        position: play_pos,
        style: model.grid.style,
    }.draw(screen_size.into(), renderer);
}

struct Backend {
    moment: Option<Instant>,
}

impl Backend {
    fn new() -> Self {
        Backend {
            moment: None,
        }
    }

    fn subscriptions(&mut self) -> Option<Msg> {
        if let Some(i) = self.moment {
            let msg = Msg::Time(i.elapsed());

            Some(msg)
        } else {
            None
        }
    }

    fn run(&mut self, model: &mut Model) {
        match model.state {
            State::Playing(_, _) =>
                if self.moment.is_none() {
                    self.moment = Some(Instant::now())
                },
            _ =>
                self.moment = None,
        };

        for c in model.commands.drain(..) {
            match c {
                Command::NoteOn(n) => {
                    match n.pitch / 31 {
                        0 => println!("0a{}_+", n.pitch % 31),
                        1 => println!("0b{}_+", n.pitch % 31),
                        2 => println!("0c{}_+", n.pitch % 31),
                        3 => println!("0d{}_+", n.pitch % 31),
                        4 => println!("0e{}_+", n.pitch % 31),
                        5 => println!("0f{}_+", n.pitch % 31),
                        6 => println!("0g{}_+", n.pitch % 31),
                        7 => println!("0h{}_+", n.pitch % 31),
                        _ => (),
                    }
                },
                Command::NoteOff(n) => {
                    match n.pitch / 31 {
                        0 => println!("0a{}-", n.pitch % 31),
                        1 => println!("0b{}-", n.pitch % 31),
                        2 => println!("0c{}-", n.pitch % 31),
                        3 => println!("0d{}-", n.pitch % 31),
                        4 => println!("0e{}-", n.pitch % 31),
                        5 => println!("0f{}-", n.pitch % 31),
                        6 => println!("0g{}-", n.pitch % 31),
                        7 => println!("0h{}-", n.pitch % 31),
                        _ => (),
                    }
                },
                Command::Stop => {
                    println!("s");
                },
            }
        }
    }
}

pub fn main() {
    println!("31edo");

    use glutin::GlContext;

    let mut events_loop = glutin::EventsLoop::new();

    let builder = glutin::WindowBuilder::new()
        .with_title("Dieseq".to_string())
        .with_dimensions(1024, 768);
    let context = glutin::ContextBuilder::new()
        .with_multisampling(8)
        .with_vsync(true);
    let (window, mut device, mut factory, main_color, mut main_depth) =
        gfx_init::<ColorFormat, DepthFormat>(builder, context, &events_loop);

    let encoder: gfx::Encoder<_, _> = factory.create_command_buffer().into();
    let mut renderer = renderer::Renderer::new(factory, encoder, main_color);

    let mut backend = Backend::new();
    let mut intent = Intent::new();
    let mut the_model = Model::new();

    let mut running = true;
    let mut screen_size = [1024.0, 768.0];
    while running {
        events_loop.poll_events(|ev| {
            use glutin::WindowEvent::*;
            if let glutin::Event::WindowEvent {event, ..} = ev {
                match event {
                    Closed =>
                        running = false,
                    Resized(w, h) => {
                        screen_size = [w as f32, h as f32];
                        intent.intent(Resized(w, h));
                        renderer.update_views(&window, &mut main_depth);
                    },
                    ev =>
                        intent.intent(ev),
                }
            }
        });

        for s in backend.subscriptions() {
            the_model = model(the_model, s);
        }

        for m in intent.messages() {
            the_model = model(the_model, m);
        }

        draw(&the_model, screen_size, &mut renderer);

        renderer.draw(screen_size, &mut device);
        window.swap_buffers().unwrap();
        device.cleanup();

        backend.run(&mut the_model);
        // let dt = ::std::time::Duration::from_millis(8);
        // ::std::thread::sleep(dt)
    }
}
