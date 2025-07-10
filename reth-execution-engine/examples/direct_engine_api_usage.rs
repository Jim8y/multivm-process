use reth_execution_engine::engine::{RethExecutionEngine, ForkchoiceState, PayloadAttributesV3};
use multivm_common::traits::execution::ExecutionEngine;
use alloy_primitives::{Address, B256};
use std::path::PathBuf;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🚀 直接使用 Engine API 方法示例");
    info!("===================================");

    // 创建引擎
    let temp_dir = PathBuf::from("./temp_direct_api");
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir).ok();
    }
    std::fs::create_dir_all(&temp_dir)?;

    let mut engine = RethExecutionEngine::new(temp_dir.clone(), 8545, 1337).await?;
    ExecutionEngine::initialize(&mut engine).await.map_err(|e| format!("初始化失败: {:?}", e))?;

    info!("✅ Reth 引擎初始化完成");

    // 获取当前状态
    let current_block = engine.get_current_block_from_reth().await?;
    let parent_hash = engine.get_block_hash(current_block).await?;
    
    info!("📊 当前区块: #{}", current_block);
    info!("📋 父区块哈希: 0x{}", hex::encode(parent_hash));

    // 🎯 方法 1: 使用 forkchoiceUpdated + getPayload + newPayload 组合
    info!("\n🔧 方法 1: 完整的 forkchoice → getPayload → newPayload 流程");
    
    let fee_recipient = Address::from_slice(&[0x42; 20]);
    let result_hash_1 = method1_full_workflow(&engine, parent_hash, fee_recipient).await?;
    info!("✅ 方法 1 成功，区块哈希: 0x{}", hex::encode(result_hash_1));

    // 等待一下，让区块被处理
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // 🎯 方法 2: 直接使用 newPayload + forkchoice 组合（如果你已经有构建好的 payload）
    info!("\n🔧 方法 2: 直接 newPayload + forkchoice 流程");
    
    // 获取新的父区块
    let new_current_block = engine.get_current_block_from_reth().await?;
    let new_parent_hash = engine.get_block_hash(new_current_block).await?;
    
    let result_hash_2 = method2_direct_payload(&engine, new_parent_hash, fee_recipient).await?;
    info!("✅ 方法 2 成功，区块哈希: 0x{}", hex::encode(result_hash_2));

    // 验证最终状态
    let final_block = engine.get_current_block_from_reth().await?;
    info!("\n📊 最终区块号: #{}", final_block);
    info!("🎉 成功演示了直接使用 Engine API 的两种方式！");

    // 清理
    ExecutionEngine::shutdown(&mut engine, Some(std::time::Duration::from_secs(5))).await.map_err(|e| format!("关闭失败: {:?}", e))?;
    std::fs::remove_dir_all(&temp_dir).ok();

    Ok(())
}

/// 方法 1: 完整的 forkchoice → getPayload → newPayload 流程
async fn method1_full_workflow(
    engine: &RethExecutionEngine,
    parent_hash: B256,
    fee_recipient: Address,
) -> Result<B256, Box<dyn std::error::Error>> {
    
    // 1. 创建 forkchoice state
    let fork_choice_state = ForkchoiceState {
        head_block_hash: parent_hash,
        safe_block_hash: parent_hash,
        finalized_block_hash: parent_hash,
    };

    // 2. 创建 payload attributes
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    
    let payload_attributes = PayloadAttributesV3 {
        timestamp,
        prev_randao: B256::ZERO,
        suggested_fee_recipient: fee_recipient,
        parent_beacon_block_root: None,
        withdrawals: None,
    };

    info!("  🔄 1. 调用 engine_forkchoiceUpdatedV2 启动区块构建...");
    
    // 3. 启动区块构建
    let fork_response = engine.engine_forkchoice_updated_v2(
        &fork_choice_state,
        Some(payload_attributes)
    ).await?;

    // 4. 提取 payload_id
    let payload_id = fork_response
        .get("result")
        .and_then(|r| r.get("payloadId"))
        .and_then(|id| id.as_str())
        .ok_or("无法获取 payload_id")?;

    info!("  📦 2. 获取 payload_id: {}", payload_id);

    // 5. 获取构建好的 payload
    info!("  📤 3. 调用 engine_getPayloadV2 获取构建的区块...");
    let payload_response = engine.engine_get_payload_v2(payload_id).await?;
    let payload = &payload_response.execution_payload;
    let block_hash = payload.payload_inner.block_hash;

    info!("  🔗 4. 获得区块哈希: 0x{}", hex::encode(block_hash));

    // 6. 提交 payload
    info!("  ✅ 5. 调用 engine_newPayloadV2 提交区块...");
    let new_payload_response = engine.engine_new_payload_v2(payload).await?;
    
    // 检查响应
    if let Some(result) = new_payload_response.get("result") {
        if let Some(status) = result.get("status").and_then(|s| s.as_str()) {
            info!("     状态: {}", status);
            if status != "VALID" && status != "SYNCING" {
                return Err(format!("newPayload 失败: {}", status).into());
            }
        }
    }

    // 7. 最终确认
    info!("  🎯 6. 调用 engine_forkchoiceUpdatedV3 最终确认...");
    let final_fork_choice = ForkchoiceState {
        head_block_hash: block_hash,
        safe_block_hash: block_hash,
        finalized_block_hash: parent_hash,
    };

    let final_response = engine.engine_forkchoice_updated_v3(
        &final_fork_choice,
        None
    ).await?;

    // 检查最终响应
    if let Some(result) = final_response.get("result") {
        if let Some(status) = result.get("payloadStatus")
            .and_then(|ps| ps.get("status"))
            .and_then(|s| s.as_str()) {
            info!("     最终状态: {}", status);
        }
    }

    Ok(block_hash)
}

/// 方法 2: 直接使用 newPayload + forkchoice（假设你已经有了构建好的 payload）
async fn method2_direct_payload(
    engine: &RethExecutionEngine,
    parent_hash: B256,
    fee_recipient: Address,
) -> Result<B256, Box<dyn std::error::Error>> {
    
    info!("  🏗️  1. 手动构造 ExecutionPayloadV2...");
    
    // 这里我们先用方法1获取一个真实的 payload，然后演示直接提交
    // 在实际使用中，你可能从其他地方获得这个 payload
    
    // 临时获取一个真实的 payload（简化演示）
    let fork_choice_state = ForkchoiceState {
        head_block_hash: parent_hash,
        safe_block_hash: parent_hash,
        finalized_block_hash: parent_hash,
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() + 12;
    
    let payload_attributes = PayloadAttributesV3 {
        timestamp,
        prev_randao: B256::ZERO,
        suggested_fee_recipient: fee_recipient,
        parent_beacon_block_root: None,
        withdrawals: None,
    };

    // 获取一个真实的 payload 用于演示
    let fork_response = engine.engine_forkchoice_updated_v2(
        &fork_choice_state,
        Some(payload_attributes)
    ).await?;

    let payload_id = fork_response
        .get("result")
        .and_then(|r| r.get("payloadId"))
        .and_then(|id| id.as_str())
        .ok_or("无法获取 payload_id")?;

    let payload_response = engine.engine_get_payload_v2(payload_id).await?;
    let payload = &payload_response.execution_payload;
    let block_hash = payload.payload_inner.block_hash;

    info!("  📋 2. 现在我们有了 payload，区块哈希: 0x{}", hex::encode(block_hash));
    
    // 现在演示直接使用 newPayload + forkchoice 的流程
    
    info!("  ✅ 3. 直接调用 engine_newPayloadV2...");
    let new_payload_response = engine.engine_new_payload_v2(payload).await?;
    
    if let Some(result) = new_payload_response.get("result") {
        if let Some(status) = result.get("status").and_then(|s| s.as_str()) {
            info!("     newPayload 状态: {}", status);
        }
    }

    info!("  🎯 4. 直接调用 engine_forkchoiceUpdatedV3 确认链头...");
    let final_fork_choice = ForkchoiceState {
        head_block_hash: block_hash,
        safe_block_hash: block_hash,
        finalized_block_hash: parent_hash,
    };

    let final_response = engine.engine_forkchoice_updated_v3(
        &final_fork_choice,
        None
    ).await?;

    if let Some(result) = final_response.get("result") {
        if let Some(status) = result.get("payloadStatus")
            .and_then(|ps| ps.get("status"))
            .and_then(|s| s.as_str()) {
            info!("     forkchoice 状态: {}", status);
        }
    }

    Ok(block_hash)
}