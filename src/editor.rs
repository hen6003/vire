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

    text_buf: TextBuf,

    command_map: HashMap<String, fn(&mut Editor, Option<String>)>,
    keybind_map: HashMap<event::Key, fn(&mut Editor, Option<String>)>,

    events: Box<dyn Iterator<Item=Result<event::Event, std::io::Error>> + 'a>,
    screen: Box<dyn Write + 'a>,
}

impl<'a> Editor<'a> {
    /// Init functions
    pub fn new() -> Self {
        use std::io::{stdin, stdout};
        use termion::input::TermRead;
        use termion::raw::IntoRawMode;

        let stdout = stdout().into_raw_mode().unwrap();
        let screen = screen::AlternateScreen::from(stdout);
        let events = stdin().events();

        let text_buf = TextBuf::from_file("test.txt");

        Self {
            running: true,
            needs_redraw: true,
            state: State::Normal,
            command: String::new(),
            text_buf,
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

    pub fn add_command(&mut self, command: &str, f: fn(&mut Editor, Option<String>)) -> &mut Self {
        self.command_map.insert(command.to_string(), f);

        self
    }
    
    pub fn add_keybind(&mut self, key: event::Key, f: fn(&mut Editor, Option<String>)) -> &mut Self {
        self.keybind_map.insert(key, f);

        self
    }

    ///====
    fn call_command(&mut self) -> Result<(), ()> { 
        let (command, arg) = {
            let command: Vec<&str> = self.command.splitn(2, ' ').collect();

            (command[0].to_owned(),
             if command.len() > 1 {
                 Some(command[1].to_owned())
             } else {
                 None
             })
        };

        let ret = match self.command_map.get(&command) {
            Some(func) => {
                func(self, arg);
                Ok(())
            },
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
                    command(self, None);
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

                            if self.call_command() == Err(()) {
                                self.print_bottom("Unknown command")
                            } else {
                                self.print_bottom("")
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
                    self.text_buf.insert(key);
                    self.needs_redraw = true;
                } else if event == event::Event::Key(Key::Backspace) {
                    self.text_buf.backspace();
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

        let data = self.text_buf.data();

        for y in 0..term_size.1 {
            let text = if let Some(row) = data.get(y as usize) {
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
            if self.needs_redraw {
                // Clear screen
                self.clear();

                // Redraw screen
                self.draw_text();

                // Flush screen
                self.screen.flush().unwrap();

                self.needs_redraw = false;
            }
            
	        let event = self.events.next().unwrap().unwrap();
            self.parse_event(event); 
        }
    }
    
    fn reset_cursor(&mut self) {
        let cursor_pos = match self.state {
            State::Normal => {
                let pos = self.text_buf.cursor();
                (pos.0 as u16 + 1, pos.1 as u16 + 1)
            },

            State::Insert => {
                let pos = self.text_buf.cursor();
                (pos.0 as u16 + 1, pos.1 as u16 + 1)
            },

            State::Command => {
                let size = terminal_size().unwrap();
                let command_len = self.command.len();

                (command_len as u16 + 2, size.1)
            }
        };

        write!(self.screen, "{}", cursor::Goto(cursor_pos.0 as u16, cursor_pos.1 as u16)).unwrap();
    }


    // Utility
    fn cur_text_buf(&mut self) -> &mut TextBuf {
        &mut self.text_buf
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
}

// Default environment functions
fn quit(editor: &mut Editor, _: Option<String>) {
    editor.stop();
}

fn write(editor: &mut Editor, path: Option<String>) {
    editor.cur_text_buf().write(path).unwrap()
}

fn writequit(editor: &mut Editor, _: Option<String>) {
    quit(editor, None);
    write(editor, None);
}

fn command_mode(editor: &mut Editor, _: Option<String>) {
    editor.set_state(State::Command);
    editor.print_bottom(":").unwrap();
}

fn normal_mode(editor: &mut Editor, _: Option<String>) {
    editor.set_state(State::Normal);
    editor.print_bottom("--NORMAL--").unwrap();
}

fn insert_mode(editor: &mut Editor, _: Option<String>) {
    editor.set_state(State::Insert);
    editor.print_bottom("--INSERT--").unwrap();
}

fn left(editor: &mut Editor, _: Option<String>) {
    editor.cur_text_buf().left()
}

fn right(editor: &mut Editor, _: Option<String>) {
    editor.cur_text_buf().right()
}

fn up(editor: &mut Editor, _: Option<String>) {
    editor.cur_text_buf().up()
}

fn down(editor: &mut Editor, _: Option<String>) {
    editor.cur_text_buf().down()
}
