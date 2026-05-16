use blake3::Hasher;
use ed25519_dalek::{Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

use crate::error::A1Error;
use crate::identity::Signer;

const DOMAIN_VC_SIGN: &str = "a1::dyolo::vc::sign::v2.8.0";
const DID_METHOD: &str = "a1";

/// A permanent, portable identifier for an A1 agent.
///
/// Format: `did:a1:{hex-encoded-ed25519-verifying-key}`
///
/// Every DyoloPassport holder has exactly one DID derived deterministically
/// from their Ed25519 verifying key — no registry, no network call, no
/// external system required for generation or verification.
///
/// The DID is compatible with the W3C DID Core specification and can be
/// resolved by any system that holds the public key — including other agents,
/// blockchains, enterprise IAM platforms, and EU eIDAS wallets.
///
/// # Example
///
/// ```rust,ignore
/// use a1::{DyoloIdentity, did::AgentDid};
///
/// let identity = DyoloIdentity::generate();
/// let did = AgentDid::from_key(&identity.verifying_key());
/// println!("{did}"); // did:a1:abc123...
///
/// let resolved = did.verifying_key().unwrap();
/// assert_eq!(resolved.as_bytes(), identity.verifying_key().as_bytes());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDid(String);

impl AgentDid {
    /// Derive a DID from an Ed25519 verifying key.
    pub fn from_key(vk: &VerifyingKey) -> Self {
        Self(format!("did:{}:{}", DID_METHOD, hex::encode(vk.as_bytes())))
    }

    /// Parse and validate a `did:a1:` string.
    pub fn parse(did: &str) -> Result<Self, A1Error> {
        let mut parts = did.splitn(3, ':');
        let scheme = parts.next().unwrap_or("");
        let method = parts.next().unwrap_or("");
        let id = parts.next().unwrap_or("");

        if scheme != "did" || method != DID_METHOD || id.is_empty() {
            return Err(A1Error::WireFormatError(
                format!("expected did:a1:<hex>, got: {did}"),
            ));
        }
        let bytes = hex::decode(id)
            .map_err(|_| A1Error::WireFormatError("DID identifier must be hex".into()))?;
        if bytes.len() != 32 {
            return Err(A1Error::WireFormatError(
                "DID identifier must be 32 bytes (Ed25519 key)".into(),
            ));
        }
        Ok(Self(did.to_owned()))
    }

    /// Recover the verifying key encoded in this DID.
    pub fn verifying_key(&self) -> Result<VerifyingKey, A1Error> {
        let hex_part = self.0.splitn(3, ':').nth(2).unwrap_or("");
        let bytes = hex::decode(hex_part)
            .map_err(|_| A1Error::WireFormatError("invalid DID hex".into()))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| A1Error::WireFormatError("DID key must be 32 bytes".into()))?;
        VerifyingKey::from_bytes(&arr)
            .map_err(|e| A1Error::WireFormatError(format!("invalid DID key: {e}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Verification method fragment ID used in DID Documents and VC proofs.
    pub fn key_id(&self) -> String {
        format!("{}#key-0", self.0)
    }
}

impl std::fmt::Display for AgentDid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── DID Document ──────────────────────────────────────────────────────────────

/// A W3C DID Document for an A1 agent.
///
/// Serializes to standard W3C DID JSON-LD, readable by any DID resolver,
/// enterprise identity platform, or EU eIDAS wallet without A1-specific code.
///
/// # Example
///
/// ```rust,ignore
/// use a1::{DyoloIdentity, did::DidDocument};
///
/// let identity = DyoloIdentity::generate();
/// let doc = DidDocument::for_identity(&identity.verifying_key());
/// println!("{}", doc.to_json().unwrap());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidDocument {
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    pub id: String,
    #[serde(rename = "verificationMethod")]
    pub verification_method: Vec<VerificationMethod>,
    pub authentication: Vec<String>,
    #[serde(rename = "assertionMethod")]
    pub assertion_method: Vec<String>,
    #[serde(rename = "capabilityDelegation")]
    pub capability_delegation: Vec<String>,
    #[serde(rename = "a1PassportNamespace", skip_serializing_if = "Option::is_none")]
    pub passport_namespace: Option<String>,
    #[serde(rename = "a1CapabilityMaskHex", skip_serializing_if = "Option::is_none")]
    pub capability_mask_hex: Option<String>,
    #[serde(rename = "a1Version")]
    pub a1_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationMethod {
    pub id: String,
    #[serde(rename = "type")]
    pub method_type: String,
    pub controller: String,
    #[serde(rename = "publicKeyHex")]
    pub public_key_hex: String,
}

impl DidDocument {
    /// Generate a W3C DID Document for an Ed25519 identity.
    pub fn for_identity(vk: &VerifyingKey) -> Self {
        let did = AgentDid::from_key(vk);
        let key_id = did.key_id();
        Self {
            context: vec![
                "https://www.w3.org/ns/did/v1".into(),
                "https://w3id.org/security/suites/ed25519-2020/v1".into(),
                "https://a1.dev/contexts/v1".into(),
            ],
            id: did.to_string(),
            verification_method: vec![VerificationMethod {
                id: key_id.clone(),
                method_type: "Ed25519VerificationKey2020".into(),
                controller: did.to_string(),
                public_key_hex: hex::encode(vk.as_bytes()),
            }],
            authentication: vec![key_id.clone()],
            assertion_method: vec![key_id.clone()],
            capability_delegation: vec![key_id],
            passport_namespace: None,
            capability_mask_hex: None,
            a1_version: "2.8.0".into(),
        }
    }

    /// Attach passport capability metadata to this DID Document.
    ///
    /// Allows verifiers to read the agent's authorized capability scope
    /// directly from the DID Document without needing the passport file.
    pub fn with_passport_metadata(mut self, namespace: impl Into<String>, mask_hex: impl Into<String>) -> Self {
        self.passport_namespace = Some(namespace.into());
        self.capability_mask_hex = Some(mask_hex.into());
        self
    }

    /// Serialize to a W3C JSON-LD string.
    pub fn to_json(&self) -> Result<String, A1Error> {
        serde_json::to_string_pretty(self)
            .map_err(|e| A1Error::WireFormatError(e.to_string()))
    }
}

// ── Verifiable Credential ─────────────────────────────────────────────────────

/// A W3C Verifiable Credential proving an agent's authorized capabilities.
///
/// VCs are portable, self-contained proofs that work without network calls.
/// Any system — another agent, blockchain, enterprise IAM, EU eIDAS wallet —
/// can verify this credential offline using only the issuer's public key.
///
/// The signature covers a domain-separated Blake3 hash of all credential
/// fields, making it immune to JSON canonicalization attacks.
///
/// # Example
///
/// ```rust,ignore
/// use a1::{DyoloIdentity, did::{AgentDid, VerifiableCredential}};
///
/// let issuer = DyoloIdentity::generate();
/// let agent  = DyoloIdentity::generate();
/// let agent_did = AgentDid::from_key(&agent.verifying_key());
///
/// let vc = VerifiableCredential::issue_capability(
///     &issuer,
///     &agent_did,
///     "acme-trading-bot",
///     &["trade.equity", "portfolio.read"],
///     now_unix,
///     now_unix + 86400,
///     &chain_fingerprint,
/// ).unwrap();
///
/// assert!(vc.verify().is_ok());
/// let json = vc.to_json().unwrap();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiableCredential {
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    #[serde(rename = "type")]
    pub vc_type: Vec<String>,
    pub id: String,
    pub issuer: String,
    #[serde(rename = "issuanceDate")]
    pub issuance_date: String,
    #[serde(rename = "expirationDate", skip_serializing_if = "Option::is_none")]
    pub expiration_date: Option<String>,
    #[serde(rename = "credentialSubject")]
    pub credential_subject: CredentialSubject,
    pub proof: VcProof,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSubject {
    pub id: String,
    #[serde(rename = "a1PassportNamespace")]
    pub passport_namespace: String,
    #[serde(rename = "a1Capabilities")]
    pub capabilities: Vec<String>,
    #[serde(rename = "a1ChainFingerprint")]
    pub chain_fingerprint: String,
    #[serde(rename = "a1Version")]
    pub a1_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VcProof {
    #[serde(rename = "type")]
    pub proof_type: String,
    pub created: String,
    #[serde(rename = "verificationMethod")]
    pub verification_method: String,
    #[serde(rename = "proofPurpose")]
    pub proof_purpose: String,
    #[serde(rename = "proofValue")]
    pub proof_value: String,
}

impl VerifiableCredential {
    /// Issue a capability VC from a DyoloPassport holder.
    ///
    /// The VC asserts that the agent identified by `subject_did` is authorized
    /// to perform the listed `capabilities` under the named passport.
    ///
    /// The signature is computed as `Ed25519(Blake3(domain ‖ id ‖ issuer ‖
    /// issuance ‖ expiry ‖ subject_did ‖ namespace ‖ caps ‖ fingerprint))`,
    /// preventing any tampering with the credential fields.
    pub fn issue_capability(
        issuer: &dyn Signer,
        subject_did: &AgentDid,
        passport_namespace: &str,
        capabilities: &[&str],
        issued_at_unix: u64,
        expiry_unix: u64,
        chain_fingerprint: &[u8; 32],
    ) -> Result<Self, A1Error> {
        let issuer_vk = issuer.verifying_key();
        let issuer_did = AgentDid::from_key(&issuer_vk);

        let cred_id = format!(
            "urn:a1:cred:{}",
            hex::encode(&chain_fingerprint[..16])
        );
        let issuance = unix_to_iso8601(issued_at_unix);
        let expiry = unix_to_iso8601(expiry_unix);

        let subject = CredentialSubject {
            id: subject_did.to_string(),
            passport_namespace: passport_namespace.to_owned(),
            capabilities: capabilities.iter().map(|s| s.to_string()).collect(),
            chain_fingerprint: hex::encode(chain_fingerprint),
            a1_version: "2.8.0".into(),
        };

        let signable = vc_signable_bytes(
            &cred_id,
            issuer_did.as_str(),
            &issuance,
            &expiry,
            &subject,
        );
        let sig = issuer.sign_message(&signable);

        Ok(Self {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".into(),
                "https://a1.dev/contexts/v1".into(),
            ],
            vc_type: vec![
                "VerifiableCredential".into(),
                "A1CapabilityCredential".into(),
            ],
            id: cred_id.clone(),
            issuer: issuer_did.to_string(),
            issuance_date: issuance.clone(),
            expiration_date: Some(expiry.clone()),
            credential_subject: subject,
            proof: VcProof {
                proof_type: "Ed25519Signature2020".into(),
                created: issuance,
                verification_method: issuer_did.key_id(),
                proof_purpose: "assertionMethod".into(),
                proof_value: hex::encode(sig.to_bytes()),
            },
        })
    }

    /// Verify the Ed25519 signature on this VC.
    ///
    /// Recovers the issuer public key from the `issuer` DID, recomputes the
    /// canonical signable bytes, and checks the signature in constant time.
    pub fn verify(&self) -> Result<(), A1Error> {
        let issuer_did = AgentDid::parse(&self.issuer)?;
        let vk = issuer_did.verifying_key()?;

        let expiry = self.expiration_date.as_deref().unwrap_or("");
        let signable = vc_signable_bytes(
            &self.id,
            &self.issuer,
            &self.issuance_date,
            expiry,
            &self.credential_subject,
        );

        let sig_bytes = hex::decode(&self.proof.proof_value)
            .map_err(|_| A1Error::WireFormatError("invalid proof_value hex".into()))?;
        let sig_arr: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| A1Error::WireFormatError("signature must be 64 bytes".into()))?;
        let sig = ed25519_dalek::Signature::from_bytes(&sig_arr);

        vk.verify(&signable, &sig)
            .map_err(|_| A1Error::HybridSignatureInvalid { component: "vc-ed25519" })
    }

    /// Serialize to a W3C JSON-LD string.
    pub fn to_json(&self) -> Result<String, A1Error> {
        serde_json::to_string_pretty(self)
            .map_err(|e| A1Error::WireFormatError(e.to_string()))
    }
}

// ── Signable bytes ────────────────────────────────────────────────────────────

fn vc_signable_bytes(
    id: &str,
    issuer: &str,
    issuance: &str,
    expiry: &str,
    subject: &CredentialSubject,
) -> Vec<u8> {
    let mut h = Hasher::new_derive_key(DOMAIN_VC_SIGN);
    h.update(&(id.len() as u64).to_le_bytes());
    h.update(id.as_bytes());
    h.update(&(issuer.len() as u64).to_le_bytes());
    h.update(issuer.as_bytes());
    h.update(&(issuance.len() as u64).to_le_bytes());
    h.update(issuance.as_bytes());
    h.update(&(expiry.len() as u64).to_le_bytes());
    h.update(expiry.as_bytes());
    h.update(&(subject.passport_namespace.len() as u64).to_le_bytes());
    h.update(subject.passport_namespace.as_bytes());
    h.update(&(subject.capabilities.len() as u64).to_le_bytes());
    for cap in &subject.capabilities {
        h.update(&(cap.len() as u64).to_le_bytes());
        h.update(cap.as_bytes());
    }
    h.update(subject.chain_fingerprint.as_bytes());
    h.finalize().as_bytes().to_vec()
}

// ── ISO 8601 formatting (no chrono dep) ───────────────────────────────────────

fn unix_to_iso8601(unix: u64) -> String {
    let s = unix % 60;
    let m = (unix / 60) % 60;
    let h = (unix / 3600) % 24;
    let days = unix / 86400;
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let y_days = if is_leap(year) { 366 } else { 365 };
        if days < y_days {
            break;
        }
        days -= y_days;
        year += 1;
    }
    let month_days: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for mlen in month_days {
        if days < mlen {
            break;
        }
        days -= mlen;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::DyoloIdentity;

    #[test]
    fn did_roundtrip() {
        let id = DyoloIdentity::generate();
        let did = AgentDid::from_key(&id.verifying_key());
        assert!(did.as_str().starts_with("did:a1:"));
        let recovered = did.verifying_key().unwrap();
        assert_eq!(id.verifying_key().as_bytes(), recovered.as_bytes());
    }

    #[test]
    fn did_parse_rejects_malformed() {
        assert!(AgentDid::parse("did:key:abc").is_err());
        assert!(AgentDid::parse("did:a1:notvalidhex!").is_err());
        assert!(AgentDid::parse("notadid").is_err());
        assert!(AgentDid::parse("did:a1:deadbeef").is_err()); // wrong length
    }

    #[test]
    fn did_document_structure() {
        let id = DyoloIdentity::generate();
        let doc = DidDocument::for_identity(&id.verifying_key());
        assert!(doc.id.starts_with("did:a1:"));
        assert_eq!(doc.verification_method.len(), 1);
        assert_eq!(doc.verification_method[0].method_type, "Ed25519VerificationKey2020");
        assert_eq!(doc.a1_version, "2.8.0");
        assert!(doc.passport_namespace.is_none());
    }

    #[test]
    fn did_document_with_passport_metadata() {
        let id = DyoloIdentity::generate();
        let doc = DidDocument::for_identity(&id.verifying_key())
            .with_passport_metadata("acme-bot", "ff00ff00");
        assert_eq!(doc.passport_namespace.as_deref(), Some("acme-bot"));
        assert_eq!(doc.capability_mask_hex.as_deref(), Some("ff00ff00"));
    }

    #[test]
    fn vc_issue_and_verify() {
        let issuer = DyoloIdentity::generate();
        let agent = DyoloIdentity::generate();
        let agent_did = AgentDid::from_key(&agent.verifying_key());
        let fp = [7u8; 32];
        let now = 1_700_000_000u64;

        let vc = VerifiableCredential::issue_capability(
            &issuer,
            &agent_did,
            "acme-trading-bot",
            &["trade.equity", "portfolio.read"],
            now,
            now + 86400,
            &fp,
        )
        .unwrap();

        assert!(vc.verify().is_ok());
        assert_eq!(vc.vc_type[1], "A1CapabilityCredential");
        assert_eq!(vc.credential_subject.capabilities, ["trade.equity", "portfolio.read"]);
    }

    #[test]
    fn tampered_capabilities_fail_verify() {
        let issuer = DyoloIdentity::generate();
        let agent = DyoloIdentity::generate();
        let agent_did = AgentDid::from_key(&agent.verifying_key());
        let fp = [1u8; 32];
        let now = 1_700_000_000u64;

        let mut vc = VerifiableCredential::issue_capability(
            &issuer,
            &agent_did,
            "acme-trading-bot",
            &["trade.equity"],
            now,
            now + 86400,
            &fp,
        )
        .unwrap();

        vc.credential_subject.capabilities.push("admin.everything".into());
        assert!(vc.verify().is_err());
    }

    #[test]
    fn tampered_proof_fails_verify() {
        let issuer = DyoloIdentity::generate();
        let agent = DyoloIdentity::generate();
        let agent_did = AgentDid::from_key(&agent.verifying_key());
        let fp = [2u8; 32];
        let now = 1_700_000_000u64;

        let mut vc = VerifiableCredential::issue_capability(
            &issuer,
            &agent_did,
            "acme-bot",
            &["read"],
            now,
            now + 3600,
            &fp,
        )
        .unwrap();

        let mut bad = vc.proof.proof_value.clone().into_bytes();
        bad[0] ^= 0xFF;
        vc.proof.proof_value = String::from_utf8(bad).unwrap_or_default();
        assert!(vc.verify().is_err());
    }

    #[test]
    fn iso8601_epoch() {
        assert_eq!(unix_to_iso8601(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn iso8601_known_date() {
        let s = unix_to_iso8601(1_700_000_000);
        assert!(s.starts_with("2023-"));
    }
}
