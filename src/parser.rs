pub const MAX_PARAMS: usize = 16;
pub const MAX_INTERMEDIATES: usize = 2;
pub const MAX_OSC: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Print,
    Execute,
    CsiDispatch,
    EscDispatch,
    OscDispatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Ground,
    Utf8,
    Escape,
    EscapeIntermediate,
    CsiParam,
    CsiIntermediate,
    CsiIgnore,
    OscString,
}

#[derive(Clone)]
pub struct Parser {
    state: State,
    pub print_char: u32,
    pub execute_byte: u8,
    pub params: [u16; MAX_PARAMS],
    pub param_count: u8,
    params_full: bool,
    pub subparam: [bool; MAX_PARAMS],
    pub intermediates: [u8; MAX_INTERMEDIATES],
    pub intermediate_count: u8,
    pub csi_private: u8,
    pub osc_data: [u8; MAX_OSC],
    pub osc_len: u16,
    utf8_buf: [u8; 4],
    utf8_remaining: u8,
    utf8_expected: u8,
}

impl Parser {
    pub const fn new() -> Self {
        Self {
            state: State::Ground,
            print_char: 0,
            execute_byte: 0,
            params: [0; MAX_PARAMS],
            param_count: 0,
            params_full: false,
            subparam: [false; MAX_PARAMS],
            intermediates: [0; MAX_INTERMEDIATES],
            intermediate_count: 0,
            csi_private: 0,
            osc_data: [0; MAX_OSC],
            osc_len: 0,
            utf8_buf: [0; 4],
            utf8_remaining: 0,
            utf8_expected: 0,
        }
    }

    pub fn feed(&mut self, byte: u8) -> Action {
        if byte == 0x1B {
            if self.state == State::OscString {
                self.state = State::Escape;
                return Action::OscDispatch;
            }
            self.enter_escape();
            return Action::None;
        }

        if byte == 0x18 || byte == 0x1A {
            self.state = State::Ground;
            return Action::None;
        }

        match self.state {
            State::Ground => self.handle_ground(byte),
            State::Utf8 => self.handle_utf8(byte),
            State::Escape => self.handle_escape(byte),
            State::EscapeIntermediate => self.handle_escape_intermediate(byte),
            State::CsiParam => self.handle_csi_param(byte),
            State::CsiIntermediate => self.handle_csi_intermediate(byte),
            State::CsiIgnore => self.handle_csi_ignore(byte),
            State::OscString => self.handle_osc_string(byte),
        }
    }

    fn enter_escape(&mut self) {
        self.state = State::Escape;
        self.intermediate_count = 0;
        self.csi_private = 0;
    }

    fn enter_csi(&mut self) {
        self.state = State::CsiParam;
        self.param_count = 0;
        self.params_full = false;
        self.intermediate_count = 0;
        self.csi_private = 0;
        self.params = [0; MAX_PARAMS];
        self.subparam = [false; MAX_PARAMS];
    }

    fn handle_ground(&mut self, byte: u8) -> Action {
        if byte < 0x20 {
            self.execute_byte = byte;
            return Action::Execute;
        }
        if byte < 0x7F {
            self.print_char = byte as u32;
            return Action::Print;
        }
        if byte == 0x7F {
            self.execute_byte = 0x7F;
            return Action::Execute;
        }
        if (0xC0..=0xDF).contains(&byte) {
            self.utf8_buf[0] = byte;
            self.utf8_expected = 2;
            self.utf8_remaining = 1;
            self.state = State::Utf8;
            return Action::None;
        }
        if (0xE0..=0xEF).contains(&byte) {
            self.utf8_buf[0] = byte;
            self.utf8_expected = 3;
            self.utf8_remaining = 2;
            self.state = State::Utf8;
            return Action::None;
        }
        if (0xF0..=0xF7).contains(&byte) {
            self.utf8_buf[0] = byte;
            self.utf8_expected = 4;
            self.utf8_remaining = 3;
            self.state = State::Utf8;
            return Action::None;
        }
        Action::None
    }

    fn handle_utf8(&mut self, byte: u8) -> Action {
        if (0x80..=0xBF).contains(&byte) {
            let idx = (self.utf8_expected - self.utf8_remaining) as usize;
            self.utf8_buf[idx] = byte;
            self.utf8_remaining -= 1;
            if self.utf8_remaining == 0 {
                self.print_char = decode_utf8(&self.utf8_buf[..self.utf8_expected as usize]);
                self.state = State::Ground;
                return Action::Print;
            }
            return Action::None;
        }
        self.state = State::Ground;
        self.handle_ground(byte)
    }

    fn handle_escape(&mut self, byte: u8) -> Action {
        if byte == b'[' {
            self.enter_csi();
            return Action::None;
        }
        if byte == b']' {
            self.state = State::OscString;
            self.osc_len = 0;
            return Action::None;
        }
        if (0x20..=0x2F).contains(&byte) {
            self.collect_intermediate(byte);
            self.state = State::EscapeIntermediate;
            return Action::None;
        }
        if (0x30..=0x7E).contains(&byte) {
            self.execute_byte = byte;
            self.state = State::Ground;
            return Action::EscDispatch;
        }
        if byte < 0x20 {
            self.execute_byte = byte;
            return Action::Execute;
        }
        self.state = State::Ground;
        Action::None
    }

    fn handle_escape_intermediate(&mut self, byte: u8) -> Action {
        if (0x20..=0x2F).contains(&byte) {
            self.collect_intermediate(byte);
            return Action::None;
        }
        if (0x30..=0x7E).contains(&byte) {
            self.execute_byte = byte;
            self.state = State::Ground;
            return Action::EscDispatch;
        }
        if byte < 0x20 {
            self.execute_byte = byte;
            return Action::Execute;
        }
        self.state = State::Ground;
        Action::None
    }

    fn handle_csi_param(&mut self, byte: u8) -> Action {
        if byte.is_ascii_digit() {
            if !self.params_full {
                let idx = if self.param_count == 0 {
                    self.param_count = 1;
                    0usize
                } else {
                    (self.param_count - 1) as usize
                };
                let digit = (byte - b'0') as u16;
                self.params[idx] = self.params[idx].saturating_mul(10).saturating_add(digit);
            }
            return Action::None;
        }
        if byte == b';' || byte == b':' {
            if self.param_count < MAX_PARAMS as u8 {
                if self.param_count == 0 {
                    self.param_count = 1;
                }
                let next = self.param_count as usize;
                self.param_count += 1;
                if self.param_count > MAX_PARAMS as u8 {
                    self.param_count = MAX_PARAMS as u8;
                    self.params_full = true;
                } else if byte == b':' && next < MAX_PARAMS {
                    self.subparam[next] = true;
                }
            }
            return Action::None;
        }
        if byte == b'?' || byte == b'>' || byte == b'!' {
            self.csi_private = byte;
            return Action::None;
        }
        if (0x20..=0x2F).contains(&byte) {
            self.collect_intermediate(byte);
            self.state = State::CsiIntermediate;
            return Action::None;
        }
        if (0x40..=0x7E).contains(&byte) {
            self.execute_byte = byte;
            self.state = State::Ground;
            return Action::CsiDispatch;
        }
        if byte < 0x20 {
            self.execute_byte = byte;
            return Action::Execute;
        }
        self.state = State::CsiIgnore;
        Action::None
    }

    fn handle_csi_intermediate(&mut self, byte: u8) -> Action {
        if (0x20..=0x2F).contains(&byte) {
            self.collect_intermediate(byte);
            return Action::None;
        }
        if (0x40..=0x7E).contains(&byte) {
            self.execute_byte = byte;
            self.state = State::Ground;
            return Action::CsiDispatch;
        }
        if byte < 0x20 {
            self.execute_byte = byte;
            return Action::Execute;
        }
        self.state = State::CsiIgnore;
        Action::None
    }

    fn handle_csi_ignore(&mut self, byte: u8) -> Action {
        if (0x40..=0x7E).contains(&byte) {
            self.state = State::Ground;
        }
        Action::None
    }

    fn handle_osc_string(&mut self, byte: u8) -> Action {
        if byte == 0x07 {
            self.state = State::Ground;
            return Action::OscDispatch;
        }
        if (0x20..=0x7E).contains(&byte) && (self.osc_len as usize) < MAX_OSC {
            self.osc_data[self.osc_len as usize] = byte;
            self.osc_len += 1;
        }
        Action::None
    }

    fn collect_intermediate(&mut self, byte: u8) {
        if (self.intermediate_count as usize) < MAX_INTERMEDIATES {
            self.intermediates[self.intermediate_count as usize] = byte;
            self.intermediate_count += 1;
        }
    }

    pub fn get_param(&self, idx: u8, default: u16) -> u16 {
        if idx >= self.param_count {
            return default;
        }
        let value = self.params[idx as usize];
        if value == 0 { default } else { value }
    }
}

fn decode_utf8(bytes: &[u8]) -> u32 {
    if bytes.is_empty() {
        return 0xFFFD;
    }
    match bytes.len() {
        2 => {
            let b0 = (bytes[0] & 0x1F) as u32;
            let b1 = (bytes[1] & 0x3F) as u32;
            (b0 << 6) | b1
        }
        3 => {
            let b0 = (bytes[0] & 0x0F) as u32;
            let b1 = (bytes[1] & 0x3F) as u32;
            let b2 = (bytes[2] & 0x3F) as u32;
            (b0 << 12) | (b1 << 6) | b2
        }
        4 => {
            let b0 = (bytes[0] & 0x07) as u32;
            let b1 = (bytes[1] & 0x3F) as u32;
            let b2 = (bytes[2] & 0x3F) as u32;
            let b3 = (bytes[3] & 0x3F) as u32;
            (b0 << 18) | (b1 << 12) | (b2 << 6) | b3
        }
        _ => 0xFFFD,
    }
}
