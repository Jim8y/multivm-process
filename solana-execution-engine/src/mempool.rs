use crate::engine_rpc_helper::SanitizedTransaction;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use solana_sdk::signature::Signature;
use tracing::{debug, info, warn};

/// Mempool 中的交易条目
#[derive(Debug, Clone)]
pub struct MempoolEntry {
    /// 交易数据
    pub transaction: SanitizedTransaction,
    /// 交易签名（用作唯一标识符）
    pub signature: Signature,
}

impl PartialEq for MempoolEntry {
    fn eq(&self, other: &Self) -> bool {
        self.signature == other.signature
    }
}

impl MempoolEntry {
    /// 创建新的 mempool 条目
    pub fn new(transaction: SanitizedTransaction) -> Self {
        let signature = transaction.signature;

        Self {
            transaction,
            signature,
        }
    }
}

/// 支持随机删除的 Solana Mempool
///
/// 这个 mempool 实现了以下特性：
/// - 队列操作：支持 FIFO 获取前 n 项
/// - 随机删除：支持根据签名快速删除任意交易
/// - 无容量限制：可以无限制地添加交易
#[derive(Debug)]
pub struct SolanaMempool {
    /// 使用 IndexMap 存储交易，保持插入顺序同时支持快速随机访问
    /// Key: 交易签名, Value: mempool 条目
    transactions: IndexMap<Signature, MempoolEntry>,
}

impl Default for SolanaMempool {
    fn default() -> Self {
        Self::new()
    }
}

impl SolanaMempool {
    /// 创建新的 mempool
    pub fn new() -> Self {
        Self {
            transactions: IndexMap::new(),
        }
    }

    /// 向 mempool 添加交易（入队操作）
    ///
    /// # Arguments
    /// * `transaction` - 要添加的交易
    ///
    /// # Returns
    /// * `Ok(())` - 成功添加
    /// * `Err(String)` - 添加失败的原因
    pub fn push(&mut self, transaction: SanitizedTransaction) -> Result<(), String> {
        let entry = MempoolEntry::new(transaction);
        let signature = entry.signature;

        // 检查交易是否已存在
        if self.transactions.contains_key(&signature) {
            return Err(format!(
                "Transaction with signature {} already exists",
                signature
            ));
        }

        // 添加交易
        self.transactions.insert(signature, entry);

        debug!(
            "Added transaction {} to mempool (total: {} transactions)",
            signature,
            self.transactions.len()
        );

        Ok(())
    }

    /// 获取队列前 n 项交易（不删除元素）
    ///
    /// # Arguments
    /// * `count` - 要获取的交易数量
    ///
    /// # Returns
    /// * `Vec<&MempoolEntry>` - 按插入顺序（FIFO）的交易引用列表，如果不足 n 项则返回所有可用的交易
    pub fn get_front(&self, count: usize) -> Vec<&MempoolEntry> {
        self.transactions.values().take(count).collect()
    }

    /// 根据签名随机删除交易
    ///
    /// # Arguments
    /// * `signature` - 要删除的交易签名
    ///
    /// # Returns
    /// * `Some(MempoolEntry)` - 成功删除的交易
    /// * `None` - 交易不存在
    pub fn remove(&mut self, signature: &Signature) -> Option<MempoolEntry> {
        if let Some(entry) = self.transactions.shift_remove(signature) {
            debug!(
                "Randomly removed transaction {} from mempool (remaining: {})",
                signature,
                self.transactions.len()
            );
            Some(entry)
        } else {
            warn!(
                "Attempted to remove non-existent transaction: {}",
                signature
            );
            None
        }
    }

    /// 批量移除多个交易
    ///
    /// # Arguments
    /// * `signatures` - 要移除的交易签名列表
    ///
    /// # Returns
    /// * `Vec<MempoolEntry>` - 成功移除的交易列表
    pub fn remove_batch(&mut self, signatures: &[Signature]) -> Vec<MempoolEntry> {
        let mut removed = Vec::new();

        for signature in signatures {
            if let Some(entry) = self.remove(signature) {
                removed.push(entry);
            }
        }

        info!("Batch removed {} transactions from mempool", removed.len());
        removed
    }

    /// 检查交易是否存在
    ///
    /// # Arguments
    /// * `signature` - 交易签名
    ///
    /// # Returns
    /// * `bool` - 交易是否存在
    pub fn contains(&self, signature: &Signature) -> bool {
        self.transactions.contains_key(signature)
    }

    /// 获取交易详情
    ///
    /// # Arguments
    /// * `signature` - 交易签名
    ///
    /// # Returns
    /// * `Option<&MempoolEntry>` - 交易详情引用
    pub fn get(&self, signature: &Signature) -> Option<&MempoolEntry> {
        self.transactions.get(signature)
    }

    /// 获取 mempool 中的交易数量
    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    /// 检查 mempool 是否为空
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    /// 清空 mempool
    pub fn clear(&mut self) {
        let count = self.transactions.len();
        self.transactions.clear();
        info!("Cleared {} transactions from mempool", count);
    }

    /// 获取所有交易的签名列表（按插入顺序）
    pub fn signatures(&self) -> Vec<Signature> {
        self.transactions.keys().copied().collect()
    }

    /// 获取 mempool 统计信息
    pub fn stats(&self) -> MempoolStats {
        MempoolStats {
            transaction_count: self.transactions.len(),
        }
    }
}

/// Mempool 统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolStats {
    /// 当前交易数量
    pub transaction_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::body::Bytes;
    use solana_sdk::{
        message::Message,
        pubkey::Pubkey,
        signature::{Keypair, Signer},
        system_instruction,
        transaction::Transaction,
    };

    fn create_test_transaction(lamports: u64) -> SanitizedTransaction {
        let from_keypair = Keypair::new();
        let to_pubkey = Pubkey::new_unique();
        let instruction =
            system_instruction::transfer(&from_keypair.pubkey(), &to_pubkey, lamports);
        let message = Message::new(&[instruction], Some(&from_keypair.pubkey()));
        let mut transaction = Transaction::new_unsigned(message);
        transaction.sign(&[&from_keypair], transaction.message.recent_blockhash);

        SanitizedTransaction {
            signature: transaction.signatures[0],
            raw_body: Bytes::from(format!("test_transaction_{}", lamports)),
        }
    }

    #[test]
    fn test_mempool_basic_operations() {
        let mut mempool = SolanaMempool::new();

        // 测试空 mempool
        assert!(mempool.is_empty());
        assert_eq!(mempool.len(), 0);
        assert!(mempool.get_front(1).is_empty());

        // 添加交易
        let tx1 = create_test_transaction(100);
        let tx2 = create_test_transaction(200);

        assert!(mempool.push(tx1.clone()).is_ok());
        assert!(mempool.push(tx2.clone()).is_ok());

        assert_eq!(mempool.len(), 2);
        assert!(!mempool.is_empty());

        // 测试包含检查
        assert!(mempool.contains(&tx1.signature));
        assert!(mempool.contains(&tx2.signature));

        // 测试获取前 n 项（不删除）
        let front_txs = mempool.get_front(1);
        assert_eq!(front_txs.len(), 1);
        assert_eq!(front_txs[0].signature, tx1.signature); // 第一个添加的交易
        assert_eq!(mempool.len(), 2); // 元素没有被删除

        // 测试随机删除
        let removed = mempool.remove(&tx2.signature).unwrap();
        assert_eq!(removed.signature, tx2.signature);
        assert_eq!(mempool.len(), 1);

        // 清理剩余交易
        mempool.remove(&tx1.signature);
        assert!(mempool.is_empty());
    }

    #[test]
    fn test_mempool_fifo_ordering() {
        let mut mempool = SolanaMempool::new();

        let tx1 = create_test_transaction(100);
        let tx2 = create_test_transaction(200);
        let tx3 = create_test_transaction(300);

        // 按顺序添加交易
        mempool.push(tx1.clone()).unwrap();
        mempool.push(tx2.clone()).unwrap();
        mempool.push(tx3.clone()).unwrap();

        // 按 FIFO 顺序获取前 3 项
        let front_txs = mempool.get_front(3);
        assert_eq!(front_txs.len(), 3);
        assert_eq!(front_txs[0].signature, tx1.signature); // 第一个添加的
        assert_eq!(front_txs[1].signature, tx2.signature); // 第二个添加的
        assert_eq!(front_txs[2].signature, tx3.signature); // 第三个添加的
    }

    #[test]
    fn test_mempool_no_capacity_limits() {
        let mut mempool = SolanaMempool::new();

        let tx1 = create_test_transaction(100);
        let tx2 = create_test_transaction(200);
        let tx3 = create_test_transaction(300);

        // 可以无限制添加交易
        assert!(mempool.push(tx1).is_ok());
        assert!(mempool.push(tx2).is_ok());
        assert!(mempool.push(tx3).is_ok());

        assert_eq!(mempool.len(), 3);
    }

    #[test]
    fn test_mempool_batch_operations() {
        let mut mempool = SolanaMempool::new();

        let tx1 = create_test_transaction(100);
        let tx2 = create_test_transaction(200);
        let tx3 = create_test_transaction(300);

        mempool.push(tx1.clone()).unwrap();
        mempool.push(tx2.clone()).unwrap();
        mempool.push(tx3.clone()).unwrap();

        // 测试获取前 n 项
        let front_txs = mempool.get_front(2);
        assert_eq!(front_txs.len(), 2);
        assert_eq!(front_txs[0].signature, tx1.signature); // 第一个添加的
        assert_eq!(front_txs[1].signature, tx2.signature); // 第二个添加的
        assert_eq!(mempool.len(), 3); // 元素没有被删除

        // 批量删除
        let signatures = vec![tx1.signature, tx3.signature];
        let removed = mempool.remove_batch(&signatures);
        assert_eq!(removed.len(), 2);
        assert_eq!(mempool.len(), 1);

        // 验证剩余的交易
        assert!(mempool.contains(&tx2.signature));
        assert!(!mempool.contains(&tx1.signature));
        assert!(!mempool.contains(&tx3.signature));
    }

    #[test]
    fn test_mempool_duplicate_key_insertion() {
        let mut mempool = SolanaMempool::new();

        // 创建一个测试交易
        let tx1 = create_test_transaction(100);

        // 第一次插入应该成功
        let result1 = mempool.push(tx1.clone());
        assert!(result1.is_ok(), "First insertion should succeed");
        assert_eq!(mempool.len(), 1);

        // 第二次插入相同的交易应该失败
        let result2 = mempool.push(tx1.clone());
        assert!(result2.is_err(), "Second insertion should fail");
        assert_eq!(mempool.len(), 1, "Mempool size should remain unchanged");

        // 验证错误消息包含签名信息
        let error_msg = result2.unwrap_err();
        assert!(
            error_msg.contains(&tx1.signature.to_string()),
            "Error message should contain the transaction signature"
        );
        assert!(
            error_msg.contains("already exists"),
            "Error message should indicate transaction already exists"
        );

        // 验证原始交易仍然存在且未被修改
        assert!(mempool.contains(&tx1.signature));
        let stored_entry = mempool.get(&tx1.signature).unwrap();
        assert_eq!(stored_entry.signature, tx1.signature);
        assert_eq!(stored_entry.transaction, tx1);
    }

    #[test]
    fn test_mempool_duplicate_key_with_different_raw_body() {
        let mut mempool = SolanaMempool::new();

        // 创建两个具有相同签名但不同raw_body的交易
        let signature = Signature::default();
        let tx1 = SanitizedTransaction {
            signature,
            raw_body: Bytes::from("first_transaction"),
        };
        let tx2 = SanitizedTransaction {
            signature,                                   // 相同的签名
            raw_body: Bytes::from("second_transaction"), // 不同的raw_body
        };

        // 第一次插入应该成功
        let result1 = mempool.push(tx1.clone());
        assert!(result1.is_ok(), "First insertion should succeed");
        assert_eq!(mempool.len(), 1);

        // 第二次插入具有相同签名的交易应该失败
        let result2 = mempool.push(tx2.clone());
        assert!(
            result2.is_err(),
            "Second insertion should fail even with different raw_body"
        );
        assert_eq!(mempool.len(), 1, "Mempool size should remain unchanged");

        // 验证原始交易仍然存在
        let stored_entry = mempool.get(&signature).unwrap();
        assert_eq!(
            stored_entry.transaction.raw_body,
            Bytes::from("first_transaction")
        );
        assert_ne!(
            stored_entry.transaction.raw_body,
            Bytes::from("second_transaction")
        );
    }
}
