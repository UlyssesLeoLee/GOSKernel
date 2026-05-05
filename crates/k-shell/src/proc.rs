// ============================================================
// k-shell :: proc — shell processing stage
//
// MERGE (proc:Module {id: "k_shell::proc", name: "proc"})
// MERGE (lib:Module {id: "k_shell::lib", name: "lib"})
// MERGE (proc)-[:STAGE_OF]->(lib)
// MERGE (proc)-[:CONSUMES]->(:Struct {name: "Input"})
// MERGE (proc)-[:PRODUCES]->(:Struct {name: "Output"})
// ============================================================

use gos_protocol::{
    ExecutorContext, ExecStatus,
    AI_CONTROL_CHAT_BEGIN, AI_CONTROL_CHAT_COMMIT,
    CHAT_CONTROL_KEY_BEGIN, CHAT_CONTROL_KEY_COMMIT,
    CHAT_CONTROL_MODEL_BEGIN, CHAT_CONTROL_MODEL_COMMIT,
    CHAT_CONTROL_API_TYPE, CHAT_CONTROL_HTTP_TOGGLE,
    CUDA_CONTROL_REPORT, CUDA_CONTROL_RESET,
    IME_MODE_ASCII, IME_MODE_ZH_PINYIN,
    NET_CONTROL_PING, NET_CONTROL_PROBE, NET_CONTROL_REPORT, NET_CONTROL_RESET,
    NIM_CONTROL_SEND, NIM_CONTROL_EXIT,
    NIM_CONTROL_MODEL_BEGIN, NIM_CONTROL_MODEL_COMMIT,
    NIM_CONTROL_PORT_BEGIN, NIM_CONTROL_PORT_COMMIT,
    NIM_CONTROL_CLEAR_HISTORY,
    RuntimeEdgeType,
};

use super::{
    pre::{DataSource, Input},
    CLIPBOARD_NODE_VEC, GRAPH_MODE_NONE, LIVE_SIGIL_FRAMES, MENU_MODE_AI_API, MENU_MODE_COMMAND,
    AI_PANEL_LINE_WIDTH, COMMAND_SCROLL_TOP,
};

// ---------------------------------------------------------------------------
// Output — what post::emit receives.
// ---------------------------------------------------------------------------
pub struct Output {
    pub status: ExecStatus,
}

// ---------------------------------------------------------------------------
// process — the full shell business logic.
//
// Handles key routing, IME composition, command history, graph navigation,
// command dispatch, and all rendering side-effects.  Returns an Output
// carrying the ExecStatus that post::emit should forward to the kernel.
// ---------------------------------------------------------------------------
pub unsafe fn process(ctx: *mut ExecutorContext, input: Input) -> Option<Output> {
    let sink = super::sink_from_ctx(ctx);
    let state = unsafe { super::state_mut(ctx) };

    let status = match input {
        // -----------------------------------------------------------------------
        // Spawn — play boot cinema and activate the live console.
        // -----------------------------------------------------------------------
        Input::Spawn => {
            super::play_boot_sequence(&sink);
            super::redraw_console(&sink, state);
            state.console_live = 1;
            ExecStatus::Done
        }

        // -----------------------------------------------------------------------
        // Heartbeat — tick the animated header / sigil / operator band.
        // -----------------------------------------------------------------------
        Input::Heartbeat => {
            state.heartbeat_divider = state.heartbeat_divider.wrapping_add(1);
            state.sigil_frame = (state.sigil_frame + 1) % LIVE_SIGIL_FRAMES as u8;
            super::save_cursor(&sink, 0);
            let snapshot = gos_runtime::snapshot();
            super::draw_runtime_header(&sink, state, snapshot);
            super::draw_runtime_gap_flux(&sink, state);
            super::draw_console_sigil(&sink, state.sigil_frame as usize);
            super::draw_ai_panel(&sink, state);
            super::draw_operator_band(&sink, state, snapshot);
            if state.heartbeat_divider % 4 == 0 {
                super::draw_command_deck_panel(&sink, state, snapshot);
                super::redraw_footer(&sink, state, false);
            }
            super::restore_cursor(&sink, 0);
            ExecStatus::Done
        }

        // -----------------------------------------------------------------------
        // Other — no-op.
        // -----------------------------------------------------------------------
        Input::Other => ExecStatus::Done,

        // -----------------------------------------------------------------------
        // Data — route by source then process the byte.
        // -----------------------------------------------------------------------
        Input::Data { source, byte } => {
            process_data(&sink, state, source, byte)
        }
    };

    Some(Output { status })
}

// ---------------------------------------------------------------------------
// process_data — inner dispatcher for Signal::Data bytes.
// ---------------------------------------------------------------------------
fn process_data(
    sink: &super::ConsoleSink,
    state: &mut super::ShellState,
    source: DataSource,
    byte: u8,
) -> ExecStatus {
    // --- IME node forwarded a composed character ---------------------------------
    if source == DataSource::Ime {
        if state.menu_mode == MENU_MODE_COMMAND {
            super::append_command_byte(sink, state, byte, true);
        }
        return ExecStatus::Done;
    }

    // --- Clipboard paste byte ---------------------------------------------------
    if source == DataSource::Clipboard {
        super::append_clipboard_byte(sink, state, byte);
        return ExecStatus::Done;
    }

    // --- AI streaming token -----------------------------------------------------
    if source == DataSource::Ai {
        super::append_ai_stream_byte(state, byte);
        super::redraw_ai_panel(sink, state, true);
        return ExecStatus::Done;
    }

    // --- Keyboard input ---------------------------------------------------------
    // PgUp / PgDn  (graph page navigation)
    if super::handle_graph_page_key(sink, state, byte) {
        return ExecStatus::Done;
    }

    // Up / Down  (command history)
    if super::handle_command_history_key(sink, state, byte) {
        return ExecStatus::Done;
    }

    // Ctrl+A — enter AI API key editor
    if byte == 0x01 && state.menu_mode != MENU_MODE_AI_API {
        super::enter_ai_api_mode(sink, state);
        return ExecStatus::Done;
    }

    // --- AI API key editor mode -------------------------------------------------
    if state.menu_mode == MENU_MODE_AI_API {
        return process_api_editor(sink, state, byte);
    }

    // Ctrl+L — toggle input language (ASCII / zh-pinyin)
    if byte == 0x0C {
        let next_lang = if state.input_lang == IME_MODE_ZH_PINYIN {
            IME_MODE_ASCII
        } else {
            IME_MODE_ZH_PINYIN
        };
        if super::sync_input_lang(sink, state, next_lang) {
            super::redraw_footer(sink, state, true);
        } else {
            super::restore_output_cursor(sink);
            super::set_color(sink, 12, 0);
            super::print_str(sink, "\n ime node unresolved\n");
            super::save_output_cursor(sink);
            super::redraw_footer(sink, state, false);
        }
        return ExecStatus::Done;
    }

    // --- zh-pinyin IME composition ----------------------------------------------
    if state.input_lang == IME_MODE_ZH_PINYIN {
        if let Some(status) = process_pinyin(sink, state, byte) {
            return status;
        }
    }

    // --- Enter / Return — execute the buffered command --------------------------
    if byte == b'\n' || byte == b'\r' {
        return process_enter(sink, state);
    }

    // --- Remaining single-byte control / printable keys -------------------------
    match byte {
        0x03 => { let _ = super::clipboard_copy_active_input(sink, state); }
        0x16 => { let _ = super::clipboard_paste_active_input(sink, state); }
        0x18 => { let _ = super::clipboard_cut_active_input(sink, state); }
        0x08 | 0x7F => {
            if super::command_pop_scalar(state) {
                super::reset_command_history_cursor(state);
                super::redraw_footer(sink, state, false);
            }
        }
        byte if byte >= 0x20 => {
            super::append_command_byte(sink, state, byte, false);
        }
        _ => {}
    }
    ExecStatus::Done
}

// ---------------------------------------------------------------------------
// process_api_editor — handle keystrokes while in the AI API key editor.
// ---------------------------------------------------------------------------
fn process_api_editor(
    sink: &super::ConsoleSink,
    state: &mut super::ShellState,
    byte: u8,
) -> ExecStatus {
    match byte {
        0x03 => {
            let _ = super::clipboard_copy_active_input(sink, state);
        }
        0x16 => {
            let _ = super::clipboard_paste_active_input(sink, state);
        }
        0x18 => {
            let _ = super::clipboard_cut_active_input(sink, state);
        }
        b'\n' | b'\r' | 0x13 => {
            if super::commit_ai_api(sink, state) {
                super::exit_ai_api_mode(sink, state, " ai uplink armed for this boot session", 10);
            } else {
                state.api_configured = 0;
                super::exit_ai_api_mode(sink, state, " ai uplink commit failed", 12);
            }
        }
        0x1B => {
            super::exit_ai_api_mode(sink, state, " ai uplink edit cancelled", 14);
        }
        0x08 | 0x7F => {
            if state.api_edit_len > 0 {
                super::reset_command_history_cursor(state);
                state.api_edit_len -= 1;
                state.api_buffer[state.api_edit_len] = 0;
            }
            super::redraw_footer(sink, state, false);
        }
        0x20..=0x7E => {
            if state.api_edit_len < state.api_buffer.len() {
                state.api_buffer[state.api_edit_len] = byte;
                state.api_edit_len += 1;
            }
            super::redraw_footer(sink, state, false);
        }
        _ => {}
    }
    ExecStatus::Done
}

// ---------------------------------------------------------------------------
// process_pinyin — handle one keystroke during zh-pinyin composition.
//
// Returns Some(status) if the byte was consumed by the IME layer, or None
// to fall through to normal command processing.
// ---------------------------------------------------------------------------
fn process_pinyin(
    sink: &super::ConsoleSink,
    state: &mut super::ShellState,
    byte: u8,
) -> Option<ExecStatus> {
    use gos_protocol::Signal;

    match byte {
        b'a'..=b'z' | b'A'..=b'Z' => {
            if state.ime_preview_len < state.ime_preview.len() {
                state.ime_preview[state.ime_preview_len] = byte.to_ascii_lowercase();
                state.ime_preview_len += 1;
                let _ = super::emit_target_signal(
                    sink,
                    state.ime_target,
                    Signal::Data { from: sink.from, byte },
                );
                super::redraw_footer(sink, state, true);
            }
            Some(ExecStatus::Done)
        }
        0x08 | 0x7F => {
            if state.ime_preview_len > 0 {
                state.ime_preview_len -= 1;
                state.ime_preview[state.ime_preview_len] = 0;
                let _ = super::emit_target_signal(
                    sink,
                    state.ime_target,
                    Signal::Data { from: sink.from, byte: 0x08 },
                );
                super::redraw_footer(sink, state, true);
                Some(ExecStatus::Done)
            } else {
                None
            }
        }
        0x1B | 0x03 => {
            if state.ime_preview_len > 0 {
                let _ = super::emit_target_signal(
                    sink,
                    state.ime_target,
                    Signal::Data { from: sink.from, byte: 0x1B },
                );
                super::clear_ime_preview(state);
                super::redraw_footer(sink, state, true);
                Some(ExecStatus::Done)
            } else {
                None
            }
        }
        b'1'..=b'9' => {
            if state.ime_preview_len > 0 {
                super::commit_ime_preview(sink, state, byte);
                super::redraw_footer(sink, state, true);
                Some(ExecStatus::Done)
            } else {
                None
            }
        }
        b' ' => {
            if state.ime_preview_len > 0 {
                super::commit_ime_preview(sink, state, b' ');
                super::redraw_footer(sink, state, true);
                Some(ExecStatus::Done)
            } else {
                None
            }
        }
        b'\n' | b'\r' => {
            if state.ime_preview_len > 0 {
                super::commit_ime_preview(sink, state, b'\n');
                super::redraw_footer(sink, state, true);
                Some(ExecStatus::Done)
            } else {
                None
            }
        }
        _ if super::is_ascii_punctuation(byte) && state.ime_preview_len > 0 => {
            let _ = super::emit_target_signal(
                sink,
                state.ime_target,
                Signal::Data { from: sink.from, byte },
            );
            super::clear_ime_preview(state);
            super::redraw_footer(sink, state, true);
            Some(ExecStatus::Done)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// process_enter — execute the buffered command line.
// ---------------------------------------------------------------------------
fn process_enter(
    sink: &super::ConsoleSink,
    state: &mut super::ShellState,
) -> ExecStatus {
    // ── Chat mode: route Enter to k-chat instead of shell dispatch ────────────
    use gos_protocol::Signal;
    if super::CHAT_MODE.load(core::sync::atomic::Ordering::SeqCst) == 1 {
        let cmd_len = state.len.min(state.buffer.len());
        let mut tmp = [0u8; 128];
        tmp[..cmd_len].copy_from_slice(&state.buffer[..cmd_len]);
        let cmd = core::str::from_utf8(&tmp[..cmd_len]).unwrap_or("").trim();
        if cmd == "exit" || cmd == "quit" || cmd == ":q" {
            // Exit chat mode
            super::CHAT_MODE.store(0, core::sync::atomic::Ordering::SeqCst);
            let chat_target = super::CHAT_TARGET.load(core::sync::atomic::Ordering::SeqCst);
            super::emit_target_signal_raw(
                sink.abi,
                chat_target,
                Signal::Control { cmd: super::CHAT_CONTROL_EXIT, val: 0 },
            );
            state.len = 0;
            super::redraw_console(sink, state);
            return ExecStatus::Done;
        }
        // Forward each byte then CHAT_CONTROL_SEND
        if state.len > 0 {
            let chat_target = super::CHAT_TARGET.load(core::sync::atomic::Ordering::SeqCst);
            for i in 0..cmd_len {
                super::emit_target_signal_raw(
                    sink.abi,
                    chat_target,
                    Signal::Data { from: super::NODE_VEC.as_u64(), byte: state.buffer[i] },
                );
            }
            super::emit_target_signal_raw(
                sink.abi,
                chat_target,
                Signal::Control { cmd: super::CHAT_CONTROL_SEND, val: 0 },
            );
        }
        state.len = 0;
        // Re-draw the chat input prompt (k-chat's post already printed the response)
        super::set_color(sink, 14, 0);
        super::print_str(sink, "You ▸ ");
        super::set_color(sink, 7, 0);
        return ExecStatus::Done;
    }

    // ── NIM mode: route Enter to k-nim ──────────────────────────────────────
    if super::NIM_MODE.load(core::sync::atomic::Ordering::SeqCst) == 1 {
        let cmd_len = state.len.min(state.buffer.len());
        let mut tmp = [0u8; 128];
        tmp[..cmd_len].copy_from_slice(&state.buffer[..cmd_len]);
        let cmd = core::str::from_utf8(&tmp[..cmd_len]).unwrap_or("").trim();
        if cmd == "exit" || cmd == "quit" || cmd == ":q" {
            super::NIM_MODE.store(0, core::sync::atomic::Ordering::SeqCst);
            let nim_target = super::NIM_TARGET.load(core::sync::atomic::Ordering::SeqCst);
            super::emit_target_signal_raw(
                sink.abi,
                nim_target,
                Signal::Control { cmd: NIM_CONTROL_EXIT, val: 0 },
            );
            state.len = 0;
            super::redraw_console(sink, state);
            return ExecStatus::Done;
        }
        if state.len > 0 {
            let nim_target = super::NIM_TARGET.load(core::sync::atomic::Ordering::SeqCst);
            for i in 0..cmd_len {
                super::emit_target_signal_raw(
                    sink.abi,
                    nim_target,
                    Signal::Data { from: super::NODE_VEC.as_u64(), byte: state.buffer[i] },
                );
            }
            super::emit_target_signal_raw(
                sink.abi,
                nim_target,
                Signal::Control { cmd: NIM_CONTROL_SEND, val: 0 },
            );
        }
        state.len = 0;
        super::set_color(sink, 14, 0); // yellow
        super::print_str(sink, "You \u{25B8} "); // "You ▸ "
        super::set_color(sink, 7, 0);
        return ExecStatus::Done;
    }

    let cmd_len = state.len.min(state.buffer.len());
    let mut cmd_buf = [0u8; 128];
    cmd_buf[..cmd_len].copy_from_slice(&state.buffer[..cmd_len]);
    let cmd = core::str::from_utf8(&cmd_buf[..cmd_len]).unwrap_or("");

    if !cmd.is_empty() {
        super::record_command_history(state);
    }

    if super::handle_graph_command(sink, state, cmd) {
        return ExecStatus::Done;
    }

    if state.graph_mode != GRAPH_MODE_NONE {
        super::clear_graph_nav(state);
        state.graph_mode = GRAPH_MODE_NONE;
        state.graph_offset = 0;
        state.graph_total = 0;
        super::clear_command_area(sink);
        super::goto(sink, COMMAND_SCROLL_TOP, 4);
        super::save_output_cursor(sink);
    }

    super::restore_output_cursor(sink);
    super::echo_command_line(sink, state);

    dispatch_text_command(sink, state, cmd);

    super::save_output_cursor(sink);
    state.len = 0;
    super::redraw_footer(sink, state, false);
    ExecStatus::Done
}

// ---------------------------------------------------------------------------
// dispatch_text_command — match the typed command string and execute it.
// ---------------------------------------------------------------------------
fn dispatch_text_command(
    sink: &super::ConsoleSink,
    state: &mut super::ShellState,
    cmd: &str,
) {
    use gos_protocol::Signal;

    if cmd == "cypher" {
        super::set_color(sink, 11, 0);
        super::print_str(sink, " cypher usage\n");
        super::set_color(sink, 7, 0);
        super::print_str(sink, "  cypher MATCH (n) RETURN n\n");
        super::print_str(sink, "  cypher MATCH (n {vector:'6.1.0.0'}) CALL activate(n)\n");
        super::print_str(sink, "  cypher MATCH ()-[e {vector:'e:6.1.0.0'}]-() CALL route(e)\n");
    } else if let Some(query) = cmd.strip_prefix("cypher ") {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " empty cypher query\n");
        } else {
            let _ = super::dispatch_cypher_query(sink, state, trimmed);
        }
    } else if super::looks_like_cypher_query(cmd) {
        let _ = super::dispatch_cypher_query(sink, state, cmd.trim());
    } else if cmd == "help" {
        super::set_color(sink, 11, 0);
        super::print_str(sink, " command index\n");
        super::set_color(sink, 7, 0);
        super::print_str(sink, "  help    show commands\n");
        super::print_str(sink, "  info    runtime snapshot\n");
        super::print_str(sink, "  graph   graph counters\n");
        super::print_str(sink, "  show    overview, or toggle node/edge context\n");
        super::print_str(sink, "  back    return to the previous graph view\n");
        super::print_str(sink, "  node <vector>  select/show one node\n");
        super::print_str(sink, "  edge <vector>  select/show one edge\n");
        super::print_str(sink, "  PgUp/PgDn  page graph overview/lists\n");
        super::print_str(sink, "  where   show current graph selection\n");
        super::print_str(sink, "  select clear  clear node/edge selection\n");
        super::print_str(sink, "  activate  activate selected node\n");
        super::print_str(sink, "  spawn     spawn selected node\n");
        super::print_str(sink, "  Up/Down   browse previous command history\n");
        super::print_str(sink, "  cypher <query>  send cypher v1 query into graph node\n");
        super::print_str(sink, "  MATCH ...       direct cypher entry without prefix\n");
        super::print_str(sink, "  net / net status  print uplink status\n");
        super::print_str(sink, "  net probe         rescan pci and refresh nic state\n");
        super::print_str(sink, "  net reset         re-init nic registers and report\n");
        super::print_str(sink, "  net ping / ping   ICMP echo to qemu gateway (10.0.2.2)\n");
        super::print_str(sink, "  cuda / cuda status  print host bridge status\n");
        super::print_str(sink, "  cuda submit <job>   submit one host-backed cuda job\n");
        super::print_str(sink, "  cuda demo           send a sample saxpy-style job\n");
        super::print_str(sink, "  cuda reset          clear bridge counters and capture state\n");
        super::print_str(sink, "  clipboard          show clipboard.mount node and mount edges\n");
        super::print_str(sink, "  clipboard clear    clear shared clipboard buffer\n");
        super::print_str(sink, "  clipboard mount <vector>    add node -[mount]-> clipboard.mount\n");
        super::print_str(sink, "  clipboard unmount <vector>  remove node -[mount]-> clipboard.mount\n");
        super::print_str(sink, "  theme              show theme.current and its active use edge\n");
        super::print_str(sink, "  theme wabi         repoint theme.current -> theme.wabi\n");
        super::print_str(sink, "  theme shoji        repoint theme.current -> theme.shoji\n");
        super::print_str(sink, "  chat    enter AI chat mode (type 'exit' to quit)\n");
        super::print_str(sink, "  chat key <k>     set AI API key for current session\n");
        super::print_str(sink, "  chat model <m>   set model name (e.g. qwen2.5:7b)\n");
        super::print_str(sink, "  chat api <type>  set backend: ollama | openai | anthropic\n");
        super::print_str(sink, "  chat http        toggle direct TCP mode (Ollama at 10.0.2.2)\n");
        super::print_str(sink, "  chat status      show current chat configuration\n");
        super::print_str(sink, "  nim     enter NIM inference mode (type 'exit' to quit)\n");
        super::print_str(sink, "  nim model <m>    set NIM model (e.g. meta/llama-3.1-8b-instruct)\n");
        super::print_str(sink, "  nim port <n>     set NIM host port (default 8000)\n");
        super::print_str(sink, "  nim clear        clear NIM conversation history\n");
        super::print_str(sink, "  nim status       show current NIM configuration\n");
        super::print_str(sink, "  ai      open bottom ai api editor\n");
        super::print_str(sink, "  ask     send prompt into ai chat lane\n");
        super::print_str(sink, "  ^C/^X/^V copy, cut, paste active input through clipboard.mount\n");
        super::print_str(sink, "  ctrl+l  toggle input language en/zh-py\n");
        super::print_str(sink, "  mem     physical memory + supervisor domain stats\n");
        super::print_str(sink, "  ps      list loaded modules and lifecycle state\n");
        super::print_str(sink, "  caps    list published capabilities\n");
        super::print_str(sink, "  instances  list spawned node instances\n");
        super::print_str(sink, "  log        show recent kernel log entries\n");
        super::print_str(sink, "  log clear  clear the log ring buffer\n");
        super::print_str(sink, "  cpu        show CPU brand, features, and topology\n");
        super::print_str(sink, "  tick       show uptime and scheduler counters\n");
        super::print_str(sink, "  events     show signal dispatch and fault event counters\n");
        super::print_str(sink, "  health     show module health, faults, and restart counts\n");
        super::print_str(sink, "  clear   redraw command deck\n");
        super::print_str(sink, "  splash  replay boot cinema\n");
    } else if cmd == "info" || cmd == "graph" {
        let snapshot = gos_runtime::snapshot();
        super::set_color(sink, 10, 0);
        super::print_str(sink, " runtime snapshot\n");
        super::set_color(sink, 7, 0);
        super::print_str(sink, "  plugins: ");
        super::print_num_inline(sink, snapshot.plugin_count);
        super::print_str(sink, "  nodes: ");
        super::print_num_inline(sink, snapshot.node_count);
        super::print_str(sink, "  edges: ");
        super::print_num_inline(sink, snapshot.edge_count);
        super::print_str(sink, "\n  ready: ");
        super::print_num_inline(sink, snapshot.ready_queue_len);
        super::print_str(sink, "  signals: ");
        super::print_num_inline(sink, snapshot.signal_queue_len);
        super::print_str(sink, "  ctrl: ");
        super::print_num_inline(sink, snapshot.control_queue_len);
        super::print_str(sink, "  stable: ");
        super::print_str(sink, if gos_runtime::is_stable() { "yes" } else { "no" });
        super::print_str(sink, "  tick: ");
        super::print_num_inline(sink, snapshot.tick as usize);
        super::print_str(sink, "\n  net: ");
        super::print_str(sink, if state.net_target == 0 { "unresolved" } else { "ctl-ready" });
        if state.net_target != 0 {
            super::print_str(sink, "  path: qemu nic -> nat -> host wifi  cmds: net/net probe/net reset");
        }
        super::print_str(sink, "\n  ai: ");
        super::print_str(sink, if state.ai_target == 0 { "offline" } else { "online" });
        super::print_str(sink, "  cypher: ");
        super::print_str(sink, if state.cypher_target == 0 { "offline" } else { "online" });
        super::print_str(sink, "  cuda: ");
        super::print_str(sink, if state.cuda_target == 0 { "offline" } else { "online" });
        super::print_str(sink, "  clip: ");
        super::print_str(sink, if super::clipboard_mounted(super::NODE_VEC) { "mounted" } else { "detached" });
        super::print_str(sink, "  bytes: ");
        super::print_num_inline(sink, super::clipboard_len());
        super::print_str(sink, "  api-key: ");
        super::print_str(sink, if state.api_configured != 0 { "armed" } else { "empty" });
        super::print_str(sink, "  bytes: ");
        super::print_num_inline(sink, state.api_len);
        super::print_str(sink, "\n  theme: ");
        let theme = super::selected_theme();
        super::print_str(sink, super::theme_name(theme));
        super::print_str(sink, "  theme-node: ");
        let mut current_line = super::LineBuf::<20>::new();
        current_line.push_vector(super::THEME_CURRENT_NODE_VEC);
        super::print_str(sink, core::str::from_utf8(current_line.as_slice()).unwrap_or("set"));
        super::print_str(sink, "\n  use-> ");
        let mut theme_line = super::LineBuf::<20>::new();
        theme_line.push_vector(super::theme_vector(theme));
        super::print_str(sink, core::str::from_utf8(theme_line.as_slice()).unwrap_or("set"));
        super::print_str(sink, "\n  lang: ");
        super::print_str(sink, super::ime_mode_label(state.input_lang));
        super::print_str(sink, "  ime-preview: ");
        super::print_num_inline(sink, state.ime_preview_len);
        super::print_str(sink, "\n  graph-mode: ");
        super::print_str(sink, super::graph_mode_label(state.graph_mode));
        super::print_str(sink, "  selected-node: ");
        if let Some(vector) = state.selected_node {
            let mut line = super::LineBuf::<24>::new();
            line.push_vector(vector);
            super::print_str(sink, core::str::from_utf8(line.as_slice()).unwrap_or("set"));
        } else {
            super::print_str(sink, "none");
        }
        super::print_str(sink, "\n");
    } else if cmd == "theme" || cmd == "themes" || cmd == "theme list" {
        let theme = super::selected_theme();
        super::set_color(sink, 11, 0);
        super::print_str(sink, " terminal themes\n");
        super::set_color(sink, 7, 0);
        super::print_str(sink, "  active: ");
        super::print_str(sink, super::theme_name(theme));
        super::print_str(sink, "  edge: theme.current -[use]-> ");
        let mut active_line = super::LineBuf::<20>::new();
        active_line.push_vector(super::theme_vector(theme));
        super::print_str(sink, core::str::from_utf8(active_line.as_slice()).unwrap_or("set"));
        super::print_str(sink, "\n  ");
        let mut current = super::LineBuf::<20>::new();
        current.push_vector(super::THEME_CURRENT_NODE_VEC);
        super::print_str(sink, core::str::from_utf8(current.as_slice()).unwrap_or("6.1.3.0"));
        super::print_str(sink, "  theme.current active theme state\n  ");
        let mut wabi = super::LineBuf::<20>::new();
        wabi.push_vector(super::THEME_WABI_NODE_VEC);
        super::print_str(sink, core::str::from_utf8(wabi.as_slice()).unwrap_or("6.1.1.0"));
        super::print_str(sink, "  theme.wabi  quiet ink / tea / moss\n  ");
        let mut shoji = super::LineBuf::<20>::new();
        shoji.push_vector(super::THEME_SHOJI_NODE_VEC);
        super::print_str(sink, core::str::from_utf8(shoji.as_slice()).unwrap_or("6.1.2.0"));
        super::print_str(sink, "  theme.shoji paper / indigo / brass\n");
    } else if let Some(selector) = cmd.strip_prefix("theme ") {
        if let Some(theme) = super::parse_theme_selector(selector) {
            if super::apply_theme_choice(sink, theme) {
                super::set_color(sink, 11, 0);
                super::print_str(sink, " theme switched -> ");
                super::set_color(sink, 15, 0);
                super::print_str(sink, super::theme_name(theme));
                super::print_str(sink, "  edge theme.current -[use]-> ");
                let mut line = super::LineBuf::<20>::new();
                line.push_vector(super::theme_vector(theme));
                super::print_str(sink, core::str::from_utf8(line.as_slice()).unwrap_or("set"));
                super::print_str(sink, "\n");
            } else {
                super::set_color(sink, 12, 0);
                super::print_str(sink, " theme switch failed\n");
            }
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " unknown theme, use: theme wabi | theme shoji\n");
        }
    } else if cmd == "clipboard" || cmd == "clip" || cmd == "clipboard status" {
        let mut edges = [gos_protocol::GraphEdgeSummary::EMPTY; 12];
        let (_, returned) =
            gos_runtime::edge_page_for_node(CLIPBOARD_NODE_VEC, 0, &mut edges).unwrap_or((0, 0));
        super::set_color(sink, 11, 0);
        super::print_str(sink, " clipboard.mount\n");
        super::set_color(sink, 7, 0);
        super::print_str(sink, "  vector: ");
        let mut node_line = super::LineBuf::<20>::new();
        node_line.push_vector(CLIPBOARD_NODE_VEC);
        super::print_str(sink, core::str::from_utf8(node_line.as_slice()).unwrap_or("6.1.4.0"));
        super::print_str(sink, "\n  bytes: ");
        super::print_num_inline(sink, super::clipboard_len());
        super::print_str(sink, "\n  mounts:\n");
        let mut listed = 0usize;
        for summary in edges.iter().take(returned) {
            if summary.edge_type != RuntimeEdgeType::Mount
                || summary.to_vector != CLIPBOARD_NODE_VEC
            {
                continue;
            }
            super::print_str(sink, "    ");
            let mut line = super::LineBuf::<24>::new();
            line.push_vector(summary.from_vector);
            super::print_str(sink, core::str::from_utf8(line.as_slice()).unwrap_or("node"));
            super::print_str(sink, "  ");
            super::print_str(sink, summary.from_key);
            super::print_str(sink, "\n");
            listed += 1;
        }
        if listed == 0 {
            super::print_str(sink, "    none\n");
        }
    } else if cmd == "clipboard clear" || cmd == "clip clear" {
        if super::clipboard_clear(sink, state.clipboard_target) {
            super::set_color(sink, 11, 0);
            super::print_str(sink, " clipboard cleared\n");
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " clipboard clear failed\n");
        }
    } else if let Some(selector) = cmd
        .strip_prefix("clipboard mount ")
        .or_else(|| cmd.strip_prefix("clip mount "))
    {
        if let Some(vector) = super::parse_clipboard_vector(selector) {
            if super::sync_clipboard_mount_for_vector(vector, true) {
                super::set_color(sink, 11, 0);
                super::print_str(sink, " clipboard mounted <- ");
                let mut line = super::LineBuf::<20>::new();
                line.push_vector(vector);
                super::print_str(sink, core::str::from_utf8(line.as_slice()).unwrap_or("set"));
                super::print_str(sink, "\n");
            } else {
                super::set_color(sink, 12, 0);
                super::print_str(sink, " clipboard mount failed\n");
            }
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " clipboard mount requires node vector\n");
        }
    } else if let Some(selector) = cmd
        .strip_prefix("clipboard unmount ")
        .or_else(|| cmd.strip_prefix("clip unmount "))
    {
        if let Some(vector) = super::parse_clipboard_vector(selector) {
            if super::sync_clipboard_mount_for_vector(vector, false) {
                super::set_color(sink, 11, 0);
                super::print_str(sink, " clipboard unmounted <- ");
                let mut line = super::LineBuf::<20>::new();
                line.push_vector(vector);
                super::print_str(sink, core::str::from_utf8(line.as_slice()).unwrap_or("set"));
                super::print_str(sink, "\n");
            } else {
                super::set_color(sink, 12, 0);
                super::print_str(sink, " clipboard unmount failed\n");
            }
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " clipboard unmount requires node vector\n");
        }
    } else if cmd == "net" || cmd == "net status" || cmd == "uplink" {
        if super::emit_target_signal(
            sink,
            state.net_target,
            Signal::Control { cmd: NET_CONTROL_REPORT, val: 0 },
        ) {
            super::set_color(sink, 11, 0);
            super::print_str(sink, " net status requested\n");
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " net uplink unresolved\n");
        }
    } else if cmd == "net probe" {
        if super::emit_target_signal(
            sink,
            state.net_target,
            Signal::Control { cmd: NET_CONTROL_PROBE, val: 0 },
        ) {
            super::set_color(sink, 11, 0);
            super::print_str(sink, " net reprobe dispatched\n");
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " net uplink unresolved\n");
        }
    } else if cmd == "net reset" {
        if super::emit_target_signal(
            sink,
            state.net_target,
            Signal::Control { cmd: NET_CONTROL_RESET, val: 0 },
        ) {
            super::set_color(sink, 11, 0);
            super::print_str(sink, " net reset dispatched\n");
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " net uplink unresolved\n");
        }
    } else if cmd == "net ping" || cmd == "ping" {
        if super::emit_target_signal(
            sink,
            state.net_target,
            Signal::Control { cmd: NET_CONTROL_PING, val: 0 },
        ) {
            gos_runtime::pump();
            super::set_color(sink, 11, 0);
            super::print_str(sink, " pinging 10.0.2.2 (qemu gateway)...\n");
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " net uplink unresolved\n");
        }
    } else if cmd == "cuda" || cmd == "cuda status" || cmd == "gpu" || cmd == "gpu status" {
        if super::emit_target_signal(
            sink,
            state.cuda_target,
            Signal::Control { cmd: CUDA_CONTROL_REPORT, val: 0 },
        ) {
            gos_runtime::pump();
            super::set_color(sink, 11, 0);
            super::print_str(sink, " cuda bridge status requested\n");
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " cuda bridge unresolved\n");
        }
    } else if cmd == "cuda reset" {
        if super::emit_target_signal(
            sink,
            state.cuda_target,
            Signal::Control { cmd: CUDA_CONTROL_RESET, val: 0 },
        ) {
            gos_runtime::pump();
            super::set_color(sink, 11, 0);
            super::print_str(sink, " cuda bridge reset dispatched\n");
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " cuda bridge unresolved\n");
        }
    } else if cmd == "cuda demo" {
        let _ = super::dispatch_cuda_submit(
            sink,
            state,
            "kernel=saxpy grid=120 block=256 bytes=4096 dtype=f32",
        );
    } else if let Some(job) = cmd.strip_prefix("cuda submit ") {
        let trimmed = job.trim();
        if trimmed.is_empty() {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " empty cuda job\n");
        } else {
            let _ = super::dispatch_cuda_submit(sink, state, trimmed);
        }
    } else if cmd == "chat" {
        // Enter interactive AI chat mode via the COM2 bridge.
        let chat_target = super::CHAT_TARGET.load(core::sync::atomic::Ordering::SeqCst);
        if chat_target == 0 {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " [chat] k-chat node not available\n");
            super::set_color(sink, 7, 0);
            super::print_str(sink, "   Start tools/chat-bridge.py on the host, then restart.\n");
        } else {
            super::CHAT_MODE.store(1, core::sync::atomic::Ordering::SeqCst);
            // Draw chat banner via VGA
            super::set_color(sink, 0, 11);  // black on cyan
            super::print_str(sink, "  GOS CHAT — AI Bridge                                                          ");
            super::set_color(sink, 8, 0);
            super::print_str(sink, "  Type a message + Enter  |  'exit' to return to shell                          \n");
            super::set_color(sink, 7, 0);
            super::print_str(sink, "\n");
            super::set_color(sink, 14, 0); // yellow
            super::print_str(sink, "You ▸ ");
            super::set_color(sink, 7, 0);
        }
        state.len = 0;
    } else if let Some(key_str) = cmd.strip_prefix("chat key ") {
        // chat key <api-key>  — stream the API key into k-chat
        dispatch_chat_key(sink, state, key_str.trim().as_bytes());
    } else if let Some(model_str) = cmd.strip_prefix("chat model ") {
        // chat model <model>  — set the direct-HTTP model name in k-chat
        dispatch_chat_model(sink, state, model_str.trim().as_bytes());
    } else if let Some(api_str) = cmd.strip_prefix("chat api ") {
        // chat api <ollama|openai|anthropic>  — set the API backend
        dispatch_chat_api(sink, state, api_str.trim());
    } else if cmd == "chat http" {
        // chat http  — toggle direct TCP/HTTP mode (bypasses COM2 bridge)
        dispatch_chat_http_toggle(sink, state);
    } else if cmd == "chat status" || cmd == "chat info" {
        // chat status  — display current chat configuration
        dispatch_chat_status(sink, state);
    } else if cmd == "nim" {
        // Enter interactive NIM inference mode.
        let nim_target = super::NIM_TARGET.load(core::sync::atomic::Ordering::SeqCst);
        if nim_target == 0 {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " [nim] k-nim node not available\n");
            super::set_color(sink, 7, 0);
        } else {
            super::NIM_MODE.store(1, core::sync::atomic::Ordering::SeqCst);
            super::set_color(sink, 0, 13); // black on magenta
            super::print_str(sink, "  GOS NIM -- NVIDIA NIM / OpenAI-compatible inference                            ");
            super::set_color(sink, 8, 0);
            super::print_str(sink, "  Type a message + Enter  |  'exit' to return to shell                          \n");
            super::set_color(sink, 7, 0);
            super::print_str(sink, "\n");
            super::set_color(sink, 14, 0); // yellow
            super::print_str(sink, "You \u{25B8} "); // "You ▸ "
            super::set_color(sink, 7, 0);
        }
        state.len = 0;
    } else if let Some(model_str) = cmd.strip_prefix("nim model ") {
        // nim model <model>  — set the NIM model name
        dispatch_nim_model(sink, state, model_str.trim().as_bytes());
    } else if let Some(port_str) = cmd.strip_prefix("nim port ") {
        // nim port <n>  — set the NIM host port
        dispatch_nim_port(sink, state, port_str.trim().as_bytes());
    } else if cmd == "nim clear" {
        // nim clear  — clear NIM conversation history
        dispatch_nim_clear(sink, state);
    } else if cmd == "nim status" || cmd == "nim info" {
        // nim status  — display current NIM configuration
        dispatch_nim_status(sink, state);
    } else if cmd == "ai" || cmd == "api" || cmd == "ai-api" {
        state.len = 0;
        super::enter_ai_api_mode(sink, state);
    } else if cmd == "ask" {
        super::push_ai_text(state, "sys> usage: ask <text>");
        super::redraw_ai_panel(sink, state, true);
    } else if let Some(_prompt) = cmd.strip_prefix("ask ") {
        let mut prompt = [0u8; 124];
        let prompt_len = state.len.saturating_sub(4).min(prompt.len());
        prompt[..prompt_len].copy_from_slice(&state.buffer[4..4 + prompt_len]);
        if prompt_len > 0 {
            let mut prefixed = [0u8; AI_PANEL_LINE_WIDTH];
            let prefix = b"you> ";
            let mut line_len = 0usize;
            for byte in prefix.iter().copied() {
                if line_len < prefixed.len() {
                    prefixed[line_len] = byte;
                    line_len += 1;
                }
            }
            for byte in prompt
                .iter()
                .copied()
                .take(prompt_len)
                .take(prefixed.len().saturating_sub(line_len))
            {
                prefixed[line_len] = super::ai_panel_byte(byte);
                line_len += 1;
            }
            super::push_ai_line(state, &prefixed[..line_len]);
        }
        if !super::emit_target_signal(
            sink,
            state.ai_target,
            Signal::Control { cmd: AI_CONTROL_CHAT_BEGIN, val: 0 },
        ) {
            super::push_ai_text(state, "sys> ai lane unresolved");
        } else {
            for byte in prompt.iter().copied().take(prompt_len) {
                let _ = super::emit_target_signal(
                    sink,
                    state.ai_target,
                    Signal::Data { from: sink.from, byte },
                );
            }
            let _ = super::emit_target_signal(
                sink,
                state.ai_target,
                Signal::Control { cmd: AI_CONTROL_CHAT_COMMIT, val: 0 },
            );
        }
        super::redraw_ai_panel(sink, state, true);
    } else if cmd == "mem" || cmd == "memory" {
        // Phase C: developer-facing memory + supervisor domain stats.
        let pmm = k_pmm::allocator().lock();
        let total_kb = (pmm.total_frames() * 4096) / 1024;
        let used_kb  = (pmm.used_frames()  * 4096) / 1024;
        let free_kb  = (pmm.free_frames()  * 4096) / 1024;
        drop(pmm);
        super::set_color(sink, 10, 0);
        super::print_str(sink, " physical memory\n");
        super::set_color(sink, 7, 0);
        super::print_str(sink, "  total: ");
        super::print_num_inline(sink, total_kb);
        super::print_str(sink, " KiB  used: ");
        super::print_num_inline(sink, used_kb);
        super::print_str(sink, " KiB  free: ");
        super::print_num_inline(sink, free_kb);
        super::print_str(sink, " KiB\n");
        // Heap stats from linked_list_allocator.
        let (heap_used, heap_free, heap_total) = k_heap::heap_stats();
        super::set_color(sink, 10, 0);
        super::print_str(sink, " kernel heap\n");
        super::set_color(sink, 7, 0);
        super::print_str(sink, "  total: ");
        super::print_num_inline(sink, heap_total / 1024);
        super::print_str(sink, " KiB  used: ");
        super::print_num_inline(sink, heap_used / 1024);
        super::print_str(sink, " KiB  free: ");
        super::print_num_inline(sink, heap_free / 1024);
        super::print_str(sink, " KiB\n");
        let fallback = gos_runtime::boot_fallback_alloc_count();
        let switches = gos_runtime::domain_switch_count();
        super::print_str(sink, "  boot-fallback-allocs: ");
        super::print_num_inline(sink, fallback as usize);
        super::print_str(sink, "  domain-switches: ");
        super::print_num_inline(sink, switches as usize);
        super::print_str(sink, "\n");
        if let Ok(sv) = gos_supervisor::snapshot() {
            super::set_color(sink, 10, 0);
            super::print_str(sink, " supervisor\n");
            super::set_color(sink, 7, 0);
            super::print_str(sink, "  modules: ");
            super::print_num_inline(sink, sv.installed_modules);
            super::print_str(sink, "  running: ");
            super::print_num_inline(sink, sv.running_modules);
            super::print_str(sink, "  domains: ");
            super::print_num_inline(sink, sv.isolated_domains);
            super::print_str(sink, "  caps: ");
            super::print_num_inline(sink, sv.published_capabilities);
            super::print_str(sink, "  revocations-pending: ");
            super::print_num_inline(sink, sv.pending_revocations);
            super::print_str(sink, "\n");
        }
    } else if cmd == "ps" || cmd == "modules" {
        use gos_protocol::ModuleLifecycle;
        super::set_color(sink, 10, 0);
        super::print_str(sink, " modules\n");
        super::set_color(sink, 7, 0);
        let mut buf = [gos_supervisor::ModuleInfo {
            handle: gos_protocol::ModuleHandle(0),
            name: "",
            state: ModuleLifecycle::Stopped,
            isolated: false,
            restart_generation: 0,
            queued_restart: false,
        }; 16];
        let mut offset = 0usize;
        let mut total = 0usize;
        loop {
            let n = gos_supervisor::module_page(offset, &mut buf);
            if n == 0 {
                break;
            }
            for info in buf[..n].iter() {
                super::print_str(sink, "  ");
                super::print_num_inline(sink, info.handle.0 as usize);
                super::print_str(sink, "  ");
                super::print_str(sink, info.name);
                super::print_str(sink, "  ");
                super::print_str(sink, match info.state {
                    ModuleLifecycle::Installed    => "installed",
                    ModuleLifecycle::Validated    => "validated",
                    ModuleLifecycle::Mapped       => "mapped",
                    ModuleLifecycle::Instantiated => "instantiated",
                    ModuleLifecycle::Running      => "running",
                    ModuleLifecycle::Quiescing    => "quiescing",
                    ModuleLifecycle::Stopped      => "stopped",
                    ModuleLifecycle::Faulted      => "faulted",
                });
                if info.isolated {
                    super::print_str(sink, "  [isolated]");
                }
                if info.queued_restart {
                    super::print_str(sink, "  [restart-queued]");
                }
                if info.restart_generation > 0 {
                    super::print_str(sink, "  restarts=");
                    super::print_num_inline(sink, info.restart_generation as usize);
                }
                super::print_str(sink, "\n");
                total += 1;
            }
            offset += n;
            if n < buf.len() {
                break;
            }
        }
        if total == 0 {
            super::print_str(sink, "  (no modules installed)\n");
        }
    } else if cmd == "caps" || cmd == "capabilities" {
        use gos_protocol::CapabilityToken;
        super::set_color(sink, 10, 0);
        super::print_str(sink, " capabilities\n");
        super::set_color(sink, 7, 0);
        let mut buf = [gos_supervisor::CapabilityInfo {
            token: CapabilityToken::ZERO,
            provider: gos_protocol::ModuleHandle(0),
            namespace: "",
            name: "",
        }; 16];
        let mut offset = 0usize;
        let mut total = 0usize;
        loop {
            let n = gos_supervisor::capability_page(offset, &mut buf);
            if n == 0 {
                break;
            }
            for info in buf[..n].iter() {
                super::print_str(sink, "  ");
                super::print_str(sink, info.namespace);
                super::print_str(sink, "::");
                super::print_str(sink, info.name);
                super::print_str(sink, "  provider=");
                super::print_num_inline(sink, info.provider.0 as usize);
                super::print_str(sink, "\n");
                total += 1;
            }
            offset += n;
            if n < buf.len() {
                break;
            }
        }
        if total == 0 {
            super::print_str(sink, "  (no capabilities published)\n");
        }
    } else if cmd == "instances" {
        use gos_protocol::{NodeInstanceLifecycle, ExecutionLaneClass};
        super::set_color(sink, 10, 0);
        super::print_str(sink, " instances\n");
        super::set_color(sink, 7, 0);
        let mut buf = [gos_supervisor::NodeInstanceSummary {
            instance_id: gos_protocol::NodeInstanceId(0),
            template_id: gos_protocol::NodeTemplateId([0u8; 16]),
            module: gos_protocol::ModuleHandle(0),
            lane: ExecutionLaneClass::Background,
            lifecycle: NodeInstanceLifecycle::Stopped,
            ready_queued: false,
            heap_quota: gos_protocol::HeapQuota::EMPTY,
            heap_pages_used: 0,
        }; 16];
        let mut offset = 0usize;
        let mut total = 0usize;
        loop {
            let n = gos_supervisor::instance_page(offset, &mut buf);
            if n == 0 {
                break;
            }
            for info in buf[..n].iter() {
                super::print_str(sink, "  ");
                super::print_num_inline(sink, info.instance_id.0 as usize);
                super::print_str(sink, "  mod=");
                super::print_num_inline(sink, info.module.0 as usize);
                super::print_str(sink, "  lane=");
                super::print_str(sink, match info.lane {
                    ExecutionLaneClass::Control    => "ctrl",
                    ExecutionLaneClass::Io         => "io",
                    ExecutionLaneClass::Compute    => "compute",
                    ExecutionLaneClass::Background => "bg",
                });
                super::print_str(sink, "  ");
                super::print_str(sink, match info.lifecycle {
                    NodeInstanceLifecycle::Allocated    => "allocated",
                    NodeInstanceLifecycle::Ready        => "ready",
                    NodeInstanceLifecycle::Running      => "running",
                    NodeInstanceLifecycle::WaitingClaim => "waiting-claim",
                    NodeInstanceLifecycle::Suspended    => "suspended",
                    NodeInstanceLifecycle::Stopped      => "stopped",
                    NodeInstanceLifecycle::Faulted      => "faulted",
                });
                super::print_str(sink, "  heap=");
                super::print_num_inline(sink, info.heap_pages_used as usize);
                super::print_str(sink, "p");
                if info.ready_queued {
                    super::print_str(sink, "  [queued]");
                }
                super::print_str(sink, "\n");
                total += 1;
            }
            offset += n;
            if n < buf.len() {
                break;
            }
        }
        if total == 0 {
            super::print_str(sink, "  (no instances)\n");
        }
    } else if cmd == "health" {
        use gos_protocol::ModuleLifecycle;
        let mut buf = [gos_supervisor::ModuleInfo {
            handle: gos_protocol::ModuleHandle(0),
            name: "",
            state: ModuleLifecycle::Stopped,
            isolated: false,
            restart_generation: 0,
            queued_restart: false,
        }; 32];
        let mut total_modules = 0usize;
        let mut running = 0usize;
        let mut faulted = 0usize;
        let mut restarting = 0usize;
        let mut offset = 0usize;
        loop {
            let n = gos_supervisor::module_page(offset, &mut buf);
            if n == 0 { break; }
            for info in buf[..n].iter() {
                total_modules += 1;
                if info.state == ModuleLifecycle::Running { running += 1; }
                if info.state == ModuleLifecycle::Faulted { faulted += 1; }
                if info.queued_restart { restarting += 1; }
            }
            offset += n;
            if n < buf.len() { break; }
        }
        let ok = faulted == 0 && restarting == 0;
        super::set_color(sink, if ok { 10 } else { 12 }, 0);
        super::print_str(sink, if ok { " health: OK\n" } else { " health: DEGRADED\n" });
        super::set_color(sink, 7, 0);
        super::print_str(sink, "  modules: ");
        super::print_num_inline(sink, total_modules);
        super::print_str(sink, "  running: ");
        super::print_num_inline(sink, running);
        super::print_str(sink, "  faulted: ");
        if faulted > 0 { super::set_color(sink, 12, 0); }
        super::print_num_inline(sink, faulted);
        super::set_color(sink, 7, 0);
        super::print_str(sink, "  restarting: ");
        if restarting > 0 { super::set_color(sink, 14, 0); }
        super::print_num_inline(sink, restarting);
        super::set_color(sink, 7, 0);
        super::print_str(sink, "\n");
        // Show faulted modules by name.
        if faulted > 0 {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " faulted modules:\n");
            super::set_color(sink, 7, 0);
            let mut offset2 = 0usize;
            loop {
                let n = gos_supervisor::module_page(offset2, &mut buf);
                if n == 0 { break; }
                for info in buf[..n].iter() {
                    if info.state == ModuleLifecycle::Faulted {
                        super::print_str(sink, "  ");
                        super::print_str(sink, info.name);
                        super::print_str(sink, "  restarts=");
                        super::print_num_inline(sink, info.restart_generation as usize);
                        super::print_str(sink, "\n");
                    }
                }
                offset2 += n;
                if n < buf.len() { break; }
            }
        }
    } else if cmd == "events" || cmd == "stats" {
        super::set_color(sink, 10, 0);
        super::print_str(sink, " event counters\n");
        super::set_color(sink, 7, 0);
        super::print_str(sink, "  signals-dispatched: ");
        super::print_num_inline(sink, gos_runtime::signal_dispatch_count() as usize);
        super::print_str(sink, "\n  activations:       ");
        super::print_num_inline(sink, gos_runtime::activation_count() as usize);
        super::print_str(sink, "\n  faults:            ");
        super::print_num_inline(sink, gos_runtime::fault_dispatch_count() as usize);
        super::print_str(sink, "\n  preemptions:       ");
        super::print_num_inline(sink, gos_runtime::preempt_count() as usize);
        super::print_str(sink, "\n  domain-switches:   ");
        super::print_num_inline(sink, gos_runtime::domain_switch_count() as usize);
        super::print_str(sink, "\n  boot-fallback-allocs: ");
        super::print_num_inline(sink, gos_runtime::boot_fallback_alloc_count() as usize);
        super::print_str(sink, "\n");
    } else if cmd == "cpu" || cmd == "cpuid" {
        // Query CPUID directly for brand string, feature flags, and topology.
        super::set_color(sink, 10, 0);
        super::print_str(sink, " cpu\n");
        super::set_color(sink, 7, 0);
        // Brand string: leaves 0x8000_0002..0x8000_0004, 12 dwords = 48 bytes.
        let mut brand = [0u8; 48];
        for (i, leaf) in (0x8000_0002u32..=0x8000_0004u32).enumerate() {
            let result = unsafe { core::arch::x86_64::__cpuid(leaf) };
            let off = i * 16;
            brand[off..off + 4].copy_from_slice(&result.eax.to_le_bytes());
            brand[off + 4..off + 8].copy_from_slice(&result.ebx.to_le_bytes());
            brand[off + 8..off + 12].copy_from_slice(&result.ecx.to_le_bytes());
            brand[off + 12..off + 16].copy_from_slice(&result.edx.to_le_bytes());
        }
        super::print_str(sink, "  model:    ");
        let brand_end = brand.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        for &b in brand[..brand_end].iter() {
            if b >= 0x20 && b < 0x7F {
                super::print_byte(sink, b);
            }
        }
        super::print_str(sink, "\n");
        // Feature flags: leaf 1 ECX and EDX.
        let feat = unsafe { core::arch::x86_64::__cpuid(1) };
        let ecx = feat.ecx;
        let edx = feat.edx;
        super::print_str(sink, "  features:");
        if edx & (1 << 25) != 0 { super::print_str(sink, " SSE"); }
        if edx & (1 << 26) != 0 { super::print_str(sink, " SSE2"); }
        if ecx & (1 << 0)  != 0 { super::print_str(sink, " SSE3"); }
        if ecx & (1 << 9)  != 0 { super::print_str(sink, " SSSE3"); }
        if ecx & (1 << 19) != 0 { super::print_str(sink, " SSE4.1"); }
        if ecx & (1 << 20) != 0 { super::print_str(sink, " SSE4.2"); }
        if ecx & (1 << 28) != 0 { super::print_str(sink, " AVX"); }
        if ecx & (1 << 12) != 0 { super::print_str(sink, " FMA"); }
        if ecx & (1 << 30) != 0 { super::print_str(sink, " RDRAND"); }
        if ecx & (1 << 5)  != 0 { super::print_str(sink, " VMX"); }
        if ecx & (1 << 26) != 0 { super::print_str(sink, " XSAVE"); }
        if edx & (1 << 4)  != 0 { super::print_str(sink, " TSC"); }
        if edx & (1 << 5)  != 0 { super::print_str(sink, " MSR"); }
        if edx & (1 << 9)  != 0 { super::print_str(sink, " APIC"); }
        super::print_str(sink, "\n");
        // Physical/logical core count from leaf 4.
        let topo = unsafe { core::arch::x86_64::__cpuid_count(4, 0) }; // leaf 4 sub-leaf 0
        let phys_cores = ((topo.eax >> 26) & 0x3F) + 1;
        let leaf1_ebx = feat.ebx;
        let logical_per_package = (leaf1_ebx >> 16) & 0xFF;
        super::print_str(sink, "  phys-cores: ");
        super::print_num_inline(sink, phys_cores as usize);
        super::print_str(sink, "  logical/pkg: ");
        super::print_num_inline(sink, logical_per_package as usize);
        super::print_str(sink, "\n");
        // Max CPUID leaf.
        let max_leaf = unsafe { core::arch::x86_64::__cpuid(0) }; // max basic leaf
        super::print_str(sink, "  max-leaf: ");
        super::print_num_inline(sink, max_leaf.eax as usize);
        let max_ext = unsafe { core::arch::x86_64::__cpuid(0x8000_0000) }; // max ext leaf
        super::print_str(sink, "  max-ext-leaf: ");
        // Print hex for extended leaf
        let v = max_ext.eax;
        super::print_str(sink, "0x");
        for shift in [28u32, 24, 20, 16, 12, 8, 4, 0] {
            let nibble = (v >> shift) & 0xF;
            super::print_byte(sink, if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble as u8 - 10) });
        }
        super::print_str(sink, "\n");
    } else if cmd == "tick" || cmd == "uptime" {
        let snapshot = gos_runtime::snapshot();
        let pit_ticks = gos_runtime::pit_tick_count();
        super::set_color(sink, 10, 0);
        super::print_str(sink, " uptime\n");
        super::set_color(sink, 7, 0);
        // PIT runs at 120 Hz; convert to wall-clock time.
        let secs = pit_ticks / 120;
        let frac = (pit_ticks % 120) * 10 / 120;
        super::print_str(sink, "  uptime:   ");
        super::print_num_inline(sink, secs as usize);
        super::print_str(sink, ".");
        super::print_num_inline(sink, frac as usize);
        super::print_str(sink, " s  (PIT 120 Hz, ");
        super::print_num_inline(sink, pit_ticks as usize);
        super::print_str(sink, " ticks)\n");
        super::print_str(sink, "  pump-tick: ");
        super::print_num_inline(sink, snapshot.tick as usize);
        super::print_str(sink, "  (work items processed)\n");
        super::print_str(sink, "  ctrl-q:   ");
        super::print_num_inline(sink, snapshot.control_queue_len);
        super::print_str(sink, "  (high-priority: Control/Spawn/Terminate)\n");
        super::print_str(sink, "  signals:  ");
        super::print_num_inline(sink, snapshot.signal_queue_len);
        super::print_str(sink, "  ready: ");
        super::print_num_inline(sink, snapshot.ready_queue_len);
        super::print_str(sink, "\n");
    } else if cmd == "log" || cmd == "logs" || cmd == "dmesg" {
        use gos_log::LogLevel;
        super::set_color(sink, 10, 0);
        super::print_str(sink, " kernel log\n");
        super::set_color(sink, 7, 0);
        let mut buf = [gos_log::LogRecord::empty(); 32];
        let n = gos_log::recent_logs(&mut buf);
        if n == 0 {
            super::print_str(sink, "  (empty)\n");
        } else {
            for rec in buf[..n].iter() {
                let (color, prefix) = match rec.level {
                    LogLevel::Trace => (8u8,  "T"),
                    LogLevel::Debug => (7u8,  "D"),
                    LogLevel::Info  => (10u8, "I"),
                    LogLevel::Warn  => (14u8, "W"),
                    LogLevel::Error => (12u8, "E"),
                };
                super::set_color(sink, color, 0);
                super::print_str(sink, prefix);
                super::print_str(sink, " [");
                // Print up to 8 bytes of the source tag as ASCII.
                let src_end = rec.source.iter().position(|&b| b == 0).unwrap_or(16).min(16);
                for &b in &rec.source[..src_end] {
                    if b >= 0x20 && b < 0x7F {
                        super::print_byte(sink, b);
                    } else {
                        super::print_byte(sink, b'?');
                    }
                }
                super::print_str(sink, "] ");
                super::set_color(sink, 7, 0);
                for &b in rec.payload_str() {
                    if b == b'\n' {
                        super::print_str(sink, "\n    ");
                    } else {
                        super::print_byte(sink, b);
                    }
                }
                if rec.truncated {
                    super::set_color(sink, 14, 0);
                    super::print_str(sink, "…");
                    super::set_color(sink, 7, 0);
                }
                super::print_str(sink, "\n");
            }
        }
    } else if cmd == "log clear" || cmd == "logs clear" || cmd == "dmesg clear" {
        gos_log::clear_log_ring();
        super::set_color(sink, 11, 0);
        super::print_str(sink, " log ring cleared\n");
        super::set_color(sink, 7, 0);
    } else if cmd == "clear" {
        state.len = 0;
        super::redraw_console(sink, state);
    } else if cmd == "splash" || cmd == "reboot-splash" {
        state.console_live = 0;
        super::play_boot_sequence(sink);
        super::redraw_console(sink, state);
        state.console_live = 1;
        state.len = 0;
    } else if !cmd.is_empty() {
        super::set_color(sink, 12, 0);
        if cmd.is_ascii() {
            super::print_str(sink, " unknown command: ");
            super::set_color(sink, 15, 0);
            super::print_str(sink, cmd);
            super::print_str(sink, "\n");
        } else {
            super::print_str(sink, " unknown command payload contains non-ascii bytes\n");
        }
    }
}

// ---------------------------------------------------------------------------
// chat subcommand helpers
// ---------------------------------------------------------------------------

/// Send `bytes` to k-chat as a streamed API key (KEY_BEGIN → Data × N → KEY_COMMIT).
fn dispatch_chat_key(
    sink:  &super::ConsoleSink,
    state: &mut super::ShellState,
    bytes: &[u8],
) {
    use gos_protocol::Signal;
    let chat_target = super::CHAT_TARGET.load(core::sync::atomic::Ordering::SeqCst);
    if chat_target == 0 {
        super::set_color(sink, 12, 0);
        super::print_str(sink, " [chat] k-chat not available\n");
        return;
    }
    super::emit_target_signal_raw(
        sink.abi,
        chat_target,
        Signal::Control { cmd: CHAT_CONTROL_KEY_BEGIN, val: 0 },
    );
    for &b in bytes {
        super::emit_target_signal_raw(
            sink.abi,
            chat_target,
            Signal::Data { from: super::NODE_VEC.as_u64(), byte: b },
        );
    }
    super::emit_target_signal_raw(
        sink.abi,
        chat_target,
        Signal::Control { cmd: CHAT_CONTROL_KEY_COMMIT, val: 0 },
    );
    super::set_color(sink, 10, 0);
    super::print_str(sink, " [chat] api key set (");
    super::print_num_inline(sink, bytes.len());
    super::print_str(sink, " bytes)\n");
    super::set_color(sink, 7, 0);
    let _ = state; // unused but kept for API consistency
}

/// Stream a model name to k-chat (MODEL_BEGIN → Data × N → MODEL_COMMIT).
fn dispatch_chat_model(
    sink:  &super::ConsoleSink,
    state: &mut super::ShellState,
    bytes: &[u8],
) {
    use gos_protocol::Signal;
    let chat_target = super::CHAT_TARGET.load(core::sync::atomic::Ordering::SeqCst);
    if chat_target == 0 {
        super::set_color(sink, 12, 0);
        super::print_str(sink, " [chat] k-chat not available\n");
        return;
    }
    super::emit_target_signal_raw(
        sink.abi,
        chat_target,
        Signal::Control { cmd: CHAT_CONTROL_MODEL_BEGIN, val: 0 },
    );
    for &b in bytes {
        super::emit_target_signal_raw(
            sink.abi,
            chat_target,
            Signal::Data { from: super::NODE_VEC.as_u64(), byte: b },
        );
    }
    super::emit_target_signal_raw(
        sink.abi,
        chat_target,
        Signal::Control { cmd: CHAT_CONTROL_MODEL_COMMIT, val: 0 },
    );
    super::set_color(sink, 10, 0);
    super::print_str(sink, " [chat] model set: ");
    for &b in bytes { super::print_byte(sink, b); }
    super::print_str(sink, "\n");
    super::set_color(sink, 7, 0);
    let _ = state;
}

/// Send CHAT_CONTROL_API_TYPE with the encoded backend index.
fn dispatch_chat_api(
    sink:  &super::ConsoleSink,
    state: &mut super::ShellState,
    name:  &str,
) {
    use gos_protocol::Signal;
    let chat_target = super::CHAT_TARGET.load(core::sync::atomic::Ordering::SeqCst);
    if chat_target == 0 {
        super::set_color(sink, 12, 0);
        super::print_str(sink, " [chat] k-chat not available\n");
        return;
    }
    let (val, label): (u8, &str) = match name {
        "openai"    => (1, "openai"),
        "anthropic" => (2, "anthropic"),
        _           => (0, "ollama"),
    };
    super::emit_target_signal_raw(
        sink.abi,
        chat_target,
        Signal::Control { cmd: CHAT_CONTROL_API_TYPE, val },
    );
    super::set_color(sink, 10, 0);
    super::print_str(sink, " [chat] api backend -> ");
    super::print_str(sink, label);
    super::print_str(sink, "\n");
    super::set_color(sink, 7, 0);
    let _ = state;
}

/// Toggle direct-HTTP mode in k-chat.
fn dispatch_chat_http_toggle(
    sink:  &super::ConsoleSink,
    state: &mut super::ShellState,
) {
    use gos_protocol::Signal;
    let chat_target = super::CHAT_TARGET.load(core::sync::atomic::Ordering::SeqCst);
    if chat_target == 0 {
        super::set_color(sink, 12, 0);
        super::print_str(sink, " [chat] k-chat not available\n");
        return;
    }
    // We toggle: read current mode from the atomic we stored, flip it.
    let current_http = super::CHAT_HTTP_MODE.load(core::sync::atomic::Ordering::SeqCst);
    let next_http = if current_http == 0 { 1u8 } else { 0u8 };
    super::CHAT_HTTP_MODE.store(next_http, core::sync::atomic::Ordering::SeqCst);
    super::emit_target_signal_raw(
        sink.abi,
        chat_target,
        Signal::Control { cmd: CHAT_CONTROL_HTTP_TOGGLE, val: next_http },
    );
    super::set_color(sink, 10, 0);
    super::print_str(sink, " [chat] http mode -> ");
    super::print_str(sink, if next_http == 1 { "direct TCP (Ollama 10.0.2.2:11434)" } else { "COM2 bridge" });
    super::print_str(sink, "\n");
    super::set_color(sink, 7, 0);
    let _ = state;
}

/// Print current chat configuration.
fn dispatch_chat_status(
    sink:  &super::ConsoleSink,
    _state: &mut super::ShellState,
) {
    let chat_target = super::CHAT_TARGET.load(core::sync::atomic::Ordering::SeqCst);
    let http_mode   = super::CHAT_HTTP_MODE.load(core::sync::atomic::Ordering::SeqCst);
    super::set_color(sink, 11, 0);
    super::print_str(sink, " chat status\n");
    super::set_color(sink, 7, 0);
    super::print_str(sink, "  node:    ");
    if chat_target == 0 {
        super::print_str(sink, "offline\n");
    } else {
        super::print_str(sink, "online\n");
    }
    super::print_str(sink, "  mode:    ");
    super::print_str(sink, if http_mode == 1 { "direct TCP/HTTP (Ollama)" } else { "COM2 bridge" });
    super::print_str(sink, "\n  cmds:    chat key <k>  chat model <m>  chat api <type>  chat http\n");
    super::print_str(sink, "  types:   ollama (default)  openai  anthropic\n");
}

// ---------------------------------------------------------------------------
// nim subcommand helpers
// ---------------------------------------------------------------------------

/// Stream a model name to k-nim (MODEL_BEGIN → Data × N → MODEL_COMMIT).
fn dispatch_nim_model(
    sink:  &super::ConsoleSink,
    state: &mut super::ShellState,
    bytes: &[u8],
) {
    use gos_protocol::Signal;
    let nim_target = super::NIM_TARGET.load(core::sync::atomic::Ordering::SeqCst);
    if nim_target == 0 {
        super::set_color(sink, 12, 0);
        super::print_str(sink, " [nim] k-nim not available\n");
        return;
    }
    super::emit_target_signal_raw(
        sink.abi,
        nim_target,
        Signal::Control { cmd: NIM_CONTROL_MODEL_BEGIN, val: 0 },
    );
    for &b in bytes {
        super::emit_target_signal_raw(
            sink.abi,
            nim_target,
            Signal::Data { from: super::NODE_VEC.as_u64(), byte: b },
        );
    }
    super::emit_target_signal_raw(
        sink.abi,
        nim_target,
        Signal::Control { cmd: NIM_CONTROL_MODEL_COMMIT, val: 0 },
    );
    super::set_color(sink, 10, 0);
    super::print_str(sink, " [nim] model set: ");
    for &b in bytes { super::print_byte(sink, b); }
    super::print_str(sink, "\n");
    super::set_color(sink, 7, 0);
    let _ = state;
}

/// Stream port digits to k-nim (PORT_BEGIN → Data × N → PORT_COMMIT).
fn dispatch_nim_port(
    sink:  &super::ConsoleSink,
    state: &mut super::ShellState,
    bytes: &[u8],
) {
    use gos_protocol::Signal;
    let nim_target = super::NIM_TARGET.load(core::sync::atomic::Ordering::SeqCst);
    if nim_target == 0 {
        super::set_color(sink, 12, 0);
        super::print_str(sink, " [nim] k-nim not available\n");
        return;
    }
    // Validate: must be ASCII digits only
    if bytes.iter().any(|&b| b < b'0' || b > b'9') || bytes.is_empty() {
        super::set_color(sink, 12, 0);
        super::print_str(sink, " [nim] port must be decimal digits (e.g. 8000)\n");
        return;
    }
    super::emit_target_signal_raw(
        sink.abi,
        nim_target,
        Signal::Control { cmd: NIM_CONTROL_PORT_BEGIN, val: 0 },
    );
    for &b in bytes {
        super::emit_target_signal_raw(
            sink.abi,
            nim_target,
            Signal::Data { from: super::NODE_VEC.as_u64(), byte: b },
        );
    }
    super::emit_target_signal_raw(
        sink.abi,
        nim_target,
        Signal::Control { cmd: NIM_CONTROL_PORT_COMMIT, val: 0 },
    );
    super::set_color(sink, 10, 0);
    super::print_str(sink, " [nim] port set: ");
    for &b in bytes { super::print_byte(sink, b); }
    super::print_str(sink, "\n");
    super::set_color(sink, 7, 0);
    let _ = state;
}

/// Send NIM_CONTROL_CLEAR_HISTORY to k-nim.
fn dispatch_nim_clear(
    sink:  &super::ConsoleSink,
    state: &mut super::ShellState,
) {
    use gos_protocol::Signal;
    let nim_target = super::NIM_TARGET.load(core::sync::atomic::Ordering::SeqCst);
    if nim_target == 0 {
        super::set_color(sink, 12, 0);
        super::print_str(sink, " [nim] k-nim not available\n");
        return;
    }
    super::emit_target_signal_raw(
        sink.abi,
        nim_target,
        Signal::Control { cmd: NIM_CONTROL_CLEAR_HISTORY, val: 0 },
    );
    super::set_color(sink, 11, 0);
    super::print_str(sink, " [nim] conversation history cleared\n");
    super::set_color(sink, 7, 0);
    let _ = state;
}

/// Print current NIM configuration.
fn dispatch_nim_status(
    sink:  &super::ConsoleSink,
    _state: &mut super::ShellState,
) {
    let nim_target = super::NIM_TARGET.load(core::sync::atomic::Ordering::SeqCst);
    super::set_color(sink, 11, 0);
    super::print_str(sink, " nim status\n");
    super::set_color(sink, 7, 0);
    super::print_str(sink, "  node:    ");
    if nim_target == 0 {
        super::print_str(sink, "offline\n");
    } else {
        super::print_str(sink, "online\n");
    }
    super::print_str(sink, "  endpoint: 10.0.2.2:8000  (NVIDIA NIM default)\n");
    super::print_str(sink, "  cmds:    nim model <m>  nim port <n>  nim clear\n");
    super::print_str(sink, "  example: nim model meta/llama-3.1-8b-instruct\n");
}
