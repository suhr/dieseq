use ui;
use super::{Vector2, rects_overlap, normalize_square, duration_seconds};
use super::Msg;
use renderer;

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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub channel: u16,
    pub time: (i16, i16),
    pub pitch: i16,
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

pub struct PianoRoll {
    state: State,
    tool: Tool,
    grid: ui::Grid,
    play_pos: f32,
    score: Score,
}

impl PianoRoll {
    pub fn new() -> Self {
        let score = Score::new();

        let grid = ui::Grid::new(
            Vector2::new(1024.0, 768.0),
            (Vector2::new(-0.25, 31.0), Vector2::new(12.0, 155.0))
        );

        PianoRoll {
            state: State::Idle,
            tool: Tool::Arrow,
            play_pos: 0.0,
            score, grid
        }
    }

    pub fn model(&mut self, msg: Msg) {
        use glutin::WindowEvent::*;
        use Msg::*;

        match msg {
            Msg::MouseWheel { position, modifiers, delta: (_, y) } => {
                if modifiers.ctrl {
                    let start = self.grid.view.0.y;
                    let end = self.grid.view.1.y;
                    let y = -y;

                    let split_ratio = position.y / self.grid.size.y;
                    let split_point = start + (end - start) * split_ratio;

                    let k =
                        if y < 0.0 || (end - start) <= 8.0 * 31.0 {
                            (1.07_f32).powf(y)
                        }
                        else {
                            1.0
                        };

                    self.grid.view.0.y = (start - split_point) * k + split_point;
                    self.grid.view.1.y = (end - split_point) * k + split_point;
                }
                else {
                    let start = self.grid.view.0.x;
                    let end = self.grid.view.1.x;
                    let y = -y;

                    let k =
                        if y < 0.0 || (end - start) <= self.grid.size.x / 16.0 {
                            (1.07_f32).powf(y)
                        }
                        else {
                            1.0
                        };

                    let split_ratio = position.x / self.grid.size.x;
                    let split_point = start + (end - start) * split_ratio;

                    self.grid.view.0.x = (start - split_point) * k + split_point;
                    self.grid.view.1.x = (end - split_point) * k + split_point;
                }
            },
            Msg::LeftPressed { position } if self.tool == Tool::Arrow => {
                self.state = State::PointSelected(position);
            },
            Msg::LeftPressed { position } if self.tool == Tool::Pencil => {
                let view_pos = self.grid.view_position(position);

                let time = (view_pos.x * self.score.measure_ticks as f32 / 2.0).round() * 2.0;
                let pitch = view_pos.y;

                self.state = State::Drawing(Brick {
                    time: (time, time),
                    pitch,
                });
            },
            Msg::LeftReleased { .. } => {
                if let State::PointSelected(point) = self.state {
                    let time = self.grid.view_position(point).x;

                    self.play_pos = time;
                    self.state = State::Idle;
                }
                if let State::SelectFrame(v0, v1) = self.state {
                    let (v0, v1) = normalize_square(
                        self.grid.view_position(v0),
                        self.grid.view_position(v1)
                    );

                    let ticks = { self.score.measure_ticks as f32 };
                    let framed: Vec<Note> = self.score.notes.iter().filter(|n| {
                        let n0 = Vector2::new(n.time.0 as f32 / ticks, n.pitch as f32 - 0.5);
                        let n1 = Vector2::new(n.time.1 as f32 / ticks, n.pitch as f32 + 0.5);

                        rects_overlap(v0, v1, n0, n1)
                    }).cloned().collect();

                    if framed.len() == 0 {
                        self.state = State::Idle
                    }
                    else {
                        self.state = State::NotesSelected(framed)
                    }
                };
                if let State::Drawing(brick) = self.state {
                    if brick.time.0.round() != brick.time.1.round() {
                        self.score.notes.push(brick.into())
                    }
                    else if brick.time.0 == brick.time.1 {
                        let (time, pitch) = (brick.time.0, brick.pitch);

                        self.score.notes.retain(|n|
                            (n.pitch as f32 - pitch).abs() >= 0.5
                            || {
                                (n.time.0 as f32) > time
                                || (n.time.1 as f32) < time
                            }
                        )
                    }

                    self.state = State::Idle
                }
            },
            Msg::LeftDrag { position, .. } => {
                let view_pos = self.grid.view_position(position);

                if let State::Drawing(brick) = self.state {
                    let brick = Brick {
                        time: (brick.time.0, view_pos.x * self.score.measure_ticks as f32),
                        pitch: view_pos.y,
                    };

                    self.state = State::Drawing(brick)
                }

                if let State::PointSelected(point) = self.state {
                    self.state = State::SelectFrame(point, position)
                }

                if let State::SelectFrame(start, _) = self.state {
                    self.state = State::SelectFrame(start, position)
                }
            },
            Msg::RightDrag { vector } => {
                let shift = -self.grid.view_vector(vector);

                let v0 = self.grid.view.0 + shift;
                let v1 = self.grid.view.1 + shift;

                if
                    (shift.y > 0.0 && v1.y <= 31.0 * 8.0)
                    || (shift.y < 0.0 && v0.y >= 0.0)
                {
                    self.grid.view.0.y = v0.y;
                    self.grid.view.1.y = v1.y;
                }

                self.grid.view.0.x = v0.x;
                self.grid.view.1.x = v1.x;
            },
            Msg::Time(t) => {
                if let State::Playing(_pos, mut ipos) = self.state {
                    let pos = self.play_pos + duration_seconds(t);
                    let ticks = pos * self.score.measure_ticks as f32;

                    if ticks as i16 > ipos {
                        ipos = ticks as i16;

                        for &n in &self.score.notes {
                            if n.time.0 == ipos {
                                // self.commands.push(Command::NoteOn(n))
                            }

                            if n.time.1 == ipos {
                                // self.commands.push(Command::NoteOff(n))
                            }
                        }
                    }

                    self.state = State::Playing(pos, ipos);
                }
            }
            WindowEvent(KeyboardInput { input, .. })
            if input.state == glutin::ElementState::Pressed => {
                match (input.scancode, &self.state.clone()) {
                    (0x02, _) => {
                        self.tool = Tool::Arrow;
                    },
                    (0x03, _) => {
                        self.tool = Tool::Pencil;
                    },
                    (0x39, &State::Playing(_, _)) => {
                        // self.commands.push(Command::Stop);

                        self.state = State::Idle
                    },
                    (0x39, _) =>
                        self.state = State::Playing(
                            self.play_pos,
                            (self.play_pos * self.score.measure_ticks as f32).round() as i16 - 1
                        ),
                    (0x20, &State::NotesSelected(ref selected)) => {
                        self.score.notes.retain(|n| !selected.contains(n));

                        self.state = State::Idle;
                    },
                    (0x1f, _) => {
                        // self.commands.push(Command::Save)
                    },
                    //(code, _) => { println!("{:x}", code); },
                    _ => (),
                }
            },
            WindowEvent(Resized(sz)) =>
                self.grid.size = Vector2::new(sz.width as f32, sz.height as f32),
            _ => (),
        }
    }

    pub fn draw(&self, screen_size: [f32; 2], renderer: &mut renderer::Renderer, scene: &mut renderer::Scene) {
        self.grid.draw(screen_size.into(), scene);

        let mut notes = self.score.notes.clone();

        if let State::Drawing(brick) = self.state {
            notes.push(brick.into())
        }

        if let State::NotesSelected(ref framed) = self.state {
            notes.retain(|n| !framed.contains(n));

            ui::NoteView {
                notes: framed.clone(),
                measure_ticks: self.score.measure_ticks,
                style: self.grid.style,
                view: self.grid.view,
                selected: true,
            }.draw(screen_size.into(), scene)
        }

        if let State::SelectFrame(v0, v1) = self.state {
            let (from, to) = normalize_square(v0, v1);
            ui::Frame {
                from, to,
                style: self.grid.style,
            }.draw(screen_size.into(), scene)
        }

        ui::NoteView {
            notes,
            measure_ticks: self.score.measure_ticks,
            style: self.grid.style,
            view: self.grid.view,
            selected: false,
        }
        .draw(screen_size.into(), scene);

        let play_pos =
            if let State::Playing(pos, _) = self.state { pos }
            else { self.play_pos };
        let play_pos = (play_pos - self.grid.view.0.x) / (self.grid.view.1.x - self.grid.view.0.x);
        ui::PlayBar {
            position: play_pos,
            style: self.grid.style,
        }.draw(screen_size.into(), scene);
    }
}