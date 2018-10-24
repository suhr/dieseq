#![allow(dead_code)]

#[macro_use] extern crate gfx;
#[macro_use] extern crate serde_derive;

extern crate gfx_device_gl;
extern crate gfx_window_glutin;
extern crate glutin;

extern crate cgmath;
extern crate palette;
extern crate clap;

extern crate serde;
extern crate ron;

use glutin::ModifiersState;
use std::time::Instant;
use gfx::Device;
use gfx_window_glutin::init as gfx_init;
use cgmath::{Vector2, ElementWise};

use renderer::{ColorFormat, DepthFormat};

mod renderer;
mod ui;

fn duration_seconds(duration: ::std::time::Duration) -> f32 {
    let int = duration.as_secs() as f32;
    let frac = duration.subsec_nanos() as f32 * 1e-9;

    int + frac
}

fn min_max<T: PartialOrd>(v0: T, v1: T) -> (T, T) {
    if v0 > v1 { (v1, v0) }
    else { (v0, v1) }
}

fn normalize_square(a0: Vector2<f32>, a1: Vector2<f32>) -> (Vector2<f32>, Vector2<f32>) {
    let (x0, x1) = min_max(a0.x, a1.x);
    let (y0, y1) = min_max(a0.y, a1.y);

    (
        [x0, y0].into(),
        [x1, y1].into(),
    )
}

fn rects_overlap(a0: Vector2<f32>, a1: Vector2<f32>, b0: Vector2<f32>, b1: Vector2<f32>) -> bool {
    a0.x < b1.x && a1.x > b0.x &&
    a0.y < b1.y && a1.y > b0.y
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub channel: u16,
    pub time: (i16, i16),
    pub pitch: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        modifiers: ModifiersState,
        delta: (f32, f32),
    },
    LeftPressed {
        position: Vector2<f32>,
    },
    LeftReleased {
        position: Vector2<f32>,
    },
    LeftDrag {
        position: Vector2<f32>,
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
            MouseWheel {delta, modifiers, ..} => {
                if let glutin::MouseScrollDelta::LineDelta(x, y) = delta {
                    self.mailbox.push(Msg::MouseWheel {
                        position: self.mouse_pos,
                        modifiers,
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
                let position = Vector2::new(position.x as f32, self.screen_size.y - position.y as f32);

                if let Some(instant) = self.lbutton_pressed {
                    if instant.elapsed() >= std::time::Duration::from_millis(50) {
                        self.mailbox.push(Msg::LeftDrag {
                            position,
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
            Resized(sz) => {
                self.screen_size = [sz.width as f32, sz.height as f32].into();
                self.mailbox.push(Msg::WindowEvent(Resized(sz)))
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

#[derive(Debug, Clone, PartialEq)]
enum State {
    Idle,
    Drawing(Brick),
    Playing(f32, i16),
    PointSelected(Vector2<f32>),
    NotesSelected(Vec<Note>),
    SelectFrame(Vector2<f32>, Vector2<f32>),
    MovingNotes(),
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
    Save,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Project {
    score: Score,
    grid: ui::Grid,
    play_pos: f32,
}

#[derive(Debug)]
struct Model {
    med: ::std::process::Child,
    file: Option<::std::path::PathBuf>,
    state: State,
    tool: Tool,
    grid: ui::Grid,
    play_pos: f32,
    score: Score,
    commands: Vec<Command>,
}

impl Model {
    fn new() -> Self {
        Self::with_file(None)
    }

    fn with_file(path: Option<::std::path::PathBuf>) -> Self {
        use std::process::{Command, Stdio};
        use std::io::Write;

        let score = Score::new();

        let grid = ui::Grid::new(
            Vector2::new(1024.0, 768.0),
            (Vector2::new(-0.25, 31.0), Vector2::new(12.0, 155.0))
        );

        let mut med = Command::new("med")
            .args(&["--pipe"])
            .stdin(Stdio::piped())
            .spawn().unwrap();

        {
            let stdin = med.stdin.as_mut().unwrap();
            stdin.write_all(format!("31edo\n").as_bytes()).unwrap();
        }

        Model {
            med,
            file: path,
            state: State::Idle,
            tool: Tool::Pencil,
            grid,
            play_pos: 0.0,
            score,
            commands: vec![],
        }
    }

    fn from_file(path: ::std::path::PathBuf) -> Self {
        let mut file = std::fs::File::open(&path).unwrap();
        let proj: Project = ron::de::from_reader(&mut file).unwrap();

        Model {
            file: Some(path.clone()),
            score: proj.score,
            grid: proj.grid,
            play_pos: proj.play_pos,
            ..Self::with_file(Some(path))
        }
    }
}

fn model(mut model: Model, msg: Msg) -> Model {
    use glutin::WindowEvent::*;
    use Msg::*;

    match msg {
        Msg::MouseWheel { position, modifiers, delta: (_, y) } => {
            if modifiers.ctrl {
                let start = model.grid.view.0.y;
                let end = model.grid.view.1.y;
                let y = -y;

                let split_ratio = position.y / model.grid.size.y;
                let split_point = start + (end - start) * split_ratio;

                let k =
                    if y < 0.0 || (end - start) <= 8.0 * 31.0 {
                        (1.07_f32).powf(y)
                    }
                    else {
                        1.0
                    };

                model.grid.view.0.y = (start - split_point) * k + split_point;
                model.grid.view.1.y = (end - split_point) * k + split_point;
            }
            else {
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
            }
        },
        Msg::LeftPressed { position } if model.tool == Tool::Arrow => {
            model.state = State::PointSelected(position);
        },
        Msg::LeftPressed { position } if model.tool == Tool::Pencil => {
            let view_pos = model.grid.view_position(position);

            let time = (view_pos.x * model.score.measure_ticks as f32 / 2.0).round() * 2.0;
            let pitch = view_pos.y;

            model.state = State::Drawing(Brick {
                time: (time, time),
                pitch,
            });
        },
        Msg::LeftReleased { .. } => {
            if let State::PointSelected(point) = model.state {
                let time = model.grid.view_position(point).x;

                model.play_pos = time;
                model.state = State::Idle;
            }
            if let State::SelectFrame(v0, v1) = model.state {
                let (v0, v1) = normalize_square(
                    model.grid.view_position(v0),
                    model.grid.view_position(v1)
                );

                let ticks = { model.score.measure_ticks as f32 };
                let framed: Vec<Note> = model.score.notes.iter().filter(|n| {
                    let n0 = Vector2::new(n.time.0 as f32 / ticks, n.pitch as f32 - 0.5);
                    let n1 = Vector2::new(n.time.1 as f32 / ticks, n.pitch as f32 + 0.5);

                    rects_overlap(v0, v1, n0, n1)
                }).cloned().collect();

                if framed.len() == 0 {
                    model.state = State::Idle
                }
                else {
                    model.state = State::NotesSelected(framed)
                }
            };
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
        Msg::LeftDrag { position, .. } => {
            let view_pos = model.grid.view_position(position);

            if let State::Drawing(brick) = model.state {
                let brick = Brick {
                    time: (brick.time.0, view_pos.x * model.score.measure_ticks as f32),
                    pitch: view_pos.y,
                };

                model.state = State::Drawing(brick)
            }

            if let State::PointSelected(point) = model.state {
                model.state = State::SelectFrame(point, position)
            }

            if let State::SelectFrame(start, _) = model.state {
                model.state = State::SelectFrame(start, position)
            }
        },
        Msg::RightDrag { vector } => {
            let shift = -model.grid.view_vector(vector);

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
                let pos = model.play_pos + duration_seconds(t);
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
            match (input.scancode, &model.state.clone()) {
                (0x02, _) => {
                    model.tool = Tool::Arrow;
                },
                (0x03, _) => {
                    model.tool = Tool::Pencil;
                },
                (0x39, &State::Playing(_, _)) => {
                    model.commands.push(Command::Stop);

                    model.state = State::Idle
                },
                (0x39, _) =>
                    model.state = State::Playing(
                        model.play_pos,
                        (model.play_pos * model.score.measure_ticks as f32).round() as i16 - 1
                    ),
                (0x20, &State::NotesSelected(ref selected)) => {
                    model.score.notes.retain(|n| !selected.contains(n));

                    model.state = State::Idle;
                },
                (0x1f, _) => {
                    model.commands.push(Command::Save)
                },
                //(code, _) => { println!("{:x}", code); },
                _ => (),
            }
        },
        WindowEvent(Resized(sz)) =>
            model.grid.size = Vector2::new(sz.width as f32, sz.height as f32),
        _ => (),
    }

    model
}

fn draw(model: &Model, screen_size: [f32; 2], renderer: &mut renderer::Renderer, scene: &mut renderer::Scene) {
    renderer.clear(model.grid.style.base3());
    scene.clear();

    model.grid.draw(screen_size.into(), scene);

    let mut notes = model.score.notes.clone();

    if let State::Drawing(brick) = model.state {
        notes.push(brick.into())
    }

    if let State::NotesSelected(ref framed) = model.state {
        notes.retain(|n| !framed.contains(n));

        ui::NoteView {
            notes: framed.clone(),
            measure_ticks: model.score.measure_ticks,
            style: model.grid.style,
            view: model.grid.view,
            selected: true,
        }.draw(screen_size.into(), scene)
    }

    if let State::SelectFrame(v0, v1) = model.state {
        let (from, to) = normalize_square(v0, v1);
        ui::Frame {
            from, to,
            style: model.grid.style,
        }.draw(screen_size.into(), scene)
    }

    ui::NoteView {
        notes,
        measure_ticks: model.score.measure_ticks,
        style: model.grid.style,
        view: model.grid.view,
        selected: false,
    }
    .draw(screen_size.into(), scene);

    let play_pos =
        if let State::Playing(pos, _) = model.state { pos }
        else { model.play_pos };
    let play_pos = (play_pos - model.grid.view.0.x) / (model.grid.view.1.x - model.grid.view.0.x);
    ui::PlayBar {
        position: play_pos,
        style: model.grid.style,
    }.draw(screen_size.into(), scene);
}

struct MainState {

}

impl MainState {

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
            use std::io::Write;
            let stdin = model.med.stdin.as_mut().unwrap();

            match c {
                Command::NoteOn(n) => {
                    match n.pitch / 31 {
                        0 => drop(stdin.write_all(format!("0a{}_+\n", n.pitch % 31).as_bytes())),
                        1 => drop(stdin.write_all(format!("0b{}_+\n", n.pitch % 31).as_bytes())),
                        2 => drop(stdin.write_all(format!("0c{}_+\n", n.pitch % 31).as_bytes())),
                        3 => drop(stdin.write_all(format!("0d{}_+\n", n.pitch % 31).as_bytes())),
                        4 => drop(stdin.write_all(format!("0e{}_+\n", n.pitch % 31).as_bytes())),
                        5 => drop(stdin.write_all(format!("0f{}_+\n", n.pitch % 31).as_bytes())),
                        6 => drop(stdin.write_all(format!("0g{}_+\n", n.pitch % 31).as_bytes())),
                        7 => drop(stdin.write_all(format!("0h{}_+\n", n.pitch % 31).as_bytes())),
                        _ => (),
                    }
                },
                Command::NoteOff(n) => {
                    match n.pitch / 31 {
                        0 => drop(stdin.write_all(format!("0a{}-\n", n.pitch % 31).as_bytes())),
                        1 => drop(stdin.write_all(format!("0b{}-\n", n.pitch % 31).as_bytes())),
                        2 => drop(stdin.write_all(format!("0c{}-\n", n.pitch % 31).as_bytes())),
                        3 => drop(stdin.write_all(format!("0d{}-\n", n.pitch % 31).as_bytes())),
                        4 => drop(stdin.write_all(format!("0e{}-\n", n.pitch % 31).as_bytes())),
                        5 => drop(stdin.write_all(format!("0f{}-\n", n.pitch % 31).as_bytes())),
                        6 => drop(stdin.write_all(format!("0g{}-\n", n.pitch % 31).as_bytes())),
                        7 => drop(stdin.write_all(format!("0h{}-\n", n.pitch % 31).as_bytes())),
                        _ => (),
                    }
                },
                Command::Stop => {
                    drop(stdin.write_all(format!("s\n").as_bytes()))
                },
                Command::Save => {
                    if let Some(ref path) = model.file {
                        let proj = Project {
                            score: model.score.clone(),
                            grid: model.grid.clone(),
                            play_pos: model.play_pos,
                        };

                        let output = ron::ser::to_string(&proj).unwrap();

                        let mut file = ::std::fs::File::create(path).unwrap();
                        drop(file.write_all(output.as_bytes()));
                    }
                },
            }
        }
    }
}

pub fn main() {
    let matches = clap::App::new("Dieseq")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A mictotonal sequencer")
        .arg(
            clap::Arg::with_name("file")
            .help("Dieseq project file")
            .index(1)
        )
        .get_matches();

    let file = matches.value_of("file");

    use glutin::GlContext;

    let mut events_loop = glutin::EventsLoop::new();

    let builder = glutin::WindowBuilder::new()
        .with_title("Dieseq".to_string())
        .with_dimensions((1024, 768).into());
    let context = glutin::ContextBuilder::new()
        .with_multisampling(8)
        .with_vsync(true);
    let (window, mut device, mut factory, main_color, mut main_depth) =
        gfx_init::<ColorFormat, DepthFormat>(builder, context, &events_loop);

    let encoder: gfx::Encoder<_, _> = factory.create_command_buffer().into();
    let mut renderer = renderer::Renderer::new(factory, encoder, main_color);
    let mut scene = renderer::Scene::new();

    let mut backend = Backend::new();
    let mut intent = Intent::new();
    let mut the_model =
        if let Some(path) = file {
            let path = ::std::path::Path::new(path);

            if path.is_file() {
                Model::from_file(path.to_owned())
            }
            else if !path.exists() {
                Model::with_file(Some(path.to_owned()))
            }
            else {
                eprintln!("Invalid file name: {}", path.to_string_lossy());
                return
            }
        }
        else {
            Model::new()
        };

    let mut running = true;
    let mut screen_size = [1024.0, 768.0];
    while running {
        events_loop.poll_events(|ev| {
            use glutin::WindowEvent::*;
            if let glutin::Event::WindowEvent {event, ..} = ev {
                match event {
                    CloseRequested =>
                        running = false,
                    Resized(sz) => {
                        screen_size = [sz.width as f32, sz.height as f32];
                        intent.intent(Resized(sz));
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

        draw(&the_model, screen_size, &mut renderer, &mut scene);

        renderer.render_scene(&scene, screen_size, &mut device);
        window.swap_buffers().unwrap();
        device.cleanup();

        backend.run(&mut the_model);
        ::std::thread::yield_now()
        // let dt = ::std::time::Duration::from_millis(8);
        // ::std::thread::sleep(dt)
    }
}
