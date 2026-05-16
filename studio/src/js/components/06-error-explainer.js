// ─────────────────────────────────────────────────────────────────────────────
// ERROR EXPLAINER — plain-English error guide with step-by-step fix paths
// ─────────────────────────────────────────────────────────────────────────────

const QUICK_ERRORS = [
  { label: 'Capability not granted',  value: 'CapabilityNotGranted' },
  { label: 'Certificate expired',     value: 'CertificateExpired' },
  { label: 'Invalid signature',       value: 'InvalidSignature' },
  { label: 'Narrowing violation',     value: 'narrowing violation' },
  { label: 'Replay / nonce used',     value: 'NonceAlreadyUsed' },
  { label: 'A1 not running',          value: 'Connection refused' },
  { label: 'Chain missing',           value: 'MissingSignedChain' },
  { label: 'Chain too deep',          value: 'chain depth exceeded' },
  { label: 'Wrong namespace',         value: 'namespace mismatch' },
  { label: 'Sub-cert scope error',    value: 'SubScopeProof' },
  { label: 'Revocation failed',       value: 'RevocationStoreError' },
  { label: 'Passport not found',      value: 'PassportNotFound' },
  { label: 'executor_pk_hex missing', value: 'missing executor_pk_hex' },
];

const SEVERITY_STYLE = {
  high:    { color: '#ef4444', bg: 'rgba(239,68,68,.07)',    border: 'rgba(239,68,68,.25)',    icon: '🔴' },
  medium:  { color: '#ca8a04', bg: 'rgba(202,138,4,.07)',    border: 'rgba(202,138,4,.25)',    icon: '🟡' },
  low:     { color: 'var(--green)', bg: 'rgba(34,197,94,.06)', border: 'rgba(34,197,94,.25)', icon: '🟢' },
  unknown: { color: 'var(--t2)', bg: 'var(--b2)',            border: 'var(--b3)',              icon: '⚪' },
};

// Maps error severity/type → navigation action
const FIX_ACTIONS = {
  'capability':  { tab: 'wizard',    label: '→ Go to Protect My Agent' },
  'expired':     { tab: 'vault',     label: '→ Open Passport Vault' },
  'revoked':     { tab: 'wizard',    label: '→ Issue a new passport' },
  'signature':   { tab: 'wizard',    label: '→ Re-issue passport' },
  'namespace':   { tab: 'vault',     label: '→ Check passport file' },
  'chain':       { tab: 'integrate', label: '→ Open AI Integration' },
  'gateway':     { tab: 'lifecycle', label: '→ Start A1' },
  'narrowing':   { tab: 'wizard',    label: '→ Adjust capabilities' },
  'subcert':     { tab: 'wizard',    label: '→ Re-issue with correct scope' },
  'revocation':  { tab: 'vault',     label: '→ Open Passport Vault' },
  'missing_pk':  { tab: 'integrate', label: '→ Open AI Integration Assistant' },
};

// Client-side plain-English explanations for common errors (shown without a gateway call)
const LOCAL_ERROR_MAP = [
  {
    match: /SubScopeProof|sub.?scope|subcert/i,
    plain_english: 'A sub-delegation certificate tried to claim more permissions than its parent passport allows.',
    likely_cause: 'The sub-cert was issued with capabilities that aren\'t in the root passport.',
    fix: 'Re-issue the sub-cert with only capabilities that appear in the parent passport. Example: if the passport has "files.read", the sub-cert cannot add "files.write".',
    fix_steps: [
      'Open the Passport Vault and check what capabilities the root passport has.',
      'Go to "Protect My Agent" and re-issue the sub-cert with a subset of those capabilities.',
      'Replace the old chain.json with the newly generated one.',
      'Restart your agent.',
    ],
    fix_type: 'subcert',
    severity: 'high',
  },
  {
    match: /RevocationStoreError|revocation.*store|store.*revoc/i,
    plain_english: 'A1 couldn\'t access its revocation store when checking if a passport has been revoked.',
    likely_cause: 'A1 is configured to use Redis or Postgres for revocation storage, but can\'t connect to it.',
    fix: 'For personal use, you don\'t need Redis or Postgres — A1 works fine with in-memory storage. If you\'re running a team deployment, check that your Redis/Postgres URL is correct.',
    fix_steps: [
      'If this is personal use: remove A1_REDIS_URL and A1_PG_URL from your environment and restart A1.',
      'If this is a team deployment: verify that Redis or Postgres is running and the connection URL is correct.',
      'Restart A1 after fixing the configuration.',
    ],
    fix_type: 'revocation',
    severity: 'medium',
  },
  {
    match: /PassportNotFound|passport.*not.*found|no passport/i,
    plain_english: 'A1 can\'t find the passport.json file your agent is pointing to.',
    likely_cause: 'The passport file was moved, renamed, or deleted — or the path in your agent\'s config is wrong.',
    fix: 'Check the Passport Vault to see where your passport files are stored. Then update your agent\'s configuration to use the correct path.',
    fix_steps: [
      'Open the Passport Vault — it shows the exact file path for each passport.',
      'Check whether the file still exists at that path.',
      'If the file was moved: update the path in your agent\'s config (or re-run AI Integration to patch it automatically).',
      'If the file was deleted: re-issue the passport from "Protect My Agent".',
    ],
    fix_type: 'namespace',
    severity: 'high',
  },
  {
    match: /missing.*executor_pk_hex|executor_pk_hex.*missing|executor_pk/i,
    plain_english: 'Your agent\'s tool call is missing the executor_pk_hex field — the agent\'s public key.',
    likely_cause: 'You\'re using the raw REST API or a custom agent, and the executor_pk_hex wasn\'t included in the request body.',
    fix: 'Use the A1Guard helper from templates/any-agent.md — it injects executor_pk_hex automatically from your passport.json so you never have to pass it manually.',
    fix_steps: [
      'Open the AI Integration Assistant and describe your agent framework.',
      'The assistant will generate code that handles executor_pk_hex automatically.',
      'If you\'re using the raw REST API: add "executor_pk_hex": "<your-agent-public-key>" to your request body. The key is in your passport.json under "holder_pk_hex".',
      'Restart your agent after updating the code.',
    ],
    fix_type: 'missing_pk',
    severity: 'medium',
  },
];

function StepFix({ steps, action }) {
  return h('div', { style: { display: 'flex', flexDirection: 'column', gap: 6 } },
    steps.map((step, i) =>
      h('div', { key: i, style: { display: 'flex', gap: 10, alignItems: 'flex-start' } },
        h('div', { style: { width: 22, height: 22, borderRadius: '50%', background: 'var(--accent)', color: '#fff', fontSize: 11, fontWeight: 700, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0, marginTop: 1 } }, i + 1),
        h('div', { style: { fontSize: 'var(--fsm)', color: 'var(--t1)', lineHeight: 1.6, flex: 1 } }, step)
      )
    ),
    action && h('button', {
      className: 'btn btn-p btn-sm',
      style: { marginTop: 8, alignSelf: 'flex-start' },
      onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: action.tab }))
    }, action.label)
  );
}

function ErrorExplainer() {
  const [errText, setErrText] = useState('');
  const [errCode, setErrCode] = useState('');
  const [result,  setResult]  = useState(null);
  const [loading, setLoading] = useState(false);

  async function explain() {
    if (!errText.trim()) return;
    setLoading(true); setResult(null);

    // Check local map first — catches errors even when gateway is down
    const localMatch = LOCAL_ERROR_MAP.find(e => e.match.test(errText));
    if (localMatch) {
      setResult(localMatch);
      setLoading(false);
      return;
    }

    const gwUrl = window.A1_GW_URL || 'http://localhost:8080';
    const r = await fetch(gwUrl + '/v1/debug/explain-error', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: errText, error_code: errCode || undefined }),
    }).then(r => r.json()).catch(() => ({
      plain_english: 'A1 is not running — start it first.',
      likely_cause:  'The gateway process is not running on your machine.',
      fix:           'Go to the Start / Stop tab and start A1.',
      fix_steps:     ['Open the "Start / Stop" tab from the sidebar.', 'Click "Start A1" and run the command shown.', 'Come back and try again.'],
      fix_type:      'gateway',
      severity:      'high',
    }));
    setResult(r);
    setLoading(false);
  }

  const sev  = result ? (SEVERITY_STYLE[result.severity] || SEVERITY_STYLE.unknown) : null;
  const act  = result?.fix_type ? FIX_ACTIONS[result.fix_type] : null;

  const fixSteps = result?.fix_steps || (result?.fix ? [result.fix] : []);

  return h('div', { style: { paddingBottom: 40, width: '100%' } },
    h('h2', { style: { fontSize: 18, fontWeight: 700, marginBottom: 4 } }, '🔎 Error Help'),
    h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', marginBottom: 16, lineHeight: 1.6 } },
      'Paste any A1 error here. Get a plain-English explanation and step-by-step fix.'),

    // Quick pick buttons
    h('div', { className: 'sg' },
      h('div', { className: 'sg-head' }, 'Common errors — click to select'),
      h('div', { className: 'sg-body' },
        h('div', { style: { display: 'flex', gap: 5, flexWrap: 'wrap', marginBottom: 12 } },
          QUICK_ERRORS.map(e => h('button', {
            key: e.value,
            className: 'btn btn-s btn-sm',
            style: { fontFamily: 'var(--mono)' },
            onClick: () => { setErrText(e.value); setResult(null); }
          }, e.label))
        ),
        h('div', { className: 'field' },
          h('label', { className: 'lbl' }, 'Error message or description'),
          h('input', {
            className: 'inp', type: 'text',
            placeholder: 'e.g. "narrowing violation" or paste the full error JSON',
            value: errText,
            onChange: e => { setErrText(e.target.value); setResult(null); },
            onKeyDown: e => e.key === 'Enter' && explain(),
          })
        ),
        h('div', { className: 'field', style: { marginTop: 8 } },
          h('label', { className: 'lbl' }, 'Error code (optional)'),
          h('input', {
            className: 'inp inp-mono', type: 'text',
            placeholder: 'e.g. E4003 or CapabilityNotGranted',
            value: errCode,
            onChange: e => { setErrCode(e.target.value); setResult(null); },
          })
        ),
        h('button', {
          className: 'btn btn-p btn-sm', style: { marginTop: 8 },
          onClick: explain, disabled: loading || !errText.trim()
        }, loading ? 'Explaining…' : 'Explain this error →')
      )
    ),

    // Result card
    result && h('div', {
      style: {
        marginTop: 12, border: '1px solid ' + sev.border, borderRadius: 'var(--r)',
        background: sev.bg, overflow: 'hidden',
      }
    },
      h('div', { style: { padding: '12px 16px', borderBottom: '1px solid ' + sev.border, display: 'flex', gap: 8, alignItems: 'center' } },
        h('span', { style: { fontSize: 20 } }, sev.icon),
        h('span', { style: { fontWeight: 700, fontSize: 'var(--fsm)', color: sev.color } }, result.plain_english)
      ),
      h('div', { style: { padding: '14px 16px', display: 'flex', flexDirection: 'column', gap: 14 } },
        h('div', null,
          h('div', { style: { fontWeight: 600, fontSize: 'var(--fxs)', color: 'var(--t2)', textTransform: 'uppercase', letterSpacing: '.05em', marginBottom: 5 } }, 'Why it happened'),
          h('div', { style: { fontSize: 'var(--fsm)', color: 'var(--t1)', lineHeight: 1.7 } }, result.likely_cause)
        ),
        fixSteps.length > 0 && h('div', null,
          h('div', { style: { fontWeight: 600, fontSize: 'var(--fxs)', color: 'var(--t2)', textTransform: 'uppercase', letterSpacing: '.05em', marginBottom: 8 } }, 'How to fix it'),
          h(StepFix, { steps: fixSteps, action: act })
        )
      )
    ),

    h(GuidedNext, { currentTab: 'errors' })
  );
}
