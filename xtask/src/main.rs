//! gos-xtask — single-entry build/test/verify command for GOS.
//!
//! Background: the workspace root `.cargo/config.toml` pins the build
//! target to `x86_64-gos-kernel.json`, which is correct for kernel
//! crates but breaks any `cargo` invocation that targets host code
//! (host harnesses, this binary itself).  Each host-side crate works
//! around it with its own `.cargo/config.toml` override — this xtask
//! ties those invocations together so a contributor never has to know
//! the convention.
//!
//! Verbs:
//!   check       — `cargo check -p gos-kernel` against the kernel target
//!   test        — run every host-side test harness
//!   all         — check + test (default)
//!   verify      — placeholder, currently same as `all`; future home for
//!                 the Rust port of `tools/verify-graph-architecture.ps1`
//!
//! Invocation: `cd xtask && cargo run -- <verb>` (no top-level alias
//! works around the global target pin).

use std::env;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let verb = args.get(1).map(String::as_str).unwrap_or("all");

    let workspace_root = match locate_workspace_root() {
        Some(path) => path,
        None => {
            eprintln!("xtask: could not locate workspace root (looked for Cargo.lock)");
            return ExitCode::from(2);
        }
    };
    println!("xtask: workspace root = {}", workspace_root.display());

    let result = match verb {
        "check" => run_check(&workspace_root),
        "test" => run_test(&workspace_root),
        "lint" => run_lint(&workspace_root),
        "qemu" => run_qemu_smoke(&workspace_root),
        "all" | "verify" => run_check(&workspace_root)
            .and_then(|_| run_test(&workspace_root))
            .and_then(|_| run_lint(&workspace_root)),
        "help" | "--help" | "-h" => {
            print_help();
            return ExitCode::SUCCESS;
        }
        other => {
            eprintln!("xtask: unknown verb '{}'. try `xtask help`.", other);
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => {
            println!("xtask: {} ok", verb);
            ExitCode::SUCCESS
        }
        Err(code) => {
            eprintln!("xtask: {} failed (exit {})", verb, code);
            ExitCode::from(code)
        }
    }
}

fn print_help() {
    println!(
        "gos-xtask\n\nverbs:\n  check    cargo check -p gos-kernel (kernel target)\n  test     run every host-side harness\n  lint     cargo clippy on kernel + each host harness, -D warnings\n  qemu     boot kernel under QEMU; pass once steady-state marker seen\n  all      check + test + lint (default)\n  verify   alias for all (future: graph-architecture verifier)\n  help     this message"
    );
}

fn run_check(root: &Path) -> Result<(), u8> {
    println!("xtask: cargo check -p gos-kernel");
    let status = Command::new("cargo")
        .args(["check", "-p", "gos-kernel"])
        .current_dir(root)
        .status();
    forward_status(status)
}

fn run_test(root: &Path) -> Result<(), u8> {
    let harnesses = [
        "host-tests/gos-supervisor-harness",
        "host-tests/gos-runtime-harness",
        "host-tests/gos-gfx-harness",
    ];
    for harness in harnesses {
        println!("xtask: cargo test in {}", harness);
        let status = Command::new("cargo")
            .arg("test")
            .current_dir(root.join(harness))
            .status();
        forward_status(status)?;
    }
    Ok(())
}

fn run_lint(root: &Path) -> Result<(), u8> {
    // Lint policy:
    //   * `-D warnings`             -> deny rustc warnings (dead_code,
    //                                  unused_*, improper_ctypes, ...)
    //   * `-A clippy::all`           -> clippy lints are advisory for now;
    //                                  the long tail (explicit_counter_loop,
    //                                  new_without_default, needless_range_
    //                                  loop, ...) is a follow-up cleanup.
    //                                  Future slices can opt back into
    //                                  specific categories via `-W
    //                                  clippy::<group>`.
    let lint_args = ["--", "-D", "warnings", "-A", "clippy::all"];

    println!("xtask: cargo clippy -p gos-kernel  (rustc warnings denied)");
    let mut kernel = vec!["clippy", "-p", "gos-kernel"];
    kernel.extend(lint_args);
    let status = Command::new("cargo")
        .args(&kernel)
        .current_dir(root)
        .status();
    forward_status(status)?;

    let harnesses = [
        "host-tests/gos-supervisor-harness",
        "host-tests/gos-runtime-harness",
        "host-tests/gos-gfx-harness",
    ];
    for harness in harnesses {
        println!("xtask: cargo clippy --all-targets  (in {})", harness);
        let mut argv = vec!["clippy", "--all-targets"];
        argv.extend(lint_args);
        let status = Command::new("cargo")
            .args(&argv)
            .current_dir(root.join(harness))
            .status();
        forward_status(status)?;
    }
    Ok(())
}

/// Phase D.2 — QEMU smoke verb.
///
/// Boots the kernel under QEMU via the workspace's `bootimage runner`
/// (configured in `.cargo/config.toml`), tails stdout for the steady-
/// state marker emitted right before the kernel starts servicing
/// interrupts, then kills the child.  Pass if the marker is seen
/// within `QEMU_SMOKE_TIMEOUT_SECS`; fail otherwise.
///
/// This is the foundation Phase I `gfx-smoke` and `gfx-interact` will
/// build on — once Vulkan host-bridge lands, the same harness will
/// add a `gfx: first frame submitted` marker check on top of this one.
const QEMU_SMOKE_TIMEOUT_SECS: u64 = 90;
const QEMU_SMOKE_MARKER: &str = "boot: enabling interrupts; entering steady-state";

fn run_qemu_smoke(root: &Path) -> Result<(), u8> {
    println!(
        "xtask: qemu smoke — booting kernel, watching for `{}` (timeout {}s)",
        QEMU_SMOKE_MARKER, QEMU_SMOKE_TIMEOUT_SECS
    );

    // `cargo run -p gos-kernel` invokes the bootimage runner configured
    // in .cargo/config.toml, which in turn launches qemu-system-x86_64
    // with `-serial stdio`.  That redirects the kernel's serial port
    // straight onto our captured stdout, which is what we tail below.
    let mut child = match Command::new("cargo")
        .args(["run", "-p", "gos-kernel"])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(err) => {
            eprintln!("xtask: failed to spawn cargo run: {}", err);
            return Err(1);
        }
    };
    let child_pid = child.id();

    let found = Arc::new(AtomicBool::new(false));
    let mut reader_threads = Vec::new();
    for (label, pipe) in [
        ("stdout", child.stdout.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>)),
        ("stderr", child.stderr.take().map(|p| Box::new(p) as Box<dyn std::io::Read + Send>)),
    ] {
        if let Some(pipe) = pipe {
            let found = Arc::clone(&found);
            let label = label.to_string();
            reader_threads.push(thread::spawn(move || {
                let reader = BufReader::new(pipe);
                for line in reader.lines().map_while(Result::ok) {
                    println!("[{}] {}", label, line);
                    if line.contains(QEMU_SMOKE_MARKER) {
                        found.store(true, Ordering::Release);
                    }
                }
            }));
        }
    }

    let deadline = Instant::now() + Duration::from_secs(QEMU_SMOKE_TIMEOUT_SECS);
    let outcome = loop {
        if found.load(Ordering::Acquire) {
            break Ok(());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                // Kernel exited on its own — only counts as success if
                // the marker was also observed before exit.
                if found.load(Ordering::Acquire) {
                    break Ok(());
                }
                eprintln!(
                    "xtask: qemu exited before marker (status: {})",
                    status.code().unwrap_or(-1)
                );
                break Err(2);
            }
            Ok(None) => {}
            Err(err) => {
                eprintln!("xtask: try_wait failed: {}", err);
                break Err(1);
            }
        }
        if Instant::now() >= deadline {
            eprintln!(
                "xtask: qemu smoke TIMEOUT after {}s — marker never seen",
                QEMU_SMOKE_TIMEOUT_SECS
            );
            break Err(3);
        }
        thread::sleep(Duration::from_millis(250));
    };

    // Cleanup: kill cargo (and on Windows, taskkill /t cascades to
    // descendant qemu-system-x86_64.exe).  We deliberately don't
    // .wait() here — on success the kernel runs forever; the user-
    // visible measurement is whether the marker appeared.
    let _ = kill_child_tree(&mut child, child_pid);
    for handle in reader_threads {
        let _ = handle.join();
    }

    match outcome {
        Ok(()) => {
            println!("xtask: qemu smoke PASS — marker observed");
            Ok(())
        }
        Err(code) => Err(code),
    }
}

fn kill_child_tree(child: &mut std::process::Child, pid: u32) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        // On Windows the bootimage runner spawns qemu-system-x86_64.exe
        // as a child of cargo; SIGKILL on cargo alone orphans QEMU and
        // leaves the disk image locked for the next run.  taskkill /T
        // walks the tree.
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .status();
        // Best-effort fallback in case taskkill couldn't find cargo.
        let _ = child.kill();
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = pid; // suppress unused warning
        child.kill()
    }
}

fn forward_status(status: std::io::Result<std::process::ExitStatus>) -> Result<(), u8> {
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(s.code().map(|c| c as u8).unwrap_or(1)),
        Err(err) => {
            eprintln!("xtask: failed to spawn cargo: {}", err);
            Err(1)
        }
    }
}

/// Walks up from CWD until it finds the kernel workspace root —
/// identified by the presence of `Cargo.lock` *and* a `crates/`
/// directory.  Falls back to None if not found within 10 levels.
fn locate_workspace_root() -> Option<PathBuf> {
    let mut cur = env::current_dir().ok()?;
    for _ in 0..10 {
        let lock = cur.join("Cargo.lock");
        let crates = cur.join("crates");
        if lock.is_file() && crates.is_dir() {
            return Some(cur);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}
