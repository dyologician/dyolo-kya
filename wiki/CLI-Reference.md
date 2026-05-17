# CLI Reference

The `a1-cli` crate provides the `a1` command-line tool for issuing passports, managing delegation certs, revoking access, and running migrations.

---

## Installation

```bash
cargo install a1-cli
```

Or run from source (from the repo root):

```bash
cargo install --path a1-cli
```

---

## Commands

### `a1 keygen`

Generate an Ed25519 keypair.

```bash
a1 keygen --out key.json
```

Output: `key.json` containing the public key hex and private key seed hex.

---

### `a1 passport issue`

Issue a new root passport for an AI agent.

```bash
a1 passport issue \
  --namespace my-trading-bot \
  --allow "trade.equity,portfolio.read" \
  --ttl 30d \
  --out my-trading-bot-passport.json
```

| Flag | Description |
|---|---|
| `--namespace` | Unique identifier for the agent (e.g. `trading-bot`) |
| `--allow` | Comma-separated capability names |
| `--ttl` | Duration: `1h`, `30d`, `1y`, or raw seconds |
| `--out` | Output path for the passport JSON file |
| `--key` | Optional: path to an existing key file (generates new keypair if omitted) |

**Output files:**
- `<namespace>-passport.json` — the passport (share with agents)
- `<namespace>-key.hex` — the 32-byte signing key seed (**store in your vault, never commit to Git**)

---

### `a1 passport inspect`

Inspect a passport file.

```bash
a1 passport inspect my-trading-bot-passport.json
```

**Output:**

```
Passport: my-trading-bot-passport.json
  Namespace        : my-trading-bot
  Capability mask  : a3b2c1d4...
  Scope root       : 7f9c4e2a...
  Holder public key: 4a1b2c3d...
  Cert issued_at   : 1746547200  (2025-05-06 12:00:00 UTC)
  Cert expires_at  : 1749139200  (2025-06-05 12:00:00 UTC)
  Status           : VALID
```

---

### `a1 passport sub`

Issue a scoped sub-delegation cert from a passport.

```bash
a1 passport sub \
  --passport my-trading-bot-passport.json \
  --key my-trading-bot-key.hex \
  --allow trade.equity \
  --ttl 1h \
  --agent-pk 4a1b2c3d...
```

| Flag | Description |
|---|---|
| `--passport` | Path to the root passport file |
| `--key` | Path to the passport's signing key |
| `--allow` | Capabilities to delegate (must be subset of passport) |
| `--ttl` | TTL for the sub-cert |
| `--agent-pk` | Hex public key of the delegatee agent |
| `--out` | Output path for the cert JSON |

---

### `a1 issue`

Issue a delegation cert via the gateway (admin).

```bash
a1 issue \
  --namespace trading-bot \
  --allow trade.equity \
  --ttl 3600 \
  --agent-pk 4a1b2c3d... \
  --gateway http://localhost:8080 \
  --secret $A1_ADMIN_SECRET
```

---

### `a1 revoke`

Revoke a cert by fingerprint. Takes effect immediately.

```bash
a1 revoke a3b2c1d4e5f6...
```

Optional flags:
- `--store redis://localhost:6379` — Redis RevocationStore URL
- `--gateway http://localhost:8080` — revoke via gateway instead

---

### `a1 revoke-batch`

Revoke multiple certs in one operation.

```bash
a1 revoke-batch a3b2c1... d4e5f6... 7a8b9c...
```

---

### `a1 inspect`

Check revocation status of a cert by fingerprint.

```bash
a1 inspect a3b2c1d4e5f6...
```

**Output:**

```
Fingerprint: a3b2c1d4e5f6...
  Status  : VALID
  Revoked : false
```

---

### `a1 verify`

Verify a `VerifiedToken` HMAC receipt.

```bash
a1 verify receipt.token
```

---

### `a1 decode`

Print all fields of a cert file for debugging.

```bash
a1 decode cert.json
```

**Output:** full cert JSON with decoded timestamps, key hex, capability mask, and SubScopeProof.

---

### `a1 policy`

Apply a YAML policy file to the gateway.

```bash
a1 policy -f policy.yaml --gateway http://localhost:8080 --secret $A1_ADMIN_SECRET
```

**Example `policy.yaml`:**

```yaml
rules:
  - name: max-ttl-trade
    capability: trade.equity
    max_ttl_seconds: 3600

  - name: min-depth-trade
    capability: trade.equity
    min_chain_depth: 2

  - name: allowed-namespaces
    namespaces:
      - trading-prod
      - trading-staging
```

---

### `a1 migrate`

Run Postgres schema migration.

```bash
a1 migrate --pg-url postgres://a1:password@localhost/a1db
```

Creates the `nonces` and `revocations` tables if they don't exist.

---

### `a1 completion`

Generate shell completions.

```bash
a1 completion bash   >> ~/.bashrc
a1 completion zsh    >> ~/.zshrc
a1 completion fish   > ~/.config/fish/completions/a1.fish
a1 completion powershell >> $PROFILE
```

---

## Common workflows

### First-time setup

```bash
# 1. Start the gateway
cd A1 && ./setup.sh

# 2. Issue a passport for your agent
a1 passport issue \
  --namespace my-agent \
  --allow "trade.equity,portfolio.read" \
  --ttl 30d

# 3. Connect your agent — paste the namespace and key path into Studio
open http://localhost:8080/studio
```

### Renew an expiring passport

```bash
a1 passport issue \
  --namespace my-agent \
  --allow "trade.equity,portfolio.read" \
  --ttl 30d \
  --key my-agent-key.hex \
  --out my-agent-passport.json
```

### Emergency revocation

```bash
# Revoke immediately — no confirmation needed
a1 revoke <compromised-cert-fingerprint>

# Verify it's revoked
a1 inspect <compromised-cert-fingerprint>
# Status: REVOKED
```

---

*Source: `a1-cli/src/` · [Back to wiki home](Home)*
