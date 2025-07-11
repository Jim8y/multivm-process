use alloy_primitives::{Address, B256};
use multivm_common::traits::execution::ExecutionEngine;
use reth_execution_engine::engine::RethExecutionEngine;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🔍 开始多区块 Engine API V2 测试");
    info!("========================================");

    // 创建临时目录
    let temp_dir = PathBuf::from("./temp_v3_test");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }
    std::fs::create_dir_all(&temp_dir)?;

    // 创建真实的 Reth 引擎
    info!("🚀 启动真实 Reth 引擎进行测试...");
    let mut engine = RethExecutionEngine::new(temp_dir.clone(), 8545, 1337).await?;

    // 初始化引擎
    engine.initialize().await?;

    // 等待引擎启动
    info!("⏳ 等待引擎完全启动...");
    sleep(Duration::from_secs(5)).await;

    // 设置费用接收者
    let fee_recipient = Address::from_slice(&[0x42; 20]);
    info!("💰 费用接收者: 0x{}", hex::encode(fee_recipient));

    // 连续生成多个区块
    let blocks_to_generate = 4;
    info!("🎯 准备连续生成 {} 个区块", blocks_to_generate);

    let initial_block = engine.get_current_block_from_reth().await?;
    info!("📋 初始区块号: {}", initial_block);

    let mut current_parent_hash = engine.get_block_hash(initial_block).await?;
    info!("📋 初始父区块哈希: 0x{}", hex::encode(current_parent_hash));

    let mut generated_blocks = Vec::new();

    for i in 1..=blocks_to_generate {
        info!("🏗️ 开始生成区块 #{} (总共{}个)...", i, blocks_to_generate);

        // 生产新区块
        let new_block_hash = engine
            .produce_block_v3(current_parent_hash, fee_recipient)
            .await?;
        info!("✅ 区块 #{} 生成成功: 0x{}", i, hex::encode(new_block_hash));

        generated_blocks.push(new_block_hash);

        // 等待区块被添加到链上
        let mut attempts = 0;
        let max_attempts = 10;
        let target_block_number = initial_block + i;

        loop {
            let current_block = engine.get_current_block_from_reth().await?;
            if current_block >= target_block_number {
                info!("✅ 区块 #{} 已添加到链上 (区块号: {})", i, current_block);
                break;
            }

            if attempts >= max_attempts {
                warn!("⚠️ 区块 #{} 添加超时，但继续下一个区块", i);
                break;
            }

            info!(
                "⏳ 等待区块 #{} 被添加到链上... (当前: {}, 目标: {}, 尝试: {}/{})",
                i,
                current_block,
                target_block_number,
                attempts + 1,
                max_attempts
            );
            sleep(Duration::from_millis(500)).await;
            attempts += 1;
        }

        // 更新父区块哈希为新生成的区块
        current_parent_hash = new_block_hash;

        // 在生成下一个区块前稍等一下
        if i < blocks_to_generate {
            info!("⏸️ 等待 1 秒后生成下一个区块...");
            sleep(Duration::from_secs(1)).await;
        }
    }

    // 最终验证
    info!("🔍 最终验证所有区块...");
    let final_block = engine.get_current_block_from_reth().await?;
    info!("📋 最终区块号: {}", final_block);

    let expected_final_block = initial_block + blocks_to_generate;
    if final_block >= expected_final_block {
        info!("✅ 成功生成 {} 个区块!", blocks_to_generate);
        info!("📈 区块号增长: {} -> {}", initial_block, final_block);
    } else {
        warn!(
            "⚠️ 可能有些区块还在同步中: 期望 {}, 实际 {}",
            expected_final_block, final_block
        );
    }

    // 验证生成的区块哈希
    info!("🔍 验证生成的区块哈希...");
    for (i, &block_hash) in generated_blocks.iter().enumerate() {
        let block_number = initial_block + (i as u64) + 1;
        if block_number <= final_block {
            let expected_hash = engine.get_block_hash(block_number).await?;
            if expected_hash == block_hash {
                info!("✅ 区块 #{} 哈希验证成功", i + 1);
            } else {
                warn!("⚠️ 区块 #{} 哈希不匹配:", i + 1);
                warn!("   期望: 0x{}", hex::encode(expected_hash));
                warn!("   实际: 0x{}", hex::encode(block_hash));
            }
        }
    }

    info!("📊 总结:");
    info!("   - 尝试生成: {} 个区块", blocks_to_generate);
    info!("   - 初始区块号: {}", initial_block);
    info!("   - 最终区块号: {}", final_block);
    info!(
        "   - 实际增长: {} 个区块",
        final_block.saturating_sub(initial_block)
    );

    // shutdown 前 sleep 1 秒，确保所有 RPC 请求完成
    sleep(Duration::from_secs(1)).await;

    // 清理
    info!("🔄 关闭引擎...");
    engine.shutdown(Some(Duration::from_secs(5))).await?;
    std::fs::remove_dir_all(&temp_dir).ok();

    info!("✅ 多区块 Engine API V2 测试完成!");

    Ok(())
}
