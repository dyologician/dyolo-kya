# Publishing Checklist

This document is the step-by-step release checklist for maintainers. Run every step in order before tagging a release.

---

## ⚠️ What NOT to upload to GitHub

The following files must never be committed. They are already excluded by `.gitignore`, but this is a manual reminder before every push:

| File / Pattern | Why |
|---|---|
| `.DS_Store` | macOS metadata — no content, pollutes every directory |
| `__MACOSX/` | macOS zip artifact — created when zipping on a Mac |
| `studio/index.html` | Compiled build output — edit `studio/src/` instead |
| `passport.json` / `*.passport.json` | **Live signing keys — never commit** |
| `*-key.hex` | Raw key material — never commit |
| `.a1/` | Local gateway state and key storage — never commit |
| `target/` | Rust build artifacts |
| `dist/` | TypeScript / Python build artifacts |
| `node_modules/` | npm packages |
| `*.env` | Environment files with secrets |

**Before pushing, always run:**

```bash
git status          # should show only files you intentionally changed
git diff --cached   # review staged changes
```

### 🧹 macOS cleanup — run this before every first push from a Mac

macOS creates `.DS_Store` files in every folder you browse in Finder, and `__MACOSX/` directories whenever you create a zip. These must be purged from the working tree and never allowed into Git.

Run this once from the repo root:

```bash
# Remove all .DS_Store files from the working tree
find . -name ".DS_Store" -not -path './.git/*' -delete

# Remove all __MACOSX directories (created when zipping on Mac)
find . -name "__MACOSX" -not -path './.git/*' -exec rm -rf {} + 2>/dev/null || true

# If any were previously tracked by Git, un-track them:
git rm --cached -r --ignore-unmatch '**/.DS_Store' '.DS_Store' '__MACOSX'

# Remove studio/index.html if it crept in (compiled output — do not commit)
git rm --cached --ignore-unmatch studio/index.html
```

After running the above, verify nothing leaked through:

```bash
git status | grep -E "DS_Store|MACOSX|studio/index"
# Should print nothing. If it does, something is wrong with .gitignore.
```

---

## Before you start

Make sure you are on `main` with a clean working tree and CI is green.

```bash
git checkout main && git pull
git status          # must be clean
```

---

## Step 1 — Version audit

All package manifests must match. Run this to check:

```bash
grep '^version' Cargo.toml a1-*/Cargo.toml
grep '^version' sdk/python/pyproject.toml
node -p "require('./sdk/typescript/package.json').version"
head -1 CAPABILITIES.md
```

All lines should print the release version. The files to update if they differ:

| File | Field |
|---|---|
| `Cargo.toml` (root) | `[package] version` |
| `a1-redis/Cargo.toml` | `[package] version` |
| `a1-pg/Cargo.toml` | `[package] version` |
| `a1-identity/Cargo.toml` | `[package] version` |
| `a1-cli/Cargo.toml` | `[package] version` |
| `a1-gateway/Cargo.toml` | `[package] version` |
| `sdk/python/pyproject.toml` | `[project] version` |
| `sdk/typescript/package.json` | `"version"` |
| `CAPABILITIES.md` | Header line (`# A1 Capabilities Reference — vX.Y.Z`) |

---

## Step 2 — Update CHANGELOG

Move everything under `[Unreleased]` to a new dated version block:

```markdown
## [X.Y.Z] — YYYY-MM-DD
```

Commit the version bump and CHANGELOG update together:

```bash
git add -A
git commit -m "chore: release vX.Y.Z"
git push
```

---

## Step 3 — Verify CI is green

Wait for GitHub Actions to finish on the commit above. All four jobs must pass:

- **Rust Core** — formatting, Clippy, tests, audit, dry-run publish
- **Python SDK** — pytest on 3.9, 3.11, 3.12
- **TypeScript SDK** — tsc, jest, build
- **Go SDK** — vet, tests, race detector

Do not proceed if any job is red.

---

## Step 4 — Tag the release

```bash
git tag -a "vX.Y.Z" -m "Release vX.Y.Z"
git push origin "vX.Y.Z"
```

The tag triggers `.github/workflows/release-binaries.yml` which builds cross-platform CLI binaries and attaches them to the GitHub Release draft.

---

## Step 5 — Publish Rust crates (in order)

Sub-crates declare `a1 = "X.Y.Z"` as a dependency from crates.io. They cannot be published until the core crate is indexed. Wait ~2 minutes between steps 5a and 5b.

```bash
# 5a. Core crate first
cargo publish -p a1

# Wait ~2 minutes for crates.io to index a1

# 5b. Storage and identity adapters (order within this group does not matter)
cargo publish -p a1-redis
cargo publish -p a1-pg
cargo publish -p a1-identity

# 5c. CLI and gateway (depend on the adapters)
cargo publish -p a1-cli
cargo publish -p a1-gateway
```

Verify each on crates.io before continuing:

```
https://crates.io/crates/a1
https://crates.io/crates/a1-redis
https://crates.io/crates/a1-pg
https://crates.io/crates/a1-identity
https://crates.io/crates/a1-cli
https://crates.io/crates/a1-gateway
```

---

## Step 6 — Publish Python SDK to PyPI

```bash
cd sdk/python
pip install build twine --quiet
python -m build
twine check dist/*
twine upload dist/*
```

Verify: `https://pypi.org/project/a1/`

---

## Step 7 — Publish TypeScript SDK to npm

```bash
cd sdk/typescript
npm run build
npm publish --access public
```

Verify: `https://www.npmjs.com/package/a1`

---

## Step 8 — Go SDK

The Go SDK is consumed directly from the GitHub repository via `go get`. No separate publish step is required — the tag from Step 4 is sufficient.

Verify:

```bash
go get github.com/dyologician/a1/sdk/go/a1/kya@vX.Y.Z
```

---

## Step 9 — Publish the GitHub Release

1. Go to **Releases** → find the draft created by the binary workflow.
2. Set the tag to `vX.Y.Z`.
3. Copy the CHANGELOG section for this version into the release body.
4. Attach any additional release assets if needed.
5. Publish.

---

## Step 10 — Post-release smoke test

```bash
# Rust
cargo add a1@X.Y.Z --features full   # in a scratch project
cargo build

# Python
pip install a1==X.Y.Z
python -c "import a1; print(a1.__version__)"

# TypeScript
npm install a1@X.Y.Z
node -e "const a1 = require('a1'); console.log('ok')"

# CLI
cargo install a1-cli@X.Y.Z
a1 --version
```

---

## Rollback

If a critical bug is found after publishing:

- **Crates.io** — `cargo yank --version X.Y.Z -p a1` (yanking does not delete but prevents new installs)
- **PyPI** — use the PyPI web UI to yank the release
- **npm** — `npm deprecate a1@X.Y.Z "Critical bug — use X.Y.Z+1"`
- **GitHub** — convert the release to a pre-release draft, add a prominent warning
- Issue a patch release (X.Y.Z+1) as soon as possible following the same checklist
