use crate::cert::DelegationCert;
use crate::chain::DyoloChain;
use crate::error::A1Error;
use crate::intent::Intent;

// ── PolicyViolation ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PolicyViolation {
    ChainDepthExceeded { allowed: u8, actual: usize },
    IntentNotAllowed { intent: String, policy: String },
    TtlExceedsMaximum { allowed_secs: u64, actual_secs: u64 },
    SubDelegationForbidden,
    RequiredExtensionAbsent { key: String },
    PrincipalNotTrusted,
}

impl std::fmt::Display for PolicyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChainDepthExceeded { allowed, actual } => {
                write!(f, "chain depth {actual} exceeds policy maximum {allowed}")
            }
            Self::IntentNotAllowed { intent, policy } => {
                write!(f, "intent '{intent}' not permitted under policy '{policy}'")
            }
            Self::TtlExceedsMaximum {
                allowed_secs,
                actual_secs,
            } => write!(
                f,
                "cert TTL {actual_secs}s exceeds policy maximum {allowed_secs}s"
            ),
            Self::SubDelegationForbidden => {
                write!(f, "sub-delegation is not permitted under this policy")
            }
            Self::RequiredExtensionAbsent { key } => {
                write!(f, "required extension '{key}' is absent from cert")
            }
            Self::PrincipalNotTrusted => {
                write!(f, "principal public key is not in the trusted set")
            }
        }
    }
}

impl std::error::Error for PolicyViolation {}

// ── CapabilitySet ─────────────────────────────────────────────────────────────

/// A set of intent action prefixes that an agent is allowed to perform.
///
/// A `CapabilitySet` is matched prefix-first: the prefix `"trade."` allows
/// `"trade.equity"`, `"trade.fx"`, and any other action that starts with
/// that string. The wildcard `"*"` allows all actions.
///
/// # Construction
///
/// ```rust
/// use a1::policy::CapabilitySet;
///
/// let caps = CapabilitySet::new()
///     .allow("trade.equity")
///     .allow("query.portfolio");
///
/// assert!(caps.permits("trade.equity"));
/// assert!(!caps.permits("trade.crypto"));
/// ```
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapabilitySet {
    prefixes: Vec<String>,
}

impl CapabilitySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn wildcard() -> Self {
        Self {
            prefixes: vec!["*".to_owned()],
        }
    }

    pub fn allow(mut self, prefix: impl Into<String>) -> Self {
        self.prefixes.push(prefix.into());
        self
    }

    pub fn permits(&self, action: &str) -> bool {
        self.prefixes.iter().any(|p| {
            if p == "*" {
                return true;
            }
            if p.ends_with('.') || p.ends_with('*') {
                let stem = p.trim_end_matches(['*', '.']);
                action.starts_with(stem)
            } else {
                action == p.as_str()
            }
        })
    }

    pub fn is_empty(&self) -> bool {
        self.prefixes.is_empty()
    }
}

// ── DelegationPolicy ─────────────────────────────────────────────────────────

/// Declarative policy governing what a delegation chain is permitted to do.
///
/// Attach a `DelegationPolicy` to a [`DyoloChain::authorize_with_policy`]
/// call to enforce enterprise-grade guardrails on top of the cryptographic
/// chain validation.
///
/// Policies compose: build a base policy and layer environment-specific
/// restrictions on top without modifying the base.
///
/// # Example
///
/// ```rust
/// use a1::policy::{DelegationPolicy, CapabilitySet};
///
/// let policy = DelegationPolicy::new("fintech-trading")
///     .max_chain_depth(3)
///     .max_ttl_secs(3600)
///     .capabilities(
///         CapabilitySet::new()
///             .allow("trade.equity")
///             .allow("query.portfolio")
///     )
///     .forbid_sub_delegation();
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DelegationPolicy {
    name: String,
    max_chain_depth: Option<u8>,
    max_ttl_secs: Option<u64>,
    capabilities: Option<CapabilitySet>,
    allow_sub_delegation: bool,
    required_extensions: Vec<String>,
    trusted_principal_pks: Vec<[u8; 32]>,
}

impl DelegationPolicy {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            max_chain_depth: None,
            max_ttl_secs: None,
            capabilities: None,
            allow_sub_delegation: true,
            required_extensions: Vec::new(),
            trusted_principal_pks: Vec::new(),
        }
    }

    pub fn permissive() -> Self {
        Self::new("permissive")
    }

    pub fn max_chain_depth(mut self, depth: u8) -> Self {
        self.max_chain_depth = Some(depth);
        self
    }

    pub fn max_ttl_secs(mut self, secs: u64) -> Self {
        self.max_ttl_secs = Some(secs);
        self
    }

    pub fn capabilities(mut self, caps: CapabilitySet) -> Self {
        self.capabilities = Some(caps);
        self
    }

    pub fn forbid_sub_delegation(mut self) -> Self {
        self.allow_sub_delegation = false;
        self
    }

    pub fn require_extension(mut self, key: impl Into<String>) -> Self {
        self.required_extensions.push(key.into());
        self
    }

    pub fn trust_principal(mut self, pk_bytes: [u8; 32]) -> Self {
        self.trusted_principal_pks.push(pk_bytes);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn check_chain(&self, chain: &DyoloChain) -> Result<(), PolicyViolation> {
        if !self.trusted_principal_pks.is_empty() {
            let pk = chain.principal_pk.as_bytes();
            let trusted = self.trusted_principal_pks.iter().any(|t| t == pk);
            if !trusted {
                return Err(PolicyViolation::PrincipalNotTrusted);
            }
        }

        if let Some(max_depth) = self.max_chain_depth {
            if chain.len() > max_depth as usize {
                return Err(PolicyViolation::ChainDepthExceeded {
                    allowed: max_depth,
                    actual: chain.len(),
                });
            }
        }

        for cert in chain.certs() {
            self.check_cert(cert)?;
        }

        Ok(())
    }

    pub fn check_cert(&self, cert: &DelegationCert) -> Result<(), PolicyViolation> {
        if let Some(max_ttl) = self.max_ttl_secs {
            let ttl = cert.expiration_unix.saturating_sub(cert.issued_at);
            if ttl > max_ttl {
                return Err(PolicyViolation::TtlExceedsMaximum {
                    allowed_secs: max_ttl,
                    actual_secs: ttl,
                });
            }
        }

        if !self.allow_sub_delegation {
            let has_sub =
                !cert.scope_proof.subset_intents.is_empty() || !cert.scope_proof.proofs.is_empty();
            if has_sub && cert.max_depth > 0 {
                return Err(PolicyViolation::SubDelegationForbidden);
            }
        }

        #[cfg(feature = "wire")]
        {
            for key in &self.required_extensions {
                if cert.extensions.get(key).is_none() {
                    return Err(PolicyViolation::RequiredExtensionAbsent { key: key.clone() });
                }
            }
        }
        #[cfg(not(feature = "wire"))]
        {
            if !self.required_extensions.is_empty() && cert.extensions_hash.is_none() {
                return Err(PolicyViolation::RequiredExtensionAbsent {
                    key: "extensions missing (compile with wire feature to inspect)".into(),
                });
            }
        }

        Ok(())
    }

    pub fn check_intent(&self, intent: &Intent) -> Result<(), PolicyViolation> {
        if let Some(caps) = &self.capabilities {
            if !caps.permits(&intent.action) {
                return Err(PolicyViolation::IntentNotAllowed {
                    intent: intent.action.clone(),
                    policy: self.name.clone(),
                });
            }
        }
        Ok(())
    }
}

// ── PolicySet ─────────────────────────────────────────────────────────────────

/// An ordered list of policies evaluated left-to-right.
///
/// The first policy that produces a `PolicyViolation` short-circuits
/// evaluation. All policies must pass for the chain to be accepted.
///
/// Use `PolicySet` when a deployment has multiple policy layers —
/// for example a base org policy composed with a team-specific restriction.
#[derive(Debug, Default)]
pub struct PolicySet {
    policies: Vec<DelegationPolicy>,
}

impl PolicySet {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, policy: DelegationPolicy) -> Self {
        self.policies.push(policy);
        self
    }

    pub fn check_chain(&self, chain: &DyoloChain) -> Result<(), A1Error> {
        for policy in &self.policies {
            policy
                .check_chain(chain)
                .map_err(|v| A1Error::PolicyViolation(v.to_string()))?;
        }
        Ok(())
    }

    pub fn check_intent(&self, intent: &Intent) -> Result<(), A1Error> {
        for policy in &self.policies {
            policy
                .check_intent(intent)
                .map_err(|v| A1Error::PolicyViolation(v.to_string()))?;
        }
        Ok(())
    }

    /// Parse a YAML string into a `PolicySet`.
    ///
    /// Requires the `policy-yaml` feature.
    #[cfg(feature = "policy-yaml")]
    #[cfg_attr(docsrs, doc(cfg(feature = "policy-yaml")))]
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum PolicyInput {
            Single(DelegationPolicy),
            List(Vec<DelegationPolicy>),
        }

        let input: PolicyInput = serde_yaml::from_str(yaml)?;
        match input {
            PolicyInput::Single(p) => Ok(Self { policies: vec![p] }),
            PolicyInput::List(policies) => Ok(Self { policies }),
        }
    }

    /// Parse a JSON string into a `PolicySet`.
    #[cfg(feature = "serde")]
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum PolicyInput {
            Single(DelegationPolicy),
            List(Vec<DelegationPolicy>),
        }

        let input: PolicyInput = serde_json::from_str(json)?;
        match input {
            PolicyInput::Single(p) => Ok(Self { policies: vec![p] }),
            PolicyInput::List(policies) => Ok(Self { policies }),
        }
    }
    
    /// Export the policy set to a YAML string for Policy-as-Code pipelines.
    #[cfg(feature = "policy-yaml")]
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(&self.policies)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(deprecated)]
    use crate::{
        cert::CertBuilder,
        chain::DyoloChain,
        identity::DyoloIdentity,
        intent::{intent_hash, IntentTree},
    };

    #[allow(deprecated)]
    fn make_chain(depth: usize, ttl: u64) -> DyoloChain {
        let root = DyoloIdentity::generate();
        let scope = IntentTree::build(vec![intent_hash("trade.equity", b"")])
            .unwrap()
            .root();
        let now = 1_700_000_000u64;
        let mut chain = DyoloChain::new(root.verifying_key(), scope);
        let mut prev = root;
        for _ in 0..depth {
            let next = DyoloIdentity::generate();
            let cert = CertBuilder::new(next.verifying_key(), scope, now, now + ttl).sign(&prev);
            chain.push(cert);
            prev = next;
        }
        chain
    }

    #[test]
    fn depth_policy_enforced() {
        let chain = make_chain(3, 3600);
        let policy = DelegationPolicy::new("test").max_chain_depth(2);
        assert!(policy.check_chain(&chain).is_err());
        let policy = DelegationPolicy::new("test").max_chain_depth(5);
        assert!(policy.check_chain(&chain).is_ok());
    }

    #[test]
    fn ttl_policy_enforced() {
        let chain = make_chain(1, 7200);
        let policy = DelegationPolicy::new("test").max_ttl_secs(3600);
        assert!(policy.check_chain(&chain).is_err());
    }

    #[test]
    fn capability_set_prefix_matching() {
        let caps = CapabilitySet::new().allow("trade.").allow("query");
        assert!(caps.permits("trade.equity"));
        assert!(caps.permits("trade.fx"));
        assert!(caps.permits("query"));
        assert!(!caps.permits("admin.delete"));
    }

    #[test]
    fn wildcard_permits_all() {
        let caps = CapabilitySet::wildcard();
        assert!(caps.permits("anything.at.all"));
    }

    #[test]
    fn intent_checked_against_capabilities() {
        let policy =
            DelegationPolicy::new("test").capabilities(CapabilitySet::new().allow("trade.equity"));
        let allowed = Intent::new("trade.equity").unwrap();
        let forbidden = Intent::new("admin.delete").unwrap();
        assert!(policy.check_intent(&allowed).is_ok());
        assert!(policy.check_intent(&forbidden).is_err());
    }
}
