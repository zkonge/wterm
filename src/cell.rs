pub const DEFAULT_COLOR: u16 = 256;

pub const FLAG_BOLD: u8 = 0x01;
pub const FLAG_DIM: u8 = 0x02;
pub const FLAG_ITALIC: u8 = 0x04;
pub const FLAG_UNDERLINE: u8 = 0x08;
pub const FLAG_BLINK: u8 = 0x10;
pub const FLAG_REVERSE: u8 = 0x20;
pub const FLAG_INVISIBLE: u8 = 0x40;
pub const FLAG_STRIKETHROUGH: u8 = 0x80;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub char: u32,
    pub fg: u16,
    pub bg: u16,
    pub flags: u8,
    pub _pad1: u8,
    pub _pad2: u8,
    pub _pad3: u8,
}

impl Cell {
    pub const BYTE_SIZE: usize = 12;
    pub const BLANK: Self = Self {
        char: ' ' as u32,
        fg: DEFAULT_COLOR,
        bg: DEFAULT_COLOR,
        flags: 0,
        _pad1: 0,
        _pad2: 0,
        _pad3: 0,
    };

    #[inline]
    pub const fn blank_with_bg(bg: u16) -> Self {
        Self {
            char: ' ' as u32,
            fg: DEFAULT_COLOR,
            bg,
            flags: 0,
            _pad1: 0,
            _pad2: 0,
            _pad3: 0,
        }
    }
}
