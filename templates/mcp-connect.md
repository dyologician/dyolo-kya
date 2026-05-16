# Zero-Code Integration via MCP

**Connect A1 to Claude Code (and any MCP-compatible agent) with one config file. No decorators. No code changes.**

---

## What is MCP?

MCP (Model Context Protocol) is an open standard that lets AI agents connect to external tools and services through a config file — no code required. Claude Code, and many other AI agents, support MCP natively.

A1 runs as an MCP server. Add it to your MCP config once, and every compatible agent can use it immediately.

---

## Step 1 — Start A1

Mac / Linux:
```bash
./setup.sh
```

Windows:
```powershell
.\setup.ps1
```

---

## Step 2 — Add the MCP config file

**For Claude Code** — create or edit `.mcp.json` in your project folder:

```json
{
  "mcpServers": {
    "a1": {
      "type": "http",
      "url": "http://localhost:8080/mcp",
      "description": "A1 cryptographic agent authorization"
    }
  }
}
```

That's it. No Python. No imports. No decorators.

**For Claude Desktop** — add to `~/Library/Application Support/Claude/claude_desktop_config.json` (Mac) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "a1": {
      "type": "http",
      "url": "http://localhost:8080/mcp"
    }
  }
}
```

---

## Step 3 — That's it

After adding the config:
- Claude Code automatically detects A1 as an available MCP tool
- When you ask Claude Code to do something, it can call `a1_authorize` to verify authorization before acting
- No restart required — MCP connections are discovered automatically

---

## What Claude Code can now do

Once connected, Claude Code can call these A1 tools directly:

| Tool | What it does |
|---|---|
| `a1_authorize` | Check if an agent is authorized for an action |
| `a1_check_health` | Verify A1 is running |
| `a1_inspect_passport` | Read a passport file's capabilities and expiry |
| `a1_list_capabilities` | List all recognized capability names |
| `a1_issue_cert` | Issue a sub-delegation cert (admin) |
| `a1_revoke` | Revoke a cert by fingerprint (admin) |

---

## Example conversation with Claude Code

Once A1 is connected, you can say:

> *"Before you execute any file operations, check with A1 that you're authorized."*

Claude Code will call `a1_authorize(intent_name="files.write", executor_pk_hex=...)` and only proceed if A1 approves.

Or:

> *"Issue a 1-hour delegation cert for portfolio.read to agent 4a1b2c..."*

Claude Code will call `a1_issue_cert(...)` and show you the result.

---

## Verify the MCP connection

```bash
# Check the MCP tool list
curl http://localhost:8080/mcp/tools | jq '.tools[].name'
```

Expected output:
```
"a1_authorize"
"a1_check_health"
"a1_inspect_passport"
"a1_list_capabilities"
"a1_issue_cert"
"a1_revoke"
```

---

## For any other MCP-compatible agent

The A1 MCP server speaks standard MCP JSON-RPC 2.0 over HTTP. Any agent that implements the MCP client protocol can connect:

```bash
# Test with raw JSON-RPC
curl -X POST http://localhost:8080/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```

---

## Still want decorators?

If you prefer the decorator approach for full control, see:
- [templates/claude-code.md](claude-code.md) — decorator-based integration
- [sdk/python/README.md](../sdk/python/README.md) — full Python SDK
- [CAPABILITIES.md](../CAPABILITIES.md) — all options

---

## Getting help

- A1 Studio: http://localhost:8080/studio
- GitHub: https://github.com/dyologician/a1
- Issues: https://github.com/dyologician/a1/issues
