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
    CUDA_CONTROL_REPORT, CUDA_CONTROL_RESET,
    IME_MODE_ASCII, IME_MODE_ZH_PINYIN,
    NET_CONTROL_PROBE, NET_CONTROL_REPORT, NET_CONTROL_RESET,
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
        super::print_str(sink, "  ai      open bottom ai api editor\n");
        super::print_str(sink, "  ask     send prompt into ai chat lane\n");
        super::print_str(sink, "  ^C/^X/^V copy, cut, paste active input through clipboard.mount\n");
        super::print_str(sink, "  ctrl+l  toggle input language en/zh-py\n");
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
