//! Production-ready persistent storage implementations

pub mod rocksdb;
pub mod wal;
pub mod snapshot;
pub mod error;
pub mod encryption;
pub mod recovery;

pub use error::{StorageError, Result};
pub use rocksdb::RocksDbStorage;
pub use encryption::{StorageEncryption, EncryptionConfig, KeyManager};
pub use recovery::{RecoveryManager, BackupInfo, WalEntry, WalOperation};

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Data directory path
    pub data_dir: std::path::PathBuf,
    /// Enable write-ahead log
    pub enable_wal: bool,
    /// WAL sync mode
    pub wal_sync: WalSyncMode,
    /// Snapshot interval (in number of entries)
    pub snapshot_interval: u64,
    /// Maximum number of snapshots to keep
    pub max_snapshots: usize,
    /// Compaction settings
    pub compaction: CompactionConfig,
}

/// WAL sync mode
#[derive(Debug, Clone, Copy)]
pub enum WalSyncMode {
    /// Sync on every write (slowest, most durable)
    Always,
    /// Sync periodically
    Periodic(std::time::Duration),
    /// Never sync (fastest, least durable)
    Never,
}

/// Compaction configuration
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Enable automatic compaction
    pub auto_compaction: bool,
    /// Compaction interval
    pub interval: std::time::Duration,
    /// Target file size for compaction
    pub target_file_size: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: std::path::PathBuf::from("./data"),
            enable_wal: true,
            wal_sync: WalSyncMode::Periodic(std::time::Duration::from_secs(1)),
            snapshot_interval: 10000,
            max_snapshots: 3,
            compaction: CompactionConfig::default(),
        }
    }
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            auto_compaction: true,
            interval: std::time::Duration::from_secs(300),
            target_file_size: 64 * 1024 * 1024, // 64MB
        }
    }
}