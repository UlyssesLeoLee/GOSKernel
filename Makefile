# GOSKernel — convenience targets
# Requires: cargo, bootimage (cargo install bootimage), QEMU

.PHONY: build run check clean serial test test-runtime test-supervisor

## Build the kernel disk image
build:
	cargo bootimage --package gos-kernel

## Boot in QEMU (serial to stdout, VGA window, e1000 NIC on QEMU user-net)
run: build
	cargo bootimage run --package gos-kernel

## Quick compile check (no image creation)
check:
	cargo check

## Connect to the QEMU monitor (must already be running)
monitor:
	telnet 127.0.0.1 55555

## Run host test harnesses (must be invoked from /tmp to avoid build-std inheritance)
test: test-runtime test-supervisor

test-runtime:
	cd /tmp && cargo +nightly test --manifest-path $(CURDIR)/host-tests/gos-runtime-harness/Cargo.toml

test-supervisor:
	cd /tmp && cargo +nightly test --manifest-path $(CURDIR)/host-tests/gos-supervisor-harness/Cargo.toml

clean:
	cargo clean
