# Connecting A1 to IronClaw

IronClaw is a security-focused Rust AI agent runtime with WASM sandboxes and encrypted enclaves. A1 + IronClaw is the highest-security combination — both are designed for environments where tool execution must be verifiably authorized.

---

## Automatic connection (recommended)

1. Start A1: `a1 start`
2. Open Studio → click **"Connect Agents"**
3. IronClaw appears in the list → click **"Connect →"**

A1 writes `a1_plugin.toml` to IronClaw's config directory (`~/.ironclaw/`) automatically.

---

## Manual connection

If auto-detection didn't find IronClaw, create `a1_plugin.toml` in `~/.ironclaw/`:

```toml
# A1 Plugin for IronClaw
[plugin.a1]
name        = "a1"
version     = "2.8.0"
description = "Cryptographic chain-of-custody authorization"
enabled     = true

[plugin.a1.gateway]
url        = "http://localhost:8080"
mcp_url    = "http://localhost:8080/mcp"
authorize  = "http://localhost:8080/v1/authorize"
healthz    = "http://localhost:8080/healthz"

[plugin.a1.behaviour]
# Block tool execution if authorization fails
block_on_deny = true
# Log every authorization to file
audit_log = "~/.ironclaw/a1-audit.jsonl"
```

Then restart IronClaw.

---

## Native Rust integration (maximum performance)

For embedded A1 directly in IronClaw without the gateway:

```toml
# IronClaw plugin Cargo.toml
[dependencies]
a1 = { version = "2.8", features = ["wire", "serde"] }
```

```rust
use a1::{DyoloPassport, Intent, SystemClock};

// In your IronClaw tool handler:
let passport = DyoloPassport::load("passport.json")?;
let intent   = Intent::new("files.read")?;
let receipt  = passport.guard_local(&chain, &agent_pk, &intent)?;

// receipt proves authorization — log it or pass it downstream
println!("{receipt}");
```

No gateway needed. All verification is local. Latency: ~22µs.

---

## Capability names for common IronClaw tools

| IronClaw tool | A1 capability |
|---|---|
| File system access | `files.read` / `files.write` |
| WASM plugin execution | `code.execute` |
| Network calls | `api.call` |
| Memory enclave | `memory.write` |
| Agent-to-agent call | `agent.delegate` |

---

## Why A1 + IronClaw is particularly powerful

Both A1 and IronClaw are built with security as the primary goal:

- **IronClaw** provides sandboxing (what code runs)
- **A1** provides authorization (who can run it and what they can do)

Together, they give you:
1. Cryptographic proof of which human authorized the action
2. WASM sandbox preventing unauthorized system access
3. Capability narrowing preventing scope creep
4. Tamper-evident `ProvableReceipt` for every action

This combination meets the requirements of SOC 2 CC6.6 and ISO 27001 A.5.15 without additional tooling.

---

## Verify the connection

```bash
curl http://localhost:8080/v1/agents/scan | jq '.agents[] | select(.id=="ironclaw")'
```

Expected:
```json
{
  "id": "ironclaw",
  "name": "IronClaw",
  "install_path": "/home/user/.ironclaw",
  "connected": true
}
```

---

## Troubleshooting

**IronClaw not detected**
- Check: `which ironclaw` or `ls ~/.ironclaw`
- Try: `ironclaw --version`

**Plugin not loading**
- Check `~/.ironclaw/a1_plugin.toml` syntax with a TOML linter
- Check IronClaw's plugin log: `cat ~/.ironclaw/logs/plugins.log`

---

## More resources

- [A1 GETTING-STARTED.md](../GETTING-STARTED.md)
- [A1 Studio](http://localhost:8080/studio)
- [IronClaw docs](https://github.com/ironclaw/ironclaw)
- [A1 Rust API docs](https://docs.rs/a1)
