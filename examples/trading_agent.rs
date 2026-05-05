use dyolo_kya::{
    CertBuilder, DyoloChain, DyoloIdentity, Intent, IntentTree, MerkleProof,
    MemoryNonceStore, MemoryRevocationStore, RevocationStore, SubScopeProof, SystemClock,
};

fn main() {
    println!("dyolo-kya v1 — Know Your Agent Protocol\n");

    let human        = DyoloIdentity::generate();
    let orchestrator = DyoloIdentity::generate();
    let executor     = DyoloIdentity::generate();

    println!("Principal   : {human:?}");
    println!("Orchestrator: {orchestrator:?}");
    println!("Executor    : {executor:?}\n");

    let trade = Intent::new("trade.equity").unwrap()
        .param("symbol", "AAPL")
        .param("side", "buy")
        .param("limit_usd", "182.50")
        .param("qty", "100")
        .hash();

    let query = Intent::new("query.portfolio").unwrap().hash();
    let drain = Intent::new("admin.drain-account").unwrap().hash();

    let principal_tree  = IntentTree::build(vec![trade, query, drain]).unwrap();
    let principal_scope = principal_tree.root();

    println!("Principal scope root: {}", hex::encode(principal_scope));

    let now    = SystemClock.unix_now();
    let expiry = now + 3_600;

    let cert_orch = CertBuilder::new(orchestrator.verifying_key(), principal_scope, now, expiry)
        .sign(&human);

    println!("\nPrincipal delegated full scope to Orchestrator.");
    println!("  cert fingerprint: {}", hex::encode(cert_orch.fingerprint()));

    let sub_proof = SubScopeProof::build(&principal_tree, &[trade, query]).unwrap();
    let sub_tree  = IntentTree::build(vec![trade, query]).unwrap();
    let sub_scope = sub_tree.root();

    let cert_exec = CertBuilder::new(executor.verifying_key(), sub_scope, now, expiry)
        .scope_proof(sub_proof)
        .max_depth(0)
        .sign(&orchestrator);

    println!("\nOrchestrator narrowed scope (trade + query) to Executor.");
    println!("  sub-scope root  : {}", hex::encode(sub_scope));
    println!("  cert fingerprint: {}", hex::encode(cert_exec.fingerprint()));

    let mut chain = DyoloChain::new(human.verifying_key(), principal_scope);
    chain.push(cert_orch).push(cert_exec);

    println!("\nChain fingerprint: {}", hex::encode(chain.fingerprint()));

    let revocation = MemoryRevocationStore::new();
    let nonces     = MemoryNonceStore::new();
    let clock      = SystemClock;

    println!("\n── Scenario 1: Authorized trade ──");

    let trade_proof = sub_tree.prove(&trade).unwrap();

    match chain.authorize(
        &executor.verifying_key(),
        &trade,
        &trade_proof,
        &clock,
        &revocation,
        &nonces,
    ) {
        Ok(action) => {
            let r = action.receipt();
            println!("[AUTHORIZED]");
            println!("  chain_depth       : {}", r.chain_depth);
            println!("  verified_scope    : {}", hex::encode(r.verified_scope_root));
            println!("  intent            : {}", hex::encode(r.intent));
            println!("  chain_fingerprint : {}", hex::encode(r.chain_fingerprint));
        }
        Err(e) => println!("[REJECTED] {e}"),
    }

    println!("\n── Scenario 2: Escalation attempt (drain) ──");

    let drain_proof = principal_tree.prove(&drain).unwrap();
    let chain2      = chain.clone();

    match chain2.authorize(
        &executor.verifying_key(),
        &drain,
        &drain_proof,
        &clock,
        &revocation,
        &nonces,
    ) {
        Ok(_)  => println!("[UNEXPECTED PASS] Scope enforcement failed"),
        Err(e) => println!("[CORRECTLY REJECTED] {e}"),
    }

    println!("\n── Scenario 3: Replay attack ──");

    let chain3 = chain.clone();

    match chain3.authorize(
        &executor.verifying_key(),
        &trade,
        &trade_proof,
        &clock,
        &revocation,
        &nonces,
    ) {
        Ok(_)  => println!("[UNEXPECTED PASS] Replay prevention failed"),
        Err(e) => println!("[CORRECTLY REJECTED] {e}"),
    }

    println!("\n── Scenario 4: Revocation ──");

    let revocation4 = MemoryRevocationStore::new();
    let nonces4     = MemoryNonceStore::new();

    let cert_orch2 = CertBuilder::new(orchestrator.verifying_key(), principal_scope, now, expiry)
        .sign(&human);
    let cert_exec2 = CertBuilder::new(executor.verifying_key(), sub_scope, now, expiry)
        .scope_proof(SubScopeProof::build(&principal_tree, &[trade, query]).unwrap())
        .max_depth(0)
        .sign(&orchestrator);

    let fp = cert_orch2.fingerprint();
    revocation4.revoke(&fp).expect("storage failure during revocation");
    println!("Revoked cert: {}", hex::encode(fp));

    let mut chain4 = DyoloChain::new(human.verifying_key(), principal_scope);
    chain4.push(cert_orch2).push(cert_exec2);

    match chain4.authorize(
        &executor.verifying_key(),
        &trade,
        &MerkleProof::default(),
        &clock,
        &revocation4,
        &nonces4,
    ) {
        Ok(_)  => println!("[UNEXPECTED PASS] Revocation failed"),
        Err(e) => println!("[CORRECTLY REJECTED] {e}"),
    }

    println!("\nProtocol demonstration complete.");
}
