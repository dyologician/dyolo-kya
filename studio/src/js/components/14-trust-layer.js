// ─────────────────────────────────────────────────────────────────────────────
// 14-trust-layer.js — Protection status, backup/restore, nudge tips, gallery,
//                     privacy badge, local LLM detection
// ─────────────────────────────────────────────────────────────────────────────

// ── NUDGE TIP SYSTEM ─────────────────────────────────────────────────────────
// Shown inline at most 3 times per tip key, then permanently dismissed.

const TIPS = {
  passport_safety: {
    icon: '🔐',
    title: 'Keep your passport.json safe',
    body: 'Never email or share your passport.json file. It contains your agent\'s signing keys. Store it in your home folder — A1 puts it there by default.',
  },
  renew_early: {
    icon: '⏰',
    title: 'Renew before it expires',
    body: 'You can renew a passport anytime — even weeks before it expires. Renewing early avoids any gap in protection.',
  },
  one_passport_per_agent: {
    icon: '🎯',
    title: 'One passport per agent',
    body: 'Create a separate passport for each AI agent you protect. This way, revoking one agent doesn\'t affect the others.',
  },
  test_after_connecting: {
    icon: '✅',
    title: 'Test after connecting',
    body: 'After connecting an agent, always run a test action. The green "Protected ✓" badge confirms A1 is working.',
  },
};

function getTipShown(key) {
  try { return parseInt(localStorage.getItem('a1_tip_' + key) || '0', 10); } catch { return 3; }
}
function markTipShown(key) {
  try { localStorage.setItem('a1_tip_' + key, String(getTipShown(key) + 1)); } catch {}
}

function NudgeTip({ tipKey }) {
  const tip = TIPS[tipKey];
  const [visible, setVisible] = useState(() => getTipShown(tipKey) < 3);
  if (!tip || !visible) return null;

  function dismiss() {
    markTipShown(tipKey);
    setVisible(false);
  }

  return h('div', {
    style: {
      display: 'flex', gap: 10, alignItems: 'flex-start',
      padding: '9px 12px', marginBottom: 10,
      border: '1px solid rgba(99,102,241,.22)',
      borderRadius: 'var(--r)',
      background: 'rgba(99,102,241,.05)',
      animation: 'fadeIn .3s ease',
    }
  },
    h('span', { style: { fontSize: 18, flexShrink: 0, lineHeight: 1.2 } }, tip.icon),
    h('div', { style: { flex: 1 } },
      h('div', { style: { fontWeight: 600, fontSize: 'var(--fsm)', color: 'var(--t1)', marginBottom: 2 } }, tip.title),
      h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', lineHeight: 1.6 } }, tip.body),
    ),
    h('button', {
      onClick: dismiss,
      style: { background: 'none', border: 'none', cursor: 'pointer', color: 'var(--t3)', fontSize: 16, padding: '0 2px', lineHeight: 1, flexShrink: 0 },
      title: 'Dismiss',
    }, '×')
  );
}

// ── PRIVACY BADGE ─────────────────────────────────────────────────────────────

function PrivacyBadge() {
  const [expanded, setExpanded] = useState(false);

  return h('div', { style: { marginTop: 16 } },
    h('div', {
      onClick: () => setExpanded(e => !e),
      style: {
        display: 'inline-flex', alignItems: 'center', gap: 7,
        padding: '5px 12px', borderRadius: 20,
        border: '1px solid rgba(34,197,94,.25)',
        background: 'rgba(34,197,94,.05)',
        cursor: 'pointer', userSelect: 'none',
      }
    },
      h('span', { style: { color: 'var(--green)', fontSize: 14 } }, '🔒'),
      h('span', { style: { fontSize: 'var(--fxs)', color: 'var(--green)', fontWeight: 600 } }, 'Your keys never leave your computer'),
      h('span', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginLeft: 2 } }, expanded ? '▲' : '▼ learn more')
    ),

    expanded && h('div', {
      style: {
        marginTop: 8, padding: '12px 14px',
        border: '1px solid rgba(34,197,94,.2)',
        borderRadius: 'var(--r)',
        background: 'rgba(34,197,94,.04)',
        display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10,
      }
    },
      [
        { icon: '🖥', title: 'Runs locally', body: 'A1 gateway is a process on your computer. No cloud, no SaaS.' },
        { icon: '🔑', title: 'Keys stay on disk', body: 'Signing keys live in ~/.a1/. They never leave your machine.' },
        { icon: '📡', title: 'No telemetry', body: 'A1 makes no outbound network calls unless you explicitly enable anchors.' },
        { icon: '👁', title: 'Open source', body: 'Every line of code is auditable at github.com/dyologician/a1.' },
      ].map(item => h('div', { key: item.title, style: { display: 'flex', gap: 8 } },
        h('span', { style: { fontSize: 16, flexShrink: 0 } }, item.icon),
        h('div', null,
          h('div', { style: { fontWeight: 600, fontSize: 'var(--fsm)', marginBottom: 2 } }, item.title),
          h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', lineHeight: 1.5 } }, item.body)
        )
      ))
    )
  );
}

// ── PROTECTION STATUS BANNER ──────────────────────────────────────────────────
// Shows on Overview and Connect Agents. "Your agent is protected ✅" with live test.

function ProtectionStatusBanner({ gwUrl }) {
  const [status, setStatus]   = useState(null); // null | 'checking' | 'ok' | 'partial' | 'none' | 'offline'
  const [agents,  setAgents]  = useState([]);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState(null);

  useEffect(() => {
    check();
  }, []);

  async function check() {
    setStatus('checking');
    const [health, pp] = await Promise.all([
      fetch(gwUrl + '/health').then(r => r.json()).catch(() => null),
      fetch(gwUrl + '/v1/passports/list').then(r => r.json()).catch(() => ({ passports: [] })),
    ]);

    if (!health) { setStatus('offline'); return; }

    const valid = (pp.passports || []).filter(p => p.status === 'valid');
    const total = (pp.passports || []).length;
    setAgents(pp.passports || []);

    if (total === 0)      setStatus('none');
    else if (valid.length === total) setStatus('ok');
    else                  setStatus('partial');
  }

  async function runTest() {
    setTesting(true); setTestResult(null);
    const r = await fetch(gwUrl + '/health')
      .then(r => r.json())
      .catch(() => null);
    setTesting(false);
    setTestResult(r ? { ok: true, latency: r.uptime_secs ? 'online' : 'online' } : { ok: false });
  }

  const CONFIG = {
    checking: { icon: '⟳', color: 'var(--t2)', bg: 'var(--b1)', border: 'var(--b3)', text: 'Checking protection status…' },
    ok:       { icon: '✅', color: 'var(--green)', bg: 'rgba(34,197,94,.06)', border: 'rgba(34,197,94,.25)', text: 'All agents are fully protected' },
    partial:  { icon: '🟡', color: '#ca8a04', bg: 'rgba(202,138,4,.06)', border: 'rgba(202,138,4,.25)', text: 'Some passports are expired — renew them to stay protected' },
    none:     { icon: '⚪', color: 'var(--t2)', bg: 'var(--b1)', border: 'var(--b3)', text: 'No passports yet — create one to protect your first agent' },
    offline:  { icon: '🔴', color: '#ef4444', bg: 'rgba(239,68,68,.06)', border: 'rgba(239,68,68,.25)', text: 'A1 is not running — agents cannot authorize any actions' },
  };

  const cfg = CONFIG[status || 'checking'];

  return h('div', {
    style: {
      padding: '10px 14px', border: '1px solid ' + cfg.border,
      borderRadius: 'var(--r)', background: cfg.bg,
      display: 'flex', alignItems: 'center', gap: 10,
      marginBottom: 14,
    }
  },
    h('span', { style: { fontSize: 20, flexShrink: 0 } }, cfg.icon),
    h('div', { style: { flex: 1 } },
      h('div', { style: { fontWeight: 700, color: cfg.color, fontSize: 'var(--fsm)' } }, cfg.text),
      status === 'ok' && agents.length > 0 && h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', marginTop: 2 } },
        agents.map(a => a.namespace || a.filename).join(' · ')
      ),
      status === 'partial' && h('div', {
        style: { color: 'var(--accent)', fontSize: 'var(--fxs)', marginTop: 3, cursor: 'pointer', fontWeight: 600 },
        onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'vault' }))
      }, '→ Open Passport Vault to renew')
    ),

    // Action buttons
    h('div', { style: { display: 'flex', gap: 6, flexShrink: 0 } },
      status === 'ok' && h('button', {
        className: 'btn btn-s btn-sm', disabled: testing,
        onClick: runTest,
      }, testing ? 'Testing…' : '▶ Test it'),
      status === 'none' && h('button', {
        className: 'btn btn-p btn-sm',
        onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'wizard' })),
      }, '🛡 Protect an agent'),
      status === 'offline' && h('button', {
        className: 'btn btn-sm',
        style: { background: 'rgba(239,68,68,.12)', color: '#ef4444', border: '1px solid rgba(239,68,68,.3)', borderRadius: 'var(--r)', padding: '4px 10px', cursor: 'pointer', fontWeight: 600, fontSize: 'var(--fxs)' },
        onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'lifecycle' })),
      }, '⚡ Start A1'),
      status !== 'checking' && h('button', {
        className: 'btn btn-s btn-sm', onClick: check,
      }, '↻')
    ),

    // Test result
    testResult && h('div', {
      style: { fontSize: 'var(--fxs)', color: testResult.ok ? 'var(--green)' : '#ef4444', marginLeft: 8 },
    }, testResult.ok ? '✓ Gateway responded — A1 is working' : '✗ No response — check Start / Stop')
  );
}

// ── PASSPORT BACKUP & RESTORE ─────────────────────────────────────────────────

function PassportBackup({ gwUrl }) {
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [exportResult, setExportResult] = useState(null);
  const [importResult, setImportResult] = useState(null);
  const [passphrase, setPassphrase] = useState('');
  const [showPass, setShowPass]     = useState(false);
  const [open, setOpen]             = useState(false);

  async function exportBackup() {
    if (!passphrase.trim()) { setExportResult({ ok: false, msg: 'Enter a passphrase to protect the backup.' }); return; }
    setExporting(true); setExportResult(null);

    const pp = await fetch(gwUrl + '/v1/passports/list')
      .then(r => r.json()).catch(() => ({ passports: [] }));

    if (!pp.passports || pp.passports.length === 0) {
      setExportResult({ ok: false, msg: 'No passports found to export.' });
      setExporting(false); return;
    }

    const files = await Promise.all(
      pp.passports.map(async p => {
        const content = await fetch(gwUrl + '/v1/passports/read?path=' + encodeURIComponent(p.path))
          .then(r => r.json()).catch(() => null);
        return { path: p.path, namespace: p.namespace, content };
      })
    );

    const payload = {
      a1_backup_version: '2.8.0',
      exported_at: new Date().toISOString(),
      passports: files.filter(f => f.content),
      directory: pp.directory,
    };

    // AES-GCM encryption with the passphrase
    const enc  = new TextEncoder();
    const key  = await crypto.subtle.importKey('raw',
      await crypto.subtle.digest('SHA-256', enc.encode(passphrase)),
      { name: 'AES-GCM' }, false, ['encrypt']);
    const iv   = crypto.getRandomValues(new Uint8Array(12));
    const data = await crypto.subtle.encrypt({ name: 'AES-GCM', iv },
      key, enc.encode(JSON.stringify(payload)));

    const combined = new Uint8Array([...iv, ...new Uint8Array(data)]);
    const b64 = btoa(String.fromCharCode(...combined));

    const blob = new Blob([JSON.stringify({
      format: 'a1-backup-v1', encrypted: true, data: b64,
    }, null, 2)], { type: 'application/json' });
    const url  = URL.createObjectURL(blob);
    const a    = document.createElement('a');
    a.href     = url;
    a.download = 'a1-passports-' + new Date().toISOString().slice(0, 10) + '.bak.json';
    a.click();
    URL.revokeObjectURL(url);

    setExportResult({ ok: true, msg: payload.passports.length + ' passports exported and encrypted.' });
    setExporting(false);
  }

  async function importBackup(file) {
    setImporting(true); setImportResult(null);
    const text = await file.text().catch(() => null);
    if (!text) { setImportResult({ ok: false, msg: 'Could not read file.' }); setImporting(false); return; }

    let parsed;
    try { parsed = JSON.parse(text); }
    catch { setImportResult({ ok: false, msg: 'Invalid backup file format.' }); setImporting(false); return; }

    if (parsed.format !== 'a1-backup-v1') {
      setImportResult({ ok: false, msg: 'This file is not an A1 passport backup.' });
      setImporting(false); return;
    }

    if (!passphrase.trim()) {
      setImportResult({ ok: false, msg: 'Enter the passphrase you used when exporting.' });
      setImporting(false); return;
    }

    const enc = new TextEncoder();
    const dec = new TextDecoder();
    const raw = Uint8Array.from(atob(parsed.data), c => c.charCodeAt(0));
    const iv  = raw.slice(0, 12);
    const ct  = raw.slice(12);

    let payload;
    try {
      const key = await crypto.subtle.importKey('raw',
        await crypto.subtle.digest('SHA-256', enc.encode(passphrase)),
        { name: 'AES-GCM' }, false, ['decrypt']);
      const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, ct);
      payload = JSON.parse(dec.decode(plain));
    } catch {
      setImportResult({ ok: false, msg: 'Wrong passphrase or corrupted backup file.' });
      setImporting(false); return;
    }

    const restored = await fetch(gwUrl + '/v1/passports/restore', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ passports: payload.passports }),
    }).then(r => r.json()).catch(e => ({ success: false, error: e.message }));

    setImportResult(restored.success
      ? { ok: true, msg: (restored.restored || payload.passports.length) + ' passports restored to ' + (payload.directory || '~/.a1/passports/') + '.' }
      : { ok: false, msg: restored.error || 'Restore failed — check the gateway log.' });
    setImporting(false);
  }

  return h('div', { className: 'sg', style: { marginBottom: 12 } },
    h('div', {
      className: 'sg-head',
      style: { cursor: 'pointer', display: 'flex', justifyContent: 'space-between', alignItems: 'center' },
      onClick: () => setOpen(o => !o),
    },
      h('span', null, '💾 Backup & Restore Passports'),
      h('span', { style: { color: 'var(--t3)', fontSize: 'var(--fxs)' } }, open ? '▲ collapse' : '▼ expand')
    ),
    open && h('div', { className: 'sg-body' },
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', lineHeight: 1.6, marginBottom: 12 } },
        'Export all your passport files to an encrypted backup. ',
        h('strong', null, 'Restore on any computer'), ' — new laptop, reinstall, or team sharing.'),

      // Passphrase
      h('div', { className: 'field', style: { marginBottom: 10 } },
        h('label', { className: 'lbl' }, 'Backup passphrase (used for both export and import)'),
        h('div', { style: { display: 'flex', gap: 6 } },
          h('input', {
            className: 'inp', type: showPass ? 'text' : 'password',
            placeholder: 'Choose a strong passphrase you\'ll remember',
            value: passphrase, onChange: e => setPassphrase(e.target.value),
            style: { flex: 1 },
          }),
          h('button', { className: 'btn btn-s btn-sm', onClick: () => setShowPass(p => !p) }, showPass ? 'Hide' : 'Show')
        ),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t3)', marginTop: 4 } },
          '⚠ If you forget this passphrase, the backup cannot be decrypted. Write it down.')
      ),

      h('div', { style: { display: 'flex', gap: 8, flexWrap: 'wrap' } },
        // Export
        h('button', {
          className: 'btn btn-p btn-sm', disabled: exporting,
          onClick: exportBackup,
        }, exporting ? 'Exporting…' : '⬇ Export encrypted backup'),

        // Import
        h('label', {
          className: 'btn btn-s btn-sm',
          style: { cursor: 'pointer', display: 'inline-flex', alignItems: 'center' },
        },
          importing ? 'Importing…' : '⬆ Restore from backup',
          h('input', {
            type: 'file', accept: '.json,.bak.json',
            style: { display: 'none' },
            disabled: importing,
            onChange: e => e.target.files[0] && importBackup(e.target.files[0]),
          })
        ),
      ),

      exportResult && h('div', { style: { marginTop: 8, fontSize: 'var(--fsm)', color: exportResult.ok ? 'var(--green)' : '#ef4444', lineHeight: 1.5 } },
        (exportResult.ok ? '✅ ' : '❌ ') + exportResult.msg),
      importResult && h('div', { style: { marginTop: 8, fontSize: 'var(--fsm)', color: importResult.ok ? 'var(--green)' : '#ef4444', lineHeight: 1.5 } },
        (importResult.ok ? '✅ ' : '❌ ') + importResult.msg),

      importResult?.ok && h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 4 } },
        'Restart your agents to pick up the restored passports.')
    )
  );
}

// ── EXAMPLE GALLERY ──────────────────────────────────────────────────────────

const EXAMPLES = [
  {
    id: 'email-assistant',
    icon: '📧',
    title: 'Email Assistant',
    desc: 'An agent that drafts, reads, and sends emails on your behalf.',
    caps: ['email.send', 'email.read', 'calendar.read'],
    ttl: '30d',
    agentType: 'python',
    scenario: 'You ask: "Draft a reply to the meeting request from Sarah." The agent reads your inbox, writes a reply, then A1 confirms you authorized it before sending.',
  },
  {
    id: 'research-agent',
    icon: '🔬',
    title: 'Research Agent',
    desc: 'Searches the web, reads documents, and summarizes findings.',
    caps: ['web.search', 'web.browse', 'files.read', 'files.write'],
    ttl: '90d',
    agentType: 'python',
    scenario: 'You ask: "Summarize recent papers on quantum error correction." The agent browses, reads PDFs, writes a summary — every file access is cryptographically authorized.',
  },
  {
    id: 'trading-bot',
    icon: '📈',
    title: 'Trading Bot',
    desc: 'Monitors markets and executes trades within your defined boundaries.',
    caps: ['trade.equity', 'trade.crypto', 'database.read', 'api.call'],
    ttl: '7d',
    agentType: 'python',
    scenario: 'Short 7-day passport = auto-expires if you forget to stop it. Every trade has a cryptographic receipt proving it was within your authorized boundaries.',
  },
  {
    id: 'file-organizer',
    icon: '🗂',
    title: 'File Organizer',
    desc: 'Reads, moves, and renames files according to your rules.',
    caps: ['files.read', 'files.write'],
    ttl: '30d',
    agentType: 'claude-code',
    scenario: 'Works great with Claude Code. The agent sorts your downloads folder — A1 ensures it can only touch files.read/write, not email or execute code.',
  },
  {
    id: 'social-scheduler',
    icon: '📱',
    title: 'Social Media Scheduler',
    desc: 'Drafts and schedules posts across platforms.',
    caps: ['social.post', 'files.read', 'web.search'],
    ttl: '30d',
    agentType: 'typescript',
    scenario: 'The agent queues your posts for the week — A1 proves every post was within your approved scope, useful if you\'re posting on behalf of a brand.',
  },
];

function ExampleGallery() {
  const [selected, setSelected]   = useState(null);
  const [loading, setLoading]     = useState(false);

  function loadExample(ex) {
    setSelected(ex.id);
    setLoading(true);
    setTimeout(() => {
      setLoading(false);
      window.dispatchEvent(new CustomEvent('a1-navigate', {
        detail: {
          tab: 'wizard',
          prefill: { name: ex.id, caps: ex.caps, ttl: ex.ttl, agentType: ex.agentType },
        }
      }));
    }, 400);
  }

  return h('div', { style: { paddingBottom: 40, width: '100%' } },
    h('h2', { style: { fontSize: 18, fontWeight: 700, marginBottom: 4 } }, '🧪 Example Agents'),
    h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', lineHeight: 1.6, marginBottom: 16 } },
      'Start from a working example. Click any card to load it into the wizard — capabilities, lifetime, and name are pre-filled.'),

    h('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(auto-fill,minmax(280px,1fr))', gap: 10 } },
      EXAMPLES.map(ex => h('div', {
        key: ex.id,
        onClick: () => loadExample(ex),
        style: {
          padding: '14px', border: '1px solid var(--b3)', borderRadius: 'var(--r)',
          background: 'var(--b1)', cursor: 'pointer', transition: 'border-color .15s',
          display: 'flex', flexDirection: 'column', gap: 8,
          opacity: loading && selected === ex.id ? .7 : 1,
        },
        onMouseEnter: e => e.currentTarget.style.borderColor = 'var(--accent)',
        onMouseLeave: e => e.currentTarget.style.borderColor = 'var(--b3)',
      },
        h('div', { style: { display: 'flex', gap: 10, alignItems: 'center' } },
          h('span', { style: { fontSize: 26, flexShrink: 0 } }, ex.icon),
          h('div', null,
            h('div', { style: { fontWeight: 700, fontSize: 'var(--fsm)', color: 'var(--t1)' } }, ex.title),
            h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 2 } }, ex.desc),
          )
        ),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', lineHeight: 1.6, fontStyle: 'italic' } },
          '"' + ex.scenario + '"'),
        h('div', { style: { display: 'flex', flexWrap: 'wrap', gap: 4, marginTop: 2 } },
          ex.caps.map(c => h('span', { key: c, style: { fontFamily: 'var(--mono)', fontSize: 8, background: 'rgba(99,102,241,.1)', color: 'var(--accent)', padding: '1px 6px', borderRadius: 10, border: '1px solid rgba(99,102,241,.18)' } }, c))
        ),
        h('div', { style: { display: 'flex', alignItems: 'center', justifyContent: 'space-between' } },
          h('span', { style: { fontSize: 'var(--fxs)', color: 'var(--t3)' } }, 'Passport lifetime: ' + ex.ttl),
          h('span', {
            style: { fontSize: 'var(--fxs)', color: 'var(--accent)', fontWeight: 600 },
          }, loading && selected === ex.id ? 'Loading…' : 'Use this →')
        )
      ))
    ),

    h(GuidedNext, { currentTab: 'wizard' })
  );
}

// ── LOCAL LLM DETECTOR ────────────────────────────────────────────────────────
// Detects Ollama (localhost:11434), LM Studio (localhost:1234), llama.cpp (localhost:8000)
// Used inside SnippetGenerator and AI Integration

const LOCAL_LLM_ENDPOINTS = [
  { name: 'Ollama',    url: 'http://localhost:11434/api/tags',  port: 11434, client: 'ollama' },
  { name: 'LM Studio', url: 'http://localhost:1234/v1/models',  port: 1234,  client: 'openai' },
  { name: 'llama.cpp', url: 'http://localhost:8080/v1/models',  port: 8080,  client: 'openai' },
];

function useLocalLLM() {
  const [found, setFound] = useState(null); // null=checking | [] | [{name,port,client,models}]

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const results = [];
      for (const ep of LOCAL_LLM_ENDPOINTS) {
        try {
          const r = await fetch(ep.url, { signal: AbortSignal.timeout(800) });
          if (r.ok) {
            const data = await r.json();
            const models = ep.client === 'ollama'
              ? (data.models || []).map(m => m.name || m.model || m).slice(0, 5)
              : (data.data || []).map(m => m.id || m.name).slice(0, 5);
            results.push({ ...ep, models });
          }
        } catch { /* not running */ }
      }
      if (!cancelled) setFound(results);
    })();
    return () => { cancelled = true; };
  }, []);

  return found;
}

function LocalLLMBanner({ localLLMs }) {
  const [open, setOpen] = useState(false);
  if (!localLLMs || localLLMs.length === 0) return null;

  const llm = localLLMs[0];

  function localSnippet(llm) {
    const model = llm.models[0] || 'llama3';
    if (llm.client === 'ollama') return [
      'from langchain_ollama import OllamaLLM',
      'from a1.passport import a1_guard, PassportClient',
      '',
      '_a1 = PassportClient(gateway_url="http://localhost:8080", passport_path="./passport.json")',
      'llm = OllamaLLM(model="' + model + '")',
      '',
      '@a1_guard(client=_a1, capability="files.read")',
      'async def my_tool(query: str, signed_chain: dict, executor_pk_hex: str) -> str:',
      '    return await llm.ainvoke(query)',
    ].join('\n');

    return [
      'from langchain_openai import ChatOpenAI',
      'from a1.passport import a1_guard, PassportClient',
      '',
      '_a1 = PassportClient(gateway_url="http://localhost:8080", passport_path="./passport.json")',
      'llm = ChatOpenAI(model="' + model + '", base_url="http://localhost:' + llm.port + '/v1", api_key="local")',
      '',
      '@a1_guard(client=_a1, capability="files.read")',
      'async def my_tool(query: str, signed_chain: dict, executor_pk_hex: str) -> str:',
      '    return await llm.ainvoke(query)',
    ].join('\n');
  }

  const snippet = localSnippet(llm);
  const [copied, setCopied] = useState(false);

  function copy() { navigator.clipboard.writeText(snippet); setCopied(true); setTimeout(() => setCopied(false), 1800); }

  return h('div', {
    style: {
      marginBottom: 12, padding: '10px 14px',
      border: '1px solid rgba(34,197,94,.25)',
      borderRadius: 'var(--r)',
      background: 'rgba(34,197,94,.05)',
    }
  },
    h('div', {
      style: { display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer' },
      onClick: () => setOpen(o => !o),
    },
      h('span', { style: { fontSize: 18 } }, '🖥'),
      h('div', { style: { flex: 1 } },
        h('div', { style: { fontWeight: 700, color: 'var(--green)', fontSize: 'var(--fsm)' } },
          llm.name + ' detected on your computer'),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 1 } },
          (llm.models.length > 0 ? 'Models: ' + llm.models.slice(0, 3).join(', ') : 'Running on port ' + llm.port) +
          ' · Click for zero-config A1 snippet'
        ),
      ),
      h('span', { style: { color: 'var(--t3)', fontSize: 'var(--fxs)' } }, open ? '▲' : '▼')
    ),
    open && h('div', { style: { marginTop: 10 } },
      h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginBottom: 6 } },
        'This snippet connects your local ' + llm.name + ' model to A1 — zero cloud, zero API key, fully private.'),
      h('div', { style: { position: 'relative' } },
        h('pre', { style: { fontFamily: 'var(--mono)', fontSize: 'var(--fxs)', background: 'var(--b2)', padding: '10px 12px', borderRadius: 'var(--r)', overflowX: 'auto', margin: 0, border: '1px solid var(--b3)', lineHeight: 1.7 } }, snippet),
        h('button', { className: 'btn btn-p btn-sm', style: { position: 'absolute', top: 8, right: 8 }, onClick: copy },
          copied ? '✓ Copied' : 'Copy')
      ),
      llm.name === 'Ollama' && h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t3)', marginTop: 6 } },
        '$ pip install langchain-ollama a1   ·   Then restart your agent.')
    )
  );
}
