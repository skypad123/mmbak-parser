use std::io::Cursor;

use crate::*;
use crate::writer::{dir_entry, file_entry, MMBakWriter};

#[test]
fn test_header_parsing() {
    // Create a minimal valid archive in memory
    let mut buf = Vec::new();
    {
        let writer = MMBakWriter::new(Cursor::new(&mut buf));
        let mut writer = writer;
        writer.add_entry(file_entry("", "test.txt", b"hello world".to_vec()));
        writer.write().unwrap();
    }

    // Parse it back
    let cursor = Cursor::new(&buf);
    let archive = MMBakFile::from_reader(cursor, buf.len() as u64).unwrap();

    let header = archive.header();
    assert_eq!(header.version, 1);
    assert_eq!(header.entry_count, 1);
}

#[test]
fn test_entry_types() {
    let mut buf = Vec::new();
    {
        let writer = MMBakWriter::new(Cursor::new(&mut buf));
        let mut writer = writer;
        writer.add_entry(dir_entry("", "mydir"));
        writer.add_entry(file_entry("mydir", "file.txt", b"content".to_vec()));
        writer.write().unwrap();
    }

    let cursor = Cursor::new(&buf);
    let archive = MMBakFile::from_reader(cursor, buf.len() as u64).unwrap();

    assert_eq!(archive.entries().len(), 2);
    assert!(archive.entries()[0].is_dir());
    assert!(archive.entries()[1].is_file());
    assert_eq!(archive.entries()[0].name, "mydir");
    assert_eq!(archive.entries()[1].name, "file.txt");
}

#[test]
fn test_metadata() {
    let mut buf = Vec::new();
    {
        let writer = MMBakWriter::new(Cursor::new(&mut buf));
        let mut writer = writer;
        writer.add_metadata("author".to_string(), "test".to_string());
        writer.add_metadata("version".to_string(), "1.0".to_string());
        writer.add_entry(file_entry("", "a.txt", vec![0u8; 100]));
        writer.write().unwrap();
    }

    let cursor = Cursor::new(&buf);
    let archive = MMBakFile::from_reader(cursor, buf.len() as u64).unwrap();

    assert_eq!(archive.metadata().len(), 2);
    assert_eq!(archive.metadata()[0].key, "author");
    assert_eq!(archive.metadata()[0].value, "test");
    assert_eq!(archive.metadata()[1].key, "version");
    assert_eq!(archive.metadata()[1].value, "1.0");
}

#[test]
fn test_find_entry() {
    let mut buf = Vec::new();
    {
        let writer = MMBakWriter::new(Cursor::new(&mut buf));
        let mut writer = writer;
        writer.add_entry(file_entry("config", "app.json", b"{}".to_vec()));
        writer.add_entry(file_entry("data", "users.db", vec![1u8; 50]));
        writer.write().unwrap();
    }

    let cursor = Cursor::new(&buf);
    let archive = MMBakFile::from_reader(cursor, buf.len() as u64).unwrap();

    assert!(archive.find_entry("config/app.json").is_some());
    assert!(archive.find_entry("data/users.db").is_some());
    assert!(archive.find_entry("nonexistent").is_none());
}

#[test]
fn test_size_calculations() {
    let mut buf = Vec::new();
    {
        let writer = MMBakWriter::new(Cursor::new(&mut buf));
        let mut writer = writer;
        writer.add_entry(file_entry("", "small.txt", vec![0u8; 100]));
        writer.add_entry(file_entry("", "large.txt", vec![0u8; 1000]));
        writer.write().unwrap();
    }

    let cursor = Cursor::new(&buf);
    let archive = MMBakFile::from_reader(cursor, buf.len() as u64).unwrap();

    assert_eq!(archive.total_uncompressed_size(), 1100);
    assert_eq!(archive.files().count(), 2);
}

#[test]
fn test_invalid_magic() {
    let data = b"XXXXinvalid data";
    let cursor = Cursor::new(data.as_slice());
    let result = MMBakFile::from_reader(cursor, data.len() as u64);

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Invalid magic bytes"));
}

#[test]
fn test_empty_archive() {
    let mut buf = Vec::new();
    {
        let writer = MMBakWriter::new(Cursor::new(&mut buf));
        writer.write().unwrap();
    }

    let cursor = Cursor::new(&buf);
    let archive = MMBakFile::from_reader(cursor, buf.len() as u64).unwrap();

    assert_eq!(archive.entries().len(), 0);
    assert_eq!(archive.metadata().len(), 0);
    assert_eq!(archive.total_uncompressed_size(), 0);
}

#[test]
fn test_entry_full_path() {
    let entry = MMBakEntry {
        entry_type: EntryType::File,
        compression: Compression::None,
        permissions: 0o644,
        path: "config/app".to_string(),
        name: "settings.json".to_string(),
        uncompressed_size: 42,
        compressed_size: 42,
        data_offset: 0,
        modified_time: std::time::SystemTime::now(),
        checksum: 0,
    };

    assert_eq!(entry.full_path(), "config/app/settings.json");

    let root_entry = MMBakEntry {
        entry_type: EntryType::File,
        compression: Compression::None,
        permissions: 0o644,
        path: "".to_string(),
        name: "README.md".to_string(),
        uncompressed_size: 100,
        compressed_size: 100,
        data_offset: 0,
        modified_time: std::time::SystemTime::now(),
        checksum: 0,
    };

    assert_eq!(root_entry.full_path(), "README.md");
}

#[test]
fn test_format_size() {
    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(512), "512 B");
    assert_eq!(format_size(1024), "1.00 KB");
    assert_eq!(format_size(1536), "1.50 KB");
    assert_eq!(format_size(1024 * 1024), "1.00 MB");
    assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
}
