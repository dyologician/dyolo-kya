//! YAML/JSON policy-as-code for a1.
//!
//! `PolicyDocument` lets operations teams define delegation constraints in
//! a human-editable document, checked into source control, without writing
//! Rust. The document is parsed and compiled into a [`a1::DelegationPolicy`]
//! at startup or via a GitOps pipeline.
//!
//! # Example YAML
//!
//! ```yaml
//! name: fintech-trading-policy
//! max_chain_depth: 3
//! max_ttl_secs: 3600
//! forbid_sub_delegation: true
//! capabilities:
//!   - "trade.equity"
//!   - "query.portfolio"
//! required_extensions:
//!   - "a1.cost_center"
//! ```

pub use a1::policy::{CapabilitySet, DelegationPolicy, PolicySet, PolicyViolation};

// ── PolicyIntent ──────────────────────────────────────────────────────────────

/// An intent declared inside a `PolicyDocument`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolicyIntent {
    pub name: String,
    #[cfg_attr(feature = "serde", serde(default))]
    pub params: std::collections::HashMap<String, String>,
}

// ── PolicyCert ────────────────────────────────────────────────────────────────

/// A cert specification declared inside a `PolicyDocument`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolicyCert {
    pub delegate_pk_hex: String,
    pub intents: Vec<PolicyIntent>,
    #[cfg_attr(feature = "serde", serde(default = "default_ttl"))]
    pub ttl_seconds: u64,
    #[cfg_attr(feature = "serde", serde(default = "default_max_depth"))]
    pub max_depth: u8,
}

fn default_ttl() -> u64 {
    3600
}
fn default_max_depth() -> u8 {
    16
}

// ── PolicyDocument ────────────────────────────────────────────────────────────

/// A human-editable YAML or JSON delegation policy document.
///
/// Compiles into a [`a1::DelegationPolicy`] via [`PolicyDocument::into_policy`].
///
/// Requires `features = ["serde"]` on `a1` for JSON support, and
/// `features = ["policy"]` on `a1-identity` for YAML support.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PolicyDocument {
    /// Unique name — appears in audit logs and error messages.
    pub name: String,

    /// Maximum delegation chain depth (number of certs).
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_chain_depth: Option<u8>,

    /// Maximum certificate TTL in seconds.
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_ttl_secs: Option<u64>,

    /// Whether sub-delegation is prohibited.
    #[cfg_attr(feature = "serde", serde(default))]
    pub forbid_sub_delegation: bool,

    /// Allowed intent prefixes. Omit to allow all.
    #[cfg_attr(feature = "serde", serde(default))]
    pub capabilities: Vec<String>,

    /// Extension keys that must be present on every cert.
    #[cfg_attr(feature = "serde", serde(default))]
    pub required_extensions: Vec<String>,

    /// Cert templates to issue when this policy is applied.
    #[cfg_attr(feature = "serde", serde(default))]
    pub certs: Vec<PolicyCert>,
}

impl PolicyDocument {
    /// Validates the structural integrity and bounds of the policy fields.
    /// Call this immediately after loading from YAML/JSON to fail fast.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.name.trim().is_empty() {
            errors.push("Policy name cannot be empty".into());
        }
        if let Some(depth) = self.max_chain_depth {
            if depth == 0 {
                errors.push("max_chain_depth must be > 0".into());
            }
        }
        if let Some(ttl) = self.max_ttl_secs {
            if ttl == 0 {
                errors.push("max_ttl_secs must be > 0".into());
            }
        }
        for (i, cert) in self.certs.iter().enumerate() {
            if cert.delegate_pk_hex.len() != 64 || hex::decode(&cert.delegate_pk_hex).is_err() {
                errors.push(format!(
                    "Invalid delegate_pk_hex in cert template {} for policy '{}'",
                    i, self.name
                ));
            }
            if cert.ttl_seconds == 0 {
                errors.push(format!("cert template {} ttl_seconds must be > 0", i));
            }
            if cert.intents.is_empty() {
                errors.push(format!(
                    "cert template {} must specify at least one intent",
                    i
                ));
            } else {
                // Semantic check: flag likely typos in intent names
                for intent in &cert.intents {
                    if intent.name.is_empty() {
                        errors.push(format!("cert template {} has an empty intent name", i));
                    }
                    if intent.name.starts_with("trad.") {
                        errors.push(format!(
                            "cert template {} intent '{}' looks like a typo of 'trade.'",
                            i, intent.name
                        ));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Compile this document into a [`DelegationPolicy`] for use with
    /// [`a1::DyoloChain::authorize_with_options`].
    pub fn into_policy(self) -> DelegationPolicy {
        let mut policy = DelegationPolicy::new(self.name);

        if let Some(depth) = self.max_chain_depth {
            policy = policy.max_chain_depth(depth);
        }

        if let Some(ttl) = self.max_ttl_secs {
            policy = policy.max_ttl_secs(ttl);
        }

        if self.forbid_sub_delegation {
            policy = policy.forbid_sub_delegation();
        }

        if !self.capabilities.is_empty() {
            let mut caps = CapabilitySet::new();
            for cap in self.capabilities {
                caps = caps.allow(cap);
            }
            policy = policy.capabilities(caps);
        }

        for key in self.required_extensions {
            policy = policy.require_extension(key);
        }

        policy
    }

    /// Parse a YAML string into a `PolicyDocument`.
    ///
    /// Requires the `policy` Cargo feature on `a1-identity`.
    #[cfg(feature = "policy")]
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Parse a JSON string into a `PolicyDocument`.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
