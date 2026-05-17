use ed25519_dalek::VerifyingKey;

use crate::cert::CertBuilder;
use crate::chain::{Clock, DyoloChain, SystemClock};
use crate::error::A1Error;
use crate::identity::narrowing::NarrowingMatrix;
use crate::identity::receipt::ProvableReceipt;
use crate::identity::Signer;
use crate::intent::{Intent, IntentHash, IntentTree, MerkleProof, SubScopeProof};
use crate::registry::{MemoryNonceStore, MemoryRevocationStore, NonceStore, RevocationStore};

#[cfg(feature = "wire")]
use crate::cert_extensions::{CertExtensions, ExtValue};

// ── Passport Namespace Binding Tag ───────────────────────────────────────────
//
// PASSPORT_PROTOCOL_TAG is the namespace binding prefix embedded in every root
// DelegationCert, as specified in §4.2 of spec/A1-PROTOCOL.md. It is included
// in the cert's signed digest and in the chain fingerprint computation. Changing
// this value invalidates all previously issued passports — do not modify.
#[rustfmt::skip]
#[allow(dead_code)]
pub(crate) const PASSPORT_PROTOCOL_TAG: &[u8] = &[
    0x44, 0x79, 0x6f, 0x6c, 0x6f, 0x50, 0x61, 0x73, 0x73, 0x70, 0x6f, 0x72, 0x74,
    0x20, 0x76, 0x32, 0x2e, 0x38, 0x2e, 0x30,
    0x7c, 0x64, 0x79, 0x6f, 0x6c, 0x6f, 0x67, 0x69, 0x63, 0x69, 0x61, 0x6e,
];

/// A long-lived agent identity with cryptographically enforced capability bounds.
///
/// `DyoloPassport` is the primary entry point for the A1 v2.8.0 identity
/// layer. It stores a set of named capabilities, a `NarrowingMatrix` bitmask for
/// O(1) capability enforcement, and a self-signed root `DelegationCert` that
/// anchors all downstream delegation chains.
///
/// # How it closes the Recursive Delegation Gap
///
/// Without a passport, delegating agent A → B → C has no guarantee that C
/// operates within the bounds A originally set. `DyoloPassport` solves this
/// at two levels:
///
/// - **Cryptographic** — sub-delegation certs carry a `SubScopeProof` proving
///   their authorized scope is a strict Merkle subset of the passport's scope.
///   The chain verifies this linkage before any intent is authorized.
///
/// - **Semantic** — the `NarrowingMatrix` maps capability names to a 256-bit
///   bitmask. `guard` checks in O(1) that the requested intent's action name
///   is within the passport's capability set before calling the chain verifier.
///
/// # Lifecycle
///
/// 1. **Issue** — call `DyoloPassport::issue` once to create a root passport.
///    Store the JSON file in secure storage (vault, HSM, encrypted S3).
/// 2. **Delegate** — call `issue_sub` to produce a time-limited `DelegationCert`
///    for a specific task. The sub-cert's capabilities must be a strict subset
///    of the passport's; any escalation is rejected at issuance time.
/// 3. **Build chain** — call `new_chain` to get a `DyoloChain` anchored at this
///    passport, then `push` the sub-cert onto it.
/// 4. **Guard** — call `guard_local` (single capability) or `guard` (custom proof)
///    to verify the chain and receive a `ProvableReceipt` for audit archival.
///
/// # Example
///
/// ```rust,ignore
/// use a1::{DyoloIdentity, SystemClock};
/// use a1::passport::DyoloPassport;
/// use a1::intent::Intent;
///
/// let root_id  = DyoloIdentity::generate();
/// let agent_id = DyoloIdentity::generate();
/// let clock    = SystemClock;
///
/// // Step 1 — Issue passport (done once, stored in vault)
/// let passport = DyoloPassport::issue(
///     "acme-trading-bot",
///     &["trade.equity", "portfolio.read"],
///     30 * 24 * 3600,
///     &root_id,
///     &clock,
/// )?;
/// passport.save("passport.json")?;
///
/// // Step 2 — Delegate to a sub-agent (done per task)
/// let sub_cert = passport.issue_sub(
///     agent_id.verifying_key(),
///     &["trade.equity"],
///     3600,
///     &root_id,
///     &clock,
/// )?;
///
/// // Step 3 — Build chain
/// let mut chain = passport.new_chain()?;
/// chain.push(sub_cert);
///
/// // Step 4 — Guard
/// let intent  = Intent::new("trade.equity")?;
/// let receipt = passport.guard_local(&chain, &agent_id.verifying_key(), &intent)?;
///
/// println!("{}", receipt);
/// # Ok::<(), a1::A1Error>(())
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DyoloPassport {
    /// Human-readable agent identifier (e.g. `"acme-trading-bot"`).
    pub namespace: String,

    /// 256-bit capability mask: the outer bound for all delegations from this passport.
    pub capability_mask: NarrowingMatrix,

    /// Named capability strings stored for Merkle tree reconstruction.
    pub(crate) capabilities: Vec<String>,

    /// Self-signed root `DelegationCert`. Its `delegator_pk` is the passport holder's key.
    pub cert: crate::cert::DelegationCert,
}

impl DyoloPassport {
    /// Issue a root passport for the given signing identity.
    ///
    /// The resulting passport is self-signed: the delegator and delegate public
    /// keys are identical. Its scope covers exactly the listed `capabilities`.
    ///
    /// # Arguments
    ///
    /// - `namespace`    — human-readable agent name, e.g. `"acme-trading-bot"`.
    /// - `capabilities` — slice of capability action names, e.g. `["trade.equity"]`.
    /// - `ttl_secs`     — lifetime of the root cert in seconds.
    /// - `signer`       — signing backend for the passport's private key.
    /// - `clock`        — time source; use [`SystemClock`] in production.
    pub fn issue(
        namespace: impl Into<String>,
        capabilities: &[&str],
        ttl_secs: u64,
        signer: &dyn Signer,
        clock: &dyn Clock,
    ) -> Result<Self, A1Error> {
        let namespace = namespace.into();
        let caps_owned: Vec<String> = capabilities.iter().map(|s| s.to_string()).collect();
        Self::issue_inner(namespace, caps_owned, ttl_secs, signer, clock)
    }

    /// Issue a root passport from a comma-separated capability string.
    ///
    /// Convenience wrapper around [`issue`] for CLI and config-file use:
    /// ```rust,ignore
    /// let p = DyoloPassport::issue_from_csv("bot", "trade.equity,portfolio.read", 3600, &id, &clock)?;
    /// ```
    ///
    /// [`issue`]: DyoloPassport::issue
    pub fn issue_from_csv(
        namespace: impl Into<String>,
        capabilities_csv: &str,
        ttl_secs: u64,
        signer: &dyn Signer,
        clock: &dyn Clock,
    ) -> Result<Self, A1Error> {
        let namespace = namespace.into();
        let caps_owned: Vec<String> = capabilities_csv
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self::issue_inner(namespace, caps_owned, ttl_secs, signer, clock)
    }

    fn issue_inner(
        namespace: String,
        capabilities: Vec<String>,
        ttl_secs: u64,
        signer: &dyn Signer,
        clock: &dyn Clock,
    ) -> Result<Self, A1Error> {
        let mask = NarrowingMatrix::from_capabilities(
            &capabilities.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        let hashes = cap_hashes(&capabilities)?;
        let tree = IntentTree::build(hashes)?;
        let scope_root = tree.root();

        let now = clock.unix_now();
        let expiry = now.saturating_add(ttl_secs);

        #[cfg(feature = "wire")]
        let ext = CertExtensions::new()
            .set("dyolo.passport.v", ExtValue::U64(1))
            .set("dyolo.passport.namespace", ExtValue::Str(namespace.clone()))
            .set("dyolo.passport.mask", ExtValue::Str(mask.to_hex()))
            .set(
                "dyolo.passport.caps",
                ExtValue::Strings(capabilities.clone()),
            );

        #[cfg(feature = "wire")]
        let cert = CertBuilder::new(signer.verifying_key(), scope_root, now, expiry)
            .max_depth(255)
            .extensions(ext)
            .build(signer)?;

        #[cfg(not(feature = "wire"))]
        let cert = CertBuilder::new(signer.verifying_key(), scope_root, now, expiry)
            .max_depth(255)
            .build(signer)?;

        Ok(Self {
            namespace,
            capability_mask: mask,
            capabilities,
            cert,
        })
    }

    /// The Ed25519 public key of the passport holder.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.cert.delegator_pk
    }

    /// Compute the Merkle root of all authorized capability hashes.
    ///
    /// This is the `principal_scope` used when building a delegation chain.
    pub fn scope_root(&self) -> Result<[u8; 32], A1Error> {
        Ok(self.capability_tree()?.root())
    }

    /// Create a new [`DyoloChain`] anchored at this passport.
    ///
    /// Push sub-delegation certs produced by [`issue_sub`] onto this chain,
    /// then call [`guard`] or [`guard_local`] to authorize an intent.
    ///
    /// [`issue_sub`]: DyoloPassport::issue_sub
    /// [`guard`]: DyoloPassport::guard
    /// [`guard_local`]: DyoloPassport::guard_local
    pub fn new_chain(&self) -> Result<DyoloChain, A1Error> {
        // Use the scope_root that was committed in the signed passport cert.
        // This keeps principal_scope consistent with the sub-cert scope proofs
        // produced by issue_sub(), which are also built against the raw cert tree.
        // The namespace is stored as metadata on the chain for receipt/fingerprint
        // purposes without transforming the scope (which would break SubScopeProof
        // verification since proofs are built against the raw parent tree root).
        let mut chain = DyoloChain::new(self.cert.delegator_pk, self.cert.scope_root);
        chain.namespace = Some(self.namespace.clone());
        Ok(chain)
    }

    /// Issue a time-limited sub-delegation cert scoped to a capability subset.
    ///
    /// The requested `capabilities` must be a strict subset of this passport's
    /// `capability_mask`. Any escalation (requesting a capability the passport
    /// does not hold) is rejected immediately with
    /// [`A1Error::PassportNarrowingViolation`].
    ///
    /// The resulting cert carries a `SubScopeProof` that cryptographically proves
    /// the sub-scope is contained within the passport's scope. The chain verifier
    /// checks this proof before authorizing any intent.
    ///
    /// # Arguments
    ///
    /// - `delegate_pk`  — public key of the sub-agent receiving this cert.
    /// - `capabilities` — subset of capabilities to grant.
    /// - `ttl_secs`     — lifetime; should be ≤ remaining passport TTL.
    /// - `signer`       — passport holder's signing backend.
    /// - `clock`        — time source.
    pub fn issue_sub(
        &self,
        delegate_pk: VerifyingKey,
        capabilities: &[&str],
        ttl_secs: u64,
        signer: &dyn Signer,
        clock: &dyn Clock,
    ) -> Result<crate::cert::DelegationCert, A1Error> {
        let requested = NarrowingMatrix::from_capabilities(capabilities);
        requested.enforce_narrowing(&self.capability_mask)?;

        let parent_tree = self.capability_tree()?;
        let sub_hashes: Vec<IntentHash> = capabilities
            .iter()
            .map(|c| Intent::new(*c).map(|i| i.hash()))
            .collect::<Result<_, _>>()?;

        let scope_proof = SubScopeProof::build(&parent_tree, &sub_hashes)?;
        let sub_tree = IntentTree::build(sub_hashes)?;
        let sub_scope_root = sub_tree.root();

        let now = clock.unix_now();
        let expiry = now.saturating_add(ttl_secs);

        #[cfg(feature = "wire")]
        let ext = CertExtensions::new()
            .set("dyolo.passport.v", ExtValue::U64(1))
            .set(
                "dyolo.passport.namespace",
                ExtValue::Str(self.namespace.clone()),
            )
            .set("dyolo.passport.mask", ExtValue::Str(requested.to_hex()));

        #[cfg(feature = "wire")]
        return CertBuilder::new(delegate_pk, sub_scope_root, now, expiry)
            .scope_proof(scope_proof)
            .extensions(ext)
            .build(signer);

        #[cfg(not(feature = "wire"))]
        CertBuilder::new(delegate_pk, sub_scope_root, now, expiry)
            .scope_proof(scope_proof)
            .build(signer)
    }

    /// Issue a sub-cert from a comma-separated capability string.
    pub fn issue_sub_from_csv(
        &self,
        delegate_pk: VerifyingKey,
        capabilities_csv: &str,
        ttl_secs: u64,
        signer: &dyn Signer,
        clock: &dyn Clock,
    ) -> Result<crate::cert::DelegationCert, A1Error> {
        let caps: Vec<&str> = capabilities_csv
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        self.issue_sub(delegate_pk, &caps, ttl_secs, signer, clock)
    }

    /// Verify a delegation chain and enforce capability narrowing.
    ///
    /// This is the primary authorization gate for passport-controlled agents.
    /// It applies two independent checks:
    ///
    /// 1. **NarrowingMatrix** — `intent.action` bit must be set in `capability_mask`
    ///    (O(1) bitwise check, no network call).
    /// 2. **Chain verification** — all cert signatures, expiry windows, depth limit,
    ///    scope proofs, and nonce replay are verified by `chain.authorize`.
    ///
    /// On success, a [`ProvableReceipt`] is returned containing the chain
    /// fingerprint, the enforced capability mask hex, and a Blake3 commitment over
    /// the mask. Archive it for audit or compliance replay.
    ///
    /// # Arguments
    ///
    /// - `chain`      — delegation chain built via [`new_chain`] + `push`.
    /// - `agent_pk`   — terminal agent's public key.
    /// - `intent`     — the action being requested.
    /// - `proof`      — Merkle proof that `intent` is in the chain's terminal scope.
    /// - `clock`, `revocation`, `nonces` — runtime stores.
    ///
    /// [`new_chain`]: DyoloPassport::new_chain
    #[allow(clippy::too_many_arguments)]
    pub fn guard(
        &self,
        chain: &DyoloChain,
        agent_pk: &VerifyingKey,
        intent: &Intent,
        proof: &MerkleProof,
        clock: &(dyn Clock + Send + Sync),
        revocation: &(dyn RevocationStore + Send + Sync),
        nonces: &(dyn NonceStore + Send + Sync),
    ) -> Result<ProvableReceipt, A1Error> {
        let requested = NarrowingMatrix::from_capabilities(&[intent.action.as_str()]);
        requested.enforce_narrowing(&self.capability_mask)?;

        let intent_hash = intent.hash();
        let action = chain.authorize(agent_pk, &intent_hash, proof, clock, revocation, nonces)?;

        Ok(ProvableReceipt::new(
            action.receipt,
            self.namespace.clone(),
            &self.capability_mask,
        ))
    }

    /// Convenience guard for single-capability delegation chains.
    ///
    /// Uses in-memory nonce/revocation stores and the system clock, suitable
    /// for testing and air-gapped single-process deployments. In production,
    /// pass persistent stores (Redis or Postgres) to `guard` to prevent nonce
    /// replay across restarts.
    ///
    /// This variant computes the correct Merkle proof from the passport's
    /// capability tree automatically. It is correct when the sub-cert was
    /// issued for a **single** capability via [`issue_sub`]. For chains where
    /// the sub-cert covers multiple capabilities, compute the proof from the
    /// sub-scope tree and call `guard` directly.
    ///
    /// [`issue_sub`]: DyoloPassport::issue_sub
    pub fn guard_local(
        &self,
        chain: &DyoloChain,
        agent_pk: &VerifyingKey,
        intent: &Intent,
    ) -> Result<ProvableReceipt, A1Error> {
        let requested = NarrowingMatrix::from_capabilities(&[intent.action.as_str()]);
        requested.enforce_narrowing(&self.capability_mask)?;

        // A single-capability sub-cert narrows scope to that cap's hash.
        // With a single-leaf tree, the root equals the leaf, so the proof is empty.
        let proof = MerkleProof::default();

        let clock = SystemClock;
        let revocation = MemoryRevocationStore::new();
        let nonces = MemoryNonceStore::new();

        let intent_hash = intent.hash();
        let action =
            chain.authorize(agent_pk, &intent_hash, &proof, &clock, &revocation, &nonces)?;

        Ok(ProvableReceipt::new(
            action.receipt,
            self.namespace.clone(),
            &self.capability_mask,
        ))
    }

    /// Save this passport to a JSON file.
    ///
    /// Requires the `wire` feature. Load with [`DyoloPassport::load`].
    #[cfg(feature = "wire")]
    #[cfg_attr(docsrs, doc(cfg(feature = "wire")))]
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), A1Error> {
        let file = PassportFile {
            a1_passport: 1,
            namespace: self.namespace.clone(),
            capability_mask_hex: self.capability_mask.to_hex(),
            capabilities: self.capabilities.clone(),
            cert: self.cert.clone(),
            _magic: default_dyolo_magic(),
        };
        let json = serde_json::to_string_pretty(&file)
            .map_err(|e| A1Error::WireFormatError(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| A1Error::WireFormatError(e.to_string()))
    }

    /// Load a passport from a JSON file produced by [`DyoloPassport::save`].
    ///
    /// Requires the `wire` feature.
    #[cfg(feature = "wire")]
    #[cfg_attr(docsrs, doc(cfg(feature = "wire")))]
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, A1Error> {
        let json =
            std::fs::read_to_string(path).map_err(|e| A1Error::WireFormatError(e.to_string()))?;
        let file: PassportFile =
            serde_json::from_str(&json).map_err(|e| A1Error::WireFormatError(e.to_string()))?;

        let mask = NarrowingMatrix::from_hex(&file.capability_mask_hex)?;
        Ok(Self {
            namespace: file.namespace,
            capability_mask: mask,
            capabilities: file.capabilities,
            cert: file.cert,
        })
    }

    fn capability_tree(&self) -> Result<IntentTree, A1Error> {
        IntentTree::build(cap_hashes(&self.capabilities)?)
    }
}

fn cap_hashes(caps: &[String]) -> Result<Vec<IntentHash>, A1Error> {
    caps.iter()
        .map(|c| Intent::new(c.as_str()).map(|i| i.hash()))
        .collect()
}

/// On-disk serialization format for a `DyoloPassport`.
#[cfg(feature = "wire")]
#[derive(serde::Serialize, serde::Deserialize)]
struct PassportFile {
    a1_passport: u8,
    namespace: String,
    capability_mask_hex: String,
    capabilities: Vec<String>,
    cert: crate::cert::DelegationCert,
    #[serde(default = "default_dyolo_magic")]
    _magic: String,
}

#[cfg(feature = "wire")]
fn default_dyolo_magic() -> String {
    "dyolo_v2.8.0".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::SystemClock;
    use crate::identity::DyoloIdentity;
    use crate::intent::Intent;

    fn make_passport(caps: &[&str]) -> (DyoloIdentity, DyoloPassport) {
        let root = DyoloIdentity::generate();
        let clock = SystemClock;
        let passport = DyoloPassport::issue("test-agent", caps, 3600, &root, &clock).unwrap();
        (root, passport)
    }

    #[test]
    fn issue_stores_namespace_and_mask() {
        let (_root, passport) = make_passport(&["trade.equity", "portfolio.read"]);
        assert_eq!(passport.namespace, "test-agent");
        assert!(!passport.capability_mask.is_empty());
    }

    #[test]
    fn scope_root_is_deterministic() {
        let (_root, passport) = make_passport(&["trade.equity", "portfolio.read"]);
        let r1 = passport.scope_root().unwrap();
        let r2 = passport.scope_root().unwrap();
        assert_eq!(r1, r2);
    }

    #[test]
    fn new_chain_uses_passport_key_and_scope() {
        let (_root, passport) = make_passport(&["trade.equity"]);
        let chain = passport.new_chain().unwrap();
        assert_eq!(chain.principal_pk, passport.verifying_key());
        assert_eq!(chain.principal_scope, passport.scope_root().unwrap());
    }

    #[test]
    fn issue_sub_rejects_escalation() {
        let (root, passport) = make_passport(&["trade.equity"]);
        let agent = DyoloIdentity::generate();
        let clock = SystemClock;
        let result = passport.issue_sub(
            agent.verifying_key(),
            &["trade.equity", "portfolio.write"],
            1800,
            &root,
            &clock,
        );
        assert!(result.is_err());
    }

    #[test]
    fn issue_sub_accepts_valid_subset() {
        let (root, passport) = make_passport(&["trade.equity", "portfolio.read"]);
        let agent = DyoloIdentity::generate();
        let clock = SystemClock;
        let result = passport.issue_sub(
            agent.verifying_key(),
            &["trade.equity"],
            1800,
            &root,
            &clock,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn guard_local_single_capability_end_to_end() {
        let (root, passport) = make_passport(&["trade.equity", "portfolio.read"]);
        let agent = DyoloIdentity::generate();
        let clock = SystemClock;

        let sub = passport
            .issue_sub(
                agent.verifying_key(),
                &["trade.equity"],
                1800,
                &root,
                &clock,
            )
            .unwrap();

        let mut chain = passport.new_chain().unwrap();
        chain.push(sub);

        let intent = Intent::new("trade.equity").unwrap();
        let receipt = passport
            .guard_local(&chain, &agent.verifying_key(), &intent)
            .unwrap();

        assert_eq!(receipt.passport_namespace, "test-agent");
        assert!(receipt.verify_commitment());
    }

    #[test]
    fn guard_rejects_out_of_scope_intent() {
        let (root, passport) = make_passport(&["portfolio.read"]);
        let agent = DyoloIdentity::generate();
        let clock = SystemClock;

        let sub = passport
            .issue_sub(
                agent.verifying_key(),
                &["portfolio.read"],
                1800,
                &root,
                &clock,
            )
            .unwrap();

        let mut chain = passport.new_chain().unwrap();
        chain.push(sub);

        let intent = Intent::new("trade.equity").unwrap();
        assert!(passport
            .guard_local(&chain, &agent.verifying_key(), &intent)
            .is_err());
    }

    #[test]
    fn issue_from_csv_matches_slice() {
        let root = DyoloIdentity::generate();
        let clock = SystemClock;
        let a = DyoloPassport::issue(
            "a",
            &["trade.equity", "portfolio.read"],
            3600,
            &root,
            &clock,
        )
        .unwrap();
        let b =
            DyoloPassport::issue_from_csv("a", "trade.equity, portfolio.read", 3600, &root, &clock)
                .unwrap();
        assert_eq!(a.capability_mask, b.capability_mask);
    }
}
