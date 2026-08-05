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
//!   image       — ADR-018: build a UEFI disk image (bootloader_api
//!                 0.11.9, UEFI-only) from the compiled gos-kernel binary
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
        "image" => build_uefi_image(&workspace_root, release_flag(&args)).map(|_| ()),
        "qemu" => run_qemu_smoke(&workspace_root, release_flag(&args)),
        "run" => run_interactive(&workspace_root, release_flag(&args)),
        "check-interfaces" => run_check_interfaces(&workspace_root),
        "all" | "verify" => run_check(&workspace_root)
            .and_then(|_| run_test(&workspace_root))
            .and_then(|_| run_lint(&workspace_root))
            .and_then(|_| run_check_interfaces(&workspace_root)),
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
        "gos-xtask\n\nverbs:\n  check               cargo check -p gos-kernel (kernel target)\n  test                run every host-side harness\n  lint                cargo clippy on kernel + each host harness, -D warnings\n  image [--release]   build a UEFI disk image (ADR-018) from the compiled kernel\n  run [--release]     interactive QEMU+OVMF boot with the full dev flag set (was `cargo run`)\n  qemu [--release]    boot kernel under QEMU+OVMF; pass once steady-state marker seen\n  check-interfaces    verify interfaces/plugins.yaml matches Rust builtin_bundle\n  all                 check + test + lint + check-interfaces (default)\n  verify              alias for all\n  help                this message"
    );
}

fn release_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "--release")
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

    // Phase I.1.1 — gos-gfx-bridge-host runs under the *stable*
    // toolchain (its rust-toolchain.toml pins stable; see that file
    // for the rationale).  The kernel-pinned nightly's `build-std`
    // setting bleeds into wgpu's 200-crate graph and either OOMs
    // rustc-LLVM or trips E0152 duplicate lang items, so we route
    // through `rustup run stable cargo test` here instead of
    // delegating to the default toolchain.  When the kernel nightly
    // pin advances past the blockers, this branch collapses back
    // into the loop above.
    let bridge_host = "crates/gos-gfx-bridge-host";
    println!("xtask: rustup run stable cargo test in {}", bridge_host);
    let status = Command::new("rustup")
        .args(["run", "stable", "cargo", "test"])
        .env_remove("CARGO_TARGET_DIR")
        .current_dir(root.join(bridge_host))
        .status();
    forward_status(status)?;

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

/// Resolves the same `target/` directory `cargo` itself would use —
/// respects `CARGO_TARGET_DIR` (set machine-wide in this project's dev
/// environments to a shared cache dir outside the repo) rather than
/// assuming `<root>/target`.
fn cargo_target_dir(root: &Path) -> PathBuf {
    env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"))
}

fn kernel_binary_path(root: &Path, release: bool) -> PathBuf {
    cargo_target_dir(root)
        .join("x86_64-gos-kernel")
        .join(if release { "release" } else { "debug" })
        .join("gos-kernel")
}

/// ADR-018 — build a UEFI disk image from the compiled `gos-kernel`
/// binary via `bootloader::UefiBoot` (pinned `=0.11.9`, `uefi`-only
/// feature — see xtask/Cargo.toml's doc comment for why). Builds the
/// kernel first as a fully separate `cargo build` invocation (not an
/// artifact-dependency — rust-lang/cargo#10444/#10647 make that panic
/// for a non-default-target artifact dep on this project's pinned
/// nightly, see ADR-018 §四) against `crates/gos-kernel`'s existing
/// custom target/build-std config, unchanged.
fn build_uefi_image(root: &Path, release: bool) -> Result<PathBuf, u8> {
    println!("xtask: cargo build -p gos-kernel{}", if release { " --release" } else { "" });
    let mut build_args = vec!["build", "-p", "gos-kernel"];
    if release {
        build_args.push("--release");
    }
    let status = Command::new("cargo").args(&build_args).current_dir(root).status();
    forward_status(status)?;

    let kernel_path = kernel_binary_path(root, release);
    if !kernel_path.is_file() {
        eprintln!("xtask: expected kernel binary at {} but it doesn't exist", kernel_path.display());
        return Err(2);
    }

    let image_path = cargo_target_dir(root).join("x86_64-gos-kernel").join(if release { "release" } else { "debug" }).join("gos-kernel-uefi.img");
    println!("xtask: building UEFI disk image from {}", kernel_path.display());
    bootloader::UefiBoot::new(&kernel_path)
        .create_disk_image(&image_path)
        .map_err(|err| {
            eprintln!("xtask: UefiBoot::create_disk_image failed: {err}");
            2u8
        })?;
    println!("xtask: wrote {}", image_path.display());
    Ok(image_path)
}

/// Copies the OVMF/EDK2 UEFI firmware into the target dir so QEMU's
/// `-drive if=pflash` (which needs a writable handle even with
/// `readonly=on`) doesn't hit a Windows ACL denial against a stock
/// `Program Files` install — verified the hard way in the ADR-018
/// spike (`拒绝访问` / access denied pointing straight at the source).
fn writable_ovmf_copy(root: &Path) -> Result<PathBuf, u8> {
    let source = env::var_os("OVMF_CODE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:/Program Files/qemu/share/edk2-x86_64-code.fd"));
    if !source.is_file() {
        eprintln!(
            "xtask: OVMF firmware not found at {} (set OVMF_CODE to override)",
            source.display()
        );
        return Err(2);
    }
    let dest = cargo_target_dir(root).join("ovmf-code.fd");
    std::fs::copy(&source, &dest).map_err(|err| {
        eprintln!("xtask: failed to copy OVMF firmware to {}: {}", dest.display(), err);
        2u8
    })?;
    Ok(dest)
}

/// Phase D.2 — QEMU smoke verb.
///
/// ADR-018: boots the UEFI disk image (built via `build_uefi_image`)
/// under QEMU+OVMF directly (no longer the `bootimage runner` —
/// bootloader 0.9's `bootimage` subcommand doesn't exist for a
/// bootloader_api 0.11 kernel), tails stdout for the steady-state
/// marker emitted right before the kernel starts servicing interrupts,
/// then kills the child.  Pass if the marker is seen within
/// `QEMU_SMOKE_TIMEOUT_SECS`; fail otherwise.
///
/// This is the foundation Phase I `gfx-smoke` and `gfx-interact` will
/// build on — once Vulkan host-bridge lands, the same harness will
/// add a `gfx: first frame submitted` marker check on top of this one.
const QEMU_SMOKE_TIMEOUT_SECS: u64 = 90;
// ADR-018: this previously read "boot: enabling interrupts; entering
// steady-state", which doesn't match any string main.rs actually emits
// (found while verifying the UEFI migration boots -- a real kernel
// reaching steady state, e.g. via the vk-input polling loop, was being
// misreported as a smoke-test failure). The real log line is emitted
// right before the steady-state loop starts.
const QEMU_SMOKE_MARKER: &str = "interrupts enabled";

fn run_qemu_smoke(root: &Path, release: bool) -> Result<(), u8> {
    let image_path = build_uefi_image(root, release)?;
    let ovmf_path = writable_ovmf_copy(root)?;

    println!(
        "xtask: qemu smoke — booting kernel, watching for `{}` (timeout {}s)",
        QEMU_SMOKE_MARKER, QEMU_SMOKE_TIMEOUT_SECS
    );

    let mut child = match Command::new("qemu-system-x86_64")
        .arg("-drive")
        .arg(format!("if=pflash,format=raw,readonly=on,file={}", ovmf_path.display()))
        .arg("-drive")
        .arg(format!("format=raw,file={}", image_path.display()))
        .arg("-serial")
        .arg("stdio")
        .arg("-display")
        .arg("none")
        .arg("-no-reboot")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(err) => {
            eprintln!("xtask: failed to spawn qemu-system-x86_64: {}", err);
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

/// ADR-018 — interactive replacement for the old `cargo run -p
/// gos-kernel` (which worked via `bootimage runner` +
/// `[package.metadata.bootimage] run-args` in crates/gos-kernel/Cargo.toml
/// -- both gone along with bootloader 0.9). Carries forward the same
/// dev-flag set those `run-args` had: WHPX/TCG accel, COM1 to stdio,
/// COM2/COM3 as TCP servers for gos-vk-viewer (14444/14445), the QEMU
/// monitor on 45555, and an e1000 NIC on user-mode networking. Runs
/// until the user kills it (Ctrl+C) or QEMU exits on its own.
fn run_interactive(root: &Path, release: bool) -> Result<(), u8> {
    let image_path = build_uefi_image(root, release)?;
    let ovmf_path = writable_ovmf_copy(root)?;

    println!("xtask: launching QEMU (interactive) — Ctrl+C to stop");
    let status = Command::new("qemu-system-x86_64")
        .arg("-drive")
        .arg(format!("if=pflash,format=raw,readonly=on,file={}", ovmf_path.display()))
        .arg("-drive")
        .arg(format!("format=raw,file={}", image_path.display()))
        .args(["-accel", "whpx", "-accel", "tcg"])
        .args(["-serial", "stdio"])
        .args(["-serial", "tcp:127.0.0.1:14444,server,nowait"])
        .args(["-serial", "tcp:127.0.0.1:14445,server,nowait"])
        .arg("-no-reboot")
        .args(["-monitor", "telnet:127.0.0.1:45555,server,nowait"])
        .args(["-netdev", "user,id=gosnet0"])
        .args(["-device", "e1000,netdev=gosnet0,mac=52:54:00:12:34:56"])
        .status();
    forward_status(status)
}

/// L+ — validate that interfaces/plugins.yaml mentions exactly the
/// same `plugin_id`s as the Rust-side `BuiltinPluginDescriptor`
/// constants in `crates/gos-kernel/src/builtin_bundle.rs`.  Catches
/// drift between the human-readable contract and the source of truth.
///
/// We don't use serde_yaml — keeping xtask dependency-free and the
/// parse trivial (`plugin_id: K_*` line scan vs `K_*_ID` const scan).
fn run_check_interfaces(root: &Path) -> Result<(), u8> {
    use std::collections::BTreeSet;
    use std::fs;

    let yaml_path = root.join("interfaces").join("plugins.yaml");
    let bundle_path = root.join("crates").join("gos-kernel").join("src").join("builtin_bundle.rs");

    let yaml_text = match fs::read_to_string(&yaml_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("xtask: cannot read {}: {}", yaml_path.display(), e);
            return Err(2);
        }
    };
    let bundle_text = match fs::read_to_string(&bundle_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("xtask: cannot read {}: {}", bundle_path.display(), e);
            return Err(2);
        }
    };

    // YAML: scan for `plugin_id: K_FOO` (with optional surrounding
    // whitespace/quotes) — strict enough to ignore comments + arbitrary
    // value formatting.
    let mut yaml_ids: BTreeSet<String> = BTreeSet::new();
    for line in yaml_text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let trimmed = line.strip_prefix("- plugin_id:").or_else(|| line.strip_prefix("plugin_id:"));
        if let Some(rest) = trimmed {
            let val = rest
                .trim()
                .trim_matches(|c: char| c == '"' || c == '\'')
                .to_string();
            if !val.is_empty() {
                yaml_ids.insert(val);
            }
        }
    }

    // Rust: scan for `const K_FOO_ID: PluginId = PluginId::from_ascii("K_FOO");`.
    let mut rust_ids: BTreeSet<String> = BTreeSet::new();
    for line in bundle_text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("const ") || !trimmed.contains("PluginId::from_ascii(\"") {
            continue;
        }
        // Extract the string literal between from_ascii(" and the next ")
        if let Some(start) = trimmed.find("from_ascii(\"") {
            let after = &trimmed[start + "from_ascii(\"".len()..];
            if let Some(end) = after.find('"') {
                let val = &after[..end];
                if !val.is_empty() {
                    rust_ids.insert(val.to_string());
                }
            }
        }
    }

    let missing_in_yaml: Vec<&String> = rust_ids.difference(&yaml_ids).collect();
    let extra_in_yaml: Vec<&String> = yaml_ids.difference(&rust_ids).collect();

    println!("xtask: check-interfaces");
    println!("  plugins.yaml ids:    {}", yaml_ids.len());
    println!("  builtin_bundle ids:  {}", rust_ids.len());

    if missing_in_yaml.is_empty() && extra_in_yaml.is_empty() {
        println!("  ✓ contracts in sync");
        return Ok(());
    }
    if !missing_in_yaml.is_empty() {
        eprintln!("  ✗ in Rust but missing from plugins.yaml:");
        for id in &missing_in_yaml {
            eprintln!("      {}", id);
        }
    }
    if !extra_in_yaml.is_empty() {
        eprintln!("  ✗ in plugins.yaml but missing from Rust:");
        for id in &extra_in_yaml {
            eprintln!("      {}", id);
        }
    }
    Err(1)
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
