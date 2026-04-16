#![no_std]


// ============================================================
// GOS KERNEL TOPOLOGY — k-ime
// This Cypher script documents the plugin's place in the kernel graph.
//
// MERGE (p:Plugin {id: "K_IME", name: "k-ime"})
// SET p.executor = "k_ime::EXECUTOR_ID", p.node_type = "Router", p.state_schema = "0x2011"
//
// -- Hardware Resources
//
// -- Exported Capabilities (APIs)
// MERGE (cap_ime_control:Capability {namespace: "ime", name: "control"})
// MERGE (p)-[:EXPORTS]->(cap_ime_control)
//
// -- Imported Capabilities (Dependencies)
// MERGE (cap_shell_input:Capability {namespace: "shell", name: "input"})
// MERGE (p)-[:IMPORTS]->(cap_shell_input)
// ============================================================


use gos_hal::ngr;
use gos_protocol::{
    packet_to_signal, ExecStatus, ExecutorContext, ExecutorId, IME_CONTROL_SET_MODE,
    IME_MODE_ASCII, IME_MODE_ZH_PINYIN, KernelAbi, NodeEvent, NodeExecutorVTable, Signal,
    VectorAddress,
};

pub const NODE_VEC: VectorAddress = VectorAddress::new(6, 3, 0, 0);
pub const EXECUTOR_ID: ExecutorId = ExecutorId::from_ascii("native.ime");
pub const EXECUTOR_VTABLE: NodeExecutorVTable = NodeExecutorVTable {
    executor_id: EXECUTOR_ID,
    on_init: Some(ime_on_init),
    on_event: Some(ime_on_event),
    on_suspend: Some(ime_on_suspend),
    on_resume: None,
    on_teardown: None,
    on_telemetry: None,
};

const MAX_COMPOSING: usize = 24;

#[repr(C)]
struct ImeState {
    shell_target: u64,
    mode: u8,
    composing: [u8; MAX_COMPOSING],
    len: usize,
}

struct CandidateEntry {
    key: &'static [u8],
    choices: &'static [&'static str],
}

static CANDIDATES: &[CandidateEntry] = &[
    CandidateEntry { key: b"ni", choices: &["你", "呢", "尼"] },
    CandidateEntry { key: b"hao", choices: &["好", "号", "浩"] },
    CandidateEntry { key: b"ma", choices: &["吗", "嘛", "马"] },
    CandidateEntry { key: b"wo", choices: &["我", "握", "窝"] },
    CandidateEntry { key: b"men", choices: &["们", "门", "闷"] },
    CandidateEntry { key: b"shi", choices: &["是", "时", "事"] },
    CandidateEntry { key: b"de", choices: &["的", "得", "德"] },
    CandidateEntry { key: b"zai", choices: &["在", "再", "载"] },
    CandidateEntry { key: b"ren", choices: &["人", "仁", "认"] },
    CandidateEntry { key: b"zhong", choices: &["中", "种", "重"] },
    CandidateEntry { key: b"guo", choices: &["国", "过", "果"] },
    CandidateEntry { key: b"wen", choices: &["文", "问", "闻"] },
    CandidateEntry { key: b"ai", choices: &["爱", "艾", "碍"] },
    CandidateEntry { key: b"zhineng", choices: &["智能", "职能", "质能"] },
    CandidateEntry { key: b"zhongwen", choices: &["中文", "重文", "中闻"] },
    CandidateEntry { key: b"xitong", choices: &["系统", "协同", "细统"] },
    CandidateEntry { key: b"shuru", choices: &["输入", "深入", "驶入"] },
    CandidateEntry { key: b"shurufa", choices: &["输入法", "输入阀", "树如法"] },
    CandidateEntry { key: b"ceshi", choices: &["测试", "策士", "测式"] },
];

unsafe fn state_mut(ctx: *mut ExecutorContext) -> &'static mut ImeState {
    let ctx = unsafe { &mut *ctx };
    unsafe { &mut *(ctx.state_ptr as *mut ImeState) }
}

fn abi_from_ctx(ctx: *mut ExecutorContext) -> &'static KernelAbi {
    let ctx_ref = unsafe { &*ctx };
    unsafe { &*ctx_ref.abi }
}

fn clear_composition(state: &mut ImeState) {
    state.composing = [0; MAX_COMPOSING];
    state.len = 0;
}

fn normalize_letter(byte: u8) -> u8 {
    if byte.is_ascii_uppercase() {
        byte + 32
    } else {
        byte
    }
}

fn lookup_candidate(key: &[u8]) -> Option<&'static CandidateEntry> {
    CANDIDATES.iter().find(|entry| entry.key == key)
}

fn is_ascii_punctuation(byte: u8) -> bool {
    matches!(
        byte,
        b'.' | b',' | b';' | b':' | b'!' | b'?' | b'(' | b')' | b'[' | b']' | b'{' | b'}'
            | b'"' | b'\'' | b'-' | b'_' | b'/' | b'\\' | b'@' | b'#' | b'$' | b'%'
            | b'^' | b'&' | b'*' | b'+' | b'='
    )
}

fn post_shell_byte(target: u64, byte: u8) {
    if target == 0 {
        return;
    }
    ngr::post_signal(
        VectorAddress::from_u64(target),
        Signal::Data {
            from: NODE_VEC.as_u64(),
            byte,
        },
    );
}

fn post_shell_bytes(target: u64, bytes: &[u8]) {
    for byte in bytes {
        post_shell_byte(target, *byte);
    }
}

fn commit_selection(state: &mut ImeState, selector: usize) {
    if state.len == 0 {
        return;
    }

    let composing = &state.composing[..state.len];
    if let Some(entry) = lookup_candidate(composing) {
        let index = selector.min(entry.choices.len().saturating_sub(1));
        post_shell_bytes(state.shell_target, entry.choices[index].as_bytes());
    } else {
        post_shell_bytes(state.shell_target, composing);
    }

    clear_composition(state);
}

unsafe extern "C" fn ime_on_init(ctx: *mut ExecutorContext) -> ExecStatus {
    let shell_target = {
        let abi = abi_from_ctx(ctx);
        if let Some(resolve_capability) = abi.resolve_capability {
            unsafe {
                resolve_capability(
                    b"shell".as_ptr(),
                    b"shell".len(),
                    b"input".as_ptr(),
                    b"input".len(),
                )
            }
        } else {
            0
        }
    };

    unsafe {
        core::ptr::write(
            (*ctx).state_ptr as *mut ImeState,
            ImeState {
                shell_target,
                mode: IME_MODE_ASCII,
                composing: [0; MAX_COMPOSING],
                len: 0,
            },
        );
    }
    ExecStatus::Done
}

unsafe extern "C" fn ime_on_event(ctx: *mut ExecutorContext, event: *const NodeEvent) -> ExecStatus {
    let state = unsafe { state_mut(ctx) };
    let signal = packet_to_signal(unsafe { (*event).signal });

    match signal {
        Signal::Control { cmd, val } => {
            if cmd == IME_CONTROL_SET_MODE {
                state.mode = if val == IME_MODE_ZH_PINYIN {
                    IME_MODE_ZH_PINYIN
                } else {
                    IME_MODE_ASCII
                };
                clear_composition(state);
            }
            ExecStatus::Done
        }
        Signal::Data { byte, .. } => {
            if state.mode == IME_MODE_ASCII {
                post_shell_byte(state.shell_target, byte);
                return ExecStatus::Done;
            }

            match byte {
                b'a'..=b'z' | b'A'..=b'Z' => {
                    if state.len < state.composing.len() {
                        state.composing[state.len] = normalize_letter(byte);
                        state.len += 1;
                    }
                }
                0x08 | 0x7F => {
                    if state.len > 0 {
                        state.len -= 1;
                        state.composing[state.len] = 0;
                    }
                }
                0x1B | 0x03 => clear_composition(state),
                b'1'..=b'9' => commit_selection(state, usize::from(byte - b'1')),
                b' ' | b'\n' | b'\r' => commit_selection(state, 0),
                _ if is_ascii_punctuation(byte) => {
                    if state.len > 0 {
                        commit_selection(state, 0);
                    }
                    post_shell_byte(state.shell_target, byte);
                }
                _ => {}
            }
            ExecStatus::Done
        }
        _ => ExecStatus::Done,
    }
}

unsafe extern "C" fn ime_on_suspend(_ctx: *mut ExecutorContext) -> ExecStatus {
    ExecStatus::Done
}
