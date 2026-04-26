//! Phase B.4.6 — minimal ET_DYN ELF parser.
//!
//! Scope of *this slice*:
//!   * Validate the file is a 64-bit little-endian ET_DYN ELF for x86_64.
//!   * Walk program headers and surface PT_LOAD segments.
//!   * Surface the entry-point offset.
//!
//! Explicitly out of scope (separate slices):
//!   * R_X86_64_RELATIVE relocation processing.
//!   * Dynamic symbol table parsing for `module_init` / `module_event` /
//!     `module_stop` discovery.
//!   * Mapping segments into a domain's image window (uses
//!     `k_vmm::create_isolated_address_space` infrastructure).
//!   * Signature verification (Phase G.2).
//!
//! Once the parser locks in, follow-up slices add each of the above
//! incrementally.  Until then the parser is enough to *reject* malformed
//! payloads at module-install time and is the foundation for an
//! external-plugin pipeline.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    TooSmall,
    BadMagic,
    NotElf64,
    NotLittleEndian,
    NotEtDyn,
    NotX86_64,
    BadProgramHeader,
    UnsupportedAbi,
    /// PT_DYNAMIC table malformed or out-of-bounds.
    BadDynamic,
    /// A relocation entry referenced an unsupported relocation type.
    /// B.4.6.1 only supports R_X86_64_RELATIVE; everything else
    /// surfaces here so the loader can refuse the module instead of
    /// silently leaving relocations unprocessed.
    UnsupportedRelocation(u32),
    /// Relocation entry pointed outside the loaded image base/size.
    RelocationOutOfBounds,
}

const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const ET_DYN: u16 = 3;
const EM_X86_64: u16 = 62;
pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_GNU_RELRO: u32 = 0x6474_E552;
pub const PF_X: u32 = 0x1;
pub const PF_W: u32 = 0x2;
pub const PF_R: u32 = 0x4;

/// One PT_LOAD segment surfaced from the program-header table.  The
/// loader uses these to compute image_len, layout virtual addresses,
/// and decide page protection per segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadSegment {
    pub virt_addr: u64,
    pub mem_len: u64,
    pub file_offset: u64,
    pub file_len: u64,
    pub flags: u32,
}

/// Result of a successful parse.  `entry_offset` is the file-relative
/// offset of the module's entry point — the loader adds `image_base`
/// after relocation to derive the runtime VA.
#[derive(Debug, Clone, Copy)]
pub struct ParsedElf<'a> {
    pub entry_offset: u64,
    pub program_headers: u16,
    pub segments_capacity: usize,
    /// Captures the raw byte slice the parse came from so follow-up
    /// slices (relocation, symbol resolution) can re-walk it without
    /// re-parsing the header.
    pub data: &'a [u8],
}

/// Read a u16 / u32 / u64 in little-endian order from `data` at offset
/// `off`.  Returns None if the slice is too short.
fn read_u16(data: &[u8], off: usize) -> Option<u16> {
    let bytes: [u8; 2] = data.get(off..off + 2)?.try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}
fn read_u32(data: &[u8], off: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(off..off + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}
fn read_u64(data: &[u8], off: usize) -> Option<u64> {
    let bytes: [u8; 8] = data.get(off..off + 8)?.try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

/// Parse and validate an ELF64 ET_DYN x86_64 image.  Returns an error
/// if any of the structural invariants fails — by the time `Ok` is
/// returned, the caller may treat the slice as a known-good ET_DYN.
pub fn parse(data: &[u8]) -> Result<ParsedElf<'_>, ElfError> {
    // ── e_ident ──────────────────────────────────────────────────────────
    if data.len() < 64 {
        return Err(ElfError::TooSmall);
    }
    if &data[..4] != &ELF_MAGIC {
        return Err(ElfError::BadMagic);
    }
    if data[4] != ELFCLASS64 {
        return Err(ElfError::NotElf64);
    }
    if data[5] != ELFDATA2LSB {
        return Err(ElfError::NotLittleEndian);
    }
    // EI_OSABI at offset 7 — accept SYSV (0) or GNU (3) for now.
    if data[7] != 0 && data[7] != 3 {
        return Err(ElfError::UnsupportedAbi);
    }

    // ── e_type / e_machine ──────────────────────────────────────────────
    let e_type = read_u16(data, 16).ok_or(ElfError::TooSmall)?;
    if e_type != ET_DYN {
        return Err(ElfError::NotEtDyn);
    }
    let e_machine = read_u16(data, 18).ok_or(ElfError::TooSmall)?;
    if e_machine != EM_X86_64 {
        return Err(ElfError::NotX86_64);
    }

    // ── e_entry / e_phoff / e_phentsize / e_phnum ───────────────────────
    let entry_offset = read_u64(data, 24).ok_or(ElfError::TooSmall)?;
    let phoff = read_u64(data, 32).ok_or(ElfError::TooSmall)?;
    let phentsize = read_u16(data, 54).ok_or(ElfError::TooSmall)?;
    let phnum = read_u16(data, 56).ok_or(ElfError::TooSmall)?;
    if phentsize as usize != 56 {
        return Err(ElfError::BadProgramHeader);
    }
    let ph_table_end = (phoff as usize)
        .checked_add(phentsize as usize * phnum as usize)
        .ok_or(ElfError::BadProgramHeader)?;
    if ph_table_end > data.len() {
        return Err(ElfError::BadProgramHeader);
    }

    Ok(ParsedElf {
        entry_offset,
        program_headers: phnum,
        segments_capacity: phnum as usize,
        data,
    })
}

impl<'a> ParsedElf<'a> {
    /// Iterate PT_LOAD segments only, calling `f` for each.  Returns the
    /// total count (so callers can detect "no PT_LOAD found").
    pub fn for_each_load_segment<F>(&self, mut f: F) -> usize
    where
        F: FnMut(LoadSegment),
    {
        let phoff = match read_u64(self.data, 32) {
            Some(v) => v as usize,
            None => return 0,
        };
        let phentsize = 56usize;
        let mut count = 0usize;
        for idx in 0..self.program_headers as usize {
            let off = phoff + idx * phentsize;
            let p_type = match read_u32(self.data, off) {
                Some(v) => v,
                None => continue,
            };
            if p_type != PT_LOAD {
                continue;
            }
            let p_flags = read_u32(self.data, off + 4).unwrap_or(0);
            let p_offset = read_u64(self.data, off + 8).unwrap_or(0);
            let p_vaddr = read_u64(self.data, off + 16).unwrap_or(0);
            let p_filesz = read_u64(self.data, off + 32).unwrap_or(0);
            let p_memsz = read_u64(self.data, off + 40).unwrap_or(0);
            f(LoadSegment {
                virt_addr: p_vaddr,
                mem_len: p_memsz,
                file_offset: p_offset,
                file_len: p_filesz,
                flags: p_flags,
            });
            count += 1;
        }
        count
    }

    /// Compute the highest virtual address touched by any PT_LOAD
    /// segment.  Used by the supervisor to size the domain's image
    /// window before mapping.
    pub fn highest_virt_end(&self) -> u64 {
        let mut high = 0u64;
        self.for_each_load_segment(|seg| {
            let end = seg.virt_addr.saturating_add(seg.mem_len);
            if end > high {
                high = end;
            }
        });
        high
    }

    /// Locate the PT_DYNAMIC segment if present, returning its
    /// (file_offset, file_len).  None when the image has no dynamic
    /// section (statically-linked ET_DYN or hand-crafted test fixture).
    pub fn dynamic_segment(&self) -> Option<(u64, u64)> {
        let phoff = read_u64(self.data, 32)? as usize;
        let phentsize = 56usize;
        for idx in 0..self.program_headers as usize {
            let off = phoff + idx * phentsize;
            let p_type = read_u32(self.data, off)?;
            if p_type == PT_DYNAMIC {
                let p_offset = read_u64(self.data, off + 8)?;
                let p_filesz = read_u64(self.data, off + 32)?;
                return Some((p_offset, p_filesz));
            }
        }
        None
    }
}

// ── Phase B.4.6.1 — R_X86_64_RELATIVE relocation ────────────────────────────
//
// PT_DYNAMIC carries `Elf64_Dyn` entries (16 bytes each: u64 d_tag,
// u64 d_val_or_ptr).  We care about three tags:
//
//   DT_RELA       (7)   = file offset of the .rela.dyn table
//   DT_RELASZ     (8)   = total bytes of the table
//   DT_RELAENT    (9)   = bytes per entry (must be 24)
//
// Each Elf64_Rela entry is `{ u64 r_offset; u64 r_info; i64 r_addend }`.
// Type is encoded in the low 32 bits of `r_info`.  R_X86_64_RELATIVE
// (type 8) does:  *(image_base + r_offset) = image_base + r_addend
//
// This is the only relocation type emitted by a position-independent
// ET_DYN compiled with `-fPIE -fno-plt -fno-pic` and no external
// symbol references — exactly what GOS plugins are constrained to.

pub const DT_NULL: u64 = 0;
pub const DT_RELA: u64 = 7;
pub const DT_RELASZ: u64 = 8;
pub const DT_RELAENT: u64 = 9;

pub const R_X86_64_RELATIVE: u32 = 8;

pub const ELF64_RELA_SIZE: usize = 24;

#[derive(Debug, Clone, Copy)]
pub struct DynamicTable {
    pub rela_offset: Option<u64>,
    pub rela_size: u64,
    pub rela_entry_size: u64,
}

impl DynamicTable {
    pub const fn empty() -> Self {
        Self {
            rela_offset: None,
            rela_size: 0,
            rela_entry_size: ELF64_RELA_SIZE as u64,
        }
    }
}

impl<'a> ParsedElf<'a> {
    /// Walk PT_DYNAMIC and collect the RELA-table descriptors.
    /// Returns `Ok(DynamicTable::empty())` for images without a
    /// dynamic segment (no relocations needed).
    pub fn parse_dynamic(&self) -> Result<DynamicTable, ElfError> {
        let mut table = DynamicTable::empty();
        let Some((dyn_off, dyn_len)) = self.dynamic_segment() else {
            return Ok(table);
        };
        let dyn_off = dyn_off as usize;
        let dyn_len = dyn_len as usize;
        if dyn_off
            .checked_add(dyn_len)
            .ok_or(ElfError::BadDynamic)?
            > self.data.len()
        {
            return Err(ElfError::BadDynamic);
        }
        let mut cur = 0usize;
        while cur + 16 <= dyn_len {
            let tag = read_u64(self.data, dyn_off + cur).ok_or(ElfError::BadDynamic)?;
            let val = read_u64(self.data, dyn_off + cur + 8).ok_or(ElfError::BadDynamic)?;
            match tag {
                DT_NULL => break,
                DT_RELA => table.rela_offset = Some(val),
                DT_RELASZ => table.rela_size = val,
                DT_RELAENT => table.rela_entry_size = val,
                _ => {} // ignored — we handle only RELATIVE for now
            }
            cur += 16;
        }
        if let Some(_) = table.rela_offset {
            if table.rela_entry_size as usize != ELF64_RELA_SIZE {
                return Err(ElfError::BadDynamic);
            }
        }
        Ok(table)
    }

    /// Apply R_X86_64_RELATIVE relocations against an in-memory image.
    ///
    /// `image` is the *loaded* image — typically a slice into the
    /// domain's mapped image window after PT_LOAD segments have been
    /// copied.  `image_base` is the virtual address that slice's
    /// byte 0 corresponds to (so relocations targeting absolute VAs
    /// resolve correctly when image_base is non-zero).
    ///
    /// Returns the number of relocations applied.  Errors out on the
    /// first non-R_X86_64_RELATIVE entry — by design, plugins must be
    /// built with no relocations our loader doesn't understand.
    pub fn apply_relative_relocations(
        &self,
        image: &mut [u8],
        image_base: u64,
    ) -> Result<usize, ElfError> {
        let table = self.parse_dynamic()?;
        let Some(rela_offset) = table.rela_offset else {
            return Ok(0);
        };
        let rela_offset = rela_offset as usize;
        let rela_end = rela_offset
            .checked_add(table.rela_size as usize)
            .ok_or(ElfError::BadDynamic)?;
        if rela_end > self.data.len() {
            return Err(ElfError::BadDynamic);
        }
        let mut applied = 0usize;
        let mut cur = rela_offset;
        while cur + ELF64_RELA_SIZE <= rela_end {
            let r_offset = read_u64(self.data, cur).ok_or(ElfError::BadDynamic)?;
            let r_info = read_u64(self.data, cur + 8).ok_or(ElfError::BadDynamic)?;
            let r_addend = read_u64(self.data, cur + 16).ok_or(ElfError::BadDynamic)? as i64;
            let r_type = (r_info & 0xFFFF_FFFF) as u32;
            cur += ELF64_RELA_SIZE;

            if r_type != R_X86_64_RELATIVE {
                return Err(ElfError::UnsupportedRelocation(r_type));
            }

            // Target offset in image (relative to image_base).
            let target = r_offset as usize;
            if target.checked_add(8).map(|end| end > image.len()).unwrap_or(true) {
                return Err(ElfError::RelocationOutOfBounds);
            }
            let resolved = image_base.wrapping_add(r_addend as u64);
            image[target..target + 8].copy_from_slice(&resolved.to_le_bytes());
            applied += 1;
        }
        Ok(applied)
    }
}
