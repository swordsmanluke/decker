use crate::decker::terminal::Cursor;
use std::cmp::{max, min};
use log::info;

impl Cursor {
    pub fn col(&self) -> u16 {
        self.x as u16
    }

    pub fn row(&self) -> u16 {
        self.y as u16
    }

    pub fn set_x(&mut self, n: i32) {
        self.x = min(max(0, n), self.x_max);
    }

    pub fn set_y(&mut self, n: i32) {
        self.y = min(max(0, n), self.y_max)
    }

    pub fn incr_x(&mut self, offset: u16) {
        self.set_x(self.x + offset as i32)
    }

    pub fn incr_y(&mut self, offset: u16) {
        self.set_y(self.y + offset as i32)
    }

    pub fn decr_x(&mut self, offset: u16) { self.set_x(self.x - offset as i32) }

    pub fn decr_y(&mut self, offset: u16) { self.set_y(self.y - offset as i32) }

    pub fn new(max_width: u16, max_height: u16) -> Self {
        Cursor {
            x: 0,
            y: 0,
            x_max: max_width as i32,
            y_max: max_height as i32
        }
    }
}