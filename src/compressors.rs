use bitflags::bitflags;
use flate2::read::ZlibDecoder;


use std::fmt::{self, Debug, Display};
use std::io::{copy, Read, Result, Write};
use std::{mem, slice};
use xz2::read::XzDecoder;
use xz2::stream::Stream;

use crate::utils::{get_set_field, get_set_field_tuple};
use crate::ReadSeek;

pub trait Decompress {
    fn decompress<R: Read + ?Sized, W: Write + ?Sized>(
        &self,
        reader: &mut R,
        writer: &mut W,
    ) -> Result<u64>;
}

#[derive(Clone, Debug)]
pub enum Compressor {
    GZIP(GzipCompressor),
    XZ(XZCompressor),
    // ZSTD(ZSTDCompressor),
    // LZO,
    // LZMA,
    // LZ4,
    Undefined,
}

impl Compressor {
    pub(crate) fn new(
        compressor: u16,
        compressor_options_present: bool,
        reader: &mut dyn ReadSeek,
    ) -> Result<Self> {
        match compressor {
            1 => {
                let opts = if compressor_options_present {
                    let mut buf = [0; GzipCompressor::SIZE];
                    reader.read_exact(&mut buf)?;
                    Some(buf)
                } else {
                    None
                };
                Ok(Compressor::GZIP(GzipCompressor::new(opts)))
            }
            4 => {
                let opts = if compressor_options_present {
                    let mut buf = [0; XZCompressor::SIZE];
                    reader.read_exact(&mut buf)?;
                    Some(buf)
                } else {
                    None
                };
                Ok(Compressor::XZ(XZCompressor::new(opts)))
            }
            // 2 => Ok(Self::LZO),
            // 3 => Ok(Self::LZMA),
            // 5 => Ok(Self::LZ4),
            // 6 => Ok(Self::ZSTD),
            _ => todo!(),
        }
    }
}

impl Display for Compressor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GZIP(c) => Display::fmt(c, f),
            Self::XZ(c) => Display::fmt(c, f),
            _ => unimplemented!(),
        }
    }
}

impl Decompress for Compressor {
    fn decompress<R: ?Sized, W: ?Sized>(&self, reader: &mut R, writer: &mut W) -> Result<u64>
    where
        R: Read,
        W: Write,
    {
        match self {
            Compressor::GZIP(c) => Decompress::decompress(c, reader, writer),
            Compressor::XZ(c) => Decompress::decompress(c, reader, writer),
            Compressor::Undefined => unimplemented!(),
        }
    }
}

impl Default for Compressor {
    fn default() -> Self {
        // Self::ZSTD(Default::default())
        todo!()
    }
}

// impl TryFrom<u16> for Compressor {
//     type Error = Error;
//
//     fn try_from(value: u16) -> Result<Self, Self::Error> {
//         match value {
//             1 => Ok(Self::GZIP),
//             2 => Ok(Self::LZO),
//             3 => Ok(Self::LZMA),
//             4 => Ok(Self::XZ),
//             5 => Ok(Self::LZ4),
//             6 => Ok(Self::ZSTD),
//             _ => Err(Error::new(ErrorKind::Other, "bad compressor option")),
//         }
//     }
// }

bitflags! {
    pub struct XZFilters: u32 {
        // const X86 = 0x0001;
        // const POWER_PC = 0x0002;
        // const IA54 = 0x0004;
        // const ARM = 0x0008;
        // const ARM_THUMB = 0x0010;
        // const SPARC = 0x0020;
        const X86 = 0x0004;
        const POWER_PC = 0x0005;
        const IA64 = 0x0006;
        const ARM = 0x0007;
        const ARM_THUMB = 0x008;
        const SPARC = 0x009;
        const UNKNOWN = 0xffff;
    }
}

impl XZFilters {
    pub fn from_le_bytes(bytes: [u8; 4]) -> Self {
        unsafe { Self::from_bits_unchecked(u32::from_le_bytes(bytes)) }
    }

    pub fn to_le_bytes(&self) -> [u8; 4] {
        self.bits.to_le_bytes()
    }
}

impl Display for XZFilters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct XZCompressor {
    dictionary_size: [u8; 4],
    // dictionary_size: u32,
    filters: [u8; 4],
    // filters: XZFilters,
    // props
    // uncompressed_size
    // inner: [u8; 8],
}

impl XZCompressor {
    get_set_field!(dictionary_size, set_dictionary_size, u32);
    get_set_field!(filters, set_filters, XZFilters);

    const SIZE: usize = 8;

    fn new(bytes: Option<[u8; Self::SIZE]>) -> Self {
        // assert_eq!(Self::SIZE, mem::size_of::<Self>());
        let mut xzc = unsafe { mem::zeroed() };
        unsafe {
            let config_slice = slice::from_raw_parts_mut(&mut xzc as *mut _ as *mut u8, Self::SIZE);
            // reader.read_exact(config_slice)?;
            let bytes = bytes.unwrap_or_default();
            config_slice.copy_from_slice(&bytes);
        }
        xzc
    }
}

impl Decompress for XZCompressor {
    fn decompress<R: Read + ?Sized, W: Write + ?Sized>(
        &self,
        compressed: &mut R,
        decompressed: &mut W,
    ) -> Result<u64> {
        // TODO: check flags argument is filter
        let s = Stream::new_stream_decoder(1000000, 0)?;
        let mut decoder = XzDecoder::new_stream(compressed, s);
        copy(&mut decoder, decompressed)
    }
}

impl Display for XZCompressor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:02x} {}]", self.dictionary_size(), self.filters())
    }
}

#[derive(Clone, Debug)]
pub struct GzipCompressor([u8; 8]);

bitflags! {
    struct GzipStrategies: u16 {
        const DEFAULT = 0x0001;
        const FILTERED = 0x0002;
        const HUFFMAN_ONLY = 0x0004;
        const RUN_LENGTH_ENCODED = 0x0008;
        const FIXED = 0x0010;
    }
}

impl GzipCompressor {
    const SIZE: usize = 8;

    fn new(bytes: Option<[u8; Self::SIZE]>) -> Self {
        let bytes = bytes.unwrap_or([0; Self::SIZE]);
        Self(bytes)
    }

    get_set_field_tuple!(compression_level, set_compression_level, u32, 0, 4);
    get_set_field_tuple!(window_size, set_window_size, u32, 4, 4);
    get_set_field_tuple!(strategies, set_strategies, u16, 8, 2);
}

impl Decompress for GzipCompressor {
    fn decompress<R: Read + ?Sized, W: Write + ?Sized>(
        &self,
        compressed: &mut R,
        decompressed: &mut W,
    ) -> Result<u64> {
        let mut decoder = ZlibDecoder::new(compressed);
        // TODO: use decompress
        // let mut decoder = FlateDecompress::new(false);
        copy(&mut decoder, decompressed)
    }
}

impl Display for GzipCompressor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{} {} {}]",
            self.compression_level(),
            self.window_size(),
            self.strategies()
        )
    }
}
