#![no_main]

risc0_zkvm::guest::entry!(main);

use blake3::Hasher;
use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use risc0_zkvm::guest::env;
use subtle::ConstantTimeEq;

// Domain constants must match dyolo:: exactly — these are version-pinned.
const DOMAIN_CERT_SIG:    &str = "dyolo::cert::sig::v1";
const DOMAIN_CERT_FP:     &str = "dyolo::cert::fp::v1";
const DOMAIN_CHAIN_FP:    &str = "dyolo::chain::fp::v1";
const DOMAIN_SUBSCOPE:    &str = "dyolo::subscope::commit::v1";
const DOMAIN_MERKLE_NODE: &str = "dyolo::merkle::node::v1";
const CERT_VERSION:       u8   = 1;

struct WireProofNode {
    hash: [u8; 32],
    is_left: bool,
}

// Wire protocol mirror of DelegationCert fields, received individually from the host.
struct WireCert {
    delegator_pk:           [u8; 32],
    delegate_pk:            [u8; 32],
    scope_root:             [u8; 32],
    subset_intents:         Vec<[u8; 32]>,
    proofs:                 Vec<Vec<WireProofNode>>,
    nonce:                  [u8; 16],
    issued_at:              u64,
    expiration_unix:        u64,
    max_depth:              u8,
    extensions_hash:        [u8; 32],
    signature:              [u8; 64],
}

pub fn main() {
    // ── Read chain inputs from host ───────────────────────────────────────────

    let principal_pk_bytes:  [u8; 32] = env::read();
    let principal_scope:     [u8; 32] = env::read();
    let num_certs:           u32      = env::read();

    let mut certs: Vec<WireCert> = Vec::with_capacity(num_certs as usize);
    for _ in 0..num_certs {
        let delegator_pk: [u8; 32] = env::read();
        let delegate_pk:  [u8; 32] = env::read();
        let scope_root:   [u8; 32] = env::read();
        
        let subset_len: u32 = env::read();
        let mut subset_intents = Vec::with_capacity(subset_len as usize);
        let mut proofs = Vec::with_capacity(subset_len as usize);
        
        for _ in 0..subset_len {
            subset_intents.push(env::read());
            let nodes_len: u32 = env::read();
            let mut nodes = Vec::with_capacity(nodes_len as usize);
            for _ in 0..nodes_len {
                nodes.push(WireProofNode {
                    hash: env::read(),
                    is_left: env::read(),
                });
            }
            proofs.push(nodes);
        }

        certs.push(WireCert {
            delegator_pk,
            delegate_pk,
            scope_root,
            subset_intents,
            proofs,
            nonce:           env::read(),
            issued_at:       env::read(),
            expiration_unix: env::read(),
            max_depth:       env::read(),
            extensions_hash: env::read(),
            signature:       env::read(),
        });
    }

    let executor_pk_bytes:    [u8; 32] = env::read();
    let intent:               [u8; 32] = env::read();
    let chain_fingerprint_in: [u8; 32] = env::read();
    let verified_at:          u64      = env::read();

    // ── Verify chain inside the ZK VM ─────────────────────────────────────────

    let mut current_scope      = principal_scope;
    let mut expected_delegator = principal_pk_bytes;
    let mut parent_expiry      = u64::MAX;
    let mut max_allowed_depth  = u8::MAX;
    let mut cert_fingerprints: Vec<[u8; 32]> = Vec::with_capacity(certs.len());

    for (i, cert) in certs.iter().enumerate() {
        // Linkage: delegator must equal the prior cert's delegate.
        assert!(
            cert.delegator_pk.ct_eq(&expected_delegator).into(),
            "dyolo::zk: broken linkage at hop {i}"
        );

        // Temporal monotonicity: child cannot outlive parent.
        assert!(
            cert.expiration_unix <= parent_expiry,
            "dyolo::zk: temporal violation at hop {i}"
        );

        // Temporal validity at the attested verification timestamp.
        assert!(cert.issued_at <= verified_at, "dyolo::zk: cert not yet valid at hop {i}");
        assert!(cert.expiration_unix >= verified_at, "dyolo::zk: cert expired at hop {i}");

        // Depth cap enforcement.
        let depth = (i + 1) as u8;
        assert!(depth <= max_allowed_depth, "dyolo::zk: max depth exceeded at hop {i}");
        if cert.max_depth < max_allowed_depth {
            max_allowed_depth = cert.max_depth;
        }

        // Scope narrowing verification.
        if cert.subset_intents.is_empty() {
            assert!(
                cert.scope_root.ct_eq(&current_scope).into(),
                "dyolo::zk: scope escalation at hop {i}"
            );
        } else {
            for (intent_hash, proof_nodes) in cert.subset_intents.iter().zip(cert.proofs.iter()) {
                let mut current = *intent_hash;
                for node in proof_nodes {
                    current = if node.is_left {
                        merkle_node(&node.hash, &current)
                    } else {
                        merkle_node(&current, &node.hash)
                    };
                }
                assert!(
                    current.ct_eq(&current_scope).into(),
                    "dyolo::zk: invalid subscope proof at hop {i}"
                );
            }
            
            // Derive the new scope root by building a tree from the subset
            let mut sorted_subset = cert.subset_intents.clone();
            sorted_subset.sort_unstable();
            sorted_subset.dedup();
            
            let mut layer = sorted_subset;
            while layer.len() > 1 {
                let next_len = (layer.len() + 1) / 2;
                let mut next = Vec::with_capacity(next_len);
                for chunk in layer.chunks(2) {
                    if chunk.len() == 2 {
                        next.push(merkle_node(&chunk[0], &chunk[1]));
                    } else {
                        next.push(chunk[0]);
                    }
                }
                layer = next;
            }
            let derived_root = layer[0];
            assert!(
                derived_root.ct_eq(&cert.scope_root).into(),
                "dyolo::zk: derived scope root mismatch at hop {i}"
            );
        }

        let scope_proof_commitment = subscope_commitment(&cert.subset_intents, &cert.proofs);

        // Ed25519 signature verification.
        let delegator_vk = VerifyingKey::from_bytes(&cert.delegator_pk)
            .expect("dyolo::zk: invalid delegator key");

        let msg = signable_bytes(
            CERT_VERSION,
            &cert.delegator_pk,
            &cert.delegate_pk,
            &cert.scope_root,
            &scope_proof_commitment,
            &cert.nonce,
            cert.issued_at,
            cert.expiration_unix,
            cert.max_depth,
            &cert.extensions_hash,
        );

        let sig = Signature::from_bytes(&cert.signature);
        delegator_vk.verify(&msg, &sig)
            .unwrap_or_else(|_| panic!("dyolo::zk: invalid signature at hop {i}"));

        cert_fingerprints.push(cert_fingerprint(&cert.signature));
        parent_expiry      = cert.expiration_unix;
        expected_delegator = cert.delegate_pk;
        current_scope      = cert.scope_root;
    }

    // ── Verify executor is the terminal delegate ──────────────────────────────

    assert!(
        expected_delegator.ct_eq(&executor_pk_bytes).into(),
        "dyolo::zk: unauthorized leaf"
    );

    // ── Recompute and assert chain fingerprint ────────────────────────────────

    let computed_fp = chain_fingerprint(&principal_pk_bytes, &principal_scope, &cert_fingerprints);
    assert!(
        computed_fp.ct_eq(&chain_fingerprint_in).into(),
        "dyolo::zk: chain fingerprint mismatch"
    );

    // ── Commit 136-byte journal ───────────────────────────────────────────────
    //
    // Layout matches DyoloZkRollup::verify_state_transition exactly:
    //   [0..32]   principal_pk
    //   [32..64]  executor_pk
    //   [64..96]  intent
    //   [96..128] state_root (chain_fingerprint)
    //   [128..136] verified_at (wall-clock attestation)

    let mut journal = [0u8; 136];
    journal[0..32].copy_from_slice(&principal_pk_bytes);
    journal[32..64].copy_from_slice(&executor_pk_bytes);
    journal[64..96].copy_from_slice(&intent);
    journal[96..128].copy_from_slice(&chain_fingerprint_in);
    journal[128..136].copy_from_slice(&verified_at.to_be_bytes());

    env::commit_slice(&journal);
}

// ── Internal helpers — mirrors a1 internals exactly ───────────────────

fn derive_key(domain: &str, version: u8) -> Hasher {
    let mut h = Hasher::new_derive_key(domain);
    h.update(&[version]);
    h
}

fn signable_bytes(
    version:                u8,
    delegator_pk:           &[u8; 32],
    delegate_pk:            &[u8; 32],
    scope_root:             &[u8; 32],
    scope_proof_commitment: &[u8; 32],
    nonce:                  &[u8; 16],
    issued_at:              u64,
    expiration_unix:        u64,
    max_depth:              u8,
    ext_commitment:         &[u8; 32],
) -> Vec<u8> {
    let mut h = derive_key(DOMAIN_CERT_SIG, version);
    h.update(delegator_pk);
    h.update(delegate_pk);
    h.update(scope_root);
    h.update(scope_proof_commitment);
    h.update(nonce);
    h.update(&issued_at.to_be_bytes());
    h.update(&expiration_unix.to_be_bytes());
    h.update(&[max_depth]);
    h.update(ext_commitment);
    h.finalize().as_bytes().to_vec()
}

fn cert_fingerprint(signature_bytes: &[u8; 64]) -> [u8; 32] {
    let mut h = derive_key(DOMAIN_CERT_FP, CERT_VERSION);
    h.update(signature_bytes);
    h.finalize().into()
}

fn chain_fingerprint(
    principal_pk:    &[u8; 32],
    principal_scope: &[u8; 32],
    cert_fps:        &[[u8; 32]],
) -> [u8; 32] {
    let mut h = derive_key(DOMAIN_CHAIN_FP, CERT_VERSION);
    h.update(principal_pk);
    h.update(principal_scope);
    for fp in cert_fps {
        h.update(fp);
    }
    h.finalize().into()
}

fn merkle_node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = derive_key(DOMAIN_MERKLE_NODE, CERT_VERSION);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

fn subscope_commitment(subset_intents: &[[u8; 32]], proofs: &[Vec<WireProofNode>]) -> [u8; 32] {
    let mut h = derive_key(DOMAIN_SUBSCOPE, CERT_VERSION);
    h.update(&(subset_intents.len() as u64).to_le_bytes());
    for intent in subset_intents {
        h.update(intent);
    }
    h.update(&(proofs.len() as u64).to_le_bytes());
    for proof in proofs {
        h.update(&(proof.len() as u64).to_le_bytes());
        for node in proof {
            h.update(&node.hash);
            h.update(&[node.is_left as u8]);
        }
    }
    h.finalize().into()
}