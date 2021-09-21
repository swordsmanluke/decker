use crate::decker::terminal::internal::ViewPort;
use crate::decker::terminal::internal::glyph_string::GlyphString;
use crate::decker::terminal::{Cursor, ScrollMode, PrintStyle, DeletionType};
use log::{info, warn};

impl ViewPort {
    pub fn new(pane_id: String, width: u16, height: u16, scroll_mode: ScrollMode) -> Self {
        ViewPort {
            pane_id,
            garbage_line: GlyphString::new(),
            visible_lines: Vec::with_capacity(height as usize),
            cur_style: PrintStyle::default(),
            scroll_mode,
            width: width as usize,
            height: height as usize,
            cursor: Cursor::new(width, height),
        }
    }

    pub fn width(&self) -> u16 {
        self.width as u16
    }

    pub fn height(&self) -> u16 {
        self.height as u16
    }

    pub fn set_scroll_mode(&mut self, mode: ScrollMode) {
        self.scroll_mode = mode
    }

    pub fn style(&self) -> PrintStyle {
        self.cur_style
    }

    pub fn apply_style(&mut self, vt100: &str) -> anyhow::Result<()> {
        self.cur_style.apply_vt100(vt100)?;
        Ok(())
    }

    pub(crate) fn clear(&mut self, deletion_type: DeletionType) {
        let row = self.cursor().row() as usize;
        let col = self.cursor().col() as usize;

        info!("{}: CSI deletion: {:?}",self.pane_id, deletion_type);

        match deletion_type {
            DeletionType::ClearLine => { self.cur_line().clear(); }
            DeletionType::ClearLineToCursor => { self.cur_line().clear_to(col); }
            DeletionType::ClearLineAfterCursor => { self.cur_line().clear_after(col); }
            DeletionType::ClearScreen => {
                self.visible_lines.iter_mut().for_each(|l| l.clear());
                self.cursor_goto(1, 1);
            }
            DeletionType::ClearScreenToCursor => {
                // Clear all the lines before us
                self.visible_lines[..row].iter_mut().for_each(|l| l.clear());
                // and our line
                self.cur_line().clear_to(col);
            }
            DeletionType::ClearScreenAfterCursor => {
                // Clear all the lines after us
                self.visible_lines[row+1..].iter_mut().for_each(|l| l.clear());
                // and our line
                self.cur_line().clear_after(col);
            }
            DeletionType::Unknown(vt100_code) => {
                warn!("{}: Unknown vt100 deletion string: {}", self.pane_id, vt100_code)
            }
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
        if self.cursor().row() == self.height as u16 {
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

        self.cursor.set_x(0);
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
        let final_row = amount + self.cursor.row();
        self.cursor.incr_y(amount);

        // If we are scrolling past the bottom row, scroll the base up.
        // TODO: This is for SCROLL, but not for FIXED panes
        if final_row >= self.height() {
            (self.height()..final_row).for_each(|_| {
                self.visible_lines.remove(0);
            });
        }
    }

    pub fn cursor_left(&mut self, amount: u16) {
        self.cursor.decr_x(amount)
    }

    pub fn cursor_right(&mut self, amount: u16) {
        self.cursor.incr_x(amount)
    }

    pub fn cursor_home(&mut self) {
        self.cursor.set_x(0)
    }

    pub fn cursor_loc(&self) -> (u16, u16) {
        (self.cursor.col(), self.cursor.row())
    }
}
