#![feature(int_log)]

// sqsh in binary
pub const MAGIC: u32 = 0x7371_7368;
pub const SUPERBLOCK_SIZE: usize = 96;
pub const METADATA_SIZE: usize = 8 * 1024;
pub const INVALID: i64 = 0xffffffffffff;
pub const INVALID_FRAG: u32 = 0xffffffff;
pub const INVALID_XATTR: u32 = 0xffffffff;
pub const INVALID_BLK: i64 = -1;
pub const USED_BLK: i64 = -2;

use std::io::{Read, Seek};
pub trait ReadSeek: Read + Seek {}
impl<RS: Read + Seek> ReadSeek for RS {}

pub mod compressors;
mod fragments;
pub mod image;
pub mod inode;
pub(crate) mod read;
pub(crate) mod superblock;
pub(crate) mod utils;

#[cfg(test)]
mod tests;
