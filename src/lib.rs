#![doc(
    html_logo_url = "https://raw.githubusercontent.com/dyologician/a1/main/docs/assets/logo.png"
)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/dyologician/a1/main/docs/assets/favicon.ico"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
//! # A1 — One Identity. Full Provenance. v2.8.0
//!
//! The cryptographic identity and authorization layer that turns anonymous AI
//! agents into accountable, verifiable entities.
//!
//! ## What it solves
//!
//! When one AI agent delegates a task to another, the authorization chain breaks
//! down — a liability called the "Recursive Delegation Gap." A1 closes that gap
//! with a native A1 Passport protocol: every action executed by any agent in a
//! delegation tree carries an irrefutable, cryptographically verified chain
//! proving exactly which human authorized it, with enforced scope boundaries
//! that hold offline.
//!
//! ## v2.8.0 additions
//!
//! - **DyoloPassport** — long-lived agent identity with cryptographically
//!   enforced capability bounds. Issue once, delegate scoped sub-certs per task.
//!   The chain of custody is irrefutable from human principal to executing agent.
//!
//! - **NarrowingMatrix** — a 256-bit O(1) capability bitmask enforcing strict
//!   subset delegation at both issuance and guard time. No external registry, no
//!   network call, no configuration at verification time. Pure bitwise arithmetic.
//!
//! - **CapabilityRegistry** — collision-free explicit name-to-bit registry for
//!   deployments with more than ~100 distinct capability names.
//!
//! - **ProvableReceipt** — an extended authorization receipt carrying the passport
//!   namespace and a Blake3 commitment over the enforced capability mask, enabling
//!   post-hoc audit without retaining any secrets.
//!
//! - **W3C DID + Verifiable Credentials** (`did` feature) — every DyoloPassport
//!   holder gets a permanent `did:a1:` identifier. Issue portable VCs for
//!   capabilities and receipts that verify offline on any platform.
//!
//! - **ZK chain commitments** (`zk` feature) — compact, O(1)-verifiable
//!   commitments to full delegation chains. Upgrade path to full zkVM proofs
//!   (RISC Zero, Jolt, SP1) without changing consumer code.
//!
//! - **Post-quantum hybrid signatures** — `HybridMlDsa44Ed25519` and
//!   `HybridMlDsa65Ed25519` wire formats. Classical Ed25519 by default;
//!   activate full ML-DSA verification with the `post-quantum` feature flag.
//!
//! - **VaultSigner backends** — AWS KMS, GCP Cloud KMS, HashiCorp Vault Transit,
//!   and Azure Key Vault signing. Root key material never touches application
//!   memory at issuance time. Zero KMS calls at verification time.
//!
//! - **SIEM exporters** — Datadog Logs, Splunk HEC, OpenTelemetry OTLP, and
//!   NDJSON file exporters. Fan-out via `CompositeExporter`.
//!
//! - **Framework integrations** — LangChain, LangGraph, LlamaIndex, AutoGen v0.4,
//!   CrewAI, Semantic Kernel, and OpenAI Agents SDK.
//!
//! ## Feature flags
//!
//! | Flag            | Description                                                             |
//! |-----------------|-------------------------------------------------------------------------|
//! | `serde`         | Serialization for all core types. Required for most integrations.       |
//! | `async`         | `AsyncNonceStore`, `AsyncRevocationStore`, `AsyncA1Context`.            |
//! | `wire`          | `SignedChain`, `VerifiedToken`, `CertExtensions` (requires `serde`).    |
//! | `did`           | W3C DID Documents and Verifiable Credentials (requires `wire`).         |
//! | `zk`            | `ZkChainCommitment` — compact chain attestation with zkVM upgrade path. |
//! | `anchor`        | `AnchoredReceipt` — on-chain provenance for Ethereum, Polygon, Base, Solana. |
//! | `negotiate`     | Agent-to-agent delegation negotiation protocol (AIP).                   |
//! | `tracing`       | Structured `tracing` spans during authorization.                        |
//! | `ffi`           | C ABI for Python, Go, Java, and Node.js (requires `wire`).              |
//! | `policy-yaml`   | Parse delegation policies from YAML files.                              |
//! | `post-quantum`  | Activate ML-DSA signature verification (hybrid certs, requires `wire`). |
//! | `schema`        | JSON Schema export for `SignedChain`.                                   |
//! | `full`          | All of the above except `ffi` and `post-quantum`.                       |

#![deny(unsafe_code)]

mod crypto;

pub mod audit;
pub mod cert;
pub mod chain;
pub mod context;
pub mod error;
pub mod hybrid;
pub mod identity;
pub mod intent;
pub mod passport;
pub mod policy;
pub mod provenance;
pub mod registry;

#[cfg(feature = "wire")]
#[cfg_attr(docsrs, doc(cfg(feature = "wire")))]
pub mod cert_extensions;

#[cfg(feature = "wire")]
#[cfg_attr(docsrs, doc(cfg(feature = "wire")))]
pub mod wire;

#[cfg(feature = "did")]
#[cfg_attr(docsrs, doc(cfg(feature = "did")))]
pub mod did;

#[cfg(feature = "zk")]
#[cfg_attr(docsrs, doc(cfg(feature = "zk")))]
pub mod zk;

#[cfg(feature = "anchor")]
#[cfg_attr(docsrs, doc(cfg(feature = "anchor")))]
pub mod anchor;

#[cfg(feature = "negotiate")]
#[cfg_attr(docsrs, doc(cfg(feature = "negotiate")))]
pub mod negotiate;

#[cfg(feature = "swarm")]
#[cfg_attr(docsrs, doc(cfg(feature = "swarm")))]
pub mod swarm;

#[cfg(feature = "governance")]
#[cfg_attr(docsrs, doc(cfg(feature = "governance")))]
pub mod governance;

#[cfg(feature = "ffi")]
#[cfg_attr(docsrs, doc(cfg(feature = "ffi")))]
#[allow(unsafe_code)]
pub mod ffi;

pub use audit::{
    AuditEvent, AuditOutcome, AuditSink, CompositeAuditSink, LogAuditSink, NoopAuditSink,
};
pub use cert::{CertBuilder, CertBundle, DelegationCert, CERT_VERSION};
pub use chain::{
    AuthorizedAction, BatchAuthorizeResult, Clock, DyoloChain, SystemClock, VerificationReceipt,
};
pub use context::A1Context;
pub use error::{A1Error, A1StorageError, StorageErrorKind};
pub use identity::{DyoloIdentity, SharedIdentity, Signer};
#[allow(deprecated)]
pub use intent::{
    intent_hash, Intent, IntentHash, IntentTree, MerkleProof, SiblingNode, SubScopeProof,
};
pub use passport::DyoloPassport;
pub use identity::narrowing::{CapabilityRegistry, NarrowingMatrix};
pub use identity::receipt::ProvableReceipt;
pub use provenance::{
    ProvenanceRoot, ProvenanceStepProof, ReasoningStepKind, ReasoningStep, ReasoningTrace,
};
pub use hybrid::{
    ChainAlgorithmCompatibility, ClassicalHybridAdapter, HybridPublicKey, HybridSignature,
    HybridSigner, SignatureAlgorithm, negotiate_algorithm,
};
pub use policy::{CapabilitySet, DelegationPolicy, PolicySet};
pub use registry::{
    fresh_nonce, MemoryNonceStore, MemoryRateLimitStore, MemoryRevocationStore, NonceStore,
    RateLimitStore, RevocationStore,
};

#[cfg(feature = "wire")]
#[cfg_attr(docsrs, doc(cfg(feature = "wire")))]
pub use cert_extensions::{CertExtensions, ExtValue};

#[cfg(feature = "did")]
#[cfg_attr(docsrs, doc(cfg(feature = "did")))]
pub use did::{AgentDid, CredentialSubject, DidDocument, VcProof, VerifiableCredential, VerificationMethod};

#[cfg(feature = "zk")]
#[cfg_attr(docsrs, doc(cfg(feature = "zk")))]
pub use zk::{ZkChainCommitment, ZkProofMode, ZkTraceProof, anchor_hash};

#[cfg(feature = "anchor")]
#[cfg_attr(docsrs, doc(cfg(feature = "anchor")))]
pub use anchor::{AnchoredReceipt, AnchorNetwork};

#[cfg(feature = "negotiate")]
#[cfg_attr(docsrs, doc(cfg(feature = "negotiate")))]
pub use negotiate::{CapabilityRequest, DelegationOffer, DelegationAcceptance, NegotiationResult};

#[cfg(feature = "swarm")]
#[cfg_attr(docsrs, doc(cfg(feature = "swarm")))]
pub use swarm::{SwarmPassport, SwarmMember, SwarmRole};

#[cfg(feature = "governance")]
#[cfg_attr(docsrs, doc(cfg(feature = "governance")))]
pub use governance::{GovernancePolicy, ApprovalGate, ApprovalToken, AuditReport, KeyRotationPolicy};

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use context::AsyncA1Context;

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use registry::r#async::{
    AsyncNonceStore, AsyncRateLimitStore, AsyncRevocationStore, SyncNonceAdapter,
    SyncRevocationAdapter,
};

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use audit::r#async::{AsyncAuditSink, SyncAuditAdapter};

#[cfg(feature = "async")]
#[cfg_attr(docsrs, doc(cfg(feature = "async")))]
pub use identity::AsyncSigner;
