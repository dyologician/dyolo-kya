use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use a1::{
    fresh_nonce, CertBuilder, Clock, DyoloChain, DyoloIdentity, DyoloPassport, Intent, IntentTree,
    MemoryNonceStore, MemoryRevocationStore, MerkleProof, NarrowingMatrix, RevocationStore,
    SubScopeProof, SystemClock,
};

fn bench_single_hop(c: &mut Criterion) {
    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let trade = Intent::new("bench").unwrap().hash();
    let tree = IntentTree::build(vec![trade]).unwrap();
    let root = tree.root();
    let now = SystemClock.unix_now();

    c.bench_function("single-hop chain authorization", |b| {
        b.iter(|| {
            let cert = CertBuilder::new(agent.verifying_key(), root, now, now + 3600)
                .nonce(fresh_nonce())
                .sign(&human);
            let mut chain = DyoloChain::new(human.verifying_key(), root);
            chain.push(cert);

            let rev = MemoryRevocationStore::new();
            let nonces = MemoryNonceStore::new();

            let _ = black_box(chain.authorize(
                black_box(&agent.verifying_key()),
                black_box(&trade),
                black_box(&MerkleProof::default()),
                black_box(&SystemClock),
                black_box(&rev),
                black_box(&nonces),
            ));
        })
    });
}

fn bench_multi_hop(c: &mut Criterion) {
    let human = DyoloIdentity::generate();
    let orch = DyoloIdentity::generate();
    let exec = DyoloIdentity::generate();

    let trade = Intent::new("trade.bench").unwrap().hash();
    let query = Intent::new("query.bench").unwrap().hash();
    let tree = IntentTree::build(vec![trade, query]).unwrap();
    let root = tree.root();
    let now = SystemClock.unix_now();

    let sub_proof = SubScopeProof::build(&tree, &[trade]).unwrap();
    let sub_root = IntentTree::build(vec![trade]).unwrap().root();

    c.bench_function("two-hop scoped chain authorization", |b| {
        b.iter(|| {
            let cert_orch = CertBuilder::new(orch.verifying_key(), root, now, now + 3600)
                .nonce(fresh_nonce())
                .sign(&human);
            let cert_exec = CertBuilder::new(exec.verifying_key(), sub_root, now, now + 3600)
                .scope_proof(sub_proof.clone())
                .max_depth(0)
                .nonce(fresh_nonce())
                .sign(&orch);

            let mut chain = DyoloChain::new(human.verifying_key(), root);
            chain.push(cert_orch).push(cert_exec);

            let rev = MemoryRevocationStore::new();
            let nonces = MemoryNonceStore::new();

            let _ = black_box(chain.authorize(
                black_box(&exec.verifying_key()),
                black_box(&trade),
                black_box(&MerkleProof::default()),
                black_box(&SystemClock),
                black_box(&rev),
                black_box(&nonces),
            ));
        })
    });
}

fn bench_revocation_check(c: &mut Criterion) {
    let rev = MemoryRevocationStore::new();
    let fp = [0u8; 32];

    c.bench_function("revocation fast-path check", |b| {
        b.iter(|| {
            let _ = black_box(rev.is_revoked(black_box(&fp)).unwrap());
        })
    });
}

fn setup_batch(
    size: usize,
) -> (
    DyoloIdentity,
    DyoloChain,
    Vec<(a1::IntentHash, MerkleProof)>,
) {
    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();

    let mut hashes = Vec::with_capacity(size);
    for i in 0..size {
        hashes.push(Intent::new(format!("trade.{}", i)).unwrap().hash());
    }

    let tree = IntentTree::build(hashes.clone()).unwrap();
    let root = tree.root();
    let now = SystemClock.unix_now();

    let cert = CertBuilder::new(agent.verifying_key(), root, now, now + 3600)
        .nonce(fresh_nonce())
        .sign(&human);

    let mut chain = DyoloChain::new(human.verifying_key(), root);
    chain.push(cert);

    let mut batch = Vec::with_capacity(size);
    for h in &hashes {
        batch.push((*h, tree.prove(h).unwrap()));
    }

    (agent, chain, batch)
}

fn bench_authorize_batch_256(c: &mut Criterion) {
    let (agent, chain, batch) = setup_batch(256);
    c.bench_function("authorize_batch_256", |b| {
        b.iter(|| {
            let rev = MemoryRevocationStore::new();
            let nonces = MemoryNonceStore::new();
            let _ = black_box(chain.authorize_batch(
                black_box(&agent.verifying_key()),
                black_box(&batch),
                black_box(&SystemClock),
                black_box(&rev),
                black_box(&nonces),
            ));
        })
    });
}

fn bench_authorize_batch_1024(c: &mut Criterion) {
    let (agent, chain, batch) = setup_batch(1024);
    c.bench_function("authorize_batch_1024", |b| {
        b.iter(|| {
            let rev = MemoryRevocationStore::new();
            let nonces = MemoryNonceStore::new();
            let _ = black_box(chain.authorize_batch(
                black_box(&agent.verifying_key()),
                black_box(&batch),
                black_box(&SystemClock),
                black_box(&rev),
                black_box(&nonces),
            ));
        })
    });
}

// ── NarrowingMatrix benchmarks ────────────────────────────────────────────────

fn bench_narrowing_single_cap(c: &mut Criterion) {
    let parent = NarrowingMatrix::from_capabilities(&[
        "trade.equity", "trade.options", "portfolio.read", "portfolio.write",
        "audit.read", "audit.export", "risk.compute", "settlement.initiate",
    ]);
    let child = NarrowingMatrix::from_capabilities(&["trade.equity"]);

    c.bench_function("NarrowingMatrix::is_subset_of (single cap vs 8-cap parent)", |b| {
        b.iter(|| {
            let _ = black_box(child.is_subset_of(black_box(&parent)));
        })
    });
}

fn bench_narrowing_enforce(c: &mut Criterion) {
    let parent = NarrowingMatrix::from_capabilities(&[
        "trade.equity", "trade.options", "portfolio.read", "portfolio.write",
        "audit.read", "audit.export", "risk.compute", "settlement.initiate",
    ]);
    let child = NarrowingMatrix::from_capabilities(&["trade.equity", "portfolio.read"]);

    c.bench_function("NarrowingMatrix::enforce_narrowing (2-cap subset of 8-cap)", |b| {
        b.iter(|| {
            let _ = black_box(child.enforce_narrowing(black_box(&parent)));
        })
    });
}

fn bench_narrowing_from_capabilities(c: &mut Criterion) {
    let mut group = c.benchmark_group("NarrowingMatrix::from_capabilities");
    for n in [1usize, 4, 16, 64, 256] {
        let caps: Vec<String> = (0..n).map(|i| format!("cap.action.{}", i)).collect();
        let cap_refs: Vec<&str> = caps.iter().map(String::as_str).collect();
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let _ = black_box(NarrowingMatrix::from_capabilities(black_box(&cap_refs)));
            })
        });
    }
    group.finish();
}

fn bench_narrowing_commitment(c: &mut Criterion) {
    let mask = NarrowingMatrix::from_capabilities(&[
        "trade.equity", "portfolio.read", "audit.read",
    ]);

    c.bench_function("NarrowingMatrix::commitment (Blake3 over 32-byte mask)", |b| {
        b.iter(|| {
            let _ = black_box(mask.commitment());
        })
    });
}

fn bench_narrowing_intersect(c: &mut Criterion) {
    let a = NarrowingMatrix::from_capabilities(&[
        "trade.equity", "portfolio.read", "audit.read", "risk.compute",
    ]);
    let b_mask = NarrowingMatrix::from_capabilities(&[
        "trade.equity", "settlement.initiate", "audit.export",
    ]);

    c.bench_function("NarrowingMatrix::intersect (4-bit & 3-bit → common)", |b| {
        b.iter(|| {
            let _ = black_box(a.intersect(black_box(&b_mask)));
        })
    });
}

// ── DyoloPassport benchmarks ──────────────────────────────────────────────────

fn bench_passport_issue(c: &mut Criterion) {
    let root = DyoloIdentity::generate();
    let clock = SystemClock;
    let caps = ["trade.equity", "portfolio.read", "audit.read", "risk.compute"];

    c.bench_function("DyoloPassport::issue (4 capabilities)", |b| {
        b.iter(|| {
            let _ = black_box(
                DyoloPassport::issue(
                    black_box("bench-agent"),
                    black_box(&caps),
                    black_box(86400),
                    black_box(&root),
                    black_box(&clock),
                )
                .unwrap(),
            );
        })
    });
}

fn bench_passport_issue_sub(c: &mut Criterion) {
    let root = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let clock = SystemClock;
    let passport = DyoloPassport::issue(
        "bench-agent",
        &["trade.equity", "portfolio.read", "audit.read", "risk.compute"],
        86400,
        &root,
        &clock,
    )
    .unwrap();

    c.bench_function("DyoloPassport::issue_sub (1-cap subset issuance)", |b| {
        b.iter(|| {
            let _ = black_box(
                passport
                    .issue_sub(
                        black_box(agent.verifying_key()),
                        black_box(&["trade.equity"]),
                        black_box(3600),
                        black_box(&root),
                        black_box(&clock),
                    )
                    .unwrap(),
            );
        })
    });
}

fn bench_passport_guard_local(c: &mut Criterion) {
    let root = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let clock = SystemClock;
    let passport = DyoloPassport::issue(
        "bench-agent",
        &["trade.equity", "portfolio.read"],
        86400,
        &root,
        &clock,
    )
    .unwrap();
    let sub = passport
        .issue_sub(agent.verifying_key(), &["trade.equity"], 3600, &root, &clock)
        .unwrap();
    let mut chain = passport.new_chain().unwrap();
    chain.push(sub);
    let intent = Intent::new("trade.equity").unwrap();

    c.bench_function("DyoloPassport::guard_local (end-to-end authorization)", |b| {
        b.iter(|| {
            let _ = black_box(
                passport
                    .guard_local(
                        black_box(&chain),
                        black_box(&agent.verifying_key()),
                        black_box(&intent),
                    )
                    .unwrap(),
            );
        })
    });
}

criterion_group!(
    benches,
    bench_single_hop,
    bench_multi_hop,
    bench_revocation_check,
    bench_authorize_batch_256,
    bench_authorize_batch_1024,
    bench_narrowing_single_cap,
    bench_narrowing_enforce,
    bench_narrowing_from_capabilities,
    bench_narrowing_commitment,
    bench_narrowing_intersect,
    bench_passport_issue,
    bench_passport_issue_sub,
    bench_passport_guard_local,
);
criterion_main!(benches);