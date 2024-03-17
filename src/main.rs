use std::io;
use std::thread;
use std::time::Instant;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    prelude::*,
    symbols::border,
    widgets::{block::*, *},
};
use rodio::source::SineWave;
use rodio::{OutputStream, Source};

mod tui;

fn main() -> io::Result<()> {
    let mut terminal = tui::init()?;
    let app_result = App::default().run(&mut terminal);
    tui::restore()?;
    app_result
}

#[derive(Debug)]
pub struct Solve {
    number: u8,
    time: f32,
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    Begin,
    Running,
    Finished,
}

#[derive(Debug)]
pub struct App {
    state: State,
    target: u8,
    counter: u8,
    total_begin: Instant,
    total_time: f32,
    current_begin: Instant,
    exit: bool,
    solves: Vec<Solve>,
}

impl Default for App {
    fn default() -> Self {
        App {
            state: State::Begin,
            target: 42,
            counter: 0,
            total_begin: Instant::now(),
            total_time: 0.0,
            current_begin: Instant::now(),
            exit: false,
            solves: vec![],
        }
    }
}

impl App {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut tui::Tui) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn render_frame(&self, frame: &mut Frame) {
        match self.state {
            State::Begin => frame.render_widget(self, frame.size()),
            State::Running => {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(frame.size());

                frame.render_widget(self, chunks[0]);
                render(self, chunks[1], frame.buffer_mut());
            }
            State::Finished => {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(frame.size());

                frame.render_widget(self, chunks[0]);
                render(self, chunks[1], frame.buffer_mut());
            }
        }
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self.state {
            State::Begin => match key_event.code {
                KeyCode::Char('q') => self.exit(),
                KeyCode::Right => self.increment_target(),
                KeyCode::Left => self.decrement_target(),
                KeyCode::Char(' ') => self.start_timing(),
                _ => {}
            },
            State::Running => match key_event.code {
                KeyCode::Char('q') => self.exit(),
                _ => self.solve_done(),
            },
            State::Finished => match key_event.code {
                KeyCode::Char('q') => self.exit(),
                KeyCode::Char('r') => self.reset(),
                _ => {}
            },
        }
    }

    fn start_timing(&mut self) {
        beep();
        self.state = State::Running;
        self.total_begin = Instant::now();
        self.current_begin = Instant::now();
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn increment_target(&mut self) {
        if self.target < 255 {
            self.target += 1;
        }
    }

    fn decrement_target(&mut self) {
        if self.target > 2 {
            self.target -= 1;
        }
    }

    fn solve_done(&mut self) {
        beep();
        let duration = self.current_begin.elapsed().as_secs_f32();
        if duration < 0.5 {
            return;
        }
        self.counter += 1;
        let duration = self.current_begin.elapsed().as_secs_f32();
        self.current_begin = Instant::now();
        self.solves.push(Solve {
            number: self.counter,
            time: duration,
        });

        if self.counter == self.target {
            let duration = self.total_begin.elapsed().as_secs_f32();
            self.total_time = duration;
            self.state = State::Finished;
        }
    }

    fn reset(&mut self) {
        self.state = State::Begin;
        self.counter = 0;
        self.solves = vec![];
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = match self.state {
            State::Begin => Title::from(" Cube Marathon Timer ".bold()),
            State::Running => Title::from(" Session in Progress! Times: "),
            State::Finished => Title::from(" Session done! Times: "),
        };
        let instructions = match self.state {
            State::Begin => Title::from(Line::from(vec![
                " Decrease ".into(),
                "<Left>".blue().bold(),
                " Increase ".into(),
                "<Right>".blue().bold(),
                " Begin ".into(),
                "<Space>".blue().bold(),
                " Quit ".into(),
                "<Q> ".blue().bold(),
            ])),
            State::Running => Title::from(Line::from(vec![
                " Current cube solved ".into(),
                "<Any key>".blue().bold(),
                " Quit ".into(),
                "<Q> ".blue().bold(),
            ])),
            State::Finished => Title::from(Line::from(vec![
                " Restart ".into(),
                "<R>".blue().bold(),
                " Quit ".into(),
                "<Q> ".blue().bold(),
            ])),
        };
        let block = Block::default()
            .title(title.alignment(Alignment::Center))
            .title(
                instructions
                    .alignment(Alignment::Center)
                    .position(Position::Bottom),
            )
            .borders(Borders::ALL)
            .border_set(border::THICK);

        let counter_text = match self.state {
            State::Begin => Text::from(vec![
                Line::from(vec![]),
                Line::from(vec![]),
                Line::from(vec![
                    "Cubes to solve: ".into(),
                    self.target.to_string().yellow(),
                ]),
            ]),
            State::Running => Text::from(
                self.solves
                    .iter()
                    .map(|solve| {
                        Line::from(vec![
                            "Cube ".into(),
                            solve.number.to_string().yellow(),
                            " solved in ".into(),
                            format!("{:2.2}", solve.time).yellow(),
                            " sec ".into(),
                        ])
                    })
                    .collect::<Vec<_>>(),
            ),
            State::Finished => Text::from(
                self.solves
                    .iter()
                    .map(|solve| {
                        Line::from(vec![
                            "Cube ".into(),
                            solve.number.to_string().yellow(),
                            " solved in ".into(),
                            format!("{:2.2}", solve.time).yellow(),
                            " sec ".into(),
                        ])
                    })
                    .collect::<Vec<_>>(),
            ),
        };

        if self.state == State::Begin {
            Paragraph::new(counter_text)
                .centered()
                .block(block)
                .render(area, buf);
        } else {
            Paragraph::new(counter_text).block(block).render(area, buf);
        }
    }
}

fn calculate_average(app: &App) -> Option<f32> {
    if app.counter == 0 {
        None
    } else {
        Some(app.solves.iter().map(|solve| solve.time).sum::<f32>() / app.counter as f32)
    }
}

fn calculate_fastest(app: &App) -> Option<f32> {
    app.solves
        .iter()
        .map(|solve| solve.time)
        .min_by(|a, b| a.partial_cmp(b).unwrap())
}

fn calculate_slowest(app: &App) -> Option<f32> {
    app.solves
        .iter()
        .map(|solve| solve.time)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
}

fn predict_total_time(app: &App) -> Option<f32> {
    let avg = calculate_average(app);
    avg.map(|a| a * app.target as f32)
}

fn predict_marathon_time(app: &App) -> f32 {
    calculate_average(app).unwrap() * 42.0
}

fn render(app: &App, area: Rect, buf: &mut Buffer) {
    let title = Title::from(" Statistics ".bold());
    let block = Block::default()
        .title(title.alignment(Alignment::Center))
        .borders(Borders::ALL)
        .border_set(border::THICK);

    let counter_text = match app.state {
        State::Begin => panic!(),
        State::Running => Text::from(vec![
            Line::from(vec![
                "Current cube: ".into(),
                (app.counter + 1).to_string().yellow(),
                " of ".into(),
                app.target.to_string().yellow(),
            ]),
            Line::from(vec![
                "Average: ".into(),
                calculate_average(&app)
                    .map(|avg| format!("{:2.2}", avg))
                    .unwrap_or("N/A".into())
                    .yellow(),
                " sec".into(),
            ]),
            Line::from(vec![
                "Fastest solve: ".into(),
                calculate_fastest(&app)
                    .map(|fast| format!("{:2.2}", fast))
                    .unwrap_or("N/A".into())
                    .yellow(),
                " sec".into(),
            ]),
            Line::from(vec![
                "Slowest solve: ".into(),
                calculate_slowest(&app)
                    .map(|slow| format!("{:2.2}", slow))
                    .unwrap_or("N/A".into())
                    .yellow(),
                " sec".into(),
            ]),
            Line::from(vec![
                "Total time so far: ".into(),
                time_to_string(app.total_begin.elapsed().as_secs_f32()).yellow(),
                " min".into(),
            ]),
            Line::from(vec![
                "Predicted total time: ".into(),
                predict_total_time(app)
                    .map(time_to_string)
                    .unwrap_or("N/A".into())
                    .yellow(),
                " min".into(),
            ]),
        ]),
        State::Finished => Text::from(vec![
            Line::from(vec![
                "Current cube: All ".into(),
                app.target.to_string().yellow(),
                " cubes solved!".into(),
            ]),
            Line::from(vec![
                "Average: ".into(),
                calculate_average(&app)
                    .map(|avg| format!("{:2.2}", avg))
                    .unwrap_or("N/A".into())
                    .yellow(),
                " sec".into(),
            ]),
            Line::from(vec![
                "Fastest solve: ".into(),
                calculate_fastest(&app)
                    .map(|fast| format!("{:2.2}", fast))
                    .unwrap_or("N/A".into())
                    .yellow(),
                " sec".into(),
            ]),
            Line::from(vec![
                "Slowest solve: ".into(),
                calculate_slowest(&app)
                    .map(|slow| format!("{:2.2}", slow))
                    .unwrap_or("N/A".into())
                    .yellow(),
                " sec".into(),
            ]),
            Line::from(vec![
                "Total time: ".into(),
                time_to_string(app.total_time).yellow(),
                " min".into(),
            ]),
            if app.target != 42 {
                Line::from(vec![
                    "Estimed marathon time: ".into(),
                    time_to_string(predict_marathon_time(&app)).yellow(),
                    " min".into(),
                ])
            } else {
                Line::from(vec![])
            },
        ]),
    };

    Paragraph::new(counter_text)
        //.centered()
        .block(block)
        .render(area, buf);
}

fn time_to_string(time_secs: f32) -> String {
    let mins = time_secs as u32 / 60;
    let secs = time_secs - mins as f32 * 60.0;
    format!("{}:{:2.2}", mins, secs)
}

fn beep() {
    thread::spawn(|| {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let source = SineWave::new(1000.0);
        stream_handle.play_raw(source.convert_samples()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
    });
}
