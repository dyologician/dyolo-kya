// ─────────────────────────────────────────────────────────────────────────────
// HELP TOOLTIP DATA
// ─────────────────────────────────────────────────────────────────────────────
const HELP={
  'sidebar':      {title:'Navigation',body:'Use the sidebar to switch between sections. The Live Log shows real-time requests. Settings controls gateway connection and display.'},
  'overview':     {title:'Overview',body:'Live stats for this Studio session — requests made, errors, latency. The Activity feed shows recent API calls. Social links connect to the A1 creator.'},
  'live-log':     {title:'Live Request Log',body:'Every API call this Studio makes is captured here with full request/response JSON. Click any row to expand. No mocks — this is real traffic.'},
  'passports':    {title:'Passports',body:'Issue cryptographic agent passports. A passport defines what an agent is allowed to do and for how long. The signing key must be saved — it cannot be recovered.'},
  'swarms':       {title:'Swarm Passports',body:'A SwarmPassport governs a group of agents with role-based delegation. Create a root swarm, then add members with scoped roles (Worker, Supervisor, etc.).'},
  'did':          {title:'DID & Verifiable Credentials',body:'Every A1 identity gets a did:a1: DID. VCs are W3C-standard credentials that any system can verify without A1-specific code.'},
  'authorize':    {title:'Test Authorization',body:'Test a delegation chain against an intent. Paste a SignedChain JSON, enter the intent name and executor public key, and see the full authorization result.'},
  'compliance':   {title:'Compliance Reports',body:'Generate audit reports covering EU AI Act, NIST AI RMF, SOC 2, and ISO 27001. Download as JSON for your auditors.'},
  'settings':     {title:'Settings',body:'Configure gateway connection, admin secret, display density, font size, and help options. Settings are saved in your browser locally.'},
  'gateway-id':   {title:'Gateway Identity',body:'The gateway\'s Ed25519 signing key and DID. Every cert issued by this gateway is signed with this key. Keep A1_SIGNING_KEY_HEX secret.'},
  'trun-id':      {title:'Collapsible ID',body:'Long keys and DIDs are truncated by default. Click ▼ to expand. Click ⎘ to copy the full value. Enable "Show Full IDs" in Settings to expand all.'},
  'social-attr':  {title:'Creator & Attribution',body:'A1 is created by @dyologician. These links are part of the integrity system — the Studio verifies them are present and unmodified on every render.'},
  'agents':       {title:'Connect Agents',body:'A1 scans your system for installed AI agents and connects them with one click. No code required. Writes .mcp.json or plugin config to the agent\'s directory automatically.'},
  'density-ctrl': {title:'UI Density',body:'Auto scales to your screen size. Compact reduces spacing for small screens. Comfortable adds breathing room on large displays.'},
  'font-ctrl':    {title:'Font Size',body:'Adjust text size across the entire Studio. Changes apply instantly. Saved between sessions.'},
  'help-btn':     {title:'Help Mode (? button)',body:'You\'re already in help mode! Drag this button anywhere. In help mode, hover over any highlighted element to see what it does.'},
  'theme-btn':    {title:'Theme Toggle',body:'Switch between dark (black) and light (white) theme. The theme is purely cosmetic — all data and connections remain unchanged.'},
  'log-filter':   {title:'Log Filter',body:'Filter log entries by path, HTTP method, or status code. Filtering does not delete entries — clear the filter to see all.'},
  'auth-chain':   {title:'Delegation Chain JSON',body:'A SignedChain is the JSON representation of a delegation from a root passport to an executing agent. Generate one with the CLI: a1 passport sub ...'},
};

