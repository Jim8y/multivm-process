//! Storage recovery and backup mechanisms

use crate::{StorageError, Result};
use std::path::{Path, PathBuf};
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

/// Backup metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// Backup timestamp
    pub timestamp: DateTime<Utc>,
    /// Original data directory path
    pub source_path: PathBuf,
    /// Backup version
    pub version: u32,
    /// Checksums of backed up files
    pub file_checksums: std::collections::HashMap<String, String>,
    /// Total size in bytes
    pub total_size: u64,
    /// Compression used
    pub compression: CompressionType,
}

/// Compression types supported
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CompressionType {
    None,
    Gzip,
    Lz4,
}

/// Recovery manager for storage
pub struct RecoveryManager {
    /// Data directory
    data_dir: PathBuf,
    /// Backup directory
    backup_dir: PathBuf,
    /// WAL directory
    wal_dir: PathBuf,
}

impl RecoveryManager {
    /// Create new recovery manager
    pub fn new(data_dir: PathBuf) -> Self {
        let backup_dir = data_dir.join("backups");
        let wal_dir = data_dir.join("wal");
        
        Self {
            data_dir,
            backup_dir,
            wal_dir,
        }
    }

    /// Create a full backup
    pub async fn create_backup(&self, name: Option<String>) -> Result<PathBuf> {
        let backup_name = name.unwrap_or_else(|| {
            format!("bkp_{}", Utc::now().format("%m%d_%H%M"))
        });
        
        let backup_path = self.backup_dir.join(&backup_name);
        
        info!("Creating backup: {}", backup_path.display());
        
        // Create backup directory
        tokio::fs::create_dir_all(&backup_path)
            .await
            .map_err(|e| StorageError::Io(e))?;

        let mut file_checksums = std::collections::HashMap::new();
        let mut total_size = 0u64;

        // For simplicity, just copy files in data_dir directly (not recursive)
        if self.data_dir.exists() {
            let mut entries = tokio::fs::read_dir(&self.data_dir)
                .await
                .map_err(|e| StorageError::Io(e))?;

            while let Some(entry) = entries.next_entry()
                .await
                .map_err(|e| StorageError::Io(e))?
            {
                let path = entry.path();
                if path.is_file() {
                    let filename = path.file_name().unwrap();
                    let dest_path = backup_path.join(filename);
                    
                    tokio::fs::copy(&path, &dest_path)
                        .await
                        .map_err(|e| StorageError::Io(e))?;

                    let checksum = calculate_file_checksum(&path).await?;
                    file_checksums.insert(filename.to_string_lossy().to_string(), checksum);

                    let metadata = tokio::fs::metadata(&path)
                        .await
                        .map_err(|e| StorageError::Io(e))?;
                    total_size += metadata.len();
                }
            }
        }

        // Create backup metadata
        let metadata = BackupMetadata {
            timestamp: Utc::now(),
            source_path: self.data_dir.clone(),
            version: 1,
            file_checksums,
            total_size,
            compression: CompressionType::None,
        };

        // Save metadata
        let metadata_path = backup_path.join("backup_metadata.json");
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| StorageError::Internal(format!("JSON serialization failed: {}", e)))?;
        tokio::fs::write(&metadata_path, metadata_json)
            .await
            .map_err(|e| StorageError::Io(e))?;

        info!("Backup created successfully: {} ({} bytes)", 
              backup_path.display(), total_size);

        Ok(backup_path)
    }

    /// Restore from backup
    pub async fn restore_from_backup(&self, backup_path: &Path) -> Result<()> {
        info!("Restoring from backup: {}", backup_path.display());

        // Load backup metadata
        let metadata_path = backup_path.join("backup_metadata.json");
        debug!("Loading metadata from: {}", metadata_path.display());
        let metadata_content = tokio::fs::read_to_string(&metadata_path)
            .await
            .map_err(|e| StorageError::Io(e))?;
        let metadata: BackupMetadata = serde_json::from_str(&metadata_content)
            .map_err(|e| StorageError::Internal(format!("JSON deserialization failed: {}", e)))?;

        debug!("Metadata loaded, {} files to restore", metadata.file_checksums.len());
        for filename in metadata.file_checksums.keys() {
            debug!("File in backup: {}", filename);
        }

        // Verify backup integrity
        debug!("Verifying backup integrity");
        self.verify_backup_integrity(backup_path, &metadata).await?;

        // Create data directory if it doesn't exist
        debug!("Creating data directory: {}", self.data_dir.display());
        tokio::fs::create_dir_all(&self.data_dir)
            .await
            .map_err(|e| StorageError::Io(e))?;

        // Directly restore files (simpler approach)
        debug!("Starting file restore");
        self.restore_files(backup_path, &metadata).await?;
        info!("Restore completed successfully");

        Ok(())
    }

    /// Verify backup integrity
    async fn verify_backup_integrity(
        &self,
        backup_path: &Path,
        metadata: &BackupMetadata,
    ) -> Result<()> {
        debug!("Verifying backup integrity");

        for (relative_path, expected_checksum) in &metadata.file_checksums {
            let file_path = backup_path.join(relative_path);
            
            if !file_path.exists() {
                return Err(StorageError::Corruption(
                    format!("Missing file in backup: {}", relative_path)
                ));
            }

            let actual_checksum = calculate_file_checksum(&file_path).await?;
            if actual_checksum != *expected_checksum {
                return Err(StorageError::Corruption(
                    format!("Checksum mismatch for file: {}", relative_path)
                ));
            }
        }

        debug!("Backup integrity verified");
        Ok(())
    }


    /// Restore files from backup
    async fn restore_files(
        &self,
        backup_path: &Path,
        metadata: &BackupMetadata,
    ) -> Result<()> {
        // Clear current data directory
        if self.data_dir.exists() {
            tokio::fs::remove_dir_all(&self.data_dir)
                .await
                .map_err(|e| StorageError::Io(e))?;
        }

        tokio::fs::create_dir_all(&self.data_dir)
            .await
            .map_err(|e| StorageError::Io(e))?;

        // Copy files from backup (simplified - files only, no directories)
        for filename in metadata.file_checksums.keys() {
            let src_path = backup_path.join(filename);
            let dest_path = self.data_dir.join(filename);

            debug!("Restoring file: {} -> {}", src_path.display(), dest_path.display());
            tokio::fs::copy(&src_path, &dest_path)
                .await
                .map_err(|e| StorageError::Io(e))?;
        }

        Ok(())
    }

    /// List available backups
    pub async fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        let mut backups = Vec::new();

        if !self.backup_dir.exists() {
            return Ok(backups);
        }

        let mut entries = tokio::fs::read_dir(&self.backup_dir)
            .await
            .map_err(|e| StorageError::Io(e))?;

        while let Some(entry) = entries.next_entry()
            .await
            .map_err(|e| StorageError::Io(e))?
        {
            if entry.file_type().await.map_err(|e| StorageError::Io(e))?.is_dir() {
                let backup_path = entry.path();
                let metadata_path = backup_path.join("backup_metadata.json");

                if metadata_path.exists() {
                    match load_backup_metadata(&metadata_path).await {
                        Ok(metadata) => {
                            backups.push(BackupInfo {
                                name: backup_path.file_name()
                                    .unwrap()
                                    .to_string_lossy()
                                    .to_string(),
                                path: backup_path,
                                metadata,
                            });
                        }
                        Err(e) => {
                            warn!("Failed to load backup metadata for {}: {}", 
                                  backup_path.display(), e);
                        }
                    }
                }
            }
        }

        // Sort by timestamp (newest first)
        backups.sort_by(|a, b| b.metadata.timestamp.cmp(&a.metadata.timestamp));

        Ok(backups)
    }

    /// Delete old backups (keep only the specified number)
    pub async fn cleanup_old_backups(&self, keep_count: usize) -> Result<()> {
        let backups = self.list_backups().await?;

        if backups.len() <= keep_count {
            return Ok(());
        }

        for backup in backups.into_iter().skip(keep_count) {
            info!("Removing old backup: {}", backup.path.display());
            tokio::fs::remove_dir_all(&backup.path)
                .await
                .map_err(|e| StorageError::Io(e))?;
        }

        Ok(())
    }

    /// Recover from WAL after crash
    pub async fn recover_from_wal(&self) -> Result<Vec<WalEntry>> {
        info!("Recovering from WAL");

        let mut recovered_entries = Vec::new();

        if !self.wal_dir.exists() {
            debug!("No WAL directory found, nothing to recover");
            return Ok(recovered_entries);
        }

        let mut wal_files = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.wal_dir)
            .await
            .map_err(|e| StorageError::Io(e))?;

        while let Some(entry) = entries.next_entry()
            .await
            .map_err(|e| StorageError::Io(e))?
        {
            if entry.file_name().to_string_lossy().ends_with(".wal") {
                wal_files.push(entry.path());
            }
        }

        // Sort WAL files by name (which should be timestamp-based)
        wal_files.sort();

        for wal_file in wal_files {
            debug!("Recovering from WAL file: {}", wal_file.display());
            match self.recover_from_wal_file(&wal_file).await {
                Ok(mut entries) => {
                    recovered_entries.append(&mut entries);
                }
                Err(e) => {
                    error!("Failed to recover from WAL file {}: {}", 
                           wal_file.display(), e);
                    // Continue with other files
                }
            }
        }

        info!("Recovered {} entries from WAL", recovered_entries.len());
        Ok(recovered_entries)
    }

    /// Recover from a single WAL file
    async fn recover_from_wal_file(&self, wal_file: &Path) -> Result<Vec<WalEntry>> {
        let content = tokio::fs::read(wal_file)
            .await
            .map_err(|e| StorageError::Io(e))?;

        let mut entries = Vec::new();
        let mut offset = 0;

        while offset < content.len() {
            // Read entry length
            if offset + 4 > content.len() {
                break;
            }

            let entry_len = u32::from_le_bytes([
                content[offset],
                content[offset + 1],
                content[offset + 2],
                content[offset + 3],
            ]) as usize;

            offset += 4;

            if offset + entry_len > content.len() {
                warn!("Truncated WAL entry found, stopping recovery from this file");
                break;
            }

            // Read entry data
            let entry_data = &content[offset..offset + entry_len];
            match bincode::deserialize::<WalEntry>(entry_data) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    warn!("Failed to deserialize WAL entry: {}", e);
                    break;
                }
            }

            offset += entry_len;
        }

        Ok(entries)
    }
}

/// Information about a backup
#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub name: String,
    pub path: PathBuf,
    pub metadata: BackupMetadata,
}

/// WAL entry for recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub operation: WalOperation,
}

/// WAL operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalOperation {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
    Clear,
}

/// Calculate SHA-256 checksum of a file
async fn calculate_file_checksum(path: &Path) -> Result<String> {
    use sha2::{Sha256, Digest};
    
    let content = tokio::fs::read(path)
        .await
        .map_err(|e| StorageError::Io(e))?;
    
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let hash = hasher.finalize();
    
    Ok(format!("{:x}", hash))
}

/// Calculate checksums recursively for all files in a directory
fn calculate_checksums_recursive<'a>(
    dir_path: &'a Path, 
    base_path: &'a Path,
    checksums: &'a mut std::collections::HashMap<String, String>,
    total_size: &'a mut u64
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = tokio::fs::read_dir(dir_path)
            .await
            .map_err(|e| StorageError::Io(e))?;

        while let Some(entry) = entries.next_entry()
            .await
            .map_err(|e| StorageError::Io(e))?
        {
            let path = entry.path();
            if path.is_file() {
                let relative_path = path.strip_prefix(base_path)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                let checksum = calculate_file_checksum(&path).await?;
                checksums.insert(relative_path, checksum);

                let metadata = tokio::fs::metadata(&path)
                    .await
                    .map_err(|e| StorageError::Io(e))?;
                *total_size += metadata.len();
            } else if path.is_dir() {
                calculate_checksums_recursive(&path, base_path, checksums, total_size).await?;
            }
        }

        Ok(())
    })
}

/// Load backup metadata from file
async fn load_backup_metadata(path: &Path) -> Result<BackupMetadata> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| StorageError::Io(e))?;
    
    serde_json::from_str(&content)
        .map_err(|e| StorageError::Internal(format!("JSON deserialization failed: {}", e)))
}

/// Recursively copy directory
fn copy_dir_recursive<'a>(src: &'a Path, dest: &'a Path) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
    tokio::fs::create_dir_all(dest)
        .await
        .map_err(|e| StorageError::Io(e))?;

    let mut entries = tokio::fs::read_dir(src)
        .await
        .map_err(|e| StorageError::Io(e))?;

    while let Some(entry) = entries.next_entry()
        .await
        .map_err(|e| StorageError::Io(e))?
    {
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path).await?;
        } else {
            tokio::fs::copy(&src_path, &dest_path)
                .await
                .map_err(|e| StorageError::Io(e))?;
        }
    }

    Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_backup_and_restore() {
        // Use a very simple path to avoid filesystem limits
        let base_dir = std::path::PathBuf::from("/tmp/test_recovery");
        if base_dir.exists() {
            let _ = std::fs::remove_dir_all(&base_dir);
        }
        
        let data_dir = base_dir.join("data");
        let recovery = RecoveryManager::new(data_dir.clone());

        // Create some test data
        tokio::fs::create_dir_all(&data_dir).await.unwrap();
        tokio::fs::write(data_dir.join("test.txt"), b"test data").await.unwrap();

        // Create backup with short name
        let backup_path = recovery.create_backup(Some("bk".to_string())).await.unwrap();
        assert!(backup_path.exists());
        
        // Debug: check what's in the backup
        println!("Backup path: {:?}", backup_path);
        let backup_files = std::fs::read_dir(&backup_path).unwrap();
        for file in backup_files {
            println!("Backup file: {:?}", file.unwrap().path());
        }

        // Modify original data
        tokio::fs::write(data_dir.join("test.txt"), b"modified data").await.unwrap();

        // Restore from backup
        recovery.restore_from_backup(&backup_path).await.expect("Failed to restore from backup");

        // Verify restoration
        let content = tokio::fs::read_to_string(data_dir.join("test.txt")).await.unwrap();
        assert_eq!(content, "test data");
        
        // Cleanup
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[tokio::test]
    async fn test_backup_list() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        let recovery = RecoveryManager::new(data_dir.clone());

        // Create some test data
        tokio::fs::create_dir_all(&data_dir).await.unwrap();
        tokio::fs::write(data_dir.join("test.txt"), b"test data").await.unwrap();

        // Create multiple backups
        recovery.create_backup(Some("backup1".to_string())).await.unwrap();
        recovery.create_backup(Some("backup2".to_string())).await.unwrap();

        // List backups
        let backups = recovery.list_backups().await.unwrap();
        assert_eq!(backups.len(), 2);
    }
}