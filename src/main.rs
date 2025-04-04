use std::mem;
use std::fmt::{format, write, Display};
use std::process::exit;
use std::path::PathBuf;
use std::fs::{canonicalize, DirEntry};
use std::io::{stdin, stdout, Write};
use crossterm::{execute, queue, cursor};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType};


struct Buffer {
    pane: Pane,
    input_buffer: Vec<char>,
    mode: Mode,
    h: u16,
    w: u16,
}

impl Buffer {
    fn new() -> Self {
        let (w, h) = terminal::size().unwrap();
        Self {
            pane: Pane::new(),
            input_buffer: vec![],
            mode: Mode::Browse,
            w,
            h,
        }
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
                self.handle_browse_keypress(&event);
            },
            Mode::Command => {
                self.handle_command_keypress(&event);
            },
        }

    }

    fn handle_browse_keypress(&mut self, event: &KeyEvent) {
        let mut stdout = stdout();

        match event.modifiers {
            KeyModifiers::NONE => match event.code {
                KeyCode::Char(key) => match key {
                    // TODO: implement a local save/restore, as the global one provided by cursor
                    // doesnt work
                    ':' => {
                        self.mode = Mode::Command;
                        let mut x: u16 = self.input_buffer.len().try_into().unwrap();
                        if x > 0 { x += 1; }
                        queue!(stdout, cursor::MoveTo(x, self.h), cursor::SetCursorStyle::BlinkingBlock).unwrap();
                        if x == 0 { write!(stdout, ":").unwrap() }
                    }
                    'j' => {
                        match &mut self.pane.contents {
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
                        match &mut self.pane.contents {
                            Some(Contents::Directory(d)) => {
                                if d.index > 0 {
                                    execute!(stdout, cursor::MoveUp(1)).unwrap();
                                    d.index -= 1;
                                }
                            }
                            _ => {},
                        };
                    },
                    'h' => {},
                    'l' => {
                        if let Some(c) = &self.pane.contents {
                            match c {
                                Contents::Directory(d)=> {
                                    let index = d.index;
                                    let file = d.files.get(index).expect("ui should allow selecting oob entries");
                                    let md = file.file_type().unwrap();
                                    if md.is_dir()  || md.is_symlink() {
                                        self.pane.update_dir(&file.path());
                                        if let Some(c) = &self.pane.contents {
                                            c.render();
                                        }
                                    }
                                },
                            }
                        }
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

    fn handle_command_keypress(&mut self, event: &KeyEvent) {
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
                    self.mode = Mode::Browse;
                    let x = self.pane.x;
                    let mut y: u16 = self.pane.y;
                    if let Some(Contents::Directory(d)) = &self.pane.contents {
                        let offset: u16 = d.index.try_into().unwrap();
                        y = self.pane.y + offset;
                    }
                    queue!(stdout, cursor::MoveTo(x, y), cursor::SetCursorStyle::SteadyBlock).unwrap();
                },
                KeyCode::Enter => {
                    let command = self.input_buffer.iter().collect::<String>();
                    self.input_buffer.clear();
                    execute!(stdout, Clear(ClearType::CurrentLine)).unwrap();
                    write!(stdout, "\r:").unwrap();
                    self.process_command(command);
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
}

// Parts of the screen
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

    fn update_dir(&mut self, path: &PathBuf) {
        let files = path.read_dir().unwrap();
        let mut files: Vec<DirEntry> = files.into_iter().map(|file| file.unwrap()).collect();
        files.sort_by_key(|name| name.path());
        let len = files.len();

        let mut location: PathBuf = PathBuf::new();
        if let Some(&mut c) = self.contents {
            match c {
                Contents::Directory(&mut d) => {
                    d.location.push(path);
                    let location = mem::take(d.location);
                },
            }
        } else {
            exit_with(1, format!("ERROR: '{path}' is not a directory", path = path.display()));
        }

        self.contents = Some(Contents::Directory(Directory { files, location, len, index: 0 }));
    }
}

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
            location: PathBuf::from("."),
            len,
            index: 0,
        }
    }

    fn render(&self) {
        let mut stdout = stdout();
        queue!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0)).unwrap();

        for file in &self.files {
            let md = file.file_type().unwrap();
            let suffix = if md.is_dir() {
                "/"
            } else if md.is_symlink() {
                "@"
            } else {
                ""
            };
            execute!(stdout, cursor::MoveToColumn(0)).unwrap();
            // TODO: strip current directory's path from the displayed path
            write!(stdout, "{name}{suffix}\n", name = file.path().to_str().expect("should be unicode")[2..].to_string()).unwrap();
        }

        execute!(stdout, cursor::MoveTo(0, 0)).unwrap();
        stdout.flush().unwrap();
    }
}

enum Contents {
    Directory(Directory),
}

impl Contents {
    fn render(&self) {
        match self {
            Contents::Directory(c) => { c.render(); },
        };
    }
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
    exit(1);
}

fn main() {
    let mut stdout = stdout();

    execute!(stdout, terminal::EnterAlternateScreen).unwrap();
    terminal::enable_raw_mode().unwrap();

    // Make buffer, pane, directory
    let mut buffer = Buffer::new();
    let mut pane = Pane::new();
    let dir = Directory::from(&PathBuf::from("."));
    pane.contents = Some(Contents::Directory(dir));
    if let Some(c) = &pane.contents { c.render(); }
    buffer.pane = pane;

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
