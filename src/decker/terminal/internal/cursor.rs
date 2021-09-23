use crate::decker::terminal::{Cursor, ScreenCoord, VirtualCoord};
use std::cmp::{max, min};
use log::info;

impl Cursor {
    pub fn col(&self) -> ScreenCoord {
        (self.x + 1) as ScreenCoord
    }

    pub fn row(&self) -> ScreenCoord {
        (self.y + 1) as ScreenCoord
    }

    pub fn x(&self) -> VirtualCoord {
        self.x
    }

    pub fn y(&self) -> VirtualCoord {
        self.y
    }

    pub fn set_x(&mut self, n: VirtualCoord) {
        self.x = min(max(0, n), self.x_max);
    }

    pub fn set_y(&mut self, n: VirtualCoord) {
        self.y = min(max(0, n), self.y_max)
    }

    pub fn incr_x(&mut self, offset: VirtualCoord) {
        self.set_x(self.x + offset)
    }

    pub fn incr_y(&mut self, offset: VirtualCoord) {
        self.set_y(self.y + offset)
    }

    pub fn decr_x(&mut self, offset: VirtualCoord) {
        if self.x > 0 {
            self.set_x(self.x - offset)
        }
    }

    pub fn decr_y(&mut self, offset: VirtualCoord) {
        let offset = min(offset, self.y);
        self.set_y(self.y - offset)
    }

    pub fn new(max_width: VirtualCoord, max_height: VirtualCoord) -> Self {
        Cursor {
            x: 0,
            y: 0,
            x_max: max_width,
            y_max: max_height,
        }
    }
}