use crate::{
    compressors::Compressor, read::read_block, superblock::Superblock, utils::get_set_field_tuple,
    ReadSeek, INVALID_FRAG, METADATA_SIZE,
};
use core::panic;
use std::{
    fmt::{Debug, Display, Write},
    io::Error,
    io::{self, ErrorKind, Read, Result},
    mem, str,
};

#[derive(Debug)]
pub enum InodeType {
    Directory,
    LDirectory,
    File,
    LFile,
    Symlink,
    LSymlink,
    BlockDevice,
    LBlockDevice,
    CharacterDevice,
    LCharacterDevice,
    NamedPipe,
    LNamedPipe,
    Socket,
    LSocket,
    Unknown,
}

impl InodeType {
    pub fn from_reader<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        let mut bytes: [u8; 2] = [0; 2];
        reader.read_exact(&mut bytes)?;
        let inode_type = u16::from_le_bytes(bytes);
        Ok(inode_type.into())
    }
}

impl From<u16> for InodeType {
    fn from(value: u16) -> Self {
        match value {
            1 => Self::Directory,
            2 => Self::File,
            3 => Self::Symlink,
            4 => Self::BlockDevice,
            5 => Self::CharacterDevice,
            6 => Self::NamedPipe,
            7 => Self::Socket,
            8 => Self::LDirectory,
            9 => Self::LFile,
            10 => Self::LSymlink,
            11 => Self::LBlockDevice,
            12 => Self::LCharacterDevice,
            13 => Self::LNamedPipe,
            14 => Self::LSocket,
            _ => {
                dbg!("bad inode_type: {}", value);
                Self::Unknown
            }
        }
    }
}

impl From<InodeType> for u16 {
    fn from(inode: InodeType) -> Self {
        match inode {
            InodeType::Directory => 1,
            InodeType::File => 2,
            InodeType::Symlink => 3,
            InodeType::BlockDevice => 4,
            InodeType::CharacterDevice => 5,
            InodeType::NamedPipe => 6,
            InodeType::Socket => 7,
            InodeType::LDirectory => 8,
            InodeType::LFile => 9,
            InodeType::LSymlink => 10,
            InodeType::LBlockDevice => 11,
            InodeType::LCharacterDevice => 12,
            InodeType::LNamedPipe => 13,
            InodeType::LSocket => 14,
            InodeType::Unknown => unimplemented!(),
        }
    }
}

pub fn read_inode_header<R: Read + ?Sized>(
    reader: &mut R,
    superblock: &Superblock,
) -> Result<InodeHeader> {
    let inode_type = InodeType::from_reader(reader)?;

    let inode_header = match inode_type {
        InodeType::Directory => {
            let dir = DirectoryInodeHeader::from_parsed_inode_type(inode_type, reader)?;
            InodeHeader::Directory(dir)
        }
        InodeType::File => {
            let reg = RegularInodeHeader::from_parsed_inode_type(inode_type, reader, superblock)?;
            InodeHeader::Regular(reg)
        }
        InodeType::Symlink => {
            let sym =
                SymlinkInodeHeader::from_parsed_inode_type(inode_type, reader, superblock, false)?;
            InodeHeader::Symlink(sym)
        }
        InodeType::LSymlink => {
            let sym =
                SymlinkInodeHeader::from_parsed_inode_type(inode_type, reader, superblock, true)?;
            InodeHeader::LSymlink(sym)
        }
        InodeType::CharacterDevice | InodeType::BlockDevice => {
            let dev = DevInodeHeader::from_parsed_inode_type(inode_type, reader)?;
            InodeHeader::Dev(dev)
        }
        InodeType::NamedPipe | InodeType::Socket => {
            let ipc = IPCInodeHeader::from_parsed_inode_type(inode_type, reader)?;
            InodeHeader::IPC(ipc)
        }
        InodeType::LDirectory => {
            let ldir =
                LDirectoryInodeHeader::from_parsed_inode_type(inode_type, reader, superblock)?;
            InodeHeader::LDirectory(ldir)
        }
        InodeType::LFile => {
            let lreg = LRegularInodeHeader::from_parsed_inode_type(inode_type, reader, superblock)?;
            InodeHeader::LRegular(lreg)
        }
        InodeType::LCharacterDevice | InodeType::LBlockDevice => {
            let ldev = LDevInodeHeader::from_parsed_inode_type(inode_type, reader)?;
            InodeHeader::LDev(ldev)
        }
        InodeType::LNamedPipe | InodeType::LSocket => {
            let lipc = LIPCInodeHeader::from_parsed_inode_type(inode_type, reader)?;
            InodeHeader::LIPC(lipc)
        }
        _ => {
            dbg!(format!("bad inode_type: {:?}", inode_type));
            unimplemented!()
        }
    };

    Ok(inode_header)
}

#[derive(Debug)]
pub enum InodeHeader {
    Directory(DirectoryInodeHeader),
    LDirectory(LDirectoryInodeHeader),
    Regular(RegularInodeHeader),
    LRegular(LRegularInodeHeader),
    Symlink(SymlinkInodeHeader),
    LSymlink(SymlinkInodeHeader),
    Dev(DevInodeHeader),
    LDev(LDevInodeHeader),
    IPC(IPCInodeHeader),
    LIPC(LIPCInodeHeader),
}

impl Display for InodeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InodeHeader::Directory(i) => {
                write!(f, "Directory: {}", i)
            }
            InodeHeader::LDirectory(i) => {
                write!(f, "LDirectory: {}", i)
            }
            InodeHeader::Regular(i) => {
                write!(f, "Regular: {}", i)
            }
            InodeHeader::LRegular(i) => {
                write!(f, "LRegular: {}", i)
            }
            InodeHeader::Symlink(i) => {
                write!(f, "Symlink: {}", i)
            }
            InodeHeader::LSymlink(i) => {
                write!(f, "LSymlink: {}", i)
            }
            InodeHeader::Dev(i) => {
                write!(f, "Dev: {}", i)
            }
            InodeHeader::LDev(i) => {
                write!(f, "LDev: {}", i)
            }
            InodeHeader::IPC(i) => {
                write!(f, "IPC: {}", i)
            }
            InodeHeader::LIPC(i) => {
                write!(f, "LIPC: {}", i)
            }
        }
    }
}

// sizeof dir -> 32
// struct squashfs_dir_inode_header {
// 	0 2 unsigned short		inode_type;
// 	2 2 unsigned short		mode;
// 	4 2 unsigned short		uid;
// 	6 2 unsigned short		guid;
// 	8 4 unsigned int		mtime;
// 	12 4 unsigned int 		inode_number;
// 	16 4 unsigned int		start_block;
// 	20 4 unsigned int		nlink;
// 	24 2 unsigned short		file_size;
// 	26 2 unsigned short		offset;
// 	28 4 unsigned int		parent_inode;
// };

pub const DIRECTORY_INODE_HEADER_SIZE: usize = 32;

#[derive(Debug)]
pub struct DirectoryInodeHeader([u8; DIRECTORY_INODE_HEADER_SIZE]);

impl DirectoryInodeHeader {
    fn from_parsed_inode_type<R: Read + ?Sized>(
        inode_type: InodeType,
        reader: &mut R,
    ) -> Result<Self> {
        let mut buf: [u8; DIRECTORY_INODE_HEADER_SIZE] = [0; DIRECTORY_INODE_HEADER_SIZE];
        let inode_type: u16 = inode_type.into();
        let inode_type_bytes = inode_type.to_le_bytes();
        buf[0] = inode_type_bytes[0];
        buf[1] = inode_type_bytes[1];
        reader.read_exact(&mut buf[2..])?;
        Ok(Self(buf))
    }

    // TODO
    fn entries<R: Read + ?Sized>(&self, _reader: &mut R) -> Vec<DirectoryEntry> {
        let _directory_start_block = self.start_block();
        vec![]
    }

    get_set_field_tuple!(inode_type, set_inode_type, u16, 0, 2);
    get_set_field_tuple!(mode, set_mode, u16, 2, 2);
    get_set_field_tuple!(uid, set_uid, u16, 4, 2);
    get_set_field_tuple!(guid, set_guid, u16, 6, 2);
    get_set_field_tuple!(mtime, set_mtime, u32, 8, 4);

    get_set_field_tuple!(inode_number, set_inode_number, u32, 12, 4);
    get_set_field_tuple!(start_block, set_start_block, u32, 16, 4);
    get_set_field_tuple!(nlink, set_nlink, u32, 20, 4);
    get_set_field_tuple!(file_size, set_file_size, u16, 24, 2);
    get_set_field_tuple!(offset, set_offset, u16, 26, 2);
    get_set_field_tuple!(parent_inode, set_parent_inode, u32, 28, 4);
}

impl Display for DirectoryInodeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "mode {:o}, parent inode: {}, file size {}, mtime {}",
            self.mode(),
            self.parent_inode(),
            self.file_size(),
            self.mtime()
        )
    }
}

// sizeof ldir -> 40
// struct squashfs_ldir_inode_header {
// 	0 2 unsigned short		inode_type;
// 	2 2 unsigned short		mode;
// 	4 2 unsigned short		uid;
// 	6 2 unsigned short		guid;
// 	8 4 unsigned int		mtime;
// 	12 4 unsigned int 		inode_number;
// 	16 4 unsigned int		nlink;
// 	20 4 unsigned int		file_size;
// 	24 4 unsigned int		start_block;
// 	28 4 unsigned int		parent_inode;
// 	32 2 unsigned short		i_count;
// 	34 2 unsigned short		offset;
// 	36 4 unsigned int		xattr;
// 	struct squashfs_dir_index	index[0];
// };

pub const LDIRECTORY_INODE_HEADER_SIZE: usize = 40;

#[derive(Debug)]
pub struct LDirectoryInodeHeader(
    [u8; LDIRECTORY_INODE_HEADER_SIZE],
    Option<Vec<DirectoryIndex>>,
);

impl LDirectoryInodeHeader {
    fn from_parsed_inode_type<R: Read + ?Sized>(
        inode_type: InodeType,
        reader: &mut R,
        _superblock: &Superblock,
    ) -> Result<Self> {
        let mut buf: [u8; LDIRECTORY_INODE_HEADER_SIZE] = [0; LDIRECTORY_INODE_HEADER_SIZE];
        let inode_type: u16 = inode_type.into();
        let inode_type_bytes = inode_type.to_le_bytes();
        buf[0] = inode_type_bytes[0];
        buf[1] = inode_type_bytes[1];
        reader.read_exact(&mut buf[2..])?;
        let mut inode = Self(buf, None);
        let mut index = Vec::with_capacity(inode.i_count() as usize);
        for _i in 0..inode.i_count() {
            let dir_ind = DirectoryIndex::from_reader(reader)?;
            // TODO: check what the rest of the buffer contains
            io::copy(
                &mut reader.take((dir_ind.size() + 1) as u64),
                &mut io::sink(),
            )?;
            index.push(dir_ind);
        }
        inode.1 = Some(index);
        Ok(inode)
    }

    pub fn inodes(&self) -> &[DirectoryIndex] {
        todo!()
    }

    get_set_field_tuple!(inode_type, set_inode_type, u16, 0, 2);
    get_set_field_tuple!(mode, set_mode, u16, 2, 2);
    get_set_field_tuple!(uid, set_uid, u16, 4, 2);
    get_set_field_tuple!(guid, set_guid, u16, 6, 2);
    get_set_field_tuple!(mtime, set_mtime, u32, 8, 4);

    get_set_field_tuple!(inode_number, set_inode_number, u32, 12, 4);
    get_set_field_tuple!(nlink, set_nlink, u32, 16, 4);
    get_set_field_tuple!(file_size, set_file_size, u32, 20, 4);
    get_set_field_tuple!(start_block, set_start_block, u32, 24, 4);
    get_set_field_tuple!(parent_inode, set_parent_inode, u32, 28, 4);
    get_set_field_tuple!(i_count, set_i_count, u16, 32, 2);
    get_set_field_tuple!(offset, set_offset, u16, 34, 2);
    get_set_field_tuple!(xattr, set_xattr, u32, 36, 4);
}

impl Display for LDirectoryInodeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut index = String::new();
        if let Some(dindex) = &self.1 {
            write!(index, "[")?;
            for d in dindex {
                write!(index, "[{}], ", d)?;
            }
            write!(index, "]")?;
        }
        write!(
            f,
            "mode {:o}, parent inode: {}, file size {}, mtime {}, i_count {}",
            self.mode(),
            self.parent_inode(),
            self.file_size(),
            self.mtime(),
            index
        )
    }
}

pub const DIRECTORY_INDEX_SIZE: usize = 12;

#[derive(Debug)]
pub struct DirectoryIndex([u8; DIRECTORY_INDEX_SIZE]);

impl DirectoryIndex {
    fn from_reader<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        let mut buf = [0; DIRECTORY_INDEX_SIZE];
        reader.read_exact(&mut buf)?;
        Ok(Self(buf))
    }

    get_set_field_tuple!(index, set_index, u32, 0, 4);
    get_set_field_tuple!(start_block, set_start_block, u32, 4, 4);
    get_set_field_tuple!(size, set_size, u32, 8, 4);
}

impl Display for DirectoryIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "index {}, start_block {}, size {}",
            self.index(),
            self.start_block(),
            self.size()
        )
    }
}

// sizeof reg -> 32
// struct squashfs_reg_inode_header {
// 0 2	unsigned short		inode_type;
// 2 2	unsigned short		mode;
// 4 2	unsigned short		uid;
// 6 2	unsigned short		guid;
// 8 4	unsigned int		mtime;
// 12 4	unsigned int 		inode_number;
// 16 4	unsigned int		start_block;
// 20 4	unsigned int		fragment;
// 24 4	unsigned int		offset;
// 28 4	unsigned int		file_size;
//      unsigned int		block_list[0];
// };

pub const REGULAR_INODE_HEADER_SIZE: usize = 32;

#[derive(Debug)]
pub struct RegularInodeHeader(
    [u8; REGULAR_INODE_HEADER_SIZE],
    Option<String>,
    Option<Vec<u32>>,
);

impl RegularInodeHeader {
    fn from_parsed_inode_type<R: Read + ?Sized>(
        inode_type: InodeType,
        reader: &mut R,
        superblock: &Superblock,
    ) -> Result<Self> {
        let mut buf: [u8; REGULAR_INODE_HEADER_SIZE] = [0; REGULAR_INODE_HEADER_SIZE];
        let inode_type: u16 = inode_type.into();
        let inode_type_bytes = inode_type.to_le_bytes();
        buf[0] = inode_type_bytes[0];
        buf[1] = inode_type_bytes[1];
        reader.read_exact(&mut buf[2..])?;
        let mut inode = Self(buf, None, None);
        let fragments = inode.fragment();
        if fragments != INVALID_FRAG && fragments > superblock.fragments() {
            return Err(Error::new(ErrorKind::Other, "corrupted filesystem"));
        }
        let fragment_blocks = fragment_blocks(fragments, inode.file_size() as u64, superblock);
        if fragment_blocks > 0 {
            let blocks = block_list(fragment_blocks, reader)?;
            inode.2 = Some(blocks);
        }
        Ok(inode)
    }

    get_set_field_tuple!(inode_type, set_inode_type, u16, 0, 2);
    get_set_field_tuple!(mode, set_mode, u16, 2, 2);
    get_set_field_tuple!(uid, set_uid, u16, 4, 2);
    get_set_field_tuple!(guid, set_guid, u16, 6, 2);
    get_set_field_tuple!(mtime, set_mtime, u32, 8, 4);
    get_set_field_tuple!(inode_number, set_inode_number, u32, 12, 4);
    get_set_field_tuple!(start_block, set_start_block, u32, 16, 4);
    get_set_field_tuple!(fragment, set_fragment, u32, 20, 4);
    get_set_field_tuple!(offset, set_offset, u32, 24, 4);
    get_set_field_tuple!(file_size, set_file_size, u32, 28, 4);
}

impl Display for RegularInodeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "mode {:o}, uid: {}, guid: {}, file size {}, mtime {}, blocks: {:?}",
            self.mode(),
            self.uid(),
            self.guid(),
            self.file_size(),
            self.mtime(),
            self.2,
        )
    }
}

// sizeof lreg -> 56
// struct squashfs_lreg_inode_header {
// 	0 2 unsigned short		inode_type;
// 	2 2 unsigned short		mode;
// 	4 2 unsigned short		uid;
// 	6 2 unsigned short		guid;
// 	8 4 unsigned int		mtime;
// 	12 4 unsigned int 		inode_number;
// 	16 8 squashfs_block		start_block;
// 	24 8 long long		file_size;
// 	32 8 long long		sparse;
// 	40 4 unsigned int		nlink;
// 	44 4 unsigned int		fragment;
// 	48 4 unsigned int		offset;
// 	52 4 unsigned int		xattr;
// };

pub const LREGULAR_INODE_HEADER_SIZE: usize = 56;

#[derive(Debug)]
pub struct LRegularInodeHeader([u8; LREGULAR_INODE_HEADER_SIZE], Option<Vec<u32>>);

impl LRegularInodeHeader {
    fn from_parsed_inode_type<R: Read + ?Sized>(
        inode_type: InodeType,
        reader: &mut R,
        superblock: &Superblock,
    ) -> Result<Self> {
        let mut buf: [u8; LREGULAR_INODE_HEADER_SIZE] = [0; LREGULAR_INODE_HEADER_SIZE];
        let inode_type: u16 = inode_type.into();
        let inode_type_bytes = inode_type.to_le_bytes();
        buf[0] = inode_type_bytes[0];
        buf[1] = inode_type_bytes[1];
        reader.read_exact(&mut buf[2..])?;
        let mut inode = Self(buf, None);
        let fragment_blocks = fragment_blocks(inode.fragment(), inode.file_size(), superblock);
        let blocks = block_list(fragment_blocks, reader)?;
        inode.1 = Some(blocks);
        Ok(inode)
    }

    get_set_field_tuple!(inode_type, set_inode_type, u16, 0, 2);
    get_set_field_tuple!(mode, set_mode, u16, 2, 2);
    get_set_field_tuple!(uid, set_uid, u16, 4, 2);
    get_set_field_tuple!(guid, set_guid, u16, 6, 2);
    get_set_field_tuple!(mtime, set_mtime, u32, 8, 4);

    get_set_field_tuple!(inode_number, set_inode_number, u32, 12, 4);
    get_set_field_tuple!(start_block, set_start_block, u64, 16, 8);
    get_set_field_tuple!(file_size, set_file_size, u64, 24, 8);
    get_set_field_tuple!(sparse, set_sparse, u64, 32, 8);
    get_set_field_tuple!(nlink, set_nlink, u32, 40, 4);
    get_set_field_tuple!(fragment, set_fragment, u32, 44, 4);
    get_set_field_tuple!(offset, set_offset, u32, 48, 4);
    get_set_field_tuple!(xattr, set_xattr, u32, 52, 4);
}

impl Display for LRegularInodeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "mode {:o}, uid: {}, guid: {}, file size {}, mtime {}, xattr: {}",
            self.mode(),
            self.uid(),
            self.guid(),
            self.file_size(),
            self.mtime(),
            self.xattr()
        )
    }
}

// sizeof symlink -> 24
// struct squashfs_symlink_inode_header {
// 0 2 	unsigned short		inode_type;
// 2 2 	unsigned short		mode;
// 4 2 	unsigned short		uid;
// 6 2 	unsigned short		guid;
// 8 4 	unsigned int		mtime;
// 12 4	unsigned int 		inode_number;
// 16 4	unsigned int		nlink;
// 20 4	unsigned int		symlink_size;
//  	char			symlink[0];
// };

pub const SYMLINK_INODE_HEADER_SIZE: usize = 24;

#[derive(Debug)]
pub struct SymlinkInodeHeader([u8; SYMLINK_INODE_HEADER_SIZE], Vec<u8>, Option<u32>);

impl SymlinkInodeHeader {
    fn from_parsed_inode_type<R: Read + ?Sized>(
        inode_type: InodeType,
        reader: &mut R,
        _superblock: &Superblock,
        is_extended: bool,
    ) -> Result<Self> {
        let mut buf: [u8; SYMLINK_INODE_HEADER_SIZE] = [0; SYMLINK_INODE_HEADER_SIZE];
        let inode_type: u16 = inode_type.into();
        let inode_type_bytes = inode_type.to_le_bytes();
        buf[0] = inode_type_bytes[0];
        buf[1] = inode_type_bytes[1];
        reader.read_exact(&mut buf[2..])?;
        let mut inode = Self(buf, vec![], None);
        let mut r = reader.take(inode.symlink_size() as u64);
        r.read_to_end(&mut inode.1)?;
        if is_extended {
            let mut r = reader.take(mem::size_of::<u32>() as u64);
            let mut buf = [0; 4];
            r.read_exact(&mut buf)?;
            inode.2 = Some(u32::from_le_bytes(buf));
        }

        Ok(inode)
    }

    fn symlink(&self) -> &str {
        match str::from_utf8(&self.1) {
            Ok(v) => v,
            Err(e) => panic!("symlink not utf8 readable: {}", e),
        }
    }

    get_set_field_tuple!(inode_type, set_inode_type, u16, 0, 2);
    get_set_field_tuple!(mode, set_mode, u16, 2, 2);
    get_set_field_tuple!(uid, set_uid, u16, 4, 2);
    get_set_field_tuple!(guid, set_guid, u16, 6, 2);
    get_set_field_tuple!(mtime, set_mtime, u32, 8, 4);

    get_set_field_tuple!(inode_number, set_inode_number, u32, 12, 4);
    get_set_field_tuple!(nlink, set_nlink, u32, 16, 4);
    get_set_field_tuple!(symlink_size, set_symlink_size, u32, 20, 4);
}

impl Display for SymlinkInodeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "inode number: {}, mode {:o}, nlink: {}, symlink size {}, mtime {}, symlink {}",
            self.inode_number(),
            self.mode(),
            self.nlink(),
            self.symlink_size(),
            self.mtime(),
            self.symlink()
        )
    }
}

// sizeof dev -> 24
// struct squashfs_dev_inode_header {
// 0 2	unsigned short		inode_type;
// 2 2	unsigned short		mode;
// 4 2	unsigned short		uid;
// 6 2	unsigned short		guid;
// 8 4	unsigned int		mtime;
// 12 4	unsigned int 		inode_number;
// 16 4	unsigned int		nlink;
// 20 4	unsigned int		rdev;
// };

pub const DEV_INODE_HEADER_SIZE: usize = 24;

#[derive(Debug)]
pub struct DevInodeHeader([u8; DEV_INODE_HEADER_SIZE], Option<String>);

impl DevInodeHeader {
    pub fn new<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf: [u8; DEV_INODE_HEADER_SIZE] = [0; DEV_INODE_HEADER_SIZE];
        reader.read_exact(&mut buf)?;
        Ok(Self(buf, None))
    }

    fn from_parsed_inode_type<R: Read + ?Sized>(
        inode_type: InodeType,
        reader: &mut R,
    ) -> Result<Self> {
        let mut buf: [u8; DEV_INODE_HEADER_SIZE] = [0; DEV_INODE_HEADER_SIZE];
        let inode_type: u16 = inode_type.into();
        let inode_type_bytes = inode_type.to_le_bytes();
        buf[0] = inode_type_bytes[0];
        buf[1] = inode_type_bytes[1];
        reader.read_exact(&mut buf[2..])?;
        Ok(Self(buf, None))
    }

    get_set_field_tuple!(inode_type, set_inode_type, u16, 0, 2);
    get_set_field_tuple!(mode, set_mode, u16, 2, 2);
    get_set_field_tuple!(uid, set_uid, u16, 4, 2);
    get_set_field_tuple!(guid, set_guid, u16, 6, 2);
    get_set_field_tuple!(mtime, set_mtime, u32, 8, 4);

    get_set_field_tuple!(inode_number, set_inode_number, u32, 12, 4);
    get_set_field_tuple!(nlink, set_nlink, u32, 16, 4);
    get_set_field_tuple!(rdev, set_rdev, u32, 20, 4);
}

impl Display for DevInodeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "inode number: {}, mode {:o}, nlink {}, mtime {}",
            self.inode_number(),
            self.mode(),
            self.nlink(),
            self.mtime(),
        )
    }
}

// sizeof ldev -> 28
// struct squashfs_ldev_inode_header {
// 	0 2 unsigned short		inode_type;
// 	2 2 unsigned short		mode;
// 	4 2 unsigned short		uid;
// 	6 2 unsigned short		guid;
// 	8 4 unsigned int		mtime;
// 	12 4 unsigned int 		inode_number;
// 	16 4 unsigned int		nlink;
// 	20 4 unsigned int		rdev;
// 	24 4 unsigned int		xattr;
// }; 28

pub const LDEV_INODE_HEADER_SIZE: usize = 28;

#[derive(Debug)]
pub struct LDevInodeHeader([u8; LDEV_INODE_HEADER_SIZE]);

impl LDevInodeHeader {
    pub fn new<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf: [u8; LDEV_INODE_HEADER_SIZE] = [0; LDEV_INODE_HEADER_SIZE];
        reader.read_exact(&mut buf)?;
        Ok(Self(buf))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut buf: [u8; LDEV_INODE_HEADER_SIZE] = [0; LDEV_INODE_HEADER_SIZE];
        buf.clone_from_slice(bytes);
        Ok(Self(buf))
    }

    fn from_parsed_inode_type<R: Read + ?Sized>(
        inode_type: InodeType,
        reader: &mut R,
    ) -> Result<Self> {
        let mut buf: [u8; LDEV_INODE_HEADER_SIZE] = [0; LDEV_INODE_HEADER_SIZE];
        let inode_type: u16 = inode_type.into();
        let inode_type_bytes = inode_type.to_le_bytes();
        buf[0] = inode_type_bytes[0];
        buf[1] = inode_type_bytes[1];
        reader.read_exact(&mut buf[2..])?;
        Ok(Self(buf))
    }

    get_set_field_tuple!(inode_type, set_inode_type, u16, 0, 2);
    get_set_field_tuple!(mode, set_mode, u16, 2, 2);
    get_set_field_tuple!(uid, set_uid, u16, 4, 2);
    get_set_field_tuple!(guid, set_guid, u16, 6, 2);
    get_set_field_tuple!(mtime, set_mtime, u32, 8, 4);

    get_set_field_tuple!(inode_number, set_inode_number, u32, 12, 4);
    get_set_field_tuple!(nlink, set_nlink, u32, 16, 4);
    get_set_field_tuple!(rdev, set_rdev, u32, 20, 4);
    get_set_field_tuple!(xattr, set_xattr, u32, 24, 4);
}

impl Display for LDevInodeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "inode number: {}, mode {:o}, nlink {}, mtime {}, rdev {}",
            self.inode_number(),
            self.mode(),
            self.nlink(),
            self.mtime(),
            self.rdev()
        )
    }
}

// sizeof ipc -> 20
// struct squashfs_ipc_inode_header {
// 0 2	unsigned short		inode_type;
// 2 2	unsigned short		mode;
// 4 2	unsigned short		uid;
// 6 2	unsigned short		guid;
// 8 4	unsigned int		mtime;
// 12 4	unsigned int 		inode_number;
// 16 4	unsigned int		nlink;
// };

pub const IPC_INODE_HEADER_SIZE: usize = 20;

#[derive(Debug)]
pub struct IPCInodeHeader([u8; IPC_INODE_HEADER_SIZE]);

impl IPCInodeHeader {
    pub fn new<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf: [u8; IPC_INODE_HEADER_SIZE] = [0; IPC_INODE_HEADER_SIZE];
        reader.read_exact(&mut buf)?;
        Ok(Self(buf))
    }

    fn from_parsed_inode_type<R: Read + ?Sized>(
        inode_type: InodeType,
        reader: &mut R,
    ) -> Result<Self> {
        let mut buf: [u8; IPC_INODE_HEADER_SIZE] = [0; IPC_INODE_HEADER_SIZE];
        let inode_type: u16 = inode_type.into();
        let inode_type_bytes = inode_type.to_le_bytes();
        buf[0] = inode_type_bytes[0];
        buf[1] = inode_type_bytes[1];
        reader.read_exact(&mut buf[2..])?;
        Ok(Self(buf))
    }

    get_set_field_tuple!(inode_type, set_inode_type, u16, 0, 2);
    get_set_field_tuple!(mode, set_mode, u16, 2, 2);
    get_set_field_tuple!(uid, set_uid, u16, 4, 2);
    get_set_field_tuple!(guid, set_guid, u16, 6, 2);
    get_set_field_tuple!(mtime, set_mtime, u32, 8, 4);

    get_set_field_tuple!(inode_number, set_inode_number, u32, 12, 4);
    get_set_field_tuple!(nlink, set_nlink, u32, 16, 4);
}

impl Display for IPCInodeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "inode number: {}, mode {:o}, nlink {}, mtime {}",
            self.inode_number(),
            self.mode(),
            self.nlink(),
            self.mtime(),
        )
    }
}

// sizeof lipc -> 24
// struct squashfs_lipc_inode_header {
// 	0 2 unsigned short		inode_type;
// 	2 2 unsigned short		mode;
// 	4 2 unsigned short		uid;
// 	6 2 unsigned short		guid;
// 	8 4 unsigned int		mtime;
// 	12 4 unsigned int 		inode_number;
// 	16 4 unsigned int		nlink;
// 	20 4 unsigned int		xattr;
// }; 24

pub const LIPC_INODE_HEADER_SIZE: usize = 24;

#[derive(Debug)]
pub struct LIPCInodeHeader([u8; LIPC_INODE_HEADER_SIZE]);

impl LIPCInodeHeader {
    pub fn new<R: Read>(reader: &mut R) -> Result<Self> {
        let mut buf: [u8; LIPC_INODE_HEADER_SIZE] = [0; LIPC_INODE_HEADER_SIZE];
        reader.read_exact(&mut buf)?;
        Ok(Self(buf))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut buf: [u8; LIPC_INODE_HEADER_SIZE] = [0; LIPC_INODE_HEADER_SIZE];
        buf.clone_from_slice(bytes);
        Ok(Self(buf))
    }

    fn from_parsed_inode_type<R: Read + ?Sized>(
        inode_type: InodeType,
        reader: &mut R,
    ) -> Result<Self> {
        let mut buf: [u8; LIPC_INODE_HEADER_SIZE] = [0; LIPC_INODE_HEADER_SIZE];
        let inode_type: u16 = inode_type.into();
        let inode_type_bytes = inode_type.to_le_bytes();
        buf[0] = inode_type_bytes[0];
        buf[1] = inode_type_bytes[1];
        reader.read_exact(&mut buf[2..])?;
        Ok(Self(buf))
    }

    get_set_field_tuple!(inode_type, set_inode_type, u16, 0, 2);
    get_set_field_tuple!(mode, set_mode, u16, 2, 2);
    get_set_field_tuple!(uid, set_uid, u16, 4, 2);
    get_set_field_tuple!(guid, set_guid, u16, 6, 2);
    get_set_field_tuple!(mtime, set_mtime, u32, 8, 4);

    get_set_field_tuple!(inode_number, set_inode_number, u32, 12, 4);
    get_set_field_tuple!(nlink, set_nlink, u32, 16, 4);
    get_set_field_tuple!(xattr, set_xattr, u32, 20, 4);
}

impl Display for LIPCInodeHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "inode number: {}, mode {:o}, nlink {}, mtime {}",
            self.inode_number(),
            self.mode(),
            self.nlink(),
            self.mtime(),
        )
    }
}

fn fragment_blocks(fragment: u32, file_size: u64, superblock: &Superblock) -> u64 {
    if fragment == INVALID_FRAG {
        (file_size + superblock.block_size() as u64 - 1) >> superblock.block_log()
    } else {
        file_size >> superblock.block_log()
    }
}

fn block_list<R: Read + ?Sized>(blocks: u64, reader: &mut R) -> Result<Vec<u32>> {
    const U32_SIZE: usize = mem::size_of::<u32>();
    let blocks_list_size = blocks as usize * U32_SIZE;
    let mut reader = reader.take(blocks_list_size as u64);
    let mut blocks_list = Vec::with_capacity(blocks_list_size);
    reader.read_to_end(&mut blocks_list)?;
    let blocks_list = blocks_list
        .chunks(U32_SIZE)
        .map(|x| {
            let mut buf: [u8; U32_SIZE] = [0; U32_SIZE];
            buf.clone_from_slice(x);
            u32::from_le_bytes(buf)
        })
        .collect();
    Ok(blocks_list)
}

#[derive(Clone, Debug)]
pub struct DirectoryEntry(Vec<u8>);

impl DirectoryEntry {
    pub fn new(buf: Vec<u8>) -> io::Result<Self> {
        Ok(Self(buf))
    }

    // get_set_field_tuple!(offset, set_offset, u16, 0, 2);
    // get_set_field_tuple!(inode_offset, set_inode_offset, u16, 2, 2);
    // get_set_field_tuple!(_type, set_type, u16, 4, 2);
    // get_set_field_tuple!(name_size, set_name_size, u16, 6, 2);
    get_set_field_tuple!(count, set_count, u32, 0, 4);
    get_set_field_tuple!(start_block, set_start_block, u32, 4, 4);
    get_set_field_tuple!(inode_number, set_inode_number, u32, 8, 4);
}

#[derive(Clone, Debug)]
pub struct InodeEntry(Vec<u8>);

impl InodeEntry {
    pub fn new(buf: Vec<u8>) -> io::Result<Self> {
        Ok(Self(buf))
    }
}

pub fn get_directory_metadata<R: ReadSeek>(
    reader: &mut R,
    compressor: &Compressor,
    directory_start: i64,
    start: i64,
    offset: i64,
) -> Result<DirectoryEntry> {
    let mut buf = vec![];
    read_block(
        reader,
        &mut buf,
        compressor,
        (directory_start + start) as u64,
        None,
    )?;
    if offset >= buf.len() as i64 {
        return Err(Error::new(ErrorKind::Other, "offset out of range"));
    }
    let entry = DirectoryEntry::new(buf[offset as usize..].to_vec())?;

    for _i in 0..entry.count() {}
    Ok(entry)
}

// pub fn opendir(&mut self, directory: &DirectoryInodeHeader) -> Result<DirectoryEntry> {
//     let offset = directory.offset();
//     let start_block = directory.start_block();
//
//     let dir_inode = self.get_directory_metadata(start_block as i64, offset as i64)?;
//     Ok(dir_inode.clone())
// }


pub fn scan_inode_table<R: ReadSeek>(
    reader: &mut R,
    superblock: &Superblock,
    compressor: &Compressor,
) -> Result<(InodeHeader, Vec<InodeHeader>)> {
    let root_inode = superblock.root_inode();
    let mut start = superblock.inode_table_start();
    let end = superblock.directory_table_start();

    dbg!(
        "scan_inode_table: root_inode {}, inode_table_start {}, directory_table_start {}",
        root_inode,
        start,
        end
    );

    // let root_inode_start = start + squashfs_inode_blk(superblock.root_inode());
    let root_inode_start = start + (((root_inode >> 16) as u32) as i64);
    let root_inode_offset = (root_inode as u32 & 0xffff) as u32;

    // let inode = inodeHeader; // may be result
    let mut root_inode_block: Option<usize> = None; // may be result

    let mut inode_table =
        Vec::with_capacity(((end - start) as usize + METADATA_SIZE) & !(METADATA_SIZE - 1_usize));
    while start < end {
        if start == root_inode_start {
            root_inode_block = Some(inode_table.len() as usize);
            dbg!("found root_inode_block: {}", inode_table.len());
        } else {
            dbg!(
                "CHECK start = {}, end = {}, root_inode = {}, diff = {}",
                start,
                end,
                root_inode_start,
                start - root_inode_start
            );
        }
        let mut buf = Vec::with_capacity(METADATA_SIZE);
        let compressed_size = read_block(reader, &mut buf, compressor, start as u64, None)?;
        start += compressed_size as i64;

        if start != end && buf.len() != METADATA_SIZE {
            panic!(
                "corrupted: bad metadata size; start = {}, end = {}, buf.len = {}",
                start,
                end,
                buf.len()
            );
        }
        inode_table.append(&mut buf);
    }

    let root_inode_block = match root_inode_block {
        Some(r) => r,
        None => {
            panic!("corrupted: no root inode block found");
        }
    };

    if (inode_table.len() - root_inode_block as usize)
        < (root_inode_offset + DIRECTORY_INODE_HEADER_SIZE as u32) as usize
    {
        panic!("corrupted: root inode metadata size incorrect");
    }

    let _root_inode_size: usize =
        inode_table.len() - (root_inode_block + root_inode_offset as usize);

    let dir_inode = read_inode_header(
        &mut inode_table[(root_inode_block + root_inode_offset as usize)..].as_ref(),
        superblock,
    )?;
    match dir_inode {
        InodeHeader::Directory(ref d) => {
            dbg!(
                "ROOT INODE: dir mode {:o} parent {}",
                d.mode(),
                d.parent_inode()
            );
        }
        InodeHeader::LDirectory(ref d) => {
            dbg!(
                "ROOT INODE: ldir mode {:o} parent {}",
                d.mode(),
                d.parent_inode(),
            );
        }
        _ => {
            dbg!("ROOT INODE not directory {:?}", &dir_inode);
        }
    }

    let mut inode_table = &inode_table[..];
    let mut inode_headers = Vec::with_capacity(superblock.inodes() as usize);
    while !inode_table.is_empty() {
        let i = read_inode_header(&mut inode_table, superblock)?;
        inode_headers.push(i);
    }

    Ok((dir_inode, inode_headers))
}