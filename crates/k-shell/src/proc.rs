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

use gos_runtime;

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
                // V2.3 epoch-diff idle skip: only repaint the command-deck
                // panel when the graph topology actually changed since the
                // last repaint.  Directly implements Demo #2 zero-idle-frames
                // for the shell panel without any special render bookkeeping.
                // V2.30: in watch mode, always repaint so tick counter updates.
                let current_epoch = gos_runtime::graph_epoch();
                let watch_active = super::WATCH_PROC_MODE.load(core::sync::atomic::Ordering::SeqCst) != 0;
                if watch_active || current_epoch != state.last_rendered_epoch {
                    state.last_rendered_epoch = current_epoch;
                    super::draw_command_deck_panel(&sink, state, snapshot);
                }
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
    // --- V2.30 watch mode: any keyboard key exits and restores normal deck --------
    if source == DataSource::Keyboard
        && super::WATCH_PROC_MODE.load(core::sync::atomic::Ordering::SeqCst) != 0
    {
        super::WATCH_PROC_MODE.store(0, core::sync::atomic::Ordering::SeqCst);
        // Force deck repaint by invalidating the epoch cache.
        state.last_rendered_epoch = u64::MAX;
        super::restore_output_cursor(sink);
        super::set_color(sink, 8, 0);
        super::print_str(sink, " watch stopped\n");
        super::save_output_cursor(sink);
        super::redraw_footer(sink, state, false);
        return ExecStatus::Done;
    }

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
    if state.input_lang == IME_MODE_ZH_PINYIN
        && let Some(status) = process_pinyin(sink, state, byte)
    {
        return status;
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
        0x08 | 0x7F if super::command_pop_scalar(state) => {
            super::reset_command_history_cursor(state);
            super::redraw_footer(sink, state, false);
        }
        0x08 | 0x7F => {}
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
        super::print_str(sink, "  modules supervisor module health (lifecycle/fault/restarts)\n");
        super::print_str(sink, "  resources / res  per-instance + total heap/gpu usage vs quota\n");
        super::print_str(sink, "  sup     supervisor internals (instances/resources/caps/lanes)\n");
        super::print_str(sink, "  restart <name>  manually restart one module by id\n");
        super::print_str(sink, "  nodes              list all live graph nodes (ps-style)\n");
        super::print_str(sink, "  nodes faulted      list only faulted nodes\n");
        super::print_str(sink, "  nodes summary      lifecycle distribution count\n");
        super::print_str(sink, "  boot verify        boot manifest edge verification report\n");
        super::print_str(sink, "  metrics export     machine-parseable key=value telemetry dump\n");
        super::print_str(sink, "  journal            journal format info and replay status\n");
        super::print_str(sink, "  proc               ps-style table: node signal counts + edge out-degree\n");
        super::print_str(sink, "  stat <vector>      detailed stat for one node (like /proc/<pid>/status)\n");
        super::print_str(sink, "  node stat clear <vector>  reset signal_count to 0 (like perf stat reset)\n");
        super::print_str(sink, "  nstat clear <vector>      alias for node stat clear\n");
        super::print_str(sink, "  kill <vector>      fault a node by vector (like kill -9 <pid>)\n");
        super::print_str(sink, "  node fault <vector>  alias for kill\n");
        super::print_str(sink, "  resume <vector>    resume a faulted/suspended node (like systemctl restart)\n");
        super::print_str(sink, "  node resume <vector>  alias for resume\n");
        super::print_str(sink, "  node info <vector> comprehensive node view: stat + edges (like systemctl status)\n");
        super::print_str(sink, "  ninfo <vector>     alias for node info\n");
        super::print_str(sink, "  node trace <vector>       signal dispatch history for one node (like strace -p <pid>)\n");
        super::print_str(sink, "  ntrace <vector>           alias for node trace\n");
        super::print_str(sink, "  node trace clear <vector> clear signal trace ring for one node (like perf trace reset)\n");
        super::print_str(sink, "  ntrace clear <vector>     alias for node trace clear\n");
        super::print_str(sink, "  node log <vector>         lifecycle event log for one node (like journalctl -u <svc>)\n");
        super::print_str(sink, "  nlog <vector>             alias for node log\n");
        super::print_str(sink, "  node log clear <vector>   clear lifecycle log for one node (like journalctl --vacuum-time)\n");
        super::print_str(sink, "  nlog clear <vector>       alias for node log clear\n");
        super::print_str(sink, "  node attr set <vector> <hex>  store u32 attribute on node (palette, flag, counter)\n");
        super::print_str(sink, "  node attr get <vector>        read u32 attribute from node (or 'none' if absent)\n");
        super::print_str(sink, "  node attr list                list all nodes that have u32 attributes set\n");
        super::print_str(sink, "  node attr list u8             list all nodes that have u8 attributes set (theme signal vals)\n");
        super::print_str(sink, "  nattr set / nattr get / nattr list / nattr list u8  aliases\n");
        super::print_str(sink, "  edges              list all live graph edges (ss-style)\n");
        super::print_str(sink, "  edges count        total edge count\n");
        super::print_str(sink, "  edges <type>       filter by type: call spawn depend signal return mount sync stream use\n");
        super::print_str(sink, "  graph diff         show topology changes since pinned epoch (like git diff)\n");
        super::print_str(sink, "  graph diff <N>     show topology changes since epoch N (e.g. graph diff 42)\n");
        super::print_str(sink, "  graph diff pin     pin current epoch as diff baseline\n");
        super::print_str(sink, "  graph diff reset   reset baseline to epoch 0 (show all since boot)\n");
        super::print_str(sink, "  graph topo         node count per l4 domain (like ip route show)\n");
        super::print_str(sink, "  graph topo <L4>    list nodes in l4 domain L4 (like ip link show)\n");
        super::print_str(sink, "  graph health       holistic health report: faults, ring, metrics (like systemctl status)\n");
        super::print_str(sink, "  graph path <A> <B> BFS shortest path from node A to node B (like traceroute)\n");
        super::print_str(sink, "  graph cycles       detect directed cycles in the graph (like tsort cycle-check)\n");
        super::print_str(sink, "  graph toposort     topological dependency ordering of all nodes (like tsort)\n");
        super::print_str(sink, "  graph reachable <V> all nodes reachable from V via directed edges (like systemctl list-dependencies --all)\n");
        super::print_str(sink, "  reachable <V>      alias for graph reachable\n");
        super::print_str(sink, "  graph degree       in/out degree per node + hub identification (like ip -s link show)\n");
        super::print_str(sink, "  degree / hub       aliases for graph degree\n");
        super::print_str(sink, "  graph closeness    outgoing closeness centrality per node (like ping avg RTT census)\n");
        super::print_str(sink, "  closeness / cc     aliases for graph closeness\n");
        super::print_str(sink, "  graph eccentricity radius/diameter per node (BFS eccentricity, like traceroute hop bound)\n");
        super::print_str(sink, "  eccentricity / ecc / radius  aliases for graph eccentricity\n");
        super::print_str(sink, "  graph katz         incoming Katz centrality per node (walk-count influence, like netstat -s)\n");
        super::print_str(sink, "  katz / kz          aliases for graph katz\n");
        super::print_str(sink, "  graph pagerank     PageRank per node (random-walk authority, like top by signal weight)\n");
        super::print_str(sink, "  pagerank / pr      aliases for graph pagerank\n");
        super::print_str(sink, "  graph hits         HITS hub/authority scores (bipartite signal-forwarder vs cited-target)\n");
        super::print_str(sink, "  hits / ha          aliases for graph hits\n");
        super::print_str(sink, "  graph community    label-propagation community detection (subsystem clustering)\n");
        super::print_str(sink, "  community / lpa    aliases for graph community\n");
        super::print_str(sink, "  graph spanning     BFS spanning forest over all live nodes (minimal backbone)\n");
        super::print_str(sink, "  spanning / span    aliases for graph spanning\n");
        super::print_str(sink, "  graph color        greedy graph coloring — conflict-free scheduling domains\n");
        super::print_str(sink, "  color / gcolor     aliases for graph color\n");
        super::print_str(sink, "  graph mst          Prim's minimum spanning forest — minimum-cost routing backbone\n");
        super::print_str(sink, "  mst / gmst         aliases for graph mst\n");
        super::print_str(sink, "  graph density      E / (N*(N-1)) sparsity metric — how interconnected the graph is\n");
        super::print_str(sink, "  density / gdensity aliases for graph density\n");
        super::print_str(sink, "  graph clustering   Watts-Strogatz global clustering coefficient (triangle density ppm)\n");
        super::print_str(sink, "  clustering / gcluster  aliases for graph clustering\n");
        super::print_str(sink, "  graph assortativity  Newman degree assortativity r \u{2208} [\u{2212}1,+1] \u{2014} hubs-connect-to-hubs?\n");
        super::print_str(sink, "  assortativity / gassort  aliases for graph assortativity\n");
        super::print_str(sink, "  graph reciprocity  fraction of directed edges that are mutual (bidirectional)\n");
        super::print_str(sink, "  reciprocity / grecip  aliases for graph reciprocity\n");
        super::print_str(sink, "  graph modularity   Newman\u{2013}Girvan Q score of LPA community partition \u{2208} [0,1]\n");
        super::print_str(sink, "  modularity / gmodq aliases for graph modularity\n");
        super::print_str(sink, "  graph rich club <k>  density among nodes with degree > k \u{2208} [0,1]\n");
        super::print_str(sink, "  richclub <k> / grichclub <k>  aliases for graph rich club\n");
        super::print_str(sink, "  graph girth        length of shortest directed cycle (acyclic \u{2192} DAG)\n");
        super::print_str(sink, "  ggirth             alias for graph girth\n");
        super::print_str(sink, "  graph wiener       Wiener index W(G) = \u{2211} pairwise BFS distances + avg path length\n");
        super::print_str(sink, "  gwiener            alias for graph wiener\n");
        super::print_str(sink, "  graph harmonic     harmonic centrality HC[v] = \u{2211} 1/d(v,u) (disconnected-safe closeness)\n");
        super::print_str(sink, "  gharm              alias for graph harmonic\n");
        super::print_str(sink, "  graph peripheral   nodes with ecc == diameter (boundary of the graph)\n");
        super::print_str(sink, "  gperiph            alias for graph peripheral\n");
        super::print_str(sink, "  graph center       nodes with ecc == radius (centre of the graph)\n");
        super::print_str(sink, "  gcenter            alias for graph center\n");
        super::print_str(sink, "  graph diameter     combined center+peripheral view: radius/diameter + core/boundary nodes\n");
        super::print_str(sink, "  gdiameter          alias for graph diameter\n");
        super::print_str(sink, "  graph snapshot     save all topology metrics as a monitoring baseline\n");
        super::print_str(sink, "  gsnapshot          alias for graph snapshot\n");
        super::print_str(sink, "  graph compare      diff current metrics against the saved snapshot (delta view)\n");
        super::print_str(sink, "  gcompare           alias for graph compare\n");
        super::print_str(sink, "  graph predict <u> <v>  link prediction: CN, Jaccard, Adamic-Adar, RA for node pair\n");
        super::print_str(sink, "  gpredict <u> <v>   alias for graph predict\n");
        super::print_str(sink, "  graph efficiency   E(G) = \u{2211} 1/d(i,j) / (n*(n-1)) \u{2014} global network efficiency\n");
        super::print_str(sink, "  geff               alias for graph efficiency\n");
        super::print_str(sink, "  graph avg clustering  (1/n)\u{2211} CC(v) \u{2014} true Watts-Strogatz per-node average\n");
        super::print_str(sink, "  gavgcc             alias for graph avg clustering\n");
        super::print_str(sink, "  graph local efficiency  E_loc=(1/n)\u{2211} E(G_v) \u{2014} Latora-Marchiori local efficiency\n");
        super::print_str(sink, "  gleff              alias for graph local efficiency\n");
        super::print_str(sink, "  graph shortest <v> Dijkstra shortest paths from node <v> (directed, weighted)\n");
        super::print_str(sink, "  shortest <v>       alias for graph shortest\n");
        super::print_str(sink, "  graph articulation cut vertices whose removal disconnects the graph\n");
        super::print_str(sink, "  garticulate        alias for graph articulation\n");
        super::print_str(sink, "  graph bridges      cut edges whose removal disconnects the graph\n");
        super::print_str(sink, "  gbridges           alias for graph bridges\n");
        super::print_str(sink, "  graph eulerian     Eulerian circuit/path existence (visit every edge once)\n");
        super::print_str(sink, "  geulerian / euler  aliases for graph eulerian\n");
        super::print_str(sink, "  graph dag longest  longest directed path (critical path) in the DAG\n");
        super::print_str(sink, "  gdaglongest / critical path  aliases for graph dag longest\n");
        super::print_str(sink, "  graph dag layers   topological level per node (parallel execution layers)\n");
        super::print_str(sink, "  gdaglayers / glayers  aliases for graph dag layers\n");
        super::print_str(sink, "  graph domtree <v>  dominator tree from entry <v> (immediate dominator per node)\n");
        super::print_str(sink, "  gdomtree <v> / dominator <v>  aliases for graph domtree\n");
        super::print_str(sink, "  graph feedback arc  feedback arcs (back-edges that cause cycles in the graph)\n");
        super::print_str(sink, "  gfas / feedback arc / gcycledges  aliases for graph feedback arc\n");
        super::print_str(sink, "  graph bipartite match  maximum bipartite matching (Kuhn, optimal A\u{2194}B pairing)\n");
        super::print_str(sink, "  gbimatch / bipartite match  aliases for graph bipartite match\n");
        super::print_str(sink, "  graph 2ecc  2-edge-connected components (nodes resilient to any single link failure)\n");
        super::print_str(sink, "  g2ecc / 2ecc / edge connected components  aliases for graph 2ecc\n");
        super::print_str(sink, "  graph fvs   feedback vertex set (min nodes to remove to break all cycles)\n");
        super::print_str(sink, "  gfvs / feedback vertex set  aliases for graph fvs\n");
        super::print_str(sink, "  graph min cut  global minimum edge cut (Stoer-Wagner; edge connectivity \u{03ba}')\n");
        super::print_str(sink, "  gmincut / min cut / edge connectivity  aliases for graph min cut\n");
        super::print_str(sink, "  graph hamiltonian  Hamiltonian path/circuit (visits every node exactly once)\n");
        super::print_str(sink, "  gham / hamiltonian  aliases for graph hamiltonian\n");
        super::print_str(sink, "  graph chordal  chordal recognition: every 4+ cycle has a chord (LexBFS PEO)\n");
        super::print_str(sink, "  gchordal / chordal  aliases for graph chordal\n");
        super::print_str(sink, "  graph bcc  biconnected components (Tarjan; APs marked 255)\n");
        super::print_str(sink, "  gbcc / biconnected / bcc  aliases for graph bcc\n");
        super::print_str(sink, "  graph ebc  edge betweenness centrality (Brandes; link criticality)\n");
        super::print_str(sink, "  gebc / edge between / ebc  aliases for graph ebc\n");
        super::print_str(sink, "  graph kappa  vertex connectivity k(G) (Even 1975; min vertex cut)\n");
        super::print_str(sink, "  gkappa / vertex connectivity / gvconn  aliases for graph kappa\n");
        super::print_str(sink, "  graph edge color  edge colouring chi'(G): min IPC slots (Vizing 1964)\n");
        super::print_str(sink, "  gedgecolor / edge color / gec  aliases for graph edge color\n");
        super::print_str(sink, "  graph spectral  rho(A) spectral radius + lambda2(L) algebraic connectivity\n");
        super::print_str(sink, "  gspectral / spectral radius / spectral / gspectrum  aliases for graph spectral\n");
        super::print_str(sink, "  graph entropy  H=\u{2212}\u{03a3}p(d)ln(p(d)) Shannon entropy of degree distribution\n");
        super::print_str(sink, "  gentropy / degree entropy  aliases for graph entropy\n");
        super::print_str(sink, "  uname              kernel version + capacity limits (like uname -a + sysctl kern.*)\n");
        super::print_str(sink, "  ver / version      alias for uname\n");
        super::print_str(sink, "  watch              live proc table in VECTOR DECK panel (like watch -n1 proc)\n");
        super::print_str(sink, "  graph watch        alias for watch\n");
        super::print_str(sink, "  watch stop         exit watch mode\n");
        super::print_str(sink, "  unfault <name>  clear a module's restart-loop counter\n");
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
    } else if cmd == "modules" || cmd == "mods" {
        super::set_color(sink, 10, 0);
        super::print_str(sink, " module health\n");
        super::set_color(sink, 7, 0);
        let mut summaries = [gos_supervisor::ModuleStatusSummary {
            handle: gos_protocol::ModuleHandle::ZERO,
            module_id: gos_protocol::ModuleId::ZERO,
            state: gos_protocol::ModuleLifecycle::Stopped,
            fault_policy: gos_protocol::ModuleFaultPolicy::Manual,
            restart_generation: 0,
            degraded: false,
        }; gos_supervisor::MAX_MODULES];
        let count = gos_supervisor::module_status_summaries(&mut summaries);
        if count == 0 {
            super::print_str(sink, "  (no modules installed)\n");
        }
        for summary in summaries.iter().take(count) {
            let raw = summary.module_id.0;
            let mut len = 0;
            while len < raw.len() && raw[len] != 0 {
                len += 1;
            }
            let name = core::str::from_utf8(&raw[..len]).unwrap_or("?");
            super::print_str(sink, "  ");
            super::print_str(sink, name);
            super::print_str(sink, "  state: ");
            super::print_str(sink, super::module_lifecycle_label(summary.state));
            super::print_str(sink, "  policy: ");
            super::print_str(sink, super::module_fault_policy_label(summary.fault_policy));
            super::print_str(sink, "  restarts: ");
            super::print_num_inline(sink, summary.restart_generation as usize);
            if summary.degraded {
                super::print_str(sink, "  DEGRADED");
            }
            super::print_str(sink, "\n");
        }
        if count > 0 {
            super::print_str(sink, "  restart <name>  manually restart a faulted/degraded module\n");
        }
    } else if cmd == "nodes" || cmd == "nodes all" {
        super::dispatch_nodes_list(sink, false);
    } else if cmd == "nodes faulted" || cmd == "nodes fault" || cmd == "faults" {
        super::dispatch_nodes_list(sink, true);
    } else if cmd == "nodes summary" || cmd == "nodes stat" {
        super::dispatch_lifecycle_summary(sink);
    } else if cmd == "plugins" || cmd == "lsmod" || cmd == "plugin list" {
        super::dispatch_plugin_list(sink);
    } else if cmd == "boot" || cmd == "boot verify" || cmd == "boot status" {
        super::dispatch_boot_verify(sink);
    } else if cmd == "metrics export" || cmd == "metrics dump" {
        super::dispatch_metrics_export(sink);
    } else if cmd == "journal" || cmd == "journal status" || cmd == "journal info" {
        super::dispatch_journal_info(sink);
    } else if cmd == "proc" || cmd == "ps" || cmd == "proc all" {
        super::dispatch_proc_list(sink);
    } else if cmd == "node attr list u8" || cmd == "nattr list u8" {
        super::dispatch_node_attr_list_u8(sink);
    } else if cmd == "node attr list" || cmd == "nattr list" {
        super::dispatch_node_attr_list(sink);
    } else if let Some(rest) = cmd
        .strip_prefix("node attr set ")
        .or_else(|| cmd.strip_prefix("nattr set "))
    {
        let parts: (&str, &str) = rest.trim().split_once(' ')
            .map(|(a, b)| (a, b))
            .unwrap_or((rest.trim(), ""));
        let vec_str = parts.0;
        let hex_str = parts.1.trim().trim_start_matches("0x").trim_start_matches("0X");
        if let (Some(vec), Ok(val)) = (
            gos_protocol::VectorAddress::parse(vec_str),
            u32::from_str_radix(hex_str, 16),
        ) {
            super::dispatch_node_attr_set(sink, vec, val);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " usage: node attr set <vec> <hex>  e.g. node attr set 6.1.1.0 00db1c21\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("node attr get ")
        .or_else(|| cmd.strip_prefix("nattr get "))
        .or_else(|| cmd.strip_prefix("node attr "))
        .or_else(|| cmd.strip_prefix("nattr "))
    {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_node_attr_get(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " node attr get requires a vector address (e.g. node attr get 6.1.1.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("node checkpoint ")
        .or_else(|| cmd.strip_prefix("ncp "))
        .or_else(|| cmd.strip_prefix("checkpoint "))
    {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_node_checkpoint(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " node checkpoint requires a vector address (e.g. node checkpoint 6.1.0.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("node stat clear ")
        .or_else(|| cmd.strip_prefix("nstat clear "))
    {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_node_stat_clear(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " node stat clear requires a vector address (e.g. node stat clear 6.1.0.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd.strip_prefix("stat ").or_else(|| cmd.strip_prefix("node stat ")) {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_node_stat(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " stat requires a vector address (e.g. stat 6.1.0.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("kill ")
        .or_else(|| cmd.strip_prefix("node fault "))
        .or_else(|| cmd.strip_prefix("fault "))
    {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_node_kill(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " kill requires a vector address (e.g. kill 6.1.0.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("resume ")
        .or_else(|| cmd.strip_prefix("node resume "))
    {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_node_resume(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " resume requires a vector address (e.g. resume 6.1.0.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("node info ")
        .or_else(|| cmd.strip_prefix("ninfo "))
    {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_node_info(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " node info requires a vector address (e.g. node info 6.1.0.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("node trace clear ")
        .or_else(|| cmd.strip_prefix("ntrace clear "))
    {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_node_trace_clear(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " node trace clear requires a vector address (e.g. node trace clear 6.1.0.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("node trace ")
        .or_else(|| cmd.strip_prefix("ntrace "))
    {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_node_trace(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " node trace requires a vector address (e.g. node trace 6.1.0.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("node log clear ")
        .or_else(|| cmd.strip_prefix("nlog clear "))
    {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_node_log_clear(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " node log clear requires a vector address (e.g. node log clear 6.1.0.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("node log ")
        .or_else(|| cmd.strip_prefix("nlog "))
    {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_node_log(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " node log requires a vector address (e.g. node log 6.1.0.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if cmd == "edges" || cmd == "edges all" {
        super::dispatch_edges_list(sink, None);
    } else if cmd == "edges count" || cmd == "edge count" {
        super::dispatch_edge_count(sink);
    } else if let Some(type_str) = cmd.strip_prefix("edges ") {
        if let Some(et) = super::parse_edge_type_filter(type_str) {
            super::dispatch_edges_list(sink, Some(et));
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " unknown edge type. Types: call spawn depend signal return mount sync stream use\n");
            super::set_color(sink, 7, 0);
        }
    } else if cmd == "graph diff" || cmd == "diff" || cmd == "diff graph" {
        let since = super::GRAPH_DIFF_PIN_EPOCH.load(core::sync::atomic::Ordering::SeqCst);
        super::dispatch_graph_diff(sink, since);
    } else if cmd == "graph diff pin" || cmd == "diff pin" {
        let epoch = gos_runtime::graph_epoch();
        super::GRAPH_DIFF_PIN_EPOCH.store(epoch, core::sync::atomic::Ordering::SeqCst);
        super::set_color(sink, 10, 0);
        super::print_str(sink, " diff baseline pinned at epoch ");
        super::print_num_inline(sink, epoch as usize);
        super::print_str(sink, "\n");
        super::set_color(sink, 7, 0);
    } else if cmd == "graph diff reset" || cmd == "diff reset" {
        super::GRAPH_DIFF_PIN_EPOCH.store(0, core::sync::atomic::Ordering::SeqCst);
        super::set_color(sink, 10, 0);
        super::print_str(sink, " diff baseline reset to epoch 0 (showing all since boot)\n");
        super::set_color(sink, 7, 0);
    } else if let Some(epoch_str) = cmd
        .strip_prefix("graph diff ")
        .or_else(|| cmd.strip_prefix("diff "))
        .filter(|s| *s != "pin" && *s != "reset")
    {
        // `graph diff <N>` — show diff since a specific epoch number supplied inline.
        // "graph diff pin" and "graph diff reset" are already matched above via exact branches.
        let trimmed = epoch_str.trim();
        if let Some(epoch) = super::parse_epoch_decimal(trimmed) {
            super::dispatch_graph_diff(sink, epoch);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " graph diff <epoch>: epoch must be a decimal number (e.g. graph diff 42)\n");
            super::set_color(sink, 7, 0);
        }
    } else if cmd == "uname" || cmd == "uname -a" || cmd == "ver" || cmd == "version" {
        super::dispatch_uname(sink);
    } else if cmd == "watch" || cmd == "graph watch" || cmd == "watch proc" || cmd == "watch nodes" {
        super::dispatch_watch_proc(sink);
    } else if cmd == "watch stop" || cmd == "watch exit" {
        super::dispatch_watch_stop(sink);
    } else if cmd == "graph health" || cmd == "health" {
        super::dispatch_graph_health(sink);
    } else if let Some(pair_str) = cmd.strip_prefix("graph path ") {
        // `graph path <from_vec> <to_vec>`
        let trimmed = pair_str.trim();
        // Find the space separating the two vector addresses.
        // Vector addresses look like "1.2.3.4" — split at the first space.
        if let Some(space) = trimmed.find(' ') {
            let from_str = trimmed[..space].trim();
            let to_str   = trimmed[space + 1..].trim();
            match (
                gos_protocol::VectorAddress::parse(from_str),
                gos_protocol::VectorAddress::parse(to_str),
            ) {
                (Some(from), Some(to)) => super::dispatch_graph_path(sink, from, to),
                (None, _) => {
                    super::set_color(sink, 12, 0);
                    super::print_str(sink, " graph path: invalid from-vector (e.g. 1.0.0.1)\n");
                    super::set_color(sink, 7, 0);
                }
                (_, None) => {
                    super::set_color(sink, 12, 0);
                    super::print_str(sink, " graph path: invalid to-vector (e.g. 1.0.0.4)\n");
                    super::set_color(sink, 7, 0);
                }
            }
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " graph path requires two vector addresses: graph path <from> <to>\n");
            super::set_color(sink, 7, 0);
        }
    } else if cmd == "graph cycles" || cmd == "cycles" || cmd == "graph cyclic" || cmd == "cyclic" {
        super::dispatch_graph_cycles(sink);
    } else if cmd == "graph toposort" || cmd == "toposort" || cmd == "topo sort" || cmd == "graph tsort" || cmd == "tsort" {
        super::dispatch_graph_toposort(sink);
    } else if cmd == "graph scc" || cmd == "scc" || cmd == "graph components" || cmd == "components" {
        super::dispatch_graph_scc(sink);
    } else if cmd == "graph condensation" || cmd == "condensation" || cmd == "condense" || cmd == "graph condense" {
        super::dispatch_graph_condensation(sink);
    } else if cmd == "graph bipartite" || cmd == "bipartite" || cmd == "graph bip" || cmd == "bip" {
        super::dispatch_graph_bipartite(sink);
    } else if cmd == "graph degree" || cmd == "degree" || cmd == "graph hub" || cmd == "hub" {
        super::dispatch_graph_degree(sink);
    } else if cmd == "graph centrality" || cmd == "centrality" || cmd == "graph central" || cmd == "central" || cmd == "betweenness" {
        super::dispatch_graph_centrality(sink);
    } else if cmd == "graph closeness" || cmd == "closeness" || cmd == "graph close" || cmd == "close centrality" || cmd == "cc" {
        super::dispatch_graph_closeness(sink);
    } else if cmd == "graph eccentricity" || cmd == "eccentricity" || cmd == "graph ecc" || cmd == "ecc" || cmd == "graph radius" || cmd == "radius" {
        super::dispatch_graph_eccentricity(sink);
    } else if cmd == "graph katz" || cmd == "katz" || cmd == "kz" || cmd == "graph influence" || cmd == "influence" {
        super::dispatch_graph_katz(sink);
    } else if cmd == "graph pagerank" || cmd == "pagerank" || cmd == "pr" || cmd == "graph rank" || cmd == "rank" {
        super::dispatch_graph_pagerank(sink);
    } else if cmd == "graph hits" || cmd == "hits" || cmd == "graph ha" || cmd == "ha" || cmd == "hub authority" {
        super::dispatch_graph_hits(sink);
    } else if cmd == "graph community" || cmd == "community" || cmd == "lpa" || cmd == "graph lpa" || cmd == "graph cluster" || cmd == "cluster" {
        super::dispatch_graph_community(sink);
    } else if cmd == "graph spanning" || cmd == "spanning" || cmd == "span" || cmd == "graph span" || cmd == "graph tree" || cmd == "gtree" {
        super::dispatch_graph_spanning(sink);
    } else if cmd == "graph color" || cmd == "color" || cmd == "gcolor" || cmd == "graph colour" || cmd == "colour" {
        super::dispatch_graph_color(sink);
    } else if cmd == "graph mst" || cmd == "mst" || cmd == "gmst" || cmd == "graph tree mst" || cmd == "min spanning" {
        super::dispatch_graph_mst(sink);
    } else if cmd == "graph sim" || cmd == "sim" || cmd == "gsim" || cmd == "graph walk" || cmd == "walk" {
        super::dispatch_graph_sim(sink, 16);
    } else if let Some(n_str) = cmd
        .strip_prefix("graph sim ")
        .or_else(|| cmd.strip_prefix("sim "))
        .or_else(|| cmd.strip_prefix("gsim "))
        .or_else(|| cmd.strip_prefix("graph walk "))
        .or_else(|| cmd.strip_prefix("walk "))
    {
        let trimmed = n_str.trim();
        if let Some(n_val) = super::parse_epoch_decimal(trimmed) {
            let steps = (n_val as u32).min(256).max(1);
            super::dispatch_graph_sim(sink, steps);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " graph sim: expected a step count 1-256 (e.g. graph sim 32)\n");
            super::set_color(sink, 7, 0);
        }
    } else if cmd == "graph between" || cmd == "between" || cmd == "gbetween"
        || cmd == "graph wbc" || cmd == "wbc" || cmd == "weighted betweenness"
    {
        super::dispatch_graph_between(sink);
    } else if cmd == "graph attractor" || cmd == "attractor" || cmd == "gattractor"
        || cmd == "graph attract" || cmd == "attract"
    {
        super::dispatch_graph_attractor(sink);
    } else if cmd == "graph density" || cmd == "density" || cmd == "gdensity" {
        super::dispatch_graph_density(sink);
    } else if cmd == "graph clustering" || cmd == "clustering" || cmd == "gcluster" {
        super::dispatch_graph_clustering(sink);
    } else if cmd == "graph transitivity" || cmd == "transitivity" || cmd == "gtrans" {
        super::dispatch_graph_transitivity(sink);
    } else if cmd == "graph kcore" || cmd == "kcore" || cmd == "gkcore"
        || cmd == "graph core" || cmd == "core decomp" || cmd == "coreness"
    {
        super::dispatch_graph_kcore(sink);
    } else if cmd == "graph assortativity" || cmd == "assortativity" || cmd == "gassort" {
        super::dispatch_graph_assortativity(sink);
    } else if cmd == "graph reciprocity" || cmd == "reciprocity" || cmd == "grecip" {
        super::dispatch_graph_reciprocity(sink);
    } else if cmd == "graph modularity" || cmd == "modularity" || cmd == "gmodq" {
        super::dispatch_graph_modularity(sink);
    } else if cmd == "graph girth" || cmd == "ggirth" {
        super::dispatch_graph_girth(sink);
    } else if cmd == "graph wiener" || cmd == "gwiener" {
        super::dispatch_graph_wiener(sink);
    } else if cmd == "graph harmonic" || cmd == "gharm" {
        super::dispatch_graph_harmonic(sink);
    } else if cmd == "graph peripheral" || cmd == "gperiph" {
        super::dispatch_graph_peripheral(sink);
    } else if cmd == "graph center" || cmd == "gcenter" {
        super::dispatch_graph_center(sink);
    } else if cmd == "graph efficiency" || cmd == "graph eff" || cmd == "geff" || cmd == "global efficiency" {
        super::dispatch_graph_global_efficiency(sink);
    } else if cmd == "graph avg clustering" || cmd == "gavgcc" {
        super::dispatch_graph_avg_clustering(sink);
    } else if cmd == "graph local efficiency" || cmd == "graph local eff" || cmd == "gleff" || cmd == "local efficiency" {
        super::dispatch_graph_local_efficiency(sink);
    } else if cmd == "graph small world" || cmd == "graph small-world" || cmd == "gsmallworld" || cmd == "small world" {
        super::dispatch_graph_small_world(sink);
    } else if cmd == "graph summary" || cmd == "gsummary" || cmd == "topology summary" || cmd == "topo summary" {
        super::dispatch_graph_summary(sink);
    } else if cmd == "graph scale free" || cmd == "graph scale-free" || cmd == "gscalefree" || cmd == "scale free" {
        super::dispatch_graph_scale_free(sink);
    } else if cmd == "graph power law" || cmd == "graph power-law" || cmd == "gpowerlaw" || cmd == "power law" || cmd == "gpl" {
        super::dispatch_graph_power_law(sink);
    } else if cmd == "graph diameter" || cmd == "gdiameter" {
        super::dispatch_graph_diameter(sink);
    } else if cmd == "graph snapshot" || cmd == "gsnapshot" {
        super::dispatch_graph_snapshot(sink);
    } else if cmd == "graph compare" || cmd == "gcompare" {
        super::dispatch_graph_compare(sink);
    } else if cmd == "graph articulation" || cmd == "garticulate" || cmd == "cut vertices" || cmd == "gcutv" {
        super::dispatch_graph_articulation(sink);
    } else if cmd == "graph bridges" || cmd == "gbridges" || cmd == "cut edges" || cmd == "gcute" {
        super::dispatch_graph_bridges(sink);
    } else if cmd == "graph eulerian" || cmd == "geulerian" || cmd == "eulerian" || cmd == "euler" {
        super::dispatch_graph_eulerian(sink);
    } else if cmd == "graph dag longest" || cmd == "gdaglongest" || cmd == "critical path" || cmd == "graph critical" || cmd == "gcritical" {
        super::dispatch_graph_dag_longest(sink);
    } else if cmd == "graph dag layers" || cmd == "gdaglayers" || cmd == "glayers" || cmd == "dag layers" {
        super::dispatch_graph_dag_layers(sink);
    } else if cmd == "graph feedback arc" || cmd == "gfas" || cmd == "feedback arc" || cmd == "gcycledges" {
        super::dispatch_graph_feedback_arc(sink);
    } else if cmd == "graph bipartite match" || cmd == "gbimatch" || cmd == "bipartite match" {
        super::dispatch_graph_bipartite_match(sink);
    } else if cmd == "graph 2ecc" || cmd == "g2ecc" || cmd == "2ecc" || cmd == "edge connected components" {
        super::dispatch_graph_2ecc(sink);
    } else if cmd == "graph truss" || cmd == "gtruss" || cmd == "truss" || cmd == "k-truss" || cmd == "ktruss" {
        super::dispatch_graph_truss(sink);
    } else if cmd == "graph clique" || cmd == "gclique" || cmd == "clique" || cmd == "max clique" || cmd == "maxclique" {
        super::dispatch_graph_clique(sink);
    } else if cmd == "graph independent set" || cmd == "graph indep" || cmd == "gindep" || cmd == "independent set" || cmd == "indep" {
        super::dispatch_graph_independent_set(sink);
    } else if cmd == "graph vertex cover" || cmd == "gvc" || cmd == "vertex cover" || cmd == "gvertexcover" || cmd == "min vertex cover" || cmd == "gmincover" {
        super::dispatch_graph_vertex_cover(sink);
    } else if cmd == "graph domset" || cmd == "gdomset" || cmd == "dominating set" || cmd == "graph dominating set" || cmd == "gdominate" || cmd == "min domset" {
        super::dispatch_graph_dominating_set(sink);
    } else if cmd == "graph mpc" || cmd == "gmpc" || cmd == "min path cover" || cmd == "graph min path cover" || cmd == "path cover" || cmd == "gdagcover" || cmd == "graph path cover" {
        super::dispatch_graph_min_path_cover(sink);
    } else if cmd == "graph fvs" || cmd == "gfvs" || cmd == "feedback vertex set" || cmd == "graph fvset" || cmd == "gfvset" || cmd == "graph feedback vertex" {
        super::dispatch_graph_fvs(sink);
    } else if cmd == "graph min cut" || cmd == "gmincut" || cmd == "min cut" || cmd == "edge connectivity" || cmd == "gedge connectivity" || cmd == "graph cut" || cmd == "gcut" {
        super::dispatch_graph_min_cut(sink);
    } else if cmd == "graph hamiltonian" || cmd == "gham" || cmd == "hamiltonian" || cmd == "graph ham" || cmd == "ghamiltonian" || cmd == "ham circuit" || cmd == "hamiltonian path" {
        super::dispatch_graph_hamiltonian(sink);
    } else if cmd == "graph chordal" || cmd == "gchordal" || cmd == "chordal" || cmd == "graph chord" || cmd == "gchord" {
        super::dispatch_graph_chordal(sink);
    } else if cmd == "graph bcc" || cmd == "gbcc" || cmd == "biconnected" || cmd == "gbiconn" || cmd == "graph biconnected" || cmd == "bcc" {
        super::dispatch_graph_bcc(sink);
    } else if cmd == "graph ebc" || cmd == "gebc" || cmd == "edge between" || cmd == "edge betweenness" || cmd == "ebc" {
        super::dispatch_graph_betweenness_edge(sink);
    } else if cmd == "graph kappa" || cmd == "gkappa" || cmd == "vertex connectivity" || cmd == "vertex conn" || cmd == "gvertconn" || cmd == "graph vertex conn" || cmd == "graph vconn" {
        super::dispatch_graph_vertex_connectivity(sink);
    } else if cmd == "graph edge color" || cmd == "gedgecolor" || cmd == "edge color" || cmd == "gec" || cmd == "graph ecolor" || cmd == "gecolor" {
        super::dispatch_graph_edge_color(sink);
    } else if cmd == "graph spectral" || cmd == "gspectral" || cmd == "spectral radius" || cmd == "spectral" || cmd == "gspectrum" || cmd == "graph spectrum" {
        super::dispatch_graph_spectral(sink);
    } else if cmd == "graph entropy" || cmd == "gentropy" || cmd == "degree entropy" || cmd == "graph deg entropy" {
        super::dispatch_graph_entropy(sink);
    } else if cmd == "graph zagreb" || cmd == "gzagreb" || cmd == "zagreb" || cmd == "zagreb index" || cmd == "graph topo index" || cmd == "randic" || cmd == "graph randic" {
        super::dispatch_graph_zagreb(sink);
    } else if cmd == "graph topo" || cmd == "gtopo" || cmd == "sum connectivity" || cmd == "gsc" || cmd == "geometric arithmetic" || cmd == "gga" || cmd == "augmented zagreb" || cmd == "gazi" || cmd == "sci ga azi" {
        super::dispatch_graph_topo_indices(sink);
    } else if cmd == "graph topo2" || cmd == "gtopo2" || cmd == "harmonic index" || cmd == "gh index" || cmd == "atom bond connectivity" || cmd == "gabc" || cmd == "forgotten index" || cmd == "gforgotten" || cmd == "ghabcf" {
        super::dispatch_graph_topo_indices2(sink);
    } else if cmd == "graph topo3" || cmd == "gtopo3" || cmd == "symmetric division deg" || cmd == "gsdd" || cmd == "inverse sum indeg" || cmd == "gisi" || cmd == "nirmala index" || cmd == "gnirmala" || cmd == "gsddisini" {
        super::dispatch_graph_topo_indices3(sink);
    } else if cmd == "graph topo4" || cmd == "gtopo4" || cmd == "sombor index" || cmd == "gsombor" || cmd == "reduced zagreb" || cmd == "grm2" || cmd == "sigma index" || cmd == "gsigma" || cmd == "gsomborrm2sigma" {
        super::dispatch_graph_topo_indices4(sink);
    } else if cmd == "graph topo5" || cmd == "gtopo5" || cmd == "hyper zagreb" || cmd == "ghm1" || cmd == "hm2 index" || cmd == "ghm2" || cmd == "arithmetic geometric" || cmd == "gag" || cmd == "ghm1hm2ag" {
        super::dispatch_graph_topo_indices5(sink);
    } else if cmd == "graph topo6" || cmd == "gtopo6" || cmd == "reformulated zagreb" || cmd == "gem1" || cmd == "atom bond sum" || cmd == "gabs" || cmd == "reduced reciprocal randic" || cmd == "grrr" || cmd == "gem1absrrr" {
        super::dispatch_graph_topo_indices6(sink);
    } else if cmd == "graph topo7" || cmd == "gtopo7" || cmd == "wiener index" || cmd == "gwiener" || cmd == "harary index" || cmd == "gharary" || cmd == "hyper wiener" || cmd == "ghyperw" || cmd == "gwienerhw" {
        super::dispatch_graph_topo_indices7(sink);
    } else if cmd == "graph topo8" || cmd == "gtopo8" || cmd == "eccentric connectivity" || cmd == "geci" || cmd == "graph eci" || cmd == "graph diameter" || cmd == "gdiameter" || cmd == "graph radius" || cmd == "gradius" || cmd == "gecidrc" {
        super::dispatch_graph_topo_indices8(sink);
    } else if cmd == "graph topo9" || cmd == "gtopo9" || cmd == "schultz mti" || cmd == "gws" || cmd == "gutman index" || cmd == "gwg" || cmd == "connective eccentric" || cmd == "gcxe" || cmd == "gwsgwgcxe" {
        super::dispatch_graph_topo_indices9(sink);
    } else if cmd == "graph topo10" || cmd == "gtopo10" || cmd == "szeged index" || cmd == "gszeged" || cmd == "revised szeged" || cmd == "grszg" || cmd == "mostar index" || cmd == "gmostar" || cmd == "gszgrsmo" {
        super::dispatch_graph_topo_indices10(sink);
    } else if cmd == "graph topo11" || cmd == "gtopo11" || cmd == "balaban j" || cmd == "gbalaban" || cmd == "transmission irregularity" || cmd == "gti" || cmd == "vertex pi" || cmd == "gpiv" || cmd == "gjtipiv" {
        super::dispatch_graph_topo_indices11(sink);
    } else if cmd == "graph topo12" || cmd == "gtopo12" || cmd == "zagreb eccentricity" || cmd == "gzagreecc" || cmd == "m1 eccentricity" || cmd == "gm1e" || cmd == "m2 eccentricity" || cmd == "gm2e" || cmd == "m3 eccentricity" || cmd == "gm3e" || cmd == "gm1em2em3e" {
        super::dispatch_graph_topo_indices12(sink);
    } else if cmd == "graph topo13" || cmd == "gtopo13" || cmd == "transmission zagreb" || cmd == "gtm1tm2" || cmd == "tm1 index" || cmd == "gtm1" || cmd == "tm2 index" || cmd == "gtm2" || cmd == "geometric arithmetic transmission" || cmd == "ggat" || cmd == "gtm1tm2gat" {
        super::dispatch_graph_topo_indices13(sink);
    } else if cmd == "graph topo14" || cmd == "gtopo14" || cmd == "total eccentricity" || cmd == "gte" || cmd == "eccentric distance sum" || cmd == "geds" || cmd == "geometric arithmetic eccentricity" || cmd == "ggea" || cmd == "gteedsge" || cmd == "gteedsegea" {
        super::dispatch_graph_topo_indices14(sink);
    } else if cmd == "graph topo15" || cmd == "gtopo15" || cmd == "leap zagreb" || cmd == "gleapzagreb" || cmd == "lm1 index" || cmd == "glm1" || cmd == "lm2 index" || cmd == "glm2" || cmd == "lm3 index" || cmd == "glm3" || cmd == "glm1lm2lm3" {
        super::dispatch_graph_topo_indices15(sink);
    } else if cmd == "graph topo16" || cmd == "gtopo16" || cmd == "product connectivity" || cmd == "gpc" || cmd == "reciprocal randic" || cmd == "grr" || cmd == "lanzhou index" || cmd == "glz" || cmd == "gpcrrlz" {
        super::dispatch_graph_topo_indices16(sink);
    } else if cmd == "graph topo17" || cmd == "gtopo17" || cmd == "zagreb coindex" || cmd == "gcoindex" || cmd == "complement zagreb" || cmd == "gcozagreb" || cmd == "forgotten coindex" || cmd == "gfbar" || cmd == "gm1barm2barfbar" {
        super::dispatch_graph_topo_indices17(sink);
    } else if cmd == "graph topo18" || cmd == "gtopo18" || cmd == "neighborhood zagreb" || cmd == "gnm1nm2" || cmd == "nm1 index" || cmd == "gnm1" || cmd == "nm2 index" || cmd == "gnm2" || cmd == "neighborhood ga" || cmd == "gga2" || cmd == "gnm1nm2ga2" {
        super::dispatch_graph_topo_indices18(sink);
    } else if cmd == "graph topo19" || cmd == "gtopo19" || cmd == "reverse wiener" || cmd == "grw" || cmd == "reciprocal complementary wiener" || cmd == "grcw" || cmd == "terminal wiener" || cmd == "gtw" || cmd == "grwrcwtw" {
        super::dispatch_graph_topo_indices19(sink);
    } else if cmd == "graph topo20" || cmd == "gtopo20" || cmd == "modified sombor" || cmd == "gsostar" || cmd == "reciprocal sombor" || cmd == "grso" || cmd == "reduced sombor" || cmd == "grsom" || cmd == "gsostarsombrsom" {
        super::dispatch_graph_topo_indices20(sink);
    } else if cmd == "graph topo21" || cmd == "gtopo21" || cmd == "abc4 index" || cmd == "gabc4" || cmd == "neighborhood harmonic" || cmd == "gnh" || cmd == "neighborhood sombor" || cmd == "gnso" || cmd == "gabc4nhnso" {
        super::dispatch_graph_topo_indices21(sink);
    } else if cmd == "graph topo22" || cmd == "gtopo22" || cmd == "neighborhood randic" || cmd == "gnr" || cmd == "neighborhood forgotten" || cmd == "gnf" || cmd == "neighborhood sumconn" || cmd == "gnsc" || cmd == "gnrnfnsc" {
        super::dispatch_graph_topo_indices22(sink);
    } else if cmd == "graph topo23" || cmd == "gtopo23" || cmd == "neighborhood hm1" || cmd == "gnhm1" || cmd == "neighborhood sdd" || cmd == "gnsdd" || cmd == "neighborhood m3" || cmd == "gnm3" || cmd == "gnhm1nsddnm3" {
        super::dispatch_graph_topo_indices23(sink);
    } else if cmd == "graph topo24" || cmd == "gtopo24" || cmd == "neighborhood isi" || cmd == "gnisi" || cmd == "neighborhood azi" || cmd == "gnazi" || cmd == "neighborhood em1" || cmd == "gnem1" || cmd == "gnisinazinemm1" {
        super::dispatch_graph_topo_indices24(sink);
    } else if cmd == "graph topo25" || cmd == "gtopo25" || cmd == "neighborhood hm2" || cmd == "gnhm2" || cmd == "neighborhood ag" || cmd == "gnag" || cmd == "neighborhood abs" || cmd == "gnabs" || cmd == "gnhm2nagnabs" {
        super::dispatch_graph_topo_indices25(sink);
    } else if cmd == "graph topo26" || cmd == "gtopo26" || cmd == "neighborhood product conn" || cmd == "gnpc" || cmd == "neighborhood reduced zagreb2" || cmd == "gnrm2" || cmd == "neighborhood reciprocal sombor" || cmd == "gnrso" || cmd == "gnpcnrm2nrso" {
        super::dispatch_graph_topo_indices26(sink);
    } else if cmd == "graph topo27" || cmd == "gtopo27" || cmd == "neighborhood reciprocal randic" || cmd == "gnrr" || cmd == "neighborhood modified sombor" || cmd == "gnsos" || cmd == "neighborhood reduced sombor" || cmd == "gnrso2" || cmd == "gnrrnsosnrso" {
        super::dispatch_graph_topo_indices27(sink);
    } else if cmd == "graph topo28" || cmd == "gtopo28" || cmd == "neighborhood nirmala" || cmd == "gnni" || cmd == "neighborhood modified nirmala" || cmd == "gnnmi" || cmd == "gnsm1" || cmd == "gnnigsm1" || cmd == "gnninnminsm1" {
        super::dispatch_graph_topo_indices28(sink);
    } else if cmd == "graph topo29" || cmd == "gtopo29" || cmd == "neighborhood zero randic" || cmd == "gnz0" || cmd == "neighborhood em2" || cmd == "gnem2" || cmd == "neighborhood sqrt vertex" || cmd == "gnse" || cmd == "gnz0nem2nse" {
        super::dispatch_graph_topo_indices29(sink);
    } else if cmd == "graph topo30" || cmd == "gtopo30" || cmd == "neighborhood quartic" || cmd == "gnvq" || cmd == "neighborhood randic32" || cmd == "gnrgs" || cmd == "neighborhood cubic sum" || cmd == "gnhcs" || cmd == "gnvqnrgsnhcs" {
        super::dispatch_graph_topo_indices30(sink);
    } else if cmd == "graph topo31" || cmd == "gtopo31" || cmd == "neighborhood sigma" || cmd == "gnsig" || cmd == "neighborhood quartic edge" || cmd == "gnhqs" || cmd == "neighborhood penta" || cmd == "gnps" || cmd == "gnsignhqsnps" {
        super::dispatch_graph_topo_indices31(sink);
    } else if cmd == "graph topo32" || cmd == "gtopo32" || cmd == "neighborhood hextic" || cmd == "gnsh" || cmd == "neighborhood quintic edge" || cmd == "gnhps" || cmd == "neighborhood weighted sombor" || cmd == "gnwso" || cmd == "gnshnhpsnwso" {
        super::dispatch_graph_topo_indices32(sink);
    } else if cmd == "graph topo33" || cmd == "gtopo33" || cmd == "neighborhood heptic" || cmd == "gnshp" || cmd == "neighborhood sextic edge" || cmd == "gnhse" || cmd == "neighborhood cubic sombor" || cmd == "gncso" || cmd == "gnshpnhsencso" {
        super::dispatch_graph_topo_indices33(sink);
    } else if cmd == "graph topo34" || cmd == "gtopo34" || cmd == "neighborhood octic" || cmd == "gnoc" || cmd == "neighborhood septic edge" || cmd == "gnhhs" || cmd == "neighborhood fourth sombor" || cmd == "gnfso" || cmd == "gnocnhhsnfso" {
        super::dispatch_graph_topo_indices34(sink);
    } else if cmd == "graph topo35" || cmd == "gtopo35" || cmd == "neighborhood nonic" || cmd == "gnnc" || cmd == "neighborhood octic edge" || cmd == "gnhoc" || cmd == "neighborhood hextic sombor" || cmd == "gnhso" || cmd == "gnncnhocnhso" {
        super::dispatch_graph_topo_indices35(sink);
    } else if cmd == "graph topo36" || cmd == "gtopo36" || cmd == "neighborhood decic" || cmd == "gndc" || cmd == "neighborhood nonic edge" || cmd == "gnhnc" || cmd == "neighborhood octic sombor" || cmd == "gnoso" || cmd == "gndcnhncnoso" {
        super::dispatch_graph_topo_indices36(sink);
    } else if cmd == "graph topo37" || cmd == "gtopo37" || cmd == "neighborhood undecic" || cmd == "gnuc" || cmd == "neighborhood decic edge" || cmd == "gnhdc" || cmd == "neighborhood tenth sombor" || cmd == "gntso" || cmd == "gnucnhdcntso" {
        super::dispatch_graph_topo_indices37(sink);
    } else if cmd == "graph topo38" || cmd == "gtopo38" || cmd == "neighborhood dodecic" || cmd == "gndoc" || cmd == "neighborhood undecic edge" || cmd == "gnhuc" || cmd == "neighborhood duodecic sombor" || cmd == "gndso" || cmd == "gndocnhucndso" {
        super::dispatch_graph_topo_indices38(sink);
    } else if cmd == "graph topo39" || cmd == "gtopo39" || cmd == "neighborhood tridecic" || cmd == "gntc" || cmd == "neighborhood dodecic edge" || cmd == "gnhdoc" || cmd == "neighborhood tetradecic sombor" || cmd == "gneso" || cmd == "gntcnhdocneso" {
        super::dispatch_graph_topo_indices39(sink);
    } else if cmd == "graph topo40" || cmd == "gtopo40" || cmd == "neighborhood tetradecic" || cmd == "gnqtc" || cmd == "neighborhood tridecic edge" || cmd == "gnhtc" || cmd == "neighborhood hexadecic sombor" || cmd == "gngso" || cmd == "gnqtcnhtcngso" {
        super::dispatch_graph_topo_indices40(sink);
    } else if cmd == "graph topo41" || cmd == "gtopo41" || cmd == "neighborhood pentadecic" || cmd == "gnptc" || cmd == "neighborhood tetradecic edge" || cmd == "gnhqtc" || cmd == "neighborhood octadecic sombor" || cmd == "gnioso" || cmd == "gnptcnhqtcnioso" {
        super::dispatch_graph_topo_indices41(sink);
    } else if cmd == "graph topo42" || cmd == "gtopo42" || cmd == "neighborhood hexadecic" || cmd == "gnstc" || cmd == "neighborhood pentadecic edge" || cmd == "gnhptc" || cmd == "neighborhood eicosic sombor" || cmd == "gnjso" || cmd == "gnstcnhptcnjso" {
        super::dispatch_graph_topo_indices42(sink);
    } else if cmd == "graph topo43" || cmd == "gtopo43" || cmd == "neighborhood heptadecic" || cmd == "gnheptc" || cmd == "neighborhood hexadecic edge" || cmd == "gnhstc" || cmd == "neighborhood docosic sombor" || cmd == "gnkso" || cmd == "gnheptcnhstcnkso" {
        super::dispatch_graph_topo_indices43(sink);
    } else if cmd == "graph topo44" || cmd == "gtopo44" || cmd == "neighborhood octadecic" || cmd == "gnoctc" || cmd == "neighborhood heptadecic edge" || cmd == "gnhoctc" || cmd == "neighborhood tetracosic sombor" || cmd == "gnlso" || cmd == "gnocthoctclso" {
        super::dispatch_graph_topo_indices44(sink);
    } else if cmd == "graph topo45" || cmd == "gtopo45" || cmd == "neighborhood nonadecic" || cmd == "gnnontc" || cmd == "neighborhood octadecic edge" || cmd == "gnhnontc" || cmd == "neighborhood hexacosic sombor" || cmd == "gnmso" || cmd == "gnnontcnhnontcnmso" {
        super::dispatch_graph_topo_indices45(sink);
    } else if cmd == "graph topo46" || cmd == "gtopo46" || cmd == "neighborhood eicosic" || cmd == "gneictc" || cmd == "neighborhood nonadecic edge" || cmd == "gnheictc" || cmd == "neighborhood octacosic sombor" || cmd == "gnnso" || cmd == "gneictcnheictcnnso" {
        super::dispatch_graph_topo_indices46(sink);
    } else if cmd == "graph topo47" || cmd == "gtopo47" || cmd == "neighborhood heneicosic" || cmd == "gnhentc" || cmd == "neighborhood eicosic edge" || cmd == "gnhhentc" || cmd == "neighborhood triacontyl sombor" || cmd == "gnpso" || cmd == "gnhentcnhhentcnpso" {
        super::dispatch_graph_topo_indices47(sink);
    } else if cmd == "graph topo65" || cmd == "gtopo65" || cmd == "neighborhood nonatriacontic" || cmd == "gnnnonatriactc" || cmd == "neighborhood octatriacontic edge" || cmd == "gnnhnonatriactc" || cmd == "neighborhood hexahexacontyl sombor" || cmd == "gnnahso" || cmd == "gnnnonatriactcnhnonatriactcnahso" {
        super::dispatch_graph_topo_indices65(sink);
    } else if cmd == "graph topo66" || cmd == "gtopo66" || cmd == "neighborhood tetracontic" || cmd == "gntetraactc" || cmd == "neighborhood nonatriacontic edge" || cmd == "gnhtetraactc" || cmd == "neighborhood octahexacontyl sombor" || cmd == "gnnaiso" || cmd == "gntetraactcnhtetraactcnaiso" {
        super::dispatch_graph_topo_indices66(sink);
    } else if cmd == "graph topo67" || cmd == "gtopo67" || cmd == "neighborhood hentetracontic" || cmd == "gnhentetraactc" || cmd == "neighborhood tetracontic edge" || cmd == "gnhhentetraactc" || cmd == "neighborhood tetracontyl sombor" || cmd == "gnnajso" || cmd == "gnhentetraactcnhhentetraactcnajso" {
        super::dispatch_graph_topo_indices67(sink);
    } else if cmd == "graph topo68" || cmd == "gtopo68" || cmd == "neighborhood dotetracontic" || cmd == "gndotetraactc" || cmd == "neighborhood hentetracontic edge" || cmd == "gnhdotetraactc" || cmd == "neighborhood dotetracontyl sombor" || cmd == "gnnakso" || cmd == "gndotetraactcnhdotetraactcnakso" {
        super::dispatch_graph_topo_indices68(sink);
    } else if cmd == "graph topo69" || cmd == "gtopo69" || cmd == "neighborhood tritetracontic" || cmd == "gntritetraactc" || cmd == "neighborhood dotetracontic edge" || cmd == "gnhtritetraactc" || cmd == "neighborhood tritetracontyl sombor" || cmd == "gnnalso" || cmd == "gntritetraactcnhtritetraactcnalso" {
        super::dispatch_graph_topo_indices69(sink);
    } else if cmd == "graph topo70" || cmd == "gtopo70" || cmd == "neighborhood tetratetracontic" || cmd == "gntetratetraactc" || cmd == "neighborhood tritetracontic edge" || cmd == "gnhtetratetraactc" || cmd == "neighborhood tetratetracontyl sombor" || cmd == "gnnamso" || cmd == "gntetratetraactcnhtetratetraactcnamso" {
        super::dispatch_graph_topo_indices70(sink);
    } else if cmd == "graph topo71" || cmd == "gtopo71" || cmd == "neighborhood pentatetracontic" || cmd == "gnpentetraactc" || cmd == "neighborhood tetratetracontic edge" || cmd == "gnhpentetraactc" || cmd == "neighborhood pentatetracontyl sombor" || cmd == "gnnanso" || cmd == "gnpentetraactcnhpentetraactcnanso" {
        super::dispatch_graph_topo_indices71(sink);
    } else if cmd == "graph topo82" || cmd == "gtopo82" || cmd == "neighborhood hexapentacontic" || cmd == "gnhexpentaactc" || cmd == "neighborhood pentapentacontic edge" || cmd == "gnnhhexpentaactc" || cmd == "neighborhood centyl sombor" || cmd == "gnnayso" || cmd == "gnhexpentaactcnhhexpentaactcnayso" {
        super::dispatch_graph_topo_indices82(sink);
    } else if cmd == "graph topo81" || cmd == "gtopo81" || cmd == "neighborhood pentapentacontic" || cmd == "gnpentapentaactc" || cmd == "neighborhood tetrapentacontic edge" || cmd == "gnhpentapentaactc" || cmd == "neighborhood octanonacontyl sombor" || cmd == "gnnaxso" || cmd == "gnpentapentaactcnhpentapentaactcnaxso" {
        super::dispatch_graph_topo_indices81(sink);
    } else if cmd == "graph topo80" || cmd == "gtopo80" || cmd == "neighborhood tetrapentacontic" || cmd == "gntetrapentaactc" || cmd == "neighborhood tripentacontic edge" || cmd == "gnhtetrapentaactc" || cmd == "neighborhood hexanonacontyl sombor" || cmd == "gnnawso" || cmd == "gntetrapentaactcnhtetrapentaactcnawso" {
        super::dispatch_graph_topo_indices80(sink);
    } else if cmd == "graph topo79" || cmd == "gtopo79" || cmd == "neighborhood tripentacontic" || cmd == "gntripentaactc" || cmd == "neighborhood dopentacontic edge" || cmd == "gnhtripentaactc" || cmd == "neighborhood tetranonacontyl sombor" || cmd == "gnnavso" || cmd == "gntripentaactcnhtripentaactcnavso" {
        super::dispatch_graph_topo_indices79(sink);
    } else if cmd == "graph topo78" || cmd == "gtopo78" || cmd == "neighborhood dopentacontic" || cmd == "gndopentaactc" || cmd == "neighborhood henpentacontic edge" || cmd == "gnnhdopentaactc" || cmd == "neighborhood dinonacontyl sombor" || cmd == "gnnauso" || cmd == "gndopentaactcnhdopentaactcnauso" {
        super::dispatch_graph_topo_indices78(sink);
    } else if cmd == "graph topo77" || cmd == "gtopo77" || cmd == "neighborhood henpentacontic" || cmd == "gnhenpentaactc" || cmd == "neighborhood pentacontic edge" || cmd == "gnnhhenpentaactc" || cmd == "neighborhood nonacontyl sombor" || cmd == "gnnatso" || cmd == "gnhenpentaactcnhhenpentaactcnatso" {
        super::dispatch_graph_topo_indices77(sink);
    } else if cmd == "graph topo76" || cmd == "gtopo76" || cmd == "neighborhood pentacontic" || cmd == "gnpentaactc" || cmd == "neighborhood nonapentacontic edge" || cmd == "gnhpentaactc" || cmd == "neighborhood octaocontyl sombor" || cmd == "gnnasso" || cmd == "gnpentaactcnhpentaactcnasso" {
        super::dispatch_graph_topo_indices76(sink);
    } else if cmd == "graph topo75" || cmd == "gtopo75" || cmd == "neighborhood nonatetracontic" || cmd == "gnnnonatetraactc" || cmd == "neighborhood octotetracontic edge" || cmd == "gnnhnonatetraactc" || cmd == "neighborhood hexaoctacontyl sombor" || cmd == "gnnarso" || cmd == "gnnnonatetraactcnhnonatetraactcnarso" {
        super::dispatch_graph_topo_indices75(sink);
    } else if cmd == "graph topo74" || cmd == "gtopo74" || cmd == "neighborhood octotetracontic" || cmd == "gnoctotetraactc" || cmd == "neighborhood heptotetracontic edge" || cmd == "gnhoctotetraactc" || cmd == "neighborhood tetrahexacontyl sombor" || cmd == "gnnaqso" || cmd == "gnoctotetraactcnhoctotetraactcnaqso" {
        super::dispatch_graph_topo_indices74(sink);
    } else if cmd == "graph topo73" || cmd == "gtopo73" || cmd == "neighborhood heptatetracontic" || cmd == "gnheptetraactc" || cmd == "neighborhood hexatetracontic edge" || cmd == "gnhheptetraactc" || cmd == "neighborhood docosacontyl sombor" || cmd == "gnnapso" || cmd == "gnheptetraactcnhheptetraactcnapso" {
        super::dispatch_graph_topo_indices73(sink);
    } else if cmd == "graph topo72" || cmd == "gtopo72" || cmd == "neighborhood hexatetracontic" || cmd == "gnhextetraactc" || cmd == "neighborhood pentatetracontic edge" || cmd == "gnhhextetraactc" || cmd == "neighborhood octacontyl sombor" || cmd == "gnnaoso" || cmd == "gnhextetraactcnhhextetraactcnaoso" {
        super::dispatch_graph_topo_indices72(sink);
    } else if cmd == "graph topo64" || cmd == "gtopo64" || cmd == "neighborhood octatriacontic" || cmd == "gnoctatriactc" || cmd == "neighborhood heptatriacontic edge" || cmd == "gnhoctatriactc" || cmd == "neighborhood tetrahexacontyl sombor" || cmd == "gnnagso" || cmd == "gnoctatriactcnhoctatriactcnagso" {
        super::dispatch_graph_topo_indices64(sink);
    } else if cmd == "graph topo63" || cmd == "gtopo63" || cmd == "neighborhood heptatriacontic" || cmd == "gnheptatriactc" || cmd == "neighborhood hexatriacontic edge" || cmd == "gnnhheptatriactc" || cmd == "neighborhood hexahexacontyl sombor" || cmd == "gnafso" || cmd == "gnheptatriactcnhheptatriactcnafso" {
        super::dispatch_graph_topo_indices63(sink);
    } else if cmd == "graph topo62" || cmd == "gtopo62" || cmd == "neighborhood hexatriacontic" || cmd == "gnhexatriactc" || cmd == "neighborhood pentatriacontic edge" || cmd == "gnnhhexatriactc" || cmd == "neighborhood hexacontyl sombor" || cmd == "gnnaeso" || cmd == "gnhexatriactcnhhexatriactcnaeso" {
        super::dispatch_graph_topo_indices62(sink);
    } else if cmd == "graph topo61" || cmd == "gtopo61" || cmd == "neighborhood pentatriacontic" || cmd == "gnpenttriactc" || cmd == "neighborhood tetratriacontic edge" || cmd == "gnhpenttriactc" || cmd == "neighborhood octopentacontyl sombor" || cmd == "gnadso" || cmd == "gnpenttriactcnhpenttriactcnadso" {
        super::dispatch_graph_topo_indices61(sink);
    } else if cmd == "graph topo60" || cmd == "gtopo60" || cmd == "neighborhood tetratriacontic" || cmd == "gntetrtriactc" || cmd == "neighborhood tritriacontic edge" || cmd == "gnhtetrtriactc" || cmd == "neighborhood hexapentacontyl sombor" || cmd == "gnnacso" || cmd == "gntetrtriactcnhtetrtriactcnacso" {
        super::dispatch_graph_topo_indices60(sink);
    } else if cmd == "graph topo59" || cmd == "gtopo59" || cmd == "neighborhood tritriacontic" || cmd == "gntritriactc" || cmd == "neighborhood dotriacontic edge" || cmd == "gnhtritriactc" || cmd == "neighborhood dopentatecontyl sombor" || cmd == "gnnabso" || cmd == "gntritriactcnhtritriactcnabso" {
        super::dispatch_graph_topo_indices59(sink);
    } else if cmd == "graph topo58" || cmd == "gtopo58" || cmd == "neighborhood dotriacontic" || cmd == "gndotriactc" || cmd == "neighborhood hentriacontic edge" || cmd == "gnhdotriactc" || cmd == "neighborhood dopentecontyl sombor" || cmd == "gnnaaso" || cmd == "gndotriactcnhdotriactcnaaso" {
        super::dispatch_graph_topo_indices58(sink);
    } else if cmd == "graph topo57" || cmd == "gtopo57" || cmd == "neighborhood hentriacontic" || cmd == "gnhentriactc" || cmd == "neighborhood triacontic edge" || cmd == "gnnhentriactc" || cmd == "neighborhood pentacontyl sombor" || cmd == "gnbso" || cmd == "gnhentriactcnhhentriactcnbso" {
        super::dispatch_graph_topo_indices57(sink);
    } else if cmd == "graph topo56" || cmd == "gtopo56" || cmd == "neighborhood triacontyl" || cmd == "gntriac" || cmd == "gntriactl" || cmd == "gntriactc" || cmd == "neighborhood nonacosic edge" || cmd == "gnhtriactc" || cmd == "neighborhood octatetracontyl sombor" || cmd == "gnaso" || cmd == "gntriactcnhtriactcnaso" {
        super::dispatch_graph_topo_indices56(sink);
    } else if cmd == "graph topo55" || cmd == "gtopo55" || cmd == "neighborhood nonacosic" || cmd == "gnnon atc" || cmd == "gnnona tc" || cmd == "gnnonatc" || cmd == "neighborhood octacosic edge" || cmd == "gnhnonatc" || cmd == "neighborhood hexatetracontyl sombor" || cmd == "gnzso" || cmd == "gnnonatcnhnonatcnzso" {
        super::dispatch_graph_topo_indices55(sink);
    } else if cmd == "graph topo54" || cmd == "gtopo54" || cmd == "neighborhood octacosic" || cmd == "gnoctatc" || cmd == "neighborhood heptacosic edge" || cmd == "gnhoctatc" || cmd == "neighborhood tetratetracontyl sombor" || cmd == "gnyso" || cmd == "gnoctatcnhoctatcnyso" {
        super::dispatch_graph_topo_indices54(sink);
    } else if cmd == "graph topo53" || cmd == "gtopo53" || cmd == "neighborhood heptacosic" || cmd == "gnheptatc" || cmd == "neighborhood hexacosic edge" || cmd == "gnhheptatc" || cmd == "neighborhood dotetracontyl sombor" || cmd == "gnxso" || cmd == "gnheptatcnhheptatcnxso" {
        super::dispatch_graph_topo_indices53(sink);
    } else if cmd == "graph topo52" || cmd == "gtopo52" || cmd == "neighborhood hexacosic" || cmd == "gnhexatc" || cmd == "neighborhood pentacosic edge" || cmd == "gnhhexatc" || cmd == "neighborhood tetracontyl sombor" || cmd == "gnvso" || cmd == "gnhexatcnhhexatcnvso" {
        super::dispatch_graph_topo_indices52(sink);
    } else if cmd == "graph topo51" || cmd == "gtopo51" || cmd == "neighborhood pentacosic" || cmd == "gnpenttc" || cmd == "neighborhood tetracosic edge" || cmd == "gnhpenttc" || cmd == "neighborhood octatriacontyl sombor" || cmd == "gnuso" || cmd == "gnpenttcnhpenttcnuso" {
        super::dispatch_graph_topo_indices51(sink);
    } else if cmd == "graph topo50" || cmd == "gtopo50" || cmd == "neighborhood tetracosic" || cmd == "gntetrtc" || cmd == "neighborhood tricosic edge" || cmd == "gnhtetrtc" || cmd == "neighborhood hexatriacontyl sombor" || cmd == "gnsso" || cmd == "gntetrtcnhtetrtcnsso" {
        super::dispatch_graph_topo_indices50(sink);
    } else if cmd == "graph topo49" || cmd == "gtopo49" || cmd == "neighborhood tricosic" || cmd == "gntrictc" || cmd == "neighborhood docosic edge" || cmd == "gnhtrictc" || cmd == "neighborhood tetratriacontyl sombor" || cmd == "gnrso" || cmd == "gntrictcnhtrictcnrso" {
        super::dispatch_graph_topo_indices49(sink);
    } else if cmd == "graph topo48" || cmd == "gtopo48" || cmd == "neighborhood docosic" || cmd == "gndoctc" || cmd == "neighborhood heneicosic edge" || cmd == "gnhdoctc" || cmd == "neighborhood dotriacontyl sombor" || cmd == "gnqso" || cmd == "gndoctcnhdoctcnqso" {
        super::dispatch_graph_topo_indices48(sink);
    } else if let Some(vec_str) = cmd
        .strip_prefix("graph arborescence ")
        .or_else(|| cmd.strip_prefix("garborescence "))
        .or_else(|| cmd.strip_prefix("arborescence "))
        .or_else(|| cmd.strip_prefix("gmsa "))
        .or_else(|| cmd.strip_prefix("min arborescence "))
    {
        match gos_protocol::VectorAddress::parse(vec_str.trim()) {
            Some(root) => super::dispatch_graph_arborescence(sink, root),
            None => {
                super::set_color(sink, 12, 0);
                super::print_str(sink, " graph arborescence: expected <root> vector (e.g. graph arborescence 1.0.0.1)\n");
                super::set_color(sink, 7, 0);
            }
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("graph domtree ")
        .or_else(|| cmd.strip_prefix("gdomtree "))
        .or_else(|| cmd.strip_prefix("dominator "))
        .or_else(|| cmd.strip_prefix("gdom "))
    {
        match gos_protocol::VectorAddress::parse(vec_str.trim()) {
            Some(start) => super::dispatch_graph_domtree(sink, start),
            None => {
                super::set_color(sink, 12, 0);
                super::print_str(sink, " graph domtree: expected <start> vector (e.g. graph domtree 1.0.0.1)\n");
                super::set_color(sink, 7, 0);
            }
        }
    } else if let Some(pair_str) = cmd
        .strip_prefix("graph predict ")
        .or_else(|| cmd.strip_prefix("gpredict "))
        .or_else(|| cmd.strip_prefix("link predict "))
        .or_else(|| cmd.strip_prefix("predict "))
    {
        let trimmed = pair_str.trim();
        if let Some(space) = trimmed.find(' ') {
            let u_s = trimmed[..space].trim();
            let v_s = trimmed[space + 1..].trim();
            match (gos_protocol::VectorAddress::parse(u_s), gos_protocol::VectorAddress::parse(v_s)) {
                (Some(u), Some(v)) => super::dispatch_graph_predict(sink, u, v),
                _ => {
                    super::set_color(sink, 12, 0);
                    super::print_str(sink, " graph predict: expected <u> <v> (e.g. graph predict 1.0.0.1 1.0.0.2)\n");
                    super::set_color(sink, 7, 0);
                }
            }
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " graph predict: expected <u> <v> (e.g. graph predict 1.0.0.1 1.0.0.2)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(k_str) = cmd
        .strip_prefix("graph rich club ")
        .or_else(|| cmd.strip_prefix("richclub "))
        .or_else(|| cmd.strip_prefix("grichclub "))
    {
        let k_trimmed = k_str.trim();
        let mut k_val: u8 = 1;
        let mut k_valid = !k_trimmed.is_empty();
        if k_valid {
            let mut v: u16 = 0;
            for b in k_trimmed.bytes() {
                if b < b'0' || b > b'9' { k_valid = false; break; }
                v = v.saturating_mul(10).saturating_add((b - b'0') as u16);
                if v > 255 { k_valid = false; break; }
            }
            if k_valid { k_val = v as u8; }
        }
        if k_valid {
            super::dispatch_graph_rich_club(sink, k_val);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " graph rich club: k must be 0\u{2013}255, e.g. `graph rich club 2`\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(pair_str) = cmd
        .strip_prefix("graph flow ")
        .or_else(|| cmd.strip_prefix("flow "))
        .or_else(|| cmd.strip_prefix("max flow ")
            .or_else(|| cmd.strip_prefix("maxflow ")))
    {
        // Expect "<source_vec> <sink_vec>"
        let trimmed = pair_str.trim();
        if let Some(space) = trimmed.find(' ') {
            let src_s = trimmed[..space].trim();
            let snk_s = trimmed[space + 1..].trim();
            match (gos_protocol::VectorAddress::parse(src_s), gos_protocol::VectorAddress::parse(snk_s)) {
                (Some(src), Some(snk)) => super::dispatch_graph_flow(sink, src, snk),
                _ => {
                    super::set_color(sink, 12, 0);
                    super::print_str(sink, " graph flow: expected <source> <sink> (e.g. graph flow 1.0.0.1 1.0.0.2)\n");
                    super::set_color(sink, 7, 0);
                }
            }
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " graph flow: expected <source> <sink> (e.g. graph flow 1.0.0.1 1.0.0.2)\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("graph shortest ")
        .or_else(|| cmd.strip_prefix("shortest "))
        .or_else(|| cmd.strip_prefix("graph dijkstra "))
        .or_else(|| cmd.strip_prefix("dijkstra "))
    {
        match gos_protocol::VectorAddress::parse(vec_str.trim()) {
            Some(src) => super::dispatch_graph_shortest(sink, src),
            None => {
                super::set_color(sink, 12, 0);
                super::print_str(sink, " graph shortest: invalid vector (e.g. 1.0.0.1)\n");
                super::set_color(sink, 7, 0);
            }
        }
    } else if let Some(vec_str) = cmd
        .strip_prefix("graph reachable ")
        .or_else(|| cmd.strip_prefix("reachable "))
        .or_else(|| cmd.strip_prefix("reach "))
        .or_else(|| cmd.strip_prefix("graph reach "))
    {
        if let Some(vec) = gos_protocol::VectorAddress::parse(vec_str.trim()) {
            super::dispatch_graph_reachable(sink, vec);
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " graph reachable requires a vector address (e.g. graph reachable 6.1.0.0)\n");
            super::set_color(sink, 7, 0);
        }
    } else if cmd == "graph topo" || cmd == "topo" {
        super::dispatch_graph_topo(sink, None);
    } else if let Some(l4_str) = cmd
        .strip_prefix("graph topo ")
        .or_else(|| cmd.strip_prefix("topo "))
    {
        let trimmed = l4_str.trim();
        if let Some(epoch_val) = super::parse_epoch_decimal(trimmed) {
            if epoch_val <= 255 {
                super::dispatch_graph_topo(sink, Some(epoch_val as u8));
            } else {
                super::set_color(sink, 12, 0);
                super::print_str(sink, " graph topo <L4>: l4 domain must be 0-255\n");
                super::set_color(sink, 7, 0);
            }
        } else {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " graph topo <L4>: l4 must be a decimal number 0-255\n");
            super::set_color(sink, 7, 0);
        }
    } else if let Some(name) = cmd.strip_prefix("unfault ") {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " usage: unfault <module-name>\n");
        } else {
            let mut upper = [0u8; 16];
            let bytes = trimmed.as_bytes();
            let len = bytes.len().min(16);
            for i in 0..len {
                upper[i] = bytes[i].to_ascii_uppercase();
            }
            let upper_str = core::str::from_utf8(&upper[..len]).unwrap_or("");
            let module_id = gos_protocol::ModuleId::from_ascii(upper_str);
            let mut summaries = [gos_supervisor::ModuleStatusSummary {
                handle: gos_protocol::ModuleHandle::ZERO,
                module_id: gos_protocol::ModuleId::ZERO,
                state: gos_protocol::ModuleLifecycle::Stopped,
                fault_policy: gos_protocol::ModuleFaultPolicy::Manual,
                restart_generation: 0,
                degraded: false,
            }; gos_supervisor::MAX_MODULES];
            let count = gos_supervisor::module_status_summaries(&mut summaries);
            let found = summaries.iter().take(count).find(|s| s.module_id == module_id);
            match found {
                Some(summary) => match gos_supervisor::clear_restart_history(summary.handle) {
                    Ok(()) => {
                        super::set_color(sink, 10, 0);
                        super::print_str(sink, " restart history cleared for ");
                        super::print_str(sink, trimmed);
                        super::print_str(sink, "\n");
                    }
                    Err(_) => {
                        super::set_color(sink, 12, 0);
                        super::print_str(sink, " clear failed (see `modules` for fault policy)\n");
                    }
                },
                None => {
                    super::set_color(sink, 12, 0);
                    super::print_str(sink, " unknown module: ");
                    super::print_str(sink, trimmed);
                    super::print_str(sink, "  (see `modules` for installed names)\n");
                }
            }
            super::set_color(sink, 7, 0);
        }
    } else if let Some(name) = cmd.strip_prefix("restart ") {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            super::set_color(sink, 12, 0);
            super::print_str(sink, " usage: restart <module-name>\n");
        } else {
            let mut upper = [0u8; 16];
            let bytes = trimmed.as_bytes();
            let len = bytes.len().min(16);
            for i in 0..len {
                upper[i] = bytes[i].to_ascii_uppercase();
            }
            let upper_str = core::str::from_utf8(&upper[..len]).unwrap_or("");
            let module_id = gos_protocol::ModuleId::from_ascii(upper_str);
            match gos_supervisor::module_handle_for_id(module_id) {
                Some(handle) => match gos_supervisor::restart_module(handle) {
                    Ok(()) => {
                        super::set_color(sink, 10, 0);
                        super::print_str(sink, " restarting ");
                        super::print_str(sink, trimmed);
                        super::set_color(sink, 7, 0);
                        if let Ok(state) = gos_supervisor::module_lifecycle(handle) {
                            super::print_str(sink, "  state: ");
                            super::print_str(sink, super::module_lifecycle_label(state));
                        }
                        super::print_str(sink, "\n");
                    }
                    Err(_) => {
                        super::set_color(sink, 12, 0);
                        super::print_str(sink, " restart failed (see `modules` for fault policy)\n");
                    }
                },
                None => {
                    super::set_color(sink, 12, 0);
                    super::print_str(sink, " unknown module: ");
                    super::print_str(sink, trimmed);
                    super::print_str(sink, "  (see `modules` for installed names)\n");
                }
            }
            super::set_color(sink, 7, 0);
        }
    } else if cmd == "sup" || cmd == "supervisor" {
        super::set_color(sink, 10, 0);
        super::print_str(sink, " supervisor snapshot\n");
        super::set_color(sink, 7, 0);
        match gos_supervisor::snapshot() {
            Ok(snap) => {
                super::print_str(sink, "  modules    installed:");
                super::print_num_inline(sink, snap.installed_modules);
                super::print_str(sink, "  running:");
                super::print_num_inline(sink, snap.running_modules);
                super::print_str(sink, "\n");
                super::print_str(sink, "  instances  live:");
                super::print_num_inline(sink, snap.live_instances);
                super::print_str(sink, "  ready:");
                super::print_num_inline(sink, snap.ready_instances);
                super::print_str(sink, "  waiting:");
                super::print_num_inline(sink, snap.waiting_instances);
                super::print_str(sink, "  suspended:");
                super::print_num_inline(sink, snap.suspended_instances);
                super::print_str(sink, "\n");
                super::print_str(sink, "  templates  registered:");
                super::print_num_inline(sink, snap.registered_templates);
                super::print_str(sink, "  domains:");
                super::print_num_inline(sink, snap.isolated_domains);
                super::print_str(sink, "\n");
                super::print_str(sink, "  resources  registered:");
                super::print_num_inline(sink, snap.registered_resources);
                super::print_str(sink, "  claims:");
                super::print_num_inline(sink, snap.active_claims);
                super::print_str(sink, "  revokes:");
                super::print_num_inline(sink, snap.pending_revocations);
                super::print_str(sink, "  restarts_q:");
                super::print_num_inline(sink, snap.queued_restarts);
                super::print_str(sink, "\n");
                super::print_str(sink, "  memory     heap_grants:");
                super::print_num_inline(sink, snap.heap_grants);
                super::print_str(sink, "  heap_pages:");
                super::print_num_inline(sink, snap.heap_pages_used);
                super::print_str(sink, "\n");
                super::print_str(sink, "  ipc        caps:");
                super::print_num_inline(sink, snap.published_capabilities);
                super::print_str(sink, "  endpoints:");
                super::print_num_inline(sink, snap.endpoints);
                super::print_str(sink, "  queued_msgs:");
                super::print_num_inline(sink, snap.queued_messages);
                super::print_str(sink, "\n");
                super::print_str(sink, "  lanes      ctrl:");
                super::print_num_inline(sink, snap.ready_control);
                super::print_str(sink, "  io:");
                super::print_num_inline(sink, snap.ready_io);
                super::print_str(sink, "  compute:");
                super::print_num_inline(sink, snap.ready_compute);
                super::print_str(sink, "  bg:");
                super::print_num_inline(sink, snap.ready_background);
                super::print_str(sink, "\n");
            }
            Err(_) => {
                super::set_color(sink, 12, 0);
                super::print_str(sink, " supervisor not bootstrapped\n");
            }
        }
    } else if cmd == "resources" || cmd == "res" {
        super::set_color(sink, 10, 0);
        super::print_str(sink, " instance resources\n");
        super::set_color(sink, 7, 0);
        let mut summaries = [gos_supervisor::InstanceResourceSummary {
            instance_id: gos_protocol::NodeInstanceId::ZERO,
            module: gos_protocol::ModuleHandle::ZERO,
            lifecycle: gos_protocol::NodeInstanceLifecycle::Stopped,
            heap_pages_used: 0,
            heap_pages_max: 0,
            gpu_bytes_used: 0,
            gpu_bytes_max: 0,
        }; gos_supervisor::MAX_INSTANCES];
        let count = gos_supervisor::instance_resource_summaries(&mut summaries);
        if count == 0 {
            super::print_str(sink, "  (no live instances)\n");
        }
        let mut total_heap_used: u64 = 0;
        let mut total_heap_max: u64 = 0;
        let mut total_gpu_used: u64 = 0;
        let mut total_gpu_max: u64 = 0;
        for summary in summaries.iter().take(count) {
            super::print_str(sink, "  instance#");
            super::print_num_inline(sink, summary.instance_id.0 as usize);
            super::print_str(sink, "  state: ");
            super::print_str(sink, super::instance_lifecycle_label(summary.lifecycle));
            super::print_str(sink, "  heap: ");
            super::print_num_inline(sink, summary.heap_pages_used as usize);
            super::print_str(sink, "/");
            super::print_num_inline(sink, summary.heap_pages_max as usize);
            super::print_str(sink, " pages  gpu: ");
            super::print_num_inline(sink, summary.gpu_bytes_used as usize);
            super::print_str(sink, "/");
            super::print_num_inline(sink, summary.gpu_bytes_max as usize);
            super::print_str(sink, " bytes\n");
            total_heap_used += summary.heap_pages_used as u64;
            total_heap_max += summary.heap_pages_max as u64;
            total_gpu_used += summary.gpu_bytes_used;
            total_gpu_max += summary.gpu_bytes_max;
        }
        if count > 0 {
            super::print_str(sink, "  total  heap: ");
            super::print_num_inline(sink, total_heap_used as usize);
            super::print_str(sink, "/");
            super::print_num_inline(sink, total_heap_max as usize);
            super::print_str(sink, " pages  gpu: ");
            super::print_num_inline(sink, total_gpu_used as usize);
            super::print_str(sink, "/");
            super::print_num_inline(sink, total_gpu_max as usize);
            super::print_str(sink, " bytes\n");
        }
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
    if bytes.is_empty() || bytes.iter().any(|b| !b.is_ascii_digit()) {
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
