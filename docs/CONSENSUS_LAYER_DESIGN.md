# MultiVM Consensus Layer (Layer 2) Design Specification

## 概述

共识层是MultiVM架构的核心组件，负责在支持多个虚拟机的分布式网络中达成一致性。该层提供统一的共识接口，支持多种共识算法，并确保跨VM状态的一致性。

## 设计目标

### 1. 统一共识接口
- 支持多种共识算法 (PBFT, Raft, HotStuff等)
- 算法可热插拔，支持运行时切换
- 为不同VM类型提供一致的共识服务

### 2. 跨VM一致性
- 保证SVM和EVM状态变更的原子性
- 支持跨VM交易的分布式提交
- 维护全局状态的一致性视图

### 3. 高性能与可扩展性
- 支持高吞吐量的交易处理
- 可扩展的节点管理
- 优化的消息传递机制

### 4. 容错与恢复
- 拜占庭容错能力
- 自动故障检测与恢复
- 数据完整性保证

## 架构组件

### 核心接口

#### 1. ConsensusEngine Trait
```rust
pub trait ConsensusEngine: Send + Sync {
    type Block;
    type Transaction;
    type Error;
    
    async fn propose_block(&self, transactions: Vec<Self::Transaction>) -> Result<Self::Block, Self::Error>;
    async fn validate_block(&self, block: &Self::Block) -> Result<bool, Self::Error>;
    async fn commit_block(&self, block: Self::Block) -> Result<(), Self::Error>;
    async fn get_current_height(&self) -> Result<u64, Self::Error>;
}
```

#### 2. MultiVMConsensus 统一管理器
```rust
pub struct MultiVMConsensus {
    consensus_engine: Box<dyn ConsensusEngine>,
    svm_interface: SvmConsensusInterface,
    evm_interface: EvmConsensusInterface,
    p2p_network: Arc<dyn P2PNetworkLayer>,
    state_manager: StateManager,
}
```

### 共识算法实现

#### 1. PBFT (Practical Byzantine Fault Tolerance)
- **适用场景**: 许可网络，强一致性要求
- **特点**: 3f+1节点容忍f个拜占庭故障
- **实现**: 三阶段协议 (pre-prepare, prepare, commit)

#### 2. Raft
- **适用场景**: 私有网络，配置管理
- **特点**: 强领导者，简单实现
- **实现**: 领导者选举 + 日志复制

#### 3. HotStuff
- **适用场景**: 高性能区块链网络
- **特点**: 线性视图变更，高吞吐量
- **实现**: 三阶段BFT with pipelining

### 状态管理

#### 1. 跨VM状态协调
```rust
pub struct CrossVMState {
    svm_state_root: H256,
    evm_state_root: H256,
    multivm_bindings: HashMap<MultivmAccountId, AccountBinding>,
    global_nonce: u64,
    timestamp: SystemTime,
}
```

#### 2. 事务处理模型
- **原子性保证**: 跨VM交易要么全部成功，要么全部失败
- **隔离级别**: 读已提交 + 快照隔离
- **持久性**: 状态变更持久化到共识日志

### 消息类型

#### 1. 共识消息
```rust
pub enum ConsensusMessage {
    Proposal {
        height: u64,
        round: u32,
        block: MultiVMBlock,
        proposer: NodeId,
        signature: Signature,
    },
    Vote {
        height: u64,
        round: u32,
        block_hash: H256,
        vote_type: VoteType,
        voter: NodeId,
        signature: Signature,
    },
    ViewChange {
        height: u64,
        new_view: u32,
        justification: ViewChangeJustification,
        node: NodeId,
    },
}
```

#### 2. 状态同步消息
```rust
pub enum StateSyncMessage {
    StateRequest {
        height: u64,
        vm_type: VmType,
        chunk_index: u32,
    },
    StateResponse {
        height: u64,
        vm_type: VmType,
        chunk_data: Vec<u8>,
        proof: StateProof,
    },
    SyncComplete {
        height: u64,
        state_root: H256,
    },
}
```

## 数据结构

### 1. MultiVM区块
```rust
pub struct MultiVMBlock {
    pub header: BlockHeader,
    pub svm_transactions: Vec<SvmTransaction>,
    pub evm_transactions: Vec<EvmTransaction>,
    pub multivm_transactions: Vec<SpecialTransaction>,
    pub state_transitions: Vec<StateTransition>,
}

pub struct BlockHeader {
    pub height: u64,
    pub previous_hash: H256,
    pub state_root: H256,
    pub transactions_root: H256,
    pub timestamp: SystemTime,
    pub proposer: NodeId,
    pub consensus_data: Vec<u8>,
}
```

### 2. 共识状态
```rust
pub struct ConsensusState {
    pub current_height: u64,
    pub current_round: u32,
    pub current_view: u32,
    pub locked_block: Option<H256>,
    pub locked_round: Option<u32>,
    pub valid_block: Option<H256>,
    pub valid_round: Option<u32>,
    pub votes: HashMap<(u64, u32), VoteSet>,
}
```

## 算法实现细节

### PBFT算法流程

#### Phase 1: Pre-prepare
1. 主节点接收交易并创建区块提案
2. 主节点广播 `Pre-prepare` 消息
3. 备份节点验证消息格式和序列号

#### Phase 2: Prepare
1. 备份节点发送 `Prepare` 消息
2. 节点收集prepare消息达到 2f+1
3. 进入prepared状态

#### Phase 3: Commit
1. 节点发送 `Commit` 消息
2. 收集commit消息达到 2f+1
3. 执行区块并更新状态

### 跨VM交易处理

#### 1. 交易验证阶段
```rust
async fn validate_cross_vm_transaction(
    &self,
    tx: &SpecialTransaction,
) -> Result<ValidationResult, ConsensusError> {
    match tx {
        SpecialTransaction::CrossVmTransfer { from, to, amount, .. } => {
            // 验证源账户余额
            let source_balance = self.get_account_balance(from).await?;
            if source_balance < *amount {
                return Ok(ValidationResult::Invalid("Insufficient balance".to_string()));
            }
            
            // 验证目标账户存在
            if !self.account_exists(to).await? {
                return Ok(ValidationResult::Invalid("Target account not found".to_string()));
            }
            
            Ok(ValidationResult::Valid)
        },
        // 其他交易类型的验证逻辑
        _ => Ok(ValidationResult::Valid),
    }
}
```

#### 2. 状态更新阶段
```rust
async fn apply_cross_vm_transaction(
    &mut self,
    tx: &SpecialTransaction,
) -> Result<StateTransition, ConsensusError> {
    let mut transitions = Vec::new();
    
    match tx {
        SpecialTransaction::CrossVmTransfer { from, to, amount, .. } => {
            // 原子性更新两个账户
            let source_transition = self.update_account_balance(from, -*amount).await?;
            let target_transition = self.update_account_balance(to, *amount).await?;
            
            transitions.push(source_transition);
            transitions.push(target_transition);
        },
        // 其他交易类型处理
        _ => {},
    }
    
    Ok(StateTransition::Batch(transitions))
}
```

## 性能优化策略

### 1. 并行处理
- **交易并行验证**: 无依赖交易可并行验证
- **状态并行更新**: 不同VM状态可并行更新
- **消息并行处理**: 网络消息异步处理

### 2. 批处理优化
- **批量交易提交**: 聚合多个交易减少共识轮次
- **状态批量更新**: 批量应用状态变更
- **网络消息批量**: 减少网络开销

### 3. 缓存策略
- **验证结果缓存**: 缓存交易验证结果
- **状态快照缓存**: 缓存状态快照减少计算
- **共识消息缓存**: 避免重复处理

## 容错机制

### 1. 节点故障处理
- **故障检测**: 心跳机制 + 超时检测
- **视图切换**: 主节点故障时自动切换
- **状态恢复**: 从其他节点同步最新状态

### 2. 网络分区处理
- **分区检测**: 检测网络连通性
- **多数分区**: 只有多数分区可以继续工作
- **分区恢复**: 网络恢复后状态同步

### 3. 数据一致性保证
- **检查点机制**: 定期创建状态检查点
- **回滚机制**: 发现不一致时回滚到安全状态
- **完整性验证**: 定期验证数据完整性

## 安全考虑

### 1. 拜占庭攻击防护
- **签名验证**: 所有消息必须正确签名
- **重放攻击防护**: 使用序列号和时间戳
- **女巫攻击防护**: 基于权益的节点选择

### 2. 状态攻击防护
- **状态验证**: 验证状态转换的合法性
- **回滚攻击防护**: 限制回滚深度
- **长程攻击防护**: 检查点 + 弱主观性

## 监控与指标

### 1. 性能指标
- **共识延迟**: 从提案到确认的时间
- **吞吐量**: 每秒处理的交易数
- **网络开销**: 共识消息的网络流量

### 2. 安全指标
- **分叉率**: 临时分叉的频率
- **活跃度**: 网络持续出块的能力
- **安全性**: 最终确认的强度

### 3. 系统指标
- **节点健康度**: 各节点的运行状态
- **资源使用**: CPU、内存、网络使用情况
- **错误率**: 各种错误的发生频率

## 未来扩展

### 1. 新共识算法支持
- **模块化设计**: 易于添加新算法
- **性能对比**: 不同算法的性能测试
- **自适应选择**: 根据场景自动选择算法

### 2. 跨链互操作
- **桥接协议**: 与其他区块链的互操作
- **状态证明**: 生成和验证跨链状态证明
- **原子交换**: 跨链资产交换

### 3. 隐私保护
- **零知识证明**: 保护交易隐私
- **同态加密**: 加密状态下的计算
- **混币协议**: 提高交易匿名性

---

*文档版本: v1.0*  
*创建时间: 2024年12月*  
*MultiVM Architecture Team* 