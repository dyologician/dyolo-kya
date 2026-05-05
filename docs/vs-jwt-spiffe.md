# dyolo-kya vs JWT delegation vs SPIFFE/SPIRE

This document answers the question enterprise security teams ask in architecture
reviews: "why dyolo-kya instead of what we already have?"

---

## The gap each technology is designed to fill

| Property | JWT delegation | SPIFFE/SPIRE | dyolo-kya |
|---|---|---|---|
| Primary audience | Microservices | Platform / infra engineers | AI agent builders & enterprises |
| Trust anchor | Auth server (JWKS) | SPIRE server (PKI) | Human principal (Ed25519) |
| Delegation model | One hop (sub / act claims) | Workload attestation | Unbounded recursive depth with scope narrowing |
| Scope commitment | None (claims are advisory) | X.509 SAN | Merkle root over intent tree — cryptographically enforced |
| Offline verification | No (JWKS fetch) | Partial (bundle cache) | Yes — fully offline with no external dependency |
| Nonce / replay protection | Optional, requires infra | Via mTLS session | Built-in nonce store, pluggable backend |
| Revocation propagation | Token expiry | CRL / OCSP | RevocationStore trait — memory, Redis, PostgreSQL |
| AI-agent aware | No | No | Yes — intent tree maps to agent task taxonomy |
| Framework integrations | None native | None | LangChain, OpenAI Assistants, AutoGen, CrewAI |
| Multi-hop chain of custody | No | No | Yes — provable linkage at every hop |
| Extension/metadata signing | No (unsigned claims) | No | Yes — CertExtensions committed into signature |

---

## JWT delegation (`act` claim — RFC 8693)

JWT delegation (token exchange, the `act` claim) lets a service say "I am
acting on behalf of user X."  It is widely deployed and well understood.

**Where it stops:**

- The chain is at most one hop.  If service A delegates to B which delegates
  to C, there is no cryptographic proof that C's action was authorized by the
  human who started the chain.  The `act` claim is advisory — nothing enforces
  that the original scope has not been widened.

- Scopes are strings in an advisory field.  Nothing prevents a delegated JWT
  from calling an endpoint outside the intended scope.

- JWT verification requires fetching a JWKS endpoint.  Offline or
  air-gapped verification is not possible without additional infrastructure.

dyolo-kya is additive here.  The `JwtBinding` in `dyolo-kya-identity` lets you
bind an existing JWT sub claim to an Ed25519 key, so you get the human-identity
link from your OIDC provider and the multi-hop enforcement from dyolo-kya in the
same deployment.

---

## SPIFFE / SPIRE

SPIFFE answers the question "which workload is this?"  SPIRE provisions
SVIDs (X.509 or JWT) to workloads based on platform attestation (node,
Kubernetes pod, AWS EC2 instance).

**Where it stops:**

- SPIFFE establishes workload identity, not human authorization.  It can
  prove that pod X is running your trading service.  It cannot prove that
  human Alice authorized pod X to place a trade on her behalf.

- There is no concept of recursive delegation or scope narrowing.  An SVID
  grants identity; the authorization decision is still left to the application.

- AI agents are not workloads in the SPIFFE sense.  Ephemeral LLM tool calls
  cannot be attested by a node agent.

dyolo-kya complements SPIFFE at the application layer.  A common pattern is:
SPIRE for workload identity (mTLS between services) and dyolo-kya for human →
agent → sub-agent delegation chains inside those workloads.

---

## When to use each

| Scenario | Best fit |
|---|---|
| Service-to-service auth in a K8s cluster | SPIFFE/SPIRE |
| OAuth2 user → service single hop | JWT / OIDC |
| Human → agent → tool multi-hop with enforceable scope | dyolo-kya |
| Audit trail for AI agent actions | dyolo-kya |
| Enterprise SSO / identity federation | OIDC (bind to dyolo-kya via JwtBinding) |
| Offline / air-gapped authorization | dyolo-kya |

---

## How they combine

```
┌─────────────────────────────────────────────────────────────────┐
│  Human  ──OIDC──▶  Auth Server  ──JWT──▶  API Gateway           │
│                                                                  │
│  Inside the gateway:                                             │
│  JwtBinding.bind(jwt_sub, ed25519_pk)  ── writes dyolo-kya cert │
│                                                                  │
│  Cert chain:  human ──cert──▶ orchestrator ──cert──▶ tool agent │
│                                                                  │
│  mTLS between services: SPIFFE SVID                             │
│  Authorization within services: dyolo-kya chain verification    │
└─────────────────────────────────────────────────────────────────┘
```

This layered model is what a Fortune 500 enterprise security architecture
looks like when AI agents are in the loop.  dyolo-kya fills the one gap
neither JWT nor SPIFFE were designed for: provable, scope-narrowing,
multi-hop human authorization of AI agent actions.
