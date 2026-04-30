use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::time::SystemTime;

use byteorder::{LittleEndian, WriteBytesExt};
use crc32fast::Hasher;

use crate::types::*;
use crate::error::Result;

/// Helper for building entries
pub struct EntryBuilder {
    pub entry_type: EntryType,
    pub compression: Compression,
    pub permissions: u16,
    pub path: String,
    pub name: String,
    pub data: Vec<u8>,
    pub modified_time: SystemTime,
}

/// Build a file entry
pub fn file_entry(
    path: &str,
    name: &str,
    data: Vec<u8>,
) -> EntryBuilder {
    EntryBuilder {
        entry_type: EntryType::File,
        compression: Compression::None,
        permissions: 0o644,
        path: path.to_string(),
        name: name.to_string(),
        data,
        modified_time: SystemTime::now(),
    }
}

/// Build a directory entry
pub fn dir_entry(path: &str, name: &str) -> EntryBuilder {
    EntryBuilder {
        entry_type: EntryType::Directory,
        compression: Compression::None,
        permissions: 0o755,
        path: path.to_string(),
        name: name.to_string(),
        data: Vec::new(),
        modified_time: SystemTime::now(),
    }
}

/// Convenience function to create a simple test archive
pub fn create_test_archive<P: AsRef<Path>>(path: P, entries: Vec<EntryBuilder>) -> Result<()> {
    let file = File::create(path)?;
    let mut writer = MMBakWriter::new(file);
    
    for entry in entries {
        writer.add_entry(entry);
    }
    
    writer.write()?;
    Ok(())
}

/// Builder for creating MMBak archive files
pub struct MMBakWriter<W: Write + Seek> {
    writer: W,
    entries: Vec<EntryBuilder>,
    metadata: Vec<(String, String)>,
    flags: ArchiveFlags,
}

impl<W: Write + Seek> MMBakWriter<W> {
    /// Create a new archive writer
    pub fn new(writer: W) -> Self {
        MMBakWriter {
            writer,
            entries: Vec::new(),
            metadata: Vec::new(),
            flags: ArchiveFlags::new(),
        }
    }

    /// Add an entry to the archive
    pub fn add_entry(&mut self, entry: EntryBuilder) {
        if entry.compression != Compression::None {
            self.flags = ArchiveFlags(self.flags.0 | 0x01);
        }
        self.entries.push(entry);
    }

    /// Add metadata key-value pair
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.push((key, value));
    }

    /// Set archive flags
    pub fn set_flags(&mut self, flags: ArchiveFlags) {
        self.flags = flags;
    }

    /// Calculate size of the entry table
    fn entry_table_size(&self) -> u64 {
        self.entries
            .iter()
            .map(|e| {
                ENTRY_DESC_SIZE as u64 + e.path.len() as u64 + e.name.len() as u64
            })
            .sum()
    }

    /// Calculate size of the metadata region
    fn metadata_size(&self) -> u64 {
        self.metadata
            .iter()
            .map(|(k, v)| 4u64 + k.len() as u64 + v.len() as u64)
            .sum()
    }

    /// Calculate the data offset (where actual file data begins)
    fn calculate_data_offset(&self) -> u64 {
        HEADER_SIZE as u64 + self.entry_table_size() + self.metadata_size()
    }

    /// Write the complete archive
    pub fn write(mut self) -> Result<()> {
        let data_offset = self.calculate_data_offset();

        self.write_header(data_offset)?;
        self.write_entries(data_offset)?;
        self.write_metadata()?;
        self.write_data(data_offset)?;
        self.write_footer()?;

        Ok(())
    }

    fn write_header(&mut self, data_offset: u64) -> Result<()> {
        // Calculate header checksum
        let mut hasher = Hasher::new();
        hasher.update(MAGIC_HEADER);

        let mut buf = [0u8; 20];
        buf[0..2].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        buf[2..4].copy_from_slice(&self.flags.0.to_le_bytes());
        buf[4..8].copy_from_slice(&(self.entries.len() as u32).to_le_bytes());
        buf[8..12].copy_from_slice(&(self.metadata.len() as u32).to_le_bytes());
        buf[12..20].copy_from_slice(&data_offset.to_le_bytes());
        hasher.update(&buf);
        let checksum = hasher.finalize();

        // Write header
        self.writer.write_all(MAGIC_HEADER)?;
        self.writer.write_u16::<LittleEndian>(FORMAT_VERSION)?;
        self.writer.write_u16::<LittleEndian>(self.flags.0)?;
        self.writer
            .write_u32::<LittleEndian>(self.entries.len() as u32)?;
        self.writer
            .write_u32::<LittleEndian>(self.metadata.len() as u32)?;
        self.writer.write_u64::<LittleEndian>(data_offset)?;
        self.writer.write_u32::<LittleEndian>(checksum)?;
        // Padding to reach HEADER_SIZE (32 bytes)
        self.writer.write_all(&[0u8; 4])?;

        Ok(())
    }

    fn write_entries(&mut self, initial_data_offset: u64) -> Result<()> {
        let mut data_offset = initial_data_offset;
        
        for i in 0..self.entries.len() {
            // Clone entry data to avoid borrow issues
            let entry = &self.entries[i];
            let entry_type = entry.entry_type;
            let compression = entry.compression;
            let permissions = entry.permissions;
            let path = entry.path.clone();
            let name = entry.name.clone();
            let data = entry.data.clone();
            let modified_time = entry.modified_time;
            
            let path_bytes = path.as_bytes();
            let name_bytes = name.as_bytes();
            let uncompressed_size = data.len() as u64;
            let compressed_size = uncompressed_size; // no compression for now
            
            let modified_timestamp = modified_time
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            
            // Calculate entry checksum
            let mut hasher = Hasher::new();
            hasher.update(&data);
            let checksum = hasher.finalize();
            
            // Write entry descriptor
            self.writer.write_u8(entry_type as u8)?;
            self.writer.write_u8(compression as u8)?;
            self.writer.write_u16::<LittleEndian>(permissions)?;
            self.writer.write_u16::<LittleEndian>(path_bytes.len() as u16)?;
            self.writer.write_u16::<LittleEndian>(name_bytes.len() as u16)?;
            self.writer.write_u64::<LittleEndian>(uncompressed_size)?;
            self.writer.write_u64::<LittleEndian>(compressed_size)?;
            self.writer.write_u64::<LittleEndian>(data_offset)?;
            self.writer.write_u64::<LittleEndian>(modified_timestamp)?;
            self.writer.write_u32::<LittleEndian>(checksum)?;
            self.writer.write_all(path_bytes)?;
            self.writer.write_all(name_bytes)?;
            
            // Advance data offset for next entry
            data_offset += Self::padded_size(data.len() as u64);
        }
        Ok(())
    }

    fn write_metadata(&mut self) -> Result<()> {
        for (key, value) in &self.metadata {
            let key_bytes = key.as_bytes();
            let val_bytes = value.as_bytes();

            self.writer
                .write_u16::<LittleEndian>(key_bytes.len() as u16)?;
            self.writer
                .write_u16::<LittleEndian>(val_bytes.len() as u16)?;
            self.writer.write_all(key_bytes)?;
            self.writer.write_all(val_bytes)?;
        }
        Ok(())
    }

    fn write_data(&mut self, data_offset: u64) -> Result<()> {
        self.writer.seek(SeekFrom::Start(data_offset))?;

        for entry in &self.entries {
            self.writer.write_all(&entry.data)?;
            // Pad to 4-byte boundary
            let padding = Self::padding(entry.data.len() as u64);
            for _ in 0..padding {
                self.writer.write_u8(0)?;
            }
        }

        Ok(())
    }

    fn write_footer(&mut self) -> Result<()> {
        let mut hasher = Hasher::new();
        for entry in &self.entries {
            hasher.update(&entry.data);
        }
        let total_checksum = hasher.finalize();

        self.writer
            .write_u32::<LittleEndian>(total_checksum)?;
        self.writer.write_all(MAGIC_FOOTER)?;
        self.writer.write_u64::<LittleEndian>(0)?; // reserved

        Ok(())
    }

    fn padded_size(size: u64) -> u64 {
        size + Self::padding(size)
    }

    fn padding(size: u64) -> u64 {
        (4 - (size % 4)) % 4
    }
}
