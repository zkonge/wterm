use crate::cell::Cell;
use crate::grid::MAX_COLS;

pub const MAX_SCROLLBACK_LINES: usize = 1000;

#[derive(Clone, Copy)]
pub struct ScrollbackLine {
    pub cells: [Cell; MAX_COLS],
    pub len: u16,
}

impl ScrollbackLine {
    pub const fn new() -> Self {
        Self {
            cells: [Cell::BLANK; MAX_COLS],
            len: 0,
        }
    }
}

pub struct Scrollback {
    pub lines: [ScrollbackLine; MAX_SCROLLBACK_LINES],
    pub count: u32,
    pub write_pos: u32,
}

impl Scrollback {
    pub const fn new() -> Self {
        Self {
            lines: [ScrollbackLine::new(); MAX_SCROLLBACK_LINES],
            count: 0,
            write_pos: 0,
        }
    }

    pub fn reset(&mut self) {
        self.count = 0;
        self.write_pos = 0;
    }

    pub fn push(&mut self, row: &[Cell], len: u16) {
        let write_idx = self.write_pos as usize;
        let line = &mut self.lines[write_idx];
        let limit = len as usize;
        line.cells[..limit].copy_from_slice(&row[..limit]);
        line.len = len;

        self.write_pos = (self.write_pos + 1) % MAX_SCROLLBACK_LINES as u32;
        if self.count < MAX_SCROLLBACK_LINES as u32 {
            self.count += 1;
        }
    }

    pub fn get_line(&self, offset: u32) -> Option<&ScrollbackLine> {
        if offset >= self.count {
            return None;
        }
        let idx = if self.count < MAX_SCROLLBACK_LINES as u32 {
            self.count - 1 - offset
        } else {
            (self.write_pos + MAX_SCROLLBACK_LINES as u32 - 1 - offset)
                % MAX_SCROLLBACK_LINES as u32
        };
        Some(&self.lines[idx as usize])
    }
}
