use a1::{DyoloIdentity, DyoloPassport, Intent, NarrowingMatrix, SystemClock};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn trading_passport() -> (DyoloIdentity, DyoloPassport) {
    let root = DyoloIdentity::generate();
    let clock = SystemClock;
    let passport = DyoloPassport::issue(
        "acme-trading-bot",
        &["trade.equity", "portfolio.read"],
        30 * 24 * 3600,
        &root,
        &clock,
    )
    .unwrap();
    (root, passport)
}

// ── Passport lifecycle ────────────────────────────────────────────────────────

#[test]
fn passport_issue_stores_correct_namespace_and_capabilities() {
    let (_root, passport) = trading_passport();
    assert_eq!(passport.namespace, "acme-trading-bot");
    assert!(!passport.capability_mask.is_empty());
    let trade_mask = NarrowingMatrix::from_capabilities(&["trade.equity"]);
    assert!(trade_mask.is_subset_of(&passport.capability_mask));
}

#[test]
fn passport_scope_root_is_deterministic() {
    let (_root, passport) = trading_passport();
    let r1 = passport.scope_root().unwrap();
    let r2 = passport.scope_root().unwrap();
    assert_eq!(
        r1, r2,
        "scope_root must be deterministic for the same capability set"
    );
}

#[test]
fn passport_new_chain_anchors_at_holder_key_and_scope() {
    let (_root, passport) = trading_passport();
    let chain = passport.new_chain().unwrap();
    assert_eq!(chain.principal_pk, passport.verifying_key());
    assert_eq!(chain.principal_scope, passport.scope_root().unwrap());
    assert_eq!(chain.namespace.as_deref(), Some("acme-trading-bot"));
}

#[test]
fn passport_guard_local_single_capability_end_to_end() {
    let (root, passport) = trading_passport();
    let agent = DyoloIdentity::generate();
    let clock = SystemClock;

    let sub = passport
        .issue_sub(
            agent.verifying_key(),
            &["trade.equity"],
            3600,
            &root,
            &clock,
        )
        .unwrap();

    let mut chain = passport.new_chain().unwrap();
    chain.push(sub);

    let intent = Intent::new("trade.equity").unwrap();
    let receipt = passport
        .guard_local(&chain, &agent.verifying_key(), &intent)
        .unwrap();

    assert_eq!(receipt.passport_namespace, "acme-trading-bot");
    assert!(
        receipt.verify_commitment(),
        "ProvableReceipt commitment must be self-consistent"
    );
    assert_eq!(receipt.inner.chain_depth, 1);
}

#[test]
fn passport_guard_rejects_out_of_scope_intent() {
    let (root, passport) = trading_passport();
    let agent = DyoloIdentity::generate();
    let clock = SystemClock;

    let sub = passport
        .issue_sub(
            agent.verifying_key(),
            &["portfolio.read"],
            3600,
            &root,
            &clock,
        )
        .unwrap();

    let mut chain = passport.new_chain().unwrap();
    chain.push(sub);

    let intent = Intent::new("trade.equity").unwrap();
    let result = passport.guard_local(&chain, &agent.verifying_key(), &intent);
    assert!(
        result.is_err(),
        "guard_local must reject intent not covered by the sub-cert scope"
    );
}

#[test]
fn passport_issue_sub_rejects_capability_escalation() {
    let (root, passport) = trading_passport();
    let agent = DyoloIdentity::generate();
    let clock = SystemClock;

    let result = passport.issue_sub(
        agent.verifying_key(),
        &["trade.equity", "admin.delete"],
        3600,
        &root,
        &clock,
    );
    assert!(
        result.is_err(),
        "issue_sub must reject capabilities not held by the passport"
    );
}

#[test]
fn passport_issue_sub_accepts_exact_capability_set() {
    let (root, passport) = trading_passport();
    let agent = DyoloIdentity::generate();
    let clock = SystemClock;

    let result = passport.issue_sub(
        agent.verifying_key(),
        &["trade.equity", "portfolio.read"],
        3600,
        &root,
        &clock,
    );
    assert!(
        result.is_ok(),
        "issue_sub must accept the exact passport capability set"
    );
}

#[test]
fn passport_narrowing_matrix_subset_algebra() {
    let parent = NarrowingMatrix::from_capabilities(&["a", "b", "c"]);
    let child = NarrowingMatrix::from_capabilities(&["a"]);
    let unrelated = NarrowingMatrix::from_capabilities(&["z"]);

    assert!(child.is_subset_of(&parent));
    assert!(!parent.is_subset_of(&child));
    assert!(!unrelated.is_subset_of(&parent));
    assert!(NarrowingMatrix::EMPTY.is_subset_of(&parent));
    assert!(!parent.is_subset_of(&NarrowingMatrix::EMPTY));
}

#[test]
fn passport_provable_receipt_commitment_tamper_detection() {
    let (root, passport) = trading_passport();
    let agent = DyoloIdentity::generate();
    let clock = SystemClock;

    let sub = passport
        .issue_sub(
            agent.verifying_key(),
            &["trade.equity"],
            3600,
            &root,
            &clock,
        )
        .unwrap();
    let mut chain = passport.new_chain().unwrap();
    chain.push(sub);

    let intent = Intent::new("trade.equity").unwrap();
    let mut receipt = passport
        .guard_local(&chain, &agent.verifying_key(), &intent)
        .unwrap();

    // Tamper with the capability_mask_hex
    receipt.capability_mask_hex = NarrowingMatrix::FULL.to_hex();
    assert!(
        !receipt.verify_commitment(),
        "tampered capability_mask_hex must fail commitment verification"
    );
}

#[test]
fn passport_issue_from_csv_matches_slice_issue() {
    let root = DyoloIdentity::generate();
    let clock = SystemClock;
    let a = DyoloPassport::issue(
        "bot",
        &["trade.equity", "portfolio.read"],
        3600,
        &root,
        &clock,
    )
    .unwrap();
    let b =
        DyoloPassport::issue_from_csv("bot", "trade.equity, portfolio.read", 3600, &root, &clock)
            .unwrap();

    assert_eq!(
        a.capability_mask, b.capability_mask,
        "CSV and slice issue must produce identical masks"
    );
}

#[test]
fn passport_sub_from_csv_matches_sub_slice() {
    let (root, passport) = trading_passport();
    let agent = DyoloIdentity::generate();
    let clock = SystemClock;

    let a = passport
        .issue_sub(
            agent.verifying_key(),
            &["trade.equity"],
            3600,
            &root,
            &clock,
        )
        .unwrap();
    let b = passport
        .issue_sub_from_csv(agent.verifying_key(), "trade.equity", 3600, &root, &clock)
        .unwrap();

    assert_eq!(
        a.scope_root, b.scope_root,
        "CSV and slice sub must produce the same scope_root"
    );
}