#![no_std]

//! Phase F.3.1 — minimal FAT32 reader.
//!
//! Implements `gos_vfs::FileSystem` against any `BlockDeviceVTable`.
//! Capabilities of *this slice*:
//!
//!   * Parse + validate the FAT32 BPB at sector 0.
//!   * Read the root directory (8.3 short-name entries only; LFN
//!     entries are skipped).
//!   * Read a file's contents up to one cluster (no FAT chain walk).
//!
//! Explicitly out of scope (separate F.3.2..x slices):
//!   * FAT chain walking for files > one cluster.
//!   * Long File Name (LFN) parsing.
//!   * Subdirectory traversal beyond root.
//!   * Writing, formatting, fsync.
//!
//! The trait surface comes from `gos_vfs::FileSystem`, so once F.3.2
//! lifts the single-cluster restriction the upper layers (shell `ls`,
//! shell `cat`, future graph-state journal in F.4) keep working
//! without re-shaping the API.

use gos_protocol::block::{BlockDeviceVTable, BlockIoStatus, BLOCK_SECTOR_SIZE_DEFAULT};
use gos_vfs::{DirEntry, FileSystem, Inode, InodeKind, InodeNum, MountId, VfsError};

/// 8.3 directory entry size — fixed by the FAT spec.
pub const FAT_DIR_ENTRY_SIZE: usize = 32;
/// Sentinel attribute value identifying an LFN slot; we skip these.
const ATTR_LFN: u8 = 0x0F;
const ATTR_DIRECTORY: u8 = 0x10;
const ATTR_VOLUME_ID: u8 = 0x08;

/// Subset of the BPB we actually use.  Validation rejects anything
/// outside the assumptions our reader makes (sector size 512, no
/// FAT12/FAT16 layouts, non-zero root cluster).
#[derive(Debug, Clone, Copy)]
pub struct Bpb {
    pub bytes_per_sector: u32,
    pub sectors_per_cluster: u32,
    pub reserved_sectors: u32,
    pub num_fats: u32,
    pub sectors_per_fat: u32,
    pub root_cluster: u32,
    pub total_sectors: u32,
    /// Sector index where the data region (cluster 2) begins.
    pub data_start_sector: u32,
}

impl Bpb {
    pub fn parse(sector0: &[u8]) -> Result<Self, VfsError> {
        if sector0.len() < 512 {
            return Err(VfsError::Io(BlockIoStatus::BadBuffer));
        }
        // 0x1FE / 0x1FF is the boot signature 0x55 / 0xAA.
        if sector0[0x1FE] != 0x55 || sector0[0x1FF] != 0xAA {
            return Err(VfsError::NotImplemented);
        }
        let bytes_per_sector = u16::from_le_bytes([sector0[0x0B], sector0[0x0C]]) as u32;
        let sectors_per_cluster = sector0[0x0D] as u32;
        let reserved_sectors = u16::from_le_bytes([sector0[0x0E], sector0[0x0F]]) as u32;
        let num_fats = sector0[0x10] as u32;
        // BPB_FATSz16 (0x16..0x18) must be 0 on FAT32; real value lives
        // at BPB_FATSz32 (0x24..0x28).
        let fatsz16 = u16::from_le_bytes([sector0[0x16], sector0[0x17]]) as u32;
        let sectors_per_fat = u32::from_le_bytes([
            sector0[0x24],
            sector0[0x25],
            sector0[0x26],
            sector0[0x27],
        ]);
        let root_cluster = u32::from_le_bytes([
            sector0[0x2C],
            sector0[0x2D],
            sector0[0x2E],
            sector0[0x2F],
        ]);
        let totsec16 = u16::from_le_bytes([sector0[0x13], sector0[0x14]]) as u32;
        let totsec32 = u32::from_le_bytes([
            sector0[0x20],
            sector0[0x21],
            sector0[0x22],
            sector0[0x23],
        ]);
        let total_sectors = if totsec16 != 0 { totsec16 } else { totsec32 };

        // Strict acceptance criteria for this slice:
        if bytes_per_sector != 512 {
            return Err(VfsError::NotImplemented);
        }
        if sectors_per_cluster == 0 || !sectors_per_cluster.is_power_of_two() {
            return Err(VfsError::NotImplemented);
        }
        if num_fats == 0 || sectors_per_fat == 0 {
            return Err(VfsError::NotImplemented);
        }
        if fatsz16 != 0 {
            // FAT12 / FAT16 — refuse.
            return Err(VfsError::NotImplemented);
        }
        if root_cluster < 2 {
            return Err(VfsError::NotImplemented);
        }

        let data_start_sector = reserved_sectors + num_fats * sectors_per_fat;

        Ok(Self {
            bytes_per_sector,
            sectors_per_cluster,
            reserved_sectors,
            num_fats,
            sectors_per_fat,
            root_cluster,
            total_sectors,
            data_start_sector,
        })
    }

    pub fn cluster_to_sector(&self, cluster: u32) -> u32 {
        self.data_start_sector + (cluster - 2) * self.sectors_per_cluster
    }

    pub fn cluster_size_bytes(&self) -> u32 {
        self.bytes_per_sector * self.sectors_per_cluster
    }
}

pub struct Fat32 {
    mount: MountId,
    block: BlockDeviceVTable,
    bpb: Bpb,
}

impl Fat32 {
    /// Mount a FAT32 volume on top of a block device.  Reads sector 0
    /// to parse the BPB; everything else is lazy.
    pub fn mount(mount: MountId, block: BlockDeviceVTable) -> Result<Self, VfsError> {
        let mut sector0 = [0u8; BLOCK_SECTOR_SIZE_DEFAULT as usize];
        let status = unsafe {
            (block.read_sector)(
                block.handle,
                0,
                sector0.as_mut_ptr(),
                BLOCK_SECTOR_SIZE_DEFAULT,
            )
        };
        if status != BlockIoStatus::Ok as i32 {
            return Err(VfsError::Io(BlockIoStatus::from_i32(status)));
        }
        let bpb = Bpb::parse(&sector0)?;
        Ok(Self { mount, block, bpb })
    }

    pub fn bpb(&self) -> &Bpb {
        &self.bpb
    }

    /// Read one sector through the underlying block device.
    fn read_sector(&self, lba: u32, buf: &mut [u8]) -> Result<(), VfsError> {
        if buf.len() != BLOCK_SECTOR_SIZE_DEFAULT as usize {
            return Err(VfsError::Io(BlockIoStatus::BadBuffer));
        }
        let status = unsafe {
            (self.block.read_sector)(
                self.block.handle,
                lba as u64,
                buf.as_mut_ptr(),
                BLOCK_SECTOR_SIZE_DEFAULT,
            )
        };
        if status != BlockIoStatus::Ok as i32 {
            Err(VfsError::Io(BlockIoStatus::from_i32(status)))
        } else {
            Ok(())
        }
    }
}

fn copy_8_3_name(raw: &[u8], dst: &mut [u8; 64]) -> u8 {
    // raw[0..8] is name (space-padded), raw[8..11] is extension.
    let mut len = 0u8;
    for i in 0..8 {
        if raw[i] != b' ' {
            dst[len as usize] = raw[i];
            len += 1;
        }
    }
    let has_ext = raw[8] != b' ';
    if has_ext {
        dst[len as usize] = b'.';
        len += 1;
        for i in 8..11 {
            if raw[i] != b' ' {
                dst[len as usize] = raw[i];
                len += 1;
            }
        }
    }
    len
}

impl FileSystem for Fat32 {
    fn mount_id(&self) -> MountId {
        self.mount
    }

    fn root(&self) -> Inode {
        Inode {
            mount: self.mount,
            num: InodeNum(self.bpb.root_cluster as u64),
            kind: InodeKind::Directory,
            size_bytes: 0,
        }
    }

    fn lookup(&self, parent: Inode, name: &[u8]) -> Result<Inode, VfsError> {
        if parent.kind != InodeKind::Directory {
            return Err(VfsError::NotADirectory);
        }
        // F.3.1 limitation: only the first cluster of the directory is
        // walked — fine for tiny root dirs (the FAT32 spec puts up to
        // 16 entries per 512-byte sector; 1-cluster root = thousands
        // of entries on default formats).
        let cluster = parent.num.0 as u32;
        let first_sector = self.bpb.cluster_to_sector(cluster);
        let mut sector = [0u8; BLOCK_SECTOR_SIZE_DEFAULT as usize];
        for s in 0..self.bpb.sectors_per_cluster {
            self.read_sector(first_sector + s, &mut sector)?;
            let entries = sector.len() / FAT_DIR_ENTRY_SIZE;
            for e in 0..entries {
                let off = e * FAT_DIR_ENTRY_SIZE;
                let entry = &sector[off..off + FAT_DIR_ENTRY_SIZE];
                if entry[0] == 0x00 {
                    return Err(VfsError::NotFound); // end-of-directory
                }
                if entry[0] == 0xE5 || entry[11] == ATTR_LFN || entry[11] & ATTR_VOLUME_ID != 0 {
                    continue;
                }
                let mut name_buf = [0u8; 64];
                let n = copy_8_3_name(&entry[..11], &mut name_buf);
                if &name_buf[..n as usize] == name {
                    let cluster_lo = u16::from_le_bytes([entry[26], entry[27]]) as u32;
                    let cluster_hi = u16::from_le_bytes([entry[20], entry[21]]) as u32;
                    let cluster = (cluster_hi << 16) | cluster_lo;
                    let size = u32::from_le_bytes([
                        entry[28], entry[29], entry[30], entry[31],
                    ]) as u64;
                    let kind = if entry[11] & ATTR_DIRECTORY != 0 {
                        InodeKind::Directory
                    } else {
                        InodeKind::File
                    };
                    return Ok(Inode {
                        mount: self.mount,
                        num: InodeNum(cluster as u64),
                        kind,
                        size_bytes: size,
                    });
                }
            }
        }
        Err(VfsError::NotFound)
    }

    fn read(&self, inode: Inode, offset: u64, out: &mut [u8]) -> Result<usize, VfsError> {
        if inode.kind != InodeKind::File {
            return Err(VfsError::NotAFile);
        }
        if offset >= inode.size_bytes {
            return Ok(0);
        }
        let cluster = inode.num.0 as u32;
        let first_sector = self.bpb.cluster_to_sector(cluster);
        let cluster_bytes = self.bpb.cluster_size_bytes() as u64;
        // F.3.1 limitation: single cluster only.  F.3.2 extends to FAT
        // chain walking.
        let max = inode.size_bytes.min(cluster_bytes) as usize;
        if offset as usize >= max {
            return Ok(0);
        }
        let want = out.len().min(max - offset as usize);
        let mut sector = [0u8; BLOCK_SECTOR_SIZE_DEFAULT as usize];
        let mut written = 0usize;
        let mut cur_off = offset as usize;
        while written < want {
            let sector_idx = cur_off / BLOCK_SECTOR_SIZE_DEFAULT as usize;
            let in_sector_off = cur_off % BLOCK_SECTOR_SIZE_DEFAULT as usize;
            self.read_sector(first_sector + sector_idx as u32, &mut sector)?;
            let chunk = (BLOCK_SECTOR_SIZE_DEFAULT as usize - in_sector_off)
                .min(want - written);
            out[written..written + chunk]
                .copy_from_slice(&sector[in_sector_off..in_sector_off + chunk]);
            written += chunk;
            cur_off += chunk;
        }
        Ok(written)
    }

    fn read_dir(
        &self,
        dir: Inode,
        cursor: u64,
        entries: &mut [DirEntry],
    ) -> Result<(usize, u64), VfsError> {
        if dir.kind != InodeKind::Directory {
            return Err(VfsError::NotADirectory);
        }
        let cluster = dir.num.0 as u32;
        let first_sector = self.bpb.cluster_to_sector(cluster);
        let cluster_entries =
            (self.bpb.cluster_size_bytes() as usize) / FAT_DIR_ENTRY_SIZE;
        let mut written = 0usize;
        let mut sector = [0u8; BLOCK_SECTOR_SIZE_DEFAULT as usize];
        let mut idx = cursor as usize;
        while idx < cluster_entries && written < entries.len() {
            let sector_idx = idx / (BLOCK_SECTOR_SIZE_DEFAULT as usize / FAT_DIR_ENTRY_SIZE);
            let in_sector =
                idx % (BLOCK_SECTOR_SIZE_DEFAULT as usize / FAT_DIR_ENTRY_SIZE);
            self.read_sector(first_sector + sector_idx as u32, &mut sector)?;
            let off = in_sector * FAT_DIR_ENTRY_SIZE;
            let entry = &sector[off..off + FAT_DIR_ENTRY_SIZE];
            if entry[0] == 0x00 {
                return Ok((written, u64::MAX));
            }
            idx += 1;
            if entry[0] == 0xE5 || entry[11] == ATTR_LFN || entry[11] & ATTR_VOLUME_ID != 0 {
                continue;
            }
            let mut de = DirEntry::empty();
            let n = copy_8_3_name(&entry[..11], &mut de.name);
            de.name_len = n;
            let cluster_lo = u16::from_le_bytes([entry[26], entry[27]]) as u32;
            let cluster_hi = u16::from_le_bytes([entry[20], entry[21]]) as u32;
            let inode_cluster = (cluster_hi << 16) | cluster_lo;
            let size = u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]) as u64;
            de.inode = Inode {
                mount: self.mount,
                num: InodeNum(inode_cluster as u64),
                kind: if entry[11] & ATTR_DIRECTORY != 0 {
                    InodeKind::Directory
                } else {
                    InodeKind::File
                },
                size_bytes: size,
            };
            entries[written] = de;
            written += 1;
        }
        let next = if idx >= cluster_entries {
            u64::MAX
        } else {
            idx as u64
        };
        Ok((written, next))
    }
}
