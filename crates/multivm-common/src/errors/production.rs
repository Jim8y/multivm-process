//! Production-ready error handling system
//!
//! This module provides a comprehensive error handling system that replaces
//! simple string-based errors with structured, actionable error types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, SystemTime};
use thiserror::Error;

/// Production-ready error type with structured information
#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum ProductionError {
    /// Configuration errors
    #[error("Configuration error: {message}")]
    Configuration {
        message: String,
        field: Option<String>,
        expected: Option<String>,
        actual: Option<String>,
        suggestions: Vec<String>,
        error_code: String,
    },

    /// Network and connectivity errors
    #[error("Network error: {message}")]
    Network {
        message: String,
        error_type: NetworkErrorType,
        endpoint: Option<String>,
        retry_after: Option<Duration>,
        permanent: bool,
        error_code: String,
    },

    /// Process management errors
    #[error("Process error: {message}")]
    Process {
        message: String,
        process_id: Option<String>,
        exit_code: Option<i32>,
        signal: Option<String>,
        recoverable: bool,
        error_code: String,
    },

    /// Authentication and authorization errors
    #[error("Authentication error: {message}")]
    Authentication {
        message: String,
        auth_type: AuthenticationType,
        required_permissions: Vec<String>,
        retry_allowed: bool,
        error_code: String,
    },

    /// Validation errors
    #[error("Validation error: {message}")]
    Validation {
        message: String,
        field: String,
        constraint: String,
        actual_value: Option<String>,
        suggestions: Vec<String>,
        error_code: String,
    },

    /// Rate limiting errors
    #[error("Rate limit exceeded: {message}")]
    RateLimit {
        message: String,
        limit_type: RateLimitType,
        current_rate: f64,
        limit: f64,
        reset_time: SystemTime,
        retry_after: Duration,
        error_code: String,
    },

    /// Resource errors (memory, disk, etc.)
    #[error("Resource error: {message}")]
    Resource {
        message: String,
        resource_type: ResourceType,
        current_usage: Option<f64>,
        limit: Option<f64>,
        unit: Option<String>,
        action_required: String,
        error_code: String,
    },

    /// Database and storage errors
    #[error("Storage error: {message}")]
    Storage {
        message: String,
        storage_type: StorageType,
        operation: String,
        key: Option<String>,
        recoverable: bool,
        retry_strategy: Option<RetryStrategy>,
        error_code: String,
    },

    /// Consensus and blockchain errors
    #[error("Consensus error: {message}")]
    Consensus {
        message: String,
        consensus_type: ConsensusErrorType,
        block_height: Option<u64>,
        validator_id: Option<String>,
        recoverable: bool,
        error_code: String,
    },

    /// Cross-VM transaction errors
    #[error("Cross-VM transaction error: {message}")]
    CrossVmTransaction {
        message: String,
        transaction_id: Option<String>,
        source_vm: Option<String>,
        target_vm: Option<String>,
        stage: TransactionStage,
        recoverable: bool,
        rollback_required: bool,
        error_code: String,
    },

    /// IPC communication errors
    #[error("IPC error: {message}")]
    Ipc {
        message: String,
        ipc_type: IpcErrorType,
        endpoint: Option<String>,
        command: Option<String>,
        recoverable: bool,
        retry_strategy: Option<RetryStrategy>,
        error_code: String,
    },

    /// Security and cryptographic errors
    #[error("Security error: {message}")]
    Security {
        message: String,
        security_type: SecurityErrorType,
        severity: SecuritySeverity,
        affected_resources: Vec<String>,
        action_required: String,
        error_code: String,
    },

    /// Timeout errors
    #[error("Timeout error: {message}")]
    Timeout {
        message: String,
        operation: String,
        timeout_duration: Duration,
        elapsed_time: Duration,
        retry_recommended: bool,
        error_code: String,
    },

    /// Serialization and data format errors
    #[error("Serialization error: {message}")]
    Serialization {
        message: String,
        format: String,
        data_type: Option<String>,
        position: Option<usize>,
        context: Option<String>,
        error_code: String,
    },

    /// Business logic errors
    #[error("Business logic error: {message}")]
    BusinessLogic {
        message: String,
        rule: String,
        context: HashMap<String, String>,
        user_action_required: bool,
        error_code: String,
    },

    /// External service errors
    #[error("External service error: {message}")]
    ExternalService {
        message: String,
        service_name: String,
        service_endpoint: Option<String>,
        http_status: Option<u16>,
        service_error_code: Option<String>,
        retry_strategy: Option<RetryStrategy>,
        error_code: String,
    },

    /// Internal system errors
    #[error("Internal error: {message}")]
    Internal {
        message: String,
        component: String,
        operation: String,
        context: HashMap<String, String>,
        trace_id: Option<String>,
        error_code: String,
    },
}

/// Network error types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NetworkErrorType {
    ConnectionRefused,
    ConnectionTimeout,
    ConnectionLost,
    DnsResolution,
    SslHandshake,
    InvalidCertificate,
    NetworkUnreachable,
    ProtocolError,
    MessageTooLarge,
    Bandwidth,
}

/// Authentication types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuthenticationType {
    ApiKey,
    Jwt,
    Signature,
    Certificate,
    OAuth,
    Basic,
    Bearer,
}

/// Rate limit types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RateLimitType {
    RequestsPerSecond,
    RequestsPerMinute,
    RequestsPerHour,
    RequestsPerDay,
    BytesPerSecond,
    TransactionsPerBlock,
    ConcurrentConnections,
}

/// Resource types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResourceType {
    Memory,
    Disk,
    Cpu,
    NetworkBandwidth,
    FileDescriptors,
    ThreadPool,
    ConnectionPool,
    QueueCapacity,
}

/// Storage types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StorageType {
    Database,
    Cache,
    FileSystem,
    ObjectStorage,
    InMemory,
    Blockchain,
}

/// Consensus error types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsensusErrorType {
    InvalidBlock,
    InvalidTransaction,
    ForkDetected,
    ValidatorSlashing,
    InsufficientStake,
    TimeoutReached,
    NetworkPartition,
    InvalidProposal,
    DoubleSign,
}

/// Transaction stages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStage {
    Validation,
    Preparation,
    Execution,
    Confirmation,
    Finalization,
    Rollback,
}

/// IPC error types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IpcErrorType {
    ConnectionFailed,
    MessageCorrupted,
    ProtocolMismatch,
    CommandNotSupported,
    ResponseTimeout,
    ChannelClosed,
    SerializationFailed,
    AuthenticationFailed,
}

/// Security error types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecurityErrorType {
    InvalidSignature,
    ExpiredCertificate,
    UnauthorizedAccess,
    InvalidToken,
    EncryptionFailed,
    KeyNotFound,
    TamperedData,
    SecurityPolicyViolation,
    SuspiciousActivity,
}

/// Security severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecuritySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Retry strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryStrategy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
    pub retry_on_errors: Vec<String>,
}

/// Error context for enhanced debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub timestamp: SystemTime,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub request_id: Option<String>,
    pub component: String,
    pub operation: String,
    pub metadata: HashMap<String, String>,
}

/// Error recovery suggestions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecovery {
    pub is_recoverable: bool,
    pub automatic_retry: bool,
    pub user_action_required: bool,
    pub suggested_actions: Vec<String>,
    pub retry_strategy: Option<RetryStrategy>,
    pub escalation_path: Option<String>,
}

/// Structured error with full context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredError {
    pub error: ProductionError,
    pub context: ErrorContext,
    pub recovery: ErrorRecovery,
    pub correlation_id: String,
    pub stack_trace: Option<Vec<String>>,
    pub related_errors: Vec<String>,
}

impl ProductionError {
    /// Create a configuration error with detailed information
    pub fn configuration<S: Into<String>>(
        message: S,
        field: Option<S>,
        expected: Option<S>,
        actual: Option<S>,
    ) -> Self {
        Self::Configuration {
            message: message.into(),
            field: field.map(|s| s.into()),
            expected: expected.map(|s| s.into()),
            actual: actual.map(|s| s.into()),
            suggestions: vec![],
            error_code: Self::generate_error_code("CONFIG"),
        }
    }

    /// Create a network error with retry information
    pub fn network<S: Into<String>>(
        message: S,
        error_type: NetworkErrorType,
        endpoint: Option<S>,
        retry_after: Option<Duration>,
    ) -> Self {
        Self::Network {
            message: message.into(),
            error_type,
            endpoint: endpoint.map(|s| s.into()),
            retry_after,
            permanent: false,
            error_code: Self::generate_error_code("NETWORK"),
        }
    }

    /// Create a process error
    pub fn process<S: Into<String>>(
        message: S,
        process_id: Option<S>,
        exit_code: Option<i32>,
        recoverable: bool,
    ) -> Self {
        Self::Process {
            message: message.into(),
            process_id: process_id.map(|s| s.into()),
            exit_code,
            signal: None,
            recoverable,
            error_code: Self::generate_error_code("PROCESS"),
        }
    }

    /// Create a validation error
    pub fn validation<S: Into<String>>(
        message: S,
        field: S,
        constraint: S,
        actual_value: Option<S>,
    ) -> Self {
        Self::Validation {
            message: message.into(),
            field: field.into(),
            constraint: constraint.into(),
            actual_value: actual_value.map(|s| s.into()),
            suggestions: vec![],
            error_code: Self::generate_error_code("VALIDATION"),
        }
    }

    /// Create a rate limit error
    pub fn rate_limit<S: Into<String>>(
        message: S,
        limit_type: RateLimitType,
        current_rate: f64,
        limit: f64,
        retry_after: Duration,
    ) -> Self {
        Self::RateLimit {
            message: message.into(),
            limit_type,
            current_rate,
            limit,
            reset_time: SystemTime::now() + retry_after,
            retry_after,
            error_code: Self::generate_error_code("RATE_LIMIT"),
        }
    }

    /// Create a cross-VM transaction error
    pub fn cross_vm_transaction<S: Into<String>>(
        message: S,
        transaction_id: Option<S>,
        source_vm: Option<S>,
        target_vm: Option<S>,
        stage: TransactionStage,
    ) -> Self {
        Self::CrossVmTransaction {
            message: message.into(),
            transaction_id: transaction_id.map(|s| s.into()),
            source_vm: source_vm.map(|s| s.into()),
            target_vm: target_vm.map(|s| s.into()),
            stage,
            recoverable: stage != TransactionStage::Finalization,
            rollback_required: matches!(stage, TransactionStage::Execution | TransactionStage::Confirmation),
            error_code: Self::generate_error_code("CROSS_VM_TX"),
        }
    }

    /// Create a security error
    pub fn security<S: Into<String>>(
        message: S,
        security_type: SecurityErrorType,
        severity: SecuritySeverity,
        action_required: S,
    ) -> Self {
        Self::Security {
            message: message.into(),
            security_type,
            severity,
            affected_resources: vec![],
            action_required: action_required.into(),
            error_code: Self::generate_error_code("SECURITY"),
        }
    }

    /// Generate unique error code
    fn generate_error_code(prefix: &str) -> String {
        use std::time::UNIX_EPOCH;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("{}-{:016X}", prefix, timestamp)
    }

    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Configuration { .. } => false,
            Self::Network { retry_after, permanent, .. } => retry_after.is_some() && !permanent,
            Self::Process { recoverable, .. } => *recoverable,
            Self::Authentication { retry_allowed, .. } => *retry_allowed,
            Self::Validation { .. } => false,
            Self::RateLimit { .. } => true,
            Self::Resource { .. } => false,
            Self::Storage { recoverable, .. } => *recoverable,
            Self::Consensus { recoverable, .. } => *recoverable,
            Self::CrossVmTransaction { recoverable, .. } => *recoverable,
            Self::Ipc { recoverable, .. } => *recoverable,
            Self::Security { severity, .. } => !matches!(severity, SecuritySeverity::Critical),
            Self::Timeout { retry_recommended, .. } => *retry_recommended,
            Self::Serialization { .. } => false,
            Self::BusinessLogic { .. } => false,
            Self::ExternalService { retry_strategy, .. } => retry_strategy.is_some(),
            Self::Internal { .. } => false,
        }
    }

    /// Get suggested retry strategy
    pub fn retry_strategy(&self) -> Option<RetryStrategy> {
        match self {
            Self::Network { retry_after, .. } => {
                if let Some(delay) = retry_after {
                    Some(RetryStrategy {
                        max_attempts: 3,
                        initial_delay: *delay,
                        max_delay: Duration::from_secs(60),
                        backoff_multiplier: 2.0,
                        jitter: true,
                        retry_on_errors: vec!["NETWORK".to_string()],
                    })
                } else {
                    None
                }
            }
            Self::RateLimit { retry_after, .. } => Some(RetryStrategy {
                max_attempts: 5,
                initial_delay: *retry_after,
                max_delay: Duration::from_secs(300),
                backoff_multiplier: 1.5,
                jitter: true,
                retry_on_errors: vec!["RATE_LIMIT".to_string()],
            }),
            Self::Storage { retry_strategy, .. } => retry_strategy.clone(),
            Self::ExternalService { retry_strategy, .. } => retry_strategy.clone(),
            Self::Timeout { .. } => Some(RetryStrategy {
                max_attempts: 2,
                initial_delay: Duration::from_secs(1),
                max_delay: Duration::from_secs(30),
                backoff_multiplier: 2.0,
                jitter: false,
                retry_on_errors: vec!["TIMEOUT".to_string()],
            }),
            _ => None,
        }
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Configuration { .. } => ErrorSeverity::High,
            Self::Network { permanent, .. } => if *permanent { ErrorSeverity::High } else { ErrorSeverity::Medium },
            Self::Process { recoverable, .. } => if *recoverable { ErrorSeverity::Medium } else { ErrorSeverity::High },
            Self::Authentication { .. } => ErrorSeverity::High,
            Self::Validation { .. } => ErrorSeverity::Medium,
            Self::RateLimit { .. } => ErrorSeverity::Low,
            Self::Resource { .. } => ErrorSeverity::High,
            Self::Storage { .. } => ErrorSeverity::Medium,
            Self::Consensus { .. } => ErrorSeverity::Critical,
            Self::CrossVmTransaction { .. } => ErrorSeverity::High,
            Self::Ipc { .. } => ErrorSeverity::Medium,
            Self::Security { severity, .. } => match severity {
                SecuritySeverity::Low => ErrorSeverity::Low,
                SecuritySeverity::Medium => ErrorSeverity::Medium,
                SecuritySeverity::High => ErrorSeverity::High,
                SecuritySeverity::Critical => ErrorSeverity::Critical,
            },
            Self::Timeout { .. } => ErrorSeverity::Medium,
            Self::Serialization { .. } => ErrorSeverity::Medium,
            Self::BusinessLogic { .. } => ErrorSeverity::Medium,
            Self::ExternalService { .. } => ErrorSeverity::Medium,
            Self::Internal { .. } => ErrorSeverity::High,
        }
    }

    /// Get user-friendly message
    pub fn user_message(&self) -> String {
        match self {
            Self::Configuration { message, suggestions, .. } => {
                if suggestions.is_empty() {
                    format!("Configuration error: {}", message)
                } else {
                    format!("Configuration error: {}. Suggestions: {}", message, suggestions.join(", "))
                }
            }
            Self::Network { message, retry_after, .. } => {
                if let Some(delay) = retry_after {
                    format!("Network error: {}. Please retry in {} seconds.", message, delay.as_secs())
                } else {
                    format!("Network error: {}. Please check your connection.", message)
                }
            }
            Self::RateLimit { message, retry_after, .. } => {
                format!("Rate limit exceeded: {}. Please wait {} seconds before retrying.", 
                       message, retry_after.as_secs())
            }
            Self::Authentication { message, .. } => {
                format!("Authentication failed: {}. Please check your credentials.", message)
            }
            Self::Validation { message, suggestions, .. } => {
                if suggestions.is_empty() {
                    format!("Validation error: {}", message)
                } else {
                    format!("Validation error: {}. Suggestions: {}", message, suggestions.join(", "))
                }
            }
            _ => self.to_string(),
        }
    }

    /// Add suggestion to error
    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        match &mut self {
            Self::Configuration { suggestions, .. } => suggestions.push(suggestion),
            Self::Validation { suggestions, .. } => suggestions.push(suggestion),
            _ => {}
        }
        self
    }

    /// Mark error as permanent
    pub fn as_permanent(mut self) -> Self {
        match &mut self {
            Self::Network { permanent, .. } => *permanent = true,
            _ => {}
        }
        self
    }

    /// Add affected resource to security error
    pub fn with_affected_resource(mut self, resource: String) -> Self {
        if let Self::Security { affected_resources, .. } = &mut self {
            affected_resources.push(resource);
        }
        self
    }
}

/// Error severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Error aggregator for collecting and analyzing errors
#[derive(Debug)]
pub struct ErrorAggregator {
    errors: Vec<StructuredError>,
    error_counts: HashMap<String, u64>,
    error_patterns: HashMap<String, ErrorPattern>,
}

/// Error pattern for detecting recurring issues
#[derive(Debug, Clone)]
pub struct ErrorPattern {
    pub pattern_id: String,
    pub error_type: String,
    pub count: u64,
    pub first_seen: SystemTime,
    pub last_seen: SystemTime,
    pub frequency: f64,
    pub trend: ErrorTrend,
}

/// Error trend analysis
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorTrend {
    Increasing,
    Decreasing,
    Stable,
    Spike,
}

impl ErrorAggregator {
    /// Create new error aggregator
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            error_counts: HashMap::new(),
            error_patterns: HashMap::new(),
        }
    }

    /// Add error to aggregator
    pub fn add_error(&mut self, error: StructuredError) {
        let error_type = format!("{:?}", error.error);
        
        // Update counts
        *self.error_counts.entry(error_type.clone()).or_insert(0) += 1;
        
        // Update patterns
        let pattern_id = self.generate_pattern_id(&error);
        self.error_patterns
            .entry(pattern_id.clone())
            .and_modify(|pattern| {
                pattern.count += 1;
                pattern.last_seen = SystemTime::now();
                pattern.frequency = self.calculate_frequency(pattern);
                pattern.trend = self.calculate_trend(pattern);
            })
            .or_insert_with(|| ErrorPattern {
                pattern_id: pattern_id.clone(),
                error_type: error_type.clone(),
                count: 1,
                first_seen: SystemTime::now(),
                last_seen: SystemTime::now(),
                frequency: 0.0,
                trend: ErrorTrend::Stable,
            });
        
        self.errors.push(error);
    }

    /// Generate pattern ID for error
    fn generate_pattern_id(&self, error: &StructuredError) -> String {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        
        format!("{:?}", error.error).hash(&mut hasher);
        error.context.component.hash(&mut hasher);
        error.context.operation.hash(&mut hasher);
        
        format!("pattern_{:016x}", hasher.finish())
    }

    /// Calculate error frequency
    fn calculate_frequency(&self, pattern: &ErrorPattern) -> f64 {
        let duration = SystemTime::now()
            .duration_since(pattern.first_seen)
            .unwrap_or_default()
            .as_secs() as f64;
        
        if duration > 0.0 {
            pattern.count as f64 / duration
        } else {
            0.0
        }
    }

    /// Calculate error trend
    fn calculate_trend(&self, _pattern: &ErrorPattern) -> ErrorTrend {
        // Simplified trend analysis
        // Real implementation would analyze historical data
        ErrorTrend::Stable
    }

    /// Get error summary
    pub fn get_summary(&self) -> ErrorSummary {
        let total_errors = self.errors.len();
        let unique_errors = self.error_counts.len();
        let critical_errors = self.errors.iter()
            .filter(|e| e.error.severity() == ErrorSeverity::Critical)
            .count();
        
        let top_errors: Vec<(String, u64)> = {
            let mut counts: Vec<_> = self.error_counts.iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect();
            counts.sort_by(|a, b| b.1.cmp(&a.1));
            counts.into_iter().take(10).collect()
        };

        ErrorSummary {
            total_errors,
            unique_errors,
            critical_errors,
            top_errors,
            patterns: self.error_patterns.values().cloned().collect(),
        }
    }
}

/// Error summary for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSummary {
    pub total_errors: usize,
    pub unique_errors: usize,
    pub critical_errors: usize,
    pub top_errors: Vec<(String, u64)>,
    pub patterns: Vec<ErrorPattern>,
}

impl Default for ErrorAggregator {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
            retry_on_errors: vec![],
        }
    }
}

/// Error builder for constructing complex errors
pub struct ErrorBuilder {
    error: ProductionError,
    context: HashMap<String, String>,
    suggestions: Vec<String>,
    affected_resources: Vec<String>,
}

impl ErrorBuilder {
    /// Start building a configuration error
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self {
            error: ProductionError::Configuration {
                message: message.into(),
                field: None,
                expected: None,
                actual: None,
                suggestions: vec![],
                error_code: ProductionError::generate_error_code("CONFIG"),
            },
            context: HashMap::new(),
            suggestions: vec![],
            affected_resources: vec![],
        }
    }

    /// Add field information
    pub fn field<S: Into<String>>(mut self, field: S, expected: Option<S>, actual: Option<S>) -> Self {
        if let ProductionError::Configuration { field: f, expected: e, actual: a, .. } = &mut self.error {
            *f = Some(field.into());
            *e = expected.map(|s| s.into());
            *a = actual.map(|s| s.into());
        }
        self
    }

    /// Add suggestion
    pub fn suggestion<S: Into<String>>(mut self, suggestion: S) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    /// Add context
    pub fn context<S: Into<String>>(mut self, key: S, value: S) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Build the final error
    pub fn build(mut self) -> ProductionError {
        // Apply suggestions to error
        match &mut self.error {
            ProductionError::Configuration { suggestions, .. } => {
                suggestions.extend(self.suggestions);
            }
            ProductionError::Validation { suggestions, .. } => {
                suggestions.extend(self.suggestions);
            }
            ProductionError::Security { affected_resources, .. } => {
                affected_resources.extend(self.affected_resources);
            }
            _ => {}
        }
        
        self.error
    }
}

/// Macro for creating structured errors with context
#[macro_export]
macro_rules! production_error {
    (config: $msg:expr) => {
        $crate::errors::production::ProductionError::configuration($msg, None::<String>, None::<String>, None::<String>)
    };
    (config: $msg:expr, field: $field:expr) => {
        $crate::errors::production::ProductionError::configuration($msg, Some($field), None::<String>, None::<String>)
    };
    (network: $msg:expr, $error_type:expr) => {
        $crate::errors::production::ProductionError::network($msg, $error_type, None::<String>, None)
    };
    (validation: $msg:expr, field: $field:expr, constraint: $constraint:expr) => {
        $crate::errors::production::ProductionError::validation($msg, $field, $constraint, None::<String>)
    };
    (security: $msg:expr, $sec_type:expr, $severity:expr, action: $action:expr) => {
        $crate::errors::production::ProductionError::security($msg, $sec_type, $severity, $action)
    };
}

/// Result type using production errors
pub type ProductionResult<T> = Result<T, ProductionError>;

/// Convert from legacy MultivmError to ProductionError
impl From<crate::MultivmError> for ProductionError {
    fn from(error: crate::MultivmError) -> Self {
        match error {
            crate::MultivmError::Configuration(msg) => {
                Self::configuration(msg, None::<String>, None::<String>, None::<String>)
            }
            crate::MultivmError::Network(msg) => {
                Self::network(msg, NetworkErrorType::ConnectionRefused, None::<String>, Some(Duration::from_secs(5)))
            }
            crate::MultivmError::Process(msg) => {
                Self::process(msg, None::<String>, None, true)
            }
            crate::MultivmError::Validation(msg) => {
                Self::validation(msg, "unknown", "validation failed", None::<String>)
            }
            crate::MultivmError::RateLimited(msg) => {
                Self::rate_limit(msg, RateLimitType::RequestsPerMinute, 0.0, 60.0, Duration::from_secs(60))
            }
            crate::MultivmError::Ipc(msg) => {
                Self::Ipc {
                    message: msg,
                    ipc_type: IpcErrorType::ConnectionFailed,
                    endpoint: None,
                    command: None,
                    recoverable: true,
                    retry_strategy: Some(RetryStrategy::default()),
                    error_code: Self::generate_error_code("IPC"),
                }
            }
            crate::MultivmError::Timeout { timeout } => {
                Self::Timeout {
                    message: format!("Operation timed out after {}s", timeout.as_secs()),
                    operation: "unknown".to_string(),
                    timeout_duration: timeout,
                    elapsed_time: timeout,
                    retry_recommended: true,
                    error_code: Self::generate_error_code("TIMEOUT"),
                }
            }
            _ => Self::Internal {
                message: error.to_string(),
                component: "legacy".to_string(),
                operation: "conversion".to_string(),
                context: HashMap::new(),
                trace_id: None,
                error_code: Self::generate_error_code("INTERNAL"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = ProductionError::configuration(
            "Invalid port number",
            Some("port"),
            Some("1-65535"),
            Some("0"),
        );
        
        assert!(matches!(error, ProductionError::Configuration { .. }));
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_error_builder() {
        let error = ErrorBuilder::configuration("Missing required field")
            .field("database_url", Some("valid URL"), Some("empty string"))
            .suggestion("Set DATABASE_URL environment variable")
            .suggestion("Use default sqlite://./data.db")
            .build();
        
        if let ProductionError::Configuration { suggestions, .. } = error {
            assert_eq!(suggestions.len(), 2);
        } else {
            panic!("Expected Configuration error");
        }
    }

    #[test]
    fn test_retry_strategy() {
        let error = ProductionError::network(
            "Connection failed",
            NetworkErrorType::ConnectionTimeout,
            Some("http://example.com"),
            Some(Duration::from_secs(5)),
        );
        
        let retry = error.retry_strategy();
        assert!(retry.is_some());
        assert_eq!(retry.unwrap().max_attempts, 3);
    }

    #[test]
    fn test_error_aggregator() {
        let mut aggregator = ErrorAggregator::new();
        
        let error = StructuredError {
            error: ProductionError::configuration("Test error", None, None, None),
            context: ErrorContext {
                timestamp: SystemTime::now(),
                trace_id: None,
                span_id: None,
                user_id: None,
                session_id: None,
                request_id: None,
                component: "test".to_string(),
                operation: "test".to_string(),
                metadata: HashMap::new(),
            },
            recovery: ErrorRecovery {
                is_recoverable: false,
                automatic_retry: false,
                user_action_required: true,
                suggested_actions: vec![],
                retry_strategy: None,
                escalation_path: None,
            },
            correlation_id: "test-123".to_string(),
            stack_trace: None,
            related_errors: vec![],
        };
        
        aggregator.add_error(error);
        
        let summary = aggregator.get_summary();
        assert_eq!(summary.total_errors, 1);
        assert_eq!(summary.unique_errors, 1);
    }
}