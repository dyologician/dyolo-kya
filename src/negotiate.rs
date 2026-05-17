use blake3::Hasher;
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

use crate::cert::DelegationCert;
use crate::error::A1Error;
use crate::identity::Signer;
use crate::registry::fresh_nonce;

const DOMAIN_NEG_REQUEST: &str = "a1::dyolo::negotiate::request::v2.8.0";
const DOMAIN_NEG_OFFER: &str = "a1::dyolo::negotiate::offer::v2.8.0";
const DOMAIN_NEG_ACCEPT: &str = "a1::dyolo::negotiate::accept::v2.8.0";

// ── Message types ─────────────────────────────────────────────────────────────

/// An agent's request for a delegated capability sub-cert.
///
/// Agent A sends a `CapabilityRequest` to Agent B (or B's gateway) when it
/// needs authorization to perform a specific action that B can delegate.
///
/// # Signature
///
/// `signature` is an Ed25519 signature over
/// `Blake3(DOMAIN || requester_did || nonce || timestamp || ttl || intent || caps...)`,
/// proving that the requester controls the private key for `requester_pk_hex`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    /// `did:a1:` identifier of the requesting agent.
    pub requester_did: String,
    /// Hex-encoded Ed25519 public key of the requesting agent.
    pub requester_pk_hex: String,
    /// Capabilities the requesting agent needs.
    pub requested_capabilities: Vec<String>,
    /// Name of the specific intent to be authorized.
    pub intent_name: String,
    /// Requested delegation lifetime in seconds.
    pub ttl_secs: u64,
    /// 16-byte anti-replay nonce (hex).
    pub nonce: String,
    /// Unix timestamp when this request was created.
    pub timestamp_unix: u64,
    /// Ed25519 signature over the canonical request bytes (hex).
    pub signature: String,
}

impl CapabilityRequest {
    /// Build and sign a capability request.
    pub fn build(
        requester: &dyn Signer,
        requested_capabilities: Vec<String>,
        intent_name: impl Into<String>,
        ttl_secs: u64,
        timestamp_unix: u64,
    ) -> Self {
        let vk = requester.verifying_key();
        let requester_did = format!("did:a1:{}", hex::encode(vk.as_bytes()));
        let nonce = fresh_nonce();
        let intent_str: String = intent_name.into();

        let msg = request_signable_bytes(
            &requester_did,
            &nonce,
            timestamp_unix,
            ttl_secs,
            &intent_str,
            &requested_capabilities,
        );
        let sig = requester.sign_message(&msg);

        Self {
            requester_did,
            requester_pk_hex: hex::encode(vk.as_bytes()),
            requested_capabilities,
            intent_name: intent_str,
            ttl_secs,
            nonce: hex::encode(nonce),
            timestamp_unix,
            signature: hex::encode(sig.to_bytes()),
        }
    }

    /// Verify the requester's signature and return the verifying key.
    pub fn verify_signature(&self) -> Result<VerifyingKey, A1Error> {
        let vk = parse_pk_hex(&self.requester_pk_hex)?;

        let nonce = parse_nonce_hex(&self.nonce)?;
        let msg = request_signable_bytes(
            &self.requester_did,
            &nonce,
            self.timestamp_unix,
            self.ttl_secs,
            &self.intent_name,
            &self.requested_capabilities,
        );
        let sig = parse_sig_hex(&self.signature)?;

        use ed25519_dalek::Verifier;
        vk.verify(&msg, &sig)
            .map_err(|_| A1Error::InvalidSignature(0))?;

        Ok(vk)
    }

    /// Verify and enforce that the request is not stale.
    ///
    /// A request is stale if `|now - timestamp| > max_age_secs`.
    pub fn verify_freshness(&self, now_unix: u64, max_age_secs: u64) -> Result<(), A1Error> {
        let age = now_unix.saturating_sub(self.timestamp_unix);
        if age > max_age_secs {
            return Err(A1Error::Expired(
                0,
                self.timestamp_unix + max_age_secs,
                now_unix,
            ));
        }
        Ok(())
    }
}

/// A delegation offer from the responding agent (or gateway).
///
/// Contains a signed `DelegationCert` scoped to the requested capabilities
/// and a new nonce that the requester must echo in the `DelegationAcceptance`.
///
/// # Signature
///
/// `signature` is an Ed25519 signature over
/// `Blake3(DOMAIN || offerer_did || request_nonce || offer_nonce || timestamp || cert_fingerprint)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationOffer {
    /// `did:a1:` identifier of the offering party.
    pub offerer_did: String,
    /// Echoed nonce from the `CapabilityRequest` (prevents substitution).
    pub request_nonce: String,
    /// The issued delegation certificate.
    pub cert: DelegationCert,
    /// New nonce for the acceptance handshake.
    pub offer_nonce: String,
    /// Unix timestamp when this offer was created.
    pub timestamp_unix: u64,
    /// Offer expiry in seconds from `timestamp_unix`.
    pub offer_ttl_secs: u64,
    /// Ed25519 signature over the canonical offer bytes (hex).
    pub signature: String,
}

impl DelegationOffer {
    /// Build and sign a delegation offer in response to a verified request.
    ///
    /// `offerer` signs the cert and the offer metadata. The cert's
    /// `delegator_pk` will be `offerer.verifying_key()`.
    pub fn build(
        offerer: &dyn Signer,
        request: &CapabilityRequest,
        cert: DelegationCert,
        timestamp_unix: u64,
        offer_ttl_secs: u64,
    ) -> Result<Self, A1Error> {
        let vk = offerer.verifying_key();
        let offerer_did = format!("did:a1:{}", hex::encode(vk.as_bytes()));
        let offer_nonce = fresh_nonce();
        let cert_fp = cert.fingerprint();

        let request_nonce = parse_nonce_hex(&request.nonce)?;
        let msg = offer_signable_bytes(
            &offerer_did,
            &request_nonce,
            &offer_nonce,
            timestamp_unix,
            &cert_fp,
        );
        let sig = offerer.sign_message(&msg);

        Ok(Self {
            offerer_did,
            request_nonce: request.nonce.clone(),
            cert,
            offer_nonce: hex::encode(offer_nonce),
            timestamp_unix,
            offer_ttl_secs,
            signature: hex::encode(sig.to_bytes()),
        })
    }

    /// Verify the offerer's signature.
    pub fn verify_signature(&self) -> Result<VerifyingKey, A1Error> {
        let pk_hex = self
            .offerer_did
            .strip_prefix("did:a1:")
            .ok_or_else(|| A1Error::WireFormatError("invalid offerer DID".into()))?;
        let vk = parse_pk_hex(pk_hex)?;

        let request_nonce = parse_nonce_hex(&self.request_nonce)?;
        let offer_nonce = parse_nonce_hex(&self.offer_nonce)?;
        let cert_fp = self.cert.fingerprint();

        let msg = offer_signable_bytes(
            &self.offerer_did,
            &request_nonce,
            &offer_nonce,
            self.timestamp_unix,
            &cert_fp,
        );
        let sig = parse_sig_hex(&self.signature)?;

        use ed25519_dalek::Verifier;
        vk.verify(&msg, &sig)
            .map_err(|_| A1Error::InvalidSignature(0))?;

        Ok(vk)
    }
}

/// The requester's final acceptance of a delegation offer.
///
/// Echoes the `offer_nonce` to confirm receipt and proves the requester
/// controls their private key. After this message, the cert in the offer
/// is live and usable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationAcceptance {
    /// `did:a1:` identifier of the accepting agent.
    pub acceptor_did: String,
    /// Echoed nonce from the `DelegationOffer`.
    pub offer_nonce: String,
    /// Unix timestamp when this acceptance was created.
    pub timestamp_unix: u64,
    /// Ed25519 signature over the canonical acceptance bytes (hex).
    pub signature: String,
}

impl DelegationAcceptance {
    /// Build and sign an acceptance for a delegation offer.
    pub fn build(
        acceptor: &dyn Signer,
        offer: &DelegationOffer,
        timestamp_unix: u64,
    ) -> Result<Self, A1Error> {
        let vk = acceptor.verifying_key();
        let acceptor_did = format!("did:a1:{}", hex::encode(vk.as_bytes()));

        let offer_nonce = parse_nonce_hex(&offer.offer_nonce)?;
        let msg = accept_signable_bytes(&acceptor_did, &offer_nonce, timestamp_unix);
        let sig = acceptor.sign_message(&msg);

        Ok(Self {
            acceptor_did,
            offer_nonce: offer.offer_nonce.clone(),
            timestamp_unix,
            signature: hex::encode(sig.to_bytes()),
        })
    }

    /// Verify the acceptor's signature.
    pub fn verify_signature(&self) -> Result<VerifyingKey, A1Error> {
        let pk_hex = self
            .acceptor_did
            .strip_prefix("did:a1:")
            .ok_or_else(|| A1Error::WireFormatError("invalid acceptor DID".into()))?;
        let vk = parse_pk_hex(pk_hex)?;

        let offer_nonce = parse_nonce_hex(&self.offer_nonce)?;
        let msg = accept_signable_bytes(&self.acceptor_did, &offer_nonce, self.timestamp_unix);
        let sig = parse_sig_hex(&self.signature)?;

        use ed25519_dalek::Verifier;
        vk.verify(&msg, &sig)
            .map_err(|_| A1Error::InvalidSignature(0))?;

        Ok(vk)
    }
}

/// The result of a complete three-way negotiation handshake.
///
/// Returned by the gateway's `/v1/negotiate` endpoint after the requester
/// calls `A1Client.negotiateDelegation()`. The cert is ready to push onto
/// a `DyoloChain`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationResult {
    /// The delegation certificate, ready to use.
    pub cert: DelegationCert,
    /// The offer that produced this cert.
    pub offer: DelegationOffer,
    /// Certificate fingerprint (hex) for logging.
    pub fingerprint_hex: String,
    /// `did:a1:` of the delegating party.
    pub offerer_did: String,
    /// `did:a1:` of the receiving agent.
    pub requester_did: String,
}

// ── Canonical byte helpers ────────────────────────────────────────────────────

fn request_signable_bytes(
    requester_did: &str,
    nonce: &[u8; 16],
    timestamp: u64,
    ttl: u64,
    intent_name: &str,
    caps: &[String],
) -> Vec<u8> {
    let mut h = Hasher::new_derive_key(DOMAIN_NEG_REQUEST);
    h.update(&(requester_did.len() as u64).to_le_bytes());
    h.update(requester_did.as_bytes());
    h.update(nonce);
    h.update(&timestamp.to_le_bytes());
    h.update(&ttl.to_le_bytes());
    h.update(&(intent_name.len() as u64).to_le_bytes());
    h.update(intent_name.as_bytes());
    h.update(&(caps.len() as u64).to_le_bytes());
    for cap in caps {
        h.update(&(cap.len() as u64).to_le_bytes());
        h.update(cap.as_bytes());
    }
    h.finalize().as_bytes().to_vec()
}

fn offer_signable_bytes(
    offerer_did: &str,
    request_nonce: &[u8; 16],
    offer_nonce: &[u8; 16],
    timestamp: u64,
    cert_fp: &[u8; 32],
) -> Vec<u8> {
    let mut h = Hasher::new_derive_key(DOMAIN_NEG_OFFER);
    h.update(&(offerer_did.len() as u64).to_le_bytes());
    h.update(offerer_did.as_bytes());
    h.update(request_nonce);
    h.update(offer_nonce);
    h.update(&timestamp.to_le_bytes());
    h.update(cert_fp);
    h.finalize().as_bytes().to_vec()
}

fn accept_signable_bytes(acceptor_did: &str, offer_nonce: &[u8; 16], timestamp: u64) -> Vec<u8> {
    let mut h = Hasher::new_derive_key(DOMAIN_NEG_ACCEPT);
    h.update(&(acceptor_did.len() as u64).to_le_bytes());
    h.update(acceptor_did.as_bytes());
    h.update(offer_nonce);
    h.update(&timestamp.to_le_bytes());
    h.finalize().as_bytes().to_vec()
}

// ── Parse helpers ─────────────────────────────────────────────────────────────

fn parse_pk_hex(hex_str: &str) -> Result<VerifyingKey, A1Error> {
    let bytes = hex::decode(hex_str)
        .map_err(|_| A1Error::WireFormatError("invalid public key hex".into()))?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| A1Error::WireFormatError("public key must be 32 bytes".into()))?;
    VerifyingKey::from_bytes(&arr)
        .map_err(|e| A1Error::WireFormatError(format!("invalid Ed25519 key: {e}")))
}

fn parse_nonce_hex(hex_str: &str) -> Result<[u8; 16], A1Error> {
    let bytes =
        hex::decode(hex_str).map_err(|_| A1Error::WireFormatError("invalid nonce hex".into()))?;
    bytes
        .try_into()
        .map_err(|_| A1Error::WireFormatError("nonce must be 16 bytes".into()))
}

fn parse_sig_hex(hex_str: &str) -> Result<ed25519_dalek::Signature, A1Error> {
    let bytes = hex::decode(hex_str)
        .map_err(|_| A1Error::WireFormatError("invalid signature hex".into()))?;
    let arr: [u8; 64] = bytes
        .try_into()
        .map_err(|_| A1Error::WireFormatError("signature must be 64 bytes".into()))?;
    Ok(ed25519_dalek::Signature::from_bytes(&arr))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert::CertBuilder;
    use crate::identity::DyoloIdentity;
    use crate::intent::Intent;

    #[test]
    fn capability_request_sign_verify() {
        let requester = DyoloIdentity::generate();
        let now = 1_700_000_000u64;
        let req = CapabilityRequest::build(
            &requester,
            vec!["trade.equity".into(), "portfolio.read".into()],
            "trade.equity",
            3600,
            now,
        );
        let vk = req.verify_signature().unwrap();
        assert_eq!(vk.as_bytes(), requester.verifying_key().as_bytes());
    }

    #[test]
    fn capability_request_tampered_fails() {
        let requester = DyoloIdentity::generate();
        let now = 1_700_000_000u64;
        let mut req = CapabilityRequest::build(
            &requester,
            vec!["trade.equity".into()],
            "trade.equity",
            3600,
            now,
        );
        req.requested_capabilities.push("admin.everything".into());
        assert!(req.verify_signature().is_err());
    }

    #[test]
    fn capability_request_freshness() {
        let requester = DyoloIdentity::generate();
        let now = 1_700_000_000u64;
        let req = CapabilityRequest::build(&requester, vec!["read".into()], "read", 3600, now);
        assert!(req.verify_freshness(now + 60, 300).is_ok());
        assert!(req.verify_freshness(now + 400, 300).is_err());
    }

    #[test]
    fn full_negotiation_handshake() {
        let requester = DyoloIdentity::generate();
        let offerer = DyoloIdentity::generate();
        let now = 1_700_000_000u64;

        let intent = Intent::new("trade.equity").unwrap().hash();
        let cert =
            CertBuilder::new(requester.verifying_key(), intent, now, now + 3600).sign(&offerer);

        let req = CapabilityRequest::build(
            &requester,
            vec!["trade.equity".into()],
            "trade.equity",
            3600,
            now,
        );
        req.verify_signature().unwrap();

        let offer = DelegationOffer::build(&offerer, &req, cert, now, 120).unwrap();
        offer.verify_signature().unwrap();

        let acceptance = DelegationAcceptance::build(&requester, &offer, now + 1).unwrap();
        acceptance.verify_signature().unwrap();
    }

    #[test]
    fn offer_tampered_cert_fails_signature() {
        let requester = DyoloIdentity::generate();
        let offerer = DyoloIdentity::generate();
        let now = 1_700_000_000u64;
        let intent = Intent::new("read").unwrap().hash();

        let cert =
            CertBuilder::new(requester.verifying_key(), intent, now, now + 3600).sign(&offerer);
        let req = CapabilityRequest::build(&requester, vec!["read".into()], "read", 3600, now);
        let mut offer = DelegationOffer::build(&offerer, &req, cert, now, 120).unwrap();
        offer.offer_nonce = hex::encode([0u8; 16]);
        assert!(offer.verify_signature().is_err());
    }
}
