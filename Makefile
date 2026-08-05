# GOSKernel — convenience targets
# Requires: cargo, QEMU

.PHONY: build run check clean serial test test-runtime test-supervisor

## Build a UEFI disk image (ADR-018: bootloader_api 0.11.9, UEFI-only —
## see xtask/src/main.rs's build_uefi_image and doc/ADR-018-bootloader-uefi-migration.md)
build:
	cd xtask && cargo run -- image

## Boot in QEMU+OVMF (serial to stdout, e1000 NIC on QEMU user-net, COM2/
## COM3 as TCP servers for gos-vk-viewer, QEMU monitor on 45555). Set
## OVMF_CODE to a writable copy of edk2-x86_64-code.fd if not using the
## default `C:/Program Files/qemu/share/edk2-x86_64-code.fd` (stock
## installs deny -drive if=pflash write access there — xtask copies it
## into the target dir automatically).
run:
	cd xtask && cargo run -- run

## Quick compile check (no image creation)
check:
	cargo check

## Connect to the QEMU monitor (must already be running via `make run`)
monitor:
	telnet 127.0.0.1 45555

## Run host test harnesses (must be invoked from /tmp to avoid build-std inheritance)
test: test-runtime test-supervisor

test-runtime:
	cd /tmp && cargo +nightly test --manifest-path $(CURDIR)/host-tests/gos-runtime-harness/Cargo.toml

test-supervisor:
	cd /tmp && cargo +nightly test --manifest-path $(CURDIR)/host-tests/gos-supervisor-harness/Cargo.toml

clean:
	cargo clean
