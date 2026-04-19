use crate::cell::Cell;

pub const MAX_COLS: usize = 256;
pub const MAX_ROWS: usize = 256;

#[derive(Clone)]
pub struct Grid {
    pub cells: [[Cell; MAX_COLS]; MAX_ROWS],
    pub cols: u16,
    pub rows: u16,
    pub dirty: [u8; MAX_ROWS],
}

impl Grid {
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self {
            cells: [[Cell::BLANK; MAX_COLS]; MAX_ROWS],
            cols,
            rows,
            dirty: [1; MAX_ROWS],
        }
    }

    pub fn reset(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.clear();
    }

    pub fn get_cell(&self, row: u16, col: u16) -> Cell {
        if row >= self.rows || col >= self.cols {
            return Cell::BLANK;
        }
        self.cells[row as usize][col as usize]
    }

    pub fn set_cell(&mut self, row: u16, col: u16, cell: Cell) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        self.cells[row as usize][col as usize] = cell;
        self.dirty[row as usize] = 1;
    }

    pub fn clear(&mut self) {
        let mut row = 0usize;
        let limit = self.rows as usize;
        while row < limit {
            self.clear_row(row as u16);
            row += 1;
        }
    }

    pub fn clear_row(&mut self, row: u16) {
        self.clear_row_as(row, Cell::BLANK);
    }

    pub fn clear_row_as(&mut self, row: u16, blank: Cell) {
        if row >= self.rows {
            return;
        }
        let row_idx = row as usize;
        let mut col = 0usize;
        let limit = self.cols as usize;
        while col < limit {
            self.cells[row_idx][col] = blank;
            col += 1;
        }
        self.dirty[row_idx] = 1;
    }

    pub fn clear_range(&mut self, row: u16, start_col: u16, end_col: u16) {
        self.clear_range_as(row, start_col, end_col, Cell::BLANK);
    }

    pub fn clear_range_as(&mut self, row: u16, start_col: u16, end_col: u16, blank: Cell) {
        if row >= self.rows {
            return;
        }
        let end = end_col.min(self.cols) as usize;
        let mut col = start_col as usize;
        let row_idx = row as usize;
        while col < end {
            self.cells[row_idx][col] = blank;
            col += 1;
        }
        self.dirty[row_idx] = 1;
    }

    pub fn scroll_up(&mut self, top: u16, bottom: u16, count: u16, blank: Cell) {
        if count == 0 || top >= bottom {
            return;
        }
        let n = count.min(bottom - top);
        let mut row = top;
        while row + n < bottom {
            self.cells[row as usize] = self.cells[(row + n) as usize];
            self.dirty[row as usize] = 1;
            row += 1;
        }
        while row < bottom {
            self.clear_row_as(row, blank);
            row += 1;
        }
    }

    pub fn scroll_down(&mut self, top: u16, bottom: u16, count: u16, blank: Cell) {
        if count == 0 || top >= bottom {
            return;
        }
        let n = count.min(bottom - top);
        let span = bottom - top - n;
        let mut i = 0u16;
        while i < span {
            let dst = bottom - 1 - i;
            let src = dst - n;
            self.cells[dst as usize] = self.cells[src as usize];
            self.dirty[dst as usize] = 1;
            i += 1;
        }
        let mut row = top;
        while row < top + n {
            self.clear_row_as(row, blank);
            row += 1;
        }
    }

    pub fn clear_dirty(&mut self) {
        let mut row = 0usize;
        let limit = self.rows as usize;
        while row < limit {
            self.dirty[row] = 0;
            row += 1;
        }
    }
}
