# Connecting A1 to Claude Code

Claude Code is the easiest way to integrate A1. It can install everything, write the code, and test the connection — you just tell it what to do.

---

## What you need first

1. **A1 gateway running** — run `./setup.sh` (or `docker compose -f docker/docker-compose.yml up -d`)
2. **A passport file** — created from the A1 Studio wizard (outputs `passport.json`)

---

## The one-prompt integration

Copy this prompt and paste it into Claude Code. Replace the bracketed parts with your values:

```
I want to protect my AI agent tools with A1 (Know Your Agent — cryptographic agent authorization).

Here's my setup:
- A1 gateway: http://localhost:8080
- Passport file: ./passport.json
- Agent namespace: [your-agent-name]
- Capabilities I need: [e.g. files.read, web.search, trade.equity]

Please:
1. Install A1: pip install a1
2. Import and use @a1_guard on my tool functions that need authorization
3. Set up a PassportClient pointing to http://localhost:8080
4. Show me the integration with a working example
5. Test that the gateway connection works
```

Claude Code will:
- Install the `a1` Python package
- Write the `@a1_guard` decorator on your tools
- Configure `PassportClient` with your gateway URL
- Test the connection to make sure everything works

---

## What the integration looks like

After Claude Code integrates A1, your tool functions will look like this:

```python
from a1.passport import a1_guard, PassportClient

client = PassportClient(
    gateway_url="http://localhost:8080",
    passport_path="./passport.json",
)

@a1_guard(client=client, capability="files.read")
async def read_file(path: str, signed_chain: dict, executor_pk_hex: str) -> str:
    with open(path) as f:
        return f.read()
```

The `@a1_guard` decorator:
- Automatically verifies the agent's authorization before your function runs
- Raises `A1AuthorizationError` if the agent is not authorized
- Produces a `ProvableReceipt` for every successful authorization

---

## Verifying it works

Ask Claude Code: *"Test the A1 integration and show me a ProvableReceipt."*

Claude Code will run a test authorization and show you output like:
```
ProvableReceipt {
  namespace: "my-agent",
  chain_depth: 1,
  chain_fingerprint: "a3b2c1...",
  capability_mask: "0000000000000003...",
  authorized: true
}
```

---

## Capability names

The capability string you put in `@a1_guard(capability="...")` must match the capability you listed when issuing the passport.

Common capabilities (use exact strings):
- `files.read` — reading files
- `files.write` — writing files
- `web.search` — searching the web
- `code.execute` — running code
- `email.send` — sending emails
- `trade.equity` — executing trades
- `portfolio.read` — reading portfolio data
- `database.read` — reading from databases
- `api.call` — calling external APIs

---

## Troubleshooting

**"Gateway not connected"**
Run `./setup.sh` or `docker compose -f docker/docker-compose.yml up -d`, then try again.

**"Authorization denied"**
The capability you're trying to use wasn't included in the passport. Re-issue the passport with the missing capability via the Studio wizard.

**"Passport file not found"**
Make sure `passport.json` is in the same directory as your agent, or update `passport_path` to the full path.

---

## More control

- See [CAPABILITIES.md](../CAPABILITIES.md) for all A1 features
- See [wiki/Passport-Guide.md](../wiki/Passport-Guide.md) for delegation chain setup
- Use the A1 Studio Developer Mode for cert inspection and batch authorization
