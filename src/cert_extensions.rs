use std::collections::BTreeMap;

use blake3::Hasher;
use crate::crypto::hasher_cert_ext;
use crate::error::KyaError;

/// Maximum allowed length in bytes for all encoded extension keys and values combined.
pub const MAX_EXTENSION_BYTES: usize = 16384;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum ExtValue {
    Str(String),
    U64(u64),
    Strings(Vec<String>),
}

/// Typed extension fields committed into a [`DelegationCert`] signature.
///
/// Extensions augment the minimal cert payload with business-level metadata
/// without breaking the cryptographic invariants of the core fields.
/// The extension map is canonically serialized and hashed before inclusion in
/// `signable_bytes`, so a tampered extension causes signature verification to fail.
///
/// # Reserved namespaces
///
/// `dyolo.*` is reserved for protocol extensions. Applications should use
/// their own reverse-DNS prefix, e.g. `acme.cost_center`.
///
/// # Well-known keys
///
/// | Key                     | Type       | Meaning                                      |
/// |-------------------------|------------|----------------------------------------------|
/// | `dyolo.rate_limit_rpm`  | `u64`      | Max requests per minute for the delegate     |
/// | `dyolo.quota_tokens`    | `u64`      | Max LLM tokens the delegate may consume      |
/// | `dyolo.geo_allow`       | `[String]` | ISO-3166-1 alpha-2 country codes allowed     |
/// | `dyolo.cost_center`     | `String`   | Billing cost-center tag for audit            |
/// | `dyolo.ttl_warn_sec`    | `u64`      | Alert threshold before expiry                |
///
/// [`DelegationCert`]: crate::DelegationCert
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CertExtensions {
    fields: BTreeMap<String, ExtValue>,
    #[cfg_attr(feature = "serde", serde(skip))]
    byte_count: usize,
}

impl CertExtensions {
    /// Create an empty extension map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an extension field. Returns `self` for chaining.
    /// Panics if the size limit is exceeded. Prefer `set_checked` for safety.
    pub fn set(mut self, key: impl Into<String>, value: impl Into<ExtValue>) -> Self {
        self.set_checked(key, value).expect("extension limit exceeded")
    }

    /// Set an extension field with size validation.
    ///
    /// Keys must be non-empty. Use a reverse-DNS prefix for application-specific
    /// keys (e.g. `acme.cost_center`). The `dyolo.*` namespace is reserved for
    /// well-known protocol keys (`dyolo.rate_limit_rpm`, `dyolo.quota_tokens`,
    /// `dyolo.geo_allow`, `dyolo.cost_center`, `dyolo.ttl_warn_sec`).
    pub fn set_checked(mut self, key: impl Into<String>, value: impl Into<ExtValue>) -> Result<Self, KyaError> {
        let key_str = key.into();

        if key_str.is_empty() {
            return Err(KyaError::WireFormatError("extension key must not be empty".into()));
        }

        let val = value.into();

        let mut add_bytes = key_str.len();
        match &val {
            ExtValue::Str(s)     => add_bytes += s.len(),
            ExtValue::U64(_)     => add_bytes += 8,
            ExtValue::Strings(v) => { for s in v { add_bytes += s.len(); } }
        }

        if self.byte_count + add_bytes > MAX_EXTENSION_BYTES {
            return Err(KyaError::WireFormatError("maximum extension byte limit exceeded".into()));
        }

        self.byte_count += add_bytes;
        self.fields.insert(key_str, val);
        Ok(self)
    }

    /// Get an extension field by key.
    pub fn get(&self, key: &str) -> Option<&ExtValue> {
        self.fields.get(key)
    }

    /// Returns `true` if no extension fields are set.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &ExtValue)> {
        self.fields.iter()
    }

    /// A deterministic 32-byte commitment over all extension fields.
    pub fn commitment(&self) -> [u8; 32] {
        let mut h = hasher_cert_ext(crate::cert::CERT_VERSION);
        if self.fields.is_empty() {
            // Explicit empty sentinel (hashes the domain + version + length 0)
            h.update(&0u64.to_le_bytes());
            return h.finalize().into();
        }
        
        h.update(&(self.fields.len() as u64).to_le_bytes());
        for (k, v) in &self.fields {
            let k_bytes = k.as_bytes();
            h.update(&(k_bytes.len() as u64).to_le_bytes());
            h.update(k_bytes);
            
            // Deterministic typed encoding
            match v {
                ExtValue::Str(s) => {
                    h.update(&[0u8]); // type tag
                    h.update(&(s.len() as u64).to_le_bytes());
                    h.update(s.as_bytes());
                }
                ExtValue::U64(n) => {
                    h.update(&[1u8]); // type tag
                    h.update(&n.to_le_bytes());
                }
                ExtValue::Strings(vec) => {
                    h.update(&[2u8]); // type tag
                    h.update(&(vec.len() as u64).to_le_bytes());
                    for s in vec {
                        h.update(&(s.len() as u64).to_le_bytes());
                        h.update(s.as_bytes());
                    }
                }
            }
        }
        h.finalize().into()
    }
}

impl std::fmt::Display for ExtValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtValue::Str(s)     => write!(f, "{s}"),
            ExtValue::U64(n)     => write!(f, "{n}"),
            ExtValue::Strings(v) => write!(f, "[{}]", v.join(", ")),
        }
    }
}

impl From<serde_json::Value> for ExtValue {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Number(n) if n.is_u64() => ExtValue::U64(n.as_u64().unwrap()),
            serde_json::Value::String(s) => ExtValue::Str(s),
            serde_json::Value::Array(arr) => ExtValue::Strings(
                arr.into_iter()
                    .map(|x| match x {
                        serde_json::Value::String(s) => s,
                        other => other.to_string(),
                    })
                    .collect(),
            ),
            other => ExtValue::Str(other.to_string()),
        }
    }
}