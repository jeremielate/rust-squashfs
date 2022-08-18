use bitflags::bitflags;

use crate::utils::get_set_field;
use crate::{INVALID_BLK, MAGIC, SUPERBLOCK_SIZE};
use std::fmt::{Debug, Display};
use std::io::{Error, ErrorKind, Read, Result};
use std::{mem, slice};

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Superblock {
    magic: [u8; 4],
    inodes: [u8; 4],
    mkfs_time: [u8; 4],
    block_size: [u8; 4],
    fragments: [u8; 4],

    compressor: [u8; 2],

    block_log: [u8; 2],
    flags: [u8; 2],

    no_ids: [u8; 2],
    version_major: [u8; 2],
    version_minor: [u8; 2],

    root_inode: [u8; 8],
    bytes_used: [u8; 8],

    id_table_start: [u8; 8],
    xattr_id_table_start: [u8; 8],
    inode_table_start: [u8; 8],
    directory_table_start: [u8; 8],
    fragment_table_start: [u8; 8],
    export_table_start: [u8; 8],
}

impl Superblock {
    // TODO: check Result
    pub fn new<R: Read>(reader: &mut R) -> Result<Self> {
        let mut sb: Self = unsafe { mem::zeroed() };
        unsafe {
            let sb_slice = slice::from_raw_parts_mut(&mut sb as *mut _ as *mut u8, SUPERBLOCK_SIZE);
            reader.read_exact(sb_slice)?;
        }

        if sb.magic() != MAGIC {
            return Err(Error::new(
                ErrorKind::Other,
                format!("invalid magic {}", sb.magic()),
            ));
        }
        if sb.block_size().ilog2() != sb.block_log().into() {
            return Err(Error::new(
                ErrorKind::Other,
                format!("invalid block size {}", sb.block_size()),
            ));
        }
        if sb.xattr_id_table_start() != INVALID_BLK {
            return Err(Error::new(
                ErrorKind::Other,
                r##"Xattrs in filesystem! These are not 
                supported on this build of Mksquashfs\n"##,
            ));
        }
        Ok(sb)
    }

    get_set_field!(magic, set_magic, u32);
    get_set_field!(inodes, set_inodes, u32);
    get_set_field!(mkfs_time, set_mkfs_time, u32);
    get_set_field!(block_size, set_block_size, u32);
    get_set_field!(fragments, set_fragments, u32);
    get_set_field!(block_log, set_block_log, u16);
    get_set_field!(compressor, set_compressor, u16);
    get_set_field!(flags, set_flags, Flags);
    get_set_field!(no_ids, set_no_ids, u16);
    get_set_field!(version_major, set_version_major, u16);
    get_set_field!(version_minor, set_version_minor, u16);
    get_set_field!(root_inode, set_root_inode, i64);
    get_set_field!(bytes_used, set_bytes_used, u64);
    get_set_field!(id_table_start, set_id_table_start, u64);
    get_set_field!(xattr_id_table_start, set_xattr_id_table_start, i64);
    get_set_field!(inode_table_start, set_inode_table_start, i64);
    get_set_field!(directory_table_start, set_directory_table_start, i64);
    get_set_field!(fragment_table_start, set_fragment_table_start, u64);
    get_set_field!(export_table_start, set_export_table_start, i64);
}

bitflags! {
    #[derive(Default)]
    pub struct Flags: u16 {
        const INODES_STORED_UNCOMPRESSED = 0x0001;
        const DATA_BLOCKS_STORED_UNCOMPRESSED = 0x0002;
        const UNUSED = 0x0004;
        const FRAGMENTS_STORED_UNCOMPRESSED = 0x0008;
        const FRAGMENTS_ARE_NOT_USED = 0x0010;
        const FRAGMENTS_ALWAYS_GENERATED = 0x0020;
        const DATA_DEDUPLICATED = 0x0040;
        const NFSEXPORT_TABLE_EXISTS = 0x0080;
        const XATTRS_STORED_UNCOMPRESSED = 0x0100;
        const NO_XATTRS_IN_ARCHIVE = 0x0200;
        const COMPRESSOR_OPTIONS_PRESENT = 0x0400;
        const IDTABLE_UNCOMPRESSED = 0x0800;
    }
}

impl Flags {
    pub fn from_le_bytes(bytes: [u8; 2]) -> Self {
        unsafe { Self::from_bits_unchecked(u16::from_le_bytes(bytes)) }
    }

    pub fn to_le_bytes(self) -> [u8; 2] {
        self.bits.to_le_bytes()
    }
}

impl Display for Flags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Display for Superblock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "
inodes {}
mkfs_time {}
block_size {}
fragments {}
block_log {}
compressor {}
flags {}
no_ids {}
version_major {}
version_minor {}
root_inode {}
bytes_used {}
id_table_start {}
xattr_id_table_start {}
inode_table_start {}
directory_table_start {}
fragment_table_start {}
export_table_start {}
",
            self.inodes(),
            self.mkfs_time(),
            self.block_size(),
            self.fragments(),
            self.block_log(),
            self.compressor(),
            self.flags(),
            self.no_ids(),
            self.version_major(),
            self.version_minor(),
            self.root_inode(),
            self.bytes_used(),
            self.id_table_start(),
            self.xattr_id_table_start(),
            self.inode_table_start(),
            self.directory_table_start(),
            self.fragment_table_start(),
            self.export_table_start()
        )
    }
}
