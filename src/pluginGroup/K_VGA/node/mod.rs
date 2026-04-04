use lazy_static::lazy_static;
use spin::Mutex;

const VGA_BUFFER_ADDR: usize = 0xb8000;
pub const BUFFER_WIDTH: usize = 80;
pub const BUFFER_HEIGHT: usize = 25;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0, Blue = 1, Green = 2, Cyan = 3, Red = 4, Magenta = 5, Brown = 6, LightGray = 7,
    DarkGray = 8, LightBlue = 9, LightGreen = 10, LightCyan = 11, LightRed = 12, Pink = 13, Yellow = 14, White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(u8);

impl ColorCode {
    #[inline]
    pub fn new(fg: Color, bg: Color) -> Self { ColorCode((bg as u8) << 4 | (fg as u8)) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ScreenChar {
    pub ascii: u8,
    pub color: ColorCode,
}

#[repr(transparent)]
pub struct Buffer {
    pub chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

impl Buffer {
    #[inline]
    pub fn write_char(&mut self, row: usize, col: usize, sc: ScreenChar) {
        unsafe { core::ptr::write_volatile(&mut self.chars[row][col] as *mut ScreenChar, sc); }
    }
    #[inline]
    pub fn read_char(&self, row: usize, col: usize) -> ScreenChar {
        unsafe { core::ptr::read_volatile(&self.chars[row][col] as *const ScreenChar) }
    }
}

pub struct Writer {
    pub col: usize,
    pub row: usize,
    pub color: ColorCode,
    pub buffer: &'static mut Buffer,
}

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        col: 0, row: 0,
        color: ColorCode::new(Color::LightGreen, Color::Black),
        buffer: unsafe { &mut *(VGA_BUFFER_ADDR as *mut Buffer) },
    });
}
