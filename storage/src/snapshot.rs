//! Snapshot management

use crate::{Result, StorageError};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Snapshot metadata
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SnapshotMetadata {
    pub index: u64,
    pub term: u64,
    pub size: u64,
    pub checksum: u32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Snapshot manager
pub struct SnapshotManager {
    dir: PathBuf,
}

impl SnapshotManager {
    /// Create new snapshot manager
    pub fn new(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }
    
    /// Save a snapshot
    pub async fn save(
        &self,
        index: u64,
        term: u64,
        data: &[u8],
    ) -> Result<SnapshotMetadata> {
        let filename = format!("snapshot-{:020}-{:020}.snap", index, term);
        let path = self.dir.join(&filename);
        let temp_path = self.dir.join(format!("{}.tmp", filename));
        
        // Write to temporary file first
        let mut file = BufWriter::new(File::create(&temp_path)?);
        
        // Write metadata
        let metadata = SnapshotMetadata {
            index,
            term,
            size: data.len() as u64,
            checksum: crc32fast::hash(data),
            created_at: chrono::Utc::now(),
        };
        
        let metadata_bytes = bincode::serialize(&metadata)?;
        file.write_all(&(metadata_bytes.len() as u32).to_le_bytes())?;
        file.write_all(&metadata_bytes)?;
        
        // Write data
        file.write_all(data)?;
        file.flush()?;
        file.get_ref().sync_all()?;
        
        // Atomic rename
        fs::rename(&temp_path, &path)?;
        
        info!(
            index,
            term,
            size = metadata.size,
            path = %path.display(),
            "Saved snapshot"
        );
        
        Ok(metadata)
    }
    
    /// Load latest snapshot
    pub async fn load_latest(&self) -> Result<Option<(SnapshotMetadata, Vec<u8>)>> {
        let mut snapshots = self.list_snapshots()?;
        if snapshots.is_empty() {
            return Ok(None);
        }
        
        // Sort by index descending
        snapshots.sort_by_key(|s| std::cmp::Reverse(s.index));
        
        // Try to load the latest valid snapshot
        for snapshot in snapshots {
            match self.load(snapshot.index, snapshot.term).await {
                Ok(result) => return Ok(Some(result)),
                Err(e) => {
                    warn!(
                        index = snapshot.index,
                        term = snapshot.term,
                        error = %e,
                        "Failed to load snapshot, trying next"
                    );
                }
            }
        }
        
        Ok(None)
    }
    
    /// Load specific snapshot
    pub async fn load(
        &self,
        index: u64,
        term: u64,
    ) -> Result<(SnapshotMetadata, Vec<u8>)> {
        let filename = format!("snapshot-{:020}-{:020}.snap", index, term);
        let path = self.dir.join(&filename);
        
        let mut file = BufReader::new(File::open(&path)?);
        
        // Read metadata length
        let mut len_bytes = [0u8; 4];
        file.read_exact(&mut len_bytes)?;
        let metadata_len = u32::from_le_bytes(len_bytes) as usize;
        
        // Read metadata
        let mut metadata_bytes = vec![0u8; metadata_len];
        file.read_exact(&mut metadata_bytes)?;
        let metadata: SnapshotMetadata = bincode::deserialize(&metadata_bytes)?;
        
        // Read data
        let mut data = vec![0u8; metadata.size as usize];
        file.read_exact(&mut data)?;
        
        // Verify checksum
        let checksum = crc32fast::hash(&data);
        if checksum != metadata.checksum {
            return Err(StorageError::Corruption(format!(
                "Snapshot checksum mismatch: expected {}, got {}",
                metadata.checksum, checksum
            )));
        }
        
        Ok((metadata, data))
    }
    
    /// List all snapshots
    pub fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>> {
        let mut snapshots = Vec::new();
        
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("snap") {
                // Parse metadata from filename
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some((index_str, term_str)) = name
                        .strip_prefix("snapshot-")
                        .and_then(|s| s.split_once('-'))
                    {
                        if let (Ok(index), Ok(term)) = 
                            (index_str.parse::<u64>(), term_str.parse::<u64>()) 
                        {
                            // Load metadata
                            match self.load_metadata(&path) {
                                Ok(metadata) => snapshots.push(metadata),
                                Err(e) => {
                                    warn!(
                                        path = %path.display(),
                                        error = %e,
                                        "Failed to load snapshot metadata"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(snapshots)
    }
    
    /// Clean up old snapshots
    pub async fn cleanup(&self, keep: usize) -> Result<()> {
        let mut snapshots = self.list_snapshots()?;
        
        if snapshots.len() <= keep {
            return Ok(());
        }
        
        // Sort by index descending
        snapshots.sort_by_key(|s| std::cmp::Reverse(s.index));
        
        // Remove old snapshots
        for snapshot in &snapshots[keep..] {
            let filename = format!(
                "snapshot-{:020}-{:020}.snap",
                snapshot.index,
                snapshot.term
            );
            let path = self.dir.join(&filename);
            
            if let Err(e) = fs::remove_file(&path) {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "Failed to remove old snapshot"
                );
            } else {
                info!(
                    index = snapshot.index,
                    term = snapshot.term,
                    "Removed old snapshot"
                );
            }
        }
        
        Ok(())
    }
    
    /// Load metadata from snapshot file
    fn load_metadata(&self, path: &Path) -> Result<SnapshotMetadata> {
        let mut file = BufReader::new(File::open(path)?);
        
        // Read metadata length
        let mut len_bytes = [0u8; 4];
        file.read_exact(&mut len_bytes)?;
        let metadata_len = u32::from_le_bytes(len_bytes) as usize;
        
        // Read metadata
        let mut metadata_bytes = vec![0u8; metadata_len];
        file.read_exact(&mut metadata_bytes)?;
        
        Ok(bincode::deserialize(&metadata_bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_snapshot_operations() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SnapshotManager::new(temp_dir.path().join("snapshots")).unwrap();
        
        // Save snapshots
        let data1 = b"snapshot data 1";
        let meta1 = manager.save(10, 1, data1).await.unwrap();
        assert_eq!(meta1.index, 10);
        assert_eq!(meta1.term, 1);
        
        let data2 = b"snapshot data 2";
        let meta2 = manager.save(20, 2, data2).await.unwrap();
        
        // List snapshots
        let snapshots = manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 2);
        
        // Load latest
        let (metadata, data) = manager.load_latest().await.unwrap().unwrap();
        assert_eq!(metadata.index, 20);
        assert_eq!(data, data2);
        
        // Load specific
        let (metadata, data) = manager.load(10, 1).await.unwrap();
        assert_eq!(metadata.index, 10);
        assert_eq!(data, data1);
        
        // Cleanup
        manager.cleanup(1).await.unwrap();
        let snapshots = manager.list_snapshots().unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].index, 20);
    }
}