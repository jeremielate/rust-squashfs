use core::panic;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::{copy, Error, ErrorKind, Read, Result, SeekFrom};
use std::ops::DerefMut;
use std::{mem, vec};

use crate::compressors::Compressor;
use crate::fragments::{FragmentEntry, FRAGMENT_ENTRY_SIZE};
use crate::inode::{scan_inode_table, DirectoryEntry, InodeEntry, InodeHeader};
use crate::read::{self, read_block, FragmentTableReader};
use crate::superblock::{Flags, Superblock};
use crate::{ReadSeek, INVALID_BLK, METADATA_SIZE, SUPERBLOCK_SIZE};

const INODE_ENTRY_SIZE: usize = 8;

#[derive(Clone, Debug)]
pub struct Image<R: ReadSeek> {
    reader: RefCell<R>,
    superblock: Superblock,
    inode_hash_table: HashMap<i64, RefCell<InodeEntry>>,
    directory_hash_table: HashMap<i64, RefCell<DirectoryEntry>>,
}

impl<'a, R: ReadSeek> Image<R> {
    pub fn new(mut reader: R) -> Result<Self> {
        let sb = Superblock::new(&mut reader)?;
        Ok(Self {
            reader: reader.into(),
            superblock: sb,
            inode_hash_table: HashMap::new(),
            directory_hash_table: HashMap::new(),
        })
    }

    pub fn get_inode_metadata(&mut self, start: i64) -> Result<RefCell<InodeEntry>> {
        if let Some(entry) = self.inode_hash_table.get(&start) {
            Ok(entry.clone())
        } else {
            let compressor = self.compressor()?;
            let inode_start = self.superblock.inode_table_start();
            let reader = self.reader.get_mut();
            let mut buf = Vec::with_capacity(METADATA_SIZE);

            read_block(
                reader,
                &mut buf,
                &compressor,
                (inode_start + start) as u64,
                Some(METADATA_SIZE as u32),
            )?;
            let entry = InodeEntry::new(buf)?;
            self.inode_hash_table.insert(start, RefCell::new(entry));
            self.inode_hash_table
                .get(&start)
                .map(Clone::clone)
                .ok_or(Error::new(ErrorKind::Other, "no entry found"))
        }
    }

    pub fn export_table(&self) -> Result<Vec<u64>> {
        let lookup_table_start = self.superblock.export_table_start();
        if lookup_table_start == INVALID_BLK {
            return Ok(vec![]);
        }
        let inodes = self.superblock.inodes() as usize;
        let lookup_bytes = inodes * INODE_ENTRY_SIZE as usize;
        // indexes
        let lookup_blocks = (lookup_bytes as usize + METADATA_SIZE - 1) / METADATA_SIZE;
        let lookup_block_bytes = lookup_blocks * mem::size_of::<u64>();
        let compressor = self.compressor()?;

        dbg!(inodes, lookup_bytes, lookup_blocks, lookup_block_bytes);

        let mut reader = self.reader.borrow_mut();
        let reader = reader.deref_mut();

        // let mut index = vec![0u8; lookup_block_bytes];
        let mut index = Vec::with_capacity(lookup_block_bytes);
        reader.seek(SeekFrom::Start(lookup_table_start as u64))?;
        copy(&mut reader.take(lookup_block_bytes as u64), &mut index)?;

        let index: Vec<i64> = index
            .chunks(mem::size_of::<i64>())
            .map(|x| {
                let v = match x.try_into() {
                    Ok(v) => v,
                    Err(e) => {
                        panic!("{}", e);
                    }
                };
                i64::from_le_bytes(v)
            })
            .collect();

        if index.len() != lookup_blocks {
            panic!(
                "index.len {}, inodes {}, lookup_blocks {}",
                index.len(),
                inodes,
                lookup_blocks
            );
        }

        let mut all_inodes = Vec::with_capacity(inodes);
        for (i, ind) in index.iter().enumerate().take(lookup_blocks) {
            let expected = match (i + 1) != lookup_blocks {
                true => METADATA_SIZE,
                false => (lookup_bytes as usize) & (METADATA_SIZE - 1),
            };

            dbg!(i, inodes, ind, expected);

            let mut block = vec![0u8; expected];
            read::read_block(
                reader,
                &mut (&mut block[..]),
                &compressor,
                *ind as u64,
                Some(expected as u32),
            )?;
            all_inodes.append(&mut block);
        }

        all_inodes
            .chunks(mem::size_of::<u64>())
            .map(|x| match x.try_into() {
                Ok(buf) => Ok(u64::from_le_bytes(buf)),
                Err(e) => Err(Error::new(
                    ErrorKind::Other,
                    format!("bad lookup id {:?}: {}", x, e),
                )),
            })
            .collect()
    }

    pub fn id_table(&self) -> Result<IDTable> {
        let no_ids = self.superblock.no_ids();

        let no_ids_bytes = no_ids as usize * mem::size_of::<u32>();
        let no_ids_blocks = (no_ids_bytes + METADATA_SIZE - 1) / METADATA_SIZE;
        let no_ids_block_bytes = no_ids_blocks * mem::size_of::<i64>();

        let compressor = self.compressor()?;
        let mut reader = self.reader.borrow_mut();
        let reader = reader.deref_mut();

        dbg!("no_ids_block_bytes {}", no_ids_block_bytes);
        // let mut index = vec![0u8; no_ids_block_bytes];
        let mut index = Vec::with_capacity(no_ids_block_bytes);
        reader.seek(SeekFrom::Start(self.superblock.id_table_start() as u64))?;
        copy(&mut reader.take(no_ids_block_bytes as u64), &mut index)?;
        let index: Vec<i64> = index
            .chunks(mem::size_of::<i64>())
            .map(|x| {
                let v = match x.try_into() {
                    Ok(v) => v,
                    Err(e) => {
                        panic!("{}", e);
                    }
                };
                i64::from_le_bytes(v)
            })
            .collect();

        let mut id_table = Vec::with_capacity(no_ids as usize);
        for (i, index) in index.iter().enumerate().take(no_ids_blocks) {
            let expected = match (i + 1) != no_ids_blocks as usize {
                true => METADATA_SIZE,
                false => no_ids_bytes & (METADATA_SIZE - 1),
            };

            dbg!(index, no_ids, i, no_ids_blocks, expected);

            let mut block = vec![0u8; expected];
            read::read_block(
                reader,
                &mut block,
                &compressor,
                *index as u64,
                Some(expected as u32),
            )?;
            id_table.append(&mut block);
        }

        let id_table = id_table
            .chunks(mem::size_of::<i32>())
            .map(|x| {
                let buf: [u8; 4] = match x.try_into() {
                    Ok(v) => v,
                    Err(e) => {
                        panic!("{}", e);
                    }
                };
                u32::from_le_bytes(buf)
            })
            .collect();

        Ok(IDTable(id_table))
    }

    pub fn compressor(&self) -> Result<Compressor> {
        let mut reader = self.reader.borrow_mut();
        let reader = reader.deref_mut();

        let compressor_options_present = self
            .superblock
            .flags()
            .contains(Flags::COMPRESSOR_OPTIONS_PRESENT);
        reader.seek(SeekFrom::Start(SUPERBLOCK_SIZE as u64))?;
        Compressor::new(
            self.superblock.compressor(),
            compressor_options_present,
            reader,
        )
    }

    pub fn read_fs(
        &mut self,
    ) -> Result<(
        Vec<FragmentEntry>,
        IDTable,
        Vec<u64>,
        InodeHeader,
        Vec<InodeHeader>,
    )> {
        let fragment_table = self.fragments()?;
        let id_table = self.id_table()?;
        let (root, inode_table) = self.inodes()?;
        let export_table = self.export_table()?;

        Ok((fragment_table, id_table, export_table, root, inode_table))
    }

    pub fn inodes(&self) -> Result<(InodeHeader, Vec<InodeHeader>)> {
        let compressor = self.compressor()?;
        let mut reader = self.reader.borrow_mut();
        let mut reader = reader.by_ref();

        scan_inode_table(&mut reader, &self.superblock, &compressor)
    }

    pub fn fragments(&self) -> Result<Vec<FragmentEntry>> {
        let compressor = self.compressor()?;
        let mut reader = self.reader.borrow_mut();
        let mut reader = reader.by_ref();

        let mut ftr = FragmentTableReader::new(&mut reader, &compressor, self.superblock())?;

        let fragments = ftr.fragments();
        let mut list = Vec::with_capacity(fragments);
        for _ in 0..fragments {
            let mut buf = [0; FRAGMENT_ENTRY_SIZE];
            ftr.read_exact(&mut buf[..])?;
            list.push(FragmentEntry::new(buf));
        }
        Ok(list)
    }

    pub fn superblock(&'a self) -> &'a Superblock {
        &self.superblock
    }
}

#[derive(Debug)]
pub struct IDTable(Vec<u32>);

impl IntoIterator for IDTable {
    type Item = u32;
    type IntoIter = <Vec<Self::Item> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl IDTable {
    fn get_id(&self, uid_gid: u16) -> u32 {
        match self.0.get(uid_gid as usize) {
            Some(v) => *v,
            None => 0,
        }
    }

    pub fn ids(&self) -> &[u32] {
        &self.0
    }
}
