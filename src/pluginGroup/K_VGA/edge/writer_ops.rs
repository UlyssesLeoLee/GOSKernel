//! Edge: writer operations
use core::fmt;
use crate::pluginGroup::K_VGA::node::{Writer, Color, ColorCode, ScreenChar, BUFFER_WIDTH, BUFFER_HEIGHT};

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.newline(),
            b'\r' => { self.col = 0; }
            byte  => {
                if self.col >= BUFFER_WIDTH { self.newline(); }
                self.buffer.write_char(self.row, self.col, ScreenChar { ascii: byte, color: self.color });
                self.col += 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            if byte.is_ascii() && (byte == b'\n' || byte == b'\r' || (byte >= 0x20 && byte <= 0x7e)) {
                self.write_byte(byte);
            } else {
                self.write_byte(0xfe); 
            }
        }
    }

    pub fn newline(&mut self) {
        if self.row < BUFFER_HEIGHT - 1 {
            self.row += 1;
        } else {
            for r in 1..BUFFER_HEIGHT {
                for c in 0..BUFFER_WIDTH {
                    let ch = self.buffer.read_char(r, c);
                    self.buffer.write_char(r - 1, c, ch);
                }
            }
            self.clear_row(BUFFER_HEIGHT - 1);
        }
        self.col = 0;
    }

    pub fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar { ascii: b' ', color: self.color };
        for c in 0..BUFFER_WIDTH { self.buffer.write_char(row, c, blank); }
    }

    pub fn set_color(&mut self, fg: Color, bg: Color) { self.color = ColorCode::new(fg, bg); }

    pub fn clear(&mut self) {
        self.row = 0;
        self.col = 0;
        for r in 0..BUFFER_HEIGHT { self.clear_row(r); }
    }

    pub fn col(&self) -> usize { self.col }

    pub fn backspace(&mut self) {
        if self.col > 0 {
            self.col -= 1;
            self.buffer.write_char(self.row, self.col, ScreenChar { ascii: b' ', color: self.color });
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}
