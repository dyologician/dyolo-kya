use a1::{
    fresh_nonce,
    policy::{CapabilitySet, PolicySet},
    AuditEvent, AuditOutcome, AuditSink, CertBuilder, Clock, DelegationPolicy, DyoloChain,
    DyoloIdentity, Intent, IntentTree, MemoryNonceStore, MemoryRevocationStore, MerkleProof,
    NonceStore, NoopAuditSink, RevocationStore, SharedIdentity, Signer, SubScopeProof, SystemClock,
};
use std::sync::{Arc, Mutex};

// ── Test infrastructure ───────────────────────────────────────────────────────

struct FixedClock(u64);
impl Clock for FixedClock {
    fn unix_now(&self) -> u64 {
        self.0
    }
}

fn base_chain(ttl: u64) -> (DyoloIdentity, DyoloIdentity, DyoloChain) {
    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let trade = Intent::new("trade.equity").unwrap().hash();
    let tree = IntentTree::build(vec![trade]).unwrap();
    let now = SystemClock.unix_now();
    let cert = CertBuilder::new(agent.verifying_key(), tree.root(), now, now + ttl)
        .nonce(fresh_nonce())
        .sign(&human);
    let mut chain = DyoloChain::new(human.verifying_key(), tree.root());
    chain.push(cert);
    (human, agent, chain)
}

// ── Capturing audit sink ──────────────────────────────────────────────────────

#[derive(Default)]
struct CaptureAuditSink {
    events: Mutex<Vec<AuditEvent>>,
}

impl AuditSink for CaptureAuditSink {
    fn emit(&self, event: AuditEvent) {
        self.events.lock().unwrap().push(event);
    }
}

// ── Adversarial clock mismatch ────────────────────────────────────────────────

#[test]
fn adversarial_clock_mismatch() {
    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let trade = Intent::new("trade").unwrap().hash();
    let tree = IntentTree::build(vec![trade]).unwrap();

    let issue_clock = 1_000_000;
    let verify_clock = 2_000_000;

    let cert = CertBuilder::new(
        agent.verifying_key(),
        tree.root(),
        issue_clock,
        issue_clock + 3600,
    )
    .sign(&human);
    let mut chain = DyoloChain::new(human.verifying_key(), tree.root());
    chain.push(cert);

    let res = chain.authorize(
        &agent.verifying_key(),
        &trade,
        &MerkleProof::default(),
        &FixedClock(verify_clock),
        &MemoryRevocationStore::new(),
        &MemoryNonceStore::new(),
    );
    assert!(res.is_err(), "desynchronized future clock must be rejected");
}

// ── Adversarial tampered subscope ─────────────────────────────────────────────

#[test]
fn adversarial_tampered_subscope() {
    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let intent1 = Intent::new("action.1").unwrap().hash();
    let intent2 = Intent::new("action.2").unwrap().hash();
    let parent = IntentTree::build(vec![intent1, intent2]).unwrap();
    let mut proof = SubScopeProof::build(&parent, &[intent1]).unwrap();

    if let Some(node) = proof.proofs[0].siblings.get_mut(0) {
        node.hash[0] ^= 0xFF;
    }

    let now = SystemClock.unix_now();
    let child_root = IntentTree::build(vec![intent1]).unwrap().root();
    let cert = CertBuilder::new(agent.verifying_key(), child_root, now, now + 3600)
        .scope_proof(proof)
        .sign(&human);

    let mut chain = DyoloChain::new(human.verifying_key(), parent.root());
    chain.push(cert);

    let res = chain.authorize(
        &agent.verifying_key(),
        &intent1,
        &MerkleProof::default(),
        &SystemClock,
        &MemoryRevocationStore::new(),
        &MemoryNonceStore::new(),
    );
    assert!(
        res.is_err(),
        "tampered SubScopeProof sibling hashes must be rejected"
    );
}

// ── Adversarial mid-execution revocation ──────────────────────────────────────

#[test]
fn adversarial_mid_execution_revocation() {
    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let trade = Intent::new("trade").unwrap().hash();
    let tree = IntentTree::build(vec![trade]).unwrap();
    let now = SystemClock.unix_now();

    let cert1 = CertBuilder::new(agent.verifying_key(), tree.root(), now, now + 3600)
        .nonce(fresh_nonce())
        .sign(&human);
    let cert2 = CertBuilder::new(agent.verifying_key(), tree.root(), now, now + 3600)
        .nonce(fresh_nonce())
        .sign(&human);

    let mut chain1 = DyoloChain::new(human.verifying_key(), tree.root());
    chain1.push(cert1);
    let mut chain2 = DyoloChain::new(human.verifying_key(), tree.root());
    chain2.push(cert2.clone());

    let rev = MemoryRevocationStore::new();
    let nonces = MemoryNonceStore::new();

    assert!(chain1
        .authorize(
            &agent.verifying_key(),
            &trade,
            &MerkleProof::default(),
            &SystemClock,
            &rev,
            &nonces
        )
        .is_ok());

    rev.revoke(&cert2.fingerprint()).unwrap();

    assert!(
        chain2
            .authorize(
                &agent.verifying_key(),
                &trade,
                &MerkleProof::default(),
                &SystemClock,
                &rev,
                &nonces
            )
            .is_err(),
        "mid-flight revocation must be enforced immediately"
    );
}

// ── Nonce replay rejection ────────────────────────────────────────────────────

#[test]
fn nonce_replay_rejected() {
    let nonce = fresh_nonce();
    let store = MemoryNonceStore::new();
    assert!(
        store.try_consume(&nonce).unwrap(),
        "first consumption must succeed"
    );
    assert!(
        !store.try_consume(&nonce).unwrap(),
        "replay must be rejected"
    );
}

// ── Policy: chain depth enforcement ──────────────────────────────────────────

#[test]
fn policy_chain_depth_enforced() {
    let human_id = DyoloIdentity::generate();
    let human = SharedIdentity(Arc::new(human_id));
    let now = SystemClock.unix_now();
    let trade = Intent::new("trade.equity").unwrap().hash();
    let tree = IntentTree::build(vec![trade]).unwrap();
    let scope = tree.root();

    let mut chain = DyoloChain::new(human.verifying_key(), scope);
    let mut prev_signer: Box<dyn Signer> = Box::new(human.clone());

    for _ in 0..3 {
        let next = DyoloIdentity::generate();
        let next_vk = next.verifying_key();
        let cert = CertBuilder::new(next_vk, scope, now, now + 3600).sign(prev_signer.as_ref());
        chain.push(cert);
        prev_signer = Box::new(next);
    }

    let shallow = DelegationPolicy::new("shallow").max_chain_depth(2);
    let deep = DelegationPolicy::new("deep").max_chain_depth(5);

    assert!(
        shallow.check_chain(&chain).is_err(),
        "chain depth 3 must exceed max 2"
    );
    assert!(
        deep.check_chain(&chain).is_ok(),
        "chain depth 3 is within max 5"
    );
}

// ── Policy: TTL enforcement ───────────────────────────────────────────────────

#[test]
fn policy_ttl_enforced() {
    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let now = 1_700_000_000u64;
    let scope = IntentTree::build(vec![Intent::new("trade").unwrap().hash()])
        .unwrap()
        .root();

    let cert = CertBuilder::new(agent.verifying_key(), scope, now, now + 7200).sign(&human);
    let mut chain = DyoloChain::new(human.verifying_key(), scope);
    chain.push(cert);

    let strict = DelegationPolicy::new("strict-ttl").max_ttl_secs(3600);
    let relaxed = DelegationPolicy::new("relaxed-ttl").max_ttl_secs(86400);

    assert!(
        strict.check_chain(&chain).is_err(),
        "7200s cert must violate 3600s TTL policy"
    );
    assert!(
        relaxed.check_chain(&chain).is_ok(),
        "7200s cert must pass 86400s TTL policy"
    );
}

// ── Policy: capability set prefix matching ────────────────────────────────────

#[test]
fn policy_capability_set_prefix_matching() {
    let caps = CapabilitySet::new().allow("trade.").allow("query");
    assert!(
        caps.permits("trade.equity"),
        "prefix 'trade.' must match 'trade.equity'"
    );
    assert!(
        caps.permits("trade.fx"),
        "prefix 'trade.' must match 'trade.fx'"
    );
    assert!(caps.permits("query"), "exact 'query' must pass");
    assert!(
        !caps.permits("admin.delete"),
        "unrelated prefix must be denied"
    );
}

#[test]
fn policy_capability_wildcard_permits_all() {
    let caps = CapabilitySet::wildcard();
    assert!(caps.permits("anything.at.all"));
    assert!(caps.permits("admin.nuclear_launch"));
}

#[test]
fn policy_intent_blocked_by_capability_set() {
    let policy = DelegationPolicy::new("trade-only")
        .capabilities(CapabilitySet::new().allow("trade.equity"));
    assert!(policy
        .check_intent(&Intent::new("trade.equity").unwrap())
        .is_ok());
    assert!(policy
        .check_intent(&Intent::new("admin.delete").unwrap())
        .is_err());
}

// ── Policy: PolicySet (layered, first violation short-circuits) ───────────────

#[test]
fn policy_set_first_violation_short_circuits() {
    let human = DyoloIdentity::generate();
    let now = 1_700_000_000u64;
    let scope = IntentTree::build(vec![Intent::new("trade").unwrap().hash()])
        .unwrap()
        .root();

    let mut chain = DyoloChain::new(human.verifying_key(), scope);
    for _ in 0..4 {
        let next = DyoloIdentity::generate();
        let cert = CertBuilder::new(next.verifying_key(), scope, now, now + 3600).sign(&human);
        chain.push(cert);
    }

    let set = PolicySet::new()
        .add(DelegationPolicy::new("depth").max_chain_depth(3))
        .add(DelegationPolicy::new("ttl").max_ttl_secs(86400));

    assert!(
        set.check_chain(&chain).is_err(),
        "depth violation must short-circuit the set"
    );
}

// ── Policy: trusted principal set ────────────────────────────────────────────

#[test]
fn policy_untrusted_principal_rejected() {
    let trusted = DyoloIdentity::generate();
    let untrusted = DyoloIdentity::generate();
    let scope = IntentTree::build(vec![Intent::new("act").unwrap().hash()])
        .unwrap()
        .root();
    let now = 1_700_000_000u64;

    let trusted_chain = {
        let mut c = DyoloChain::new(trusted.verifying_key(), scope);
        c.push(
            CertBuilder::new(
                DyoloIdentity::generate().verifying_key(),
                scope,
                now,
                now + 3600,
            )
            .sign(&trusted),
        );
        c
    };
    let untrusted_chain = {
        let mut c = DyoloChain::new(untrusted.verifying_key(), scope);
        c.push(
            CertBuilder::new(
                DyoloIdentity::generate().verifying_key(),
                scope,
                now,
                now + 3600,
            )
            .sign(&untrusted),
        );
        c
    };

    let policy = DelegationPolicy::new("acl").trust_principal(*trusted.verifying_key().as_bytes());

    assert!(policy.check_chain(&trusted_chain).is_ok());
    assert!(policy.check_chain(&untrusted_chain).is_err());
}

// ── authorize_with_options: audit sink captures success event ─────────────────

#[test]
fn audit_sink_records_success_event() {
    let (_human, agent, chain) = base_chain(3600);
    let trade = Intent::new("trade.equity").unwrap().hash();
    let nonces = MemoryNonceStore::new();
    let sink = Arc::new(CaptureAuditSink::default());

    let _ = chain
        .authorize_with_options(
            &agent.verifying_key(),
            &trade,
            &MerkleProof::default(),
            &SystemClock,
            &MemoryRevocationStore::new(),
            &nonces,
            None,
            sink.as_ref(),
        )
        .expect("authorization must succeed");

    let events = sink.events.lock().unwrap();
    assert_eq!(events.len(), 1, "exactly one audit event must be emitted");
    assert_eq!(events[0].outcome, AuditOutcome::Authorized);
    assert_eq!(events[0].chain_depth, 1);
    assert!(
        events[0].chain_fingerprint.is_some(),
        "fingerprint must be set on success"
    );
}

// ── authorize_with_options: audit sink captures failure event on replay ────────

#[test]
fn audit_sink_records_failure_event_on_replay() {
    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let trade = Intent::new("trade").unwrap().hash();
    let tree = IntentTree::build(vec![trade]).unwrap();
    let now = SystemClock.unix_now();
    let nonce = fresh_nonce();

    let cert = CertBuilder::new(agent.verifying_key(), tree.root(), now, now + 3600)
        .nonce(nonce)
        .sign(&human);

    let rev = MemoryRevocationStore::new();
    let nonces = MemoryNonceStore::new();
    let sink = Arc::new(CaptureAuditSink::default());

    let make_chain = |c: &a1::DelegationCert| {
        let mut ch = DyoloChain::new(human.verifying_key(), tree.root());
        ch.push(c.clone());
        ch
    };

    let _ = make_chain(&cert)
        .authorize_with_options(
            &agent.verifying_key(),
            &trade,
            &MerkleProof::default(),
            &SystemClock,
            &rev,
            &nonces,
            None,
            sink.as_ref(),
        )
        .expect("first authorization must succeed");

    let _ = make_chain(&cert).authorize_with_options(
        &agent.verifying_key(),
        &trade,
        &MerkleProof::default(),
        &SystemClock,
        &rev,
        &nonces,
        None,
        sink.as_ref(),
    );

    let events = sink.events.lock().unwrap();
    assert_eq!(events.len(), 2, "two audit events must be emitted");
    assert_eq!(events[0].outcome, AuditOutcome::Authorized);
    assert_eq!(events[1].outcome, AuditOutcome::Denied);
}

// ── authorize_with_options: policy gate blocks over-depth chain ───────────────

#[test]
fn authorize_with_options_policy_blocks_over_depth() {
    let human_id = DyoloIdentity::generate();
    let human = SharedIdentity(Arc::new(human_id));
    let now = SystemClock.unix_now();
    let trade = Intent::new("trade").unwrap().hash();
    let scope = IntentTree::build(vec![trade]).unwrap().root();

    let mut chain = DyoloChain::new(human.verifying_key(), scope);
    let mut prev_signer: Box<dyn Signer> = Box::new(human.clone());
    let mut prev_vk = human.verifying_key();

    for _ in 0..3 {
        let next = DyoloIdentity::generate();
        let next_vk = next.verifying_key();
        let cert = CertBuilder::new(next_vk, scope, now, now + 3600)
            .nonce(fresh_nonce())
            .sign(prev_signer.as_ref());
        chain.push(cert);
        prev_vk = next_vk;
        prev_signer = Box::new(next);
    }

    let policy = PolicySet::new().add(DelegationPolicy::new("depth-2").max_chain_depth(2));

    let res = chain.authorize_with_options(
        &prev_vk,
        &trade,
        &MerkleProof::default(),
        &SystemClock,
        &MemoryRevocationStore::new(),
        &MemoryNonceStore::new(),
        Some(&policy),
        &NoopAuditSink,
    );

    assert!(
        res.is_err(),
        "policy gate must block chain depth 3 when max is 2"
    );
}

// ── authorize_with_options: policy passes valid chain ─────────────────────────

#[test]
fn authorize_with_options_policy_passes_valid_chain() {
    let (_human, agent, chain) = base_chain(3600);
    let trade = Intent::new("trade.equity").unwrap().hash();
    let policy = PolicySet::new().add(
        DelegationPolicy::new("generous")
            .max_chain_depth(5)
            .max_ttl_secs(86400)
            .capabilities(CapabilitySet::new().allow("trade.")),
    );

    let res = chain.authorize_with_options(
        &agent.verifying_key(),
        &trade,
        &MerkleProof::default(),
        &SystemClock,
        &MemoryRevocationStore::new(),
        &MemoryNonceStore::new(),
        Some(&policy),
        &NoopAuditSink,
    );
    assert!(res.is_ok(), "valid chain must pass a generous policy");
}

// ── authorize_batch: all-or-nothing atomicity ─────────────────────────────────

#[test]
fn authorize_batch_all_or_nothing_on_bad_intent() {
    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let trade = Intent::new("trade.equity").unwrap().hash();
    let bad = Intent::new("DRAIN_ACCOUNT").unwrap().hash();
    let tree = IntentTree::build(vec![trade]).unwrap();
    let now = SystemClock.unix_now();

    let cert = CertBuilder::new(agent.verifying_key(), tree.root(), now, now + 3600)
        .nonce(fresh_nonce())
        .sign(&human);
    let mut chain = DyoloChain::new(human.verifying_key(), tree.root());
    chain.push(cert.clone());

    let nonces = MemoryNonceStore::new();
    let t_proof = tree.prove(&trade).unwrap();

    let result = chain.authorize_batch(
        &agent.verifying_key(),
        &[(trade, t_proof), (bad, MerkleProof::default())],
        &SystemClock,
        &MemoryRevocationStore::new(),
        &nonces,
    );

    assert!(
        !result.all_authorized,
        "batch must be rejected when any intent fails"
    );
    assert!(
        !nonces.is_consumed(&cert.nonce).unwrap(),
        "nonces must remain unconsumed when batch fails"
    );
}

// ── Nonce: concurrent try_consume — exactly one winner ───────────────────────

#[test]
fn nonce_try_consume_concurrent_exactly_one_winner() {
    use std::thread;
    let store = Arc::new(MemoryNonceStore::new());
    let nonce = fresh_nonce();
    let wins: usize = (0..32)
        .map(|_| {
            let s = Arc::clone(&store);
            thread::spawn(move || s.try_consume(&nonce).unwrap() as usize)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().unwrap())
        .sum();
    assert_eq!(wins, 1, "exactly one thread must win the nonce race");
}

// ── Revocation: batch marks all fingerprints ──────────────────────────────────

#[test]
fn revocation_batch_marks_all_fingerprints() {
    let store = MemoryRevocationStore::new();
    let fps: Vec<[u8; 32]> = (0..8u8)
        .map(|i| {
            let mut f = [0u8; 32];
            f[0] = i;
            f
        })
        .collect();
    store.revoke_batch(&fps).unwrap();
    for fp in &fps {
        assert!(
            store.is_revoked(fp).unwrap(),
            "every batched fingerprint must be revoked"
        );
    }
}

// ── Property-based Testing ────────────────────────────────────────────────────

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn fuzz_intent_creation(action in ".*") {
            let intent = Intent::new(&action);
            if action.is_empty() || action.len() > 256 {
                prop_assert!(intent.is_err());
            } else {
                prop_assert!(intent.is_ok());
            }
        }

        #[test]
        fn fuzz_chain_depth_enforcement(depth in 1..255u8, max_depth in 1..255u8) {
            let human_id = DyoloIdentity::generate();
            let human    = SharedIdentity(Arc::new(human_id));
            let now      = SystemClock.unix_now();
            let trade    = Intent::new("trade.fuzz").unwrap().hash();
            let scope    = IntentTree::build(vec![trade]).unwrap().root();

            let mut chain                        = DyoloChain::new(human.verifying_key(), scope);
            let mut prev_signer: Box<dyn Signer> = Box::new(human.clone());

            for _ in 0..depth {
                let next    = DyoloIdentity::generate();
                let next_vk = next.verifying_key();
                let cert    = CertBuilder::new(next_vk, scope, now, now + 3600)
                    .nonce(fresh_nonce()).max_depth(max_depth)
                    .sign(prev_signer.as_ref());
                chain.push(cert);
                prev_signer = Box::new(next);
            }

            let policy = DelegationPolicy::new("fuzz-depth").max_chain_depth(max_depth);
            let res = policy.check_chain(&chain);

            if depth > max_depth {
                prop_assert!(res.is_err());
            } else {
                prop_assert!(res.is_ok());
            }
        }

        #[test]
        fn fuzz_batch_conflicts(
            valid_intents in proptest::collection::vec("[a-z]{1,10}", 1..10),
            bad_intents in proptest::collection::vec("[A-Z]{1,10}", 1..10)
        ) {
            let human = DyoloIdentity::generate();
            let agent = DyoloIdentity::generate();
            let now   = SystemClock.unix_now();

            let mut valid_hashes = Vec::new();
            for act in &valid_intents {
                valid_hashes.push(Intent::new(act).unwrap().hash());
            }

            let tree = IntentTree::build(valid_hashes.clone()).unwrap();

            // Maximal nonce test
            let cert = CertBuilder::new(agent.verifying_key(), tree.root(), now, now + 3600)
                .nonce([0xFF; 16]).sign(&human);

            let mut chain = DyoloChain::new(human.verifying_key(), tree.root());
            chain.push(cert);

            let mut batch = Vec::new();
            for h in &valid_hashes {
                batch.push((*h, tree.prove(h).unwrap()));
            }
            for act in &bad_intents {
                batch.push((Intent::new(act).unwrap().hash(), MerkleProof::default()));
            }

            let nonces = MemoryNonceStore::new();
            let result = chain.authorize_batch(
                &agent.verifying_key(),
                &batch,
                &SystemClock,
                &MemoryRevocationStore::new(),
                &nonces,
            );

            prop_assert!(!result.all_authorized);
        }
    }
}

// ── Passport integration tests (v2.1) ─────────────────────────────────────────

#[cfg(test)]
mod passport_tests {
    use a1::{
        DyoloIdentity, DyoloPassport, Intent, NarrowingMatrix, SystemClock,
    };

    fn make_passport(namespace: &str, caps: &[&str]) -> (DyoloIdentity, DyoloPassport) {
        let root = DyoloIdentity::generate();
        let clock = SystemClock;
        let passport = DyoloPassport::issue(namespace, caps, 3600, &root, &clock).unwrap();
        (root, passport)
    }

    #[test]
    fn passport_issue_and_scope_root() {
        let (_root, passport) = make_passport("acme-bot", &["trade.equity", "portfolio.read"]);
        assert_eq!(passport.namespace, "acme-bot");
        assert!(!passport.capability_mask.is_empty());
        let scope = passport.scope_root().unwrap();
        assert_ne!(scope, [0u8; 32]);
    }

    #[test]
    fn passport_new_chain_principal_matches() {
        let (_root, passport) = make_passport("test-agent", &["trade.equity"]);
        let chain = passport.new_chain().unwrap();
        assert_eq!(chain.principal_pk, passport.verifying_key());
        assert_eq!(chain.principal_scope, passport.scope_root().unwrap());
    }

    #[test]
    fn passport_issue_sub_subset_succeeds() {
        let (root, passport) = make_passport("bot", &["trade.equity", "portfolio.read"]);
        let agent = DyoloIdentity::generate();
        let clock = SystemClock;
        let sub = passport.issue_sub(agent.verifying_key(), &["trade.equity"], 1800, &root, &clock);
        assert!(sub.is_ok(), "valid subset should be accepted");
    }

    #[test]
    fn passport_issue_sub_escalation_rejected() {
        let (root, passport) = make_passport("bot", &["portfolio.read"]);
        let agent = DyoloIdentity::generate();
        let clock = SystemClock;
        let result =
            passport.issue_sub(agent.verifying_key(), &["trade.equity"], 1800, &root, &clock);
        assert!(result.is_err(), "capability escalation must be rejected");
    }

    #[test]
    fn guard_local_single_capability_end_to_end() {
        let (root, passport) = make_passport("trading-bot", &["trade.equity", "portfolio.read"]);
        let agent = DyoloIdentity::generate();
        let clock = SystemClock;

        let sub = passport
            .issue_sub(agent.verifying_key(), &["trade.equity"], 1800, &root, &clock)
            .unwrap();

        let mut chain = passport.new_chain().unwrap();
        chain.push(sub);

        let intent = Intent::new("trade.equity").unwrap();
        let receipt = passport.guard_local(&chain, &agent.verifying_key(), &intent).unwrap();

        assert_eq!(receipt.passport_namespace, "trading-bot");
        assert!(receipt.verify_commitment(), "commitment must be valid");
        assert_eq!(receipt.inner.chain_depth, 1);
    }

    #[test]
    fn guard_local_rejects_out_of_scope() {
        let (root, passport) = make_passport("read-only-bot", &["portfolio.read"]);
        let agent = DyoloIdentity::generate();
        let clock = SystemClock;

        let sub = passport
            .issue_sub(agent.verifying_key(), &["portfolio.read"], 1800, &root, &clock)
            .unwrap();

        let mut chain = passport.new_chain().unwrap();
        chain.push(sub);

        let intent = Intent::new("trade.equity").unwrap();
        assert!(
            passport.guard_local(&chain, &agent.verifying_key(), &intent).is_err(),
            "out-of-scope intent must be rejected"
        );
    }

    #[test]
    fn narrowing_matrix_subset_semantics() {
        let parent = NarrowingMatrix::from_capabilities(&["trade.equity", "portfolio.read"]);
        let child = NarrowingMatrix::from_capabilities(&["trade.equity"]);
        let empty = NarrowingMatrix::EMPTY;
        let full = NarrowingMatrix::FULL;

        assert!(child.is_subset_of(&parent));
        assert!(!parent.is_subset_of(&child));
        assert!(empty.is_subset_of(&parent));
        assert!(parent.is_subset_of(&full));
        assert!(child.enforce_narrowing(&parent).is_ok());
        assert!(parent.enforce_narrowing(&child).is_err());
    }

    #[test]
    fn narrowing_matrix_commitment_stable_and_unique() {
        let a = NarrowingMatrix::from_capabilities(&["trade.equity"]);
        let b = NarrowingMatrix::from_capabilities(&["portfolio.write"]);
        assert_eq!(a.commitment(), a.commitment());
        assert_ne!(a.commitment(), b.commitment());
    }

    #[test]
    fn narrowing_matrix_roundtrip_hex() {
        let m = NarrowingMatrix::from_capabilities(&["trade.equity", "audit.read"]);
        let hex = m.to_hex();
        let m2 = NarrowingMatrix::from_hex(&hex).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn narrowing_matrix_from_csv() {
        let csv = NarrowingMatrix::from_csv("trade.equity , portfolio.read, audit.read");
        let slice = NarrowingMatrix::from_capabilities(&["trade.equity", "portfolio.read", "audit.read"]);
        assert_eq!(csv, slice);
    }

    #[test]
    fn provable_receipt_commitment_valid() {
        let (root, passport) = make_passport("ns", &["trade.equity"]);
        let agent = DyoloIdentity::generate();
        let clock = SystemClock;

        let sub = passport
            .issue_sub(agent.verifying_key(), &["trade.equity"], 900, &root, &clock)
            .unwrap();
        let mut chain = passport.new_chain().unwrap();
        chain.push(sub);

        let intent = Intent::new("trade.equity").unwrap();
        let receipt = passport.guard_local(&chain, &agent.verifying_key(), &intent).unwrap();

        assert!(receipt.verify_commitment());
        let mask = NarrowingMatrix::from_hex(&receipt.capability_mask_hex).unwrap();
        assert_eq!(mask.commitment(), receipt.narrowing_commitment);
    }

    #[test]
    fn passport_issue_from_csv_matches_slice() {
        let root = DyoloIdentity::generate();
        let clock = SystemClock;
        let a = DyoloPassport::issue(
            "ns",
            &["trade.equity", "portfolio.read"],
            3600,
            &root,
            &clock,
        )
        .unwrap();
        let b =
            DyoloPassport::issue_from_csv("ns", "trade.equity, portfolio.read", 3600, &root, &clock)
                .unwrap();
        assert_eq!(a.capability_mask, b.capability_mask);
    }

    #[test]
    fn two_hop_delegation_chain() {
        let (root, passport) =
            make_passport("enterprise-agent", &["trade.equity", "portfolio.read"]);
        let agent = DyoloIdentity::generate();
        let clock = SystemClock;

        let sub = passport
            .issue_sub(agent.verifying_key(), &["trade.equity"], 1800, &root, &clock)
            .unwrap();

        let mut chain = passport.new_chain().unwrap();
        chain.push(sub);

        let intent = Intent::new("trade.equity").unwrap();
        let receipt = passport
            .guard_local(&chain, &agent.verifying_key(), &intent)
            .unwrap();
        assert_eq!(receipt.inner.chain_depth, 1);
        assert!(receipt.verify_commitment());
    }
}
