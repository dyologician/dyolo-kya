# How A1 Compares

This page answers the question every architect asks: "Why not just use JWTs, or SPIFFE, or OAuth scopes?"

Short answer: those tools solve identity. A1 solves **recursive delegation with provable scope narrowing across an unbounded agent tree**. That is a different problem.

---

## Feature comparison

| | A1 | JWT / OIDC | SPIFFE / SPIRE | OAuth2 scopes |
|---|---|---|---|---|
| Multi-hop agent delegation | ✅ Native | ❌ Manual chain | ❌ Flat | ❌ Flat |
| O(1) scope narrowing | ✅ NarrowingMatrix | ❌ String comparison | ❌ SVID has no scopes | ❌ Set intersection |
| Enforced subset delegation | ✅ Cryptographic | ❌ Convention only | ❌ None | ❌ Convention only |
| Works air-gapped | ✅ Always | ⚠️ Requires IDP | ⚠️ Requires SPIRE | ❌ Requires AS |
| Verifiable chain of custody | ✅ Every hop | ❌ Single issuer | ❌ Single workload | ❌ Single grant |
| Tamper-evident audit receipt | ✅ Blake3 commitment | ❌ None | ❌ None | ❌ None |
| Human-to-agent traceability | ✅ Irrefutable | ⚠️ If you build it | ❌ None | ⚠️ If you build it |
| Key revocation | ✅ RevocationStore | ❌ Short TTL only | ✅ SVID rotation | ⚠️ Token introspection |
| Framework middleware | ✅ 7+ frameworks | ❌ DIY | ❌ DIY | ❌ DIY |
| Enterprise KMS backends | ✅ AWS/GCP/Vault/Azure | ✅ JWKS endpoint | ✅ HSM support | ✅ JWKS endpoint |
| No network at verify time | ✅ Always | ❌ JWKS fetch | ⚠️ SPIRE agent needed | ❌ Introspection call |
| Compliance pack | ✅ SOC2 + ISO27001 | ❌ None | ❌ None | ❌ None |

---

## JWT / OIDC

JWTs are an excellent tool for proving a single identity claim to a single service. They were not designed for multi-hop delegation.

**Where JWTs fall short for AI agents:**

- There is no standard for embedding a delegation chain inside a JWT. Every team invents a different `"delegated_by"` claim format, which cannot be verified by a third party.
- Scope narrowing is convention. Nothing prevents an agent from forwarding a JWT with all its scopes intact to a downstream agent, which then uses those scopes itself.
- JWTs require an IDP at verification time (JWKS fetch) unless you pre-cache the signing key. A1 requires no network call at all.
- An audit trail based on JWT logs is manually assembled. A `ProvableReceipt` is self-contained and tamper-evident.

**A1 + JWT:** These are complementary. Use `a1-identity`'s `JwtBinding` to anchor an Ed25519 passport to an existing OIDC `sub` claim. Your IDP stays the human identity source; A1 handles everything downstream.

---

## SPIFFE / SPIRE

SPIFFE defines workload identity — a cryptographic X.509 certificate proving "this process is service A." SPIRE is its reference implementation.

**Where SPIFFE falls short for AI agents:**

- SPIFFE identifies workloads, not delegation relationships. There is no concept of "workload A delegated action X to workload B with scope Y."
- SVID certificates have no capability model. You cannot cryptographically narrow what a workload is allowed to do.
- SPIRE requires a sidecar agent and a SPIRE server. A1 runs with no sidecar and no server at verification time.
- The chain of custody ends at the workload boundary. If agent B re-delegates to agent C, SPIFFE cannot prove that the original human authorized it.

**A1 + SPIFFE:** Use `SpiffeBinding` to bind a `DyoloIdentity` key to an existing SVID. The SVID proves the workload; the passport proves the human authorization chain.

---

## OAuth2 scopes

OAuth2 token delegation (RFC 8693 Token Exchange) allows exchanging a token for a more-restricted one. It is the closest existing standard to what A1 does.

**Where OAuth2 token exchange falls short:**

- Every delegation hop requires a round-trip to the Authorization Server. This is unacceptable for high-frequency agent calls (milliseconds matter in trading or real-time inference).
- The AS is a centralized single point of failure. If it is unreachable, delegation fails.
- Token exchange scopes are strings. There is no cryptographic proof that "scope A in sub-token is a strict subset of scope A in parent token." An AS implementation mistake could grant broader access.
- Chain depth is not enforced. Nothing prevents A → B → C → D → ... → ∞ delegation.
- No tamper-evident audit trail is produced. You get server logs, not cryptographic receipts.

**A1 + OAuth2:** Use an OAuth2 token to authenticate the human operator to your internal tooling. Generate a `DyoloPassport` on behalf of that human identity. From that point, all agent delegation is handled locally with no further AS involvement.

---

## Performance

| Operation | A1 | JWT RS256 verify | SPIFFE SVID check | OAuth2 introspect |
|---|---|---|---|---|
| Single-hop auth | ~5 µs | ~200 µs | ~1 ms (SPIRE) | ~20 ms (network) |
| NarrowingMatrix check | ~150 ns | N/A | N/A | N/A |
| 256-intent batch | ~800 µs | N/A | N/A | N/A |
| Passport guard end-to-end | ~12 µs | N/A | N/A | N/A |

Numbers are representative order-of-magnitude estimates from the A1 criterion bench suite on an M-class laptop CPU. Run `cargo bench` for exact numbers on your hardware.

---

## Summary

Use A1 when you need:

1. **Recursive multi-agent delegation** with proof that every hop stayed inside the original human's boundaries.
2. **Cryptographic scope narrowing** — not convention, not configuration, not trusting your agents to behave.
3. **Air-gap operation** — authorization decisions that work when the network is down, the IDP is unreachable, or the deployment is classified.
4. **A tamper-evident audit trail** your compliance team can verify independently.

Use JWTs, SPIFFE, or OAuth2 alongside A1 — not instead of it. They solve different layers of the same problem.
