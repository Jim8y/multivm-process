//! RocksDB-based storage implementation

use crate::{error::Result, StorageConfig, StorageError};
use async_trait::async_trait;
use consensus::{
    storage::{LogEntry, Storage},
    NodeId,
};
use rocksdb::{
    ColumnFamilyDescriptor, DBCompressionType, Direction, IteratorMode, Options, WriteBatch, DB,
};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info};

const CF_DEFAULT: &str = "default";
const CF_LOG: &str = "log";
const CF_SNAPSHOT: &str = "snapshot";
const CF_META: &str = "meta";

const KEY_CURRENT_TERM: &[u8] = b"current_term";
const KEY_VOTED_FOR: &[u8] = b"voted_for";
const KEY_LAST_APPLIED: &[u8] = b"last_applied";
const KEY_COMMIT_INDEX: &[u8] = b"commit_index";

/// RocksDB storage implementation
#[derive(Debug)]
pub struct RocksDbStorage {
    db: Arc<DB>,
    config: StorageConfig,
}

impl RocksDbStorage {
    /// Create new RocksDB storage
    pub async fn new(config: StorageConfig) -> Result<Self> {
        // Ensure data directory exists
        tokio::fs::create_dir_all(&config.data_dir).await?;
        
        let db = Arc::new(Self::open_db(&config.data_dir)?);
        
        info!(
            data_dir = %config.data_dir.display(),
            "Opened RocksDB storage"
        );
        
        Ok(Self { db, config })
    }
    
    /// Open RocksDB with column families
    fn open_db(path: &Path) -> Result<DB> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        
        // Performance optimizations
        opts.set_max_write_buffer_number(4);
        opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
        opts.set_target_file_size_base(64 * 1024 * 1024); // 64MB
        opts.set_max_bytes_for_level_base(256 * 1024 * 1024); // 256MB
        opts.set_compression_type(DBCompressionType::Lz4);
        
        // Column families
        let cf_opts = Options::default();
        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_DEFAULT, Options::default()),
            ColumnFamilyDescriptor::new(CF_LOG, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_SNAPSHOT, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_META, cf_opts),
        ];
        
        DB::open_cf_descriptors(&opts, path, cfs).map_err(Into::into)
    }
    
    /// Get log entry key
    fn log_key(index: u64) -> Vec<u8> {
        index.to_be_bytes().to_vec()
    }
    
    /// Parse log entry key
    fn parse_log_key(key: &[u8]) -> Result<u64> {
        if key.len() != 8 {
            return Err(StorageError::Corruption(format!(
                "Invalid log key length: {}",
                key.len()
            )));
        }
        Ok(u64::from_be_bytes(key.try_into().unwrap()))
    }
    
    /// Sync to disk if configured
    async fn sync_if_needed(&self) -> Result<()> {
        match self.config.wal_sync {
            crate::WalSyncMode::Always => {
                self.db.flush()?;
            }
            crate::WalSyncMode::Periodic(_) => {
                // Handled by background task
            }
            crate::WalSyncMode::Never => {
                // No sync
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Storage for RocksDbStorage {
    async fn save_term(&self, term: u64) -> consensus::Result<()> {
        {
            let cf = self.db.cf_handle(CF_META).unwrap();
            self.db
                .put_cf(&cf, KEY_CURRENT_TERM, term.to_be_bytes())
                .map_err(StorageError::from)?;
        }
        self.sync_if_needed().await?;
        Ok(())
    }
    
    async fn load_term(&self) -> consensus::Result<u64> {
        let cf = self.db.cf_handle(CF_META).unwrap();
        match self.db.get_cf(&cf, KEY_CURRENT_TERM).map_err(StorageError::from)? {
            Some(data) => {
                if data.len() != 8 {
                    return Err(StorageError::Corruption(
                        "Invalid term data".to_string()
                    ).into());
                }
                Ok(u64::from_be_bytes(data.as_slice().try_into().unwrap()))
            }
            None => Ok(0),
        }
    }
    
    async fn save_vote(&self, candidate: Option<NodeId>) -> consensus::Result<()> {
        {
            let cf = self.db.cf_handle(CF_META).unwrap();
            match candidate {
                Some(node_id) => {
                    self.db
                        .put_cf(&cf, KEY_VOTED_FOR, node_id.as_bytes())
                        .map_err(StorageError::from)?;
                }
                None => {
                    self.db
                        .delete_cf(&cf, KEY_VOTED_FOR)
                        .map_err(StorageError::from)?;
                }
            }
        }
        self.sync_if_needed().await?;
        Ok(())
    }
    
    async fn load_vote(&self) -> consensus::Result<Option<NodeId>> {
        let cf = self.db.cf_handle(CF_META).unwrap();
        match self.db.get_cf(&cf, KEY_VOTED_FOR).map_err(StorageError::from)? {
            Some(data) => {
                if data.len() != 16 {
                    return Err(StorageError::Corruption(
                        "Invalid vote data".to_string()
                    ).into());
                }
                let bytes: [u8; 16] = data.as_slice().try_into().unwrap();
                Ok(Some(NodeId::from_bytes(bytes)))
            }
            None => Ok(None),
        }
    }
    
    async fn append_entries(&self, entries: &[LogEntry]) -> consensus::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        
        {
            let cf = self.db.cf_handle(CF_LOG)
                .ok_or_else(|| StorageError::Internal("Log column family not found".to_string()))?;
            let mut batch = WriteBatch::default();
            
            for entry in entries {
                let key = Self::log_key(entry.index);
                let value = bincode::serialize(entry).map_err(StorageError::from)?;
                batch.put_cf(&cf, key, value);
            }
            
            self.db.write(batch).map_err(StorageError::from)?;
        }
        
        self.sync_if_needed().await?;
        
        debug!(
            entries = entries.len(),
            first_index = entries[0].index,
            last_index = entries[entries.len() - 1].index,
            "Appended log entries"
        );
        
        Ok(())
    }
    
    async fn get_entry(&self, index: u64) -> consensus::Result<Option<LogEntry>> {
        if index == 0 {
            return Ok(None);
        }
        
        let cf = self.db.cf_handle(CF_LOG).unwrap();
        let key = Self::log_key(index);
        
        match self.db.get_cf(&cf, &key).map_err(StorageError::from)? {
            Some(data) => {
                let entry = bincode::deserialize(&data).map_err(StorageError::from)?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }
    
    async fn get_entries(&self, start: u64, end: u64) -> consensus::Result<Vec<LogEntry>> {
        if start == 0 || start >= end {
            return Ok(Vec::new());
        }
        
        let cf = self.db.cf_handle(CF_LOG).unwrap();
        let mut entries = Vec::new();
        
        let start_key = Self::log_key(start);
        let iter = self.db.iterator_cf(&cf, IteratorMode::From(&start_key, Direction::Forward));
        
        for item in iter {
            let (key, value) = item.map_err(StorageError::from)?;
            let index = Self::parse_log_key(&key)?;
            
            if index >= end {
                break;
            }
            
            let entry: LogEntry = bincode::deserialize(&value).map_err(StorageError::from)?;
            entries.push(entry);
        }
        
        Ok(entries)
    }
    
    async fn last_index(&self) -> consensus::Result<u64> {
        let cf = self.db.cf_handle(CF_LOG).unwrap();
        let iter = self.db.iterator_cf(&cf, IteratorMode::End);
        
        match iter.into_iter().next() {
            Some(Ok((key, _))) => {
                let index = Self::parse_log_key(&key)?;
                Ok(index)
            }
            _ => Ok(0),
        }
    }
    
    async fn last_term(&self) -> consensus::Result<u64> {
        let last_index = self.last_index().await?;
        if last_index == 0 {
            return Ok(0);
        }
        
        match self.get_entry(last_index).await? {
            Some(entry) => Ok(entry.term),
            None => Ok(0),
        }
    }
    
    async fn delete_entries_from(&self, index: u64) -> consensus::Result<()> {
        {
            let cf = self.db.cf_handle(CF_LOG)
                .ok_or_else(|| StorageError::Internal("Log column family not found".to_string()))?;
            let mut batch = WriteBatch::default();
            
            let start_key = Self::log_key(index);
            let iter = self.db.iterator_cf(&cf, IteratorMode::From(&start_key, Direction::Forward));
            
            for item in iter {
                let (key, _) = item.map_err(StorageError::from)?;
                batch.delete_cf(&cf, key);
            }
            
            self.db.write(batch).map_err(StorageError::from)?;
        }
        
        self.sync_if_needed().await?;
        
        info!(from_index = index, "Deleted log entries");
        Ok(())
    }
    
    async fn save_snapshot(&self, index: u64, term: u64, data: Vec<u8>) -> consensus::Result<()> {
        {
            let cf = self.db.cf_handle(CF_SNAPSHOT).unwrap();
            let key = format!("{:020}_{:020}", index, term);
            
            self.db
                .put_cf(&cf, key.as_bytes(), &data)
                .map_err(StorageError::from)?;
        }
        
        self.sync_if_needed().await?;
        
        info!(index, term, size = data.len(), "Saved snapshot");
        
        // Clean up old snapshots
        self.cleanup_old_snapshots().await?;
        
        Ok(())
    }
    
    async fn load_snapshot(&self) -> consensus::Result<Option<(u64, u64, Vec<u8>)>> {
        let cf = self.db.cf_handle(CF_SNAPSHOT).unwrap();
        let iter = self.db.iterator_cf(&cf, IteratorMode::End);
        
        match iter.into_iter().next() {
            Some(Ok((key, value))) => {
                let key_str = std::str::from_utf8(&key)
                    .map_err(|_| StorageError::Corruption("Invalid snapshot key".to_string()))?;
                    
                let parts: Vec<&str> = key_str.split('_').collect();
                if parts.len() != 2 {
                    return Err(StorageError::Corruption(
                        "Invalid snapshot key format".to_string()
                    ).into());
                }
                
                let index = parts[0].parse::<u64>()
                    .map_err(|_| StorageError::Corruption("Invalid snapshot index".to_string()))?;
                let term = parts[1].parse::<u64>()
                    .map_err(|_| StorageError::Corruption("Invalid snapshot term".to_string()))?;
                    
                Ok(Some((index, term, value.to_vec())))
            }
            _ => Ok(None),
        }
    }
}

impl RocksDbStorage {
    /// Clean up old snapshots
    async fn cleanup_old_snapshots(&self) -> Result<()> {
        let cf = self.db.cf_handle(CF_SNAPSHOT).unwrap();
        let mut snapshots = Vec::new();
        
        // Collect all snapshots
        let iter = self.db.iterator_cf(&cf, IteratorMode::Start);
        for item in iter {
            let (key, _) = item?;
            snapshots.push(key.to_vec());
        }
        
        // Keep only the latest max_snapshots
        if snapshots.len() > self.config.max_snapshots {
            let to_delete = snapshots.len() - self.config.max_snapshots;
            let mut batch = WriteBatch::default();
            
            for key in &snapshots[..to_delete] {
                batch.delete_cf(&cf, key);
            }
            
            self.db.write(batch)?;
            info!(deleted = to_delete, "Cleaned up old snapshots");
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    async fn test_storage() -> (RocksDbStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig {
            data_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let storage = RocksDbStorage::new(config).await.unwrap();
        (storage, temp_dir)
    }
    
    #[tokio::test]
    async fn test_term_and_vote() {
        let (storage, _dir) = test_storage().await;
        
        // Test term
        assert_eq!(storage.load_term().await.unwrap(), 0);
        storage.save_term(42).await.unwrap();
        assert_eq!(storage.load_term().await.unwrap(), 42);
        
        // Test vote
        assert_eq!(storage.load_vote().await.unwrap(), None);
        let node_id = NodeId::new();
        storage.save_vote(Some(node_id)).await.unwrap();
        assert_eq!(storage.load_vote().await.unwrap(), Some(node_id));
        storage.save_vote(None).await.unwrap();
        assert_eq!(storage.load_vote().await.unwrap(), None);
    }
    
    #[tokio::test]
    async fn test_log_entries() {
        let (storage, _dir) = test_storage().await;
        
        // Test empty log
        assert_eq!(storage.last_index().await.unwrap(), 0);
        assert_eq!(storage.last_term().await.unwrap(), 0);
        
        // Add entries
        let entries = vec![
            LogEntry { term: 1, index: 1, data: b"cmd1".to_vec() },
            LogEntry { term: 1, index: 2, data: b"cmd2".to_vec() },
            LogEntry { term: 2, index: 3, data: b"cmd3".to_vec() },
        ];
        
        storage.append_entries(&entries).await.unwrap();
        
        // Verify
        assert_eq!(storage.last_index().await.unwrap(), 3);
        assert_eq!(storage.last_term().await.unwrap(), 2);
        
        // Get single entry
        let entry = storage.get_entry(2).await.unwrap().unwrap();
        assert_eq!(entry.index, 2);
        assert_eq!(entry.data, b"cmd2");
        
        // Get range
        let range = storage.get_entries(2, 4).await.unwrap();
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].index, 2);
        assert_eq!(range[1].index, 3);
        
        // Delete from
        storage.delete_entries_from(2).await.unwrap();
        assert_eq!(storage.last_index().await.unwrap(), 1);
    }
}