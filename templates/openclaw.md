# Connecting A1 to OpenClaw

OpenClaw is a popular open-source personal AI agent (Node.js) that runs locally and connects to WhatsApp, Telegram, email, calendar, shell, and browser. A1 protects OpenClaw's tool executions with cryptographic authorization.

---

## Automatic connection (recommended)

1. Start A1: `a1 start`
2. Open Studio → click **"Connect Agents"**
3. OpenClaw appears in the list → click **"Connect →"**

A1 writes `.mcp.json` to OpenClaw's config directory automatically.

---

## Manual connection

If auto-detection didn't find OpenClaw, create `.mcp.json` in your OpenClaw config directory (usually `~/.openclaw/`):

```json
{
  "mcpServers": {
    "a1": {
      "type": "http",
      "url": "http://localhost:8080/mcp",
      "description": "A1 — cryptographic agent authorization"
    }
  }
}
```

Then restart OpenClaw.

---

## What happens after connecting

Once connected, OpenClaw can call A1 tools before executing skills:

- `a1_authorize` — verify the agent is allowed to run a capability
- `a1_check_health` — verify A1 is running
- `a1_inspect_passport` — check passport details and expiry

OpenClaw will check authorization before executing any skill you've tagged with a capability restriction.

---

## Protecting specific skills (developer mode)

For full skill-level protection, wrap OpenClaw skill handlers with A1 using the TypeScript SDK:

```bash
npm install a1
```

```typescript
import { withA1Passport, PassportClient } from "a1/passport";

const client = new PassportClient({
    gatewayUrl: "http://localhost:8080",
    passportPath: "./passport.json",
});

// Wrap any OpenClaw skill
const guardedSkill = withA1Passport(originalSkill, {
    client,
    capability: "email.send",
});

// Register the guarded version in OpenClaw
openclaw.registerSkill("send_email", guardedSkill);
```

---

## Capability names for common OpenClaw skills

| OpenClaw skill | A1 capability |
|---|---|
| Send email | `email.send` |
| Read email | `email.read` |
| Browse web | `web.search` |
| Run shell command | `code.execute` |
| Read files | `files.read` |
| Write files | `files.write` |
| Calendar | `calendar.write` |
| Send WhatsApp | `api.call` |

Define your own for custom skills: any dot-separated string works.

---

## Verify the connection

```bash
curl http://localhost:8080/v1/agents/scan | jq '.agents[] | select(.id=="openclaw")'
```

Expected:
```json
{
  "id": "openclaw",
  "name": "OpenClaw",
  "install_path": "/home/user/.openclaw",
  "connected": true
}
```

---

## Troubleshooting

**OpenClaw not detected**
- Check that OpenClaw is installed: `which openclaw` or `ls ~/.openclaw`
- If installed in a non-standard path, use the "Custom path" option in Studio

**Authorization failing**
- Make sure A1 is running: `a1 status`
- Check the Live Log in A1 Studio for error details
- Verify the passport has the required capability: `a1 passport inspect passport.json`

---

## More resources

- [A1 GETTING-STARTED.md](../GETTING-STARTED.md)
- [A1 Studio](http://localhost:8080/studio)
- [OpenClaw docs](https://github.com/openclaw/openclaw)
