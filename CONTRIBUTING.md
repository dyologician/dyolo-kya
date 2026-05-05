# Contributing to dyolo-kya

Welcome to the `dyolo-kya` ecosystem. This project establishes the definitive cryptographic chain-of-custody protocol for recursive AI agent delegation. We are building the identity and authorization layer for the next generation of enterprise AI, and we demand the highest standards of engineering, security, and performance.

By contributing to this repository, you agree to adhere to the elite engineering standards outlined below. 

---

## 1. Core Engineering Principles

*   **Security & Cryptography First:** This is a security protocol. Do not roll your own crypto. Rely strictly on the established primitives (`ed25519-dalek`, `blake3`, `subtle`). Every authorization path must be constant-time where applicable to prevent timing attacks.
*   **Zero TOCTOU Vulnerabilities:** Storage adapters (Redis, PostgreSQL) must use single-roundtrip, atomic operations for nonce consumption. Check-then-set logic will be immediately rejected.
*   **Absolute Determinism:** Intent hashing and Merkle tree generation must remain strictly deterministic. Order-independent parameter sorting must be preserved across all SDKs.
*   **Stateless by Default:** The gateway and SDKs operate in stateless environments. Do not introduce in-memory caching for authorization decisions unless explicitly feature-gated and strictly bounded.

---

## 2. Local Development Environment

`dyolo-kya` is a multi-language ecosystem. You only need to set up the environments for the specific components you are modifying, though full-stack contributors will need the complete toolchain.

### Prerequisites
*   **Rust:** `1.78+` (via `rustup`)
*   **Node.js:** `20+` (for the TypeScript SDK)
*   **Python:** `3.11+` (for the Python SDK)
*   **Go:** `1.21+` (for the Go SDK)
*   **Docker & Docker Compose:** Required for testing the Gateway, Redis, and Postgres integrations.

### Setting Up
1. **Clone the repository:**
   ```bash
   git clone [https://github.com/dyologician/dyolo-kya.git](https://github.com/dyologician/dyolo-kya.git)
   cd dyolo-kya
Bootstrap the core workspace (Rust):

Bash
cargo build --workspace --all-features
cargo test --workspace --all-features
Start the local infrastructure (Database/Cache):

Bash
docker-compose -f docker/docker-compose.yml up -d redis postgres
3. Language-Specific Guidelines
Rust (Core, Gateway, CLI, Storage Adapters)

Formatting: Code must pass cargo fmt.

Linting: Code must pass cargo clippy --all-targets --all-features -- -D warnings. Zero warnings allowed.

Safety: #![deny(unsafe_code)] is strictly enforced globally. The only exception is the ffi module, which must isolate and exhaustively document safety contracts.

Documentation: All public APIs must have rustdoc comments with embedded doctests.

TypeScript SDK (sdk/typescript)

Types: Strict mode is enabled. No any types allowed; use unknown if absolute necessary.

Compilation: Must pass npx tsc --noEmit without errors.

Testing: Run npx jest. Coverage must remain at 100% for cryptographic verification paths.

Python SDK (sdk/python)

Typing: Must pass mypy --strict. The py.typed marker ensures enterprise consumers can validate their integrations.

Testing: Run pytest. All API endpoints and validation logic must be fully covered.

Go SDK (sdk/go)

Formatting: Must pass gofmt and go vet.

Testing: Run go test -v ./.... Race conditions must be tested via go test -race.

4. Pull Request Standards
We do not merge broken code. Your PR must satisfy the following checklist before requesting a review from the core team:

Atomic Commits: Keep commits logically separated. Rebase and squash dirty commit histories before opening the PR.

Passing CI: The GitHub Actions pipeline must pass completely. This includes all cross-language tests, formatting, and linting checks.

Test Coverage: Any new feature must be accompanied by unit tests. If modifying the core chain validation, add adversarial test cases (e.g., tampered scope proofs, clock drift edge cases).

No Logic Truncation: Do not submit partial implementations, "TODO" comments in critical paths, or mocked cryptographic functions. Deliver fully functional, production-ready code.

Commit Message Format

We enforce conventional commits for automated changelog generation:

feat: [scope] description

fix: [scope] description

sec: [scope] description (for security-related enhancements)

docs: update architecture documentation

5. Security Protocols & Disclosure
If you discover a cryptographic bypass, scope escalation flaw, temporal validation bug, or TOCTOU vulnerability, DO NOT open a public issue or PR.

Refer to SECURITY.md for our responsible disclosure protocol. Email the vulnerability directly to the security team. We maintain a strict 48-hour triage SLA for all cryptographic and authorization-boundary reports.