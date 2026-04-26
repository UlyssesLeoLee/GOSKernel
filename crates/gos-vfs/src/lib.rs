#![no_std]

//! Phase F.2 — minimal VFS trait surface.
//!
//! This crate defines the *types* every filesystem implementation in
//! GOS will speak (Inode, DirEntry, FileSystem trait), and the
//! resource-handle book-keeping the supervisor needs to track open
//! files per instance.  No actual filesystem ships here — FAT32 read,
//! FAT32 write, and the graph-state journal are independent F.3 / F.4
//! / F.5 slices that build on this.
//!
//! Why bother before the first FS lands?  Because manifests now want
//! to declare `RESOURCE_FILE_HANDLE` claims, and the supervisor needs
//! a stable shape for those claims long before the actual FS reads a
//! single byte.

use gos_protocol::block::{BlockDeviceVTable, BlockIoStatus};

/// Filesystem-wide identifier (for an `Inode` to be unique we need
/// `(MountId, InodeNum)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MountId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct InodeNum(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InodeKind {
    File = 1,
    Directory = 2,
    /// Reserved for symlinks / special files in future slices.
    Other = 0xFF,
}

#[derive(Debug, Clone, Copy)]
pub struct Inode {
    pub mount: MountId,
    pub num: InodeNum,
    pub kind: InodeKind,
    pub size_bytes: u64,
}

/// One entry in a directory listing.  Names are bounded to 64 bytes —
/// matches FAT32's LFN limit for the read path; longer-name FSes can
/// either truncate or expose multi-segment APIs in future slices.
#[derive(Clone, Copy)]
pub struct DirEntry {
    pub inode: Inode,
    pub name_len: u8,
    pub name: [u8; 64],
}

impl DirEntry {
    pub const fn empty() -> Self {
        Self {
            inode: Inode {
                mount: MountId(0),
                num: InodeNum(0),
                kind: InodeKind::Other,
                size_bytes: 0,
            },
            name_len: 0,
            name: [0; 64],
        }
    }

    pub fn name(&self) -> &[u8] {
        &self.name[..self.name_len as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsError {
    NotFound,
    NotADirectory,
    NotAFile,
    PermissionDenied,
    Io(BlockIoStatus),
    /// The filesystem implementation is still in stub form — see the
    /// Phase F roadmap in `plan/OPTIMIZATION_PLAN.md`.
    NotImplemented,
}

/// Mounted-filesystem trait.  Every concrete FS (FAT32 read in F.3,
/// FAT32 write in F.5, graph journal in F.4, in-memory test FS in
/// host harnesses) implements this interface.
pub trait FileSystem {
    fn mount_id(&self) -> MountId;
    fn root(&self) -> Inode;

    /// Resolve a single path component starting from `parent`.
    fn lookup(&self, parent: Inode, name: &[u8]) -> Result<Inode, VfsError>;

    /// Read up to `out.len()` bytes starting at `offset`.  Returns
    /// the number of bytes actually written into `out`.
    fn read(
        &self,
        inode: Inode,
        offset: u64,
        out: &mut [u8],
    ) -> Result<usize, VfsError>;

    /// Iterate directory entries starting at `cursor`; writes up to
    /// `entries.len()` entries and returns `(written, next_cursor)`.
    /// `next_cursor == u64::MAX` indicates end-of-directory.
    fn read_dir(
        &self,
        dir: Inode,
        cursor: u64,
        entries: &mut [DirEntry],
    ) -> Result<(usize, u64), VfsError>;
}

/// A mount provider needs a block device under it (for FAT32) or
/// nothing at all (for in-memory FSes).  This struct is what the
/// runtime hands to each concrete FS during mount.
pub struct MountSource {
    pub block: Option<BlockDeviceVTable>,
}

impl MountSource {
    pub const fn empty() -> Self {
        Self { block: None }
    }

    pub const fn from_block(vtable: BlockDeviceVTable) -> Self {
        Self {
            block: Some(vtable),
        }
    }
}
