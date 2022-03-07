use std::fs::File;
use std::io::{Read, Write, Result, Error, ErrorKind};
use std::path::PathBuf;

pub struct TextBuf {
    cursor: (usize, usize),
    file: Option<PathBuf>,
    changed: bool,
    data: Vec<String>,
}

impl TextBuf {
    pub fn empty() -> Self {
        Self {
            cursor: (0,0),
            file: None,
            changed: true,
            data: vec![String::new()],
        }
    }

    pub fn from_file(path: &str) -> Self {
        let mut file = File::open(path).unwrap();
        let mut data = "".to_string();

        file.read_to_string(&mut data).unwrap();

        let mut data: Vec<String> = data.split('\n').map(|x| {x.to_string()}).collect();
        data.pop();
    
        Self {
            cursor: (0,0),
            file: Some(PathBuf::from(path)),
            changed: true,
            data,
        }
    }

    pub fn write(&mut self, path: Option<String>) -> Result<()> {
        let mut file = if let Some(path) = path {
            File::create(path)?
        } else if let Some(path) = &self.file {
            File::create(path)?
        } else {
            return Err(Error::new(ErrorKind::InvalidInput, "No filename"))
        };

        for l in &self.data {
            file.write_all(l.as_bytes())?;
            file.write_all(b"\n")?;
        }
    
        file.sync_all()?;

        self.changed = false;

        Ok(())
    }

    pub fn insert(&mut self, ch: char) {
        if ch != '\n' {
            self.data[self.cursor.1].insert(self.cursor.0, ch);
            self.right();
        } else {
            let split = self.data[self.cursor.1].chars().collect::<Vec<char>>();
            let split = split.split_at(self.cursor.0);
            // Convert Vec<char> back into String
            let split = (split.0.iter().collect::<String>(), 
                         split.1.iter().collect::<String>());

            self.data[self.cursor.1] = split.0;
            self.cursor = (0, self.cursor.1 + 1);
            self.data.insert(self.cursor.1, split.1);
        }
        
        self.changed = true;
    }
    
    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.insert(c);
        }

        self.changed = true;
    }

    pub fn backspace(&mut self) {
        if self.cursor.0 > 0 {
            self.left();
            self.data[self.cursor.1].remove(self.cursor.0);
        } else if self.cursor.1 > 0 {
            let line_data = self.data[self.cursor.1].clone();
            let new_cursor_x = self.data[self.cursor.1 - 1].len();

            self.data[self.cursor.1 - 1].push_str(&line_data);
            self.data.remove(self.cursor.1);
            
            self.up();
            self.cursor = (new_cursor_x, self.cursor.1);
        }
    }

    pub fn get(&self) -> char {
        self.data[self.cursor.0].chars().nth(self.cursor.1).unwrap()
    }

    // Cursor moving
    pub fn left(&mut self) {
        let new_pos = if self.cursor.0 > 0 {
            self.cursor.0 - 1
        } else {
            0
        };

        self.cursor = (new_pos, self.cursor.1)
    }
    
    pub fn right(&mut self) {
        let new_pos = if self.cursor.0 < self.data[self.cursor.1].len() {
            self.cursor.0 + 1
        } else {
            self.cursor.0
        };

        self.cursor = (new_pos, self.cursor.1)
    }
    
    fn lock_to_row(&mut self) {
        if self.cursor.0 > self.data[self.cursor.1].len() {
            self.cursor = (self.data[self.cursor.1].len(), self.cursor.1)
        }
    }

    pub fn up(&mut self) {
        let new_pos = if self.cursor.1 > 0 {
            self.cursor.1 - 1
        } else {
            0
        };

        self.cursor = (self.cursor.0, new_pos);
        self.lock_to_row();
    }
    
    pub fn down(&mut self) {
        let new_pos = if self.cursor.1 + 1 < self.data.len() {
            self.cursor.1 + 1
        } else {
            self.cursor.1
        };

        self.cursor = (self.cursor.0, new_pos);
        self.lock_to_row();
    }

    pub fn data(&self) -> &[String] {
        &self.data
    }

    pub fn cursor(&self) -> (usize, usize) {
        self.cursor
    }
}
