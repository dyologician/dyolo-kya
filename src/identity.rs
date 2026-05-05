use ed25519_dalek::{Signature, Signer as DalekSigner, SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use zeroize::ZeroizeOnDrop;

// ── Signer trait ──────────────────────────────────────────────────────────────

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
/// use dyolo_kya::Signer;
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

// ── AsyncSigner trait ─────────────────────────────────────────────────────────

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

// ── DyoloIdentity ─────────────────────────────────────────────────────────────

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
        Self { signing_key: SigningKey::generate(&mut OsRng) }
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
        Self { signing_key: SigningKey::from_bytes(bytes) }
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

    /// Sign `message` using this identity's private key.
    ///
    /// Prefer calling [`CertBuilder::sign`] over this method directly.
    ///
    /// [`CertBuilder::sign`]: crate::cert::CertBuilder::sign
    pub(crate) fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }
}

/// [`DyoloIdentity`] implements [`Signer`] so it can be used wherever a
/// `&dyn Signer` is required without any wrapping.
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
        write!(f, "DyoloIdentity(vk:{:02x}{:02x}{:02x}{:02x}…)", b[0], b[1], b[2], b[3])
    }
}

// ── SharedIdentity ────────────────────────────────────────────────────────────

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
        self.0.sign_message(msg)
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncSigner for SharedIdentity {
    fn verifying_key(&self) -> VerifyingKey {
        self.0.verifying_key()
    }

    async fn sign_message(&self, msg: &[u8]) -> Signature {
        // Under the hood, DyoloIdentity's sign_message is CPU-bound but extremely fast,
        // so it does not block the async executor in a problematic way.
        self.0.sign_message(msg)
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
impl AsyncSigner for DyoloIdentity {
    fn verifying_key(&self) -> VerifyingKey {
        self.verifying_key()
    }

    async fn sign_message(&self, msg: &[u8]) -> Signature {
        self.sign_message(msg)
    }
}