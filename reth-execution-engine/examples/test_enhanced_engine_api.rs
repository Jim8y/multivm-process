use reth_execution_engine::engine_api::{
    EngineApiClientBuilder, PayloadAttributes, RetryConfig, WithdrawalRequest,
};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Testing Enhanced Engine API Client");
    println!("=====================================");

    // Create JWT secret
    let jwt_secret = Arc::new(RwLock::new(Some(
        "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
    )));

    // Configure advanced retry strategy
    let retry_config = RetryConfig {
        max_retries: 3,
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(10),
        backoff_multiplier: 2.0,
        timeout: Duration::from_secs(30),
        jitter: true,
    };

    // Build Engine API client
    let client = EngineApiClientBuilder::new()
        .engine_url("http://localhost:8551".to_string())
        .jwt_secret(jwt_secret.clone())
        .retry_config(retry_config)
        .max_concurrent_requests(20)
        .build()?;

    // Test 1: Health Check
    println!("\n📊 1. Health Check Test");
    println!("----------------------");

    match client.health_check().await {
        Ok(health_status) => {
            info!("Health check completed!");
            println!("  ✅ Healthy: {}", health_status.is_healthy);
            println!("  ⏱️  Response time: {:?}", health_status.response_time);
            println!("  🔧 Capabilities: {:?}", health_status.capabilities);
            if let Some(error) = health_status.error_message {
                println!("  ❌ Error: {}", error);
            }
        }
        Err(e) => {
            warn!("Health check failed: {}", e);
            println!("  ❌ Health check failed: {}", e);
        }
    }

    // Test 2: Create mock execution payload with blob support
    println!("\n🔧 2. Engine API V3 Methods Test");
    println!("--------------------------------");

    let execution_payload = create_mock_execution_payload();
    let blob_hashes = vec![
        "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string(),
        "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890".to_string(),
    ];
    let parent_beacon_block_root =
        "0x9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba";

    // Test engine_newPayloadV3
    println!("  Testing engine_newPayloadV3 with blob support...");
    match client
        .new_payload_v3(&execution_payload, &blob_hashes, parent_beacon_block_root)
        .await
    {
        Ok(response) => {
            info!("newPayloadV3 succeeded!");
            println!("    ✅ Status: {:?}", response.status);
            println!("    ⏱️  Processing time: {:?}", response.processing_time);
            if let Some(blob_gas) = response.blob_gas_used {
                println!("    💾 Blob gas used: {}", blob_gas);
            }
            if let Some(hash) = response.latest_valid_hash {
                println!("    🔗 Latest valid hash: {}", hash);
            }
        }
        Err(e) => {
            warn!("newPayloadV3 failed: {}", e);
            println!("    ❌ Failed: {}", e);
        }
    }

    // Test 3: Fork choice update with withdrawals
    println!("\n💰 3. Fork Choice Update with Withdrawals");
    println!("------------------------------------------");

    let forkchoice_state = create_mock_forkchoice_state();
    let payload_attributes = PayloadAttributes {
        timestamp: 1234567890,
        prev_randao: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
            .to_string(),
        suggested_fee_recipient: "0xabcdef1234567890abcdef1234567890abcdef12".to_string(),
        withdrawals: Some(vec![
            WithdrawalRequest {
                index: 0,
                validator_index: 12345,
                address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
                amount: 1000000000000000000, // 1 ETH
            },
            WithdrawalRequest {
                index: 1,
                validator_index: 12346,
                address: "0xabcdef1234567890abcdef1234567890abcdef12".to_string(),
                amount: 2000000000000000000, // 2 ETH
            },
        ]),
        parent_beacon_block_root: Some(parent_beacon_block_root.to_string()),
    };

    match client
        .forkchoice_updated_v3(&forkchoice_state, Some(&payload_attributes))
        .await
    {
        Ok(response) => {
            info!("forkchoiceUpdatedV3 succeeded!");
            println!("    ✅ Status: {:?}", response.payload_status);
            println!("    ⏱️  Processing time: {:?}", response.processing_time);
            println!(
                "    💰 Withdrawals processed: {}",
                response.withdrawals_processed
            );
            if let Some(payload_id) = response.payload_id {
                println!("    🆔 Payload ID: {}", payload_id);

                // Test engine_getPayloadV3 if we got a payload ID
                println!("\n📦 4. Get Payload V3 with Blob Bundle");
                println!("--------------------------------------");

                match client.get_payload_v3(&payload_id).await {
                    Ok(payload_response) => {
                        info!("getPayloadV3 succeeded!");
                        println!("    ✅ Payload retrieved successfully");
                        println!(
                            "    ⏱️  Processing time: {:?}",
                            payload_response.processing_time
                        );

                        if let Some(ref blobs_bundle) = payload_response.blobs_bundle {
                            println!("    🫧 Blob bundle details:");
                            println!("        Count: {}", blobs_bundle.blob_count);
                            println!(
                                "        Total size: {} bytes",
                                blobs_bundle.total_size_bytes
                            );
                            println!("        Commitments: {}", blobs_bundle.commitments.len());
                            println!("        Proofs: {}", blobs_bundle.proofs.len());
                        }

                        if let Some(block_value) = payload_response.block_value {
                            println!("    💵 Block value: {}", block_value);
                        }

                        if let Some(override_builder) = payload_response.should_override_builder {
                            println!("    🔧 Should override builder: {}", override_builder);
                        }
                    }
                    Err(e) => {
                        warn!("getPayloadV3 failed: {}", e);
                        println!("    ❌ Failed: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            warn!("forkchoiceUpdatedV3 failed: {}", e);
            println!("    ❌ Failed: {}", e);
        }
    }

    // Test 5: Show metrics
    println!("\n📈 5. Engine API Metrics");
    println!("------------------------");

    let metrics = client.get_metrics();
    println!("  📊 Total requests: {}", metrics.requests_total);
    println!("  ✅ Successful requests: {}", metrics.requests_successful);
    println!("  ❌ Failed requests: {}", metrics.requests_failed);
    println!("  🔄 Total retries: {}", metrics.retries_total);
    println!(
        "  ⏱️  Average response time: {}ms",
        metrics.average_response_time_ms
    );
    println!("  📦 Payload submissions: {}", metrics.payload_submissions);
    println!("  🔀 Forkchoice updates: {}", metrics.forkchoice_updates);
    println!("  📥 Payload retrievals: {}", metrics.payload_retrievals);
    println!(
        "  🫧 Blob bundles processed: {}",
        metrics.blob_bundles_processed
    );
    println!(
        "  💰 Withdrawals processed: {}",
        metrics.withdrawals_processed
    );
    println!("  📊 Success rate: {:.2}%", metrics.success_rate);

    println!("\n🎉 Enhanced Engine API Client Test Complete!");
    println!("==============================================");

    Ok(())
}

fn create_mock_execution_payload() -> Value {
    json!({
        "parentHash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        "feeRecipient": "0xabcdef1234567890abcdef1234567890abcdef12",
        "stateRoot": "0x9876543210fedcba9876543210fedcba9876543210fedcba9876543210fedcba",
        "receiptsRoot": "0x1111111111111111111111111111111111111111111111111111111111111111",
        "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
        "prevRandao": "0x2222222222222222222222222222222222222222222222222222222222222222",
        "blockNumber": "0x1",
        "gasLimit": "0x1c9c380",
        "gasUsed": "0x5208",
        "timestamp": "0x6478e5f0",
        "extraData": "0x",
        "baseFeePerGas": "0x7",
        "blockHash": "0x3333333333333333333333333333333333333333333333333333333333333333",
        "transactions": [
            "0x02f86c0102843b9aca0085029e7822d68252089abcdef1234567890abcdef1234567890abcdef12880de0b6b3a764000080c001a0c305c89c6e9b72fe7bcbdbbb6c7b5a2b8c5c8b9a4c7b3a9c8b2d1e0f9g8h7i6j5k4l3m2n1o0p9q8r7s6t5u4v3w2x1y0z9"
        ],
        "withdrawals": [
            {
                "index": "0x0",
                "validatorIndex": "0x3039",
                "address": "0x1234567890abcdef1234567890abcdef12345678",
                "amount": "0xde0b6b3a7640000"
            }
        ],
        "blobGasUsed": "0x20000",
        "excessBlobGas": "0x0"
    })
}

fn create_mock_forkchoice_state() -> Value {
    json!({
        "headBlockHash": "0x3333333333333333333333333333333333333333333333333333333333333333",
        "safeBlockHash": "0x2222222222222222222222222222222222222222222222222222222222222222",
        "finalizedBlockHash": "0x1111111111111111111111111111111111111111111111111111111111111111"
    })
}
