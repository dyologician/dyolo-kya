use crate::error::A1Error;
use crate::identity::Signer;
use crate::registry::fresh_nonce;
use blake3::Hasher;
use serde::{Deserialize, Serialize};

const DOMAIN_GOV_POLICY: &str = "a1::dyolo::governance::policy::v2.8.0";
const DOMAIN_GOV_APPROVAL: &str = "a1::dyolo::governance::approval::v2.8.0";
const DOMAIN_GOV_AUDIT: &str = "a1::dyolo::governance::audit::v2.8.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalGate {
    pub capability: String,
    pub approver_did: String,
    pub approval_ttl_secs: u64,
    #[serde(default)]
    pub allow_retroactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalToken {
    pub capability: String,
    pub approver_did: String,
    pub agent_did: String,
    pub action_id: String,
    pub granted_at_unix: u64,
    pub expires_at_unix: u64,
    pub nonce: String,
    pub signature: String,
}

impl ApprovalToken {
    pub fn issue(
        approver: &dyn Signer,
        capability: impl Into<String>,
        agent_did: impl Into<String>,
        action_id: impl Into<String>,
        granted_at: u64,
        ttl_secs: u64,
    ) -> Self {
        let vk = approver.verifying_key();
        let approver_did = format!("did:a1:{}", hex::encode(vk.as_bytes()));
        let capability = capability.into();
        let agent_did = agent_did.into();
        let action_id = action_id.into();
        let nonce = fresh_nonce();
        let expires = granted_at + ttl_secs;
        let msg = approval_signable(
            &capability,
            &approver_did,
            &agent_did,
            &action_id,
            granted_at,
            expires,
            &nonce,
        );
        let sig = approver.sign_message(&msg);
        Self {
            capability,
            approver_did,
            agent_did,
            action_id,
            granted_at_unix: granted_at,
            expires_at_unix: expires,
            nonce: hex::encode(nonce),
            signature: hex::encode(sig.to_bytes()),
        }
    }

    pub fn verify(&self, now_unix: u64) -> Result<(), A1Error> {
        if now_unix > self.expires_at_unix {
            return Err(A1Error::Expired(0, self.expires_at_unix, now_unix));
        }
        let pk_hex = self
            .approver_did
            .strip_prefix("did:a1:")
            .ok_or_else(|| A1Error::WireFormatError("invalid approver DID".into()))?;
        let pk_bytes =
            hex::decode(pk_hex).map_err(|_| A1Error::WireFormatError("invalid DID hex".into()))?;
        let pk_arr: [u8; 32] = pk_bytes
            .try_into()
            .map_err(|_| A1Error::WireFormatError("key must be 32 bytes".into()))?;
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&pk_arr)
            .map_err(|_| A1Error::WireFormatError("invalid Ed25519 key".into()))?;
        let nonce_bytes = hex::decode(&self.nonce)
            .map_err(|_| A1Error::WireFormatError("invalid nonce hex".into()))?;
        let nonce: [u8; 16] = nonce_bytes
            .try_into()
            .map_err(|_| A1Error::WireFormatError("nonce must be 16 bytes".into()))?;
        let msg = approval_signable(
            &self.capability,
            &self.approver_did,
            &self.agent_did,
            &self.action_id,
            self.granted_at_unix,
            self.expires_at_unix,
            &nonce,
        );
        let sig_bytes = hex::decode(&self.signature)
            .map_err(|_| A1Error::WireFormatError("invalid sig hex".into()))?;
        let sig_arr: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| A1Error::WireFormatError("sig must be 64 bytes".into()))?;
        let sig = ed25519_dalek::Signature::from_bytes(&sig_arr);
        use ed25519_dalek::Verifier;
        vk.verify(&msg, &sig)
            .map_err(|_| A1Error::HybridSignatureInvalid {
                component: "approval-token",
            })
    }
}

fn approval_signable(
    capability: &str,
    approver_did: &str,
    agent_did: &str,
    action_id: &str,
    granted_at: u64,
    expires_at: u64,
    nonce: &[u8; 16],
) -> Vec<u8> {
    let mut h = Hasher::new_derive_key(DOMAIN_GOV_APPROVAL);
    for s in [capability, approver_did, agent_did, action_id] {
        h.update(&(s.len() as u64).to_le_bytes());
        h.update(s.as_bytes());
    }
    h.update(&granted_at.to_le_bytes());
    h.update(&expires_at.to_le_bytes());
    h.update(nonce);
    h.finalize().as_bytes().to_vec()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationPolicy {
    pub max_age_days: u32,
    pub mandatory: bool,
    pub rotation_authority_did: Option<String>,
}

impl Default for KeyRotationPolicy {
    fn default() -> Self {
        Self {
            max_age_days: 90,
            mandatory: true,
            rotation_authority_did: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RotationStatus {
    Valid { days_remaining: u64 },
    Recommended { age_days: u64 },
    Required { age_days: u64 },
}

impl KeyRotationPolicy {
    pub fn check(&self, issued_at_unix: u64, now_unix: u64) -> RotationStatus {
        let age_days = now_unix.saturating_sub(issued_at_unix) / 86400;
        if age_days >= self.max_age_days as u64 {
            if self.mandatory {
                RotationStatus::Required { age_days }
            } else {
                RotationStatus::Recommended { age_days }
            }
        } else {
            RotationStatus::Valid {
                days_remaining: self.max_age_days as u64 - age_days,
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernancePolicy {
    #[serde(default)]
    pub approval_gates: Vec<ApprovalGate>,
    #[serde(default)]
    pub key_rotation: KeyRotationPolicy,
    #[serde(default = "default_max_depth")]
    pub max_chain_depth: u8,
    #[serde(default)]
    pub allowed_namespaces: Vec<String>,
    #[serde(default)]
    pub blocked_capabilities: Vec<String>,
    #[serde(default)]
    pub require_human_approval_for: Vec<String>,
    #[serde(default = "default_true")]
    pub audit_all_authorizations: bool,
}

fn default_max_depth() -> u8 {
    16
}
fn default_true() -> bool {
    true
}

impl Default for GovernancePolicy {
    fn default() -> Self {
        Self {
            approval_gates: vec![],
            key_rotation: KeyRotationPolicy::default(),
            max_chain_depth: 16,
            allowed_namespaces: vec![],
            blocked_capabilities: vec![],
            require_human_approval_for: vec![],
            audit_all_authorizations: true,
        }
    }
}

impl GovernancePolicy {
    pub fn from_json(json: &str) -> Result<Self, A1Error> {
        serde_json::from_str(json).map_err(|e| A1Error::WireFormatError(e.to_string()))
    }
    pub fn from_env() -> Result<Option<Self>, A1Error> {
        match std::env::var("A1_GOVERNANCE_POLICY_FILE") {
            Ok(path) => {
                let json = std::fs::read_to_string(&path)
                    .map_err(|e| A1Error::WireFormatError(format!("cannot read {path}: {e}")))?;
                Ok(Some(Self::from_json(&json)?))
            }
            Err(_) => Ok(None),
        }
    }
    pub fn check_namespace(&self, ns: &str) -> Result<(), A1Error> {
        if self.allowed_namespaces.is_empty() || self.allowed_namespaces.iter().any(|n| n == ns) {
            Ok(())
        } else {
            Err(A1Error::PolicyViolation(format!(
                "namespace '{ns}' not in governance allowlist"
            )))
        }
    }
    pub fn check_capability_not_blocked(&self, cap: &str) -> Result<(), A1Error> {
        if self.blocked_capabilities.iter().any(|c| c == cap) {
            Err(A1Error::PolicyViolation(format!(
                "capability '{cap}' is blocked by governance policy"
            )))
        } else {
            Ok(())
        }
    }
    pub fn check_chain_depth(&self, depth: usize) -> Result<(), A1Error> {
        if depth > self.max_chain_depth as usize {
            Err(A1Error::MaxDepthExceeded(depth, self.max_chain_depth))
        } else {
            Ok(())
        }
    }
    pub fn requires_human_approval(&self, cap: &str) -> bool {
        self.require_human_approval_for.iter().any(|c| c == cap)
    }
    pub fn commitment(&self) -> Result<[u8; 32], A1Error> {
        let json =
            serde_json::to_string(self).map_err(|e| A1Error::WireFormatError(e.to_string()))?;
        let mut h = Hasher::new_derive_key(DOMAIN_GOV_POLICY);
        h.update(json.as_bytes());
        Ok(h.finalize().into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub title: String,
    pub scope: String,
    pub period_start_unix: u64,
    pub period_end_unix: u64,
    pub total_authorizations: u64,
    pub denied_authorizations: u64,
    pub revocations_issued: u64,
    pub passports_due_rotation: u64,
    pub policy_commitment_hex: String,
    pub generated_at: String,
    pub compliance_standards: Vec<String>,
    pub findings: Vec<AuditFinding>,
    pub report_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    pub severity: FindingSeverity,
    pub code: String,
    pub description: String,
    pub count: u64,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingSeverity {
    Info,
    Warning,
    Critical,
}

impl AuditReport {
    pub fn new(
        scope: impl Into<String>,
        period_start: u64,
        period_end: u64,
        policy: &GovernancePolicy,
    ) -> Result<Self, A1Error> {
        let scope = scope.into();
        let policy_hex = hex::encode(policy.commitment()?);
        Ok(Self {
            title: format!("A1 Compliance Audit — {scope}"),
            scope,
            period_start_unix: period_start,
            period_end_unix: period_end,
            total_authorizations: 0,
            denied_authorizations: 0,
            revocations_issued: 0,
            passports_due_rotation: 0,
            policy_commitment_hex: policy_hex,
            generated_at: unix_to_iso(period_end),
            compliance_standards: vec![
                "EU AI Act (Art. 13, 14, 17)".into(),
                "NIST AI RMF Govern 1.7, 6.2".into(),
                "SOC 2 Type II (CC6.1, CC7.2)".into(),
                "ISO/IEC 27001:2022 A.9".into(),
            ],
            findings: vec![],
            report_hash: String::new(),
        })
    }
    pub fn add_finding(&mut self, f: AuditFinding) {
        self.findings.push(f);
    }
    pub fn finalize(&mut self) -> Result<(), A1Error> {
        let json =
            serde_json::to_string(self).map_err(|e| A1Error::WireFormatError(e.to_string()))?;
        let mut h = Hasher::new_derive_key(DOMAIN_GOV_AUDIT);
        h.update(json.as_bytes());
        self.report_hash = hex::encode(h.finalize().as_bytes());
        Ok(())
    }
}

fn unix_to_iso(unix: u64) -> String {
    let (s, m, h) = (unix % 60, (unix / 60) % 60, (unix / 3600) % 24);
    let mut d = unix / 86400;
    let mut y = 1970u64;
    loop {
        let yl = if (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400) {
            366
        } else {
            365
        };
        if d < yl {
            break;
        }
        d -= yl;
        y += 1;
    }
    let ml: [u64; 12] = if (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut mo = 1u64;
    for mlen in ml {
        if d < mlen {
            break;
        }
        d -= mlen;
        mo += 1;
    }
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}
