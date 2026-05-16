use std::sync::Arc;

use ed25519_dalek::VerifyingKey;

use crate::audit::{AuditSink, NoopAuditSink};
use crate::chain::{AuthorizedAction, BatchAuthorizeResult, Clock, DyoloChain, SystemClock};
use crate::error::A1Error;
use crate::intent::{IntentHash, MerkleProof};
use crate::policy::PolicySet;
use crate::registry::{MemoryNonceStore, MemoryRevocationStore, NonceStore, RevocationStore};

// ── Sync context ──────────────────────────────────────────────────────────────

/// A wiring context that holds all runtime dependencies required for chain
/// authorization.
///
/// `A1Context` is the recommended entry point for applications that do not
/// need fine-grained control over each authorization call. Configure once at
/// startup, share across threads via `Arc<A1Context>`, and call `authorize`.
///
/// # Example
///
/// ```rust,ignore
/// use a1::context::A1Context;
/// use a1::{DyoloChain, intent::{Intent, MerkleProof}};
///
/// let ctx = A1Context::builder().build();
///
/// let action = ctx.authorize(&chain, &agent_pk, &intent_hash, &proof)?;
/// println!("authorized depth={}", action.receipt().chain_depth);
/// ```
pub struct A1Context {
    pub revocation: Arc<dyn RevocationStore>,
    pub nonces: Arc<dyn NonceStore>,
    pub clock: Arc<dyn Clock + Send + Sync>,
    pub policy: Option<PolicySet>,
    pub audit: Arc<dyn AuditSink>,
    pub namespace: Option<String>,
}

impl A1Context {
    pub fn builder() -> A1ContextBuilder {
        A1ContextBuilder::default()
    }

    pub fn authorize(
        &self,
        chain: &DyoloChain,
        agent_pk: &VerifyingKey,
        intent: &IntentHash,
        proof: &MerkleProof,
    ) -> Result<AuthorizedAction, A1Error> {
        chain.authorize_with_options(
            agent_pk,
            intent,
            proof,
            self.clock.as_ref(),
            self.revocation.as_ref(),
            self.nonces.as_ref(),
            self.policy.as_ref(),
            self.audit.as_ref(),
        )
    }

    pub fn authorize_batch(
        &self,
        chain: &DyoloChain,
        agent_pk: &VerifyingKey,
        intents: &[(IntentHash, MerkleProof)],
    ) -> BatchAuthorizeResult {
        chain.authorize_batch(
            agent_pk,
            intents,
            self.clock.as_ref(),
            self.revocation.as_ref(),
            self.nonces.as_ref(),
        )
    }

    /// Probe both storage backends. Returns `Err` if either is unhealthy.
    ///
    /// Call this from your process health endpoint so load balancers can drain
    /// a replica before its backing store degrades authorization decisions.
    pub fn health_check(&self) -> Result<(), A1Error> {
        self.revocation
            .health_check()
            .map_err(|e| A1Error::StorageUnhealthy(format!("revocation: {e}")))?;
        self.nonces
            .health_check()
            .map_err(|e| A1Error::StorageUnhealthy(format!("nonces: {e}")))?;
        Ok(())
    }
}

// ── Async context ─────────────────────────────────────────────────────────────

#[cfg(feature = "async")]
pub struct AsyncA1Context {
    pub revocation: Arc<dyn crate::registry::r#async::AsyncRevocationStore>,
    pub nonces: Arc<dyn crate::registry::r#async::AsyncNonceStore>,
    pub clock: Arc<dyn Clock + Send + Sync>,
    pub policy: Option<PolicySet>,
    pub audit: Arc<dyn AuditSink>,
    pub namespace: Option<String>,
}

#[cfg(feature = "async")]
impl AsyncA1Context {
    pub fn builder() -> AsyncA1ContextBuilder {
        AsyncA1ContextBuilder::default()
    }

    pub async fn authorize(
        &self,
        chain: &DyoloChain,
        agent_pk: &VerifyingKey,
        intent: &IntentHash,
        proof: &MerkleProof,
    ) -> Result<AuthorizedAction, A1Error> {
        chain
            .authorize_async_with_options(
                agent_pk,
                intent,
                proof,
                self.clock.as_ref(),
                self.revocation.as_ref(),
                self.nonces.as_ref(),
                self.policy.as_ref(),
                self.audit.as_ref(),
            )
            .await
    }

    pub async fn authorize_batch(
        &self,
        chain: &DyoloChain,
        agent_pk: &VerifyingKey,
        intents: &[(IntentHash, MerkleProof)],
    ) -> BatchAuthorizeResult {
        chain
            .authorize_batch_async(
                agent_pk,
                intents,
                self.clock.as_ref(),
                self.revocation.as_ref(),
                self.nonces.as_ref(),
            )
            .await
    }

    pub async fn health_check(&self) -> Result<(), A1Error> {
        self.revocation
            .health_check()
            .await
            .map_err(|e| A1Error::StorageUnhealthy(format!("revocation: {e}")))?;
        self.nonces
            .health_check()
            .await
            .map_err(|e| A1Error::StorageUnhealthy(format!("nonces: {e}")))?;
        Ok(())
    }
}

// ── A1ContextBuilder ─────────────────────────────────────────────────────────

#[derive(Default)]
pub struct A1ContextBuilder {
    revocation: Option<Arc<dyn RevocationStore>>,
    nonces: Option<Arc<dyn NonceStore>>,
    clock: Option<Arc<dyn Clock + Send + Sync>>,
    policy: Option<PolicySet>,
    audit: Option<Arc<dyn AuditSink>>,
    namespace: Option<String>,
}

impl A1ContextBuilder {
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

    pub fn build(self) -> A1Context {
        A1Context {
            revocation: self
                .revocation
                .unwrap_or_else(|| Arc::new(MemoryRevocationStore::new())),
            nonces: self
                .nonces
                .unwrap_or_else(|| Arc::new(MemoryNonceStore::new())),
            clock: self.clock.unwrap_or_else(|| Arc::new(SystemClock)),
            policy: self.policy,
            audit: self.audit.unwrap_or_else(|| Arc::new(NoopAuditSink)),
            namespace: self.namespace,
        }
    }
}

// ── AsyncA1ContextBuilder ────────────────────────────────────────────────────

#[cfg(feature = "async")]
#[derive(Default)]
pub struct AsyncA1ContextBuilder {
    revocation: Option<Arc<dyn crate::registry::r#async::AsyncRevocationStore>>,
    nonces: Option<Arc<dyn crate::registry::r#async::AsyncNonceStore>>,
    clock: Option<Arc<dyn Clock + Send + Sync>>,
    policy: Option<PolicySet>,
    audit: Option<Arc<dyn AuditSink>>,
    namespace: Option<String>,
}

#[cfg(feature = "async")]
impl AsyncA1ContextBuilder {
    pub fn revocation(
        mut self,
        store: impl crate::registry::r#async::AsyncRevocationStore + 'static,
    ) -> Self {
        self.revocation = Some(Arc::new(store));
        self
    }

    pub fn nonces(
        mut self,
        store: impl crate::registry::r#async::AsyncNonceStore + 'static,
    ) -> Self {
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

    pub fn build(self) -> AsyncA1Context {
        use crate::registry::r#async::{SyncNonceAdapter, SyncRevocationAdapter};

        AsyncA1Context {
            revocation: self.revocation.unwrap_or_else(|| {
                Arc::new(SyncRevocationAdapter(
                    Arc::new(MemoryRevocationStore::new()),
                ))
            }),
            nonces: self
                .nonces
                .unwrap_or_else(|| Arc::new(SyncNonceAdapter(Arc::new(MemoryNonceStore::new())))),
            clock: self.clock.unwrap_or_else(|| Arc::new(SystemClock)),
            policy: self.policy,
            audit: self.audit.unwrap_or_else(|| Arc::new(NoopAuditSink)),
            namespace: self.namespace,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(deprecated)]
    use crate::{
        cert::CertBuilder,
        identity::DyoloIdentity,
        intent::{intent_hash, IntentTree},
    };

    #[test]
    #[allow(deprecated)]
    fn context_builder_defaults_and_authorizes() {
        let ctx = A1Context::builder().build();
        let human = DyoloIdentity::generate();
        let agent = DyoloIdentity::generate();
        let trade = intent_hash("trade.equity", b"");
        let tree = IntentTree::build(vec![trade]).unwrap();
        let root = tree.root();
        let now = SystemClock.unix_now();
        let cert =
            CertBuilder::new(agent.verifying_key(), root, now, now + 86400 * 365).sign(&human);

        let mut chain = DyoloChain::new(human.verifying_key(), root).with_drift_tolerance(9999999);
        chain.push(cert);

        let proof = tree.prove(&trade).unwrap();
        let result = ctx.authorize(&chain, &agent.verifying_key(), &trade, &proof);
        assert!(
            result.is_ok(),
            "context builder authorization failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn sync_context_health_check_returns_ok() {
        let ctx = A1Context::builder().build();
        assert!(ctx.health_check().is_ok());
    }
}
