use crate::rex::terminal::Cursor;
use std::cmp::max;

impl Cursor {
    pub fn col(&self) -> u16 {
        self.x as u16
    }

    pub fn row(&self) -> u16 {
        self.y as u16
    }

    pub fn set_x(&mut self, n: i32) {
        self.x = max(1, n)
    }

    pub fn set_y(&mut self, n: i32) {
        self.y = max(1, n)
    }

    pub fn incr_x(&mut self, offset: u16) {
        self.set_x(self.x + offset as i32)
    }

    pub fn incr_y(&mut self, offset: u16) {
        self.set_y(self.y + offset as i32)
    }

    pub fn decr_x(&mut self, offset: u16) { self.set_x(self.x - offset as i32) }

    pub fn decr_y(&mut self, offset: u16) { self.set_y(self.y - offset as i32) }

    pub fn new() -> Self {
        Cursor {
            x: 1, // screen is 1-indexed
            y: 1,
        }
    }
}