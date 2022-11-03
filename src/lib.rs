#![no_std]

#[cfg(feature = "builder")]
extern crate alloc;

use core::num::NonZeroUsize;

#[cfg(feature = "builder")]
pub use miniz_oxide::deflate::CompressionLevel;

mod num;
use num::*;

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct Header {
    magic: [u8; 16],
    name: [u8; 24],
    len: LittleEndianU64,
    next_file: LittleEndianU64,
}

unsafe impl bytemuck::AnyBitPattern for Header {}
unsafe impl bytemuck::Zeroable for Header {}
unsafe impl bytemuck::NoUninit for Header {}

impl Header {
    const MAGIC: [u8; 16] = *b"LINUIZARCHIVEV01";

    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name)
            .map(|name| name.trim_end_matches('\0'))
            .unwrap_or("Unknown")
    }

    pub fn len(&self) -> NonZeroUsize {
        // ### Safety: Value is known to be non-zero.
        unsafe { NonZeroUsize::new_unchecked(self.len.get() as usize) }
    }
}

impl core::fmt::Debug for Header {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Header")
            .field("Magic", &core::str::from_utf8(&self.magic))
            .field("Name", &self.name())
            .field("Length", &self.len().get())
            .field("Next File", &self.next_file.get())
            .finish()
    }
}

pub struct ArchiveReader<'a>(&'a [u8]);

impl<'a> ArchiveReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self(data)
    }
}

impl<'a> Iterator for ArchiveReader<'a> {
    type Item = (Header, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        use core::mem::size_of;

        if self.0.is_empty() {
            None
        } else {
            let header_bytes = &self.0[..size_of::<Header>()];
            self.0 = &self.0[size_of::<Header>()..];

            bytemuck::try_from_bytes::<Header>(header_bytes)
                .ok()
                .filter(|header| header.magic == Header::MAGIC)
                .map(|header| {
                    let section_data = &self.0[..(header.next_file.get() as usize)];
                    self.0 = &self.0[(header.next_file.get() as usize)..];

                    (*header, section_data)
                })
        }
    }
}

#[cfg(feature = "builder")]
#[derive(Debug)]
pub struct ArchiveBuilderError;

#[cfg(feature = "builder")]
pub struct ArchiveBuilder {
    data: alloc::vec::Vec<u8>,
    compression_level: CompressionLevel,
}

#[cfg(feature = "builder")]
impl ArchiveBuilder {
    pub fn new(compression_level: CompressionLevel) -> Self {
        Self {
            data: alloc::vec::Vec::new(),
            compression_level,
        }
    }

    pub fn push_data(
        &mut self,
        name: &str,
        data: &[u8],
    ) -> Result<(Header, alloc::vec::Vec<u8>), ArchiveBuilderError> {
        if name.len() > 24 {
            return Err(ArchiveBuilderError);
        }

        let name_bytes = {
            let mut bytes = [0u8; 24];
            name.bytes()
                .enumerate()
                .for_each(|(index, byte)| bytes[index] = byte);
            bytes
        };

        let compressed_bytes =
            miniz_oxide::deflate::compress_to_vec(data, self.compression_level as u8);
        let header = Header {
            magic: Header::MAGIC,
            name: name_bytes,
            len: LittleEndianU64::new(data.len() as u64),
            next_file: LittleEndianU64::new(compressed_bytes.len() as u64),
        };
        let header_bytes = bytemuck::bytes_of(&header);

        self.data.extend(header_bytes);
        self.data.extend(compressed_bytes.iter());

        Ok((header, compressed_bytes))
    }

    pub fn take_data(self) -> alloc::vec::Vec<u8> {
        self.data
    }
}
