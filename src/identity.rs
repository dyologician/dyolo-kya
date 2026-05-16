pub mod narrowing;
pub mod receipt;

use ed25519_dalek::{Signature, Signer as DalekSigner, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use zeroize::ZeroizeOnDrop;

/// Abstraction over an Ed25519 signing backend.
///
/// Implement this trait to integrate hardware security modules (HSMs),
/// cloud key management services (AWS KMS, Azure Key Vault, HashiCorp Vault,
/// Google Cloud KMS), or any other backend that can produce Ed25519 signatures
/// without exposing raw private key bytes.
///
/// The in-process [`DyoloIdentity`] implements this trait and is the
/// recommended default for development and testing. Production deployments
/// should implement `Signer` over their KMS of choice.
///
/// # Security contract
///
/// Implementors MUST ensure that:
/// - `verifying_key()` always returns the public key matching the private key
///   used by `sign_message()`.
/// - `sign_message()` MUST NOT pre-hash `msg`; the caller already applies
///   domain separation before calling this function.
///
/// # Note on Async SDKs
///
/// If your KMS SDK is asynchronous (like `aws-sdk-kms` or Google Cloud KMS),
/// do **not** block the thread using `block_on` inside this trait. Doing so
/// can cause deadlocks on single-threaded runtimes and thread-pool exhaustion
/// on multi-threaded ones.
///
/// Instead, implement the [`AsyncSigner`] trait and use `CertBuilder::sign_async`.
///
/// # Example — HashiCorp Vault Transit skeleton
///
/// ```rust,ignore
/// use a1::Signer;
/// use base64::{engine::general_purpose, Engine as _};
///
/// struct VaultSigner { vault_addr: String, key_name: String, public_key: VerifyingKey }
///
/// impl Signer for VaultSigner {
///     fn verifying_key(&self) -> VerifyingKey { self.public_key }
///
///     fn sign_message(&self, msg: &[u8]) -> ed25519_dalek::Signature {
///         let encoded = general_purpose::STANDARD.encode(msg);
///         // POST /v1/transit/sign/{key_name} with {"input": encoded}
///         // Parse "data.signature" from response, strip "vault:v1:" prefix,
///         // base64-decode, convert to [u8; 64], return Signature::from_bytes(...)
///         todo!()
///     }
/// }
/// ```
pub trait Signer: Send + Sync {
    /// Return the Ed25519 public key corresponding to this signing backend.
    fn verifying_key(&self) -> VerifyingKey;

    /// Produce an Ed25519 signature over `msg`.
    ///
    /// `msg` is the raw domain-separated bytes produced internally by
    /// [`DelegationCert::signable_bytes`] — never a pre-hashed digest.
    ///
    /// [`DelegationCert::signable_bytes`]: crate::cert::DelegationCert::signable_bytes
    fn sign_message(&self, msg: &[u8]) -> Signature;
}

/// Asynchronous abstraction over an Ed25519 signing backend.
///
/// Use this trait to integrate cloud key management services (AWS KMS, Azure
/// Key Vault, HashiCorp Vault, Google Cloud KMS) whose SDKs are strictly async.
#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
#[async_trait::async_trait]
pub trait AsyncSigner: Send + Sync {
    /// Return the Ed25519 public key corresponding to this signing backend.
    fn verifying_key(&self) -> VerifyingKey;

    /// Produce an Ed25519 signature over `msg` asynchronously.
    async fn sign_message(&self, msg: &[u8]) -> Signature;
}

/// In-process Ed25519 identity for development, testing, and single-process
/// deployments.
///
/// The signing key is held in process memory and zeroized on drop.
/// For production deployments at scale, implement [`Signer`] over your
/// HSM or KMS so private key material never touches application memory.
///
/// # Cloning
///
/// This struct intentionally does **not** implement `Clone`. Copying private
/// key material across memory locations defeats `ZeroizeOnDrop` protections.
/// If you need to share an identity across multiple threads or async tasks,
/// use the provided [`SharedIdentity`] convenience wrapper instead.
#[derive(ZeroizeOnDrop)]
pub struct DyoloIdentity {
    signing_key: SigningKey,
}

impl DyoloIdentity {
    /// Generate a new random identity using the OS entropy source.
    pub fn generate() -> Self {
        Self {
            signing_key: SigningKey::generate(&mut OsRng),
        }
    }

    /// Restore an identity from a 32-byte signing key seed.
    ///
    /// # Security
    ///
    /// Source these bytes from secure storage (e.g. an HSM export, an
    /// encrypted secrets manager, or a PKCS#8 file). Zeroize the input
    /// buffer after calling this function.
    ///
    /// ```rust,ignore
    /// use zeroize::Zeroize;
    /// let mut seed = fetch_seed_from_vault();
    /// let identity = DyoloIdentity::from_signing_bytes(&seed);
    /// seed.zeroize();
    /// ```
    pub fn from_signing_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(bytes),
        }
    }

    /// Export the raw 32-byte signing key seed.
    ///
    /// # Security
    ///
    /// The returned value is a secret. Zeroize it after use.
    /// Do not log, store unencrypted, or transmit across a network.
    pub fn to_signing_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// The Ed25519 public key for this identity.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    #[allow(dead_code)]
    pub(crate) fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }
}

impl Signer for DyoloIdentity {
    fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    fn sign_message(&self, msg: &[u8]) -> Signature {
        self.signing_key.sign(msg)
    }
}

impl std::fmt::Debug for DyoloIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let vk = self.verifying_key();
        let b = vk.as_bytes();
        write!(
            f,
            "DyoloIdentity(vk:{:02x}{:02x}{:02x}{:02x}…)",
            b[0], b[1], b[2], b[3]
        )
    }
}

/// A thread-safe, clonable reference to a [`DyoloIdentity`].
///
/// Since [`DyoloIdentity`] cannot be cloned (to preserve zeroization guarantees),
/// `SharedIdentity` provides an `Arc` wrapper that implements `Clone` and `Signer`.
/// Useful for passing a single identity into multiple async workers.
#[derive(Clone, Debug)]
pub struct SharedIdentity(pub std::sync::Arc<DyoloIdentity>);

impl Signer for SharedIdentity {
    fn verifying_key(&self) -> VerifyingKey {
        self.0.verifying_key()
    }

    fn sign_message(&self, msg: &[u8]) -> Signature {
        Signer::sign_message(&*self.0, msg)
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncSigner for SharedIdentity {
    fn verifying_key(&self) -> VerifyingKey {
        self.0.verifying_key()
    }

    async fn sign_message(&self, msg: &[u8]) -> Signature {
        Signer::sign_message(&*self.0, msg)
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncSigner for DyoloIdentity {
    fn verifying_key(&self) -> VerifyingKey {
        self.verifying_key()
    }

    async fn sign_message(&self, msg: &[u8]) -> Signature {
        Signer::sign_message(self, msg)
    }
}

/// A trait for HTTP clients to transmit signing requests to a KMS.
#[cfg(feature = "async")]
#[async_trait::async_trait]
pub trait KmsHttpClient: Send + Sync {
    /// Post a payload to the KMS endpoint and return the raw response bytes.
    async fn post(&self, url: &str, headers: &[(&str, &str)], body: &[u8]) -> Result<Vec<u8>, crate::error::A1Error>;
}

/// Production-ready HashiCorp Vault Transit backend for A1 Passports.
///
/// Ensures private keys never touch application memory. Signatures are strictly
/// bound to the `v2.8.0` cryptographic domain natively via the Vault `context`.
#[cfg(feature = "async")]
pub struct VaultSigner {
    vault_addr: String,
    token: String,
    key_name: String,
    public_key: VerifyingKey,
    http_client: Box<dyn KmsHttpClient>,
}

#[cfg(feature = "async")]
impl std::fmt::Debug for VaultSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultSigner")
            .field("vault_addr", &self.vault_addr)
            .field("key_name", &self.key_name)
            .field("public_key", &hex::encode(self.public_key.as_bytes()))
            .finish()
    }
}

#[cfg(feature = "async")]
impl VaultSigner {
    pub fn new(
        vault_addr: String,
        token: String,
        key_name: String,
        public_key_hex: &str,
        http_client: Box<dyn KmsHttpClient>,
    ) -> Result<Self, crate::error::A1Error> {
        let pk_bytes = hex::decode(public_key_hex)
            .map_err(|_| crate::error::A1Error::WireFormatError("invalid hex".into()))?;
        let public_key = VerifyingKey::from_bytes(
            &pk_bytes.try_into().map_err(|_| crate::error::A1Error::WireFormatError("must be 32 bytes".into()))?
        ).map_err(|_| crate::error::A1Error::WireFormatError("invalid ed25519 key".into()))?;

        Ok(Self {
            vault_addr,
            token,
            key_name,
            public_key,
            http_client,
        })
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncSigner for VaultSigner {
    fn verifying_key(&self) -> VerifyingKey {
        self.public_key
    }

    async fn sign_message(&self, msg: &[u8]) -> Signature {
        // Vault expects base64 encoded input for the transit engine
        use base64::{engine::general_purpose, Engine as _};
        let encoded_input = general_purpose::STANDARD.encode(msg);
        
        // Structure the Vault Transit API request. 
        // The context parameter cryptographically binds the operation to the enforcer domain
        // and persistently records the invocation marker in enterprise Vault audit logs.
        let payload = format!("{{\"input\": \"{}\", \"context\": \"ZHlvbG9fdjIuOC4w\"}}", encoded_input);
        let url = format!("{}/v1/transit/sign/{}", self.vault_addr, self.key_name);
        let headers = [
            ("X-Vault-Token", self.token.as_str()),
            ("Content-Type", "application/json"),
        ];

        let resp_bytes = self.http_client.post(&url, &headers, payload.as_bytes())
            .await
            .expect("vault KMS post failed");

        let resp_json: serde_json::Value = serde_json::from_slice(&resp_bytes)
            .expect("vault returned invalid JSON");

        let sig_b64 = resp_json["data"]["signature"]
            .as_str()
            .expect("vault response missing data.signature")
            .trim_start_matches("vault:v1:");

        let sig_bytes = general_purpose::STANDARD.decode(sig_b64)
            .expect("vault signature base64 decode failed");

        Signature::from_bytes(
            &sig_bytes.try_into().expect("vault signature must be 64 bytes")
        )
    }
}
