# Contributing to A1

Welcome to the `a1` ecosystem. This project establishes the definitive cryptographic chain-of-custody protocol for recursive AI agent delegation. We are building the identity and authorization layer for the next generation of enterprise AI, and we demand the highest standards of engineering, security, and performance.

By contributing to this repository, you agree to adhere to the engineering standards outlined below and to the Contributor License Agreement.

---

## Table of Contents

1. [Core Engineering Principles](#1-core-engineering-principles)
2. [Local Development Environment](#2-local-development-environment)
3. [Language-Specific Guidelines](#3-language-specific-guidelines)
4. [Pull Request Standards](#4-pull-request-standards)
5. [Security Contributions](#5-security-contributions)
6. [Contributor License Agreement](#6-contributor-license-agreement)
7. [Code of Conduct](#7-code-of-conduct)

---

## 1. Core Engineering Principles

**Security and cryptography first.** This is a security protocol. Do not roll your own crypto. Rely strictly on the established primitives (`ed25519-dalek`, `blake3`, `subtle`). Every authorization path must be constant-time where applicable to prevent timing attacks.

**Zero TOCTOU vulnerabilities.** Storage adapters (Redis, PostgreSQL) must use single-roundtrip, atomic operations for nonce consumption. Check-then-set logic will be immediately rejected.

**Absolute determinism.** Intent hashing and Merkle tree generation must remain strictly deterministic. Order-independent parameter sorting must be preserved across all SDKs.

**Stateless by default.** The gateway and SDKs operate in stateless environments. Do not introduce in-memory caching for authorization decisions unless explicitly feature-gated and strictly bounded.

**No breaking changes to wire formats.** The `SignedChain` and `DelegationCert` wire formats must remain backward compatible within a major version. Chains issued under v2.0.0 must remain valid under v2.8.0+. New fields must be optional and versioned.

---

## 2. Local Development Environment

`a1` is a multi-language ecosystem. You only need to set up the environments for the specific components you are modifying, though full-stack contributors will need the complete toolchain.

### Prerequisites

| Tool | Minimum version | Purpose |
|---|---|---|
| Rust | 1.85+ (via `rustup`) | Core crate, gateway, CLI, storage adapters |
| Node.js | 20+ | TypeScript SDK |
| Python | 3.9+ | Python SDK |
| Go | 1.21+ | Go SDK |
| Docker + Docker Compose | Latest stable | Gateway, Redis, Postgres integration tests |

### Setting Up

**1. Clone the repository:**

```bash
git clone https://github.com/dyologician/a1.git
cd a1
```

**2. Bootstrap the core workspace (Rust):**

```bash
cargo build --workspace --all-features
cargo test --workspace --all-features
```

**3. Start the local infrastructure (Database / Cache):**

```bash
docker compose -f docker/docker-compose.yml up -d redis postgres
```

**4. Run the integration tests:**

```bash
cargo test --test integration
cargo test --test passport_integration
```

**5. Set up the Python SDK:**

```bash
cd sdk/python
pip install -e ".[dev]"
pytest
```

**6. Set up the TypeScript SDK:**

```bash
cd sdk/typescript
npm install
npx tsc --noEmit
npx jest
```

**7. Set up the Go SDK:**

```bash
cd sdk/go
go test -v ./...
go test -race ./...
```

---

## 3. Language-Specific Guidelines

### Rust (Core, Gateway, CLI, Storage Adapters)

**Formatting.** Code must pass `cargo fmt --all`. No exceptions. Configure your editor to format on save.

**Linting.** Code must pass `cargo clippy --all-targets --all-features -- -D warnings`. Zero warnings allowed. Every new warning introduced by your change must be fixed, not `#[allow(...)]`'d, unless there is an explicit technical reason.

**Safety.** `#![deny(unsafe_code)]` is strictly enforced globally. The only permitted use of `unsafe` is in the `ffi` module, which must:
- Isolate all `unsafe` blocks with explicit safety contracts in doc comments.
- Be gated behind the `ffi` feature flag.
- Not introduce any `unsafe` in non-FFI code paths.

**Documentation.** All public APIs must have `rustdoc` comments. All non-trivial public functions must include embedded `# Examples` doctests. Internal functions should have `//` comments explaining non-obvious logic.

**Testing.** New public API surface must be covered by tests in `tests/integration.rs` or a module-level `#[cfg(test)]` block. Cryptographic paths must include at minimum: a happy-path test, an invalid-signature test, and a scope-escalation test.

**Feature flags.** New features must be gated behind an appropriate feature flag. Feature-gated code must compile cleanly both with and without the flag.

**Performance.** Operations in the hot authorization path must not allocate. Profile with `cargo bench` before and after changes that touch `NarrowingMatrix`, `DyoloChain::authorize`, or `authorize_batch`.

### TypeScript SDK (`sdk/typescript`)

**Types.** Strict mode is enabled (`"strict": true` in `tsconfig.json`). `any` types are not allowed; use `unknown` with a type guard when the type is genuinely unknown at compile time.

**Compilation.** Must pass `npx tsc --noEmit` without errors or warnings.

**Testing.** Run `npx jest`. Coverage must remain at 100% for cryptographic verification paths (passport parsing, chain validation, signature checks). Framework integration wrappers require at minimum a happy-path test and an error-propagation test.

**Exports.** New public API must be exported from `src/index.ts`. New integration tools must be exported from `src/integrations.ts`. Do not create new top-level export files without discussion.

**ESM + CJS.** The SDK publishes both ESM and CJS builds. All new code must compile cleanly with both `tsconfig.esm.json` and `tsconfig.cjs.json`. Do not use constructs that only work in one module system.

### Python SDK (`sdk/python`)

**Typing.** Must pass `mypy --strict`. The `py.typed` marker ensures enterprise consumers can validate their integrations statically. Every function parameter and return value must be annotated. Use `from __future__ import annotations` where needed for forward references.

**Formatting.** Code must be formatted with `black` and pass `ruff check`.

**Testing.** Run `pytest`. All `client.py` methods, validation logic, and KMS integration paths must be fully covered. New framework integrations require: a test with a mock HTTP client (using `respx`), an async test for async paths, and a test for authorization failure handling.

**Async.** All new network operations must be `async`. Synchronous wrappers may be provided for convenience but must not block the event loop.

**Dependencies.** `httpx` is the only mandatory dependency. All other dependencies (framework integrations, KMS clients, SIEM exporters) must be optional extras in `pyproject.toml`. Never add a required dependency without discussion.

**Python version support.** The SDK supports Python 3.9+. Do not use syntax or stdlib features unavailable in 3.9 without a version guard.

### Go SDK (`sdk/go`)

**Formatting.** Must pass `gofmt -l .` (no output means clean). Must pass `go vet ./...`.

**Testing.** Run `go test -v ./...`. Race conditions must be tested via `go test -race ./...`.

**Error handling.** Do not use `panic` in library code. Return errors explicitly. Error types must be exported and documented.

**Generics.** The `WithPassport[T, R]` guard uses generics (Go 1.21+). New generic code must include type constraint documentation explaining the bounds.

**Module path.** The module path is `github.com/dyologician/a1/sdk/go/a1`. All imports in examples and documentation must use this exact path.

---

## 4. Pull Request Standards

We do not merge broken code. Your PR must satisfy the following checklist before requesting a review from the core team.

### Before opening a PR

- [ ] Your branch is rebased on the latest `main` (no merge commits).
- [ ] All commits are logically separated and have meaningful messages (`fix: correct NarrowingMatrix bit ordering` not `fix stuff`).
- [ ] You have run the full test suite locally for every language you modified.
- [ ] You have added tests for every new behavior introduced.
- [ ] You have updated documentation (README, CAPABILITIES.md, relevant wiki pages) for any user-visible changes.
- [ ] If you changed a wire format, you have verified backward compatibility with a test using a fixture from the previous version.

### CI requirements

The GitHub Actions CI pipeline (`ci.yml`) must pass completely:

| Job | What it checks |
|---|---|
| `rust` | `cargo fmt`, `cargo clippy -D warnings`, `cargo test --all-features` |
| `python` | `pytest` (all tests) |
| `typescript` | `tsc --noEmit`, `jest` |
| `go` | `go test -v ./...` |

A PR with any failing CI job will not be reviewed.

### Commit message format

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`, `security`.

Scopes: `core`, `gateway`, `cli`, `python`, `typescript`, `go`, `redis`, `pg`, `ffi`, `zk`, `did`.

Examples:
```
feat(core): add ML-DSA-65 hybrid signature support
fix(gateway): correct rate limiter bucket refill calculation
docs(python): add LlamaIndex integration guide
security(core): use subtle::ConstantTimeEq for nonce comparison
```

### Review process

1. Open the PR with a description explaining the problem, the solution, and any tradeoffs.
2. A core team member will review within 5 business days.
3. Address all review comments. Re-request review after each round of changes.
4. Once approved and CI is green, a maintainer will merge using squash-merge for single-commit PRs or merge-commit for multi-commit feature branches.

---

## 5. Security Contributions

If you discover a security vulnerability, **do not open a public PR or Issue**. Follow the responsible disclosure process in [SECURITY.md](SECURITY.md).

Security fixes follow an expedited process:
1. Report via email to workwithdyolo@gmail.com.
2. We develop a fix in a private fork.
3. We coordinate a release date with you.
4. We publish the fix, the CVE (if applicable), and a public acknowledgement.

Security contributors are credited in CHANGELOG.md and SECURITY.md by default. Anonymity is available on request.

---

## 6. Contributor License Agreement

By submitting a pull request to this repository, you agree that:

1. Your contribution is your original work (or you have the right to submit it).
2. You grant the project maintainers a perpetual, worldwide, non-exclusive, royalty-free license to use, reproduce, modify, and distribute your contribution under the MIT OR Apache-2.0 license.
3. You understand and agree that your contribution becomes part of the project and may be redistributed under the project's license.

If you are contributing on behalf of an employer or under a work-made-for-hire arrangement, you represent that you have the authority to grant this license.

---

## 7. Code of Conduct

This project follows a simple standard: be professional, be constructive, and be respectful.

We will not tolerate harassment, personal attacks, or discriminatory behavior of any kind in any project space (GitHub Issues, Discussions, Pull Requests, code comments).

Violations can be reported to workwithdyolo@gmail.com. Confirmed violations will result in removal from the project.

---

*Thank you for contributing to A1. Every line of code, documentation fix, and test you add makes the AI agent ecosystem more accountable and more secure.*
