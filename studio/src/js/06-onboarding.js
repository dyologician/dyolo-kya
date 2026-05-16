// ─────────────────────────────────────────────────────────────────────────────
// ONBOARDING MODAL — v2.8.0
// ─────────────────────────────────────────────────────────────────────────────

const ONBOARD_STEPS = [
  {
    icon: 'A1',
    title: 'Welcome to A1 Studio',
    body: 'A1 gives every AI agent a cryptographic identity — so you always know exactly what it did and who authorized it.',
    list: [
      'Everything runs on your own machine. No cloud account required.',
      'Works with Claude Code, ChatGPT, LangChain, CrewAI, and 15+ frameworks.',
      'A1 Studio is your local control panel. Keep this tab open while you work.',
    ],
  },
  {
    icon: '🗺️',
    title: 'The mental model — think of it like a work badge',
    body: 'Before your agent can take any action (send an email, read a file, make a trade), it shows a badge proving it\'s allowed to. A1 issues and checks that badge automatically.',
    list: [
      '🪪  Passport = your agent\'s ID card. Created once, says "this agent is allowed to do X and Y". Stored in a file called passport.json.',
      '🎫  Sub-cert = a single-session permission slip. Scoped to exactly what\'s needed right now. Expires in hours — limited blast radius.',
      '🔗  Chain = the ID card + permission slip bundled together. Your agent shows this with every action it takes.',
      '✅  A1 checks every action: valid badge? right permissions? not expired? If anything is wrong, the action is blocked — automatically, cryptographically.',
    ],
  },
  {
    icon: '🛡',
    title: 'Step 1 — Protect My Agent',
    body: 'The first thing you do is create a Passport — your agent\'s signed identity card.',
    list: [
      'Give your agent a name (e.g. "email-helper" or "trading-bot").',
      'Choose what it\'s allowed to do (capabilities like "email.send", "files.read").',
      'Pick how long the passport lasts (30 days is a good start).',
      '⚠️  A1 saves a passport.json file — never commit it to Git. A1 adds it to .gitignore automatically.',
    ],
  },
  {
    icon: '🔌',
    title: 'Step 2 — Connect & Restart Your Agent',
    body: 'After creating the passport, connect it to your AI tool — then restart the agent.',
    list: [
      'Go to "Connect Agents" and pick your framework from the list.',
      'A1 writes the connection config or code patch automatically.',
      '🔄  RESTART REQUIRED: After patching, you MUST restart your agent (Claude Code, your Python script, etc.) before anything changes. The AI Integration tab shows the exact restart command for your framework.',
      '⚠️  If you patched your agent but nothing seems different — a missing restart is the #1 cause. Stop the process completely, then start it again.',
    ],
  },
  {
    icon: '🗄️',
    title: 'Passport Vault — Manage All Agents',
    body: 'The Passport Vault shows every agent you\'ve protected — in one place.',
    list: [
      'See which passports are valid, expiring, or expired at a glance.',
      'Renew any passport in one click — no fingerprints or hex strings needed.',
      'Revoke an agent\'s access instantly if something goes wrong.',
      '📁  passport.json lives in the folder where you ran setup.sh (or setup.ps1 on Windows). A1 adds it to .gitignore automatically — never move, rename, or delete it. If you need to find it, the Vault shows the full path.',
    ],
  },
  {
    icon: '⚡',
    title: 'Keep A1 Running — set up auto-start',
    body: 'A1 runs as a local process. If it stops, your agents cannot authorize actions until you restart it.',
    list: [
      '🔴  A red dot in the sidebar = A1 is offline. Your agents will fail with "connection refused" until you start it.',
      '✅  Go to "Start / Stop" → click "Enable Auto-start" — A1 then starts automatically every time you log in.',
      'If your agents stop working after a reboot, check the status dot first — 90% of the time, A1 just isn\'t running.',
      'Run manually anytime: ./setup.sh (Mac/Linux) or .\\setup.ps1 (Windows)',
    ],
  },
  {
    icon: '🔒',
    title: 'Advanced features — you don\'t need them',
    body: 'You may see mentions of Redis, Postgres, KMS, and compliance reports. These are for production deployments with teams.',
    list: [
      '✅  For personal use: the defaults are perfect. A1 works great out of the box with no extra setup.',
      '🏢  Redis / Postgres: only needed if you want agent state to survive server restarts in a team deployment.',
      '🔑  KMS (Key Management Service): for teams storing signing keys in AWS/Azure/Vault. Not needed for local use.',
      '📋  Compliance reports: SOC 2, ISO 27001 audit trails — for enterprise teams, not individuals.',
    ],
  },
  {
    icon: '🔎',
    title: 'When Something Goes Wrong',
    body: 'A1 blocks unauthorized actions and tells you exactly why.',
    list: [
      'Go to "Error Help" and paste any error message — even raw JSON from the terminal.',
      'You\'ll get a plain-English explanation and numbered fix steps.',
      'Common fix: capability missing → re-issue passport with the right permissions.',
      'Common fix: "connection refused" → A1 isn\'t running. Go to Start / Stop and start it.',
    ],
  },
];

function OnboardModal({ onClose }) {
  const [step, setStep] = useState(0);
  const s    = ONBOARD_STEPS[step];
  const last = step === ONBOARD_STEPS.length - 1;

  return h('div', { className: 'modal-overlay', onClick: e => { if (e.target === e.currentTarget) onClose(); } },
    h('div', { className: 'modal' },
      h('div', { className: 'modal-head' },
        h('span', { style: { fontSize: 30, display: 'block', marginBottom: 8 } }, s.icon),
        h('h2', null, s.title),
        s.body && h('p', null, s.body)
      ),
      h('div', { className: 'modal-body' },
        s.list && h('ul', { className: 'onboard-step' },
          s.list.map((item, i) => h('li', { key: i }, item))
        )
      ),
      h('div', { className: 'modal-foot' },
        h('div', { className: 'step-dots' },
          ONBOARD_STEPS.map((_, i) => h('div', { key: i, className: 'step-dot' + (i === step ? ' on' : '') }))
        ),
        h('div', { style: { display: 'flex', gap: 7 } },
          step > 0 && h('button', { className: 'btn btn-s btn-sm', onClick: () => setStep(s => s - 1) }, '← Back'),
          h('button', { className: 'btn btn-p btn-sm', onClick: () => last ? onClose() : setStep(s => s + 1) },
            last ? 'Get Started →' : 'Next →'
          )
        )
      )
    )
  );
}
