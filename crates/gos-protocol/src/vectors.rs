use crate::VectorAddress;

/// Centralized Vector Address Table for the GOS Kernel
///
/// This table defines the stable, graph-native topological addresses for all
/// core kernel components and plugins. This prevents accidental collisions between
/// individually developed plugins.
///
/// Format: VectorAddress::new(net, subnet, device, channel)
///
/// Group 1: Core Hardware & Drivers
pub const CORE_PANIC: VectorAddress = VectorAddress::new(1, 0, 0, 0);
pub const CORE_VGA: VectorAddress = VectorAddress::new(1, 1, 0, 0);
pub const CORE_SERIAL: VectorAddress = VectorAddress::new(1, 2, 0, 0);
pub const CORE_GDT: VectorAddress = VectorAddress::new(1, 3, 0, 0);
pub const CORE_IDT: VectorAddress = VectorAddress::new(1, 4, 0, 0);
pub const CORE_PIC: VectorAddress = VectorAddress::new(1, 5, 0, 0);
pub const CORE_PIT: VectorAddress = VectorAddress::new(1, 6, 0, 0);
pub const CORE_PS2: VectorAddress = VectorAddress::new(1, 7, 0, 0);
pub const CORE_CPUID: VectorAddress = VectorAddress::new(1, 8, 0, 0);
pub const CORE_HAL_VADDR: VectorAddress = VectorAddress::new(1, 9, 0, 0);
pub const CORE_HAL_META: VectorAddress = VectorAddress::new(1, 10, 0, 0);
pub const CORE_PMM: VectorAddress = VectorAddress::new(1, 11, 0, 0);
pub const CORE_CTX: VectorAddress = VectorAddress::new(1, 12, 0, 0); // native.core

// Group 2: Advanced Memory Management
pub const MEM_VMM: VectorAddress = VectorAddress::new(2, 2, 0, 0);
pub const MEM_HEAP: VectorAddress = VectorAddress::new(2, 3, 0, 0);

// Group 6: OS Services & Applications
pub const SVC_SHELL: VectorAddress = VectorAddress::new(6, 1, 0, 0);
pub const SVC_SHELL_THEME_WABI: VectorAddress = VectorAddress::new(6, 1, 1, 0);
pub const SVC_SHELL_THEME_SHOJI: VectorAddress = VectorAddress::new(6, 1, 2, 0);
pub const SVC_SHELL_THEME_CURRENT: VectorAddress = VectorAddress::new(6, 1, 3, 0);
pub const SVC_SHELL_CLIPBOARD: VectorAddress = VectorAddress::new(6, 1, 4, 0);

pub const SVC_AI: VectorAddress = VectorAddress::new(6, 2, 0, 0);
pub const SVC_IME: VectorAddress = VectorAddress::new(6, 3, 0, 0);
pub const SVC_NET: VectorAddress = VectorAddress::new(6, 4, 0, 0);
pub const SVC_MOUSE: VectorAddress = VectorAddress::new(6, 5, 0, 0);
pub const SVC_CYPHER: VectorAddress = VectorAddress::new(6, 6, 0, 0);
pub const SVC_CUDA: VectorAddress = VectorAddress::new(6, 7, 0, 0);
pub const SVC_CHAT: VectorAddress = VectorAddress::new(6, 8, 0, 0);
pub const SVC_NIM: VectorAddress  = VectorAddress::new(6, 9, 0, 0);
