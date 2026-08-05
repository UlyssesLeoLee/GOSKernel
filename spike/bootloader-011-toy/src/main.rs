// ADR-018 spike -- avoids `-Z bindeps` artifact-dependencies entirely
// (rust-lang/cargo#10444 / #10647: panics "no entry found for key" when
// an artifact dependency specifies a non-default target, still open on
// this project's pinned nightly-2026-04-02). Instead: the kernel is
// built as a completely separate `cargo build` invocation, and this
// tool only knows the resulting binary's path -- the same "two
// independent build steps" shape as the current bootimage-based
// pipeline, just swapping the image-building half for
// `bootloader::UefiBoot`.
//
// UEFI-only (see ../Cargo.toml's `features = ["uefi"]`): bootloader
// 0.11.9's own bundled BIOS-stage target JSON files hit a schema this
// nightly's rustc rejects (ADR-018 §六), and UEFI is the only mode this
// project's real target hardware (2014 Mac mini) needs anyway.
//
// Run from a location with NO ancestor .cargo/config.toml conflicts --
// nested under E:\GOSKernel, this needs its own local .cargo/config.toml
// (see ./.cargo/config.toml and kernel/.cargo/config.toml) to escape
// the repo's [build] target / build-std inheritance. ADR-018 §六's full
// proof ran from a genuinely clean location (outside E:\GOSKernel
// entirely) after this in-tree config juggling still hit a
// `[unstable] build-std` merge-doesn't-override wrinkle -- documented
// there, not re-solved here.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let mut args = env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "build".to_string());
    let kernel_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("kernel/target/x86_64-gos-kernel/debug/toy-kernel"));

    let out_dir = PathBuf::from("target");
    std::fs::create_dir_all(&out_dir).unwrap();
    let uefi_path = out_dir.join("toy-uefi.img");

    println!("[spike] building UEFI disk image from {}", kernel_path.display());
    bootloader::UefiBoot::new(&kernel_path)
        .create_disk_image(&uefi_path)
        .expect("UefiBoot::create_disk_image failed");
    println!("[spike] wrote {}", uefi_path.display());

    if mode == "uefi" || mode == "run" {
        run_qemu_uefi(&uefi_path);
    }
}

/// OVMF_CODE should point at a *writable* copy of the firmware --
/// `-drive if=pflash` opens it read-write by default even with
/// `readonly=on` semantics needing an explicit flag, and a stock
/// install under `Program Files` will deny access. Copy
/// `edk2-x86_64-code.fd` somewhere writable first (ADR-018 §六).
fn run_qemu_uefi(image: &PathBuf) {
    let ovmf = env::var("OVMF_CODE").expect(
        "set OVMF_CODE to a writable copy of edk2-x86_64-code.fd (e.g. QEMU's share/ dir copied locally)",
    );
    let status = Command::new("qemu-system-x86_64")
        .arg("-drive")
        .arg(format!("if=pflash,format=raw,readonly=on,file={ovmf}"))
        .arg("-drive")
        .arg(format!("format=raw,file={}", image.display()))
        .arg("-serial")
        .arg("stdio")
        .arg("-display")
        .arg("none")
        .arg("-no-reboot")
        .status()
        .expect("failed to launch qemu-system-x86_64");
    println!("[spike] qemu (uefi) exited: {status}");
}
