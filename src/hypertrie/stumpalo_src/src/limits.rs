// Maximum address in the userspace virtual address range.
// Uses the largest standard configuration to ensure correctness across all systems.

// Supports both 4-level (48-bit) and 5-level (57-bit) paging.
// Use 57-bit limit (128 PiB) as the safe, universal maximum.
#[cfg(target_arch = "x86_64")]
pub(crate) const MAX_USABLE: usize = (1 << 57) - 1;

// Supports configurations up to 52-bit virtual address space (4 PiB).
#[cfg(target_arch = "aarch64")]
pub(crate) const MAX_USABLE: usize = (1 << 52) - 1;

// Supports standard 48-bit Sv48 and upcoming 57-bit Sv57 modes.
// Use 57-bit limit as the safe, universal maximum.
#[cfg(target_arch = "riscv64")]
pub(crate) const MAX_USABLE: usize = (1 << 57) - 1;

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "riscv64"
)))]
// Fallback for unknown architectures. Will result in an extra conditional branch in the fast path.
// Not a big deal.
pub(crate) const MAX_USABLE: usize = usize::MAX;

pub(crate) const SAFE_SIZE: usize = usize::MAX - MAX_USABLE;
