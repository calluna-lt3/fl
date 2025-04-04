use std::process::exit;
use std::path::PathBuf;
use std::fs::DirEntry;
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
                    ':' => {
                        // use cursor::SavePosition here
                        self.mode = Mode::Command;
                        queue!(stdout, cursor::MoveTo(0, self.h), cursor::SetCursorStyle::BlinkingBlock).unwrap();
                        write!(stdout, ":").unwrap();
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
                    'l' => {},
                    _ => {},
                },
                _ => {},
            },

            KeyModifiers::CONTROL => match event.code {
                KeyCode::Char(key) => match key {
                    'c' => { sigint(); },
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
                    self.input_buffer.pop();
                    write!(stdout, "\r").unwrap();
                },
                KeyCode::Esc => {
                    self.mode = Mode::Browse;

                    let x = self.pane.x;
                    let mut y = self.pane.y;
                    if let Some(Contents::Directory(d)) = &self.pane.contents {
                        y = self.pane.y + d.index;
                    }
                    queue!(stdout, cursor::MoveTo(x, y), cursor::SetCursorStyle::SteadyBlock).unwrap();
                },
                _ => {},
            },

            KeyModifiers::CONTROL => match event.code {
                KeyCode::Char(key) => match key {
                    'c' => { sigint(); },
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
}

struct Directory {
    files: Vec<DirEntry>,
    len: u16,
    index: u16,
}

impl Directory {
    fn from(path: &PathBuf) -> Self {
        let files = path.read_dir().unwrap();
        let mut files: Vec<DirEntry> = files.into_iter().map(|file| file.unwrap()).collect();
        files.sort_by_key(|name| name.path());
        let len: u16 = files.len().try_into().expect("directory has less files than can fit in 16 bits");
        Self {
            files,
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

fn sigint() {
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
