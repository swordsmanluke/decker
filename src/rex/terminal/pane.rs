use crate::rex::terminal::vt100_string::VT100String;

pub struct Pane {
    id: String,
    // Location and Dimensions
    x: u16,
    y: u16,
    height: u16,
    width: u16,

    // Cached lines
    lines: Vec<VT100String>,

    // virtual cursor location
    cur_x: usize,
    cur_y: usize
}

impl Pane {
    pub fn new(id: &str, x: u16, y: u16, height: u16, width: u16) -> Pane {
        let mut lines = (0..height).map(|_| VT100String::new("")).collect::<Vec<VT100String>>();

        Pane {
            id: String::from(id),
            x, y,
            height, width,
            lines,
            cur_x: 0,
            cur_y: 0
        }
    }

    pub fn push(&mut self, s: &str) -> anyhow::Result<()> {
        let mut vert_line = self.cur_y as usize;

        for c in s.chars() {
            match c {
                '\n' => {
                    self.cur_x = 0;
                    self.cur_y += 1;
                    if self.cur_y >= self.height as usize {
                        self.cur_y = (self.height - 1) as usize;
                        self.lines.remove(0); // discard the topmost line of output
                        self.lines.push(VT100String::new(""));
                    }
                }
                _ => {
                    let line = self.lines.get_mut(vert_line).unwrap();
                    line.set(self.cur_x, c);
                    self.cur_x += 1;
                }
            }
        }

        Ok(())
    }

    // A Handle for testing
    fn plaintext(&self) -> String {
        self.lines.iter().map(|l| l.as_str().to_owned()).collect::<Vec<String>>().join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_displays_blank_space_on_creation() {
        let pane = Pane::new("p1", 1, 1, 10, 20);
        assert_eq!("\n\n\n\n\n\n\n\n\n", pane.plaintext());
    }

    #[test]
    fn it_displays_pushed_text() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("a line of text");
        assert_eq!("a line of text\n\n\n\n\n\n\n\n\n", pane.plaintext());
    }

    #[test]
    fn it_moves_the_cursor_horizontally_after_writing() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("a line of text");
        assert_eq!(0, pane.cur_y);
        assert_eq!(14, pane.cur_x);
    }

    #[test]
    fn it_moves_the_cursor_vertically_after_newline() {
        let mut pane = Pane::new("p1", 1, 1, 10, 20);
        pane.push("two lines\nof text");
        assert_eq!(1, pane.cur_y);
        assert_eq!(7, pane.cur_x);
    }
}

