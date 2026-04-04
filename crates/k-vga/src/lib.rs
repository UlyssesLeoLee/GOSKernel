#![no_std]

use gos_protocol::{
    packet_to_signal, DISPLAY_CONTROL_POINTER_COL, DISPLAY_CONTROL_POINTER_ROW,
    DISPLAY_CONTROL_POINTER_VISIBLE, ExecStatus, ExecutorContext, ExecutorId, NodeEvent,
    NodeExecutorVTable, Signal, VectorAddress,
};
use x86_64::instructions::port::Port;

pub const NODE_VEC: VectorAddress = VectorAddress::new(1, 1, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.vga");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(vga_on_init),
    on_event: Some(vga_on_event),
    on_suspend: Some(vga_on_suspend),
    on_resume: None,
    on_teardown: None,
};

const VGA_TEXT_BUFFER_ADDR: usize = 0xB8000;
const VGA_CURSOR_INDEX: u16 = 0x3D4;
const VGA_CURSOR_DATA: u16 = 0x3D5;
pub const SCREEN_WIDTH: usize = 80;
pub const SCREEN_HEIGHT: usize = 25;
pub const BUFFER_WIDTH: usize = SCREEN_WIDTH;
pub const BUFFER_HEIGHT: usize = SCREEN_HEIGHT;
const CELL_COUNT: usize = BUFFER_WIDTH * BUFFER_HEIGHT;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(pub u8);

impl ColorCode {
    pub const fn new(fg: Color, bg: Color) -> Self {
        Self((bg as u8) << 4 | (fg as u8))
    }

    pub const fn fg(self) -> u8 {
        self.0 & 0x0F
    }

    pub const fn bg(self) -> u8 {
        (self.0 >> 4) & 0x0F
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ScreenChar {
    pub ascii: u8,
    pub color: ColorCode,
}

impl ScreenChar {
    pub const fn blank(color: ColorCode) -> Self {
        Self { ascii: b' ', color }
    }
}

#[repr(C)]
struct VgaState {
    column_position: u8,
    row_position: u8,
    color_code: ColorCode,
    scroll_top: u8,
    scroll_bottom: u8,
    saved_row: [u8; 2],
    saved_col: [u8; 2],
    pointer_col: u8,
    pointer_row: u8,
    pointer_visible: u8,
    cells: [ScreenChar; CELL_COUNT],
}

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut VgaState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut VgaState) }
}

fn text_buffer() -> *mut u16 {
    VGA_TEXT_BUFFER_ADDR as *mut u16
}

fn cell_index(row: usize, col: usize) -> usize {
    row * BUFFER_WIDTH + col
}

fn blank_cell(color: ColorCode) -> ScreenChar {
    ScreenChar::blank(color)
}

fn compose_vga_word(ascii: u8, color: ColorCode) -> u16 {
    (u16::from(color.0) << 8) | u16::from(ascii)
}

fn update_hw_cursor(row: usize, col: usize) {
    let pos = (row.min(BUFFER_HEIGHT - 1) * BUFFER_WIDTH + col.min(BUFFER_WIDTH - 1)) as u16;
    let mut index = Port::<u8>::new(VGA_CURSOR_INDEX);
    let mut data = Port::<u8>::new(VGA_CURSOR_DATA);
    unsafe {
        index.write(0x0F);
        data.write((pos & 0x00FF) as u8);
        index.write(0x0E);
        data.write((pos >> 8) as u8);
    }
}

fn render_cell(state: &VgaState, row: usize, col: usize) {
    let mut cell = state.cells[cell_index(row, col)];

    if state.pointer_visible != 0
        && usize::from(state.pointer_row.min((BUFFER_HEIGHT - 1) as u8)) == row
        && usize::from(state.pointer_col.min((BUFFER_WIDTH - 1) as u8)) == col
    {
        let fg = cell.color.fg();
        let bg = cell.color.bg();
        cell.color = ColorCode((fg << 4) | bg);
    }

    unsafe {
        text_buffer()
            .add(cell_index(row, col))
            .write_volatile(compose_vga_word(cell.ascii, cell.color));
    }
}

fn render_row(state: &VgaState, row: usize) {
    let mut col = 0usize;
    while col < BUFFER_WIDTH {
        render_cell(state, row, col);
        col += 1;
    }
}

fn render_full(state: &VgaState) {
    let mut row = 0usize;
    while row < BUFFER_HEIGHT {
        render_row(state, row);
        row += 1;
    }
    update_hw_cursor(
        usize::from(state.row_position),
        usize::from(state.column_position),
    );
}

fn set_cursor(state: &mut VgaState, row: usize, col: usize) {
    state.row_position = row.min(BUFFER_HEIGHT - 1) as u8;
    state.column_position = col.min(BUFFER_WIDTH - 1) as u8;
    update_hw_cursor(
        usize::from(state.row_position),
        usize::from(state.column_position),
    );
}

fn clear_row(state: &mut VgaState, row: usize) {
    let blank = blank_cell(state.color_code);
    let mut col = 0usize;
    while col < BUFFER_WIDTH {
        state.cells[cell_index(row, col)] = blank;
        col += 1;
    }
    render_row(state, row);
}

fn clear_screen(state: &mut VgaState) {
    let blank = blank_cell(state.color_code);
    let mut idx = 0usize;
    while idx < CELL_COUNT {
        state.cells[idx] = blank;
        idx += 1;
    }
    state.row_position = 0;
    state.column_position = 0;
    render_full(state);
}

fn draw_header(state: &mut VgaState, title: &str, fg: Color, bg: Color) {
    let original = state.color_code;
    state.color_code = ColorCode::new(fg, bg);
    clear_row(state, 0);
    let start = (BUFFER_WIDTH.saturating_sub(title.len())) / 2;
    let mut col = start;
    for byte in title.bytes() {
        if col >= BUFFER_WIDTH {
            break;
        }
        state.cells[cell_index(0, col)] = ScreenChar {
            ascii: byte,
            color: state.color_code,
        };
        col += 1;
    }
    render_row(state, 0);
    state.color_code = original;
    set_cursor(state, 1, 0);
}

fn scroll_body(state: &mut VgaState) {
    let top = usize::from(state.scroll_top).min(BUFFER_HEIGHT - 1);
    let bottom = usize::from(state.scroll_bottom).min(BUFFER_HEIGHT - 1);
    let mut row = top + 1;
    while row <= bottom {
        let mut col = 0usize;
        while col < BUFFER_WIDTH {
            state.cells[cell_index(row - 1, col)] = state.cells[cell_index(row, col)];
            col += 1;
        }
        row += 1;
    }
    let blank = blank_cell(state.color_code);
    let mut col = 0usize;
    while col < BUFFER_WIDTH {
        state.cells[cell_index(bottom, col)] = blank;
        col += 1;
    }
    let mut row = top;
    while row <= bottom {
        render_row(state, row);
        row += 1;
    }
    set_cursor(state, bottom, 0);
}

fn new_line(state: &mut VgaState) {
    let bottom = usize::from(state.scroll_bottom).min(BUFFER_HEIGHT - 1);
    if usize::from(state.row_position) < bottom {
        state.row_position = state.row_position.saturating_add(1);
        state.column_position = 0;
        update_hw_cursor(
            usize::from(state.row_position),
            usize::from(state.column_position),
        );
    } else {
        scroll_body(state);
    }
}

fn write_byte(state: &mut VgaState, byte: u8) {
    match byte {
        b'\n' => new_line(state),
        b'\r' => set_cursor(state, usize::from(state.row_position), 0),
        0x08 => {
            if state.column_position > 0 {
                state.column_position -= 1;
                let row = usize::from(state.row_position);
                let col = usize::from(state.column_position);
                state.cells[cell_index(row, col)] = blank_cell(state.color_code);
                render_cell(state, row, col);
                update_hw_cursor(row, col);
            }
        }
        byte => {
            if usize::from(state.column_position) >= BUFFER_WIDTH {
                new_line(state);
            }
            let row = usize::from(state.row_position);
            let col = usize::from(state.column_position);
            state.cells[cell_index(row, col)] = ScreenChar {
                ascii: byte,
                color: state.color_code,
            };
            render_cell(state, row, col);
            state.column_position = state.column_position.saturating_add(1);
            update_hw_cursor(
                usize::from(state.row_position),
                usize::from(state.column_position.min((BUFFER_WIDTH - 1) as u8)),
            );
        }
    }
}

fn handle_pointer_move(state: &mut VgaState, cmd: u8, val: u8) {
    let old_row = usize::from(state.pointer_row.min((BUFFER_HEIGHT - 1) as u8));
    let old_col = usize::from(state.pointer_col.min((BUFFER_WIDTH - 1) as u8));
    if cmd == DISPLAY_CONTROL_POINTER_COL {
        state.pointer_col = val.min((BUFFER_WIDTH - 1) as u8);
    } else if cmd == DISPLAY_CONTROL_POINTER_ROW {
        state.pointer_row = val.min((BUFFER_HEIGHT - 1) as u8);
    } else if cmd == DISPLAY_CONTROL_POINTER_VISIBLE {
        state.pointer_visible = u8::from(val != 0);
    }
    render_cell(state, old_row, old_col);
    let new_row = usize::from(state.pointer_row.min((BUFFER_HEIGHT - 1) as u8));
    let new_col = usize::from(state.pointer_col.min((BUFFER_WIDTH - 1) as u8));
    render_cell(state, new_row, new_col);
}

fn handle_control(state: &mut VgaState, cmd: u8, val: u8) {
    match cmd {
        1 => {
            let bg = state.color_code.bg();
            state.color_code = ColorCode((bg << 4) | (val & 0x0F));
        }
        2 => {
            let fg = state.color_code.fg();
            state.color_code = ColorCode(((val & 0x0F) << 4) | fg);
        }
        3 => {
            let mut row = 1usize;
            while row < BUFFER_HEIGHT {
                clear_row(state, row);
                row += 1;
            }
            set_cursor(state, 1, 0);
        }
        4 => draw_header(
            state,
            " [ GOS v0.2 :: GRAPH MESH CONSOLE ] ",
            Color::Black,
            Color::LightCyan,
        ),
        5 => set_cursor(state, usize::from(val), usize::from(state.column_position)),
        6 => set_cursor(state, usize::from(state.row_position), usize::from(val)),
        7 => clear_screen(state),
        8 => {
            clear_row(state, usize::from(val).min(BUFFER_HEIGHT - 1));
            set_cursor(state, usize::from(val).min(BUFFER_HEIGHT - 1), 0);
        }
        9 => {
            let slot = usize::from(val.min(1));
            state.saved_row[slot] = state.row_position;
            state.saved_col[slot] = state.column_position;
        }
        10 => {
            let slot = usize::from(val.min(1));
            set_cursor(
                state,
                usize::from(state.saved_row[slot]),
                usize::from(state.saved_col[slot]),
            );
        }
        11 => state.scroll_top = val.min((BUFFER_HEIGHT - 1) as u8),
        12 => state.scroll_bottom = val.min((BUFFER_HEIGHT - 1) as u8),
        DISPLAY_CONTROL_POINTER_COL | DISPLAY_CONTROL_POINTER_ROW | DISPLAY_CONTROL_POINTER_VISIBLE => {
            handle_pointer_move(state, cmd, val)
        }
        _ => {}
    }
}

unsafe extern "C" fn vga_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    let base_color = ColorCode::new(Color::LightGreen, Color::Black);
    let mut cells = [ScreenChar::blank(base_color); CELL_COUNT];
    let mut idx = 0usize;
    while idx < CELL_COUNT {
        cells[idx] = ScreenChar::blank(base_color);
        idx += 1;
    }

    unsafe {
        core::ptr::write(
            (*ctx).state_ptr as *mut VgaState,
            VgaState {
                column_position: 0,
                row_position: 0,
                color_code: base_color,
                scroll_top: 2,
                scroll_bottom: (BUFFER_HEIGHT - 1) as u8,
                saved_row: [0; 2],
                saved_col: [0; 2],
                pointer_col: (BUFFER_WIDTH / 2) as u8,
                pointer_row: (BUFFER_HEIGHT / 2) as u8,
                pointer_visible: 0,
                cells,
            },
        );
    }

    let state = unsafe { state_mut(ctx) };
    clear_screen(state);
    ExecStatus::Done
}

unsafe extern "C" fn vga_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let state = unsafe { state_mut(ctx) };
    let signal = packet_to_signal(unsafe { (*event).signal });
    match signal {
        Signal::Data { byte, .. } => write_byte(state, byte),
        Signal::Control { cmd, val } => handle_control(state, cmd, val),
        _ => {}
    }
    ExecStatus::Done
}

unsafe extern "C" fn vga_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
