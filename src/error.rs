use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageErrorKind {
    Transient,
    Permanent,
}

#[derive(Debug, Clone)]
pub struct KyaStorageError {
    pub kind:    StorageErrorKind,
    pub message: String,
}

impl PartialEq for KyaStorageError {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

impl Eq for KyaStorageError {}

impl KyaStorageError {
    pub fn transient(msg: impl Into<String>) -> Self {
        Self { kind: StorageErrorKind::Transient, message: msg.into() }
    }

    pub fn permanent(msg: impl Into<String>) -> Self {
        Self { kind: StorageErrorKind::Permanent, message: msg.into() }
    }

    pub fn is_transient(&self) -> bool {
        self.kind == StorageErrorKind::Transient
    }
}

impl std::fmt::Display for KyaStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.kind {
            StorageErrorKind::Transient => "transient",
            StorageErrorKind::Permanent => "permanent",
        };
        write!(f, "{label} storage error: {}", self.message)
    }
}

impl std::error::Error for KyaStorageError {}

#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum KyaError {
    #[error("delegation chain is empty")]
    EmptyChain,

    #[error("storage backend failure: {0}")]
    StorageFailure(KyaStorageError),

    #[error("chain does not anchor to the declared principal")]
    RootMismatch,

    #[error("delegation linkage broken at hop {0}")]
    BrokenLinkage(usize),

    #[error("invalid signature at hop {0}")]
    InvalidSignature(usize),

    #[error("delegation at hop {0} not yet valid (issued_at={1}, now={2})")]
    NotYetValid(usize, u64, u64),

    #[error("delegation at hop {0} has expired (expiry={1}, now={2})")]
    Expired(usize, u64, u64),

    #[error("temporal violation at hop {0}: child expiry {1} exceeds parent expiry {2}")]
    TemporalViolation(usize, u64, u64),

    #[error("depth limit exceeded at hop {0} (limit={1})")]
    MaxDepthExceeded(usize, u8),

    #[error("sub-scope proof is structurally invalid")]
    InvalidSubScopeProof,

    #[error("scope escalation at hop {0}: delegated scope is not within the delegator's authorization")]
    ScopeEscalation(usize),

    #[error("executing agent is not the terminal delegate")]
    UnauthorizedLeaf,

    #[error("execution intent is not within the terminal scope")]
    ScopeViolation,

    #[error("nonce has already been consumed")]
    NonceReplay,

    #[error("delegation certificate has been revoked")]
    Revoked,

    #[error("intent is not present in this tree")]
    IntentNotFound,

    #[error("intent tree requires at least one intent")]
    EmptyTree,

    #[error("wire format error: {0}")]
    WireFormatError(String),

    #[error("unsupported certificate version: expected {expected}, got {got}")]
    UnsupportedVersion { expected: u8, got: u8 },

    #[error("policy violation: {0}")]
    PolicyViolation(String),

    #[error("batch authorization failed at index {index}: {reason}")]
    BatchItemFailed { index: usize, reason: String },

    #[error("MAC verification failed")]
    MacVerificationFailed,

    #[error("namespace mismatch: chain namespace is '{chain}', authorization requested for '{requested}'")]
    NamespaceMismatch { chain: String, requested: String },

    #[error("rate limit exceeded for key")]
    RateLimitExceeded,

    #[error("storage health check failed: {0}")]
    StorageUnhealthy(String),
}

impl KyaError {
    pub fn as_storage_error(&self) -> Option<&KyaStorageError> {
        if let Self::StorageFailure(e) = self { Some(e) } else { None }
    }

    pub fn is_transient_storage_failure(&self) -> bool {
        self.as_storage_error().map_or(false, |e| e.is_transient())
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Expired(..)              => "CERT_EXPIRED",
            Self::Revoked                  => "CERT_REVOKED",
            Self::NonceReplay              => "NONCE_REPLAY",
            Self::ScopeViolation           => "SCOPE_VIOLATION",
            Self::ScopeEscalation(_)       => "SCOPE_ESCALATION",
            Self::InvalidSignature(_)      => "INVALID_SIGNATURE",
            Self::BrokenLinkage(_)         => "CHAIN_BROKEN_LINKAGE",
            Self::MaxDepthExceeded(..)     => "CHAIN_DEPTH_EXCEEDED",
            Self::PolicyViolation(_)       => "POLICY_VIOLATION",
            Self::StorageFailure(_)        => "STORAGE_ERROR",
            Self::BatchItemFailed { .. }   => "BATCH_ITEM_FAILED",
            Self::MacVerificationFailed    => "MAC_VERIFICATION_FAILED",
            Self::NamespaceMismatch { .. } => "NAMESPACE_MISMATCH",
            Self::RateLimitExceeded        => "RATE_LIMIT_EXCEEDED",
            Self::StorageUnhealthy(_)      => "STORAGE_UNHEALTHY",
            _                              => "AUTHORIZATION_FAILED",
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            Self::StorageFailure(e) if e.is_transient() => 503,
            Self::StorageFailure(_)                      => 500,
            Self::StorageUnhealthy(_)                    => 503,
            Self::RateLimitExceeded                      => 429,
            Self::EmptyChain | Self::WireFormatError(_) | Self::UnsupportedVersion { .. } => 400,
            Self::Revoked | Self::Expired(..) | Self::NotYetValid(..) | Self::NonceReplay => 401,
            Self::ScopeViolation | Self::ScopeEscalation(_) | Self::UnauthorizedLeaf     => 403,
            Self::InvalidSignature(_) | Self::RootMismatch | Self::BrokenLinkage(_)      => 403,
            Self::PolicyViolation(_) | Self::NamespaceMismatch { .. }                    => 403,
            Self::MacVerificationFailed                                                   => 401,
            _                                                                             => 403,
        }
    }
}