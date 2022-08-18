use crate::compressors::{Compressor, Decompress};
use crate::fragments::FRAGMENT_ENTRY_SIZE;
use crate::superblock::Superblock;
use crate::{ReadSeek, METADATA_SIZE};
use std::io::{copy, Read, Result, Seek, SeekFrom, Write};

const COMPRESSED_BIT: u16 = 1 << 15;

fn read_block_header<R: ReadSeek + ?Sized>(reader: &mut R) -> Result<(bool, u16)> {
    let mut block_header: [u8; 2] = [0; 2];
    reader.read_exact(&mut block_header[..])?;
    let block_header = u16::from_le_bytes(block_header);

    let compressed = (block_header & COMPRESSED_BIT) == 0;
    let compressed_size = block_header & !(COMPRESSED_BIT);

    if compressed_size as usize > METADATA_SIZE {
        panic!("bad metadata size");
    }

    Ok((compressed, compressed_size))
}

pub fn read_block<R: ReadSeek + ?Sized, W: Write + ?Sized>(
    reader: &mut R,
    writer: &mut W,
    compressor: &Compressor,
    start: u64,
    expected: Option<u32>,
) -> Result<u16> {
    reader.seek(SeekFrom::Start(start))?;
    let (compressed, compressed_size) = read_block_header(reader)?;

    if compressed {
        let mut buf = Vec::with_capacity(compressed_size as usize);
        copy(&mut reader.take(compressed_size as u64), &mut buf)?;

        eprintln!("try decompress, buf.len {}", buf.len());
        let written = compressor.decompress(&mut (&buf[..]), writer)?;
        if let Some(expected) = expected {
            if expected as u64 != written {
                panic!("expected ({}) != written ({})", expected, written);
            }
        }
        Ok(compressed_size + 2)
    } else {
        copy(&mut reader.take(compressed_size as u64), writer)?;
        Ok(compressed_size + 2)
    }
}

#[derive(Debug)]
pub struct FragmentTableReader<'a, R: ReadSeek> {
    reader: R,
    compressor: &'a Compressor,
    position: usize,
    index: Vec<u64>,
    fragments: usize,
    buffer_position: u64,
    buffer: Vec<u8>,
}

impl<'a, R: ReadSeek> FragmentTableReader<'a, R> {
    pub fn new(mut reader: R, compressor: &'a Compressor, superblock: &Superblock) -> Result<Self> {
        let fragments = superblock.fragments();
        let indexes =
            ((fragments as usize * FRAGMENT_ENTRY_SIZE) + METADATA_SIZE - 1) / METADATA_SIZE;
        let indexes_bytes = indexes * 8;

        let mut index = Vec::with_capacity(indexes_bytes);
        reader.seek(SeekFrom::Start(superblock.fragment_table_start() as u64))?;
        copy(&mut (&mut reader).take(indexes_bytes as u64), &mut index)?;

        let index: Vec<u64> = index
            .chunks(8)
            .map(|x| {
                let v = match x.try_into() {
                    Ok(v) => v,
                    Err(e) => {
                        panic!("{}", e);
                    }
                };
                u64::from_le_bytes(v)
            })
            .collect();

        Ok(Self {
            reader,
            compressor,
            position: 0,
            index,
            fragments: fragments as usize,
            buffer_position: 0,
            buffer: Vec::with_capacity(METADATA_SIZE),
        })
    }

    pub fn fragments(&self)  -> usize {
        self.fragments
    }
}

impl<'a, R: ReadSeek> Read for FragmentTableReader<'a, R> {
    fn read(&mut self, writer: &mut [u8]) -> Result<usize> {
        let mut written = 0;
        let writer_len = writer.len();
        let index_len = self.index.len();

        if (self.buffer_position as usize) < self.buffer.len() {
            let buffer_remainder = self.buffer.len() - self.buffer_position as usize;
            if writer_len < buffer_remainder {
                writer.copy_from_slice(
                    &self.buffer[self.buffer_position as usize
                        ..(self.buffer_position as usize + writer_len)],
                );
                self.buffer_position += writer_len as u64;
                return Ok(writer_len);
            }
            writer[..buffer_remainder]
                .copy_from_slice(&self.buffer[self.buffer_position as usize..]);
            written += buffer_remainder;
            self.buffer.truncate(0);
            self.buffer_position = 0;
        }

        if self.position == index_len {
            return Ok(written);
        }

        let expected = match (self.position + 1) != self.index.len() {
            true => METADATA_SIZE as u32,
            false => ((self.fragments * FRAGMENT_ENTRY_SIZE) & (METADATA_SIZE - 1)) as u32,
        };

        let block_size = read_block(
            &mut self.reader,
            &mut self.buffer,
            self.compressor,
            self.index[self.position],
            Some(expected),
        )?;

        eprintln!("block size read {}", block_size);
        self.position += 1;

        let left_to_write = self.buffer.len().min(writer_len - written);
        writer[written..(written + left_to_write)].copy_from_slice(&self.buffer[..left_to_write]);
        written += left_to_write;
        self.buffer_position += left_to_write as u64;

        Ok(written)
    }
}
