use blake3::Hasher;
use ed25519_dalek::VerifyingKey;

use crate::audit::{AuditEvent, AuditOutcome, AuditSink, NoopAuditSink};
use crate::cert::DelegationCert;
use crate::crypto::DOMAIN_CHAIN_FP;
use crate::error::A1Error;
use crate::intent::{IntentHash, MerkleProof};
use crate::policy::PolicySet;
use crate::registry::{NonceStore, RevocationStore};
#[cfg(feature = "tracing")]
use tracing::Instrument;

// ── Clock ─────────────────────────────────────────────────────────────────────

pub trait Clock {
    fn unix_now(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn unix_now(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is before the Unix epoch")
            .as_secs()
    }
}

// ── VerificationReceipt ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VerificationReceipt {
    pub chain_depth: usize,
    pub verified_scope_root: IntentHash,
    pub intent: IntentHash,
    pub verified_at_unix: u64,
    pub chain_fingerprint: [u8; 32],
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub namespace: Option<String>,
}

impl VerificationReceipt {
    pub fn fingerprint_hex(&self) -> String {
        hex::encode(self.chain_fingerprint)
    }

    #[cfg(feature = "wire")]
    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + 8 + 32 + 32 + 8 + 32 + 64);
        out.extend_from_slice(b"a1_dyolo_v2.8.0:");
        out.extend_from_slice(&(self.chain_depth as u64).to_be_bytes());
        out.extend_from_slice(&self.verified_scope_root);
        out.extend_from_slice(&self.intent);
        out.extend_from_slice(&self.verified_at_unix.to_be_bytes());
        out.extend_from_slice(&self.chain_fingerprint);
        if let Some(ns) = &self.namespace {
            out.extend_from_slice(&(ns.len() as u64).to_be_bytes());
            out.extend_from_slice(ns.as_bytes());
        } else {
            out.extend_from_slice(&0u64.to_be_bytes());
        }
        out
    }
}

// ── BatchAuthorizeResult ──────────────────────────────────────────────────────

#[derive(Debug)]
pub struct BatchAuthorizeResult {
    pub receipts: Vec<Option<VerificationReceipt>>,
    pub errors: Vec<Option<A1Error>>,
    pub all_authorized: bool,
}

impl BatchAuthorizeResult {
    pub fn authorized_count(&self) -> usize {
        self.receipts.iter().filter(|r| r.is_some()).count()
    }
}

// ── AuthorizedAction ──────────────────────────────────────────────────────────

#[must_use]
#[non_exhaustive]
pub struct AuthorizedAction {
    pub receipt: VerificationReceipt,
}

impl AuthorizedAction {
    pub(crate) fn new(receipt: VerificationReceipt) -> Self {
        Self { receipt }
    }

    pub fn receipt(&self) -> &VerificationReceipt {
        &self.receipt
    }
}

impl std::fmt::Debug for AuthorizedAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthorizedAction")
            .field("receipt", &self.receipt)
            .finish()
    }
}

// ── Internal validation result ────────────────────────────────────────────────

struct ChainValidationResult {
    depth: usize,
    verified_scope_root: IntentHash,
    verified_at_unix: u64,
    seen_nonces: Vec<[u8; 16]>,
    cert_fingerprints: Vec<[u8; 32]>,
    chain_fingerprint: [u8; 32],
}

// ── Namespace scope derivation ────────────────────────────────────────────────

fn namespace_scope(namespace: &str, scope: &IntentHash) -> IntentHash {
    let mut h = blake3::Hasher::new_derive_key("a1::dyolo::namespace::scope::v2.8.0");
    h.update(&(namespace.len() as u64).to_le_bytes());
    h.update(namespace.as_bytes());
    h.update(scope);
    h.finalize().into()
}

// ── DyoloChain ────────────────────────────────────────────────────────────────

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DyoloChain {
    pub principal_pk: VerifyingKey,
    pub principal_scope: IntentHash,
    certs: Vec<DelegationCert>,
    pub drift_tolerance_secs: u64,
    pub namespace: Option<String>,
}

impl DyoloChain {
    pub fn new(principal_pk: VerifyingKey, principal_scope: IntentHash) -> Self {
        Self {
            principal_pk,
            principal_scope,
            certs: Vec::new(),
            drift_tolerance_secs: 15,
            namespace: None,
        }
    }

    /// Attach a namespace to this chain.
    ///
    /// The effective scope root becomes a namespace-derived hash of the original
    /// `principal_scope`, cryptographically binding the chain to one tenant namespace.
    /// A cert issued for namespace "tenant-a" cannot authorize under namespace "tenant-b".
    ///
    /// # Multi-tenancy
    ///
    /// Use this when building a chain on behalf of a specific tenant. The human
    /// principal must compute the namespaced scope before issuing the first cert:
    ///
    /// ```rust,ignore
    /// let chain = DyoloChain::new(pk, original_scope)
    ///     .with_namespace("acme-corp");
    /// let namespaced_scope = chain.principal_scope;
    /// let cert = CertBuilder::new(agent_pk, namespaced_scope, now, expiry).sign(&human);
    /// chain.push(cert);
    /// ```
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        let ns = namespace.into();
        self.principal_scope = namespace_scope(&ns, &self.principal_scope);
        self.namespace = Some(ns);
        self
    }

    pub fn with_drift_tolerance(mut self, secs: u64) -> Self {
        self.drift_tolerance_secs = secs;
        self
    }

    pub fn push(&mut self, cert: DelegationCert) -> &mut Self {
        self.certs.push(cert);
        self
    }

    pub fn len(&self) -> usize {
        self.certs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.certs.is_empty()
    }
    pub fn certs(&self) -> &[DelegationCert] {
        &self.certs
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        let mut h = Hasher::new_derive_key(DOMAIN_CHAIN_FP);
        h.update(b"a1::dyolo::chain::v2.8.0");
        h.update(self.principal_pk.as_bytes());
        h.update(&self.principal_scope);
        if let Some(ns) = &self.namespace {
            h.update(&(ns.len() as u64).to_le_bytes());
            h.update(ns.as_bytes());
        } else {
            h.update(&0u64.to_le_bytes());
        }
        self.certs
            .iter()
            .fold(h, |mut h, cert| {
                h.update(&cert.fingerprint());
                h
            })
            .finalize()
            .into()
    }

    // ── Structural validation (CPU-only, no I/O) ──────────────────────────────

    fn validate_structure(
        &self,
        agent_pk: &VerifyingKey,
        intent: &IntentHash,
        proof: &MerkleProof,
        clock: &dyn Clock,
        drift_tolerance: u64,
    ) -> Result<ChainValidationResult, A1Error> {
        if self.certs.is_empty() {
            return Err(A1Error::EmptyChain);
        }
        if self.certs[0].delegator_pk != self.principal_pk {
            return Err(A1Error::RootMismatch);
        }

        let now = clock.unix_now();
        let tolerated_early = now.saturating_add(drift_tolerance);
        let tolerated_late = now.saturating_sub(drift_tolerance);
        let chain_len = self.certs.len();

        if chain_len > 255 {
            return Err(A1Error::MaxDepthExceeded(255, 255));
        }

        let mut current_scope = self.principal_scope;
        let mut expected_delegator = self.principal_pk;
        let mut depth: usize = 0;
        let mut max_allowed_depth = u8::MAX;
        let mut parent_expiry = u64::MAX;

        let mut seen_nonces: Vec<[u8; 16]> = Vec::with_capacity(chain_len);
        let mut cert_fingerprints: Vec<[u8; 32]> = Vec::with_capacity(chain_len);
        let mut batch_sigs: Vec<ed25519_dalek::Signature> = Vec::with_capacity(chain_len);
        let mut batch_pks: Vec<VerifyingKey> = Vec::with_capacity(chain_len);
        let mut batch_msgs: Vec<Vec<u8>> = Vec::with_capacity(chain_len);

        for (i, cert) in self.certs.iter().enumerate() {
            if cert.delegator_pk != expected_delegator {
                return Err(A1Error::BrokenLinkage(i));
            }

            if cert.version != crate::cert::CERT_VERSION {
                return Err(A1Error::UnsupportedVersion {
                    expected: crate::cert::CERT_VERSION,
                    got: cert.version,
                });
            }

            #[cfg(feature = "wire")]
            let ext_commit = cert.extensions.commitment();
            #[cfg(not(feature = "wire"))]
            let ext_commit = cert.extensions_hash.unwrap_or_else(|| {
                let mut h = crate::crypto::derive_key("a1::dyolo::cert::ext::v2.8.0", cert.version);
                h.update(&0u64.to_le_bytes());
                h.finalize().into()
            });

            batch_msgs.push(DelegationCert::signable_bytes(
                cert.version,
                &cert.delegator_pk,
                &cert.delegate_pk,
                &cert.scope_root,
                &cert.scope_proof,
                &cert.nonce,
                cert.issued_at,
                cert.expiration_unix,
                cert.max_depth,
                &ext_commit,
            ));
            batch_sigs.push(cert.signature);
            batch_pks.push(cert.delegator_pk);

            if tolerated_early < cert.issued_at {
                return Err(A1Error::NotYetValid(i, cert.issued_at, now));
            }
            if cert.expiration_unix < tolerated_late {
                return Err(A1Error::Expired(i, cert.expiration_unix, now));
            }
            if cert.expiration_unix > parent_expiry {
                return Err(A1Error::TemporalViolation(
                    i,
                    cert.expiration_unix,
                    parent_expiry,
                ));
            }

            depth += 1;
            if depth > max_allowed_depth as usize {
                return Err(A1Error::MaxDepthExceeded(i, max_allowed_depth));
            }
            if cert.max_depth < max_allowed_depth {
                max_allowed_depth = cert.max_depth;
            }

            for seen in &seen_nonces {
                if seen == &cert.nonce {
                    return Err(A1Error::NonceReplay);
                }
            }
            seen_nonces.push(cert.nonce);
            cert_fingerprints.push(cert.fingerprint());

            let is_passthrough =
                cert.scope_proof.subset_intents.is_empty() && cert.scope_proof.proofs.is_empty();
            if is_passthrough {
                use subtle::ConstantTimeEq;
                if cert.scope_root.ct_eq(&current_scope).unwrap_u8() == 0 {
                    return Err(A1Error::ScopeEscalation(i));
                }
            } else {
                let derived = cert
                    .scope_proof
                    .verify_and_derive_root(&current_scope)
                    .map_err(|_| A1Error::ScopeEscalation(i))?;
                use subtle::ConstantTimeEq;
                if derived.ct_eq(&cert.scope_root).unwrap_u8() == 0 {
                    return Err(A1Error::ScopeEscalation(i));
                }
            }

            parent_expiry = cert.expiration_unix;
            current_scope = cert.scope_root;
            expected_delegator = cert.delegate_pk;
        }

        if expected_delegator != *agent_pk {
            return Err(A1Error::UnauthorizedLeaf);
        }

        {
            let msgs_refs: Vec<&[u8]> = batch_msgs.iter().map(|m| m.as_slice()).collect();
            if ed25519_dalek::verify_batch(&msgs_refs, &batch_sigs, &batch_pks).is_err() {
                for (i, cert) in self.certs.iter().enumerate() {
                    if !cert.verify_signature() {
                        return Err(A1Error::InvalidSignature(i));
                    }
                }
                return Err(A1Error::InvalidSignature(0));
            }
        }

        let intent_authorized = if proof.siblings.is_empty() {
            use subtle::ConstantTimeEq;
            intent.ct_eq(&current_scope).into()
        } else {
            proof.verify(intent, &current_scope)
        };
        if !intent_authorized {
            return Err(A1Error::ScopeViolation);
        }

        Ok(ChainValidationResult {
            depth,
            verified_scope_root: current_scope,
            verified_at_unix: now,
            seen_nonces,
            cert_fingerprints,
            chain_fingerprint: self.fingerprint(),
        })
    }

    // ── authorize ─────────────────────────────────────────────────────────────

    pub fn authorize(
        &self,
        agent_pk: &VerifyingKey,
        intent: &IntentHash,
        proof: &MerkleProof,
        clock: &(dyn Clock + Send + Sync),
        revocation: &(dyn RevocationStore + Send + Sync),
        nonces: &(dyn NonceStore + Send + Sync),
    ) -> Result<AuthorizedAction, A1Error> {
        self.authorize_with_options(
            agent_pk,
            intent,
            proof,
            clock,
            revocation,
            nonces,
            None,
            &NoopAuditSink,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn authorize_with_options(
        &self,
        agent_pk: &VerifyingKey,
        intent_h: &IntentHash,
        proof: &MerkleProof,
        clock: &(dyn Clock + Send + Sync),
        revocation: &(dyn RevocationStore + Send + Sync),
        nonces: &(dyn NonceStore + Send + Sync),
        policy: Option<&PolicySet>,
        sink: &dyn AuditSink,
    ) -> Result<AuthorizedAction, A1Error> {
        #[cfg(feature = "tracing")]
        let _span =
            tracing::info_span!("a1::authorize", chain_len = self.certs.len()).entered();

        let principal_hex = hex::encode(self.principal_pk.as_bytes());
        let executor_hex = hex::encode(agent_pk.as_bytes());

        let result =
            self.authorize_inner(agent_pk, intent_h, proof, clock, revocation, nonces, policy);

        let outcome = match &result {
            Ok(_) => AuditOutcome::Authorized,
            Err(A1Error::PolicyViolation(_)) => AuditOutcome::PolicyViolation,
            Err(e) if e.is_transient_storage_failure() => AuditOutcome::StorageError,
            Err(_) => AuditOutcome::Denied,
        };

        let mut event = AuditEvent::new(
            outcome,
            principal_hex,
            executor_hex,
            self.certs.len(),
            intent_h,
            clock.unix_now(),
        );

        if let Ok(action) = &result {
            event = event.with_fingerprint(action.receipt.chain_fingerprint);
            #[cfg(feature = "tracing")]
            tracing::info!(
                chain_depth = action.receipt.chain_depth,
                chain_fingerprint = %action.receipt.fingerprint_hex(),
                "a1: authorization succeeded"
            );
        } else if let Err(e) = &result {
            event = event.with_error(e.to_string());
            #[cfg(feature = "tracing")]
            tracing::warn!(error = %e, "a1: authorization failed");
        }

        sink.emit(event);
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn authorize_inner(
        &self,
        agent_pk: &VerifyingKey,
        intent_h: &IntentHash,
        proof: &MerkleProof,
        clock: &(dyn Clock + Send + Sync),
        revocation: &(dyn RevocationStore + Send + Sync),
        nonces: &(dyn NonceStore + Send + Sync),
        policy: Option<&PolicySet>,
    ) -> Result<AuthorizedAction, A1Error> {
        if let Some(p) = policy {
            p.check_chain(self)?;
        }

        let v =
            self.validate_structure(agent_pk, intent_h, proof, clock, self.drift_tolerance_secs)?;

        for fp in &v.cert_fingerprints {
            if revocation
                .is_revoked(fp)
                .map_err(A1Error::StorageFailure)?
            {
                return Err(A1Error::Revoked);
            }
        }

        if !nonces
            .try_consume_batch(&v.seen_nonces)
            .map_err(A1Error::StorageFailure)?
        {
            return Err(A1Error::NonceReplay);
        }

        Ok(AuthorizedAction::new(VerificationReceipt {
            chain_depth: v.depth,
            verified_scope_root: v.verified_scope_root,
            intent: *intent_h,
            verified_at_unix: v.verified_at_unix,
            chain_fingerprint: v.chain_fingerprint,
            namespace: self.namespace.clone(),
        }))
    }

    // ── authorize_batch ───────────────────────────────────────────────────────

    pub fn authorize_batch(
        &self,
        agent_pk: &VerifyingKey,
        intents: &[(IntentHash, MerkleProof)],
        clock: &(dyn Clock + Send + Sync),
        revocation: &(dyn RevocationStore + Send + Sync),
        nonces: &(dyn NonceStore + Send + Sync),
    ) -> BatchAuthorizeResult {
        if intents.is_empty() {
            return BatchAuthorizeResult {
                receipts: Vec::new(),
                errors: Vec::new(),
                all_authorized: true,
            };
        }

        let now = clock.unix_now();
        let first_intent = &intents[0].0;
        let first_proof = &intents[0].1;

        let v = match self.validate_structure(
            agent_pk,
            first_intent,
            first_proof,
            clock,
            self.drift_tolerance_secs,
        ) {
            Ok(v) => v,
            Err(e) => {
                let n = intents.len();
                let msg = e.to_string();
                return BatchAuthorizeResult {
                    receipts: vec![None; n],
                    errors: (0..n)
                        .map(|i| {
                            Some(A1Error::BatchItemFailed {
                                index: i,
                                reason: msg.clone(),
                            })
                        })
                        .collect(),
                    all_authorized: false,
                };
            }
        };

        for fp in &v.cert_fingerprints {
            match revocation.is_revoked(fp) {
                Ok(true) => {
                    let n = intents.len();
                    return BatchAuthorizeResult {
                        receipts: vec![None; n],
                        errors: (0..n).map(|_| Some(A1Error::Revoked)).collect(),
                        all_authorized: false,
                    };
                }
                Err(e) => {
                    let n = intents.len();
                    let msg = A1Error::StorageFailure(e).to_string();
                    return BatchAuthorizeResult {
                        receipts: vec![None; n],
                        errors: (0..n)
                            .map(|_| {
                                Some(A1Error::BatchItemFailed {
                                    index: 0,
                                    reason: msg.clone(),
                                })
                            })
                            .collect(),
                        all_authorized: false,
                    };
                }
                Ok(false) => {}
            }
        }

        let mut receipts: Vec<Option<VerificationReceipt>> = Vec::with_capacity(intents.len());
        let mut errors: Vec<Option<A1Error>> = Vec::with_capacity(intents.len());
        let mut all_ok = true;

        for (i, (intent_h, proof)) in intents.iter().enumerate() {
            let intent_authorized = if proof.siblings.is_empty() {
                use subtle::ConstantTimeEq;
                intent_h.ct_eq(&v.verified_scope_root).into()
            } else {
                proof.verify(intent_h, &v.verified_scope_root)
            };

            if intent_authorized {
                receipts.push(Some(VerificationReceipt {
                    chain_depth: v.depth,
                    verified_scope_root: v.verified_scope_root,
                    intent: *intent_h,
                    verified_at_unix: now,
                    chain_fingerprint: v.chain_fingerprint,
                    namespace: self.namespace.clone(),
                }));
                errors.push(None);
            } else {
                receipts.push(None);
                errors.push(Some(A1Error::BatchItemFailed {
                    index: i,
                    reason: A1Error::ScopeViolation.to_string(),
                }));
                all_ok = false;
            }
        }

        if !all_ok {
            return BatchAuthorizeResult {
                receipts,
                errors,
                all_authorized: false,
            };
        }

        match nonces.try_consume_batch(&v.seen_nonces) {
            Ok(true) => {}
            Ok(false) => {
                let n = intents.len();
                return BatchAuthorizeResult {
                    receipts: vec![None; n],
                    errors: (0..n).map(|_| Some(A1Error::NonceReplay)).collect(),
                    all_authorized: false,
                };
            }
            Err(e) => {
                let n = intents.len();
                let msg = A1Error::StorageFailure(e).to_string();
                return BatchAuthorizeResult {
                    receipts: vec![None; n],
                    errors: (0..n)
                        .map(|_| {
                            Some(A1Error::BatchItemFailed {
                                index: 0,
                                reason: msg.clone(),
                            })
                        })
                        .collect(),
                    all_authorized: false,
                };
            }
        }

        BatchAuthorizeResult {
            receipts,
            errors,
            all_authorized: true,
        }
    }

    // ── authorize_async ───────────────────────────────────────────────────────

    #[cfg(feature = "async")]
    pub async fn authorize_async(
        &self,
        agent_pk: &VerifyingKey,
        intent: &IntentHash,
        proof: &MerkleProof,
        clock: &(dyn Clock + Send + Sync),
        revocation: &(dyn crate::registry::r#async::AsyncRevocationStore + Send + Sync),
        nonces: &(dyn crate::registry::r#async::AsyncNonceStore + Send + Sync),
    ) -> Result<AuthorizedAction, A1Error> {
        self.authorize_async_with_options(
            agent_pk,
            intent,
            proof,
            clock,
            revocation,
            nonces,
            None,
            &NoopAuditSink,
        )
        .await
    }

    #[cfg(feature = "async")]
    #[allow(clippy::too_many_arguments)]
    pub async fn authorize_async_with_options(
        &self,
        agent_pk: &VerifyingKey,
        intent_h: &IntentHash,
        proof: &MerkleProof,
        clock: &(dyn Clock + Send + Sync),
        revocation: &(dyn crate::registry::r#async::AsyncRevocationStore + Send + Sync),
        nonces: &(dyn crate::registry::r#async::AsyncNonceStore + Send + Sync),
        policy: Option<&PolicySet>,
        sink: &dyn AuditSink,
    ) -> Result<AuthorizedAction, A1Error> {
        let principal_hex = hex::encode(self.principal_pk.as_bytes());
        let executor_hex = hex::encode(agent_pk.as_bytes());

        #[cfg(feature = "tracing")]
        let span = tracing::info_span!("a1::authorize_async", chain_len = self.certs.len());

        let result = async {
            if let Some(p) = policy {
                p.check_chain(self)?;
            }

            let v = self.validate_structure(
                agent_pk,
                intent_h,
                proof,
                clock,
                self.drift_tolerance_secs,
            )?;

            for fp in &v.cert_fingerprints {
                if revocation
                    .is_revoked(fp)
                    .await
                    .map_err(A1Error::StorageFailure)?
                {
                    return Err(A1Error::Revoked);
                }
            }

            if !nonces
                .try_consume_batch(&v.seen_nonces)
                .await
                .map_err(A1Error::StorageFailure)?
            {
                return Err(A1Error::NonceReplay);
            }

            Ok(AuthorizedAction::new(VerificationReceipt {
                chain_depth: v.depth,
                verified_scope_root: v.verified_scope_root,
                intent: *intent_h,
                verified_at_unix: v.verified_at_unix,
                chain_fingerprint: v.chain_fingerprint,
                namespace: self.namespace.clone(),
            }))
        };

        #[cfg(feature = "tracing")]
        let result = result.instrument(span).await;
        #[cfg(not(feature = "tracing"))]
        let result = result.await;

        let outcome = match &result {
            Ok(_) => AuditOutcome::Authorized,
            Err(A1Error::PolicyViolation(_)) => AuditOutcome::PolicyViolation,
            Err(e) if e.is_transient_storage_failure() => AuditOutcome::StorageError,
            Err(_) => AuditOutcome::Denied,
        };

        let mut event = AuditEvent::new(
            outcome,
            principal_hex,
            executor_hex,
            self.certs.len(),
            intent_h,
            clock.unix_now(),
        );

        if let Ok(action) = &result {
            event = event.with_fingerprint(action.receipt.chain_fingerprint);
        } else if let Err(e) = &result {
            event = event.with_error(e.to_string());
        }

        sink.emit(event);
        result
    }

    #[cfg(feature = "async")]
    pub async fn authorize_batch_async(
        &self,
        agent_pk: &VerifyingKey,
        intents: &[(IntentHash, MerkleProof)],
        clock: &(dyn Clock + Send + Sync),
        revocation: &(dyn crate::registry::r#async::AsyncRevocationStore + Send + Sync),
        nonces: &(dyn crate::registry::r#async::AsyncNonceStore + Send + Sync),
    ) -> BatchAuthorizeResult {
        if intents.is_empty() {
            return BatchAuthorizeResult {
                receipts: Vec::new(),
                errors: Vec::new(),
                all_authorized: true,
            };
        }

        let now = clock.unix_now();
        let first_intent = &intents[0].0;
        let first_proof = &intents[0].1;

        let v = match self.validate_structure(
            agent_pk,
            first_intent,
            first_proof,
            clock,
            self.drift_tolerance_secs,
        ) {
            Ok(v) => v,
            Err(e) => {
                let n = intents.len();
                let msg = e.to_string();
                return BatchAuthorizeResult {
                    receipts: vec![None; n],
                    errors: (0..n)
                        .map(|i| {
                            Some(A1Error::BatchItemFailed {
                                index: i,
                                reason: msg.clone(),
                            })
                        })
                        .collect(),
                    all_authorized: false,
                };
            }
        };

        for fp in &v.cert_fingerprints {
            match revocation.is_revoked(fp).await {
                Ok(true) => {
                    let n = intents.len();
                    return BatchAuthorizeResult {
                        receipts: vec![None; n],
                        errors: (0..n).map(|_| Some(A1Error::Revoked)).collect(),
                        all_authorized: false,
                    };
                }
                Err(e) => {
                    let n = intents.len();
                    let msg = A1Error::StorageFailure(e).to_string();
                    return BatchAuthorizeResult {
                        receipts: vec![None; n],
                        errors: (0..n)
                            .map(|_| {
                                Some(A1Error::BatchItemFailed {
                                    index: 0,
                                    reason: msg.clone(),
                                })
                            })
                            .collect(),
                        all_authorized: false,
                    };
                }
                Ok(false) => {}
            }
        }

        let mut receipts: Vec<Option<VerificationReceipt>> = Vec::with_capacity(intents.len());
        let mut errors: Vec<Option<A1Error>> = Vec::with_capacity(intents.len());
        let mut all_ok = true;

        for (i, (intent_h, proof)) in intents.iter().enumerate() {
            let intent_authorized = if proof.siblings.is_empty() {
                use subtle::ConstantTimeEq;
                intent_h.ct_eq(&v.verified_scope_root).into()
            } else {
                proof.verify(intent_h, &v.verified_scope_root)
            };

            if intent_authorized {
                receipts.push(Some(VerificationReceipt {
                    chain_depth: v.depth,
                    verified_scope_root: v.verified_scope_root,
                    intent: *intent_h,
                    verified_at_unix: now,
                    chain_fingerprint: v.chain_fingerprint,
                    namespace: self.namespace.clone(),
                }));
                errors.push(None);
            } else {
                receipts.push(None);
                errors.push(Some(A1Error::BatchItemFailed {
                    index: i,
                    reason: A1Error::ScopeViolation.to_string(),
                }));
                all_ok = false;
            }
        }

        if !all_ok {
            return BatchAuthorizeResult {
                receipts,
                errors,
                all_authorized: false,
            };
        }

        match nonces.try_consume_batch(&v.seen_nonces).await {
            Ok(true) => {}
            Ok(false) => {
                let n = intents.len();
                return BatchAuthorizeResult {
                    receipts: vec![None; n],
                    errors: (0..n).map(|_| Some(A1Error::NonceReplay)).collect(),
                    all_authorized: false,
                };
            }
            Err(e) => {
                let n = intents.len();
                let msg = A1Error::StorageFailure(e).to_string();
                return BatchAuthorizeResult {
                    receipts: vec![None; n],
                    errors: (0..n)
                        .map(|_| {
                            Some(A1Error::BatchItemFailed {
                                index: 0,
                                reason: msg.clone(),
                            })
                        })
                        .collect(),
                    all_authorized: false,
                };
            }
        }

        BatchAuthorizeResult {
            receipts,
            errors,
            all_authorized: true,
        }
    }
}

impl Clone for DyoloChain {
    fn clone(&self) -> Self {
        Self {
            principal_pk: self.principal_pk,
            principal_scope: self.principal_scope,
            certs: self.certs.clone(),
            drift_tolerance_secs: self.drift_tolerance_secs,
            namespace: self.namespace.clone(),
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
        registry::{MemoryNonceStore, MemoryRevocationStore},
    };

    struct FixedClock(u64);
    impl Clock for FixedClock {
        fn unix_now(&self) -> u64 {
            self.0
        }
    }

    #[allow(deprecated)]
    fn setup() -> (DyoloIdentity, DyoloIdentity, DyoloIdentity, IntentTree, u64) {
        let human = DyoloIdentity::generate();
        let agent_a = DyoloIdentity::generate();
        let agent_b = DyoloIdentity::generate();
        let trade = intent_hash("TRADE_AAPL_100", b"limit=182.50");
        let query = intent_hash("QUERY_PORTFOLIO", b"");
        let tree = IntentTree::build(vec![trade, query]).unwrap();
        let now = 1_700_000_000u64;
        (human, agent_a, agent_b, tree, now)
    }

    #[allow(deprecated)]
    fn two_hop_chain(
        human: &DyoloIdentity,
        a: &DyoloIdentity,
        b: &DyoloIdentity,
        scope: IntentHash,
        now: u64,
    ) -> DyoloChain {
        let expiry = now + 3600;
        let ca = CertBuilder::new(a.verifying_key(), scope, now, expiry).sign(human);
        let cb = CertBuilder::new(b.verifying_key(), scope, now, expiry).sign(a);
        let mut chain = DyoloChain::new(human.verifying_key(), scope);
        chain.push(ca).push(cb);
        chain
    }

    #[test]
    #[allow(deprecated)]
    fn full_delegation_chain_succeeds() {
        let (human, a, b, tree, now) = setup();
        let root = tree.root();
        let trade = intent_hash("TRADE_AAPL_100", b"limit=182.50");
        let proof = tree.prove(&trade).unwrap();
        let chain = two_hop_chain(&human, &a, &b, root, now);
        let action = chain
            .authorize(
                &b.verifying_key(),
                &trade,
                &proof,
                &FixedClock(now),
                &MemoryRevocationStore::new(),
                &MemoryNonceStore::new(),
            )
            .unwrap();
        assert_eq!(action.receipt.chain_depth, 2);
        assert_eq!(action.receipt.intent, trade);
        assert!(action.receipt.namespace.is_none());
    }

    #[test]
    #[allow(deprecated)]
    fn namespace_isolation_different_scopes() {
        let human = DyoloIdentity::generate();
        let agent = DyoloIdentity::generate();
        let now = SystemClock.unix_now();

        // Two namespaced chains from the same base scope must produce different principal_scopes
        let base_scope: IntentHash = intent_hash("trade", b"");
        let chain_a = DyoloChain::new(human.verifying_key(), base_scope).with_namespace("tenant-a");
        let chain_b = DyoloChain::new(human.verifying_key(), base_scope).with_namespace("tenant-b");

        assert_ne!(
            chain_a.principal_scope, chain_b.principal_scope,
            "namespaced chains must have different effective scopes"
        );
        assert_ne!(chain_a.fingerprint(), chain_b.fingerprint());

        // The namespaced principal_scope IS the intent for a single-scope chain.
        // A cert whose scope_root = scope_a can authorize scope_a as the intent
        // (empty proof: intent must equal current_scope after traversal).
        let scope_a = chain_a.principal_scope;
        let scope_b = chain_b.principal_scope;

        let cert_a =
            CertBuilder::new(agent.verifying_key(), scope_a, now, now + 86400).sign(&human);
        let cert_b =
            CertBuilder::new(agent.verifying_key(), scope_b, now, now + 86400).sign(&human);

        // Tenant-a chain authorizes scope_a as the intent
        let mut ca = DyoloChain::new(human.verifying_key(), scope_a);
        ca.push(cert_a);
        let ok_a = ca.authorize(
            &agent.verifying_key(),
            &scope_a,
            &MerkleProof::default(),
            &FixedClock(now),
            &MemoryRevocationStore::new(),
            &MemoryNonceStore::new(),
        );
        assert!(
            ok_a.is_ok(),
            "tenant-a cert should authorize under tenant-a scope: {:?}",
            ok_a.err()
        );

        // Tenant-b chain authorizes scope_b as the intent
        let mut cb = DyoloChain::new(human.verifying_key(), scope_b);
        cb.push(cert_b);
        let ok_b = cb.authorize(
            &agent.verifying_key(),
            &scope_b,
            &MerkleProof::default(),
            &FixedClock(now),
            &MemoryRevocationStore::new(),
            &MemoryNonceStore::new(),
        );
        assert!(
            ok_b.is_ok(),
            "tenant-b cert should authorize under tenant-b scope: {:?}",
            ok_b.err()
        );

        // Cross-namespace: cert issued for scope_b must fail under a chain expecting scope_a
        // because cert.scope_root (scope_b) != ca.principal_scope (scope_a) => BrokenLinkage or ScopeEscalation
        let cert_b_wrong =
            CertBuilder::new(agent.verifying_key(), scope_b, now, now + 86400).sign(&human);
        let mut c_wrong = DyoloChain::new(human.verifying_key(), scope_a);
        c_wrong.push(cert_b_wrong);
        let result = c_wrong.authorize(
            &agent.verifying_key(),
            &scope_a,
            &MerkleProof::default(),
            &FixedClock(now),
            &MemoryRevocationStore::new(),
            &MemoryNonceStore::new(),
        );
        assert!(
            result.is_err(),
            "cert scoped to tenant-b must not work under tenant-a chain"
        );
    }

    #[test]
    #[allow(deprecated)]
    fn batch_authorize_all_or_nothing() {
        let (human, a, b, tree, now) = setup();
        let root = tree.root();
        let trade = intent_hash("TRADE_AAPL_100", b"limit=182.50");
        let query = intent_hash("QUERY_PORTFOLIO", b"");
        let t_proof = tree.prove(&trade).unwrap();
        let q_proof = tree.prove(&query).unwrap();
        let chain = two_hop_chain(&human, &a, &b, root, now);
        let result = chain.authorize_batch(
            &b.verifying_key(),
            &[(trade, t_proof), (query, q_proof)],
            &FixedClock(now),
            &MemoryRevocationStore::new(),
            &MemoryNonceStore::new(),
        );
        assert!(result.all_authorized);
        assert_eq!(result.authorized_count(), 2);
    }

    #[test]
    #[allow(deprecated)]
    fn chain_fingerprint_stable() {
        let (human, a, b, tree, now) = setup();
        let chain = two_hop_chain(&human, &a, &b, tree.root(), now);
        assert_eq!(chain.fingerprint(), chain.clone().fingerprint());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    #[allow(deprecated)]
    async fn authorize_async_succeeds() {
        use crate::registry::r#async::{SyncNonceAdapter, SyncRevocationAdapter};
        use std::sync::Arc;
        let (human, a, b, tree, now) = setup();
        let root = tree.root();
        let trade = intent_hash("TRADE_AAPL_100", b"limit=182.50");
        let proof = tree.prove(&trade).unwrap();
        let chain = two_hop_chain(&human, &a, &b, root, now);
        let rev = SyncRevocationAdapter(Arc::new(MemoryRevocationStore::new()));
        let nonces = SyncNonceAdapter(Arc::new(MemoryNonceStore::new()));
        let action = chain
            .authorize_async(
                &b.verifying_key(),
                &trade,
                &proof,
                &FixedClock(now),
                &rev,
                &nonces,
            )
            .await
            .unwrap();
        assert_eq!(action.receipt.chain_depth, 2);
    }
}
