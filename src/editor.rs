use std::io::Write;
use std::collections::HashMap;
use termion::*;
use termion::event::Key;
use termion::terminal_size;
use crate::text_buf::TextBuf;

pub enum State {
    Normal,
    Command,
    Insert,
}

pub struct Editor<'a> {
    running: bool,
    needs_redraw: bool,
    command: String,
    state: State,

    text_bufs: Vec<(TextBuf, usize)>,
    cur_text_buf: usize,

    command_map: HashMap<String, fn(&mut Editor, &[String]) -> Option<String>>,
    keybind_map: HashMap<event::Key, fn(&mut Editor, &[String]) -> Option<String>>,

    events: Box<dyn Iterator<Item=Result<event::Event, std::io::Error>> + 'a>,
    screen: Box<dyn Write + 'a>,
}

impl<'a> Editor<'a> {
    /// Init functions
    pub fn new(path: Option<String>) -> Self {
        use std::io::{stdin, stdout};
        use termion::input::TermRead;
        use termion::raw::IntoRawMode;

        let stdout = stdout().into_raw_mode().unwrap();
        let screen = screen::AlternateScreen::from(stdout);
        let events = stdin().events();

        let text_bufs = vec![
            (match path {
                Some(p) => TextBuf::from_file(&p),
                None => TextBuf::empty(),
            }, 0)
        ];

        Self {
            running: true,
            needs_redraw: true,
            state: State::Normal,
            command: String::new(),
            text_bufs,
            cur_text_buf: 0,
            command_map: HashMap::new(),
            keybind_map: HashMap::new(),
            events: Box::new(events),
            screen: Box::new(screen),
        }
    }

    pub fn default_environment(&mut self) -> &mut Self {
        self
        // Movement
            .add_keybind(Key::Left, left)
            .add_keybind(Key::Right, right)
            .add_keybind(Key::Up, up)
            .add_keybind(Key::Down, down)
            
            .add_keybind(Key::Char('h'), left)
            .add_keybind(Key::Char('l'), right)
            .add_keybind(Key::Char('k'), up)
            .add_keybind(Key::Char('j'), down)
        // Modes
            .add_keybind(Key::Char(':'), command_mode)
            .add_keybind(Key::Esc, normal_mode)
            .add_keybind(Key::Char('i'), insert_mode)
        // Misc
            .add_command("quit", quit)
            .add_command("q", quit)
            .add_command("write", write)
            .add_command("w", write)
            .add_command("wq", writequit);

        self
    }

    pub fn add_command(&mut self, command: &str, f: fn(&mut Editor, &[String]) -> Option<String>) -> &mut Self {
        self.command_map.insert(command.to_string(), f);

        self
    }
    
    pub fn add_keybind(&mut self, key: event::Key, f: fn(&mut Editor, &[String]) -> Option<String>) -> &mut Self {
        self.keybind_map.insert(key, f);

        self
    }

    ///====
    fn call_command(&mut self) -> Result<Option<String>, ()> { 
        let command: Vec<&str> = self.command.split(' ').collect();
        let args: Vec<String> = command[1..].to_owned().into_iter().map(|a| a.to_owned()).collect();

        let ret = match self.command_map.get(command[0]) {
            Some(func) => Ok(func(self, &args)),
            None => Err(()),
        };

        self.command.clear();

        ret
    }

    fn run_keybind(&mut self, event: event::Event) {
	   match event {
            event::Event::Key(Key::Char('\n')) => (),

            event::Event::Key(key) => {
                if let Some(command) = self.keybind_map.get(&key) {
                    if let Some(s) = command(self, &[]) {
                        self.print_bottom(&s).unwrap();
                    }
                }
            },

            _ => (),
	   };
    }

    fn parse_event(&mut self, event: event::Event) {
        match self.state {
            State::Command => {
	   	        match event {
                    event::Event::Key(key) => match key {
                        Key::Esc => {
                            self.state = State::Normal;
                        },
                        // Newline
                        Key::Char('\n') => {
                            self.state = State::Normal;

                            match self.call_command() {
                                Ok(s) => if let Some(s) = s {
                                    self.print_bottom(&s)
                                } else {
                                    Ok(())
                                },
                                Err(()) => self.print_bottom("Unknown command"),
                            }.unwrap();
                        },

                        // Don't go back when no characters have been removed 
                        Key::Backspace => if self.command.pop().is_some() {
	   		                write!(self.screen, "{}{}", cursor::Left(1), clear::UntilNewline).unwrap();
                        },

                        // Append to command
                        Key::Char(a) => {
                            let a = a as char;
	   		                write!(self.screen, "{}", a).unwrap();
                            self.command.push(a);
                        },

                        _ => (),
                    },
                    
                    _ => (),
                };
            },

            State::Normal => {
                self.run_keybind(event);

                self.reset_cursor();
            },

            State::Insert => {
                if let event::Event::Key(Key::Char(key)) = event {
                    self.cur_text_buf().insert(key);
                    self.needs_redraw = true;
                } else if event == event::Event::Key(Key::Backspace) {
                    self.cur_text_buf().backspace();
                    self.needs_redraw = true;
                } else {
                    self.run_keybind(event);
                }

                self.reset_cursor();
            }
        }

        self.screen.flush().unwrap();
    }

    pub fn draw_text(&mut self) {
        let term_size = terminal_size().unwrap();
        // Account for bottem two rows being used for status
        let term_size = (term_size.0, term_size.1 - 1);

        // Goto top left
        write!(self.screen, "{}", cursor::Goto(1,1)).unwrap();
        
        let (text_buf, offset) = &self.text_bufs[self.cur_text_buf];
        let data = text_buf.data();

        for y in 0..term_size.1 {
            let text = if let Some(row) = data.get(y as usize + offset) {
                row
            } else {
                "~"
            };

            write!(self.screen, "{}\r\n", text).unwrap();
        }
            
        self.reset_cursor();
    }

    pub fn start_loop(&mut self) { 
        while self.running {  
            // Scroll
            let (text_buf, offset) = &mut self.text_bufs[self.cur_text_buf];
            let term_size = terminal_size().unwrap();

            if text_buf.cursor().1 as isize - *offset as isize > term_size.1 as isize - 2 {
                self.text_bufs[self.cur_text_buf].1 += 1;
                self.needs_redraw = true;
            } else if (text_buf.cursor().1 as isize - *offset as isize) < 0 {
                self.text_bufs[self.cur_text_buf].1 -= 1;
                self.needs_redraw = true;
            }

            if self.needs_redraw {
                // Clear screen
                self.clear();

                // Redraw screen
                self.draw_text();

                // Flush screen
                self.screen.flush().unwrap();

                self.needs_redraw = false;
            }

            // Check events
	        let event = self.events.next().unwrap().unwrap();
            self.parse_event(event); 
        }
    }
    
    fn reset_cursor(&mut self) {
        let size = terminal_size().unwrap();

        let cursor_pos = match self.state {
            State::Normal => {
                let text_buf = &self.text_bufs[self.cur_text_buf];
                let pos = text_buf.0.cursor();
                (pos.0 as u16 + 1, (pos.1 + 1 - text_buf.1) as u16)
            },

            State::Insert => {
                let text_buf = &self.text_bufs[self.cur_text_buf];
                let pos = text_buf.0.cursor();
                (pos.0 as u16 + 1, (pos.1 + 1 - text_buf.1) as u16)
            },

            State::Command => {
                let command_len = self.command.len();

                (command_len as u16 + 2, size.1)
            }
        };

        write!(self.screen, "{}", cursor::Goto(cursor_pos.0 as u16, cursor_pos.1 as u16)).unwrap();
    }

    // Utility
    fn cur_text_buf(&mut self) -> &mut TextBuf {
        &mut self.text_bufs[self.cur_text_buf].0
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn redraw(&mut self) {
        self.needs_redraw = true;
    }

    pub fn print_bottom(&mut self, status: &str) -> Result<(), std::io::Error> {
        let term_size = terminal_size().unwrap();
        write!(self.screen, "{}{}{}", cursor::Goto(0, term_size.1), clear::CurrentLine, status)
    }

    pub fn set_state(&mut self, state: State) {
        self.state = state;
    }

    pub fn clear(&mut self) {
        write!(self.screen, "{}", clear::All).unwrap();
    }

    pub fn new_text_buf(&mut self, path: Option<String>) {
        self.text_bufs.push(
            (match path {
                Some(p) => TextBuf::from_file(&p),
                None => TextBuf::empty(),
            }, 0)
        );
    }
}

// Default environment functions
fn quit(editor: &mut Editor, _: &[String]) -> Option<String> {
    editor.stop();
    None
}

fn write(editor: &mut Editor, args: &[String]) -> Option<String> {
    let path = args.into_iter().next();

    let path = match path {
        Some(s) => Some(s.to_string()),
        None => None,
    };

    Some(match editor.cur_text_buf().write(path) {
        Ok(_) => "File written".to_string(),
        Err(e) => format!("Failed to write file: {}", e),
    })
}

fn writequit(editor: &mut Editor, _: &[String]) -> Option<String> {
    quit(editor, &[]);
    write(editor, &[]);
    None
}

fn command_mode(editor: &mut Editor, _: &[String]) -> Option<String> {
    editor.set_state(State::Command);
    Some(":".to_string())
}

fn normal_mode(editor: &mut Editor, _: &[String]) -> Option<String> {
    editor.set_state(State::Normal);
    Some("--NORMAL--".to_string())
}

fn insert_mode(editor: &mut Editor, _: &[String]) -> Option<String> {
    editor.set_state(State::Insert);
    Some("--INSERT--".to_string())
}

fn left(editor: &mut Editor, _: &[String]) -> Option<String> {
    editor.cur_text_buf().left();
    None
}

fn right(editor: &mut Editor, _: &[String]) -> Option<String> {
    editor.cur_text_buf().right();
    None
}

fn up(editor: &mut Editor, _: &[String]) -> Option<String> {
    editor.cur_text_buf().up();
    None
}

fn down(editor: &mut Editor, _: &[String]) -> Option<String> {
    editor.cur_text_buf().down();
    None
}
