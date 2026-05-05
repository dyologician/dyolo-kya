use std::sync::Arc;

use ed25519_dalek::VerifyingKey;

use crate::audit::{AuditSink, NoopAuditSink};
use crate::chain::{AuthorizedAction, BatchAuthorizeResult, Clock, DyoloChain, SystemClock};
use crate::error::KyaError;
use crate::intent::{IntentHash, MerkleProof};
use crate::policy::PolicySet;
use crate::registry::{
    MemoryNonceStore, MemoryRevocationStore,
    NonceStore, RevocationStore,
};

// ── Sync context ──────────────────────────────────────────────────────────────

/// A wiring context that holds all runtime dependencies required for chain
/// authorization.
///
/// `KyaContext` is the recommended entry point for applications that do not
/// need fine-grained control over each authorization call. Configure once at
/// startup, share across threads via `Arc<KyaContext>`, and call `authorize`.
///
/// # Example
///
/// ```rust,ignore
/// use dyolo_kya::context::KyaContext;
/// use dyolo_kya::{DyoloChain, intent::{Intent, MerkleProof}};
///
/// let ctx = KyaContext::builder().build();
///
/// let action = ctx.authorize(&chain, &agent_pk, &intent_hash, &proof)?;
/// println!("authorized depth={}", action.receipt().chain_depth);
/// ```
pub struct KyaContext {
    pub revocation:  Arc<dyn RevocationStore>,
    pub nonces:      Arc<dyn NonceStore>,
    pub clock:       Arc<dyn Clock + Send + Sync>,
    pub policy:      Option<PolicySet>,
    pub audit:       Arc<dyn AuditSink>,
    pub namespace:   Option<String>,
}

impl KyaContext {
    pub fn builder() -> KyaContextBuilder {
        KyaContextBuilder::default()
    }

    pub fn authorize(
        &self,
        chain:    &DyoloChain,
        agent_pk: &VerifyingKey,
        intent:   &IntentHash,
        proof:    &MerkleProof,
    ) -> Result<AuthorizedAction, KyaError> {
        chain.authorize_with_options(
            agent_pk, intent, proof,
            self.clock.as_ref(),
            self.revocation.as_ref(),
            self.nonces.as_ref(),
            self.policy.as_ref(),
            self.audit.as_ref(),
        )
    }

    pub fn authorize_batch(
        &self,
        chain:    &DyoloChain,
        agent_pk: &VerifyingKey,
        intents:  &[(IntentHash, MerkleProof)],
    ) -> BatchAuthorizeResult {
        chain.authorize_batch(
            agent_pk, intents,
            self.clock.as_ref(),
            self.revocation.as_ref(),
            self.nonces.as_ref(),
        )
    }

    /// Probe both storage backends. Returns `Err` if either is unhealthy.
    ///
    /// Call this from your process health endpoint so load balancers can drain
    /// a replica before its backing store degrades authorization decisions.
    pub fn health_check(&self) -> Result<(), KyaError> {
        self.revocation
            .health_check()
            .map_err(|e| KyaError::StorageUnhealthy(format!("revocation: {e}")))?;
        self.nonces
            .health_check()
            .map_err(|e| KyaError::StorageUnhealthy(format!("nonces: {e}")))?;
        Ok(())
    }
}

// ── Async context ─────────────────────────────────────────────────────────────

#[cfg(feature = "async")]
pub struct AsyncKyaContext {
    pub revocation: Arc<dyn crate::registry::r#async::AsyncRevocationStore>,
    pub nonces:     Arc<dyn crate::registry::r#async::AsyncNonceStore>,
    pub clock:      Arc<dyn Clock + Send + Sync>,
    pub policy:     Option<PolicySet>,
    pub audit:      Arc<dyn AuditSink>,
    pub namespace:  Option<String>,
}

#[cfg(feature = "async")]
impl AsyncKyaContext {
    pub fn builder() -> AsyncKyaContextBuilder {
        AsyncKyaContextBuilder::default()
    }

    pub async fn authorize(
        &self,
        chain:    &DyoloChain,
        agent_pk: &VerifyingKey,
        intent:   &IntentHash,
        proof:    &MerkleProof,
    ) -> Result<AuthorizedAction, KyaError> {
        chain.authorize_async_with_options(
            agent_pk, intent, proof,
            self.clock.as_ref(),
            self.revocation.as_ref(),
            self.nonces.as_ref(),
            self.policy.as_ref(),
            self.audit.as_ref(),
        ).await
    }

    pub async fn authorize_batch(
        &self,
        chain:    &DyoloChain,
        agent_pk: &VerifyingKey,
        intents:  &[(IntentHash, MerkleProof)],
    ) -> BatchAuthorizeResult {
        chain.authorize_batch_async(
            agent_pk, intents,
            self.clock.as_ref(),
            self.revocation.as_ref(),
            self.nonces.as_ref(),
        ).await
    }

    pub async fn health_check(&self) -> Result<(), KyaError> {
        self.revocation.health_check().await
            .map_err(|e| KyaError::StorageUnhealthy(format!("revocation: {e}")))?;
        self.nonces.health_check().await
            .map_err(|e| KyaError::StorageUnhealthy(format!("nonces: {e}")))?;
        Ok(())
    }
}

// ── KyaContextBuilder ─────────────────────────────────────────────────────────

pub struct KyaContextBuilder {
    revocation: Option<Arc<dyn RevocationStore>>,
    nonces:     Option<Arc<dyn NonceStore>>,
    clock:      Option<Arc<dyn Clock + Send + Sync>>,
    policy:     Option<PolicySet>,
    audit:      Option<Arc<dyn AuditSink>>,
    namespace:  Option<String>,
}

impl Default for KyaContextBuilder {
    fn default() -> Self {
        Self {
            revocation: None,
            nonces:     None,
            clock:      None,
            policy:     None,
            audit:      None,
            namespace:  None,
        }
    }
}

impl KyaContextBuilder {
    pub fn revocation(mut self, store: impl RevocationStore + 'static) -> Self {
        self.revocation = Some(Arc::new(store));
        self
    }

    pub fn nonces(mut self, store: impl NonceStore + 'static) -> Self {
        self.nonces = Some(Arc::new(store));
        self
    }

    pub fn clock(mut self, clock: impl Clock + Send + Sync + 'static) -> Self {
        self.clock = Some(Arc::new(clock));
        self
    }

    pub fn policy(mut self, policy: PolicySet) -> Self {
        self.policy = Some(policy);
        self
    }

    pub fn audit(mut self, sink: impl AuditSink + 'static) -> Self {
        self.audit = Some(Arc::new(sink));
        self
    }

    pub fn namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = Some(ns.into());
        self
    }

    pub fn build(self) -> KyaContext {
        KyaContext {
            revocation: self.revocation.unwrap_or_else(|| Arc::new(MemoryRevocationStore::new())),
            nonces:     self.nonces.unwrap_or_else(|| Arc::new(MemoryNonceStore::new())),
            clock:      self.clock.unwrap_or_else(|| Arc::new(SystemClock)),
            policy:     self.policy,
            audit:      self.audit.unwrap_or_else(|| Arc::new(NoopAuditSink)),
            namespace:  self.namespace,
        }
    }
}

// ── AsyncKyaContextBuilder ────────────────────────────────────────────────────

#[cfg(feature = "async")]
pub struct AsyncKyaContextBuilder {
    revocation: Option<Arc<dyn crate::registry::r#async::AsyncRevocationStore>>,
    nonces:     Option<Arc<dyn crate::registry::r#async::AsyncNonceStore>>,
    clock:      Option<Arc<dyn Clock + Send + Sync>>,
    policy:     Option<PolicySet>,
    audit:      Option<Arc<dyn AuditSink>>,
    namespace:  Option<String>,
}

#[cfg(feature = "async")]
impl Default for AsyncKyaContextBuilder {
    fn default() -> Self {
        Self {
            revocation: None,
            nonces:     None,
            clock:      None,
            policy:     None,
            audit:      None,
            namespace:  None,
        }
    }
}

#[cfg(feature = "async")]
impl AsyncKyaContextBuilder {
    pub fn revocation(mut self, store: impl crate::registry::r#async::AsyncRevocationStore + 'static) -> Self {
        self.revocation = Some(Arc::new(store));
        self
    }

    pub fn nonces(mut self, store: impl crate::registry::r#async::AsyncNonceStore + 'static) -> Self {
        self.nonces = Some(Arc::new(store));
        self
    }

    pub fn clock(mut self, clock: impl Clock + Send + Sync + 'static) -> Self {
        self.clock = Some(Arc::new(clock));
        self
    }

    pub fn policy(mut self, policy: PolicySet) -> Self {
        self.policy = Some(policy);
        self
    }

    pub fn audit(mut self, sink: impl AuditSink + 'static) -> Self {
        self.audit = Some(Arc::new(sink));
        self
    }

    pub fn namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = Some(ns.into());
        self
    }

    pub fn build(self) -> AsyncKyaContext {
        use crate::registry::r#async::{SyncRevocationAdapter, SyncNonceAdapter};

        AsyncKyaContext {
            revocation: self.revocation.unwrap_or_else(|| {
                Arc::new(SyncRevocationAdapter(Arc::new(MemoryRevocationStore::new())))
            }),
            nonces: self.nonces.unwrap_or_else(|| {
                Arc::new(SyncNonceAdapter(Arc::new(MemoryNonceStore::new())))
            }),
            clock:     self.clock.unwrap_or_else(|| Arc::new(SystemClock)),
            policy:    self.policy,
            audit:     self.audit.unwrap_or_else(|| Arc::new(NoopAuditSink)),
            namespace: self.namespace,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cert::CertBuilder,
        identity::DyoloIdentity,
        intent::{intent_hash, IntentTree},
    };

    #[test]
    #[allow(deprecated)]
    fn context_builder_defaults_and_authorizes() {
        let ctx   = KyaContext::builder().build();
        let human = DyoloIdentity::generate();
        let agent = DyoloIdentity::generate();
        let trade = intent_hash("trade.equity", b"");
        let tree  = IntentTree::build(vec![trade]).unwrap();
        let root  = tree.root();
        let now   = 1_700_000_000u64;
        let cert  = CertBuilder::new(agent.verifying_key(), root, now, now + 3600).sign(&human);

        let mut chain = DyoloChain::new(human.verifying_key(), root)
            .with_drift_tolerance(9999999);
        chain.push(cert);

        let proof = tree.prove(&trade).unwrap();
        let result = ctx.authorize(&chain, &agent.verifying_key(), &trade, &proof);
        assert!(result.is_ok(), "context builder authorization failed: {:?}", result.err());
    }

    #[test]
    fn sync_context_health_check_returns_ok() {
        let ctx = KyaContext::builder().build();
        assert!(ctx.health_check().is_ok());
    }
}