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
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

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
        "gos-xtask\n\nverbs:\n  check    cargo check -p gos-kernel (kernel target)\n  test     run every host-side harness\n  lint     cargo clippy on kernel + each host harness, -D warnings\n  all      check + test + lint (default)\n  verify   alias for all (future: graph-architecture verifier)\n  help     this message"
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
