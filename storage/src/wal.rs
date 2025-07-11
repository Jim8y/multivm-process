//! Write-ahead log implementation

use crate::{Result, StorageError};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use tokio::sync::Mutex;
use tracing::{debug, info};

/// WAL entry
#[derive(Debug)]
pub struct WalEntry {
    pub sequence: u64,
    pub data: Vec<u8>,
}

/// Write-ahead log
pub struct WriteAheadLog {
    path: PathBuf,
    writer: Mutex<BufWriter<File>>,
    sequence: Mutex<u64>,
}

impl WriteAheadLog {
    /// Create new WAL
    pub fn new(dir: &Path, name: &str) -> Result<Self> {
        let path = dir.join(format!("{}.wal", name));
        
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
            
        let writer = BufWriter::new(file);
        
        // Read last sequence number
        let sequence = Self::read_last_sequence(&path)?;
        
        info!(path = %path.display(), sequence, "Opened WAL");
        
        Ok(Self {
            path,
            writer: Mutex::new(writer),
            sequence: Mutex::new(sequence),
        })
    }
    
    /// Append entry to WAL
    pub async fn append(&self, data: &[u8]) -> Result<u64> {
        let mut writer = self.writer.lock().await;
        let mut sequence = self.sequence.lock().await;
        
        *sequence += 1;
        let entry_sequence = *sequence;
        
        // Write header: sequence (8 bytes) + length (4 bytes)
        writer.write_all(&entry_sequence.to_le_bytes())?;
        writer.write_all(&(data.len() as u32).to_le_bytes())?;
        
        // Write data
        writer.write_all(data)?;
        
        // Write checksum
        let checksum = crc32fast::hash(data);
        writer.write_all(&checksum.to_le_bytes())?;
        
        writer.flush()?;
        
        debug!(sequence = entry_sequence, size = data.len(), "Appended to WAL");
        
        Ok(entry_sequence)
    }
    
    /// Sync WAL to disk
    pub async fn sync(&self) -> Result<()> {
        let mut writer = self.writer.lock().await;
        writer.flush()?;
        writer.get_mut().sync_all()?;
        Ok(())
    }
    
    /// Read all entries from WAL
    pub fn read_all(&self) -> Result<Vec<WalEntry>> {
        Self::read_entries_from_file(&self.path)
    }
    
    /// Read entries from a file
    fn read_entries_from_file(path: &Path) -> Result<Vec<WalEntry>> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut entries = Vec::new();
        
        loop {
            // Read header
            let mut header = [0u8; 12];
            match reader.read_exact(&mut header) {
                Ok(_) => {},
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
            
            let sequence = u64::from_le_bytes(header[0..8].try_into().unwrap());
            let length = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
            
            // Read data
            let mut data = vec![0u8; length];
            reader.read_exact(&mut data)?;
            
            // Read and verify checksum
            let mut checksum_bytes = [0u8; 4];
            reader.read_exact(&mut checksum_bytes)?;
            let stored_checksum = u32::from_le_bytes(checksum_bytes);
            let computed_checksum = crc32fast::hash(&data);
            
            if stored_checksum != computed_checksum {
                return Err(StorageError::Corruption(format!(
                    "WAL checksum mismatch at sequence {}",
                    sequence
                )));
            }
            
            entries.push(WalEntry { sequence, data });
        }
        
        Ok(entries)
    }
    
    /// Truncate WAL after recovery
    pub async fn truncate(&self) -> Result<()> {
        // Close current writer
        let mut writer = self.writer.lock().await;
        writer.flush()?;
        drop(writer);
        
        // Truncate file
        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)?;
            
        // Reopen for append
        let file = OpenOptions::new()
            .append(true)
            .open(&self.path)?;
            
        *self.writer.lock().await = BufWriter::new(file);
        *self.sequence.lock().await = 0;
        
        info!("Truncated WAL");
        Ok(())
    }
    
    /// Read last sequence number
    fn read_last_sequence(path: &Path) -> Result<u64> {
        if !path.exists() {
            return Ok(0);
        }
        
        // Read entries directly without creating a full WAL instance
        let entries = Self::read_entries_from_file(path)?;
        
        Ok(entries.last().map(|e| e.sequence).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_wal_operations() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::new(temp_dir.path(), "test").unwrap();
        
        // Append entries
        let seq1 = wal.append(b"entry1").await.unwrap();
        let seq2 = wal.append(b"entry2").await.unwrap();
        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        
        // Sync
        wal.sync().await.unwrap();
        
        // Read back
        let entries = wal.read_all().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sequence, 1);
        assert_eq!(entries[0].data, b"entry1");
        assert_eq!(entries[1].sequence, 2);
        assert_eq!(entries[1].data, b"entry2");
        
        // Truncate
        wal.truncate().await.unwrap();
        let entries = wal.read_all().unwrap();
        assert_eq!(entries.len(), 0);
    }
}