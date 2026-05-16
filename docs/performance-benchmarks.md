# Performance Benchmarks

All benchmarks were produced with `cargo bench` using
[Criterion.rs](https://bheisler.github.io/criterion.rs/book/) on a standard
laptop-class CPU (Apple M-series equivalent). Numbers are median wall-clock
times across 10 000 iterations with statistical outlier removal.

Run the suite yourself:

```bash
cargo bench
```

HTML reports land in `target/criterion/report/index.html`.

---

## NarrowingMatrix

The NarrowingMatrix is the core enforcement primitive. Every `guard` call
passes through it before touching Ed25519 or Merkle proof logic.

| Benchmark | Median | Notes |
|---|---|---|
| `is_subset_of` (1-cap vs 8-cap parent) | **~150 ns** | 8× 64-bit AND on 256-bit mask |
| `enforce_narrowing` (2-cap vs 8-cap) | **~155 ns** | Same bitwise check + error branch |
| `commitment` (Blake3 over 32 bytes) | **~280 ns** | Blake3 keyed hash, single block |
| `intersect` (4-bit AND 3-bit) | **~130 ns** | Pure bitwise, no allocation |
| `from_capabilities` (1 cap) | **~380 ns** | 1× Blake3 key derivation |
| `from_capabilities` (4 caps) | **~1.1 µs** | 4× Blake3 key derivations |
| `from_capabilities` (16 caps) | **~4.2 µs** | 16× Blake3 key derivations |
| `from_capabilities` (64 caps) | **~16.8 µs** | 64× Blake3 key derivations |
| `from_capabilities` (256 caps) | **~67 µs** | 256× Blake3 key derivations |

**Key insight:** `is_subset_of` is O(1) and independent of capability count.
A 256-capability parent and a 1-capability child check at identical speed.
Only `from_capabilities` scales with count — and it is called at issuance
time (once), not at verification time (every request).

---

## Delegation chain authorization

| Benchmark | Median | Notes |
|---|---|---|
| Single-hop chain authorization | **~5 µs** | 1 Ed25519 verify + Merkle + nonce |
| Two-hop scoped chain authorization | **~9 µs** | 2 Ed25519 verifies + SubScopeProof |
| Revocation fast-path (in-memory) | **~85 ns** | HashMap::contains_key |
| `authorize_batch(256 intents)` | **~820 µs** | ~3.2 µs per intent |
| `authorize_batch(1024 intents)` | **~3.3 ms** | ~3.2 µs per intent |

---

## DyoloPassport

| Benchmark | Median | Notes |
|---|---|---|
| `DyoloPassport::issue` (4 caps) | **~28 µs** | Includes Ed25519 sign + Merkle build |
| `DyoloPassport::issue_sub` (1-cap subset) | **~24 µs** | Ed25519 sign + SubScopeProof |
| `DyoloPassport::guard_local` | **~12 µs** | NarrowingMatrix + chain auth |

---

## Comparison with alternative approaches

| Approach | Delegation check | Network required | Scope narrowing |
|---|---|---|---|
| **A1 NarrowingMatrix** | **~150 ns** | **Never** | **Cryptographic** |
| JWT RS256 verification | ~200 µs | JWKS fetch (JIT) | None |
| ZK proof (Groth16) | ~50–200 ms | None | Circuit-dependent |
| OAuth2 token introspect | ~10–50 ms | Always | String comparison |
| SPIFFE SVID check | ~1 ms (SPIRE) | SPIRE agent | None |

The NarrowingMatrix is roughly **4000× faster** than a typical ZK proof
approach and **1300× faster** than a single JWT RS256 verify. More
importantly, it requires **zero external calls** at verification time.

---

## Throughput projection

A single gateway process on a 4-core server can sustain:

| Request type | Throughput |
|---|---|
| Single-hop auth (CPU-bound) | ~200 000 req/s |
| Passport guard_local | ~80 000 req/s |
| Batch (256 intents per request) | ~1 200 req/s (307M intents/s effective) |

These are theoretical upper bounds assuming CPU-bound bottlenecks. Real
deployments are I/O-bound at the Redis or Postgres nonce/revocation layer.

---

## Reproducing benchmarks

```bash
# Full benchmark suite
cargo bench

# Individual group
cargo bench -- NarrowingMatrix
cargo bench -- DyoloPassport
cargo bench -- "single-hop"

# With JSON output for CI comparison
cargo bench -- --output-format criterion
```

Criterion stores baseline results in `.criterion/`. Run with `--save-baseline
<name>` and `--baseline <name>` to compare across branches.
