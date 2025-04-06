// TODO: support symlink

use std::fmt::{format, Display};
use std::process::exit;
use std::path::PathBuf;
use std::fs::{DirEntry, File};
use std::io::{stdout, Write};
use crossterm::{execute, queue, cursor};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType};

const DEBUG_LOG: &'static str = "debug.log";

struct Buffer {
    panes: Vec<Pane>,
    center: usize,
    input_buffer: Vec<char>,
    mode: Mode,
    x: u16,
    y: u16,
    h: u16,
    w: u16,
}

// TODO: manually initialize first buffer
impl Buffer {
    fn new() -> Self {
        let (w, h) = terminal::size().unwrap();
        let x = w/3;
        let y = 0;
        let panes: Vec<Pane> = (0..3).into_iter().map(|i| {
            Pane {
                contents: None,
                x: i * x,
                y,
                w: x,
                h,
            }}).collect();
        let center = 1;
        Self {
            panes,
            center,
            input_buffer: vec![],
            mode: Mode::Browse,
            x,
            y,
            w,
            h,
        }
    }

    fn init(&mut self, path: &PathBuf) {
        let mut path = path.canonicalize().expect("please");
        self.mut_center().set_dir(&PathBuf::from(&path));
        path.pop();
        self.mut_left().set_dir(&PathBuf::from(&path));
        self.render();
    }

    // idk if this is idiomatic or not, but its easy ^^
    fn get_center(&self)     -> &Pane     { self.panes.get(self.center).expect("center pane always in bounds") }
    fn mut_center(&mut self) -> &mut Pane { self.panes.get_mut(self.center).expect("center pane always in bounds") }
    fn get_left(&self)       -> &Pane     { self.panes.get(self.center.checked_sub(1).unwrap_or(2)).expect("left pane always in bounds") }
    fn mut_left(&mut self)   -> &mut Pane { self.panes.get_mut(self.center.checked_sub(1).unwrap_or(2)).expect("left pane always in bounds") }
    fn get_right(&self)      -> &Pane     { self.panes.get((self.center + 1) % 3).expect("right pane always in bounds") }
    fn mut_right(&mut self)  -> &mut Pane { self.panes.get_mut((self.center + 1) % 3).expect("right pane always in bounds") }

    fn preview() {
        // TODO: render rightmost pane so that traversal is possible
    }

    fn traverse_up(&mut self) {
        let c_pane = self.mut_center();
        if let Some(c) = &c_pane.contents {
            match c {
                Contents::Directory(d)=> {
                    let file = d.files.get(d.index).expect("ui shouldnt allow selecting oob entries");
                    let md = file.file_type().unwrap();
                    if md.is_dir() || md.is_symlink() {
                        self.center = (self.center + 1) % 3;

                        // TODO: i hate thiss
                        self.mut_left().x   = 0;
                        self.mut_center().x = self.x;
                        self.mut_right().x  = self.x * 2;
                    }
                },
            }
        }
    }

    fn traverse_down(&mut self) {
        let mut path = match &self.get_left().contents {
            None => { return; },
            Some(c) => {
                match c {
                    Contents::Directory(d) => { PathBuf::from(&d.location) },
                }
            },
        };


        self.center = self.center.checked_sub(1).unwrap_or(2);

        if path.eq(&PathBuf::from("/")) {
            self.mut_left().contents = None;
        } else {
            path.pop();
            self.mut_left().set_dir(&path);
        }


        // TODO: i hate thiss
        self.mut_left().x   = 0;
        self.mut_center().x = self.x;
        self.mut_right().x  = self.x * 2;
    }

    fn browse(&mut self) {
        self.mode = Mode::Browse;
        let x = self.x;
        let mut y = self.y;
        let c_pane = self.mut_center();
        if let Some(Contents::Directory(d)) = &c_pane.contents {
            let offset: u16 = d.index.try_into().unwrap();
            y += offset;
        }
        queue!(stdout(), cursor::MoveTo(x, y), cursor::SetCursorStyle::SteadyBlock).unwrap();
    }

    fn command(&mut self) {
        let mut stdout = stdout();
        self.mode = Mode::Command;
        queue!(stdout, cursor::MoveTo(0, self.h), Clear(ClearType::CurrentLine), cursor::SetCursorStyle::BlinkingBlock).unwrap();
        write!(stdout, ":").unwrap();
    }

    fn process_command(&mut self, command: String) {
        match command.as_str() {
            "q"     => { exit_success(); },
            "quit"  => { exit_success(); },
            "find"  => { todo!("finding files"); },
            _ => {},
        }
    }

    fn handle_keypress(&mut self, event: &KeyEvent) {
        match self.mode {
            Mode::Browse => {
                self.browse_keypress(&event);
            },
            Mode::Command => {
                self.cmd_keypress(&event);
            },
        }

    }

    fn browse_keypress(&mut self, event: &KeyEvent) {
        let mut stdout = stdout();
        let c_pane = self.mut_center();

        match event.modifiers {
            KeyModifiers::NONE => match event.code {
                KeyCode::Char(key) => match key {
                    ':' => { self.command(); }
                    'q' => { exit_success(); }
                    'j' => {
                        match &mut c_pane.contents {
                            Some(Contents::Directory(d)) => {
                                if d.index < d.len - 1 {
                                    execute!(stdout, cursor::MoveDown(1)).unwrap();
                                    d.index += 1;
                                }
                            }
                            _ => {},
                        };
                    },
                    'k' => {
                        match &mut c_pane.contents {
                            Some(Contents::Directory(d)) => {
                                if d.index > 0 {
                                    execute!(stdout, cursor::MoveUp(1)).unwrap();
                                    d.index -= 1;
                                }
                            }
                            _ => {},
                        };
                    },
                    'h' => {
                        self.traverse_down();
                        self.render();
                    },
                    'l' => {
                        self.traverse_up();
                        self.render();
                    },
                    _ => {},
                },
                _ => {},
            },

            KeyModifiers::CONTROL => match event.code {
                KeyCode::Char(key) => match key {
                    'c' => { exit_with(1, "SIGINT"); },
                    _ => {},
                },
                _ => {},
            },
            _ => {},
        };


        stdout.flush().unwrap();
    }

    fn cmd_keypress(&mut self, event: &KeyEvent) {
        let mut stdout = stdout();

        match event.modifiers {
            KeyModifiers::NONE => match event.code {
                KeyCode::Char(key) => match key {
                    _ => {
                        self.input_buffer.push(key);
                        write!(stdout, "{}", key).unwrap();
                    },
                },
                KeyCode::Backspace => {
                    if self.input_buffer.len() > 0 {
                        self.input_buffer.pop();
                        execute!(stdout, Clear(ClearType::CurrentLine)).unwrap();
                        write!(stdout, "\r:{}", self.input_buffer.iter().collect::<String>()).unwrap();
                    }
                },
                KeyCode::Esc => {
                    execute!(stdout, Clear(ClearType::CurrentLine)).unwrap();
                    self.input_buffer.clear();
                    self.browse();
                },
                KeyCode::Enter => {
                    let command = self.input_buffer.iter().collect::<String>();
                    self.input_buffer.clear();
                    self.process_command(command);
                    // TODO: display result of command
                    self.browse();
                }
                _ => {},
            },

            KeyModifiers::CONTROL => match event.code {
                KeyCode::Char(key) => match key {
                    'c' => { exit_with(1, "SIGINT"); },
                    _ => {},
                },
                _ => {},
            },
            _ => {},
        };

        stdout.flush().unwrap();
    }

    fn render(&self) {
        execute!(stdout(), Clear(ClearType::All)).unwrap();
        self.panes.iter().for_each(|p| p.try_render());
        execute!(stdout(), cursor::MoveTo(self.x, self.y)).unwrap();
    }
}

#[derive(Debug)]
struct Pane {
    contents: Option<Contents>,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
}

impl Pane {
    fn new() -> Self {
        Self {
            contents: None,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        }
    }

    fn set_dir(&mut self, path: &PathBuf) {
        // TODO: handle no permission error
        let files = path.read_dir().unwrap();
        let mut files: Vec<DirEntry> = files.into_iter().map(|file| file.unwrap()).collect();
        files.sort_by_key(|name| name.path());
        let len = files.len();

        // NOTE: revisit this, i think this needs to be fixed
        let location = PathBuf::from(path);
        self.contents = Some(Contents::Directory(Directory { files, location, len, index: 0 }));
    }

    // TODO: do something on None, will be useful when trying to render unsupported types
    fn try_render(&self) {
        let mut stdout = stdout();
        execute!(stdout, cursor::MoveTo(self.x, self.y)).unwrap();

        if let Some(c) = &self.contents {
            match c {
                Contents::Directory(d) => {
                    for file in &d.files {
                        let md = file.file_type().unwrap();
                        let suffix = if md.is_dir() {
                            "/"
                        } else if md.is_symlink() {
                            "@"
                        } else {
                            ""
                        };

                        execute!(stdout, cursor::MoveToColumn(self.x)).unwrap();

                        // TODO: cutoff if output is too long
                        let location = d.location.as_path().to_str().expect("should be unicode").to_string() + "/";
                        write!(stdout, "{name}{suffix}\n", name = file.path().to_str().expect("should be unicode").replace(&location, "")).unwrap();
                    }

                },
            };
        }

        execute!(stdout, cursor::MoveTo(self.x, self.y)).unwrap();
        stdout.flush().unwrap();
    }
}

#[derive(Debug)]
struct Directory {
    files: Vec<DirEntry>,
    location: PathBuf,
    len: usize,
    index: usize,
}

impl Directory {
    fn from(path: &PathBuf) -> Self {
        let files = path.read_dir().unwrap();
        let mut files: Vec<DirEntry> = files.into_iter().map(|file| file.unwrap()).collect();
        files.sort_by_key(|name| name.path());
        let len = files.len();
        Self {
            files,
            location: PathBuf::from(&path),
            len,
            index: 0,
        }
    }

}

#[derive(Debug)]
enum Contents {
    Directory(Directory),
}

enum Mode {
    Command,
    Browse,
}

enum SortBy {
    Alphabetical,
    FileKind,
    Both,
}

fn log<T>(message: T)
where
    T: Display
{
    let mut file = File::options().append(true).create(true).open(DEBUG_LOG).unwrap();
    write!(file, "{}\n", message).unwrap();
}

fn exit_with<T>(code: i32, message: T)
where
    T: Display
{
    terminal::disable_raw_mode().unwrap();
    execute!(stdout(), terminal::LeaveAlternateScreen).unwrap();
    eprintln!("{message}");
    exit(code);
}

fn exit_success() {
    terminal::disable_raw_mode().unwrap();
    execute!(stdout(), terminal::LeaveAlternateScreen).unwrap();
    exit(0);
}

fn main() {
    let mut stdout = stdout();

    execute!(stdout, terminal::EnterAlternateScreen).unwrap();
    terminal::enable_raw_mode().unwrap();

    let mut buffer = Buffer::new();
    buffer.init(&PathBuf::from("."));

    execute!(stdout, cursor::SetCursorStyle::SteadyBlock).unwrap();
    stdout.flush().unwrap();

    loop {
        match event::read().unwrap() {
            Event::Key(e) => {
                buffer.handle_keypress(&e);
            },
            _ => break,
        };
    }
}
