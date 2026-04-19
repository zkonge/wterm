use crate::cell::{
    Cell, DEFAULT_COLOR, FLAG_BLINK, FLAG_BOLD, FLAG_DIM, FLAG_INVISIBLE, FLAG_ITALIC,
    FLAG_REVERSE, FLAG_STRIKETHROUGH, FLAG_UNDERLINE,
};
use crate::grid::{Grid, MAX_COLS, MAX_ROWS};
use crate::parser::{Action, Parser};
use crate::scrollback::Scrollback;

pub const DEBUG_LOG_MAX: usize = 32;
const EMPTY_DEBUG_LOG_ENTRY: DebugLogEntry = DebugLogEntry {
    final_byte: 0,
    private_marker: 0,
    param_count: 0,
    _pad: 0,
    params: [0; 4],
};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DebugLogEntry {
    pub final_byte: u8,
    pub private_marker: u8,
    pub param_count: u8,
    pub _pad: u8,
    pub params: [u16; 4],
}

const _: () = assert!(core::mem::size_of::<DebugLogEntry>() == 12);

pub struct Terminal {
    pub grid: Grid,
    pub parser: Parser,
    pub scrollback: Scrollback,
    pub cols: u16,
    pub rows: u16,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub wrap_pending: bool,
    saved_cursor_row: u16,
    saved_cursor_col: u16,
    saved_fg: u16,
    saved_bg: u16,
    saved_flags: u8,
    current_fg: u16,
    current_bg: u16,
    current_flags: u8,
    scroll_top: u16,
    scroll_bottom: u16,
    auto_wrap: bool,
    origin_mode: bool,
    pub cursor_keys_app: bool,
    pub bracketed_paste: bool,
    linefeed_mode: bool,
    alt_grid: Grid,
    alt_saved_cursor_row: u16,
    alt_saved_cursor_col: u16,
    alt_saved_fg: u16,
    alt_saved_bg: u16,
    alt_saved_flags: u8,
    pub using_alt_screen: bool,
    pub title_buf: [u8; 256],
    pub title_len: u16,
    pub title_changed: bool,
    pub response_buf: [u8; 64],
    pub response_len: u8,
    pub debug_log: [DebugLogEntry; DEBUG_LOG_MAX],
    debug_log_idx: u8,
    pub debug_log_count: u32,
    tab_stops: [u8; MAX_COLS],
}

impl Terminal {
    pub const fn new() -> Self {
        Self {
            grid: Grid::new(1, 1),
            parser: Parser::new(),
            scrollback: Scrollback::new(),
            cols: 1,
            rows: 1,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            wrap_pending: false,
            saved_cursor_row: 0,
            saved_cursor_col: 0,
            saved_fg: DEFAULT_COLOR,
            saved_bg: DEFAULT_COLOR,
            saved_flags: 0,
            current_fg: DEFAULT_COLOR,
            current_bg: DEFAULT_COLOR,
            current_flags: 0,
            scroll_top: 0,
            scroll_bottom: 1,
            auto_wrap: true,
            origin_mode: false,
            cursor_keys_app: false,
            bracketed_paste: false,
            linefeed_mode: false,
            alt_grid: Grid::new(1, 1),
            alt_saved_cursor_row: 0,
            alt_saved_cursor_col: 0,
            alt_saved_fg: DEFAULT_COLOR,
            alt_saved_bg: DEFAULT_COLOR,
            alt_saved_flags: 0,
            using_alt_screen: false,
            title_buf: [0; 256],
            title_len: 0,
            title_changed: false,
            response_buf: [0; 64],
            response_len: 0,
            debug_log: [EMPTY_DEBUG_LOG_ENTRY; DEBUG_LOG_MAX],
            debug_log_idx: 0,
            debug_log_count: 0,
            tab_stops: init_tab_stops(),
        }
    }

    fn blank_cell(&self) -> Cell {
        Cell::blank_with_bg(self.current_bg)
    }

    fn log_unhandled(&mut self, final_byte: u8, private_marker: u8) {
        let mut entry = DebugLogEntry {
            final_byte,
            private_marker,
            param_count: self.parser.param_count,
            _pad: 0,
            params: [0; 4],
        };
        let copy_count = self.parser.param_count.min(4);
        let mut i = 0usize;
        while i < copy_count as usize {
            entry.params[i] = self.parser.params[i];
            i += 1;
        }
        self.debug_log[self.debug_log_idx as usize] = entry;
        self.debug_log_idx = (self.debug_log_idx + 1) % DEBUG_LOG_MAX as u8;
        self.debug_log_count = self.debug_log_count.saturating_add(1);
    }

    pub fn reset(&mut self, cols: u16, rows: u16) {
        self.grid.reset(cols, rows);
        self.parser = Parser::new();
        self.cols = cols;
        self.rows = rows;
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.cursor_visible = true;
        self.wrap_pending = false;
        self.saved_cursor_row = 0;
        self.saved_cursor_col = 0;
        self.saved_fg = DEFAULT_COLOR;
        self.saved_bg = DEFAULT_COLOR;
        self.saved_flags = 0;
        self.current_fg = DEFAULT_COLOR;
        self.current_bg = DEFAULT_COLOR;
        self.current_flags = 0;
        self.scroll_top = 0;
        self.scroll_bottom = rows;
        self.auto_wrap = true;
        self.origin_mode = false;
        self.cursor_keys_app = false;
        self.bracketed_paste = false;
        self.linefeed_mode = false;
        self.alt_saved_cursor_row = 0;
        self.alt_saved_cursor_col = 0;
        self.alt_saved_fg = DEFAULT_COLOR;
        self.alt_saved_bg = DEFAULT_COLOR;
        self.alt_saved_flags = 0;
        self.using_alt_screen = false;
        self.title_len = 0;
        self.title_changed = false;
        self.response_len = 0;
        self.debug_log = [EMPTY_DEBUG_LOG_ENTRY; DEBUG_LOG_MAX];
        self.debug_log_idx = 0;
        self.debug_log_count = 0;
        self.tab_stops = init_tab_stops();
    }

    pub fn write(&mut self, data: &[u8]) {
        for &byte in data {
            self.process_byte(byte);
        }
    }

    pub fn resize(&mut self, new_cols: u16, new_rows: u16) {
        let cols = clamp_dimension(new_cols, MAX_COLS as u16);
        let rows = clamp_dimension(new_rows, MAX_ROWS as u16);
        let old_cols = self.cols;
        let old_rows = self.rows;

        if cols == old_cols && rows == old_rows {
            return;
        }

        if cols < old_cols {
            let preserve_rows = rows.min(old_rows);
            let mut row = 0u16;
            while row < preserve_rows {
                let mut col = cols;
                while col < old_cols {
                    self.grid.cells[row as usize][col as usize] = Cell::BLANK;
                    col += 1;
                }
                row += 1;
            }
        }

        if rows < old_rows && !self.using_alt_screen {
            let mut row = rows;
            while row < old_rows {
                let len = cols.min(old_cols);
                self.scrollback
                    .push(&self.grid.cells[row as usize][..len as usize], len);
                row += 1;
            }
        }

        self.cols = cols;
        self.rows = rows;
        self.grid.cols = cols;
        self.grid.rows = rows;

        if rows > old_rows {
            let mut row = old_rows;
            while row < rows {
                self.grid.clear_row(row);
                row += 1;
            }
        }

        if cols > old_cols {
            let preserve_rows = old_rows.min(rows);
            let mut row = 0u16;
            while row < preserve_rows {
                let mut col = old_cols;
                while col < cols {
                    self.grid.cells[row as usize][col as usize] = Cell::BLANK;
                    col += 1;
                }
                self.grid.dirty[row as usize] = 1;
                row += 1;
            }
        }

        self.scroll_top = 0;
        self.scroll_bottom = rows;

        if self.cursor_col >= cols {
            self.cursor_col = cols - 1;
        }
        if self.cursor_row >= rows {
            self.cursor_row = rows - 1;
        }

        let mut row = 0u16;
        while row < rows {
            self.grid.dirty[row as usize] = 1;
            row += 1;
        }
    }

    fn process_byte(&mut self, byte: u8) {
        match self.parser.feed(byte) {
            Action::None => {}
            Action::Print => self.print_char(self.parser.print_char),
            Action::Execute => self.execute_control(self.parser.execute_byte),
            Action::CsiDispatch => self.handle_csi(),
            Action::EscDispatch => self.handle_esc(),
            Action::OscDispatch => self.handle_osc(),
        }
    }

    fn print_char(&mut self, codepoint: u32) {
        if self.wrap_pending {
            self.cursor_col = 0;
            self.do_linefeed();
            self.wrap_pending = false;
        }

        self.grid.set_cell(
            self.cursor_row,
            self.cursor_col,
            Cell {
                char: codepoint,
                fg: self.current_fg,
                bg: self.current_bg,
                flags: self.current_flags,
                _pad1: 0,
                _pad2: 0,
                _pad3: 0,
            },
        );

        if self.cursor_col < self.cols - 1 {
            self.cursor_col += 1;
        } else if self.auto_wrap {
            self.wrap_pending = true;
        }
    }

    fn execute_control(&mut self, byte: u8) {
        match byte {
            0x07 => {}
            0x08 | 0x7F => self.backspace(),
            0x09 => self.horizontal_tab(),
            0x0A..=0x0C => {
                self.do_linefeed();
                if self.linefeed_mode {
                    self.carriage_return();
                }
            }
            0x0D => self.carriage_return(),
            _ => {}
        }
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            self.wrap_pending = false;
        }
    }

    fn horizontal_tab(&mut self) {
        let mut col = self.cursor_col as usize + 1;
        while col < self.cols as usize {
            if self.tab_stops[col] == 1 {
                break;
            }
            col += 1;
        }
        self.cursor_col = if col >= self.cols as usize {
            self.cols - 1
        } else {
            col as u16
        };
        self.wrap_pending = false;
    }

    fn do_linefeed(&mut self) {
        if self.cursor_row + 1 >= self.scroll_bottom {
            if !self.using_alt_screen && self.scroll_top == 0 {
                self.scrollback
                    .push(&self.grid.cells[self.scroll_top as usize][..self.cols as usize], self.cols);
            }
            self.grid
                .scroll_up(self.scroll_top, self.scroll_bottom, 1, self.blank_cell());
        } else {
            self.cursor_row += 1;
        }
    }

    fn carriage_return(&mut self) {
        self.cursor_col = 0;
        self.wrap_pending = false;
    }

    fn handle_esc(&mut self) {
        let byte = self.parser.execute_byte;
        let has_inter = self.parser.intermediate_count > 0;
        let inter0 = if has_inter { self.parser.intermediates[0] } else { 0 };

        if has_inter && inter0 == b'#' && byte == b'8' {
            self.decaln();
            return;
        }

        match byte {
            b'7' => self.save_cursor(),
            b'8' => self.restore_cursor(),
            b'D' => self.do_linefeed(),
            b'E' => {
                self.carriage_return();
                self.do_linefeed();
            }
            b'M' => self.reverse_index(),
            b'c' => self.full_reset(),
            b'H' => self.set_tab_stop(),
            _ => {}
        }
    }

    fn decaln(&mut self) {
        let mut row = 0u16;
        while row < self.rows {
            let mut col = 0u16;
            while col < self.cols {
                self.grid.set_cell(
                    row,
                    col,
                    Cell {
                        char: b'E' as u32,
                        ..Cell::BLANK
                    },
                );
                col += 1;
            }
            row += 1;
        }
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    fn set_tab_stop(&mut self) {
        if self.cursor_col < MAX_COLS as u16 {
            self.tab_stops[self.cursor_col as usize] = 1;
        }
    }

    fn save_cursor(&mut self) {
        self.saved_cursor_row = self.cursor_row;
        self.saved_cursor_col = self.cursor_col;
        self.saved_fg = self.current_fg;
        self.saved_bg = self.current_bg;
        self.saved_flags = self.current_flags;
    }

    fn restore_cursor(&mut self) {
        self.cursor_row = self.saved_cursor_row;
        self.cursor_col = self.saved_cursor_col;
        self.current_fg = self.saved_fg;
        self.current_bg = self.saved_bg;
        self.current_flags = self.saved_flags;
        self.wrap_pending = false;
    }

    fn reverse_index(&mut self) {
        if self.cursor_row == self.scroll_top {
            self.grid
                .scroll_down(self.scroll_top, self.scroll_bottom, 1, self.blank_cell());
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
        }
    }

    fn full_reset(&mut self) {
        self.reset(self.cols, self.rows);
    }

    fn handle_csi(&mut self) {
        let final_byte = self.parser.execute_byte;

        if self.parser.csi_private == b'?' {
            self.handle_private_mode(final_byte);
            return;
        }
        if self.parser.csi_private == b'!' && final_byte == b'p' {
            self.soft_reset();
            return;
        }
        if self.parser.csi_private == b'>' {
            self.log_unhandled(final_byte, b'>');
            return;
        }

        match final_byte {
            b'A' => self.cursor_up(self.parser.get_param(0, 1)),
            b'B' => self.cursor_down(self.parser.get_param(0, 1)),
            b'C' => self.cursor_forward(self.parser.get_param(0, 1)),
            b'D' => self.cursor_backward(self.parser.get_param(0, 1)),
            b'E' => {
                self.cursor_down(self.parser.get_param(0, 1));
                self.cursor_col = 0;
            }
            b'F' => {
                self.cursor_up(self.parser.get_param(0, 1));
                self.cursor_col = 0;
            }
            b'G' => self.cursor_to_column(self.parser.get_param(0, 1)),
            b'H' | b'f' => {
                self.cursor_position(self.parser.get_param(0, 1), self.parser.get_param(1, 1))
            }
            b'J' => self.erase_in_display(self.parser.get_param(0, 0)),
            b'K' => self.erase_in_line(self.parser.get_param(0, 0)),
            b'L' => self.insert_lines(self.parser.get_param(0, 1)),
            b'M' => self.delete_lines(self.parser.get_param(0, 1)),
            b'P' => self.delete_chars(self.parser.get_param(0, 1)),
            b'S' => self.scroll_up_n(self.parser.get_param(0, 1)),
            b'T' => self.scroll_down_n(self.parser.get_param(0, 1)),
            b'X' => self.erase_chars(self.parser.get_param(0, 1)),
            b'a' => self.cursor_forward(self.parser.get_param(0, 1)),
            b'd' => self.cursor_to_row(self.parser.get_param(0, 1)),
            b'e' => self.cursor_down(self.parser.get_param(0, 1)),
            b'g' => self.clear_tab_stop(self.parser.get_param(0, 0)),
            b'm' => self.handle_sgr(),
            b'n' => self.handle_device_status(),
            b'r' => self.set_scroll_region(self.parser.get_param(0, 1), self.parser.get_param(1, self.rows)),
            b's' => self.save_cursor(),
            b't' => {}
            b'u' => self.restore_cursor(),
            b'@' => self.insert_blanks(self.parser.get_param(0, 1)),
            b'`' => self.cursor_to_column(self.parser.get_param(0, 1)),
            _ => self.log_unhandled(final_byte, 0),
        }
    }

    fn handle_private_mode(&mut self, final_byte: u8) {
        match final_byte {
            b'h' => self.set_private_mode(true),
            b'l' => self.set_private_mode(false),
            _ => self.log_unhandled(final_byte, b'?'),
        }
    }

    fn set_private_mode(&mut self, enabled: bool) {
        let count = if self.parser.param_count == 0 {
            1
        } else {
            self.parser.param_count
        };
        let mut i = 0usize;
        while i < count as usize {
            let mode = self.parser.params[i];
            match mode {
                1 => self.cursor_keys_app = enabled,
                6 => self.origin_mode = enabled,
                7 => self.auto_wrap = enabled,
                12 => {}
                20 => self.linefeed_mode = enabled,
                25 => self.cursor_visible = enabled,
                47 | 1047 => self.switch_screen(enabled, false),
                1048 => {
                    if enabled {
                        self.save_cursor();
                    } else {
                        self.restore_cursor();
                    }
                }
                1049 => self.switch_screen(enabled, true),
                2004 => self.bracketed_paste = enabled,
                _ => {}
            }
            i += 1;
        }
    }

    fn switch_screen(&mut self, alt: bool, save_cursor: bool) {
        if alt == self.using_alt_screen {
            return;
        }

        if alt {
            if save_cursor {
                self.save_cursor_to_alt();
            }
            unsafe {
                core::ptr::copy_nonoverlapping(&self.grid, &mut self.alt_grid, 1);
            }
            self.grid.reset(self.cols, self.rows);
            self.using_alt_screen = true;
        } else {
            unsafe {
                core::ptr::copy_nonoverlapping(&self.alt_grid, &mut self.grid, 1);
            }
            self.using_alt_screen = false;
            if save_cursor {
                self.restore_cursor_from_alt();
            }
            let mut row = 0u16;
            while row < self.rows {
                self.grid.dirty[row as usize] = 1;
                row += 1;
            }
        }
        self.scroll_top = 0;
        self.scroll_bottom = self.rows;
    }

    fn save_cursor_to_alt(&mut self) {
        self.alt_saved_cursor_row = self.cursor_row;
        self.alt_saved_cursor_col = self.cursor_col;
        self.alt_saved_fg = self.current_fg;
        self.alt_saved_bg = self.current_bg;
        self.alt_saved_flags = self.current_flags;
    }

    fn restore_cursor_from_alt(&mut self) {
        self.cursor_row = self.alt_saved_cursor_row;
        self.cursor_col = self.alt_saved_cursor_col;
        self.current_fg = self.alt_saved_fg;
        self.current_bg = self.alt_saved_bg;
        self.current_flags = self.alt_saved_flags;
        self.wrap_pending = false;
    }

    fn soft_reset(&mut self) {
        self.cursor_visible = true;
        self.origin_mode = false;
        self.auto_wrap = true;
        self.cursor_keys_app = false;
        self.bracketed_paste = false;
        self.scroll_top = 0;
        self.scroll_bottom = self.rows;
        self.reset_style();
    }

    fn handle_device_status(&mut self) {
        let param = self.parser.get_param(0, 0);
        if param == 6 {
            let row = self.cursor_row + 1;
            let col = self.cursor_col + 1;
            let mut buf = [0u8; 64];
            let mut len = 0u8;
            buf[len as usize] = 0x1B;
            len += 1;
            buf[len as usize] = b'[';
            len += 1;
            len = append_u16(&mut buf, len, row);
            buf[len as usize] = b';';
            len += 1;
            len = append_u16(&mut buf, len, col);
            buf[len as usize] = b'R';
            len += 1;
            self.response_buf = buf;
            self.response_len = len;
        }
    }

    fn cursor_up(&mut self, n: u16) {
        let amount = if n == 0 { 1 } else { n };
        self.cursor_row = self.cursor_row.saturating_sub(amount);
        self.wrap_pending = false;
    }

    fn cursor_down(&mut self, n: u16) {
        let amount = if n == 0 { 1 } else { n };
        let max = self.rows - 1;
        self.cursor_row = self.cursor_row.saturating_add(amount).min(max);
        self.wrap_pending = false;
    }

    fn cursor_forward(&mut self, n: u16) {
        let amount = if n == 0 { 1 } else { n };
        let max = self.cols - 1;
        self.cursor_col = self.cursor_col.saturating_add(amount).min(max);
        self.wrap_pending = false;
    }

    fn cursor_backward(&mut self, n: u16) {
        let amount = if n == 0 { 1 } else { n };
        self.cursor_col = self.cursor_col.saturating_sub(amount);
        self.wrap_pending = false;
    }

    fn cursor_position(&mut self, row_param: u16, col_param: u16) {
        let row = if row_param == 0 { 0 } else { row_param - 1 };
        let col = if col_param == 0 { 0 } else { col_param - 1 };
        self.cursor_row = row.min(self.rows - 1);
        self.cursor_col = col.min(self.cols - 1);
        self.wrap_pending = false;
    }

    fn cursor_to_column(&mut self, col_param: u16) {
        let col = if col_param == 0 { 0 } else { col_param - 1 };
        self.cursor_col = col.min(self.cols - 1);
        self.wrap_pending = false;
    }

    fn cursor_to_row(&mut self, row_param: u16) {
        let row = if row_param == 0 { 0 } else { row_param - 1 };
        self.cursor_row = row.min(self.rows - 1);
        self.wrap_pending = false;
    }

    fn erase_in_display(&mut self, mode: u16) {
        let blank = self.blank_cell();
        match mode {
            0 => {
                self.grid
                    .clear_range_as(self.cursor_row, self.cursor_col, self.cols, blank);
                let mut row = self.cursor_row + 1;
                while row < self.rows {
                    self.grid.clear_row_as(row, blank);
                    row += 1;
                }
            }
            1 => {
                let mut row = 0u16;
                while row < self.cursor_row {
                    self.grid.clear_row_as(row, blank);
                    row += 1;
                }
                self.grid
                    .clear_range_as(self.cursor_row, 0, self.cursor_col + 1, blank);
            }
            2 | 3 => {
                let mut row = 0u16;
                while row < self.rows {
                    self.grid.clear_row_as(row, blank);
                    row += 1;
                }
                if mode == 3 {
                    self.scrollback.reset();
                }
            }
            _ => {}
        }
    }

    fn erase_in_line(&mut self, mode: u16) {
        let blank = self.blank_cell();
        match mode {
            0 => self
                .grid
                .clear_range_as(self.cursor_row, self.cursor_col, self.cols, blank),
            1 => self
                .grid
                .clear_range_as(self.cursor_row, 0, self.cursor_col + 1, blank),
            2 => self.grid.clear_row_as(self.cursor_row, blank),
            _ => {}
        }
    }

    fn erase_chars(&mut self, n: u16) {
        let count = if n == 0 { 1 } else { n };
        let end = self.cursor_col.saturating_add(count).min(self.cols);
        self.grid
            .clear_range_as(self.cursor_row, self.cursor_col, end, self.blank_cell());
    }

    fn insert_lines(&mut self, n: u16) {
        if self.cursor_row < self.scroll_top || self.cursor_row >= self.scroll_bottom {
            return;
        }
        self.grid.scroll_down(
            self.cursor_row,
            self.scroll_bottom,
            if n == 0 { 1 } else { n },
            self.blank_cell(),
        );
    }

    fn delete_lines(&mut self, n: u16) {
        if self.cursor_row < self.scroll_top || self.cursor_row >= self.scroll_bottom {
            return;
        }
        self.grid.scroll_up(
            self.cursor_row,
            self.scroll_bottom,
            if n == 0 { 1 } else { n },
            self.blank_cell(),
        );
    }

    fn delete_chars(&mut self, n: u16) {
        let count = if n == 0 { 1 } else { n } as usize;
        let row = self.cursor_row as usize;
        let blank = self.blank_cell();
        let mut col = self.cursor_col as usize;
        while col + count < self.cols as usize {
            self.grid.cells[row][col] = self.grid.cells[row][col + count];
            col += 1;
        }
        while col < self.cols as usize {
            self.grid.cells[row][col] = blank;
            col += 1;
        }
        self.grid.dirty[row] = 1;
    }

    fn insert_blanks(&mut self, n: u16) {
        let count = if n == 0 { 1 } else { n } as usize;
        let row = self.cursor_row as usize;
        let start = self.cursor_col as usize;
        let cols = self.cols as usize;
        let blank = self.blank_cell();
        if start + count >= cols {
            self.grid
                .clear_range_as(self.cursor_row, self.cursor_col, self.cols, blank);
            return;
        }
        let mut col = cols - 1;
        while col >= start + count {
            self.grid.cells[row][col] = self.grid.cells[row][col - count];
            if col == 0 {
                break;
            }
            col -= 1;
        }
        let end = (start + count).min(cols);
        let mut col = start;
        while col < end {
            self.grid.cells[row][col] = blank;
            col += 1;
        }
        self.grid.dirty[row] = 1;
    }

    fn scroll_up_n(&mut self, n: u16) {
        let count = if n == 0 { 1 } else { n };
        if !self.using_alt_screen && self.scroll_top == 0 {
            let mut i = 0u16;
            while i < count && i < self.scroll_bottom - self.scroll_top {
                self.scrollback.push(
                    &self.grid.cells[(self.scroll_top + i) as usize][..self.cols as usize],
                    self.cols,
                );
                i += 1;
            }
        }
        self.grid
            .scroll_up(self.scroll_top, self.scroll_bottom, count, self.blank_cell());
    }

    fn scroll_down_n(&mut self, n: u16) {
        self.grid.scroll_down(
            self.scroll_top,
            self.scroll_bottom,
            if n == 0 { 1 } else { n },
            self.blank_cell(),
        );
    }

    fn set_scroll_region(&mut self, top_param: u16, bottom_param: u16) {
        let top = if top_param == 0 { 0 } else { top_param - 1 };
        let bottom = bottom_param.min(self.rows);
        if top < bottom {
            self.scroll_top = top;
            self.scroll_bottom = bottom;
            self.cursor_row = if self.origin_mode { top } else { 0 };
            self.cursor_col = 0;
            self.wrap_pending = false;
        }
    }

    fn clear_tab_stop(&mut self, mode: u16) {
        match mode {
            0 => {
                if self.cursor_col < MAX_COLS as u16 {
                    self.tab_stops[self.cursor_col as usize] = 0;
                }
            }
            3 => self.tab_stops.fill(0),
            _ => {}
        }
    }

    fn handle_sgr(&mut self) {
        if self.parser.param_count == 0 {
            self.reset_style();
            return;
        }

        let mut i = 0usize;
        while i < self.parser.param_count as usize {
            let param = self.parser.params[i];
            match param {
                0 => self.reset_style(),
                1 => self.current_flags |= FLAG_BOLD,
                2 => self.current_flags |= FLAG_DIM,
                3 => self.current_flags |= FLAG_ITALIC,
                4 => {
                    if i + 1 < self.parser.param_count as usize && self.parser.subparam[i + 1] {
                        let sub = self.parser.params[i + 1];
                        if sub == 0 {
                            self.current_flags &= !FLAG_UNDERLINE;
                        } else {
                            self.current_flags |= FLAG_UNDERLINE;
                        }
                        i += 1;
                    } else {
                        self.current_flags |= FLAG_UNDERLINE;
                    }
                }
                5 => self.current_flags |= FLAG_BLINK,
                7 => self.current_flags |= FLAG_REVERSE,
                8 => self.current_flags |= FLAG_INVISIBLE,
                9 => self.current_flags |= FLAG_STRIKETHROUGH,
                22 => self.current_flags &= !(FLAG_BOLD | FLAG_DIM),
                23 => self.current_flags &= !FLAG_ITALIC,
                24 => self.current_flags &= !FLAG_UNDERLINE,
                25 => self.current_flags &= !FLAG_BLINK,
                27 => self.current_flags &= !FLAG_REVERSE,
                28 => self.current_flags &= !FLAG_INVISIBLE,
                29 => self.current_flags &= !FLAG_STRIKETHROUGH,
                30..=37 => self.current_fg = param - 30,
                38 => i += self.parse_extended_color(i, true),
                39 => self.current_fg = DEFAULT_COLOR,
                40..=47 => self.current_bg = param - 40,
                48 => i += self.parse_extended_color(i, false),
                49 => self.current_bg = DEFAULT_COLOR,
                90..=97 => self.current_fg = param - 90 + 8,
                100..=107 => self.current_bg = param - 100 + 8,
                _ => {
                    while i + 1 < self.parser.param_count as usize && self.parser.subparam[i + 1] {
                        i += 1;
                    }
                }
            }
            i += 1;
        }
    }

    fn parse_extended_color(&mut self, start: usize, foreground: bool) -> usize {
        if start + 1 >= self.parser.param_count as usize {
            return 0;
        }
        let kind = self.parser.params[start + 1];
        if kind == 5 && start + 2 < self.parser.param_count as usize {
            if foreground {
                self.current_fg = self.parser.params[start + 2];
            } else {
                self.current_bg = self.parser.params[start + 2];
            }
            return 2;
        }
        if kind == 2 && start + 4 < self.parser.param_count as usize {
            let r = self.parser.params[start + 2] as u8;
            let g = self.parser.params[start + 3] as u8;
            let b = self.parser.params[start + 4] as u8;
            let color = rgb_to_256(r, g, b);
            if foreground {
                self.current_fg = color;
            } else {
                self.current_bg = color;
            }
            return 4;
        }
        0
    }

    fn reset_style(&mut self) {
        self.current_fg = DEFAULT_COLOR;
        self.current_bg = DEFAULT_COLOR;
        self.current_flags = 0;
    }

    fn handle_osc(&mut self) {
        if self.parser.osc_len < 2 {
            return;
        }
        let data = &self.parser.osc_data[..self.parser.osc_len as usize];
        if (data[0] == b'0' || data[0] == b'2') && data[1] == b';' {
            let title = &data[2..];
            let len = title.len().min(self.title_buf.len());
            self.title_buf[..len].copy_from_slice(&title[..len]);
            self.title_len = len as u16;
            self.title_changed = true;
        }
    }
}

const fn init_tab_stops() -> [u8; MAX_COLS] {
    let mut stops = [0u8; MAX_COLS];
    let mut i = 8usize;
    while i < MAX_COLS {
        stops[i] = 1;
        i += 8;
    }
    stops
}

const fn clamp_dimension(value: u16, max: u16) -> u16 {
    if value == 0 {
        1
    } else if value > max {
        max
    } else {
        value
    }
}

fn append_u16(buf: &mut [u8], start: u8, value: u16) -> u8 {
    let mut value = value;
    let mut tmp = [0u8; 5];
    let mut count = 0usize;
    if value == 0 {
        buf[start as usize] = b'0';
        return start + 1;
    }
    while value > 0 {
        tmp[count] = (value % 10) as u8 + b'0';
        value /= 10;
        count += 1;
    }
    let mut pos = start as usize;
    let mut i = count;
    while i > 0 {
        i -= 1;
        buf[pos] = tmp[i];
        pos += 1;
    }
    pos as u8
}

fn rgb_to_256(r: u8, g: u8, b: u8) -> u16 {
    if r == g && g == b {
        if r < 8 {
            return 16;
        }
        if r > 248 {
            return 231;
        }
        let idx = ((r as u32 - 8) / 10).min(23) as u16;
        return idx + 232;
    }
    let ri = ((r as u32 * 5 + 127) / 255) as u16;
    let gi = ((g as u32 * 5 + 127) / 255) as u16;
    let bi = ((b as u32 * 5 + 127) / 255) as u16;
    16 + ri * 36 + gi * 6 + bi
}
