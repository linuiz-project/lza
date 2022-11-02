#![no_std]

#[cfg(feature = "writer")]
extern crate alloc;

mod num;
use num::*;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Header {
    magic: [u8; 16],
    name: [u8; 24],
    pub len: LittleEndianU64,
    pub next_file: LittleEndianU64,
}

unsafe impl bytemuck::AnyBitPattern for Header {}
unsafe impl bytemuck::Zeroable for Header {}
unsafe impl bytemuck::NoUninit for Header {}

impl Header {
    const MAGIC: [u8; 16] = *b"LINUIZARCHIVEV01";

    pub fn name(&self) -> Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(&self.name)
    }
}

pub struct ArchiveReader<'a> {
    data: &'a [u8],
}

impl<'a> Iterator for ArchiveReader<'a> {
    type Item = (Header, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        use core::mem::size_of;

        if self.data.is_empty() {
            None
        } else {
            let header_bytes = &self.data[..size_of::<Header>()];
            self.data = &self.data[size_of::<Header>()..];

            bytemuck::try_from_bytes::<Header>(header_bytes)
                .ok()
                .filter(|header| header.magic == Header::MAGIC)
                .map(|header| {
                    let section_data = &self.data[..(header.next_file.get() as usize)];
                    self.data = &self.data[(header.next_file.get() as usize)..];

                    (*header, section_data)
                })
        }
    }
}

#[cfg(feature = "writer")]
pub struct ArchiveWriterError;

#[cfg(feature = "writer")]
pub struct ArchiveBuilder {
    data: alloc::vec::Vec<u8>,
    compression_level: miniz_oxide::deflate::CompressionLevel,
}

#[cfg(feature = "writer")]
impl ArchiveBuilder {
    pub fn new(compression_level: miniz_oxide::deflate::CompressionLevel) -> Self {
        Self {
            data: alloc::vec::Vec::new(),
            compression_level,
        }
    }

    pub fn push_data(&mut self, name: &str, data: &[u8]) -> Result<(), ArchiveWriterError> {
        if name.len() > 24 {
            return Err(ArchiveWriterError);
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

        self.data.extend(header_bytes.iter());
        self.data.extend(compressed_bytes.iter());

        Ok(())
    }

    pub fn take_data(self) -> alloc::vec::Vec<u8> {
        self.data
    }
}
