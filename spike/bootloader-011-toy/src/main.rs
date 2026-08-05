// ADR-018 spike -- avoids `-Z bindeps` artifact-dependencies entirely
// (rust-lang/cargo#10444 / #10647: panics "no entry found for key" when
// an artifact dependency specifies a non-default target, still open on
// this project's pinned nightly-2026-04-02). Instead: the kernel is
// built as a completely separate `cargo build` invocation (see
// spike/bootloader-011-toy/build-and-run.ps1), and this tool only knows
// the resulting binary's path -- the same "two independent build steps"
// shape as the current bootimage-based pipeline, just swapping the
// image-building half for `bootloader::BiosBoot`/`UefiBoot`.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let mut args = env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "build".to_string());
    let kernel_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("kernel/target/x86_64-unknown-none/debug/toy-kernel"));

    let out_dir = PathBuf::from("target");
    std::fs::create_dir_all(&out_dir).unwrap();
    let bios_path = out_dir.join("toy-bios.img");
    let uefi_path = out_dir.join("toy-uefi.img");

    if mode == "build" || mode == "bios" || mode == "uefi" {
        println!("[spike] building disk images from {}", kernel_path.display());
        bootloader::BiosBoot::new(&kernel_path)
            .create_disk_image(&bios_path)
            .expect("BiosBoot::create_disk_image failed");
        println!("[spike] wrote {}", bios_path.display());

        bootloader::UefiBoot::new(&kernel_path)
            .create_disk_image(&uefi_path)
            .expect("UefiBoot::create_disk_image failed");
        println!("[spike] wrote {}", uefi_path.display());
    }

    if mode == "bios" {
        run_qemu_bios(&bios_path);
    } else if mode == "uefi" {
        run_qemu_uefi(&uefi_path);
    }
}

fn run_qemu_bios(image: &PathBuf) {
    let status = Command::new("qemu-system-x86_64")
        .arg("-drive")
        .arg(format!("format=raw,file={}", image.display()))
        .arg("-serial")
        .arg("stdio")
        .arg("-display")
        .arg("none")
        .arg("-no-reboot")
        .status()
        .expect("failed to launch qemu-system-x86_64");
    println!("[spike] qemu (bios) exited: {status}");
}

fn run_qemu_uefi(image: &PathBuf) {
    let ovmf = env::var("OVMF_CODE")
        .unwrap_or_else(|_| "C:/Program Files/qemu/share/edk2-x86_64-code.fd".to_string());
    let status = Command::new("qemu-system-x86_64")
        .arg("-bios")
        .arg(ovmf)
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
