// ─────────────────────────────────────────────────────────────────────────────
// GUIDED NEXT — reusable "what to do next" footer for each tab
// ─────────────────────────────────────────────────────────────────────────────

const NEXT_STEPS = {
  wizard: [
    { tab: 'agents',   icon: '🔌', label: 'Connect to your agent',   desc: 'Link A1 to your AI tool (Claude Code, ChatGPT, LangChain, etc.)' },
    { tab: 'vault',    icon: '🗄️', label: 'View your passports',     desc: 'See all issued passports, renew expiring ones' },
  ],
  agents: [
    { tab: 'chat',     icon: '🔌', label: 'Test the connection',     desc: 'Send a test message to confirm your agent is protected' },
    { tab: 'vault',    icon: '🗄️', label: 'Manage passports',       desc: 'See passport status, renew or revoke' },
  ],
  chat: [
    { tab: 'vault',    icon: '🗄️', label: 'Manage passports',       desc: 'Track expiry, renew, or revoke agents' },
    { tab: 'errors',   icon: '🔎', label: 'Decode errors',          desc: 'Something not working? Get a plain-English explanation' },
  ],
  vault: [
    { tab: 'wizard',   icon: '🛡', label: 'Issue a new passport',   desc: 'Protect another agent' },
    { tab: 'errors',   icon: '🔎', label: 'Troubleshoot errors',    desc: 'Understand what went wrong' },
  ],
  lifecycle: [
    { tab: 'wizard',   icon: '🛡', label: 'Protect an agent',       desc: 'Create a passport for your first agent' },
    { tab: 'agents',   icon: '🔌', label: 'Connect an agent',       desc: 'Link A1 to your AI tool' },
  ],
  errors: [
    { tab: 'wizard',   icon: '🛡', label: 'Re-issue a passport',    desc: 'Create a fresh passport to fix most errors' },
    { tab: 'vault',    icon: '🗄️', label: 'Check passport status',  desc: 'See if any passport has expired' },
    { tab: 'lifecycle',icon: '⚡', label: 'Restart A1',             desc: 'Start or restart the gateway' },
  ],
  integrate: [
    { tab: 'chat',     icon: '🔌', label: 'Test the connection',    desc: 'Verify the integration worked' },
    { tab: 'errors',   icon: '🔎', label: 'Decode errors',          desc: 'If something isn\'t working, start here' },
  ],
};

function GuidedNext({ currentTab }) {
  const steps = NEXT_STEPS[currentTab];
  if (!steps || steps.length === 0) return null;

  return h('div', { style: { marginTop: 28, borderTop: '1px solid var(--b3)', paddingTop: 18 } },
    h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t3)', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '.05em', marginBottom: 10 } },
      'What to do next'),
    h('div', { style: { display: 'flex', gap: 8, flexWrap: 'wrap' } },
      steps.map(s => h('div', {
        key: s.tab,
        onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: s.tab })),
        style: { display: 'flex', gap: 10, alignItems: 'flex-start', flex: '1 1 220px', padding: '10px 14px', border: '1px solid var(--b3)', borderRadius: 'var(--r)', cursor: 'pointer', background: 'var(--b1)', transition: 'border-color .15s' },
        onMouseEnter: e => e.currentTarget.style.borderColor = 'var(--accent)',
        onMouseLeave: e => e.currentTarget.style.borderColor = 'var(--b3)',
      },
        h('span', { style: { fontSize: 20, flexShrink: 0, lineHeight: 1 } }, s.icon),
        h('div', null,
          h('div', { style: { fontWeight: 600, fontSize: 'var(--fsm)', color: 'var(--t1)' } }, s.label, h('span', { style: { marginLeft: 4, color: 'var(--accent)' } }, '→')),
          h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 2, lineHeight: 1.5 } }, s.desc)
        )
      ))
    )
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// OFFLINE BANNER — shown in any tab when A1 gateway is not reachable
// ─────────────────────────────────────────────────────────────────────────────

function OfflineBanner({ health }) {
  if (health !== null && health !== false) return null;

  return h('div', {
    style: {
      background: 'rgba(239,68,68,.07)',
      border: '1px solid rgba(239,68,68,.28)',
      borderRadius: 'var(--r)',
      padding: '10px 14px',
      marginBottom: 16,
      display: 'flex',
      gap: 10,
      alignItems: 'center',
    }
  },
    h('span', { style: { fontSize: 20, flexShrink: 0 } }, '🔴'),
    h('div', { style: { flex: 1 } },
      h('div', { style: { fontWeight: 700, color: '#ef4444', fontSize: 'var(--fsm)' } }, 'A1 is not running'),
      h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', marginTop: 2 } },
        'Your agents cannot authorize any actions until A1 is back online.')
    ),
    h('button', {
      className: 'btn btn-sm',
      style: { flexShrink: 0, background: 'rgba(239,68,68,.12)', color: '#ef4444', border: '1px solid rgba(239,68,68,.3)', borderRadius: 'var(--r)', padding: '5px 12px', cursor: 'pointer', fontWeight: 600, fontSize: 'var(--fxs)' },
      onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'lifecycle' }))
    }, 'Start A1 →')
  );
}
