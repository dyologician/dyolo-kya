//! C ABI exports for a1.
//!
//! Enable with `features = ["ffi"]`.
//!
//! This module exposes a stable C ABI so that any language capable of calling
//! a shared library can integrate a1 without a Rust toolchain:
//!
//! - **Python** — `ctypes` or `cffi`
//! - **Go** — `cgo` (`#include "a1.h"`)
//! - **Java / JVM** — JNA or JNI
//! - **Node.js** — `node-ffi-napi` or a native addon
//! - **C / C++** — link against the `.so` / `.dll` / `.dylib` directly
//!
//! # Generated header
//!
//! Run `cbindgen --config cbindgen.toml --output a1.h` from the
//! workspace root to regenerate the C header from this source file.
//! The header is published to `include/a1.h` on every release.
//!
//! # Thread safety
//!
//! All exported functions are thread-safe. The chain, identity, and store
//! handles are heap-allocated Rust values wrapped in opaque pointers; the
//! caller must not alias or free them outside the provided `_free` functions.
//!
//! # Error handling
//!
//! Every function that can fail returns a `A1Status` integer:
//! - `0` (`A1_OK`) — success
//! - Any other value — a `A1Error` variant (see `A1Status` enum)
//!
//! On failure, `dyolo_last_error()` returns a nul-terminated UTF-8 string
//! describing the error; the string is valid until the next FFI call on
//! the same thread.
//!
//! # Memory model
//!
//! - Objects returned as `*mut OpaqueType` are heap-allocated by Rust.
//!   The caller MUST free them with the corresponding `_free` function.
//! - Byte buffers (`*mut u8`) written by Rust are caller-allocated; the
//!   length is always passed in and the function writes at most that many bytes.
//! - String pointers returned by `dyolo_last_error()` are thread-local and
//!   must NOT be freed by the caller.

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CStr, CString};
use std::panic;

use crate::chain::{DyoloChain, SystemClock};
use crate::error::A1Error;
use crate::identity::DyoloIdentity;
use crate::intent::{Intent, MerkleProof};
use crate::registry::{MemoryNonceStore, MemoryRevocationStore};

// Thread-local error string (avoids the need for a global mutex).
thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_last_error(msg: impl Into<Vec<u8>>) {
    LAST_ERROR.with(|e| {
        let s =
            CString::new(msg).unwrap_or_else(|_| CString::new("error contains nul byte").unwrap());
        *e.borrow_mut() = Some(s);
    });
}

fn a1_error_to_status(e: &A1Error) -> c_int {
    match e {
        A1Error::EmptyChain => A1Status::A1ErrEmptyChain as c_int,
        A1Error::StorageFailure(_) => A1Status::A1ErrStorageFailure as c_int,
        A1Error::RootMismatch => A1Status::A1ErrRootMismatch as c_int,
        A1Error::BrokenLinkage(_) => A1Status::A1ErrBrokenLinkage as c_int,
        A1Error::InvalidSignature(_) => A1Status::A1ErrInvalidSig as c_int,
        A1Error::NotYetValid(..) => A1Status::A1ErrNotYetValid as c_int,
        A1Error::Expired(..) => A1Status::A1ErrExpired as c_int,
        A1Error::TemporalViolation(..) => A1Status::A1ErrTemporalViol as c_int,
        A1Error::MaxDepthExceeded(..) => A1Status::A1ErrMaxDepth as c_int,
        A1Error::InvalidSubScopeProof => A1Status::A1ErrInvalidProof as c_int,
        A1Error::ScopeEscalation(_) => A1Status::A1ErrScopeEscal as c_int,
        A1Error::UnauthorizedLeaf => A1Status::A1ErrUnauthorized as c_int,
        A1Error::ScopeViolation => A1Status::A1ErrScopeViol as c_int,
        A1Error::NonceReplay => A1Status::A1ErrNonceReplay as c_int,
        A1Error::Revoked => A1Status::A1ErrRevoked as c_int,
        A1Error::IntentNotFound => A1Status::A1ErrIntentNotFound as c_int,
        A1Error::EmptyTree => A1Status::A1ErrEmptyTree as c_int,
        A1Error::WireFormatError(_) => A1Status::A1ErrWireFormat as c_int,
        A1Error::UnsupportedVersion { .. } => A1Status::A1ErrUnsupportedVer as c_int,
        A1Error::PolicyViolation(_) => A1Status::A1ErrPolicyViolation as c_int,
        A1Error::BatchItemFailed { .. } => A1Status::A1ErrBatchItemFailed as c_int,
        A1Error::MacVerificationFailed => A1Status::A1ErrMacFailed as c_int,
        A1Error::NamespaceMismatch { .. } => A1Status::A1ErrNamespaceMismatch as c_int,
        A1Error::RateLimitExceeded => A1Status::A1ErrRateLimit as c_int,
        A1Error::StorageUnhealthy(_) => A1Status::A1ErrStorageUnhealthy as c_int,
        A1Error::PassportNarrowingViolation => A1Status::A1ErrPassportNarrowing as c_int,
        _ => A1Status::A1ErrUnknown as c_int,
    }
}

/// Return value of every fallible FFI function.
///
/// `A1_OK = 0` is the only success value; all other values are errors.
/// Call `dyolo_last_error()` immediately after a non-zero return to read
/// a human-readable description of the failure.
/// Returns a `A1Status` code detailing the failure reason.
/// Documented for `cbindgen` export.
#[repr(C)]
pub enum A1Status {
    /// Operation succeeded.
    A1Ok = 0,
    /// The delegation chain is empty.
    A1ErrEmptyChain = 1,
    /// The storage backend failed.
    A1ErrStorageFailure = 2,
    /// Chain root does not match the expected principal.
    A1ErrRootMismatch = 3,
    /// Delegation link broken at hop N.
    A1ErrBrokenLinkage = 4,
    /// Invalid cryptographic signature at hop N.
    A1ErrInvalidSig = 5,
    /// Certificate is not yet valid (clock drift or future issuance).
    A1ErrNotYetValid = 6,
    /// Certificate has expired.
    A1ErrExpired = 7,
    /// Temporal violation: child outlives parent.
    A1ErrTemporalViol = 8,
    /// Delegation depth exceeds policy or cert maximum.
    A1ErrMaxDepth = 9,
    /// The sub-scope or merkle proof is invalid.
    A1ErrInvalidProof = 10,
    /// Scope escalation: child attempts to delegate scope it does not have.
    A1ErrScopeEscal = 11,
    /// The executing agent is not the terminal delegate.
    A1ErrUnauthorized = 12,
    /// The requested intent is not permitted by the terminal scope.
    A1ErrScopeViol = 13,
    /// Nonce replay detected.
    A1ErrNonceReplay = 14,
    /// A certificate in the chain has been revoked.
    A1ErrRevoked = 15,
    /// Intent not found in the scope tree.
    A1ErrIntentNotFound = 16,
    /// Attempted to build an empty scope tree.
    A1ErrEmptyTree = 17,
    /// Invalid wire format (JSON/CBOR parse error).
    A1ErrWireFormat = 18,
    /// Unsupported certificate version.
    A1ErrUnsupportedVer = 19,
    /// Delegation policy violation.
    A1ErrPolicyViolation = 20,
    /// Batch authorization failed at one or more indices.
    A1ErrBatchItemFailed = 21,
    /// MAC/HMAC verification failed.
    A1ErrMacFailed = 22,
    /// Chain namespace does not match the requested namespace.
    A1ErrNamespaceMismatch = 23,
    /// Rate limit exceeded.
    A1ErrRateLimit = 24,
    /// Storage health check failed.
    A1ErrStorageUnhealthy = 25,
    /// Passport capability narrowing violation.
    A1ErrPassportNarrowing = 26,
    /// A Rust panic occurred.
    A1ErrPanic = 98,
    /// Unknown or unmapped error.
    A1ErrUnknown = 99,
}

/// Opaque handle to a [`DyoloIdentity`].
pub struct OpaqueIdentity(DyoloIdentity);

/// Opaque handle to a persistent [`MemoryRevocationStore`].
pub struct OpaqueRevocationStore(MemoryRevocationStore);

/// Opaque handle to a persistent [`MemoryNonceStore`].
pub struct OpaqueNonceStore(MemoryNonceStore);

/// Opaque handle to a [`DyoloChain`] plus its in-process stores.
#[allow(dead_code)]
pub struct OpaqueChain {
    chain: DyoloChain,
    rev: MemoryRevocationStore,
    nonces: MemoryNonceStore,
}

// ── Error reporting ───────────────────────────────────────────────────────────

/// Returns a nul-terminated UTF-8 string describing the last error that
/// occurred on this thread, or a null pointer if no error has been set.
///
/// The returned pointer is valid until the next FFI call on this thread.
/// Do NOT free it.
///
/// # Safety
///
/// `LAST_ERROR` is thread-local; this function is safe to call from any thread
/// that previously called a `dyolo_*` function.
///
/// # Example (Python cffi)
/// ```python
/// err = lib.dyolo_last_error()
/// if err:
///     print(ffi.string(err).decode())
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dyolo_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ref().map_or(std::ptr::null(), |s| s.as_ptr()))
}

// ── Identity ──────────────────────────────────────────────────────────────────

/// Generate a new random `DyoloIdentity` and return an opaque handle.
///
/// The caller MUST free the returned handle with `dyolo_identity_free()`.
///
/// Returns `NULL` on allocation failure (extremely unlikely).
#[unsafe(no_mangle)]
pub extern "C" fn dyolo_identity_generate() -> *mut OpaqueIdentity {
    Box::into_raw(Box::new(OpaqueIdentity(DyoloIdentity::generate())))
}

/// Restore a `DyoloIdentity` from a 32-byte signing key seed.
///
/// `seed` must point to exactly 32 bytes of key material. The caller retains
/// ownership of `seed` and must zeroize it after calling this function.
///
/// Returns `NULL` if `seed` is null.
///
/// # Safety
/// `seed` must be a valid pointer to at least 32 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dyolo_identity_from_seed(seed: *const u8) -> *mut OpaqueIdentity {
    if seed.is_null() {
        set_last_error("dyolo_identity_from_seed: seed pointer is null");
        return std::ptr::null_mut();
    }
    let bytes: [u8; 32] = unsafe { std::slice::from_raw_parts(seed, 32) }
        .try_into()
        .expect("seed is always 32 bytes");
    Box::into_raw(Box::new(OpaqueIdentity(DyoloIdentity::from_signing_bytes(
        &bytes,
    ))))
}

/// Write the 32-byte Ed25519 verifying key of `identity` into `out`.
///
/// `out` must point to a caller-allocated buffer of at least 32 bytes.
///
/// Returns `A1_OK` (0) on success.
///
/// # Safety
/// `identity` and `out` must be valid, non-null pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dyolo_identity_verifying_key(
    identity: *const OpaqueIdentity,
    out: *mut u8,
) -> c_int {
    if identity.is_null() || out.is_null() {
        set_last_error("null pointer argument");
        return A1Status::A1ErrUnknown as c_int;
    }
    let vk = unsafe { (*identity).0.verifying_key() };
    unsafe { std::ptr::copy_nonoverlapping(vk.as_bytes().as_ptr(), out, 32) };
    A1Status::A1Ok as c_int
}

/// Free a `DyoloIdentity` handle previously returned by `dyolo_identity_generate`
/// or `dyolo_identity_from_seed`.
///
/// # Safety
/// `identity` must be a valid, non-null pointer returned by this library.
/// Calling this twice on the same pointer is undefined behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dyolo_identity_free(identity: *mut OpaqueIdentity) {
    if !identity.is_null() {
        let _ = unsafe { Box::from_raw(identity) };
    }
}

// ── Stores ────────────────────────────────────────────────────────────────────

/// Allocate a new persistent in-memory revocation store.
#[unsafe(no_mangle)]
pub extern "C" fn dyolo_revocation_store_new() -> *mut OpaqueRevocationStore {
    Box::into_raw(Box::new(
        OpaqueRevocationStore(MemoryRevocationStore::new()),
    ))
}

/// Free a revocation store handle.
///
/// # Safety
///
/// `store` must be a valid pointer obtained from `dyolo_revocation_store_new()`,
/// or `NULL`. Must not be freed more than once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dyolo_revocation_store_free(store: *mut OpaqueRevocationStore) {
    if !store.is_null() {
        let _ = unsafe { Box::from_raw(store) };
    }
}

/// Allocate a new persistent in-memory nonce store.
#[unsafe(no_mangle)]
pub extern "C" fn dyolo_nonce_store_new() -> *mut OpaqueNonceStore {
    Box::into_raw(Box::new(OpaqueNonceStore(MemoryNonceStore::new())))
}

/// Free a nonce store handle.
///
/// # Safety
///
/// `store` must be a valid pointer obtained from `dyolo_nonce_store_new()`,
/// or `NULL`. Must not be freed more than once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dyolo_nonce_store_free(store: *mut OpaqueNonceStore) {
    if !store.is_null() {
        let _ = unsafe { Box::from_raw(store) };
    }
}

// ── Cert Operations ───────────────────────────────────────────────────────────

/// Revoke a certificate by fingerprint in the provided revocation store.
/// `fingerprint_hex` must be a 64-character null-terminated hex string.
///
/// # Safety
///
/// `store` must be a valid non-null pointer from `dyolo_revocation_store_new()`.
/// `fingerprint_hex` must be a valid null-terminated C string of exactly 64 hex characters.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dyolo_cert_revoke(
    store: *mut OpaqueRevocationStore,
    fingerprint_hex: *const c_char,
) -> c_int {
    let result = panic::catch_unwind(|| {
        if store.is_null() || fingerprint_hex.is_null() {
            return Err("null pointer argument".to_string());
        }
        let fp_str = unsafe { CStr::from_ptr(fingerprint_hex) }
            .to_str()
            .map_err(|e| format!("invalid utf-8: {e}"))?;
        let fp_bytes: [u8; 32] = hex::decode(fp_str)
            .map_err(|e| format!("invalid hex: {e}"))?
            .try_into()
            .map_err(|_| "fingerprint must be 32 bytes".to_string())?;

        use crate::registry::RevocationStore;
        unsafe { &(*store).0 }
            .revoke(&fp_bytes)
            .map_err(|e| e.to_string())
    });

    match result {
        Ok(Ok(())) => A1Status::A1Ok as c_int,
        Ok(Err(msg)) => {
            set_last_error(msg);
            A1Status::A1ErrUnknown as c_int
        }
        Err(_) => {
            set_last_error("internal panic");
            A1Status::A1ErrPanic as c_int
        }
    }
}

// ── JSON authorize ────────────────────────────────────────────────────────────

/// Verify a JSON-encoded `SignedChain` for a given intent and write the
/// JSON-encoded `VerifiedToken` (HMAC-authenticated receipt) into `out_buf`.
///
/// Parameters:
/// - `rev_store`       — pointer to a persistent `OpaqueRevocationStore`
/// - `nonce_store`     — pointer to a persistent `OpaqueNonceStore`
/// - `chain_json`      — nul-terminated UTF-8 JSON from `SignedChain::to_json()`
/// - `agent_pk_hex`    — nul-terminated hex-encoded 32-byte Ed25519 verifying key
/// - `intent_action`   — nul-terminated UTF-8 action name (e.g. `"trade.equity"`)
/// - `mac_key`         — pointer to 32 bytes of MAC key shared with the executor
/// - `out_buf`         — caller-allocated buffer for the JSON output
/// - `out_buf_len`     — length of `out_buf` in bytes
///
/// Returns `A1_OK` (0) on success, a non-zero `A1Status` on failure.
/// On failure, `dyolo_last_error()` contains the error description.
/// On success, `out_buf` contains a nul-terminated JSON `VerifiedToken`.
///
/// # Safety
/// All pointer arguments must be valid and non-null.
#[cfg(feature = "wire")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dyolo_authorize_json(
    rev_store: *mut OpaqueRevocationStore,
    nonce_store: *mut OpaqueNonceStore,
    chain_json: *const c_char,
    agent_pk_hex: *const c_char,
    intent_action: *const c_char,
    mac_key: *const u8,
    out_buf: *mut c_char,
    out_buf_len: usize,
) -> c_int {
    let result = panic::catch_unwind(|| -> Result<String, (c_int, String)> {
        // Safety: all pointers are checked non-null by the guard below.
        if chain_json.is_null()
            || agent_pk_hex.is_null()
            || intent_action.is_null()
            || mac_key.is_null()
            || out_buf.is_null()
            || out_buf_len == 0
        {
            return Err((A1Status::A1ErrUnknown as c_int, "null or zero-length argument".to_string()));
        }

        if rev_store.is_null() || nonce_store.is_null() {
            return Err((A1Status::A1ErrUnknown as c_int, "null store pointer argument".to_string()));
        }

        let chain_str = unsafe { CStr::from_ptr(chain_json) }
            .to_str()
            .map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("chain_json is not valid UTF-8: {e}")))?;

        let pk_hex = unsafe { CStr::from_ptr(agent_pk_hex) }
            .to_str()
            .map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("agent_pk_hex is not valid UTF-8: {e}")))?;

        let action = unsafe { CStr::from_ptr(intent_action) }
            .to_str()
            .map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("intent_action is not valid UTF-8: {e}")))?;

        let mac: [u8; 32] = unsafe { std::slice::from_raw_parts(mac_key, 32) }
            .try_into()
            .map_err(|_| (A1Status::A1ErrUnknown as c_int, "mac_key must be 32 bytes".to_string()))?;

        // Parse agent public key
        let pk_bytes: [u8; 32] = hex::decode(pk_hex)
            .map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("invalid agent_pk_hex: {e}")))?
            .try_into()
            .map_err(|_| (A1Status::A1ErrUnknown as c_int, "agent_pk must be 32 bytes".to_string()))?;
        let agent_pk = ed25519_dalek::VerifyingKey::from_bytes(&pk_bytes)
            .map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("invalid agent public key: {e}")))?;

        // Deserialize the chain
        let signed: crate::wire::SignedChain =
            serde_json::from_str(chain_str).map_err(|e| (A1Status::A1ErrWireFormat as c_int, format!("chain_json parse error: {e}")))?;

        #[allow(deprecated)]
        let chain = signed
            .into_chain()
            .map_err(|e| (A1Status::A1ErrWireFormat as c_int, format!("chain conversion error: {e}")))?;

        // Full intent path - use Intent::new to build the structural hash safely
        let intent = Intent::new(action).map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("intent error: {e}")))?;
        let intent_hash = intent.hash();

        let action_result = chain
            .authorize(
                &agent_pk,
                &intent_hash,
                &MerkleProof::default(), // Pass-through root match only via this basic FFI endpoint
                &SystemClock,
                unsafe { &(*rev_store).0 },
                unsafe { &(*nonce_store).0 },
            )
            .map_err(|e| (a1_error_to_status(&e), e.to_string()))?;

        let token = crate::wire::VerifiedToken::sign(&action_result.receipt, &mac);
        serde_json::to_string(&token).map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("token serialization: {e}")))
    });

    match result {
        Ok(Ok(json)) => {
            let cstr = CString::new(json).unwrap_or_default();
            let bytes = cstr.as_bytes_with_nul();
            if bytes.len() > out_buf_len {
                set_last_error(format!(
                    "output buffer too small: need {}, got {out_buf_len}",
                    bytes.len()
                ));
                return A1Status::A1ErrUnknown as c_int;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf as *mut u8, bytes.len())
            };
            A1Status::A1Ok as c_int
        }
        Ok(Err((code, msg))) => {
            set_last_error(msg);
            code
        }
        Err(_panic) => {
            set_last_error("internal panic in dyolo_authorize_json");
            A1Status::A1ErrPanic as c_int
        }
    }
}

/// Authorize without a `VerifiedToken` — write a JSON `VerificationReceipt`
/// to `out_buf`. Use this when no cross-service MAC transport is needed
/// (e.g. the caller is logging the receipt for audit purposes only).
///
/// Parameters match `dyolo_authorize_json` except there is no `mac_key`.
///
/// Returns `A1_OK` (0) on success.
///
/// # Safety
/// All pointer arguments must be valid and non-null.
#[cfg(feature = "wire")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dyolo_authorize_receipt_json(
    rev_store: *mut OpaqueRevocationStore,
    nonce_store: *mut OpaqueNonceStore,
    chain_json: *const c_char,
    agent_pk_hex: *const c_char,
    intent_action: *const c_char,
    out_buf: *mut c_char,
    out_buf_len: usize,
) -> c_int {
    let result = panic::catch_unwind(|| -> Result<String, (c_int, String)> {
        if chain_json.is_null()
            || agent_pk_hex.is_null()
            || intent_action.is_null()
            || out_buf.is_null()
            || out_buf_len == 0
            || rev_store.is_null()
            || nonce_store.is_null()
        {
            return Err((A1Status::A1ErrUnknown as c_int, "null or zero-length argument".to_string()));
        }

        let chain_str = unsafe { CStr::from_ptr(chain_json) }
            .to_str()
            .map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("chain_json: {e}")))?;
        let pk_hex = unsafe { CStr::from_ptr(agent_pk_hex) }
            .to_str()
            .map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("agent_pk_hex: {e}")))?;
        let action = unsafe { CStr::from_ptr(intent_action) }
            .to_str()
            .map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("intent_action: {e}")))?;

        let pk_bytes: [u8; 32] = hex::decode(pk_hex)
            .map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("invalid agent_pk_hex: {e}")))?
            .try_into()
            .map_err(|_| (A1Status::A1ErrUnknown as c_int, "agent_pk must be 32 bytes".to_string()))?;
        let agent_pk = ed25519_dalek::VerifyingKey::from_bytes(&pk_bytes)
            .map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("invalid agent public key: {e}")))?;

        let signed: crate::wire::SignedChain =
            serde_json::from_str(chain_str).map_err(|e| (A1Status::A1ErrWireFormat as c_int, format!("chain_json parse error: {e}")))?;

        #[allow(deprecated)]
        let chain = signed.into_chain().map_err(|e| (A1Status::A1ErrWireFormat as c_int, format!("{e}")))?;

        let intent = Intent::new(action).map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("intent error: {e}")))?;
        let intent_hash = intent.hash();

        let authorized = chain
            .authorize(
                &agent_pk,
                &intent_hash,
                &MerkleProof::default(),
                &SystemClock,
                unsafe { &(*rev_store).0 },
                unsafe { &(*nonce_store).0 },
            )
            .map_err(|e| (a1_error_to_status(&e), e.to_string()))?;

        serde_json::to_string(&authorized.receipt)
            .map_err(|e| (A1Status::A1ErrUnknown as c_int, format!("receipt serialization: {e}")))
    });

    match result {
        Ok(Ok(json)) => {
            let cstr = CString::new(json).unwrap_or_default();
            let bytes = cstr.as_bytes_with_nul();
            if bytes.len() > out_buf_len {
                set_last_error(format!(
                    "output buffer too small: need {}, got {out_buf_len}",
                    bytes.len()
                ));
                return A1Status::A1ErrUnknown as c_int;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf as *mut u8, bytes.len())
            };
            A1Status::A1Ok as c_int
        }
        Ok(Err((code, msg))) => {
            set_last_error(msg);
            code
        }
        Err(_) => {
            set_last_error("internal panic in dyolo_authorize_receipt_json");
            A1Status::A1ErrPanic as c_int
        }
    }
}

/// Authorize with a provided Merkle Proof.
/// `proof_json` must be a serialized `MerkleProof`.
///
/// # Safety
///
/// All pointer arguments must be valid, non-null, null-terminated C strings (except `mac_key`
/// which must point to at least 32 bytes, and `out_buf` which must point to `out_buf_len` bytes).
/// `rev_store` and `nonce_store` must be valid pointers from their respective `_new` functions.
#[cfg(feature = "wire")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dyolo_authorize_with_proof_json(
    rev_store: *mut OpaqueRevocationStore,
    nonce_store: *mut OpaqueNonceStore,
    chain_json: *const c_char,
    agent_pk_hex: *const c_char,
    intent_action: *const c_char,
    proof_json: *const c_char,
    mac_key: *const u8,
    out_buf: *mut c_char,
    out_buf_len: usize,
) -> c_int {
    let result = panic::catch_unwind(|| {
        if chain_json.is_null()
            || agent_pk_hex.is_null()
            || intent_action.is_null()
            || mac_key.is_null()
            || out_buf.is_null()
            || out_buf_len == 0
            || proof_json.is_null()
            || rev_store.is_null()
            || nonce_store.is_null()
        {
            return Err("null argument".to_string());
        }

        let proof_str = unsafe { CStr::from_ptr(proof_json) }
            .to_str()
            .map_err(|e| e.to_string())?;
        let proof: MerkleProof = serde_json::from_str(proof_str).map_err(|e| e.to_string())?;

        let chain_str = unsafe { CStr::from_ptr(chain_json) }
            .to_str()
            .map_err(|e| e.to_string())?;
        let action = unsafe { CStr::from_ptr(intent_action) }
            .to_str()
            .map_err(|e| e.to_string())?;

        let pk_hex = unsafe { CStr::from_ptr(agent_pk_hex) }
            .to_str()
            .map_err(|e| e.to_string())?;
        let pk_bytes: [u8; 32] = hex::decode(pk_hex)
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|_| "32 bytes".to_string())?;
        let agent_pk =
            ed25519_dalek::VerifyingKey::from_bytes(&pk_bytes).map_err(|e| e.to_string())?;

        let mac: [u8; 32] = unsafe { std::slice::from_raw_parts(mac_key, 32) }
            .try_into()
            .map_err(|_| "32 bytes".to_string())?;

        let signed: crate::wire::SignedChain =
            serde_json::from_str(chain_str).map_err(|e| e.to_string())?;

        #[allow(deprecated)]
        let chain = signed.into_chain().map_err(|e| e.to_string())?;

        let intent = Intent::new(action).map_err(|e| e.to_string())?;
        let intent_hash = intent.hash();

        let action_result = chain
            .authorize(
                &agent_pk,
                &intent_hash,
                &proof,
                &SystemClock,
                unsafe { &(*rev_store).0 },
                unsafe { &(*nonce_store).0 },
            )
            .map_err(|e| e.to_string())?;

        let token = crate::wire::VerifiedToken::sign(&action_result.receipt, &mac);
        serde_json::to_string(&token).map_err(|e| e.to_string())
    });

    match result {
        Ok(Ok(json)) => {
            let cstr = CString::new(json).unwrap_or_default();
            let bytes = cstr.as_bytes_with_nul();
            if bytes.len() > out_buf_len {
                return A1Status::A1ErrUnknown as c_int;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf as *mut u8, bytes.len())
            };
            A1Status::A1Ok as c_int
        }
        Ok(Err(msg)) => {
            set_last_error(msg);
            A1Status::A1ErrUnknown as c_int
        }
        Err(_) => A1Status::A1ErrPanic as c_int,
    }
}

// ── Version ───────────────────────────────────────────────────────────────────

/// Return the nul-terminated semantic version string of this build
/// (e.g. `"2.0.0"`).
///
/// The returned pointer is valid for the lifetime of the process.
/// Do NOT free it.
#[unsafe(no_mangle)]
pub extern "C" fn dyolo_version() -> *const c_char {
    // SAFETY: This is a static string literal, always valid UTF-8 and nul-terminated.
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}