use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dyolo_kya::{
    CertBuilder, DyoloChain, DyoloIdentity, Intent, IntentTree,
    MemoryNonceStore, MemoryRevocationStore, MerkleProof,
    RevocationStore, SystemClock, SubScopeProof, fresh_nonce,
};

fn bench_single_hop(c: &mut Criterion) {
    let human = DyoloIdentity::generate();
    let agent = DyoloIdentity::generate();
    let trade = Intent::new("bench").unwrap().hash();
    let tree  = IntentTree::build(vec![trade]).unwrap();
    let root  = tree.root();
    let now   = SystemClock.unix_now();

    c.bench_function("single-hop chain authorization", |b| {
        b.iter(|| {
            let cert = CertBuilder::new(agent.verifying_key(), root, now, now + 3600)
                .nonce(fresh_nonce())
                .sign(&human);
            let mut chain = DyoloChain::new(human.verifying_key(), root);
            chain.push(cert);

            let rev    = MemoryRevocationStore::new();
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
    let human  = DyoloIdentity::generate();
    let orch   = DyoloIdentity::generate();
    let exec   = DyoloIdentity::generate();

    let trade = Intent::new("trade.bench").unwrap().hash();
    let query = Intent::new("query.bench").unwrap().hash();
    let tree  = IntentTree::build(vec![trade, query]).unwrap();
    let root  = tree.root();
    let now   = SystemClock.unix_now();

    let sub_proof = SubScopeProof::build(&tree, &[trade]).unwrap();
    let sub_root  = IntentTree::build(vec![trade]).unwrap().root();

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

            let rev    = MemoryRevocationStore::new();
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
    let fp  = [0u8; 32];

    c.bench_function("revocation fast-path check", |b| {
        b.iter(|| {
            let _ = black_box(rev.is_revoked(black_box(&fp)).unwrap());
        })
    });
}

fn setup_batch(size: usize) -> (DyoloIdentity, DyoloChain, Vec<(dyolo_kya::IntentHash, MerkleProof)>) {
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

criterion_group!(benches, bench_single_hop, bench_multi_hop, bench_revocation_check, bench_authorize_batch_256, bench_authorize_batch_1024);
criterion_main!(benches);
