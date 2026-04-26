//! Phase B.4.6.3 — `.gosmod` load pipeline.
//!
//! Reads an ET_DYN ELF from the VFS, validates the header, walks
//! PT_LOAD segments into a caller-provided image buffer, applies
//! R_X86_64_RELATIVE relocations, and resolves the
//! `module_init / module_event / module_stop` entry-point offsets via
//! the dynamic symbol table.
//!
//! The output is a `LoadedModule` describing where each segment
//! should land + the resolved entry offsets, ready to be paired with
//! a runtime `ModuleDescriptor`.  We deliberately don't *call* into
//! `gos_supervisor::install_module` from here — that's the kernel's
//! responsibility once it has wired domain-window allocation and the
//! `module_init` C-ABI thunk.

use gos_vfs::{FileSystem, Inode, VfsError};

use crate::elf::{parse as parse_elf, ElfError, LoadSegment};

#[derive(Debug, Clone, Copy)]
pub enum LoadError {
    /// Underlying VFS read failed.
    Vfs(VfsError),
    /// ELF parser rejected the image.
    Elf(ElfError),
    /// Caller's image buffer is too small for the loaded layout.
    ImageBufferTooSmall {
        required: u64,
    },
    /// Caller asked for at most `max_segments` PT_LOAD segments but
    /// the file contained more.
    TooManySegments,
    /// File-side segment range extended past the end of the source
    /// blob — corruption.
    SegmentOutOfFile,
}

impl From<VfsError> for LoadError {
    fn from(e: VfsError) -> Self {
        Self::Vfs(e)
    }
}

impl From<ElfError> for LoadError {
    fn from(e: ElfError) -> Self {
        Self::Elf(e)
    }
}

/// Static cap on the number of PT_LOAD segments we record per module.
/// 8 is comfortable: typical no_std plugins emit text + rodata + data
/// + bss + maybe a tdata.
pub const MAX_LOAD_SEGMENTS: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct LoadedModule {
    /// Offset of `module_init` from the loaded image base, or `None`
    /// if the module didn't export one (legal for purely passive
    /// modules; the supervisor decides whether that's acceptable).
    pub module_init_offset: Option<u64>,
    pub module_event_offset: Option<u64>,
    pub module_stop_offset: Option<u64>,
    /// Number of relocations applied by `apply_relative_relocations`.
    pub relocations_applied: usize,
    /// Highest PT_LOAD virt-end — sized so the supervisor knows how
    /// big a domain image window must be.
    pub image_size: u64,
    /// PT_LOAD segments populated; only the first `segment_count`
    /// entries are valid.
    pub segments: [LoadSegment; MAX_LOAD_SEGMENTS],
    pub segment_count: usize,
}

impl LoadedModule {
    pub fn segments(&self) -> &[LoadSegment] {
        &self.segments[..self.segment_count]
    }
}

/// Read an ELF blob from `fs` at `inode` into `source` and return the
/// number of bytes read.  Returns an error if the file is larger than
/// `source.len()`.
pub fn read_module_blob<F: FileSystem>(
    fs: &F,
    inode: Inode,
    source: &mut [u8],
) -> Result<usize, LoadError> {
    if inode.size_bytes as usize > source.len() {
        return Err(LoadError::ImageBufferTooSmall {
            required: inode.size_bytes,
        });
    }
    let n = fs.read(inode, 0, source)?;
    Ok(n)
}

/// Full pipeline: parse + lay out + relocate + resolve entry points.
///
/// `source` is the raw ELF file bytes (already read from disk).
/// `image` is a writable buffer that will hold the *loaded* image
/// (size >= `parsed.highest_virt_end()`).  `image_base` is the VA
/// `image.as_ptr()` will live at after the supervisor maps it into
/// the domain.
pub fn load_etdyn(
    source: &[u8],
    image: &mut [u8],
    image_base: u64,
) -> Result<LoadedModule, LoadError> {
    let parsed = parse_elf(source).map_err(LoadError::Elf)?;
    let image_size = parsed.highest_virt_end();
    if (image.len() as u64) < image_size {
        return Err(LoadError::ImageBufferTooSmall {
            required: image_size,
        });
    }

    // Zero the image first — covers .bss (PT_LOAD with mem_len > file_len).
    for byte in image.iter_mut().take(image_size as usize) {
        *byte = 0;
    }

    // Copy each PT_LOAD's file-bytes into the image at p_vaddr.
    let mut segments = [LoadSegment {
        virt_addr: 0,
        mem_len: 0,
        file_offset: 0,
        file_len: 0,
        flags: 0,
    }; MAX_LOAD_SEGMENTS];
    let mut count: usize = 0;
    let mut overflow = false;
    let mut bad_file = false;
    parsed.for_each_load_segment(|seg| {
        if count >= MAX_LOAD_SEGMENTS {
            overflow = true;
            return;
        }
        let from = seg.file_offset as usize;
        let to_take = seg.file_len as usize;
        let dest_start = seg.virt_addr as usize;
        let dest_end = dest_start.saturating_add(to_take);
        if from.saturating_add(to_take) > source.len() || dest_end > image.len() {
            bad_file = true;
            return;
        }
        if to_take > 0 {
            image[dest_start..dest_end].copy_from_slice(&source[from..from + to_take]);
        }
        segments[count] = seg;
        count += 1;
    });
    if overflow {
        return Err(LoadError::TooManySegments);
    }
    if bad_file {
        return Err(LoadError::SegmentOutOfFile);
    }

    // Apply R_X86_64_RELATIVE.
    let relocations_applied = parsed.apply_relative_relocations(image, image_base)?;

    // Resolve entry points via .dynsym.
    let module_init_offset = parsed.lookup_dynamic_symbol(b"module_init")?;
    let module_event_offset = parsed.lookup_dynamic_symbol(b"module_event")?;
    let module_stop_offset = parsed.lookup_dynamic_symbol(b"module_stop")?;

    Ok(LoadedModule {
        module_init_offset,
        module_event_offset,
        module_stop_offset,
        relocations_applied,
        image_size,
        segments,
        segment_count: count,
    })
}
