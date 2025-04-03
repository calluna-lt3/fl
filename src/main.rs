use std::process::exit;
use std::path::PathBuf;
use std::fs::{metadata, read_dir};
use std::io::{stdin, stdout, Write};
use std::vec;
use crossterm::{execute, cursor, queue, ExecutableCommand};
use crossterm::event::{self, Event, KeyEvent, KeyCode, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType};

// Values assigned for cmp
#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
enum FileType {
    Directory = 0,
    Regular   = 1,
    Symlink   = 2,
    Unknown   = 3,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct FileEntry {
    name: String,
    kind: FileType,
}

impl FileEntry {
    fn new(path: &PathBuf) -> Self {
        let name = path.to_str().expect("Filename should be valid unicode")[2..].to_string();
        let md = metadata(&path).unwrap();
        let kind = if md.is_file() {
            FileType::Regular
        } else if md.is_dir() {
            FileType::Directory
        } else if md.is_symlink() {
            FileType::Symlink
        } else {
            FileType::Unknown
        };

        Self { name, kind }
    }
}

enum RenderKind {
    Alphabetical,
    FileKind,
    Both,
}

#[derive(Clone)]
struct Directory<'a> {
    files: Vec<FileEntry>,
    display: Vec<&'a FileEntry>,
    len: u16,
    index: u16, // this index is into display, not into files
}

impl<'a> Directory<'a> {
    fn from(path: &PathBuf) -> Self {
        let files = read_dir(&path).unwrap();
        let files: Vec<FileEntry> = files
            .map(|file| {
                FileEntry::new(&file.unwrap().path())
            })
        .collect();

        let size = files.len().try_into().unwrap();
        Self {
            files,
            display: vec![],
            len: size,
            index: 0,
        }
    }

}

struct Buffer {
    pane: Pane,
    mode: Mode,
    input: Vec<char>,
    w: u16,
    h: u16,
}

impl Buffer {
    fn from(pane: &Pane) -> Self {
        let mode = Mode::Browse;
        let pane = pane.clone();
        let (w, h) = terminal::size().unwrap();
        Self {
            pane,
            mode,
            input: vec![],
            w,
            h,
        }
    }

    // i should just write my own basic readline library
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
                        if self.pane.directory.index < self.pane.directory.len - 1 {
                            execute!(stdout, cursor::MoveDown(1)).unwrap();
                            self.pane.directory.index += 1;
                        }
                    },
                    'k' => {
                        if self.pane.directory.index > 0 {
                            execute!(stdout, cursor::MoveUp(1)).unwrap();
                            self.pane.directory.index -= 1;
                        }
                    },
                    'h' => {},
                    'l' => {},

                    // sort (cycles directory/alphabetical)
                    's' => {
                    },
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
                        self.input.push(key);
                        write!(stdout, "{}", key).unwrap();
                    },
                },
                KeyCode::Backspace => {
                    self.input.pop();
                    write!(stdout, "\r").unwrap();
                },
                KeyCode::Esc => {
                    self.mode = Mode::Browse;
                    let x: u16 = self.pane.x;
                    let y: u16 = self.pane.y + self.pane.directory.index;
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

#[derive(Clone)]
struct Pane {
    directory: Directory<'a>,
    x: u16,
    y: u16,
    w: usize,
    h: usize,
}

impl Pane {
    fn from(directory: &Directory) -> Self {
        let directory: Directory = directory.clone();
        Self {
            directory,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        }
    }

    fn render_dir(&self, kind: RenderKind) {
        let mut stdout = stdout();

        // clear terminal
        queue!(stdout, Clear(ClearType::All), cursor::MoveTo(0, 0)).unwrap();
        self.directory.files.iter().for_each(|entry| self.directory.display.push(entry));

        match kind {
            RenderKind::Alphabetical => self.directory.display.sort_by(|a, b| (*a).name.cmp(&b.name)),
            RenderKind::FileKind     => self.directory.display.sort_by(|a, b| (*a).kind.cmp(&b.kind)),
            RenderKind::Both         => todo!("render by both"),
        }

        // Would be cool to sort by directories + names, or just either

        // lets do a sort !!
        let mut count = 0;
        for file in self.directory.display {
            let suffix = if file.kind == FileType::Directory {
                "/"
            } else if file.kind == FileType::Symlink {
                "@"
            } else {
                ""
            };

            stdout.execute(cursor::MoveTo(0, count)).unwrap();
            write!(stdout, "{name}{suffix}\n", name = file.name).unwrap();
            count += 1;
        }
    }
}

enum Mode {
    Command,
    Browse,
}

fn sigint() {
    terminal::disable_raw_mode().unwrap();
    execute!(stdout(), terminal::LeaveAlternateScreen).unwrap();
    exit(1);
}

fn main() {
    let mut stdout = stdout();
    let _stdin = stdin();

    let path = PathBuf::from(".");
    let directory = Directory::from(&path);
    let pane = Pane::from(&directory);
    let mut buffer = Buffer::from(&pane);


    execute!(stdout, terminal::EnterAlternateScreen).unwrap();
    terminal::enable_raw_mode().unwrap();

    buffer.pane.render_dir(RenderKind::FileKind);
    stdout.execute(cursor::MoveTo(0, 0)).unwrap();

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
