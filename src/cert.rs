use ed25519_dalek::{Signature, Verifier, VerifyingKey};

use crate::chain::Clock;
use crate::crypto::{hasher_cert_fp, hasher_cert_sig};
use crate::error::A1Error;
use crate::identity::Signer;
use crate::intent::IntentHash;
use crate::registry::fresh_nonce;
use crate::SubScopeProof;

#[cfg(feature = "wire")]
use crate::cert_extensions::CertExtensions;

/// Wire format version for `DelegationCert`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CertVersion {
    V1 = 1,
}

impl CertVersion {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

pub const CERT_VERSION: u8 = 1;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DelegationCert {
    pub version: u8,
    pub delegator_pk: VerifyingKey,
    pub delegate_pk: VerifyingKey,
    pub scope_root: IntentHash,
    pub scope_proof: SubScopeProof,
    pub nonce: [u8; 16],
    pub issued_at: u64,
    pub expiration_unix: u64,
    pub max_depth: u8,
    #[cfg(not(feature = "wire"))]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub extensions_hash: Option<[u8; 32]>,
    #[cfg(feature = "wire")]
    #[serde(default)]
    pub extensions: CertExtensions,
    pub signature: Signature,
}

impl DelegationCert {
    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    pub fn signable_bytes(
        version: u8,
        delegator_pk: &VerifyingKey,
        delegate_pk: &VerifyingKey,
        scope_root: &IntentHash,
        scope_proof: &SubScopeProof,
        nonce: &[u8; 16],
        issued_at: u64,
        expiration_unix: u64,
        max_depth: u8,
        ext_commitment: &[u8; 32],
    ) -> Vec<u8> {
        // We use hasher_cert_sig to get the explicit version byte in the domain derivation.
        let mut h = hasher_cert_sig(version);
        h.update(b"a1::dyolo::cert::sig::v2.8.0");
        h.update(delegator_pk.as_bytes());
        h.update(delegate_pk.as_bytes());
        h.update(scope_root);
        h.update(&scope_proof.commitment());
        h.update(nonce);
        h.update(&issued_at.to_be_bytes());
        h.update(&expiration_unix.to_be_bytes());
        h.update(&[max_depth]);
        h.update(ext_commitment);
        // We extract the finalized bytes directly as the signable payload to keep the Ed25519
        // signature robust against long message attacks while providing a fixed size.
        h.finalize().as_bytes().to_vec()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn issue(
        delegator: &dyn Signer,
        delegate_pk: VerifyingKey,
        scope_root: IntentHash,
        scope_proof: SubScopeProof,
        nonce: [u8; 16],
        issued_at: u64,
        expiration_unix: u64,
        max_depth: u8,
        #[cfg(feature = "wire")] extensions: CertExtensions,
        #[cfg(not(feature = "wire"))] extensions_hash: Option<[u8; 32]>,
    ) -> Self {
        let delegator_pk = delegator.verifying_key();

        #[cfg(feature = "wire")]
        let ext_commit = extensions.commitment();
        #[cfg(not(feature = "wire"))]
        let ext_commit = extensions_hash.unwrap_or_else(|| {
            let mut h = crate::crypto::derive_key("a1::dyolo::cert::ext::v2.8.0", CERT_VERSION);
            h.update(&0u64.to_le_bytes());
            h.finalize().into()
        });

        let msg = Self::signable_bytes(
            CERT_VERSION,
            &delegator_pk,
            &delegate_pk,
            &scope_root,
            &scope_proof,
            &nonce,
            issued_at,
            expiration_unix,
            max_depth,
            &ext_commit,
        );
        Self {
            version: CERT_VERSION,
            delegator_pk,
            delegate_pk,
            scope_root,
            scope_proof,
            nonce,
            issued_at,
            expiration_unix,
            max_depth,
            #[cfg(not(feature = "wire"))]
            extensions_hash,
            #[cfg(feature = "wire")]
            extensions,
            signature: delegator.sign_message(&msg),
        }
    }

    pub fn verify_signature(&self) -> bool {
        #[cfg(feature = "wire")]
        let ext_commit = self.extensions.commitment();
        #[cfg(not(feature = "wire"))]
        let ext_commit = self.extensions_hash.unwrap_or_else(|| {
            let mut h = crate::crypto::derive_key("a1::dyolo::cert::ext::v2.8.0", self.version);
            h.update(&0u64.to_le_bytes());
            h.finalize().into()
        });

        let msg = Self::signable_bytes(
            self.version,
            &self.delegator_pk,
            &self.delegate_pk,
            &self.scope_root,
            &self.scope_proof,
            &self.nonce,
            self.issued_at,
            self.expiration_unix,
            self.max_depth,
            &ext_commit,
        );
        self.delegator_pk.verify(&msg, &self.signature).is_ok()
    }

    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        let mut h = hasher_cert_fp(self.version);
        h.update(b"a1::dyolo::cert::fp::v2.8.0");
        h.update(&self.signature.to_bytes());
        h.finalize().into()
    }

    pub fn fingerprint_hex(&self) -> String {
        hex::encode(self.fingerprint())
    }

    pub fn ttl_secs(&self) -> u64 {
        self.expiration_unix.saturating_sub(self.issued_at)
    }
}

// ── CertBuilder ───────────────────────────────────────────────────────────────

pub struct CertBuilder {
    delegate_pk: VerifyingKey,
    scope_root: IntentHash,
    scope_proof: SubScopeProof,
    nonce: [u8; 16],
    issued_at: u64,
    expiration_unix: u64,
    max_depth: u8,
    #[cfg(feature = "wire")]
    extensions: CertExtensions,
    #[cfg(not(feature = "wire"))]
    extensions_hash: Option<[u8; 32]>,
}

impl CertBuilder {
    pub fn new(
        delegate_pk: VerifyingKey,
        scope_root: IntentHash,
        issued_at: u64,
        expiration_unix: u64,
    ) -> Self {
        Self {
            delegate_pk,
            scope_root,
            scope_proof: SubScopeProof::full_passthrough(),
            nonce: fresh_nonce(),
            issued_at,
            expiration_unix,
            max_depth: 16,
            #[cfg(feature = "wire")]
            extensions: CertExtensions::new(),
            #[cfg(not(feature = "wire"))]
            extensions_hash: None,
        }
    }

    pub fn scope_proof(mut self, proof: SubScopeProof) -> Self {
        self.scope_proof = proof;
        self
    }

    pub fn nonce(mut self, nonce: [u8; 16]) -> Self {
        self.nonce = nonce;
        self
    }

    pub fn max_depth(mut self, depth: u8) -> Self {
        self.max_depth = depth;
        self
    }

    #[cfg(feature = "wire")]
    pub fn extensions(mut self, ext: CertExtensions) -> Self {
        self.extensions = ext;
        self
    }

    #[cfg(not(feature = "wire"))]
    pub fn extensions_hash(mut self, hash: [u8; 32]) -> Self {
        self.extensions_hash = Some(hash);
        self
    }

    pub fn build(self, delegator: &dyn Signer) -> Result<DelegationCert, A1Error> {
        if self.issued_at >= self.expiration_unix {
            return Err(A1Error::WireFormatError(format!(
                "issued_at ({}) must be strictly less than expiration_unix ({})",
                self.issued_at, self.expiration_unix
            )));
        }
        Ok(DelegationCert::issue(
            delegator,
            self.delegate_pk,
            self.scope_root,
            self.scope_proof,
            self.nonce,
            self.issued_at,
            self.expiration_unix,
            self.max_depth,
            #[cfg(feature = "wire")]
            self.extensions,
            #[cfg(not(feature = "wire"))]
            self.extensions_hash,
        ))
    }

    pub fn sign(self, delegator: &dyn Signer) -> DelegationCert {
        self.build(delegator)
            .expect("invalid certificate configuration: issued_at must be before expiration_unix")
    }

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    pub async fn build_async(
        self,
        delegator: &dyn crate::identity::AsyncSigner,
    ) -> Result<DelegationCert, A1Error> {
        if self.issued_at >= self.expiration_unix {
            return Err(A1Error::WireFormatError(format!(
                "issued_at ({}) must be strictly less than expiration_unix ({})",
                self.issued_at, self.expiration_unix
            )));
        }
        let delegator_pk = delegator.verifying_key();

        #[cfg(feature = "wire")]
        let ext_commit = self.extensions.commitment();
        #[cfg(not(feature = "wire"))]
        let ext_commit = self.extensions_hash.unwrap_or_else(|| {
            let mut h = crate::crypto::derive_key("a1::dyolo::cert::ext::v2.8.0", CERT_VERSION);
            h.update(&0u64.to_le_bytes());
            h.finalize().into()
        });

        let msg = DelegationCert::signable_bytes(
            CERT_VERSION,
            &delegator_pk,
            &self.delegate_pk,
            &self.scope_root,
            &self.scope_proof,
            &self.nonce,
            self.issued_at,
            self.expiration_unix,
            self.max_depth,
            &ext_commit,
        );

        Ok(DelegationCert {
            version: CERT_VERSION,
            delegator_pk,
            delegate_pk: self.delegate_pk,
            scope_root: self.scope_root,
            scope_proof: self.scope_proof,
            nonce: self.nonce,
            issued_at: self.issued_at,
            expiration_unix: self.expiration_unix,
            max_depth: self.max_depth,
            #[cfg(not(feature = "wire"))]
            extensions_hash: self.extensions_hash,
            #[cfg(feature = "wire")]
            extensions: self.extensions,
            signature: delegator.sign_message(&msg).await,
        })
    }

    #[cfg(feature = "async")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
    pub async fn sign_async(self, delegator: &dyn crate::identity::AsyncSigner) -> DelegationCert {
        self.build_async(delegator)
            .await
            .expect("invalid certificate configuration: issued_at must be before expiration_unix")
    }
}

// ── CertBundle ────────────────────────────────────────────────────────────────

/// A batch of delegation certs issued in a single atomic call.
///
/// All certs in a bundle share the same delegator and timestamp but may have
/// different delegates, scopes, and TTLs. Issuing in a bundle is semantically
/// equivalent to issuing each cert individually; the bundle is purely a
/// transport convenience that lets callers issue a full sub-tree of delegations
/// in one round-trip to the gateway.
///
/// # Example
///
/// ```rust,ignore
/// use a1::cert::CertBundle;
///
/// let bundle = CertBundle::issue(&human, now, vec![
///     CertBuilder::new(agent_a.verifying_key(), scope_a, now, now + 3600),
///     CertBuilder::new(agent_b.verifying_key(), scope_b, now, now + 1800),
/// ]);
/// assert_eq!(bundle.len(), 2);
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CertBundle {
    pub certs: Vec<DelegationCert>,
    pub issued_at: u64,
}

impl CertBundle {
    pub fn issue(delegator: &dyn Signer, issued_at: u64, builders: Vec<CertBuilder>) -> Self {
        let certs = builders.into_iter().map(|b| b.sign(delegator)).collect();
        Self { certs, issued_at }
    }

    pub fn from_certs(certs: Vec<DelegationCert>, clock: &dyn Clock) -> Self {
        let issued_at = clock.unix_now();
        Self { certs, issued_at }
    }

    pub fn len(&self) -> usize {
        self.certs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.certs.is_empty()
    }

    pub fn fingerprints(&self) -> Vec<[u8; 32]> {
        self.certs.iter().map(|c| c.fingerprint()).collect()
    }
}
