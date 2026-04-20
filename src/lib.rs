#![allow(non_snake_case)]

use std::cell::UnsafeCell;

mod cell;
mod grid;
mod parser;
mod scrollback;
mod terminal;

use cell::Cell;
use grid::{MAX_COLS, MAX_ROWS};
use terminal::{DEBUG_LOG_MAX, DebugLogEntry, Terminal};

const INPUT_BUFFER_SIZE: usize = 8192;

struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

static TERMINAL: Global<Terminal> = Global(UnsafeCell::new(Terminal::new()));
static INPUT_BUFFER: Global<[u8; INPUT_BUFFER_SIZE]> =
    Global(UnsafeCell::new([0; INPUT_BUFFER_SIZE]));
static EMPTY_SCROLLBACK_LINE: Global<[u8; MAX_COLS * Cell::BYTE_SIZE]> =
    Global(UnsafeCell::new([0; MAX_COLS * Cell::BYTE_SIZE]));

fn terminal_mut() -> &'static mut Terminal {
    unsafe { &mut *TERMINAL.0.get() }
}

fn input_buffer_mut() -> &'static mut [u8; INPUT_BUFFER_SIZE] {
    unsafe { &mut *INPUT_BUFFER.0.get() }
}

fn clamp_dimension(value: u32, max: usize) -> u16 {
    if value == 0 {
        1
    } else if value > max as u32 {
        max as u16
    } else {
        value as u16
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn init(cols: u32, rows: u32) {
    let cols = clamp_dimension(cols, MAX_COLS);
    let rows = clamp_dimension(rows, MAX_ROWS);
    let terminal = terminal_mut();
    terminal.reset(cols, rows);
    terminal.scrollback.reset();
}

#[unsafe(no_mangle)]
pub extern "C" fn resizeTerminal(cols: u32, rows: u32) {
    let cols = clamp_dimension(cols, MAX_COLS);
    let rows = clamp_dimension(rows, MAX_ROWS);
    terminal_mut().resize(cols, rows);
}

#[unsafe(no_mangle)]
pub extern "C" fn getWriteBuffer() -> *mut u8 {
    input_buffer_mut().as_mut_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn writeBytes(len: u32) {
    let len = len.min(INPUT_BUFFER_SIZE as u32) as usize;
    let chunk = &input_buffer_mut()[..len];
    terminal_mut().write(chunk);
}

#[unsafe(no_mangle)]
pub extern "C" fn getGridPtr() -> *const u8 {
    terminal_mut().grid.cells.as_ptr().cast()
}

#[unsafe(no_mangle)]
pub extern "C" fn getDirtyPtr() -> *const u8 {
    terminal_mut().grid.dirty.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn clearDirty() {
    terminal_mut().grid.clear_dirty();
}

#[unsafe(no_mangle)]
pub extern "C" fn getCursorRow() -> u32 {
    terminal_mut().cursor_row as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn getCursorCol() -> u32 {
    terminal_mut().cursor_col as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn getCursorVisible() -> u32 {
    u32::from(terminal_mut().cursor_visible)
}

#[unsafe(no_mangle)]
pub extern "C" fn getCols() -> u32 {
    terminal_mut().cols as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn getRows() -> u32 {
    terminal_mut().rows as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn getCursorKeysApp() -> u32 {
    u32::from(terminal_mut().cursor_keys_app)
}

#[unsafe(no_mangle)]
pub extern "C" fn getBracketedPaste() -> u32 {
    u32::from(terminal_mut().bracketed_paste)
}

#[unsafe(no_mangle)]
pub extern "C" fn getUsingAltScreen() -> u32 {
    u32::from(terminal_mut().using_alt_screen)
}

#[unsafe(no_mangle)]
pub extern "C" fn getTitlePtr() -> *const u8 {
    terminal_mut().title_buf.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn getTitleLen() -> u32 {
    terminal_mut().title_len as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn getTitleChanged() -> u32 {
    let terminal = terminal_mut();
    if terminal.title_changed {
        terminal.title_changed = false;
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn getScrollbackCount() -> u32 {
    terminal_mut().scrollback.count
}

#[unsafe(no_mangle)]
pub extern "C" fn getScrollbackLine(offset: u32) -> *const u8 {
    let terminal = terminal_mut();
    if let Some(line) = terminal.scrollback.get_line(offset) {
        line.cells.as_ptr().cast()
    } else {
        unsafe { (*EMPTY_SCROLLBACK_LINE.0.get()).as_ptr() }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn getScrollbackLineLen(offset: u32) -> u32 {
    terminal_mut()
        .scrollback
        .get_line(offset)
        .map(|line| line.len as u32)
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn getResponsePtr() -> *const u8 {
    terminal_mut().response_buf.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn getResponseLen() -> u32 {
    terminal_mut().response_len as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn clearResponse() {
    terminal_mut().response_len = 0;
}

#[unsafe(no_mangle)]
pub extern "C" fn getDebugLogPtr() -> *const u8 {
    terminal_mut()
        .debug_log
        .as_ptr()
        .cast::<DebugLogEntry>()
        .cast::<u8>()
}

#[unsafe(no_mangle)]
pub extern "C" fn getDebugLogCount() -> u32 {
    terminal_mut().debug_log_count
}

#[unsafe(no_mangle)]
pub extern "C" fn getDebugLogEntrySize() -> u32 {
    core::mem::size_of::<DebugLogEntry>() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn getDebugLogMax() -> u32 {
    DEBUG_LOG_MAX as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn getCellSize() -> u32 {
    Cell::BYTE_SIZE as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn getMaxCols() -> u32 {
    MAX_COLS as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn with_terminal<T>(cols: u32, rows: u32, f: impl FnOnce(&mut Terminal) -> T) -> T {
        let _guard = TEST_LOCK.lock().unwrap();
        init(cols, rows);
        f(terminal_mut())
    }

    #[test]
    fn basic_print() {
        with_terminal(80, 24, |terminal| {
            terminal.write(b"Hello");
            let h = terminal.grid.get_cell(0, 0);
            let e = terminal.grid.get_cell(0, 1);
            assert_eq!('H' as u32, h.char);
            assert_eq!('e' as u32, e.char);
            assert_eq!(5, terminal.cursor_col);
        });
    }

    #[test]
    fn linefeed_and_carriage_return() {
        with_terminal(80, 24, |terminal| {
            terminal.write(b"AB\r\nCD");
            assert_eq!('A' as u32, terminal.grid.get_cell(0, 0).char);
            assert_eq!('C' as u32, terminal.grid.get_cell(1, 0).char);
            assert_eq!(1, terminal.cursor_row);
            assert_eq!(2, terminal.cursor_col);
        });
    }

    #[test]
    fn cursor_movement_csi() {
        with_terminal(80, 24, |terminal| {
            terminal.write(b"\x1b[5;10H");
            assert_eq!(4, terminal.cursor_row);
            assert_eq!(9, terminal.cursor_col);
        });
    }

    #[test]
    fn sgr_colors() {
        with_terminal(80, 24, |terminal| {
            terminal.write(b"\x1b[31mR\x1b[0mN");
            let red = terminal.grid.get_cell(0, 0);
            let normal = terminal.grid.get_cell(0, 1);
            assert_eq!(1, red.fg);
            assert_eq!(cell::DEFAULT_COLOR, normal.fg);
        });
    }

    #[test]
    fn erase_in_display() {
        with_terminal(80, 24, |terminal| {
            terminal.write(b"ABCDE\x1b[1;3H\x1b[J");
            assert_eq!('A' as u32, terminal.grid.get_cell(0, 0).char);
            assert_eq!('B' as u32, terminal.grid.get_cell(0, 1).char);
            assert_eq!(' ' as u32, terminal.grid.get_cell(0, 2).char);
        });
    }

    #[test]
    fn scroll_on_linefeed_at_bottom() {
        with_terminal(80, 3, |terminal| {
            terminal.write(b"L1\r\nL2\r\nL3\r\nL4");
            assert_eq!('L' as u32, terminal.grid.get_cell(0, 0).char);
            assert_eq!('2' as u32, terminal.grid.get_cell(0, 1).char);
        });
    }

    #[test]
    fn wrap_pending() {
        with_terminal(5, 3, |terminal| {
            terminal.write(b"12345");
            assert!(terminal.wrap_pending);
            assert_eq!(0, terminal.cursor_row);
            terminal.write(b"6");
            assert_eq!(1, terminal.cursor_row);
            assert_eq!(1, terminal.cursor_col);
        });
    }

    #[test]
    fn alternate_screen_buffer() {
        with_terminal(80, 24, |terminal| {
            terminal.write(b"main screen");
            assert_eq!('m' as u32, terminal.grid.get_cell(0, 0).char);
            terminal.write(b"\x1b[?1049h");
            assert!(terminal.using_alt_screen);
            assert_eq!(' ' as u32, terminal.grid.get_cell(0, 0).char);
            terminal.write(b"alt screen");
            terminal.write(b"\x1b[?1049l");
            assert!(!terminal.using_alt_screen);
            assert_eq!('m' as u32, terminal.grid.get_cell(0, 0).char);
        });
    }

    #[test]
    fn erase_inherits_current_background_color() {
        with_terminal(80, 24, |terminal| {
            terminal.write(b"\x1b[41m");
            terminal.write(b"\x1b[2K");
            let cell = terminal.grid.get_cell(0, 0);
            assert_eq!(1, cell.bg);
            assert_eq!(' ' as u32, cell.char);
            terminal.write(b"\x1b[2J");
            let cell2 = terminal.grid.get_cell(5, 10);
            assert_eq!(1, cell2.bg);
            terminal.write(b"\x1b[0m\x1b[2K");
            let cell3 = terminal.grid.get_cell(0, 0);
            assert_eq!(cell::DEFAULT_COLOR, cell3.bg);
        });
    }

    #[test]
    fn scroll_fills_new_lines_with_current_background() {
        with_terminal(80, 3, |terminal| {
            terminal.write(b"\x1b[42m");
            terminal.write(b"L1\r\nL2\r\nL3\r\nL4");
            let blank = terminal.grid.get_cell(2, 79);
            assert_eq!(2, blank.bg);
        });
    }

    #[test]
    fn scrollback() {
        with_terminal(80, 3, |terminal| {
            terminal.write(b"L1\r\nL2\r\nL3\r\nL4\r\nL5");
            assert_eq!(2, terminal.scrollback.count);
            let line0 = terminal.scrollback.get_line(0).unwrap();
            assert_eq!('L' as u32, line0.cells[0].char);
            assert_eq!('2' as u32, line0.cells[1].char);
        });
    }
}
