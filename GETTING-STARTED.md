# Getting Started with A1

**Protect your AI agent in 5 minutes. No account. No cloud. Runs on your computer.**

---

## Fastest install (one command)

Mac or Linux — paste this into your terminal:

```bash
curl -fsSL https://github.com/dyologician/a1/releases/latest/download/install.sh | sh
```

Then:

```bash
a1 start
```

That opens A1 Studio in your browser automatically.

**Windows** — double-click `setup.bat`, or run `setup.ps1` in PowerShell.

---

## Mental model in 30 seconds

Three objects. That's all A1 has:

| Object | Plain-English name | Analogy |
|---|---|---|
| **Passport** (`passport.json`) | Your agent's ID card | A government-issued passport |
| **Sub-cert** | A task-specific permission slip | A single-entry visa |
| **Chain** (`chain.json`) | Proof that both are linked | The stamps inside the passport |

When your agent wants to do something, it shows the **chain** (ID card + visa, stapled together). A1 checks both. If anything is expired, revoked, or out-of-scope, the action is blocked — cryptographically, not just by a rule someone could forget to enforce.

That's the whole mental model. The rest of this guide is just the steps to create each object.

---

## What is A1?

A1 gives your AI agent a cryptographic identity — like a passport. When your agent tries to do something (read a file, execute a trade, send an email), A1 checks:

- Is this agent allowed to do this?
- Was this actually authorized by a human?
- Is the authorization still valid (not expired, not revoked)?

If anything is wrong, the action is blocked. Cryptographically. Not just by a policy rule that can be bypassed.

**Why does this matter?** When one AI agent hands off a task to another agent — which hands off to another — there's usually no way to prove the final action was actually authorized by the original human. A1 fixes this with a verifiable chain of proof.

---

## Before you start

You need:
- **Docker Desktop** — to run A1 locally ([get it here](https://docs.docker.com/get-docker/))
- **5 minutes**

You do NOT need:
- An account
- Internet access after setup
- Knowledge of cryptography
- Rust, Python, or any coding experience (to issue a passport)

---

## Step 1 — Start A1

**If you installed with the curl command above:**
```bash
a1 start
```

**If you downloaded the zip:**
```bash
./setup.sh        # Mac / Linux (terminal)
```
```
setup.bat         # Windows (double-click, no terminal needed)
```
```powershell
.\setup.ps1      # Windows (PowerShell)
```

A1 downloads a pre-built binary automatically (no Docker needed) and opens Studio in your browser. Auto-start and a desktop launcher are configured silently.

---

## Step 2 — Open A1 Studio

After running setup, your browser opens automatically to:

```
http://localhost:8080/studio
```

You'll see the **"Protect My Agent"** wizard. This is designed for non-technical users.

---

## Step 3 — Create your agent's passport

The wizard walks you through 3 steps:

**Step 1: Name your agent**
Give your agent a descriptive name like "Trading Bot" or "Research Assistant". This becomes its cryptographic identity.

**Step 2: Choose what it can do**
Check the boxes for what your agent should be allowed to do:
- 📁 Read Files
- ✏️ Write Files
- 🌐 Search the Web
- 📧 Send Emails
- 📈 Execute Trades
- ...and more

You can also add custom capabilities if your agent does something specific.

**Step 3: Set how long it's valid**
Choose 1 day, 1 week, 30 days, etc. After this time, the passport expires and needs to be renewed.

Then click **"Generate setup command"** — the wizard creates the exact command you need to run.

---

## Step 4 — Create the passport file

The wizard shows you a command that looks like this:

```bash
a1 passport issue \
  --namespace trading-bot \
  --allow "trade.equity,portfolio.read" \
  --ttl 30d \
  --out passport.json
```

You have several options:

**Option A — Ask Claude Code to do it:**
Copy the command from the wizard and paste it into Claude Code: *"Run this command for me."* Claude Code will install the CLI and run the command automatically.

**Option B — Run it yourself:**
Open a terminal, install the CLI first (one time only):
```bash
# If published to crates.io:
cargo install a1-cli

# If running from source (run from the repo root):
cargo install --path a1-cli
```

> **Common mistake:** `cargo install --path . --bin a1-cli` fails — the root is the library crate. Use `--path a1-cli` (the CLI sub-folder).

Then paste and run the generated command. It creates `passport.json` in your current folder.

**Option C — Use any AI assistant:**
Paste the command into ChatGPT, Gemini, or any assistant and ask it to help you run it.

The result is `passport.json` — your agent's cryptographic identity file.

> **⚠️ Keep `passport.json` safe — three rules:**
> 1. **Do not commit it to Git.** Add `passport.json` and `*-key.hex` to your `.gitignore` immediately.
> 2. **Do not share it publicly.** Anyone with the key file can sign actions on your agent's behalf.
> 3. **Back it up offline.** If you lose the key file, you must reissue the passport. Existing chains from the old passport will stop validating.
>
> For teams, store key files in a secrets manager (AWS Secrets Manager, 1Password Secrets Automation, HashiCorp Vault). See [Enterprise Deployment](docs/enterprise-deployment.md) for details.

---

## Step 5 — Connect to your AI agent (auto-detection)

In A1 Studio, click **"Connect Agents"** in the sidebar. A1 scans your computer for installed agents (OpenClaw, IronClaw, Claude Code, etc.) and shows them in a list.

Click **"Connect →"** next to your agent. A1 writes the integration config file automatically. Done.

If your agent isn't detected automatically:

| Framework | What you get |
|---|---|
| **Claude Code** | A prompt to paste — Claude Code handles everything |
| **OpenAI Agents** | Python decorator code |
| **LangChain** | LangChain tool wrapper code |
| **LangGraph** | LangGraph node decorator code |
| **CrewAI** | CrewAI tool wrapper code |
| **TypeScript** | TypeScript middleware code |
| **Any (REST)** | HTTP API calls that work in any language |

Copy the code and add it to your agent. Or paste it into Claude Code and ask it to integrate it.

---

## What happens after integration

Once A1 is protecting your agent:

1. Every time your agent tries to use a protected tool, A1 checks the authorization chain.
2. If the agent is authorized → the tool runs. A receipt is produced.
3. If the agent is NOT authorized (wrong scope, expired, revoked) → the tool is blocked immediately.
4. Every authorized action creates a `ProvableReceipt` — a cryptographic proof you can audit later.

---

## Frequently asked questions

**Is my data sent anywhere?**
No. A1 runs 100% on your machine. Nothing is sent to any server, cloud, or third party. Your passport keys never leave your computer.

**What if I want to stop using A1?**
Just remove the A1 decorator (`@a1_guard`, `withA1Passport`, etc.) from your agent code. Your agent works exactly as before.

**Can I use A1 with multiple agents?**
Yes. Run the wizard again for each agent. Each gets its own passport and capability set.

**What if the gateway is not running?**
Your agent will fail authorization until you restart it with `./setup.sh` (or `docker compose up -d`). This is by design — a stopped gateway means nothing can be authorized, which is the safe default.

**My agent isn't detected automatically — what exactly do I paste?**
Copy the code shown in Studio for your framework and add it to your agent file, or paste the whole block into Claude Code with the message: *"Add this A1 integration to my agent."* Claude Code will handle it. For the REST option (any language), see [templates/any-agent.md](templates/any-agent.md) — it includes a zero-boilerplate helper that manages `signed_chain` and `executor_pk_hex` for you automatically.

**Do I need Docker?**
No — the binary installer (`install.sh` / `setup.ps1`) handles everything without Docker. Docker is only needed if the binary fails on your system (unusual). If you see a message like `binary not found for your platform`, install [Docker Desktop](https://docs.docker.com/get-docker/) and re-run `./setup.sh` — it will fall back to the Docker image automatically.

**Personal use vs. production — what's the difference?**
For personal use, the defaults are fine: A1 stores state in memory and resets on restart (no data is lost that matters — you just re-run `a1 start`). For production deployments where you need state to survive restarts, set `A1_REDIS_URL` or `A1_PG_URL` in your environment, set `A1_RATE_LIMIT_RPS` for your traffic, and store signing keys in a KMS. See [docs/enterprise-deployment.md](docs/enterprise-deployment.md). **You do not need any of this for personal use.**

**Can I revoke an agent's access?**
Yes, immediately:
```bash
a1 revoke <fingerprint> --store redis://localhost:6379
```
Or use the Developer Mode in A1 Studio.

**What does "30 days" mean?**
The passport expires after 30 days. After that, the agent cannot authorize any actions until you issue a new passport. You set this when creating the passport.

---

## For developers: quick reference

```bash
# Start gateway (developer)
# First run compiles Rust — takes 3–10 min. Subsequent starts are instant.
docker compose -f docker/docker-compose.yml up -d
# or: ./setup.sh

# Generate keypair
a1 keygen --out key.json

# Issue a passport
a1 passport issue --namespace my-agent --allow "trade.equity,files.read" --ttl 30d

# Inspect a passport
a1 passport inspect passport.json

# Issue sub-delegation cert (for agent-to-agent delegation)
a1 passport sub --passport passport.json --allow trade.equity --ttl 1h --agent-pk <hex>

# Revoke a cert
a1 revoke <fingerprint>

# Gateway endpoints
curl http://localhost:8080/healthz
curl http://localhost:8080/.well-known/a1-configuration
```

For the full developer reference, see [CAPABILITIES.md](CAPABILITIES.md) and [CONTRIBUTING.md](CONTRIBUTING.md).

---

## Getting help

- **GitHub Issues:** https://github.com/dyologician/a1/issues
- **Wiki:** See the [wiki/](wiki/) folder for detailed guides
- **Security issues:** workwithdyolo@gmail.com (do not use GitHub Issues for security reports)

---

*A1 is built and maintained by dyolo ([@dyologician](https://github.com/dyologician)). MIT OR Apache-2.0.*
