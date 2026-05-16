## Summary

<!-- One paragraph: what this PR does and why. Link the issue it closes if applicable. -->

Closes #

---

## Type of change

- [ ] Bug fix (non-breaking)
- [ ] New feature (non-breaking)
- [ ] Breaking change (wire format, public API, behaviour)
- [ ] Documentation only
- [ ] Performance improvement
- [ ] Refactor / cleanup

---

## Changes

<!-- Brief list of what changed and where. Be specific — reviewers should be able to audit the diff with this as a guide. -->

- `src/chain.rs` — ...
- `sdk/python/a1/passport.py` — ...

---

## Security impact

<!-- Every PR must answer this. "None" is a valid answer if you are certain. -->

**Cryptographic primitives changed?** No / Yes — explain:

**Wire format changed?** No / Yes — backward compatible? Explain:

**New attack surface introduced?** No / Yes — mitigated how?

**Constant-time paths affected?** No / Yes — explain:

---

## Testing

<!-- How did you verify this works and does not break existing behaviour? -->

- [ ] `cargo test --all-features` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] New unit tests added for the changed logic
- [ ] Integration tests pass (`cargo test --test integration`)
- [ ] Benchmarks checked — no regression (`cargo bench`)

**SDK tests (check all that apply to this PR):**
- [ ] Python: `pytest` passes in `sdk/python/`
- [ ] TypeScript: `npm test` passes in `sdk/typescript/`
- [ ] Go: `go test ./...` passes in `sdk/go/`

---

## Documentation

- [ ] `CHANGELOG.md` updated with a clear entry under `[Unreleased]`
- [ ] Public API changes reflected in `CAPABILITIES.md`
- [ ] New gateway endpoints added to the REST API reference in `README.md`
- [ ] New env vars added to the environment variable table in `README.md`
- [ ] `docs/` or `wiki/` updated if applicable

---

## Breaking changes

<!-- If this is a breaking change, describe the migration path clearly. -->

N/A

---

## Checklist

- [ ] I have read [CONTRIBUTING.md](../CONTRIBUTING.md)
- [ ] My changes follow the engineering principles (no unsafe Rust outside `ffi`, no global state, deterministic hashing, atomic nonce operations)
- [ ] I have not introduced any new dependencies without discussion
- [ ] This PR does not expose secrets, keys, or test credentials