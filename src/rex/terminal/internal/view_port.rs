use crate::rex::terminal::internal::ViewPort;
use crate::rex::terminal::internal::glyph_string::GlyphString;
use crate::rex::terminal::{Cursor, ScrollMode, PrintStyle};

impl ViewPort {
    pub fn new(width: u16, height: u16, scroll_mode: ScrollMode) -> Self {
        ViewPort {
            garbage_line: GlyphString::new(),
            visible_lines: Vec::with_capacity(height as usize),
            cur_style: PrintStyle::default(),
            scroll_mode,
            width: width as usize,
            height: height as usize,
            cursor: Cursor::new(),
        }
    }

    pub fn take_visible_lines(&mut self) -> &mut Vec<GlyphString> {
        if self.visible_lines.len() > self.height {
            match self.scroll_mode {
                ScrollMode::Scroll => {
                    while self.visible_lines.len() > self.height {
                        self.visible_lines.remove(0);
                    }
                }
                ScrollMode::Fixed => {
                    self.visible_lines.truncate(self.height);
                }
            }
        }

        &mut self.visible_lines
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    pub fn newline(&mut self) {
        if self.cursor().row() == (self.height - 1) as u16 {
            match self.scroll_mode {
                ScrollMode::Scroll => {
                    self.remove(0);
                    self.visible_lines.push(GlyphString::new());
                }
                ScrollMode::Fixed => {
                    // This output will be truncated later
                    self.visible_lines.push(GlyphString::new());
                }
            }
        }

        self.cursor.set_x(1);
        self.cursor.incr_y(1);
    }

    pub fn cur_line(&mut self) -> &mut GlyphString {
        self.mut_line(self.cursor.y as usize)
    }

    pub fn mut_line(&mut self, index: usize) -> &mut GlyphString {
        if index >= self.height {
            &mut self.garbage_line
        } else {
            while self.visible_lines.len() < self.height {
                self.visible_lines.push(GlyphString::new());
            }

            self.visible_lines.get_mut(index).unwrap()
        }
    }

    pub fn remove(&mut self, index: usize) -> GlyphString {
        self.visible_lines.remove(index)
    }

    pub fn cursor_goto(&mut self, row: u16, col: u16) {
        self.cursor.set_x(col as i32);
        self.cursor.set_y(row as i32);
    }

    pub fn cursor_up(&mut self, amount: u16) {
        self.cursor.decr_y(amount);
    }

    pub fn cursor_down(&mut self, amount: u16) {
        self.cursor.incr_y(amount)
    }

    pub fn cursor_left(&mut self, amount: u16) {
        self.cursor.decr_x(amount)
    }

    pub fn cursor_right(&mut self, amount: u16) {
        self.cursor.incr_x(amount)
    }

    pub fn cursor_home(&mut self) {
        self.cursor.set_x(1)
    }
}
