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

    #[cfg(test)]
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
        let cols = self.cols as usize;
        for row in &mut self.cells[..self.rows as usize] {
            row[..cols].fill(Cell::BLANK);
        }
        self.dirty[..self.rows as usize].fill(1);
    }

    pub fn clear_row_as(&mut self, row: u16, blank: Cell) {
        if row >= self.rows {
            return;
        }
        let row_idx = row as usize;
        self.cells[row_idx][..self.cols as usize].fill(blank);
        self.dirty[row_idx] = 1;
    }

    #[allow(dead_code)]
    pub fn clear_range(&mut self, row: u16, start_col: u16, end_col: u16) {
        self.clear_range_as(row, start_col, end_col, Cell::BLANK);
    }

    pub fn clear_range_as(&mut self, row: u16, start_col: u16, end_col: u16, blank: Cell) {
        if row >= self.rows {
            return;
        }
        let end = end_col.min(self.cols) as usize;
        let row_idx = row as usize;
        self.cells[row_idx][start_col as usize..end].fill(blank);
        self.dirty[row_idx] = 1;
    }

    pub fn clear_rows_as(&mut self, start_row: u16, end_row: u16, blank: Cell) {
        let start = start_row.min(self.rows) as usize;
        let end = end_row.min(self.rows) as usize;
        if start >= end {
            return;
        }

        let cols = self.cols as usize;
        for row in &mut self.cells[start..end] {
            row[..cols].fill(blank);
        }
        self.dirty[start..end].fill(1);
    }

    pub fn scroll_up(&mut self, top: u16, bottom: u16, count: u16, blank: Cell) {
        if count == 0 || top >= bottom {
            return;
        }
        let top = top as usize;
        let bottom = bottom as usize;
        let n = count.min((bottom - top) as u16) as usize;

        self.cells.copy_within(top + n..bottom, top);
        self.dirty[top..bottom - n].fill(1);
        for row in &mut self.cells[bottom - n..bottom] {
            row[..self.cols as usize].fill(blank);
        }
        self.dirty[bottom - n..bottom].fill(1);
    }

    pub fn scroll_down(&mut self, top: u16, bottom: u16, count: u16, blank: Cell) {
        if count == 0 || top >= bottom {
            return;
        }
        let top = top as usize;
        let bottom = bottom as usize;
        let n = count.min((bottom - top) as u16) as usize;

        self.cells.copy_within(top..bottom - n, top + n);
        self.dirty[top + n..bottom].fill(1);
        for row in &mut self.cells[top..top + n] {
            row[..self.cols as usize].fill(blank);
        }
        self.dirty[top..top + n].fill(1);
    }

    pub fn clear_dirty(&mut self) {
        self.dirty[..self.rows as usize].fill(0);
    }
}
