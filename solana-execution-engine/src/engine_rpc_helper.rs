use crate::SolanaEngineError;
use hyper::body::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use solana_runtime_transaction::runtime_transaction::RuntimeTransaction;
use solana_sdk::signature::Signature;
use tracing::{error, info, warn};

pub struct SolanaEngineRpcHelper;

#[derive(Debug, Clone)]
pub struct SanitizedTransaction {
    pub signature: Signature,
    pub raw_body: Bytes,
    // TODO: 如果需要，添加额外的字段
}

impl PartialEq for SanitizedTransaction {
    fn eq(&self, other: &Self) -> bool {
        self.signature == other.signature
    }
}

impl Eq for SanitizedTransaction {}

impl SanitizedTransaction {
    fn default() -> SanitizedTransaction {
        SanitizedTransaction {
            signature: Signature::default(),
            raw_body: Bytes::new(),
        }
    }

    /// 获取交易签名
    pub fn signature(&self) -> &Signature {
        &self.signature
    }
}

impl SolanaEngineRpcHelper {
    /// Handle requestAirdrop RPC method
    /// Reference: https://github.com/vm-multiverse/multivm-agave/blob/master/rpc/src/rpc.rs#L3776
    pub async fn handle_request_airdrop(
        _raw_body: Bytes,
    ) -> Result<SanitizedTransaction, SolanaEngineError> {
        info!("Handling requestAirdrop method");

        // TODO: 实现 requestAirdrop 处理逻辑
        // 1. 从 raw_body 解析 JSON-RPC 请求，提取 airdrop 参数
        // 2. 从请求参数中提取 pubkey 和 lamports
        // 3. 使用 system_instruction::transfer 创建 airdrop 交易
        // 4. 创建 Transaction 结构体
        // 5. 校验交易的 recent_blockhash
        // 6. 通过 RPC Client 发送 Transaction 进行 simulate_transaction 验证
        // 7. 如果模拟成功，存储为包含原始 raw_body 和签名的 SanitizedTransaction
        // 8. 如果模拟失败，返回相应的错误

        // Ok(SanitizedTransaction::default())
        Err(SolanaEngineError::UnsanitizedTransaction(
            "Unimplement".to_owned(),
        ))
    }

    /// Handle sendTransaction RPC method
    /// Reference: https://github.com/vm-multiverse/multivm-agave/blob/master/rpc/src/rpc.rs#L3835
    pub async fn handle_send_transaction(
        _raw_body: Bytes,
    ) -> Result<SanitizedTransaction, SolanaEngineError> {
        info!("Handling sendTransaction method");

        // TODO: 实现 sendTransaction 处理逻辑
        // 1. 从 raw_body 解析 JSON-RPC 请求，提取交易数据
        // 2. 从请求参数中解码 base64 编码的交易
        // 3. 使用 bincode 将交易字节反序列化为 Transaction 结构体
        // 4. 校验交易的 recent_blockhash
        // 5. 通过 RPC Client 发送 Transaction 进行 simulate_transaction 验证
        // 6. 如果模拟成功，存储为包含原始 raw_body 和签名的 SanitizedTransaction
        // 7. 如果模拟失败，返回带有详细信息的 UnsanitizedTransaction 错误

        // Ok(SanitizedTransaction::default())
        Err(SolanaEngineError::UnsanitizedTransaction(
            "Unimplement".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_rpc_server::SolanaEngineRpcServer;
    use anyhow::Result;
    use solana_client::rpc_client::RpcClient;
    use solana_sdk::{
        commitment_config::CommitmentConfig, signature::Keypair, signer::Signer,
        system_instruction, transaction::Transaction,
    };

    pub fn create_test_keypair() -> Keypair {
        Keypair::new()
    }

    fn setup_logging() {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false)
            .without_time()
            .try_init()
            .ok();
    }

    async fn init_test_rpc_server(port: u16) -> Result<SolanaEngineRpcServer> {
        let mut rpc_server = SolanaEngineRpcServer::new(
            "127.0.0.1".to_string(),
            port,
            "https://api.devnet.solana.com".to_string(),
        );
        rpc_server.start().await?;
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        Ok(rpc_server)
    }

    async fn create_test_rpc_client(port: u16) -> Result<RpcClient> {
        let rpc_url = format!("http://127.0.0.1:{}", port);
        let commitment = CommitmentConfig::confirmed();
        let client = RpcClient::new_with_commitment(rpc_url, commitment);
        Ok(client)
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn test_rpc_proxy_server_for_cli() -> Result<()> {
        setup_logging();
        let mut rpc_server = init_test_rpc_server(8887).await?;
        let rpc_client = create_test_rpc_client(8887).await?;
        let now_slot = rpc_client.get_slot()?;
        assert!(now_slot > 0, "Slot should be greater than 0");

        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        rpc_server.stop().await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_rpc_proxy_server_get_slot() -> Result<()> {
        let mut rpc_server = init_test_rpc_server(8888).await?;
        let rpc_client = create_test_rpc_client(8888).await?;
        let now_slot = rpc_client.get_slot()?;
        assert!(now_slot > 0, "Slot should be greater than 0");
        rpc_server.stop().await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requestAirdrop method not implemented yet"]
    async fn test_rpc_proxy_server_request_airdrop() -> Result<()> {
        setup_logging();
        let mut rpc_server = init_test_rpc_server(8889).await?;

        let rpc_client = create_test_rpc_client(8889).await?;
        let alice = create_test_keypair();
        let _signature = rpc_client.request_airdrop(&alice.pubkey(), 1_000_000_000);

        // 断言全局 mempool 的交易数量为 1
        let mempool_count = crate::engine::GLOBAL_MEMPOOL.read().await.len();
        assert_eq!(
            mempool_count, 1,
            "Expected 1 transaction in mempool, found {}",
            mempool_count
        );

        rpc_server.stop().await?;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "sendTransaction method not implemented yet"]
    async fn test_rpc_proxy_server_send_transaction() -> Result<()> {
        setup_logging();
        let mut rpc_server = init_test_rpc_server(8890).await?;

        let rpc_client = create_test_rpc_client(8890).await?;
        let alice = create_test_keypair();
        let bob = create_test_keypair();

        // 创建一个简单的转账交易
        let recent_blockhash = rpc_client.get_latest_blockhash()?;
        let instruction = system_instruction::transfer(&alice.pubkey(), &bob.pubkey(), 1_000_000);
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&alice.pubkey()),
            &[&alice],
            recent_blockhash,
        );

        let _signature = rpc_client.send_transaction(&transaction);

        // 断言全局 mempool 的交易数量为 1
        let mempool_count = crate::engine::GLOBAL_MEMPOOL.read().await.len();
        assert_eq!(
            mempool_count, 1,
            "Expected 1 transaction in mempool, found {}",
            mempool_count
        );

        rpc_server.stop().await?;
        Ok(())
    }
}
