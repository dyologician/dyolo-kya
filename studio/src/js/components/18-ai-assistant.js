// ─────────────────────────────────────────────────────────────────────────────
// AI ASSISTANT — knowledge base + local LLM bridge
// ─────────────────────────────────────────────────────────────────────────────

const _KB_NS = (function () {
  const _b = [0x64, 0x79, 0x6f, 0x6c, 0x6f];
  let h = 0x811c9dc5;
  _b.forEach(b => { h ^= b; h = Math.imul(h, 0x01000193) >>> 0; });
  return h.toString(16).padStart(8, '0');
})();

const KB = [
  {
    id: 'what-is-a1',
    category: 'Overview',
    title: 'What is A1?',
    tags: ['overview', 'intro', 'basics', 'what is', 'explain'],
    body: `A1 is a cryptographic identity and authorization layer for AI agents. It solves the "Recursive Delegation Gap" — the fact that when one AI agent delegates work to another, there is no verifiable chain of who authorized what.

With A1, every agent gets a Passport: a signed cryptographic credential that says exactly which human authorized it, what it is allowed to do (capabilities), and for how long (TTL). Every action the agent takes is checked against that passport in real time. If the agent tries anything outside its allowed capabilities, A1 blocks it immediately.

A1 uses Ed25519 signatures and zero-knowledge proofs. An irrefutable chain of custody is produced for every delegated action — proving the human → orchestrator → executor path is authorized at every step.

A1 is a Rust library (crate), a Python/TypeScript/Go SDK, and a local gateway server. The A1 Studio is the visual dashboard for managing all of this without code.`,
  },
  {
    id: 'who-is-dyolo',
    category: 'Community',
    title: 'Who is Dyolo? — creator of A1',
    tags: ['who is dyolo', 'dyolo', 'creator', 'author', 'dyologician', 'founder', 'who made a1', 'who built a1', 'about dyolo'],
    body: `A1 is built and maintained by Dyolo, whose handle is @dyologician everywhere.

CORRECT HANDLES (all platforms):
  X (Twitter):  https://x.com/dyologician
  GitHub:       https://github.com/dyologician
  Reddit:       u/dyologician

NOTE: There is no "Daniel Bowen" connected to A1. That name does not exist in this project. The creator is known only as Dyolo / @dyologician.

GITHUB REPO: https://github.com/dyologician/a1
  The A1 source code, documentation, issues, and discussions all live here.

WHAT DYOLO BUILT:
  Dyolo designed and implemented A1 from the ground up — the cryptographic passport system,
  the NarrowingMatrix capability enforcement, the ProvableReceipt audit trail, the self-hosted
  gateway, the Python/TypeScript/Go SDKs, all framework integrations, and A1 Studio.

A1 IS OPEN SOURCE:
  License: MIT OR Apache-2.0
  Anyone can contribute. See CONTRIBUTING.md for the process.

CONTACT / SECURITY:
  Security issues: workwithdyolo@gmail.com (do NOT use GitHub Issues for security reports)
  General discussions: https://github.com/dyologician/a1/discussions`,
  },
  {
    id: 'narrowing-matrix',
    category: 'Technical',
    title: 'NarrowingMatrix — O(1) capability enforcement',
    tags: ['narrowing', 'narrowingmatrix', 'capabilities', 'bitmask', 'subset', 'enforcement', 'delegation scope', 'capability mask'],
    body: `The NarrowingMatrix is the cryptographic mechanism that enforces capability subset relationships in A1.

HOW IT WORKS:
  - Each capability name (e.g. "trade.equity") maps to a bit position in a 256-bit field via Blake3 hashing (domain: "dyolo::narrowing::v1").
  - Every DelegationCert carries a NarrowingMatrix representing its allowed capabilities.
  - A sub-cert's mask must be a bitwise subset of its parent: child_mask & parent_mask == child_mask.
  - This check is 8x 64-bit AND operations — effectively O(1) regardless of capability count.

SPEED: ~1–150 ns per check. No network call, no registry, no config file at verification time.

WHAT IT PREVENTS:
  - Privilege escalation: an agent cannot claim capabilities its parent did not grant.
  - Scope inflation: each delegation hop can only narrow, never widen, the capability set.

COLLISION HANDLING:
  For up to ~20 capabilities, the Blake3 hash collision probability is under 0.1%.
  For larger deployments, use CapabilityRegistry to assign explicit bit positions.

RUST USAGE:
  use a1::NarrowingMatrix;
  let parent = NarrowingMatrix::from_capabilities(&["trade.equity", "portfolio.read"])?;
  let child  = NarrowingMatrix::from_capabilities(&["trade.equity"])?;
  assert!(child.is_subset_of(&parent));  // passes
  parent.enforce_narrowing(&child)?;     // returns PassportNarrowingViolation if child > parent`,
  },
  {
    id: 'provable-receipt',
    category: 'Technical',
    title: 'ProvableReceipt — tamper-evident audit proof',
    tags: ['receipt', 'provablereceipt', 'audit', 'proof', 'compliance', 'trail', 'authorized action', 'chain fingerprint'],
    body: `A ProvableReceipt is produced for every action A1 authorizes. It is a cryptographically self-contained proof that a specific action was authorized at a specific time.

WHAT IT CONTAINS:
  - Chain depth (number of delegation hops)
  - Chain fingerprint (Blake3 over all certs — changes if any cert is altered)
  - Authorized intent hash (the specific action that was permitted)
  - Passport namespace
  - Enforced capability mask (hex)
  - Blake3 commitment over the capability mask (proves the scope at authorization time)
  - Optional ProvenanceRoot (Merkle commitment to the agent's full reasoning trace)

WHY IT MATTERS:
  - Receipts are independently verifiable without any secrets or network access.
  - They cannot be forged or altered after issuance (Blake3 commitment).
  - They satisfy EU AI Act Article 13 transparency requirements and SOC 2 CC6.1.
  - You can ship a receipt to an auditor, store it in a SIEM, or anchor it on-chain.

REASONING TRACE:
  Agents can record a step-by-step trace (thoughts, tool calls, observations).
  ReasoningTrace::finalize() produces a Merkle root bound to the chain fingerprint.
  Individual steps can be selectively disclosed via ProvenanceStepProof.

EXAMPLE OUTPUT:
  ProvableReceipt { namespace=trading-bot, depth=2, fingerprint=a3f7..., mask=0x0003... }`,
  },
  {
    id: 'post-quantum',
    category: 'Technical',
    title: 'Post-quantum hybrid signatures — ML-DSA / CRYSTALS-Dilithium',
    tags: ['post-quantum', 'ml-dsa', 'dilithium', 'quantum', 'hybrid', 'pq', 'future proof', 'crystals'],
    body: `A1 v2.8 supports post-quantum hybrid signatures so you can protect agent chains against future quantum computers without breaking anything today.

THREE ALGORITHM MODES:
  Ed25519 (default)            — Classical, 128-bit security. All existing deployments.
  HybridMlDsa44Ed25519         — Ed25519 + ML-DSA-44. NIST Level 2 (128-bit post-quantum).
  HybridMlDsa65Ed25519         — Ed25519 + ML-DSA-65. NIST Level 3 (192-bit PQ, recommended for finance/gov).

KEY SIZES:
  ML-DSA-44: public key = 1312 bytes, signature = 2420 bytes
  ML-DSA-65: public key = 1952 bytes, signature = 3309 bytes

HOW TO ENABLE:
  In Cargo.toml: a1 = { version = "2.8", features = ["full", "post-quantum"] }
  Existing Ed25519 chains remain 100% valid. No migration required.
  Algorithm negotiation: negotiate_algorithm() picks the strongest your build supports.

CHAIN MIGRATION RULES:
  Ed25519 certs followed by hybrid leaf certs: OK.
  Hybrid certs followed by Ed25519 certs: REJECTED (monotonic upgrade only).

PQ CONTEXT BINDING:
  Even without the post-quantum feature enabled, every cert's pq_context commitment
  is verified — proving the declared algorithm intent cryptographically.`,
  },
  {
    id: 'framework-integrations',
    category: 'Integrations',
    title: 'Framework integrations — LangChain, LangGraph, CrewAI, AutoGen, and more',
    tags: ['langchain', 'langgraph', 'llamaindex', 'autogen', 'crewai', 'semantic kernel', 'openai agents', 'framework', 'integration', 'decorator'],
    body: `A1 has native one-liner integrations for all major AI agent frameworks.

PYTHON DECORATORS (one line each):

  LangChain:
    from a1.langchain_tool import A1AuthorizationTool
    tool = A1AuthorizationTool(name="execute_trade", intent_name="trade.equity", client=client, func=fn, chain=chain, executor_pk_hex=pk)

  LangGraph:
    from a1.langgraph_tool import a1_node
    @a1_node(intent_name="trade.equity", client=client, propagate_receipt=True)
    async def execute_trade(state): ...

  LlamaIndex:
    from a1.llamaindex_tool import a1_llamaindex_tool
    tool = a1_llamaindex_tool(fn=read_portfolio_fn, intent_name="portfolio.read", client=client, ...)

  AutoGen v0.4:
    from a1.autogen_tool import build_a1_function_tool
    tool = build_a1_function_tool(fn=execute_trade, intent_name="trade.equity", client=client, ...)

  CrewAI:
    from a1.crewai_tool import A1AuthorizationTool
    tool = A1AuthorizationTool(func=execute_trade, intent_name="trade.equity", gateway_url="http://localhost:8080", ...)

  Semantic Kernel:
    from a1.semantic_kernel_tool import a1_sk_function
    @a1_sk_function(intent_name="trade.equity", client=client, description="Execute equity trade.")
    async def execute_trade(self, symbol: str, ...) -> str: ...

  OpenAI Agents SDK:
    from a1.openai_tool import a1_openai_function
    @a1_openai_function(intent_name="trade.equity", client=client)
    async def execute_trade(symbol: str, qty: int) -> str: ...

TYPESCRIPT:
  import { withA1Passport } from "a1/passport";
  const guarded = withA1Passport(executeTrade, { client, capability: "trade.equity" });

GO:
  import "github.com/dyologician/a1/sdk/go/a1"
  guarded := a1.WithPassport(executeTrade, passport)`,
  },
  {
    id: 'kms-integrations',
    category: 'Enterprise',
    title: 'KMS signing backends — AWS, GCP, HashiCorp Vault, Azure',
    tags: ['kms', 'aws kms', 'gcp kms', 'hashicorp vault', 'azure key vault', 'vault', 'signing', 'key management', 'hsm'],
    body: `A1 never locks you into storing root keys in a file. Production deployments use a KMS so key material never enters application memory.

SUPPORTED KMS BACKENDS:
  AWS KMS:
    from a1.vault import AwsKmsSigner
    signer = AwsKmsSigner(key_id="alias/a1-passport-root", region="us-east-1")

  GCP Cloud KMS:
    from a1.vault import GcpKmsSigner
    signer = GcpKmsSigner(project="my-project", location="global", key_ring="a1-keys", key="passport-root")

  HashiCorp Vault (Transit engine, ed25519 key):
    from a1.vault import HashiCorpVaultSigner
    signer = HashiCorpVaultSigner(vault_addr="https://vault.corp.example.com", key_name="a1-passport-root")

  Azure Key Vault:
    from a1.vault import AzureKeyVaultSigner
    signer = AzureKeyVaultSigner(vault_url="https://my-vault.vault.azure.net", key_name="a1-passport-root")

IMPORTANT: At verification time, zero KMS calls are made. The verifying key is embedded in the cert — authorization is fully local even when KMS is used for signing.

GATEWAY DOCKER WITH KMS:
  docker run -e A1_SIGNING_KEY_HEX=<key> -p 8080:8080 ghcr.io/dyologician/a1:latest
  For KMS: configure the appropriate SDK environment variables (AWS_REGION, VAULT_ADDR, etc.)`,
  },
  {
    id: 'siem-integrations',
    category: 'Enterprise',
    title: 'SIEM audit log export — Datadog, Splunk, OpenTelemetry, NDJSON',
    tags: ['siem', 'datadog', 'splunk', 'opentelemetry', 'otlp', 'audit log', 'logging', 'audit trail', 'export'],
    body: `Every A1 authorization event can be exported to your existing SIEM with zero extra configuration.

PYTHON SIEM EXPORTERS:
  from a1.siem import DatadogLogExporter, SplunkHecExporter, OpenTelemetryExporter, CompositeExporter

  Datadog:
    exporter = DatadogLogExporter(api_key=os.environ["DD_API_KEY"], service="trading-agents")

  Splunk (HEC):
    exporter = SplunkHecExporter(url="https://splunk.corp.com:8088", token=os.environ["SPLUNK_HEC_TOKEN"])

  OpenTelemetry (OTLP):
    exporter = OpenTelemetryExporter(endpoint="http://otel-collector:4318", service_name="agents")

  Composite (multiple sinks simultaneously):
    exporter = CompositeExporter([datadog_exporter, splunk_exporter])
    exporter.export_dict(audit_event)

NDJSON FILE (for any log pipeline):
  exporter = JsonlFileExporter(path="/var/log/a1-audit.jsonl")

WHAT EACH AUDIT EVENT CONTAINS:
  - Timestamp, namespace, agent fingerprint
  - Authorized capability (intent)
  - Chain depth and chain fingerprint
  - ProvableReceipt commitment
  - Pass/fail result and error code if blocked`,
  },
  {
    id: 'compliance',
    category: 'Enterprise',
    title: 'Compliance — SOC 2, ISO 27001, EU AI Act, NIST AI RMF',
    tags: ['compliance', 'soc2', 'iso27001', 'eu ai act', 'nist', 'audit', 'regulatory', 'certify', 'hipaa', 'gdpr'],
    body: `A1 ships with compliance documentation you can hand directly to your auditor.

INCLUDED COMPLIANCE DOCUMENTS (in docs/compliance/):
  soc2-mapping.md         — SOC 2 Type II Trust Service Criteria mapping. Ready for auditors.
  iso27001-mapping.md     — ISO/IEC 27001:2022 Annex A control mapping with codebase evidence pointers.
  sample-audit-report.md  — Pre-filled audit report template. Replace bracketed placeholders and submit.

REGULATORY ALIGNMENT:
  EU AI Act Article 13   — A1 ProvableReceipts satisfy transparency and traceability requirements.
  NIST AI RMF Govern 1.7 — Every agent action is cryptographically linked to a human authorization.
  SOC 2 CC6.1            — Logical access controls with verifiable chain of custody.
  HIPAA / GDPR           — Namespace isolation + audit trail supports data access governance.

HOW A1 MAPS TO CONTROLS:
  Human oversight (enforced, not logged) → DyoloPassport + DyoloChain
  Audit trail (tamper-evident)           → ProvableReceipt with Blake3 commitment
  Access revocation                       → RevocationStore (Redis or Postgres, sub-ms latency)
  Multi-tenant isolation                  → Namespace-scoped chains (hard separation)
  Key management                          → KMS/Vault signing backends`,
  },
  {
    id: 'performance-benchmarks',
    category: 'Technical',
    title: 'Performance benchmarks — latency numbers',
    tags: ['performance', 'benchmark', 'speed', 'latency', 'throughput', 'bench', 'fast', 'nanoseconds', 'microseconds'],
    body: `Typical A1 authorization latencies on Apple M3 / AWS c7g.large. Run "cargo bench" to reproduce locally.

CORE OPERATIONS:
  NarrowingMatrix::is_subset_of          ~1–150 ns    (bitwise AND, O(1))
  NarrowingMatrix::from_capabilities     ~350 ns      (8 capabilities via Blake3)
  NarrowingMatrix::commitment            ~80 ns       (Blake3 over 32 bytes)
  Single-hop chain authorization         ~5 µs
  Two-hop scoped chain authorization     ~12 µs
  DyoloPassport::guard_local end-to-end  ~22 µs
  authorize_batch (256 intents)          ~1.1 ms
  authorize_batch (1024 intents)         ~4.3 ms

GATEWAY THROUGHPUT (Redis, 8-core):
  /v1/authorize                          ~15,000 req/s

COMPARISON TO ALTERNATIVES:
  NarrowingMatrix vs Groth16 ZK proof    ~4000× faster
  NarrowingMatrix vs policy engine eval  no external process, no network, ~100–1000× faster

WHAT THIS MEANS IN PRACTICE:
  A1 authorization adds <25 µs to a single agent action — far below the latency of any
  LLM inference call (typically 100–2000 ms). The authorization overhead is imperceptible.`,
  },
  {
    id: 'cli-reference',
    category: 'Setup',
    title: 'CLI reference — all a1 commands',
    tags: ['cli', 'command line', 'commands', 'terminal', 'a1 cli', 'keygen', 'verify', 'revoke', 'inspect', 'decode', 'migrate', 'completion'],
    body: `Full reference for the a1 command-line tool. Install with: cargo install a1-cli

KEYPAIR MANAGEMENT:
  a1 keygen --out key.json                        Generate a new Ed25519 keypair

PASSPORT COMMANDS:
  a1 passport issue \\
    --namespace my-agent \\
    --allow "trade.equity,portfolio.read" \\
    --ttl 30d \\
    --out passport.json                           Issue a root passport

  a1 passport inspect passport.json              Show namespace, capabilities, expiry, status

  a1 passport sub \\
    --passport passport.json \\
    --allow trade.equity \\
    --ttl 1h \\
    --agent-pk <hex>                              Issue a sub-delegation cert

CHAIN / VERIFICATION:
  a1 verify chain.json --principal-pk <hex>      Verify a signed delegation chain

  a1 decode cert.json                            Decode a raw cert (debug)

REVOCATION:
  a1 revoke <fingerprint> --store redis://localhost:6379
  a1 revoke-batch <fp1> <fp2> --store redis://localhost:6379

POLICY:
  a1 policy -f policy.yaml                       Apply a YAML delegation policy

MIGRATIONS:
  a1 migrate                                     Run Postgres schema migration

GATEWAY:
  a1 start                                       Start the gateway + open Studio

SHELL COMPLETIONS:
  a1 completion bash                             Also: fish, zsh, powershell`,
  },
  {
    id: 'feature-flags',
    category: 'Technical',
    title: 'Rust feature flags — what each flag unlocks',
    tags: ['features', 'feature flags', 'cargo', 'rust features', 'flags', 'serde', 'wire', 'async', 'did', 'zk', 'anchor', 'ffi', 'cbor', 'full', 'post-quantum'],
    body: `A1 is a modular Rust crate. Enable only what you need via Cargo feature flags.

CORE FLAGS:
  serde         — Serialization for all types (JSON, CBOR)
  wire          — SignedChain, VerifiedToken, CertExtensions wire types
  async         — Async storage traits, AsyncA1Context, VaultSigner
  full          — All flags except ffi and post-quantum (recommended for most users)

ADVANCED / OPTIONAL FLAGS:
  did           — AgentDid, DidDocument, VerifiableCredential (W3C DID + VC)
  zk            — ZkChainCommitment, ZkProofMode, anchor_hash
  anchor        — On-chain anchoring via anchor_hash
  negotiate     — Algorithm negotiation for hybrid deployments
  swarm         — Swarm coordination primitives
  governance    — On-chain governance vote recording
  tracing       — tracing spans during authorization
  ffi           — C ABI for embedding in Python, Go, Java, Node.js
  policy-yaml   — YAML policy file parsing
  post-quantum  — Full ML-DSA-44/ML-DSA-65 signature verification
  schema        — JSON Schema export for SignedChain
  cbor          — Binary wire encoding for bandwidth-constrained environments

RECOMMENDED CARGO.TOML:
  [dependencies]
  a1 = { version = "2.8", features = ["full"] }

  For production with post-quantum:
  a1 = { version = "2.8", features = ["full", "post-quantum"] }`,
  },
  {
    id: 'gateway-env-vars',
    category: 'Enterprise',
    title: 'Gateway environment variables — full reference',
    tags: ['environment variables', 'env vars', 'config', 'gateway config', 'production config', 'docker env', 'A1_SIGNING_KEY_HEX', 'A1_REDIS_URL'],
    body: `All environment variables for the A1 gateway. Set these in Docker, Kubernetes, or your shell.

REQUIRED FOR PRODUCTION:
  A1_SIGNING_KEY_HEX      32-byte hex Ed25519 seed for gateway signing identity. Generated randomly if unset (resets on restart — not suitable for production).
  A1_MAC_KEY_HEX          32-byte hex key for VerifiedToken HMAC. Same caveat as above.
  A1_ADMIN_SECRET         Bearer token for admin endpoints. SET THIS in production.

STORAGE BACKENDS:
  A1_REDIS_URL            Redis URL (e.g. redis://127.0.0.1/). Enables durable nonce + revocation store.
  A1_PG_URL               Postgres URL (e.g. postgres://user:pass@host/db). Alternative to Redis.

NETWORKING:
  GATEWAY_ADDR            Bind address. Default: 0.0.0.0:8080
  A1_PUBLIC_BASE_URL      Used in .well-known discovery doc. Default: http://localhost:8080
  A1_CORS_ALLOWED_ORIGIN  CORS origin (use * for permissive, or your frontend origin)
  A1_TRUSTED_PROXY_MODE   x-forwarded-for, fly-client-ip, or cf-connecting-ip

RATE LIMITING:
  A1_RATE_LIMIT_RPS       Per-IP requests per second. Default: 500

LOGGING:
  RUST_LOG                Log filter. Default: a1_gateway=info

RULE OF THUMB: For personal use on your laptop, ignore all of this — the defaults work fine.
For production, set at minimum: A1_SIGNING_KEY_HEX, A1_MAC_KEY_HEX, A1_ADMIN_SECRET, and A1_REDIS_URL or A1_PG_URL.`,
  },
  {
    id: 'sdk-go',
    category: 'SDKs',
    title: 'Go SDK — installation and usage',
    tags: ['go', 'golang', 'go sdk', 'withpassport', 'go module', 'go get'],
    body: `A1 has a first-class Go SDK at github.com/dyologician/a1/sdk/go/a1.

INSTALL:
  go get github.com/dyologician/a1/sdk/go/a1

BASIC USAGE (middleware pattern):
  import "github.com/dyologician/a1/sdk/go/a1"

  // Wrap any function with passport authorization
  guarded := a1.WithPassport(executeTrade, passport)

  // Call the guarded function — A1 checks authorization first
  result, err := guarded(ctx, symbol, qty)

PASSPORT LOADING:
  passport, err := a1.LoadPassport("passport.json")

CLIENT (REST gateway):
  client := a1.NewPassportClient("http://localhost:8080")

GENERIC GUARD (type-safe):
  // WithPassport[T, R] is a generic guard function
  // T = input type, R = return type

DOCS:
  Full reference: https://pkg.go.dev/github.com/dyologician/a1/sdk/go/a1
  Source:         https://github.com/dyologician/a1/tree/main/sdk/go`,
  },

  {
    id: 'getting-started',
    category: 'Setup',
    title: 'Getting Started (step by step)',
    tags: ['setup', 'quickstart', 'install', 'onboarding', 'first steps', 'start', 'begin', 'how to'],
    body: `The correct order to get started with A1:

1. INSTALL A1 GATEWAY
   Run the install script:  curl -fsSL https://raw.githubusercontent.com/dyologician/a1/main/install.sh | sh
   Or with cargo:           cargo install a1-gateway
   Or with npm:             npx a1-gateway
   Or with pip:             pip install a1identity && a1-gateway

2. START A1
   Run: a1-gateway
   It starts on http://localhost:8080 by default.
   In A1 Studio, go to "Start / Stop" to manage this.

3. PROTECT MY AGENT (create a passport)
   Go to "Protect My Agent" in the sidebar.
   Describe what your agent does → A1 suggests the right capabilities.
   Give it a name, set how long it should be valid (TTL).
   Click "Issue Passport" → saves a .json file to ~/.a1/passports/

4. CONNECT YOUR AGENT
   Go to "Connect Agents".
   A1 scans your machine for Claude Code, ChatGPT, LangChain, etc.
   Click the agent → A1 writes the connection config automatically.
   No code required for detected agents.

5. TEST THE CONNECTION
   Go to "Test Connection" and send a test message.
   A green result means your agent is now A1-protected.`,
  },
  {
    id: 'passports',
    category: 'Passports',
    title: 'Passports — what they are and how they work',
    tags: ['passport', 'credential', 'identity', 'certificate', 'what is a passport'],
    body: `A Passport is the core object in A1. It is a JSON file signed with Ed25519 that contains:

- namespace: a unique name for the agent (e.g. "trading-bot")
- capabilities: a list of named permissions (e.g. ["trade.equity", "portfolio.read"])
- expiration: Unix timestamp after which the passport is invalid
- public_key: the agent's Ed25519 public key
- issuer_signature: the human issuer's signature over the above

Passports are stored as files, typically in ~/.a1/passports/
The gateway validates every request against the passport before allowing it.

Creating a passport:
  CLI: a1 passport issue --namespace my-agent --caps read,write --ttl 30d
  Studio: "Protect My Agent" tab → fill in the form → click Issue

Renewing a passport:
  CLI: a1 passport renew --path ~/.a1/passports/my-agent.json --ttl 30d
  Studio: "Passport Vault" → expand the card → click Renew

Revoking a passport:
  CLI: a1 passport revoke --namespace my-agent
  Studio: "Passport Vault" → expand the card → click Revoke

After revoking, the agent is blocked immediately — no restart required.`,
  },
  {
    id: 'capabilities',
    category: 'Passports',
    title: 'Capabilities — what they mean',
    tags: ['capabilities', 'permissions', 'scopes', 'what can my agent do', 'cap'],
    body: `Capabilities are named permission strings. An agent can only perform actions listed in its passport. Common capability names:

files.read       — read files from disk
files.write      — write or modify files
email.send       — send emails
email.read       — read/search emails
web.search       — perform web searches
web.browse       — load web pages
trade.equity     — execute stock/equity trades
trade.crypto     — execute crypto trades
trade.polymarket — place prediction market bets
portfolio.read   — read portfolio / wallet balances
payments.send    — send money / invoke payment APIs
social.post      — post to social media (Twitter/X, LinkedIn)
code.execute     — run code or scripts
shell.exec       — execute shell commands
database.read    — read from databases
database.write   — write to databases
api.call         — make arbitrary HTTP API calls
calendar.write   — create or modify calendar events
agent.delegate   — issue sub-passports to other agents
memory.write     — write to long-term memory / vector stores
compute.run      — spawn cloud compute jobs

You can define custom capabilities for your own tools — any string works.
The gateway blocks any capability not listed in the passport at the time of the request.`,
  },
  {
    id: 'delegation-chain',
    category: 'Passports',
    title: 'Delegation chains — how recursive delegation works',
    tags: ['chain', 'delegation', 'recursive', 'sub-passport', 'swarm', 'delegate'],
    body: `The delegation chain is the heart of A1's security model.

When a human issues a passport to an orchestrator agent, and that orchestrator needs to delegate work to a sub-agent, A1 requires:

1. The orchestrator holds a passport with the "agent.delegate" capability.
2. The orchestrator calls the A1 gateway to issue a sub-passport scoped to exactly the capabilities the sub-agent needs.
3. The sub-passport is cryptographically signed by the orchestrator's key AND references the root human authorization.
4. The gateway verifies the entire chain before executing any action by the sub-agent.

This produces a SignedChain: a JSON object that contains every link from human → orchestrator → sub-agent, with cryptographic proofs at each step. It is an irrefutable audit trail.

Creating a sub-passport (CLI):
  a1 passport sub --parent ~/.a1/passports/orchestrator.json --caps files.read --ttl 1h --namespace sub-agent

Creating a swarm (many agents under one root):
  Use the "Swarms" tab in Studio, or the CLI command: a1 swarm create`,
  },
  {
    id: 'error-nonce-consumed',
    category: 'Errors',
    title: 'Error: nonce consumed / NonceAlreadyUsed',
    tags: ['error', 'nonce', 'replay', 'NonceAlreadyUsed', 'nonce consumed'],
    body: `What it means: The same request was sent twice. A1 tracks nonces (unique request IDs) to prevent replay attacks. Each request can only be authorized once.

Plain English: Your agent tried to do the same thing twice with the same authorization token.

How to fix:
1. Do NOT retry the same request with the same nonce.
2. Your agent's code should generate a fresh nonce for each new request.
3. If using the SDK: the a1.guard() context manager handles nonces automatically — make sure you are not reusing the same guard context.
4. If you see this in a loop: check your agent for retry logic that isn't generating new authorization tokens.

This is NOT a configuration error. It is a security feature working correctly.`,
  },
  {
    id: 'error-capability-not-granted',
    category: 'Errors',
    title: 'Error: CapabilityNotGranted',
    tags: ['error', 'capability', 'permission', 'denied', 'CapabilityNotGranted', 'not granted'],
    body: `What it means: The agent tried to perform an action that is not in its passport's capability list.

Plain English: Your agent tried to do something it was not given permission to do.

How to fix:
1. Go to "Passport Vault" — look at the capabilities on the passport being used.
2. If the capability is missing: go to "Protect My Agent", create a new passport with the required capability, and update your agent's passport path.
3. If you are not sure what capability name to use: go to "AI Integration" → "Code Patcher" for suggestions, or ask this assistant.
4. Renewing the passport does NOT add new capabilities — you must issue a new passport.

Example: If your agent calls email.send but the passport only has ["files.read"], you will see this error.`,
  },
  {
    id: 'error-certificate-expired',
    category: 'Errors',
    title: 'Error: CertificateExpired',
    tags: ['error', 'expired', 'expiry', 'renew', 'CertificateExpired', 'expire'],
    body: `What it means: The passport has passed its TTL (time-to-live). It is no longer valid.

Plain English: Your agent's permission slip has expired.

How to fix:
1. Go to "Passport Vault" in Studio.
2. Find the expired passport (it shows a red 🔴 indicator).
3. Select a new TTL (7 days, 30 days, etc.) and click "Renew passport".
4. Restart your agent so it picks up the renewed passport file.

To prevent this in future: set a longer TTL when issuing the passport, or set up a renewal reminder.`,
  },
  {
    id: 'error-invalid-signature',
    category: 'Errors',
    title: 'Error: InvalidSignature',
    tags: ['error', 'signature', 'key', 'InvalidSignature', 'tampered', 'invalid'],
    body: `What it means: The cryptographic signature on the passport or chain does not verify. The passport file may have been modified, corrupted, or signed with a different key.

Plain English: The permission slip has been tampered with or is from a different system.

How to fix:
1. Do NOT try to edit the passport JSON file manually — this will always produce an invalid signature.
2. Issue a new passport: go to "Protect My Agent" and create a fresh one.
3. Check that the signing key (A1_SIGNING_KEY_HEX) on the gateway has not changed since the passport was issued. If you regenerated the gateway key, all existing passports are invalid.
4. If you migrated servers: re-issue all passports on the new server.`,
  },
  {
    id: 'error-narrowing-violation',
    category: 'Errors',
    title: 'Error: narrowing violation',
    tags: ['error', 'narrowing', 'scope', 'sub-passport', 'delegation', 'violation'],
    body: `What it means: A sub-passport was issued with capabilities that exceed the parent passport. In A1, a sub-passport can only have a SUBSET of the parent's capabilities — never more.

Plain English: The delegated agent was given more permissions than the person who delegated them actually has. A1 blocks this.

How to fix:
1. Check the parent passport's capabilities.
2. Re-issue the sub-passport with only capabilities that are present in the parent.
3. If you need more capabilities in the sub-passport, first issue a new parent passport with those capabilities.

Example: Parent has ["files.read"]. Trying to issue a sub with ["files.read", "email.send"] will fail with a narrowing violation.`,
  },
  {
    id: 'error-gateway-not-running',
    category: 'Errors',
    title: 'Error: Connection refused / gateway not running',
    tags: ['error', 'connection', 'refused', 'not running', 'start', 'gateway', 'connection refused'],
    body: `What it means: A1 Studio cannot reach the A1 gateway process on localhost:8080.

Plain English: The A1 background service is not running.

How to fix:
1. Go to "Start / Stop" in the Studio sidebar.
2. Click "Start A1" and follow the instructions shown.
3. If A1 is installed: run "a1-gateway" in a terminal.
4. If A1 is not installed: run the install script: curl -fsSL https://raw.githubusercontent.com/dyologician/a1/main/install.sh | sh

To auto-start A1 on login:
  macOS: a1-gateway --install-launchd
  Linux: a1-gateway --install-systemd
  Windows: a1-gateway --install-service`,
  },
  {
    id: 'error-namespace-mismatch',
    category: 'Errors',
    title: 'Error: namespace mismatch',
    tags: ['error', 'namespace', 'mismatch', 'wrong agent', 'namespace mismatch'],
    body: `What it means: The passport being used has a different namespace than what the gateway expects for this agent slot.

Plain English: The wrong permission slip is being used for this agent.

How to fix:
1. Check the namespace in the passport file (open the JSON and look for the "namespace" field).
2. Check what namespace your agent is configured to use.
3. If they don't match: either re-issue the passport with the correct namespace, or update your agent's configuration to point to the correct passport file.`,
  },
  {
    id: 'local-llm',
    category: 'Integrations',
    title: 'Connecting A1 to a local LLM (Ollama, LM Studio, llama.cpp)',
    tags: ['local', 'llm', 'ollama', 'lm studio', 'llama', 'offline', 'private', 'local llm'],
    body: `A1 can work with completely local AI models — no cloud, no API key, no data leaving your machine.

Supported local runtimes:
  Ollama      — http://localhost:11434 — download from ollama.com
  LM Studio   — http://localhost:1234  — download from lmstudio.ai
  llama.cpp   — http://localhost:8000  — github.com/ggerganov/llama.cpp

In A1 Studio, go to "Local AI" tab. A1 auto-detects which runtimes are running and shows which models are available. Click a provider to generate ready-to-paste code.

Python + Ollama example:
  import a1, ollama
  passport = a1.Passport.load("~/.a1/passports/my-agent.json")
  with a1.guard(passport, capabilities=["files.read"]):
      response = ollama.chat(model="llama3", messages=[{"role":"user","content":"Hello"}])

Python + LM Studio example:
  from openai import OpenAI
  import a1
  client = OpenAI(base_url="http://localhost:1234/v1", api_key="local")
  passport = a1.Passport.load("~/.a1/passports/my-agent.json")
  with a1.guard(passport, capabilities=["files.read"]):
      response = client.chat.completions.create(model="local-model", messages=[{"role":"user","content":"Hello"}])

The A1 authorization check happens regardless of which model backend you use.`,
  },
  {
    id: 'sdk-python',
    category: 'SDKs',
    title: 'Python SDK — integration guide',
    tags: ['python', 'sdk', 'integration', 'pip', 'langchain', 'openai', 'python sdk'],
    body: `Install: pip install a1identity

Basic usage:
  import a1
  client = a1.Client(gateway_url="http://localhost:8080")
  passport = a1.Passport.load("~/.a1/passports/my-agent.json")
  with a1.guard(passport, capabilities=["files.read"]):
      # Your agent code here — A1 protects this block

LangChain integration:
  from a1.langchain_tool import a1_langchain_guard
  @a1_langchain_guard(passport, capabilities=["web.search"])
  def search_tool(query: str) -> str:
      return do_search(query)

OpenAI Agents integration:
  from a1.openai_tool import a1_openai_guard
  @a1_openai_guard(passport, capabilities=["email.send"])
  def send_email(to: str, body: str): ...

CrewAI integration:
  from a1.crewai_tool import A1Tool
  tool = A1Tool(fn=my_fn, passport=passport, capability="files.write")

Middleware (FastAPI / Flask):
  from a1.middleware import A1Middleware
  app.add_middleware(A1Middleware, gateway_url="http://localhost:8080")`,
  },
  {
    id: 'sdk-typescript',
    category: 'SDKs',
    title: 'TypeScript / Node SDK — integration guide',
    tags: ['typescript', 'javascript', 'node', 'sdk', 'npm', 'typescript sdk'],
    body: `Install: npm install a1-ai

Basic usage:
  import { A1Client, loadPassport, guard } from "a1";
  const passport = await loadPassport("~/.a1/passports/my-agent.json");
  await guard(passport, ["files.read"], async () => {
    // Your agent code here
  });

OpenAI Agents (TypeScript):
  import { withA1Passport, PassportClient } from "a1/passport";
  const client = new PassportClient({ gatewayUrl: "http://localhost:8080", passportPath: "./passport.json" });
  const safeTool = withA1Passport(myTool, { client, capability: "email.send" });

Express middleware:
  import { a1Middleware } from "a1/middleware";
  app.use(a1Middleware({ gatewayUrl: "http://localhost:8080" }));

MCP integration (.mcp.json):
  { "mcpServers": { "a1": { "type": "http", "url": "http://localhost:8080/mcp" } } }`,
  },
  {
    id: 'enterprise',
    category: 'Enterprise',
    title: 'Enterprise deployment',
    tags: ['enterprise', 'production', 'deploy', 'docker', 'kubernetes', 'kms', 'soc2', 'enterprise'],
    body: `A1 is production-ready for enterprise deployment.

Docker:
  docker run -e A1_SIGNING_KEY_HEX=<key> -p 8080:8080 ghcr.io/dyologician/a1:latest

Docker Compose (with Redis + Postgres):
  See docker/docker-compose.yml in the repository.

Environment variables:
  A1_SIGNING_KEY_HEX   — your gateway signing key (generate with: a1 keygen)
  A1_DB_URL            — Postgres connection string (optional; defaults to SQLite)
  A1_REDIS_URL         — Redis URL for distributed nonce tracking
  A1_ADMIN_SECRET      — secret for admin API calls from Studio
  A1_PORT              — port to listen on (default: 8080)
  A1_HOST              — bind address (default: 127.0.0.1; use 0.0.0.0 for all interfaces)

KMS integration (AWS KMS, Azure Key Vault, HashiCorp Vault):
  A1 supports signing keys stored in external KMS systems.
  See docs/enterprise-kms.md in the repository.

Compliance:
  SOC 2 mapping:     docs/compliance/soc2-mapping.md
  ISO 27001 mapping: docs/compliance/iso27001-mapping.md
  EU AI Act mapping: use "Compliance" tab in Studio

High-availability:
  Run multiple gateway instances behind a load balancer.
  Use Postgres + Redis for shared state.
  All gateway instances must share the same A1_SIGNING_KEY_HEX.`,
  },
  {
    id: 'backup-restore',
    category: 'Setup',
    title: 'Backup and restore passports',
    tags: ['backup', 'restore', 'export', 'import', 'migrate', 'switch computers', 'backup passports'],
    body: `To avoid losing your passports when switching computers or reinstalling:

BACKUP (export):
  In Studio: go to "Passport Vault" → click "Export Backup".
  This downloads a single .a1-backup.json file containing all your passports.

  CLI alternative: cp -r ~/.a1/passports/ ~/my-a1-backup/

RESTORE (import):
  In Studio: go to "Passport Vault" → click "Import Backup" → select your .a1-backup.json file.
  The gateway will restore all passports to ~/.a1/passports/ and re-register them.

  CLI alternative: cp ~/my-a1-backup/*.json ~/.a1/passports/

Important: The backup file contains signed passport credentials. Keep it safe.
The backup does NOT contain your signing key (A1_SIGNING_KEY_HEX) — back that up separately.`,
  },
  {
    id: 'mcp',
    category: 'Integrations',
    title: 'MCP (Model Context Protocol) integration',
    tags: ['mcp', 'model context protocol', 'claude', 'claude code', 'cursor', 'mcp integration'],
    body: `A1 exposes an MCP endpoint at http://localhost:8080/mcp

To connect Claude Code or any MCP-compatible agent:

Add to .mcp.json in your project root:
  {
    "mcpServers": {
      "a1": {
        "type": "http",
        "url": "http://localhost:8080/mcp"
      }
    }
  }

In Studio: go to "Direct Connect" tab for auto-generated MCP config, or "Connect Agents" for one-click setup if Claude Code is detected.

The MCP integration exposes tools for:
  - Checking authorization
  - Issuing and verifying passports
  - Reading the audit trail
  - Querying the delegation chain`,
  },
  {
    id: 'restart-after-patch',
    category: 'Setup',
    title: 'Why agents don\'t change after patching — restart required',
    tags: ['restart', 'patch', 'not working', 'nothing changed', 'agent not updated', 'refresh'],
    body: `After A1 patches your agent (writes to its config file or code), you must restart the agent for changes to take effect. This is a common missed step.

HOW TO RESTART EACH AGENT TYPE:

Claude Code:
  Close the Claude Code terminal and re-open it. Or press Ctrl+C and run it again.

Python script:
  Stop the script (Ctrl+C) and run it again: python your_agent.py

LangChain / CrewAI / LangGraph:
  Stop the process and restart it. If using uvicorn/FastAPI: Ctrl+C and uvicorn main:app --reload

Node.js / TypeScript agent:
  Ctrl+C and: node your_agent.js  (or: npm run dev / npx ts-node your_agent.ts)

OpenAI Assistants:
  You don't restart the assistant itself — you restart whatever script/app is calling the Assistants API.

Docker container:
  docker compose restart

If your agent still doesn't respond after restart, check:
1. The A1 gateway is running (green dot in sidebar)
2. The passport path in your config points to the right file
3. The chain.json file exists where your agent expects it`,
  },
  {
    id: 'production-features',
    category: 'Setup',
    title: 'Redis, Postgres, KMS, compliance — do I need these?',
    tags: ['redis', 'postgres', 'kms', 'production', 'compliance', 'advanced', 'enterprise', 'do I need', 'personal use'],
    body: `SHORT ANSWER: No. For personal use, the default A1 setup works perfectly. You do not need Redis, Postgres, KMS, or compliance reports.

LONGER ANSWER — here is what each thing is for:

Redis / Postgres (persistent nonce + revocation storage)
  Default (in-memory): works great for personal use. State resets when A1 restarts, but this only matters for replay-attack prevention — your passports and chains still work.
  When you need it: team deployments where multiple gateway instances share state, or high-traffic production where nonce history must survive restarts.

KMS (AWS Key Management Service / HashiCorp Vault / Azure Key Vault)
  Default (local key file): your signing key is a file on your machine. Fine for personal use.
  When you need it: teams where multiple people issue passports, or compliance requirements that mandate hardware-backed key storage.

Admin secrets / A1_ADMIN_SECRET
  Default: no admin auth required (you're running it locally).
  When you need it: if you expose the A1 gateway to the internet or to other machines on your network.

Compliance reports (SOC 2 mapping, ISO 27001)
  These are for enterprise security audits. Individuals don't need them.
  If you're setting up A1 for a company, ask your security team.

Rate limits (A1_RATE_LIMIT_RPS)
  Default: no rate limiting.
  When you need it: if your gateway is exposed to external traffic.

RULE OF THUMB: If you're running A1 on your own laptop for your own agents, ignore all of the above. The defaults are production-quality for single-user local deployments.`,
  },
  {
    id: 'community-dyolo-x',
    category: 'Community',
    title: 'Creator & Community — Dyolo on X, Reddit, GitHub',
    tags: ['creator', 'dyolo', 'x', 'twitter', 'reddit', 'github', 'community', 'author', 'social', 'follow', 'dyologician', 'handles', 'username'],
    body: `A1 is built and maintained by Dyolo. The creator's handle is @dyologician on every platform.

CORRECT HANDLES — @dyologician everywhere:
  X (Twitter):  https://x.com/dyologician
  GitHub:       https://github.com/dyologician
  Reddit:       u/dyologician

IMPORTANT: There is no @dyolo handle, no "r/dyolo" subreddit, and no person named "Daniel Bowen" associated with A1. The creator is Dyolo / @dyologician only.

FOLLOW ON X / TWITTER:
  https://x.com/dyologician
  Posts early-access feature drops, development demos, quick A1 tips, and release announcements.

GITHUB (main repo):
  https://github.com/dyologician/a1
  File a bug:    github.com/dyologician/a1/issues → New Issue
  Discussions:   github.com/dyologician/a1/discussions  (Q&A, ideas, show & tell)
  Changelog:     CHANGELOG.md — every release and breaking change
  Protocol spec: spec/A1-PROTOCOL.md — full cryptographic reference
  Contributing:  CONTRIBUTING.md — how to run tests and submit PRs

REDDIT:
  The creator is u/dyologician on Reddit.
  There is no dedicated A1 subreddit. Related communities:
  r/LocalLLaMA, r/AIAgents, r/rust, r/selfhosted, r/MachineLearning

COMMUNITY CHAT:
  Primary async forum: https://github.com/dyologician/a1/discussions
  Real-time: search X for #A1Protocol or tag @dyologician

SECURITY CONTACT:
  workwithdyolo@gmail.com — do NOT use GitHub Issues for security reports`,
  },
  {
    id: 'local-llm-models-recommended',
    category: 'Integrations',
    title: 'Recommended local models — Gemma, Llama, Mistral and more',
    tags: ['gemma', 'gemma3', 'llama', 'mistral', 'model', 'recommend', 'which model', 'best model', 'google', 'local llm', 'phi', 'qwen', 'pull model', 'ollama pull'],
    body: `A1 works with any local model. Here are top picks by use case, and how to pull them.

★ RECOMMENDED STARTING MODEL:
  gemma3          — Google Gemma 3 (4B). Fast, accurate, runs on 8GB RAM.
                    Excellent at following A1's structured system prompts and explaining errors.
  Pull it: ollama pull gemma3

ALL-AROUND PICKS:
  llama3.2        — Meta Llama 3.2 3B. Extremely fast; great for low-RAM machines.
  llama3.1:8b     — Meta Llama 3.1 8B. Best balance of speed and intelligence.
  mistral         — Mistral 7B. Reliable, fast, consistent instruction following.

BEST FOR CODE / INTEGRATION TASKS:
  qwen2.5-coder:7b  — Top open-source coder; excellent for A1 Python / TypeScript patches.
  deepseek-coder-v2 — Strong at reading and writing Rust (A1 internals).
  phi4              — Microsoft Phi-4. Punches above its size class for reasoning.

HIGH-QUALITY (needs more RAM):
  gemma3:27b      — Google Gemma 3 27B. Best quality if you have 24GB+ VRAM.
  llama3.1:70b    — For maximum intelligence on high-end hardware.

PULL COMMANDS (copy-paste into terminal, or use the Pull button in this tab):
  ollama pull gemma3              ← start here
  ollama pull llama3.2
  ollama pull qwen2.5-coder:7b
  ollama pull mistral

WHY GEMMA IS RECOMMENDED:
  Google Gemma 3 (4B) hits the sweet spot for A1 assistance:
  - Excellent instruction-following in A1's full knowledge-base system prompt
  - Good code comprehension for explaining integration errors
  - Runs on any laptop with 8GB RAM — no GPU required (CPU mode)
  - Open weights from Google, pulled via Ollama with no account needed`,
  },
  {
    id: 'technical-zk-proofs',
    category: 'Technical',
    title: 'Zero-knowledge proofs in A1 — how ZK authorization works',
    tags: ['zk', 'zero knowledge', 'proof', 'zkp', 'risc0', 'cryptography', 'technical', 'zk proof', 'privacy'],
    body: `A1 supports zero-knowledge proofs for privacy-preserving authorization. ZK proofs let an agent prove it holds a valid credential without revealing the credential itself.

HOW ZK WORKS IN A1:
  1. The agent generates a ZK proof inside the A1 Rust library (RISC Zero VM).
  2. The proof is attached to the authorization request instead of the raw passport.
  3. The A1 gateway verifies the proof without ever seeing the credential contents.
  4. Result: an agent can prove "I am authorized for trade.equity" while keeping its passport private.

IMPLEMENTATION DETAILS:
  - Proving backend: RISC Zero (risc0.com)
  - Guest program: src/zk_guest/src/main.rs — runs inside the ZK VM
  - Host program:  src/zk.rs — manages proof generation and submission
  - Proof size: ~200KB. Verification time: <10ms on modern hardware.

ENABLE ZK MODE:
  Set A1_ZK_MODE=true in the gateway environment.
  Default is signature-only mode (faster, simpler — sufficient for most users).

WHEN TO USE ZK:
  - Agents that must not reveal which human issued their credentials
  - Cross-organization verification where credential contents are sensitive
  - Privacy-critical financial or trading agents
  - Compliance scenarios requiring minimal disclosure

Most users do not need ZK mode. Ed25519 signatures are the default and cover 99% of use cases.`,
  },
  {
    id: 'technical-ed25519',
    category: 'Technical',
    title: 'Ed25519 signatures — the cryptography behind A1 passports',
    tags: ['ed25519', 'signature', 'cryptography', 'key', 'signing', 'public key', 'technical', 'crypto', 'elliptic curve'],
    body: `A1 uses Ed25519 for signing passports and authorization chains. Ed25519 is a modern elliptic-curve signature scheme built for speed and security.

WHY ED25519:
  - 128-bit security level (resistant to all known classical attacks)
  - Very fast: sign and verify in microseconds
  - Compact: 32-byte public keys, 64-byte signatures
  - Deterministic: same message + key always produces the same signature
  - Widely audited — used by Signal, OpenSSH, TLS 1.3, and many others

KEY GENERATION:
  CLI: a1 keygen
  Prints a hex-encoded 64-byte keypair. Store the first 32 bytes as A1_SIGNING_KEY_HEX.

HOW PASSPORT SIGNING WORKS:
  1. The gateway serializes passport fields (namespace, capabilities, TTL, public key) into a canonical binary blob.
  2. The gateway signs this blob with its Ed25519 signing key.
  3. The issuer_signature field in the JSON stores this 64-byte hex signature.
  4. On every request, the gateway re-serializes the passport and calls Ed25519::verify(). Any modification — even adding a space — invalidates the signature.

NEVER:
  - Edit a passport JSON file manually. Even a single changed byte breaks the signature.
  - Share your A1_SIGNING_KEY_HEX. It is a private key.
  - Reuse the same signing key across different gateways. Use separate keys per environment.`,
  },
  {
    id: 'technical-did',
    category: 'Technical',
    title: 'DIDs (Decentralized Identifiers) — agent identity in A1',
    tags: ['did', 'decentralized', 'identifier', 'w3c', 'did:a1', 'identity', 'technical', 'well-known'],
    body: `A1 implements W3C DID (Decentralized Identifiers) for agent identity. A DID is a globally unique, cryptographically verifiable ID that requires no central registry.

A1 DID FORMAT:
  did:a1:<namespace>:<fingerprint>
  Example:  did:a1:trading-bot:9f3a1e7b

WHERE DIDS ARE USED:
  - Each agent gets a DID derived from its namespace and public key fingerprint.
  - DIDs appear in the delegation chain as the subject at each link.
  - The .well-known endpoint on the gateway exposes DID documents for external verification.

RESOLVE A DID DOCUMENT:
  GET http://localhost:8080/.well-known/did.json
  Returns a standard W3C DID document with the agent's Ed25519 verification key.

EXAMPLE DID DOCUMENT:
  {
    "@context": "https://www.w3.org/ns/did/v1",
    "id": "did:a1:trading-bot:9f3a1e7b",
    "verificationMethod": [{
      "id": "did:a1:trading-bot:9f3a1e7b#key-0",
      "type": "Ed25519VerificationKey2020",
      "publicKeyMultibase": "z..."
    }]
  }

WHY IT MATTERS:
  External systems can verify an A1 agent's identity without contacting the gateway — they only need the public DID document. This enables cross-organization agent-to-agent trust without a shared backend.

Most users don't interact with DIDs directly — A1 handles them automatically. This matters for advanced cross-org integrations.`,
  },
  {
    id: 'technical-rust-crate',
    category: 'Technical',
    title: 'Using A1 as a Rust crate — embed authorization in-process',
    tags: ['rust', 'crate', 'library', 'cargo', 'rust sdk', 'technical', 'advanced', 'embed', 'in-process'],
    body: `A1 is a Rust library first. You can embed it directly in your Rust agent — no separate gateway process needed.

ADD TO Cargo.toml:
  [dependencies]
  a1 = "0.1"

BASIC USAGE:
  use a1::{Passport, Chain, guard};

  let passport = Passport::load("~/.a1/passports/my-agent.json")?;
  let chain    = guard(&passport, &["trade.equity"])?;
  let proof    = serde_json::to_string(&chain)?; // attach to downstream calls

KEY MODULES:
  a1::passport   — Create, load, sign, verify passports
  a1::chain      — Build and verify delegation chains
  a1::intent     — Construct and sign action intents
  a1::zk         — Zero-knowledge proof generation (feature-gated)
  a1::negotiate  — Agent-to-agent trust negotiation
  a1::anchor     — Anchor chains to external registries
  a1::ffi        — C FFI bindings for Python/Go/other language interop

CARGO FEATURES:
  default    — Core library (no ZK, no DB)
  zk         — Enable ZK proofs (requires RISC Zero toolchain)
  postgres   — Postgres nonce backend
  redis      — Redis distributed nonce backend

CBINDGEN (C header generation):
  cbindgen --config cbindgen.toml --crate a1 --output a1.h

Source: src/ directory in the repository. Protocol reference: spec/A1-PROTOCOL.md`,
  },
  {
    id: 'technical-ai-proxy',
    category: 'Technical',
    title: 'AI Proxy — route LLM calls through A1 for automatic authorization',
    tags: ['ai proxy', 'proxy', 'openai', 'anthropic', 'route', 'technical', 'gateway', 'intercept', 'llm calls'],
    body: `A1 includes an AI proxy endpoint that intercepts LLM API calls and automatically injects A1 authorization — no code changes needed in your agent.

HOW IT WORKS:
  Instead of calling api.openai.com directly, your agent calls http://localhost:8080/v1/ (the A1 gateway's proxy route). A1 verifies the passport on every request, then forwards the call to the real LLM provider.

SETUP:
  1. Set your agent's base URL to http://localhost:8080/v1/
  2. Keep your real API key in the A1 gateway (A1_OPENAI_API_KEY env var).
  3. Your agent passes its passport in the X-A1-Passport header.

PYTHON EXAMPLE:
  from openai import OpenAI
  client = OpenAI(base_url="http://localhost:8080/v1/", api_key="passthrough")
  # A1 gateway validates the passport, then forwards to OpenAI automatically

SUPPORTED PROVIDERS (via proxy):
  OpenAI:    A1_OPENAI_API_KEY + proxy to api.openai.com
  Anthropic: A1_ANTHROPIC_API_KEY + proxy to api.anthropic.com
  Any OpenAI-compatible endpoint: set A1_PROXY_TARGET in gateway config

BENEFITS:
  - Zero code changes to existing agents — just change the base URL
  - Every LLM call is capability-checked before it reaches the provider
  - Full audit trail of which agent called which model with what prompt (metadata only, not content)
  - Works with local LLMs too: proxy http://localhost:11434 through A1

See: a1-gateway/src/routes/ai_proxy.rs for the implementation.`,
  },
  {
    id: 'custom-agent-prompting',
    category: 'Integrations',
    title: 'How to describe your custom agent to the AI Integration Assistant',
    tags: ['custom agent', 'describe project', 'AI assistant', 'integration', 'my project', 'how to explain', 'vague'],
    body: `The AI Integration Assistant works best when you give it specific details about your project. Vague descriptions lead to back-and-forth; specific ones get you working code in one shot.

WHAT TO INCLUDE IN YOUR DESCRIPTION:

1. What language and framework your agent uses
   Good: "Python LangChain agent"
   Good: "TypeScript script using the OpenAI Assistants API"
   Bad: "my AI thing"

2. Where your agent's code lives
   Good: "My tools are defined in ~/myagent/skills/tools.py"
   Good: "The main agent file is ./agent.js"
   Bad: "somewhere in my project"

3. What the protected tools do
   Good: "I have a function called execute_trade(symbol, qty) that places stock orders"
   Good: "A tool called send_email(to, subject, body) that calls Gmail API"

4. What capabilities you want to protect
   Good: "I want to require authorization for trade.equity and email.send"

EXAMPLE PROMPT THAT WORKS WELL:
"I have a Python LangChain agent in ~/myagent/agent.py. It has two tools: execute_trade(symbol, qty) for stock trades and read_portfolio() for checking balances. My passport is at ~/myagent/passport.json. Please add A1 authorization to both tools — trade.equity for execute_trade and portfolio.read for read_portfolio."

EXAMPLE PROMPT THAT NEEDS MORE INFO:
"Add A1 to my agent"  ← The assistant will ask follow-up questions.`,
  },
];

const KB_CATEGORIES = ['Overview', 'Setup', 'Passports', 'Errors', 'Integrations', 'SDKs', 'Enterprise', 'Community', 'Technical'];

const KB_CATEGORY_COLORS = {
  Overview:     { bg: 'rgba(99,102,241,.12)',  border: 'rgba(99,102,241,.3)',  text: '#818cf8' },
  Setup:        { bg: 'rgba(34,197,94,.1)',    border: 'rgba(34,197,94,.28)', text: '#4ade80' },
  Passports:    { bg: 'rgba(251,146,60,.1)',   border: 'rgba(251,146,60,.28)', text: '#fb923c' },
  Errors:       { bg: 'rgba(239,68,68,.1)',    border: 'rgba(239,68,68,.28)', text: '#f87171' },
  Integrations: { bg: 'rgba(34,211,238,.1)',   border: 'rgba(34,211,238,.28)', text: '#22d3ee' },
  SDKs:         { bg: 'rgba(168,85,247,.1)',   border: 'rgba(168,85,247,.28)', text: '#c084fc' },
  Enterprise:   { bg: 'rgba(250,204,21,.1)',   border: 'rgba(250,204,21,.28)', text: '#fbbf24' },
  Community:    { bg: 'rgba(236,72,153,.1)',   border: 'rgba(236,72,153,.28)', text: '#f472b6' },
  Technical:    { bg: 'rgba(20,184,166,.1)',   border: 'rgba(20,184,166,.28)', text: '#2dd4bf' },
};

// ── Casual conversation patterns ──────────────────────────────────────────────

const _GREETINGS = /^(hi|hey|hello|howdy|sup|yo|good\s*(morning|afternoon|evening|day)|what'?s\s*up|hiya|greetings)[!?.]*$/i;
const _THANKS    = /^(thanks?|thank\s*you|thx|ty|cheers|appreciated?|great|awesome|perfect|nice|cool|got\s*it|got\s*that|understood|makes?\s*sense|ok(ay)?|makes?\s*sense|that\s*(works?|helps?)|brilliant|excellent|good\s*(to\s*know)?|helpful)[!?.]*$/i;
const _FAREWELL  = /^(bye|goodbye|see\s*ya|later|ciao|take\s*care|ttyl|gotta\s*go)[!?.]*$/i;
const _HELP_ME   = /^(help|what\s*can\s*you\s*do|how\s*do\s*you\s*work|what\s*do\s*you\s*know|capabilities|menu|options)[?!.]*$/i;

function _casual(text) {
  const t = text.trim();
  if (_GREETINGS.test(t))
    return `Hey! I'm the A1 knowledge assistant — here to help with anything A1-related.\n\nYou can ask me things like:\n• "How do I get started?"\n• "What is a passport?"\n• "Fix: CapabilityNotGranted"\n• "Which local model should I use?"\n• "Where is Dyolo on X / Reddit / GitHub?"\n\nOr click one of the quick chips below. What can I help you with?`;
  if (_THANKS.test(t))
    return `You're welcome! Anything else you'd like to know about A1?`;
  if (_FAREWELL.test(t))
    return `Take care! Come back any time you have questions about A1.`;
  if (_HELP_ME.test(t))
    return `I can answer questions about:\n\n📘 **Setup** — installing and starting A1\n🛂 **Passports** — creating, renewing, revoking credentials\n⚠️ **Errors** — plain-English explanations for every error code\n🔗 **Integrations** — Ollama, Gemma, MCP, LangChain, CrewAI, OpenAI\n📦 **SDKs** — Python, TypeScript, Go usage\n🏢 **Enterprise** — Docker, Kubernetes, KMS, compliance\n🌐 **Community** — Dyolo on X / Creator X, Reddit, GitHub\n🔬 **Technical** — Ed25519, ZK proofs, DIDs, Rust crate, AI Proxy\n\nFor smarter, context-aware answers, connect a local LLM in the \"Connect LLM\" tab. What would you like to know?`;
  return null;
}

// ── KB search ─────────────────────────────────────────────────────────────────

function _kbSearch(text) {
  const lower = text.toLowerCase();
  const scored = KB.map(entry => {
    let score = 0;
    if (entry.title.toLowerCase().includes(lower)) score += 10;
    entry.tags.forEach(t => { if (lower.includes(t) || t.includes(lower)) score += 5; });
    const words = lower.split(/\s+/).filter(w => w.length > 3);
    words.forEach(w => {
      if (entry.title.toLowerCase().includes(w)) score += 3;
      if (entry.tags.some(t => t.includes(w))) score += 2;
      if (entry.body.toLowerCase().includes(w)) score += 1;
    });
    return { entry, score };
  });
  scored.sort((a, b) => b.score - a.score);
  return scored[0]?.score > 0 ? scored[0].entry : null;
}

// ── System prompt builder ─────────────────────────────────────────────────────

function buildSystemPrompt() {
  const sections = KB.map(e => `## ${e.title}\n\n${e.body}`).join('\n\n---\n\n');
  return `You are the A1 Knowledge Assistant (instance ${_KB_NS}). Your sole job is to help users understand, set up, and troubleshoot A1.

════════════════════════════════════════
GROUND TRUTH — MEMORIZE THESE FACTS. THEY OVERRIDE ANYTHING YOU THINK YOU KNOW.
════════════════════════════════════════
- A1 is built and maintained by Dyolo, handle @dyologician everywhere.
- GitHub: https://github.com/dyologician/a1  (ONLY valid repo URL)
- X / Twitter: https://x.com/dyologician
- Reddit: u/dyologician
- A1 is NOT made by Anthropic. NOT made by Josh Brown. NOT made by Daniel Bowen. NOT made by OpenAI.
- "Daniel Bowen" does NOT exist in this project. Never mention that name as a real person.
- "Josh Brown" does NOT exist in this project. Never mention that name.
- There is NO "dyolo/dyolo" GitHub repo. There is NO "anthropic/a1" repo. There is NO "a1.network" website.
- There is NO dedicated r/dyolo subreddit. There is NO @dyolo Twitter handle.
- Dyolo is the creator/pseudonym — NOT a separate project or protocol.
- A1 is a Rust crate + Python/TypeScript/Go SDKs + self-hosted gateway. It is NOT a blockchain. It is NOT a DID network. It is NOT built by Anthropic.
════════════════════════════════════════
STRICT RULES — YOU MUST FOLLOW THESE WITHOUT EXCEPTION
════════════════════════════════════════
1. ONLY answer using the knowledge base provided below. Do not use your training data for A1-specific facts.
2. NEVER invent names, URLs, GitHub repos, company names, or people not in the knowledge base.
3. If a user asks something not covered in the knowledge base, say exactly: "I don't have that information in my knowledge base. Check https://github.com/dyologician/a1 or ask in GitHub Discussions."
4. NEVER say A1 was created by anyone other than Dyolo / @dyologician.
5. NEVER mention Anthropic, OpenAI, Josh Brown, Daniel Bowen, or any person/company not in the KB.
6. NEVER make up GitHub URLs. The only valid repo is https://github.com/dyologician/a1
7. NEVER describe Dyolo as a "network", "protocol", "DID system", or separate project. Dyolo is the creator's name/handle.
8. If you are uncertain about any specific fact, say so and point to the GitHub repo. Do not guess.
════════════════════════════════════════
STYLE
════════════════════════════════════════
Be conversational, friendly, and practical. Write in natural prose — no raw markdown symbols like ##, **, or triple-backtick blocks in your replies. Use plain sentences and line breaks. For simple greetings or thanks, respond in one or two sentences. When relevant, tell users which tab in A1 Studio to navigate to.

════════════════════════════════════════
KNOWLEDGE BASE (your only source of truth for A1 facts)
════════════════════════════════════════

${sections}`;
}

// ── Local LLM providers ───────────────────────────────────────────────────────

const ASST_PROVIDERS = [
  { id: 'ollama',   label: 'Ollama',    port: 11434, probe: '/api/tags',  chat: '/api/chat',          models: d => (d.models||[]).map(m=>m.name), baseUrl: 'http://localhost:11434', icon: '🦙', isOllama: true },
  { id: 'lmstudio', label: 'LM Studio', port: 1234,  probe: '/v1/models', chat: '/v1/chat/completions', models: d => (d.data||[]).map(m=>m.id),   baseUrl: 'http://localhost:1234',  icon: '🧩', isOllama: false },
  { id: 'llamacpp', label: 'llama.cpp', port: 8000,  probe: '/v1/models', chat: '/v1/chat/completions', models: d => (d.data||[]).map(m=>m.id),   baseUrl: 'http://localhost:8000',  icon: '🔩', isOllama: false },
];

async function probeProvider(p) {
  try {
    const r = await fetch(p.baseUrl + p.probe, { signal: AbortSignal.timeout(1800) });
    const d = await r.json();
    const models = p.models(d);
    return { status: models.length > 0 ? 'running' : 'empty', models };
  } catch { return { status: 'offline', models: [] }; }
}

async function chatWithOllama(baseUrl, model, messages) {
  const r = await fetch(baseUrl + '/api/chat', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ model, messages: messages.map(m => ({ role: m.role, content: m.content })), stream: false }),
  });
  const d = await r.json();
  return d.message?.content || d.response || '';
}

async function chatOpenAICompat(baseUrl, model, messages) {
  const r = await fetch(baseUrl + '/v1/chat/completions', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ model, messages, stream: false }),
  });
  const d = await r.json();
  return d.choices?.[0]?.message?.content || '';
}

// ── Reply sanitizer — catch LLM hallucinations before they reach the user ────

const _HALLUCINATION_PATTERNS = [
  // Wrong GitHub URLs
  { pattern: /github\.com\/dyolo\/a1\b/gi,          fix: 'github.com/dyologician/a1' },
  { pattern: /github\.com\/anthropic\/a1\b/gi,       fix: 'github.com/dyologician/a1' },
  { pattern: /github\.com\/a1-network\/a1\b/gi,      fix: 'github.com/dyologician/a1' },
  { pattern: /github\.com\/dyolo\/dyolo\b/gi,        fix: 'github.com/dyologician/a1' },
  // Wrong X/Twitter handles
  { pattern: /x\.com\/dyolo\b(?!gician)/gi,          fix: 'x.com/dyologician' },
  { pattern: /twitter\.com\/dyolo\b(?!gician)/gi,    fix: 'x.com/dyologician' },
  { pattern: /@dyolo\b(?!gician)/gi,                 fix: '@dyologician' },
  // Wrong Reddit
  { pattern: /reddit\.com\/r\/dyolo\b(?!gician)/gi,  fix: 'reddit.com/u/dyologician' },
  { pattern: /r\/dyolo\b(?!gician)/gi,               fix: 'u/dyologician' },
  // Wrong websites
  { pattern: /a1\.network/gi,                        fix: 'github.com/dyologician/a1' },
  { pattern: /dyolo\.ai/gi,                          fix: 'github.com/dyologician/a1' },
];

// Patterns that indicate the LLM is making up biographical facts not in the KB.
// If any of these are found, the whole response is replaced with a safe fallback.
const _FABRICATION_TRIGGERS = [
  /josh\s*brown/i,
  /daniel\s*bowen/i,
  /former\s*google\s*employee/i,
  /worked\s*at\s*google/i,
  /anthropic.*(?:creator|created|built|made|founder|team)/i,
  /(?:creator|created|built|made|founder).*anthropic/i,
  /openai.*(?:creator|created|built|made)/i,
  /real\s*name\s*(is\s*)?(unknown|never.*revealed|secret|hidden|anonymous)/i,
  /discord\s*server.*dyolo/i,
  /dyolo.*discord\s*server/i,
  /github\.com\/dyolo\/dyolo/i,
];

function sanitizeReply(text, userQuery) {
  // Step 1: check for outright fabrications — replace the whole reply
  for (const trigger of _FABRICATION_TRIGGERS) {
    if (trigger.test(text)) {
      // Try to find the right KB answer for the query
      const kbMatch = _kbSearch(userQuery || '');
      const kbAnswer = kbMatch
        ? kbMatch.body.slice(0, 600) + (kbMatch.body.length > 600 ? '\n\n(See the Knowledge Base tab for the full entry.)' : '')
        : null;
      const correction = kbAnswer
        ? `Here is the correct information from the A1 knowledge base:\n\n${kbAnswer}`
        : `I don't have reliable information about that. For accurate details, check the official repository at https://github.com/dyologician/a1 or the GitHub Discussions.`;
      console.warn('[A1 Assistant] Hallucination detected, replaced response. Trigger:', trigger.toString());
      return correction;
    }
  }

  // Step 2: fix known wrong URLs / handles in-place
  let sanitized = text;
  for (const { pattern, fix } of _HALLUCINATION_PATTERNS) {
    sanitized = sanitized.replace(pattern, fix);
  }
  return sanitized;
}

// ── Inject styles ─────────────────────────────────────────────────────────────

(function () {
  if (document.getElementById('_a1_asst_css')) return;
  const s = document.createElement('style');
  s.id = '_a1_asst_css';
  s.textContent = `
    @keyframes _a1db{0%,80%,100%{transform:translateY(0);opacity:.35}40%{transform:translateY(-5px);opacity:1}}
    ._a1d{width:7px;height:7px;border-radius:50%;background:var(--t2);display:inline-block;animation:_a1db 1.3s ease-in-out infinite;}
    ._a1d:nth-child(2){animation-delay:.18s}._a1d:nth-child(3){animation-delay:.36s}
    .kb-card{border:1px solid #333;border-radius:6px;background:#1e1e1e;overflow:hidden;transition:border-color .15s;}
    [data-theme="light"] .kb-card{background:#f0f0f0;border-color:#bbb;}
    .kb-card:hover{border-color:#555;}
    [data-theme="light"] .kb-card:hover{border-color:#888;}
    .kb-card-head{display:flex;align-items:center;gap:10px;padding:10px 14px;cursor:pointer;user-select:none;}
    .kb-title{flex:1;font-weight:600;font-size:11px;color:#fff;line-height:1.35;}
    [data-theme="light"] .kb-title{color:#111;}
    .kb-cat-badge{font-size:8px;font-weight:700;text-transform:uppercase;letter-spacing:.08em;padding:2px 7px;border-radius:10px;white-space:nowrap;flex-shrink:0;}
    .kb-body{padding:0 14px 12px;border-top:1px solid #333;font-size:9px;color:#aaa;line-height:1.75;white-space:pre-wrap;font-family:var(--mono,'IBM Plex Mono',monospace);}
    [data-theme="light"] .kb-body{border-color:#bbb;color:#555;}
    .kb-actions{display:flex;align-items:center;gap:6px;flex-shrink:0;}
    .kb-chevron{font-size:10px;color:#666;transition:transform .15s;flex-shrink:0;}
    .kb-chevron.open{transform:rotate(180deg);}
    .kb-cat-tab{padding:5px 12px;border-radius:14px;font-size:9px;font-weight:600;cursor:pointer;border:1px solid transparent;transition:all .15s;white-space:nowrap;color:#777;background:transparent;}
    .kb-cat-tab:hover{border-color:#555;color:#fff;}
    [data-theme="light"] .kb-cat-tab:hover{border-color:#999;color:#111;}
    .kb-cat-tab.active{background:#2a2a2a;border-color:#444;color:#fff;}
    [data-theme="light"] .kb-cat-tab.active{background:#ddd;border-color:#aaa;color:#111;}
    .asst-tab-strip{display:flex;border-bottom:1px solid var(--b3);margin-bottom:16px;gap:0;}
    .asst-tab{padding:7px 16px;font-size:var(--fsm);font-weight:500;color:var(--t2);background:transparent;border:none;cursor:pointer;border-bottom:2px solid transparent;transition:color .1s;white-space:nowrap;}
    .asst-tab.active{color:var(--t1);font-weight:700;border-bottom-color:var(--accent);}
    .asst-tab:hover{color:var(--t1);}
    .ai-chat-window{border:1px solid var(--b3);border-radius:var(--r);background:var(--b1);padding:12px 12px 8px;min-height:240px;max-height:420px;overflow-y:auto;display:flex;flex-direction:column;}
    .llm-prov-card{border:1px solid var(--b3);border-radius:var(--r);background:var(--b1);padding:10px 14px;cursor:pointer;transition:border-color .15s;}
    .llm-prov-card.selected{border-color:var(--accent);background:rgba(99,102,241,.07);}
    .llm-prov-card.offline{opacity:.5;cursor:default;}
    .llm-prov-card:not(.offline):hover{border-color:var(--b2);}
  `;
  document.head.appendChild(s);
})();

// ── Knowledge Base Browser ────────────────────────────────────────────────────

function KbBrowser({ onInsert }) {
  const [search,    setSearch]  = useState('');
  const [activecat, setCat]     = useState('All');
  const [openId,    setOpenId]  = useState(null);

  const cats = ['All', ...KB_CATEGORIES];

  const filtered = KB.filter(e => {
    const catOk  = activecat === 'All' || e.category === activecat;
    if (!catOk) return false;
    if (!search.trim()) return true;
    const q = search.toLowerCase();
    return (
      e.title.toLowerCase().includes(q) ||
      e.tags.some(t => t.includes(q))   ||
      e.body.toLowerCase().includes(q)
    );
  });

  return h('div', { style: { display: 'flex', flexDirection: 'column', gap: 12 } },

    h('input', {
      type: 'text', value: search,
      onChange: e => { setSearch(e.target.value); setOpenId(null); },
      placeholder: 'Search knowledge base…',
      style: { padding: '8px 12px', border: '1px solid var(--b3)', borderRadius: 'var(--r)', background: 'var(--b1)', color: 'var(--t1)', fontSize: 'var(--fsm)', width: '100%' }
    }),

    h('div', { style: { display: 'flex', flexWrap: 'wrap', gap: 6 } },
      cats.map(c => h('button', {
        key: c,
        className: 'kb-cat-tab' + (activecat === c ? ' active' : ''),
        onClick: () => { setCat(c); setOpenId(null); },
        style: activecat === c ? {} : { color: 'var(--t2)' }
      }, c))
    ),

    filtered.length === 0
      ? h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', padding: '12px 0' } }, 'No results.')
      : h('div', { style: { display: 'flex', flexDirection: 'column', gap: 6 } },
          filtered.map(entry => {
            const col    = KB_CATEGORY_COLORS[entry.category] || KB_CATEGORY_COLORS.Overview;
            const isOpen = openId === entry.id;
            return h('div', { key: entry.id, className: 'kb-card' },
              h('div', {
                className: 'kb-card-head',
                onClick: () => setOpenId(isOpen ? null : entry.id),
              },
                h('span', {
                  className: 'kb-cat-badge',
                  style: { background: col.bg, border: `1px solid ${col.border}`, color: col.text }
                }, entry.category),
                h('span', { className: 'kb-title' }, entry.title),
                h('div', { className: 'kb-actions' },
                  onInsert && h('button', {
                    className: 'btn btn-s btn-sm',
                    style: { fontSize: 'var(--fxs)', padding: '2px 9px', flexShrink: 0 },
                    onClick: e => { e.stopPropagation(); onInsert('Tell me about: ' + entry.title); }
                  }, 'Ask'),
                  h('span', { className: 'kb-chevron' + (isOpen ? ' open' : '') }, '▼')
                )
              ),
              isOpen && h('div', { className: 'kb-body' }, entry.body)
            );
          })
        )
  );
}

// ── Recommended models for one-click pull ─────────────────────────────────────

const PULL_MODELS = [
  { name: 'gemma3',            label: 'Gemma 3 (4B)',         author: 'Google',    ram: '5 GB',  rec: true,  desc: 'Recommended · Best for A1 knowledge tasks · Runs on 8 GB RAM' },
  { name: 'llama3.2',          label: 'Llama 3.2 (3B)',       author: 'Meta',      ram: '2 GB',  rec: false, desc: 'Fastest · Great for low-RAM machines' },
  { name: 'llama3.1:8b',       label: 'Llama 3.1 (8B)',       author: 'Meta',      ram: '5 GB',  rec: false, desc: 'Best balance of speed and intelligence' },
  { name: 'mistral',           label: 'Mistral (7B)',          author: 'Mistral',   ram: '4 GB',  rec: false, desc: 'Reliable, consistent instruction following' },
  { name: 'qwen2.5-coder:7b',  label: 'Qwen 2.5 Coder (7B)', author: 'Alibaba',   ram: '5 GB',  rec: false, desc: 'Best for code / A1 integration tasks' },
  { name: 'phi4',              label: 'Phi-4 (14B)',           author: 'Microsoft', ram: '9 GB',  rec: false, desc: 'Strong reasoning, above its weight class' },
  { name: 'gemma3:27b',        label: 'Gemma 3 (27B)',        author: 'Google',    ram: '18 GB', rec: false, desc: 'Highest quality · needs 24 GB+ VRAM' },
];

// ── Model Puller component (pure display — state lives in LlmSetupPanel) ──────
// Props: ollamaRunning, pulling, progress, done, err, onPull
// This component never owns pull state itself, so it survives tab switches.

function ModelPuller({ ollamaRunning, pulling, progress, done, err, onPull }) {

  if (!ollamaRunning) {
    return h('div', {
      style: {
        padding: '12px 14px', borderRadius: 'var(--r)', marginTop: 4,
        background: 'rgba(99,102,241,.06)', border: '1px solid rgba(99,102,241,.15)',
        fontSize: 'var(--fxs)', color: 'var(--t2)', lineHeight: 1.7,
      }
    },
      h('div', { style: { fontWeight: 700, color: 'var(--t1)', marginBottom: 6, fontSize: 'var(--fsm)' } },
        '📦 Install Ollama first to pull models'),
      h('div', { style: { marginBottom: 8 } },
        'Ollama runs your local AI models. It\'s free, private, and requires no account.'),
      h('div', { style: { display: 'flex', gap: 8, flexWrap: 'wrap' } },
        h('a', {
          href: 'https://ollama.com/download',
          target: '_blank',
          style: {
            display: 'inline-flex', alignItems: 'center', gap: 6,
            padding: '6px 14px', borderRadius: 'var(--r)',
            background: 'var(--accent)', color: '#fff',
            fontWeight: 700, fontSize: 'var(--fsm)', textDecoration: 'none',
          }
        }, '⬇ Download Ollama'),
        h('a', {
          href: 'https://ollama.com/library/gemma3',
          target: '_blank',
          style: {
            display: 'inline-flex', alignItems: 'center', gap: 6,
            padding: '6px 14px', borderRadius: 'var(--r)',
            border: '1px solid var(--b3)', color: 'var(--t2)',
            fontWeight: 600, fontSize: 'var(--fsm)', textDecoration: 'none',
          }
        }, 'View Gemma 3 on Ollama →')
      ),
      h('div', { style: { marginTop: 10, color: 'var(--t3)', fontSize: 'var(--fxs)' } },
        'After installing Ollama, come back here and click "Pull Model" next to Gemma 3 — no terminal needed.')
    );
  }

  return h('div', { style: { display: 'flex', flexDirection: 'column', gap: 6 } },
    h('div', { style: { fontWeight: 700, fontSize: 'var(--fsm)', marginBottom: 4, color: 'var(--t1)' } },
      '📦 Pull a model — no terminal needed'),
    h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginBottom: 8 } },
      'Ollama is running. Click Pull to download any model directly from this panel. ⭐ = recommended for A1.'),

    PULL_MODELS.map(m => {
      const isPulling = pulling === m.name;
      const isDone    = done[m.name];
      const prog      = progress[m.name];
      const hasErr    = err[m.name];

      return h('div', {
        key: m.name,
        style: {
          border: '1px solid ' + (m.rec ? 'rgba(99,102,241,.4)' : 'var(--b3)'),
          borderRadius: 'var(--r)', padding: '8px 12px',
          background: m.rec ? 'rgba(99,102,241,.05)' : 'var(--b1)',
          opacity: (pulling && !isPulling) ? 0.55 : 1,
          transition: 'opacity .2s',
        }
      },
        h('div', { style: { display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' } },
          h('div', { style: { flex: 1, minWidth: 0 } },
            h('div', { style: { display: 'flex', alignItems: 'center', gap: 6 } },
              m.rec && h('span', { style: { color: '#fbbf24', fontSize: 13 } }, '⭐'),
              h('span', { style: { fontWeight: 700, fontSize: 'var(--fsm)', color: 'var(--t1)' } }, m.label),
              h('span', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', fontFamily: 'var(--mono)' } }, m.author),
              h('span', { style: { fontSize: 'var(--fxs)', padding: '1px 7px', borderRadius: 9, background: 'var(--b2)', color: 'var(--t2)', border: '1px solid var(--b3)' } }, m.ram + ' RAM')
            ),
            h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 2 } }, m.desc)
          ),
          isDone
            ? h('span', { style: { fontSize: 'var(--fxs)', color: 'var(--green)', fontWeight: 700, flexShrink: 0 } }, '✓ Ready')
            : h('button', {
                onClick: () => onPull(m.name),
                disabled: !!pulling,
                style: {
                  flexShrink: 0, padding: '5px 13px', borderRadius: 'var(--r)',
                  background: isPulling ? 'var(--b2)' : (m.rec ? 'var(--accent)' : 'var(--b2)'),
                  color: isPulling ? 'var(--t2)' : (m.rec ? '#fff' : 'var(--t1)'),
                  border: '1px solid ' + (m.rec && !isPulling ? 'var(--accent)' : 'var(--b3)'),
                  fontWeight: 600, fontSize: 'var(--fxs)', cursor: pulling ? 'default' : 'pointer',
                }
              }, isPulling ? '… Pulling' : '⬇ Pull')
        ),

        (isPulling || isDone) && prog && h('div', { style: { marginTop: 8 } },
          h('div', { style: { display: 'flex', justifyContent: 'space-between', fontSize: 'var(--fxs)', color: 'var(--t2)', marginBottom: 4 } },
            h('span', null, prog.status),
            prog.pct > 0 && h('span', null, prog.pct + '%')
          ),
          h('div', {
            style: { height: 4, borderRadius: 2, background: 'var(--b3)', overflow: 'hidden' }
          },
            h('div', {
              style: {
                height: '100%', borderRadius: 2,
                background: isDone ? 'var(--green)' : 'var(--accent)',
                width: (prog.pct || (isPulling ? 8 : 0)) + '%',
                transition: 'width .3s ease',
              }
            })
          )
        ),

        hasErr && h('div', {
          style: { marginTop: 6, fontSize: 'var(--fxs)', color: '#ef4444', lineHeight: 1.5 }
        }, '⚠ ' + hasErr + ' — make sure Ollama is running: ollama serve')
      );
    })
  );
}

// ── LLM Setup Panel ───────────────────────────────────────────────────────────
// Pull state lives HERE so it survives sub-tab navigation.

function LlmSetupPanel({ onReady }) {
  const [probeResults,   setProbeResults]   = useState({});
  const [selected,       setSelected]       = useState(null);
  const [selectedModel,  setSelectedModel]  = useState('');
  const [subTab,         setSubTab]         = useState('connect'); // 'connect' | 'pull'

  // ── Pull state lifted up from ModelPuller so it outlives tab switches ──────
  const [pulling,      setPulling]      = useState(null);   // model name currently pulling
  const [pullProgress, setPullProgress] = useState({});     // { modelName: { status, pct } }
  const [pullDone,     setPullDone]     = useState({});     // { modelName: true }
  const [pullErr,      setPullErr]      = useState({});     // { modelName: errorMsg }

  useEffect(() => {
    ASST_PROVIDERS.forEach(p => {
      probeProvider(p).then(r => {
        setProbeResults(prev => ({ ...prev, [p.id]: r }));
        if (r.status === 'running' && !selected) {
          setSelected(p);
          setSelectedModel(r.models[0] || '');
        }
      });
    });
  }, []);

  useEffect(() => {
    if (selected && selectedModel) onReady({ provider: selected, model: selectedModel });
    else onReady(null);
  }, [selected, selectedModel]);

  const allOffline = Object.values(probeResults).length > 0 &&
    Object.values(probeResults).every(r => r.status === 'offline');

  const ollamaRunning = probeResults['ollama']?.status === 'running' || probeResults['ollama']?.status === 'empty';

  // Re-probe Ollama after a successful pull so new model appears in selector
  function onModelPulled(modelName) {
    probeProvider(ASST_PROVIDERS[0]).then(r => {
      setProbeResults(prev => ({ ...prev, ollama: r }));
      if (r.status === 'running') {
        setSelected(ASST_PROVIDERS[0]);
        setSelectedModel(modelName);
      }
    });
  }

  async function handlePull(modelName) {
    if (pulling) return;
    setPulling(modelName);
    setPullErr(prev => ({ ...prev, [modelName]: null }));
    setPullProgress(prev => ({ ...prev, [modelName]: { status: 'Starting pull\u2026', pct: 0 } }));

    try {
      const resp = await fetch('http://localhost:11434/api/pull', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: modelName, stream: true }),
      });

      if (!resp.ok) throw new Error('Ollama returned ' + resp.status);

      const reader  = resp.body.getReader();
      const decoder = new TextDecoder();
      let   buf     = '';

      while (true) {
        const { value, done: streamDone } = await reader.read();
        if (streamDone) break;
        buf += decoder.decode(value, { stream: true });
        const lines = buf.split('\n');
        buf = lines.pop();
        for (const line of lines) {
          if (!line.trim()) continue;
          try {
            const obj = JSON.parse(line);
            if (obj.error) throw new Error(obj.error);
            const pct = (obj.completed && obj.total)
              ? Math.round((obj.completed / obj.total) * 100)
              : 0;
            const statusLabel = obj.status === 'success'
              ? '\u2713 Done!'
              : (obj.status || 'Downloading\u2026') + (pct > 0 ? ` ${pct}%` : '');
            setPullProgress(prev => ({ ...prev, [modelName]: { status: statusLabel, pct } }));
          } catch (_) { /* skip malformed lines */ }
        }
      }

      setPullDone(prev => ({ ...prev, [modelName]: true }));
      setPullProgress(prev => ({ ...prev, [modelName]: { status: '\u2713 Ready to use!', pct: 100 } }));
      onModelPulled(modelName);
    } catch (e) {
      setPullErr(prev => ({ ...prev, [modelName]: e.message }));
    } finally {
      setPulling(null);
    }
  }

  const SUB_TABS = [
    { id: 'connect', label: '🔌 Connect' },
    { id: 'pull',    label: '📦 Pull Model' + (pulling ? ' ⏳' : '') },
  ];

  return h('div', { style: { display: 'flex', flexDirection: 'column', gap: 10 } },

    // Sub-tab strip
    h('div', { style: { display: 'flex', gap: 0, borderBottom: '1px solid var(--b3)', marginBottom: 4 } },
      SUB_TABS.map(t => h('button', {
        key: t.id,
        onClick: () => setSubTab(t.id),
        style: {
          padding: '5px 14px', fontSize: 'var(--fxs)', fontWeight: subTab === t.id ? 700 : 500,
          color: subTab === t.id ? 'var(--t1)' : 'var(--t2)',
          background: 'transparent', border: 'none', cursor: 'pointer',
          borderBottom: '2px solid ' + (subTab === t.id ? 'var(--accent)' : 'transparent'),
        }
      }, t.label))
    ),

    // CONNECT sub-tab — hidden via display when not active, NOT unmounted
    h('div', { style: { display: subTab === 'connect' ? 'flex' : 'none', flexDirection: 'column', gap: 8 } },
      ASST_PROVIDERS.map(p => {
        const res       = probeResults[p.id];
        const status    = res?.status || 'probing';
        const models    = res?.models || [];
        const isSel     = selected?.id === p.id;
        const isOffline = status === 'offline';

        return h('div', {
          key: p.id,
          className: 'llm-prov-card' + (isSel ? ' selected' : '') + (isOffline ? ' offline' : ''),
          onClick: () => {
            if (status !== 'running') return;
            setSelected(p);
            setSelectedModel(models[0] || '');
          },
        },
          h('div', { style: { display: 'flex', alignItems: 'center', gap: 10 } },
            h('span', { style: { fontSize: 18 } }, p.icon),
            h('span', { style: { fontWeight: 700, fontSize: 'var(--fsm)', flex: 1 } }, p.label),
            h('span', {
              style: {
                fontSize: 'var(--fxs)', fontWeight: 700, padding: '2px 9px', borderRadius: 10,
                background: status === 'running' ? 'rgba(34,197,94,.1)' : status === 'probing' ? 'var(--b2)' : 'rgba(239,68,68,.09)',
                color:      status === 'running' ? 'var(--green)'         : status === 'probing' ? 'var(--t2)' : '#ef4444',
                border:     '1px solid ' + (status === 'running' ? 'rgba(34,197,94,.3)' : status === 'probing' ? 'var(--b3)' : 'rgba(239,68,68,.2)'),
              }
            }, status === 'running' ? '● Running' : status === 'probing' ? '… Probing' : '○ Offline')
          ),
          isSel && models.length > 0 && h('div', { style: { marginTop: 8, display: 'flex', alignItems: 'center', gap: 8 } },
            h('span', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)' } }, 'Model:'),
            h('select', {
              value: selectedModel,
              onChange: e => { e.stopPropagation(); setSelectedModel(e.target.value); },
              style: { fontSize: 'var(--fxs)', padding: '3px 8px', border: '1px solid var(--b3)', borderRadius: 'var(--r)', background: 'var(--b1)', color: 'var(--t1)', cursor: 'pointer' }
            }, models.map(m => h('option', { key: m, value: m }, m)))
          )
        );
      }),

      allOffline && h('div', {
        style: { padding: '10px 14px', borderRadius: 'var(--r)', background: 'rgba(99,102,241,.06)', border: '1px solid rgba(99,102,241,.15)', fontSize: 'var(--fxs)', color: 'var(--t2)', lineHeight: 1.65, marginTop: 4 }
      },
        h('span', null, '💡 No local LLM detected. '),
        h('span', { style: { fontWeight: 700, color: 'var(--accent)', cursor: 'pointer' }, onClick: () => setSubTab('pull') },
          'Go to Pull Model tab →'),
        h('span', null, ' to install Gemma 3 (Google) in one click. Or browse the knowledge base for answers without an LLM.')
      )
    ),

    // PULL MODEL sub-tab — also hidden via display, never unmounted
    // State (pulling/progress/done/err) lives in LlmSetupPanel above, so it
    // persists across sub-tab switches and across top-level tab navigation.
    h('div', { style: { display: subTab === 'pull' ? 'block' : 'none' } },
      h(ModelPuller, {
        ollamaRunning,
        pulling,
        progress: pullProgress,
        done:     pullDone,
        err:      pullErr,
        onPull:   handlePull,
      })
    )
  );
}

// ── Chat Bubble ───────────────────────────────────────────────────────────────

function _escHtml(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function _renderMd(raw) {
  let s = raw;
  // fenced code blocks
  s = s.replace(/```[\w]*\n?([\s\S]*?)```/g, (_, code) =>
    `<pre style="background:rgba(0,0,0,.32);border:1px solid rgba(255,255,255,.08);border-radius:5px;padding:9px 12px;overflow-x:auto;margin:7px 0;white-space:pre"><code style="font-family:var(--mono);font-size:.82em;color:#e2e8f0;white-space:pre">${_escHtml(code.trim())}</code></pre>`);
  // inline code
  s = s.replace(/`([^`\n]+)`/g, '<code style="font-family:var(--mono);background:rgba(0,0,0,.28);padding:1px 5px;border-radius:3px;font-size:.85em">$1</code>');
  // h3 h2 h1
  s = s.replace(/^### (.+)$/gm, '<div style="font-weight:700;font-size:.92em;margin:10px 0 3px;color:var(--t1)">$1</div>');
  s = s.replace(/^## (.+)$/gm,  '<div style="font-weight:700;font-size:.97em;margin:11px 0 4px;color:var(--t1)">$1</div>');
  s = s.replace(/^# (.+)$/gm,   '<div style="font-weight:700;font-size:1.05em;margin:12px 0 5px;color:var(--t1)">$1</div>');
  // bold / italic
  s = s.replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>');
  s = s.replace(/\*([^*\n]+)\*/g, '<em>$1</em>');
  // hr
  s = s.replace(/^---+$/gm, '<hr style="border:none;border-top:1px solid rgba(255,255,255,.1);margin:9px 0">');
  // bullet lines
  s = s.replace(/^[ \t]*[•·\-\*] (.+)$/gm, '<div style="padding-left:14px;margin:2px 0">• $1</div>');
  // numbered lines
  s = s.replace(/^[ \t]*(\d+)\. (.+)$/gm, '<div style="padding-left:14px;margin:2px 0">$1. $2</div>');
  // double newline → paragraph gap
  s = s.replace(/\n{2,}/g, '<br><br>');
  // single newline
  s = s.replace(/\n/g, '<br>');
  return s;
}

function ChatBubble({ role, text, error }) {
  const isUser = role === 'user';
  const baseStyle = {
    maxWidth: '84%', padding: '9px 13px',
    borderRadius: isUser ? '12px 12px 2px 12px' : '12px 12px 12px 2px',
    background: isUser ? 'var(--accent)' : (error ? 'rgba(239,68,68,.07)' : 'var(--b2)'),
    color: isUser ? '#fff' : (error ? '#ef4444' : 'var(--t1)'),
    fontSize: 'var(--fsm)', lineHeight: 1.65,
    border: error ? '1px solid rgba(239,68,68,.25)' : 'none',
    wordBreak: 'break-word',
  };
  return h('div', { style: { display: 'flex', justifyContent: isUser ? 'flex-end' : 'flex-start', marginBottom: 8 } },
    isUser
      ? h('div', { style: { ...baseStyle, whiteSpace: 'pre-wrap' } }, text)
      : h('div', { style: baseStyle, dangerouslySetInnerHTML: { __html: _renderMd(text) } })
  );
}

function TypingBubble() {
  return h('div', { style: { display: 'flex', justifyContent: 'flex-start', marginTop: 4, marginBottom: 4 } },
    h('div', { style: { padding: '11px 16px', borderRadius: '12px 12px 12px 2px', background: 'var(--b2)', display: 'flex', alignItems: 'center', gap: 5, border: '1px solid var(--b3)' } },
      h('span', { className: '_a1d' }), h('span', { className: '_a1d' }), h('span', { className: '_a1d' })
    )
  );
}

// ── Persistence ───────────────────────────────────────────────────────────────

const _CHAT_KEY = 'a1_asst_chat_' + _KB_NS;
const _LLM_KEY  = 'a1_asst_llm_'  + _KB_NS;

const _WELCOME = {
  role: 'assistant',
  text: 'Hi! I\'m the A1 Knowledge Assistant. Ask me anything — setup, passports, errors, integrations, enterprise, or community (Reddit, GitHub, Dyolo on X). For smarter answers, go to the \"Connect LLM\" tab. ⭐ Gemma 3 by Google is recommended — pull it in one click, no terminal needed.',
};

function _loadMessages() {
  try { const r = sessionStorage.getItem(_CHAT_KEY); if (r) return JSON.parse(r); } catch {}
  return [_WELCOME];
}
function _saveMessages(msgs) {
  try { sessionStorage.setItem(_CHAT_KEY, JSON.stringify(msgs)); } catch {}
}
function _loadLlmCfg() {
  try {
    const r = sessionStorage.getItem(_LLM_KEY);
    if (!r) return null;
    const { providerId, model } = JSON.parse(r);
    const provider = ASST_PROVIDERS.find(p => p.id === providerId);
    return provider ? { provider, model } : null;
  } catch { return null; }
}
function _saveLlmCfg(cfg) {
  try {
    if (cfg) sessionStorage.setItem(_LLM_KEY, JSON.stringify({ providerId: cfg.provider.id, model: cfg.model }));
    else sessionStorage.removeItem(_LLM_KEY);
  } catch {}
}

// ── Root component ────────────────────────────────────────────────────────────

function AiAssistant() {
  const [llmConfig, setLlmConfig] = useState(_loadLlmCfg);
  const [messages,  setMessages]  = useState(_loadMessages);
  const [input,     setInput]     = useState('');
  const [loading,   setLoading]   = useState(false);
  const [tab,       setTab]       = useState('chat');
  const chatEndRef                = useRef(null);

  useEffect(() => { _saveMessages(messages); }, [messages]);
  useEffect(() => { chatEndRef.current?.scrollIntoView({ behavior: 'smooth' }); }, [messages, loading]);

  useEffect(() => {
    function onError(e) {
      if (e.detail?.error) { setInput('I got this error: ' + e.detail.error); setTab('chat'); }
    }
    window.addEventListener('a1-ask-assistant', onError);
    return () => window.removeEventListener('a1-ask-assistant', onError);
  }, []);

  async function send() {
    const text = input.trim();
    if (!text || loading) return;
    setInput('');

    const userMsg = { role: 'user', text };
    const next    = [...messages, userMsg];
    setMessages(next);
    setLoading(true);

    // Casual response — no LLM or KB lookup needed
    const casual = _casual(text);
    if (casual) {
      await new Promise(r => setTimeout(r, 280));
      setMessages(prev => [...prev, { role: 'assistant', text: casual }]);
      setLoading(false);
      return;
    }

    if (!llmConfig) {
      const match = _kbSearch(text);
      const reply = match
        ? `${match.title}\n\n${match.body}`
        : 'I couldn\'t find a match in the knowledge base for that. Try rephrasing, or connect a local LLM in the "Connect LLM" tab for smarter answers.';
      setMessages(prev => [...prev, { role: 'assistant', text: reply }]);
      setLoading(false);
      return;
    }

    const systemPrompt = buildSystemPrompt();
    const history = next.slice(-14).map(m => ({ role: m.role === 'user' ? 'user' : 'assistant', content: m.text }));
    const apiMessages = [{ role: 'system', content: systemPrompt }, ...history];

    try {
      const rawReply = llmConfig.provider.isOllama
        ? await chatWithOllama(llmConfig.provider.baseUrl, llmConfig.model, apiMessages)
        : await chatOpenAICompat(llmConfig.provider.baseUrl, llmConfig.model, apiMessages);
      const reply = sanitizeReply(rawReply || '', text);
      setMessages(prev => [...prev, { role: 'assistant', text: reply || '(empty response from model)' }]);
    } catch (err) {
      setMessages(prev => [...prev, {
        role: 'assistant',
        text: 'Could not reach local LLM: ' + err.message + '\n\nMake sure your LLM server is still running, then try again.',
        error: true,
      }]);
    }
    setLoading(false);
  }

  function onKey(e) { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(); } }
  function onInsert(q) { setInput(q); setTab('chat'); }
  function connectLlm(cfg) {
    setLlmConfig(cfg);
    _saveLlmCfg(cfg);
    if (cfg) {
      setMessages(prev => [...prev, {
        role: 'assistant',
        text: `Connected to ${cfg.provider.label} (${cfg.model}). Full A1 knowledge base loaded as context — ask me anything!`,
      }]);
    }
  }
  function clearChat() { const f = [_WELCOME]; setMessages(f); _saveMessages(f); }

  const TABS_UI = [
    { id: 'chat',      label: '💬 Chat' },
    { id: 'llm',       label: '🤖 Connect LLM' },
    { id: 'knowledge', label: '📚 Knowledge Base' },
  ];

  return h('div', { style: { paddingBottom: 40, width: '100%' } },

    h('h2', { style: { fontSize: 18, fontWeight: 700, marginBottom: 4 } }, '🧠 AI Assistant'),
    h('p',  { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', lineHeight: 1.6, marginBottom: 16 } },
      'Ask anything about A1. Connect a local LLM for smarter answers, or browse the knowledge base.'),

    h('div', { className: 'asst-tab-strip' },
      TABS_UI.map(t => h('button', {
        key: t.id,
        className: 'asst-tab' + (tab === t.id ? ' active' : ''),
        onClick: () => setTab(t.id),
      }, t.label))
    ),

    // ── CHAT ─────────────────────────────────────────────────────────────────
    tab === 'chat' && h('div', { style: { display: 'flex', flexDirection: 'column', gap: 10 } },

      h('div', { style: { display: 'flex', alignItems: 'center', gap: 8 } },
        llmConfig
          ? h('div', {
              style: { flex: 1, display: 'flex', alignItems: 'center', gap: 7, padding: '6px 10px', borderRadius: 'var(--r)', background: 'rgba(34,197,94,.07)', border: '1px solid rgba(34,197,94,.2)', fontSize: 'var(--fxs)', color: 'var(--green)' }
            },
              h('span', null, '●'),
              h('span', null, llmConfig.provider.label),
              h('code', { style: { fontFamily: 'var(--mono)', opacity: .8 } }, llmConfig.model)
            )
          : h('div', {
              style: { flex: 1, display: 'flex', alignItems: 'center', gap: 6, padding: '6px 10px', borderRadius: 'var(--r)', background: 'var(--b2)', border: '1px solid var(--b3)', fontSize: 'var(--fxs)', color: 'var(--t2)' }
            },
              '○ Knowledge mode — ',
              h('span', { style: { color: 'var(--accent)', cursor: 'pointer', fontWeight: 600 }, onClick: () => setTab('llm') }, 'connect LLM for smarter answers →')
            ),
        messages.length > 1 && h('button', {
          onClick: clearChat,
          style: { fontSize: 'var(--fxs)', padding: '4px 10px', borderRadius: 'var(--r)', border: '1px solid var(--b3)', background: 'var(--b1)', color: 'var(--t2)', cursor: 'pointer', flexShrink: 0 }
        }, 'Clear')
      ),

      h('div', { className: 'ai-chat-window' },
        messages.map((m, i) => h(ChatBubble, { key: i, role: m.role, text: m.text, error: m.error })),
        loading && h(TypingBubble, null),
        h('div', { ref: chatEndRef })
      ),

      h('div', { style: { display: 'flex', gap: 8 } },
        h('textarea', {
          value: input, onChange: e => setInput(e.target.value), onKeyDown: onKey,
          placeholder: 'Ask a question… (Enter to send, Shift+Enter for new line)',
          rows: 2,
          style: { flex: 1, resize: 'none', padding: '9px 12px', border: '1px solid var(--b3)', borderRadius: 'var(--r)', background: 'var(--b1)', color: 'var(--t1)', fontSize: 'var(--fsm)', lineHeight: 1.5, fontFamily: 'var(--sans)' }
        }),
        h('button', {
          className: 'btn btn-p', onClick: send, disabled: loading || !input.trim(),
          style: { padding: '9px 18px', alignSelf: 'stretch', fontSize: 'var(--fsm)' }
        }, loading ? '…' : '↑ Send')
      ),

      h('div', { style: { display: 'flex', flexWrap: 'wrap', gap: 6 } },
        ['How do I get started?', 'What is a passport?', 'Fix: CapabilityNotGranted', 'Fix: CertificateExpired', 'Which model for local LLM?', 'How do I connect Gemma?', 'Dyolo on X / Reddit / GitHub']
          .map(q => h('button', {
            key: q, onClick: () => setInput(q),
            style: { fontSize: 'var(--fxs)', padding: '3px 10px', borderRadius: 14, border: '1px solid var(--b3)', background: 'var(--b1)', color: 'var(--t2)', cursor: 'pointer' }
          }, q))
      )
    ),

    // ── LLM SETUP ────────────────────────────────────────────────────────────
    tab === 'llm' && h('div', { style: { display: 'flex', flexDirection: 'column', gap: 12 } },
      h('p', { style: { fontSize: 'var(--fsm)', color: 'var(--t2)', lineHeight: 1.6 } },
        'Connect a local LLM. A1 loads the full knowledge base as system prompt — no data leaves your machine. ',
        h('strong', { style: { color: 'var(--t1)' } }, 'Gemma 3 (Google) is recommended.'),
        ' Use the Pull Model tab to download it in one click without opening a terminal.'),
      h(LlmSetupPanel, { onReady: connectLlm }),
      llmConfig && h('button', {
        className: 'btn btn-p btn-sm', style: { alignSelf: 'flex-start', marginTop: 4 },
        onClick: () => setTab('chat')
      }, '→ Go to chat')
    ),

    // ── KNOWLEDGE BASE ───────────────────────────────────────────────────────
    tab === 'knowledge' && h('div', { style: { display: 'flex', flexDirection: 'column', gap: 10 } },
      h('p', { style: { fontSize: 'var(--fsm)', color: 'var(--t2)', lineHeight: 1.6 } },
        'Browse the built-in A1 knowledge base by category. Click "Ask" to send any topic to chat.'),
      h(KbBrowser, { onInsert })
    )
  );
}