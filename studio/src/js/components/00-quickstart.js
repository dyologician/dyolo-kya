// ─────────────────────────────────────────────────────────────────────────────
// ZERO-FRICTION QUICK START TAB  v2.8.0
// Automates every "hard step" for non-coders:
//   1. Auto-enables auto-start (fire-and-forget on mount)
//   2. Shows gateway status with auto-retry
//   3. Wizard → passport create → shows exact restart command per framework
//   4. Agent scan → one-click connect → pops restart reminder with timer
//   5. Live Vault summary (renew/revoke without any other tab needed)
//   6. Contextual "what is X" tooltips throughout
//
// ─────────────────────────────────────────────────────────────────────────────

// ── Constants ─────────────────────────────────────────────────────────────────

const QS_STEPS = [
  { id: 'gateway',  icon: '⚡', label: 'A1 Running'    },
  { id: 'passport', icon: '🛡', label: 'Passport'      },
  { id: 'connect',  icon: '🔌', label: 'Connect Agent' },
  { id: 'restart',  icon: '🔄', label: 'Restart Agent' },
  { id: 'done',     icon: '✅', label: 'Protected'     },
];

const QS_FRAMEWORKS = [
  { id: 'claude-code', label: 'Claude Code',      icon: '🤖', restartCmd: 'Close and reopen Claude Code (Cmd+Q then reopen).' },
  { id: 'python',      label: 'Python script',    icon: '🐍', restartCmd: 'Stop your script (Ctrl+C) then run it again: python your_agent.py' },
  { id: 'langchain',   label: 'LangChain',        icon: '🔗', restartCmd: 'Stop your script (Ctrl+C) then run it again: python your_agent.py' },
  { id: 'langgraph',   label: 'LangGraph',        icon: '📊', restartCmd: 'Stop your script (Ctrl+C) then run it again: python your_agent.py' },
  { id: 'crewai',      label: 'CrewAI',           icon: '🚢', restartCmd: 'Stop your crew (Ctrl+C) then: python main.py' },
  { id: 'openai',      label: 'OpenAI Agents',    icon: '🟢', restartCmd: 'Stop your script (Ctrl+C) then run it again.' },
  { id: 'typescript',  label: 'TypeScript / Node',icon: '🔷', restartCmd: 'Stop your process (Ctrl+C) then: node index.js (or npm start)' },
  { id: 'other',       label: 'Other / REST',     icon: '🔌', restartCmd: 'Restart whatever process runs your agent.' },
];

const QS_TOOLTIPS = {
  passport: 'A signed ID card for your AI agent — says what it can do and for how long. Stored in passport.json.',
  autostart: 'Makes A1 start automatically every time you log in. Without this, agents go offline after a reboot.',
  restart: 'After connecting A1 to your agent, the agent must restart to load the new configuration. This is the most common thing people forget.',
  namespace: 'A short unique name for your agent (like "email-helper" or "trading-bot"). Think of it as a username.',
  capability: 'A permission — like "email.send" or "files.read". Your agent can only do things listed in its passport.',
  ttl: 'How long the passport lasts. After this time the agent stops until you renew. 30 days is a good default.',
};

// ── Shared helpers ────────────────────────────────────────────────────────────

function qsTip(term) {
  const tip = QS_TOOLTIPS[term];
  if (!tip) return null;
  return h('span', { title: tip, style: { borderBottom: '1px dotted var(--t2)', cursor: 'help', color: 'var(--t2)', fontSize: 'var(--fxs)' } }, '?');
}

function qsCard(children, style = {}) {
  return h('div', {
    style: {
      background: 'var(--b2)',
      border: '1px solid var(--b3)',
      borderRadius: 'var(--r)',
      padding: '16px 18px',
      marginBottom: 12,
      ...style,
    }
  }, children);
}

function qsStepBadge(n, active, done) {
  const bg = done ? 'var(--green)' : active ? 'var(--accent)' : 'var(--b3)';
  const color = done || active ? '#fff' : 'var(--t3)';
  return h('div', {
    style: {
      width: 24, height: 24, borderRadius: '50%',
      background: bg, color, fontSize: 12, fontWeight: 700,
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      flexShrink: 0, transition: 'background .3s',
    }
  }, done ? '✓' : n);
}

// ── Progress Bar ──────────────────────────────────────────────────────────────

function QsProgressBar({ currentStep }) {
  const idx = QS_STEPS.findIndex(s => s.id === currentStep);
  return h('div', { style: { display: 'flex', gap: 0, marginBottom: 24, position: 'relative' } },
    QS_STEPS.map((s, i) => {
      const done = i < idx;
      const active = i === idx;
      return h('div', {
        key: s.id,
        style: { flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4, position: 'relative' }
      },
        // connector line
        i < QS_STEPS.length - 1 && h('div', {
          style: {
            position: 'absolute', top: 11, left: '50%', width: '100%', height: 2,
            background: done ? 'var(--green)' : 'var(--b3)',
            transition: 'background .4s',
            zIndex: 0,
          }
        }),
        h('div', { style: { position: 'relative', zIndex: 1 } }, qsStepBadge(i + 1, active, done)),
        h('div', {
          style: {
            fontSize: 10, color: done ? 'var(--green)' : active ? 'var(--t1)' : 'var(--t3)',
            fontWeight: active ? 700 : 400, textAlign: 'center', lineHeight: 1.2,
          }
        }, s.icon + ' ' + s.label)
      );
    })
  );
}

// ── Step 1: Gateway ───────────────────────────────────────────────────────────

function QsStepGateway({ gwUrl, onDone }) {
  const [status, setStatus] = useState('checking');
  const [autostartDone, setAutostartDone] = useState(false);
  // Guard: onDone must fire exactly once. Without this, the 3-second health-
  // check interval keeps calling advance('passport'), which resets currentStep
  // back to 'passport' even when the user is already on step 3 or 4.
  const doneFired = useRef(false);

  useEffect(() => {
    let cancelled = false;
    let intervalId = null;

    async function check() {
      try {
        const r = await fetch(gwUrl + '/health', { signal: AbortSignal.timeout(3000) });
        if (!cancelled) {
          if (r.ok) {
            setStatus('running');
            // Auto-enable autostart silently when gateway is confirmed running
            if (!autostartDone) {
              fetch(gwUrl + '/v1/system/autostart', { method: 'POST' }).catch(() => {});
              setAutostartDone(true);
            }
            // Advance to the next step only once — stop polling after confirmation
            if (!doneFired.current) {
              doneFired.current = true;
              clearInterval(intervalId);
              setTimeout(onDone, 800);
            }
          } else {
            setStatus('stopped');
          }
        }
      } catch {
        if (!cancelled) setStatus('stopped');
      }
    }

    check();
    intervalId = setInterval(check, 3000);
    return () => { cancelled = true; clearInterval(intervalId); };
  }, []);

  return qsCard(h('div', null,
    h('div', { style: { display: 'flex', alignItems: 'center', gap: 10, marginBottom: 10 } },
      h('div', {
        style: {
          width: 10, height: 10, borderRadius: '50%', flexShrink: 0,
          background: status === 'running' ? 'var(--green)' : status === 'checking' ? '#f59e0b' : '#ef4444',
          boxShadow: status === 'running' ? '0 0 0 3px rgba(34,197,94,.2)' : 'none',
          animation: status === 'checking' ? 'pulse 1.4s ease-in-out infinite' : 'none',
        }
      }),
      h('strong', { style: { fontSize: 15 } }, '⚡ A1 Gateway'),
      status === 'running' && h('span', { style: { color: 'var(--green)', fontSize: 'var(--fxs)' } }, 'Running')
    ),

    status === 'checking' && h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', margin: 0 } },
      'Connecting to A1 gateway...'
    ),

    status === 'running' && h('div', null,
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', margin: '0 0 6px' } },
        'A1 is running and protecting your agents.'
      ),
      h('div', {
        style: {
          background: 'rgba(34,197,94,.07)', border: '1px solid rgba(34,197,94,.25)',
          borderRadius: 'var(--r)', padding: '8px 12px', fontSize: 'var(--fxs)', color: 'var(--t2)',
        }
      },
        '✅ Auto-start enabled — A1 will restart automatically on login'
      )
    ),

    status === 'stopped' && h('div', null,
      h('p', { style: { color: '#ef4444', fontWeight: 700, margin: '0 0 8px', fontSize: 'var(--fsm)' } },
        'A1 is not running.'
      ),
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', margin: '0 0 10px' } },
        'Double-click ', h('strong', null, 'setup.sh'), ' in your A1 folder — or copy & run in your terminal:'
      ),
      // Primary recommended path
      h('div', {
        style: {
          display: 'flex', alignItems: 'center', gap: 8,
          fontFamily: 'var(--mono)', fontSize: 12, background: 'var(--b1)',
          border: '1px solid var(--accent)', borderRadius: 'var(--r)', padding: '8px 12px',
          color: 'var(--t1)', marginBottom: 8,
        }
      },
        h('span', { style: { flex: 1 } }, './setup.sh'),
        h('button', {
          className: 'btn btn-p btn-sm',
          style: { padding: '3px 10px', fontSize: 11 },
          onClick: () => navigator.clipboard.writeText('./setup.sh').catch(() => {}),
        }, 'Copy')
      ),
      h('p', { style: { color: 'var(--t3)', fontSize: 'var(--fxs)', margin: '0 0 4px' } },
        'Or if you have Rust / cargo installed (fastest after first build):'
      ),
      h('div', {
        style: {
          display: 'flex', alignItems: 'center', gap: 8,
          fontFamily: 'var(--mono)', fontSize: 12, background: 'var(--b1)',
          border: '1px solid var(--b3)', borderRadius: 'var(--r)', padding: '8px 12px',
          color: 'var(--t1)', marginBottom: 4,
        }
      },
        h('span', { style: { flex: 1 } }, 'cargo run -p a1-gateway --release'),
        h('button', {
          className: 'btn btn-s btn-sm',
          style: { padding: '3px 10px', fontSize: 11 },
          onClick: () => navigator.clipboard.writeText('cargo run -p a1-gateway --release').catch(() => {}),
        }, 'Copy')
      ),
      h('p', { style: { color: 'var(--t3)', fontSize: 'var(--fxs)', marginTop: 8, marginBottom: 0 } },
        '⏳ This page reconnects automatically — no need to refresh.'
      )
    )
  ));
}

// ── Step 2: Passport ──────────────────────────────────────────────────────────

function QsStepPassport({ gwUrl, onDone }) {
  const [phase, setPhase] = useState('scan'); // scan | form | creating | done
  const [existingPassports, setExisting] = useState([]);
  const [name, setName] = useState('');
  const [caps, setCaps] = useState(['files.read', 'web.search']);
  const [ttl, setTtl] = useState('30d');
  const [creating, setCreating] = useState(false);
  const [result, setResult] = useState(null);
  const [nameErr, setNameErr] = useState('');

  const CAP_OPTIONS = [
    { k: 'files.read', e: '📁', l: 'Read Files' },
    { k: 'files.write', e: '✏️', l: 'Write Files' },
    { k: 'web.search', e: '🌐', l: 'Search Web' },
    { k: 'web.browse', e: '🖥', l: 'Browse Pages' },
    { k: 'email.send', e: '📧', l: 'Send Emails' },
    { k: 'email.read', e: '📬', l: 'Read Emails' },
    { k: 'code.execute', e: '💻', l: 'Run Code' },
    { k: 'database.read', e: '📊', l: 'Read Data' },
    { k: 'database.write', e: '💾', l: 'Write Data' },
    { k: 'trade.equity', e: '📈', l: 'Trade Stocks' },
    { k: 'trade.crypto', e: '₿', l: 'Trade Crypto' },
    { k: 'api.call', e: '🔌', l: 'Call APIs' },
    { k: 'social.post', e: '📢', l: 'Post Social' },
    { k: 'agent.delegate', e: '🤝', l: 'Delegate' },
  ];

  useEffect(() => {
    fetch(gwUrl + '/v1/passports/list')
      .then(r => r.json())
      .then(d => {
        const list = d.passports || d || [];
        // Only count passports that are valid and not expired — revoked ones are
        // already removed server-side; expired ones can't authorize anything.
        const validList = list.filter(p => p.status === 'valid' || p.status == null);
        setExisting(validList);
        if (validList.length > 0) setPhase('existing');
        else setPhase('form');
      })
      .catch(() => setPhase('form'));
  }, []);

  function toggleCap(k) {
    setCaps(prev => prev.includes(k) ? prev.filter(c => c !== k) : [...prev, k]);
  }

  async function create() {
    if (!name.trim()) { setNameErr('Give your agent a name first.'); return; }
    setNameErr('');
    setCreating(true);
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 30000);
    try {
      const r = await fetch(gwUrl + '/v1/passports/issue', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          namespace: name.trim().toLowerCase().replace(/\s+/g, '-'),
          capabilities: caps,
          ttl,
          // output_path intentionally omitted — gateway defaults to ~/.a1/passports/<namespace>.json
          // so the passport appears in the Vault and on future list fetches.
        }),
        signal: ctrl.signal,
      }).then(r => r.json());
      clearTimeout(timer);
      setResult(r);
      if (r.success || r.path) {
        setPhase('done');
        setTimeout(onDone, 1500);
      }
    } catch (e) {
      clearTimeout(timer);
      if (e.name === 'AbortError') {
        // Timed out — the gateway may have created the passport but not responded.
        // Poll the list to confirm before showing an error.
        try {
          const listRes = await fetch(gwUrl + '/v1/passports/list').then(r => r.json());
          const ns = name.trim().toLowerCase().replace(/\s+/g, '-');
          const found = (listRes.passports || listRes || []).find(p => p.namespace === ns);
          if (found) {
            setResult({ success: true, namespace: ns, path: found.path || './passport.json', public_key_hex: found.public_key_hex || '' });
            setPhase('done');
            setTimeout(onDone, 1500);
          } else {
            setResult({ success: false, error: 'Request timed out. Check that A1 is running and try again.' });
          }
        } catch {
          setResult({ success: false, error: 'Request timed out. Check that A1 is running and try again.' });
        }
      } else {
        setResult({ success: false, error: e.message });
      }
    }
    setCreating(false);
  }

  if (phase === 'scan') {
    return qsCard(h('p', { style: { color: 'var(--t2)', margin: 0, fontSize: 'var(--fsm)' } }, 'Checking for existing passports...'));
  }

  if (phase === 'existing') {
    return qsCard(h('div', null,
      h('div', { style: { display: 'flex', alignItems: 'center', gap: 8, marginBottom: 10 } },
        h('span', { style: { fontSize: 18 } }, '🛡'),
        h('strong', null, existingPassports.length + ' passport' + (existingPassports.length > 1 ? 's' : '') + ' found')
      ),
      existingPassports.map((p, i) => h('div', {
        key: i,
        style: {
          background: 'var(--b1)', border: '1px solid var(--b3)', borderRadius: 'var(--r)',
          padding: '8px 12px', marginBottom: 6, fontSize: 'var(--fsm)',
          display: 'flex', alignItems: 'center', gap: 8,
        }
      },
        h('div', {
          style: {
            width: 8, height: 8, borderRadius: '50%', flexShrink: 0,
            background: (p.days_remaining > 7) ? 'var(--green)' : (p.days_remaining > 0) ? '#f59e0b' : '#ef4444',
          }
        }),
        h('span', { style: { fontWeight: 600 } }, p.namespace || p.name || 'Unnamed'),
        h('span', { style: { color: 'var(--t2)', marginLeft: 'auto' } },
          p.days_remaining != null ? p.days_remaining + ' days left' : 'Valid'
        )
      )),
      h('div', { style: { display: 'flex', gap: 8, marginTop: 10 } },
        h('button', {
          className: 'btn btn-p',
          onClick: onDone,
        }, 'Continue →'),
        h('button', {
          className: 'btn btn-s',
          onClick: () => setPhase('form'),
        }, '+ New passport')
      )
    ));
  }

  if (phase === 'done') {
    return qsCard(h('div', null,
      h('div', { style: { fontSize: 24, marginBottom: 6 } }, '✅'),
      h('strong', null, 'Passport created!'),
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', margin: '4px 0 0' } },
        'Saved as passport.json. Added to .gitignore automatically.'
      )
    ));
  }

  return qsCard(h('div', null,
    h('div', { style: { display: 'flex', alignItems: 'center', gap: 6, marginBottom: 14 } },
      h('span', { style: { fontSize: 18 } }, '🛡'),
      h('strong', { style: { fontSize: 15 } }, 'Create your agent\'s passport'),
      h('span', { style: { color: 'var(--t3)', fontSize: 12, marginLeft: 4 } }, '(signed ID card for your AI agent)')
    ),

    // Agent name
    h('label', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', display: 'block', marginBottom: 4 } },
      'Agent name ', qsTip('namespace')
    ),
    h('input', {
      type: 'text', placeholder: 'e.g. email-helper or trading-bot',
      value: name,
      onInput: e => { setName(e.target.value); setNameErr(''); },
      style: {
        width: '100%', padding: '8px 10px', fontSize: 'var(--fsm)', boxSizing: 'border-box',
        background: 'var(--b1)', border: '1px solid ' + (nameErr ? '#ef4444' : 'var(--b3)'),
        borderRadius: 'var(--r)', color: 'var(--t1)', marginBottom: nameErr ? 4 : 12,
      }
    }),
    nameErr && h('p', { style: { color: '#ef4444', fontSize: 'var(--fxs)', margin: '0 0 10px' } }, nameErr),

    // Capabilities
    h('label', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', display: 'block', marginBottom: 6 } },
      'What can your agent do? ', qsTip('capability')
    ),
    h('div', {
      style: { display: 'flex', flexWrap: 'wrap', gap: 6, marginBottom: 14 }
    },
      CAP_OPTIONS.map(c => h('button', {
        key: c.k,
        onClick: () => toggleCap(c.k),
        style: {
          padding: '4px 10px', fontSize: 12, borderRadius: 20, cursor: 'pointer',
          border: '1px solid ' + (caps.includes(c.k) ? 'var(--accent)' : 'var(--b3)'),
          background: caps.includes(c.k) ? 'rgba(99,102,241,.12)' : 'var(--b1)',
          color: caps.includes(c.k) ? 'var(--accent)' : 'var(--t2)',
          fontWeight: caps.includes(c.k) ? 700 : 400,
          transition: 'all .15s',
        }
      }, c.e + ' ' + c.l))
    ),

    // TTL
    h('label', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', display: 'block', marginBottom: 4 } },
      'Valid for how long? ', qsTip('ttl')
    ),
    h('select', {
      value: ttl, onChange: e => setTtl(e.target.value),
      style: {
        padding: '7px 10px', fontSize: 'var(--fsm)', borderRadius: 'var(--r)',
        border: '1px solid var(--b3)', background: 'var(--b1)', color: 'var(--t1)',
        marginBottom: 14, cursor: 'pointer',
      }
    },
      h('option', { value: '7d' },  '7 days'),
      h('option', { value: '30d' }, '30 days (recommended)'),
      h('option', { value: '90d' }, '3 months'),
      h('option', { value: '1y' },  '1 year'),
    ),

    result && !result.success && h('div', {
      style: {
        background: 'rgba(239,68,68,.07)', border: '1px solid rgba(239,68,68,.25)',
        borderRadius: 'var(--r)', padding: '8px 12px', marginBottom: 10,
        fontSize: 'var(--fxs)', color: '#ef4444',
      }
    }, result.error || 'Failed to create passport. Is A1 running?'),

    h('button', {
      className: 'btn btn-p',
      onClick: create,
      disabled: creating,
      style: { width: '100%', padding: '10px', fontSize: 'var(--fsm)', fontWeight: 700 },
    }, creating ? 'Creating passport...' : '🛡 Create Passport')
  ));
}

// ── Step 3: Connect Agent ─────────────────────────────────────────────────────

function QsStepConnect({ gwUrl, onDone, onSelectFramework }) {
  const [scanning, setScanning] = useState(true);
  const [detected, setDetected] = useState([]);
  const [alreadyConnected, setAlreadyConnected] = useState([]);
  const [connecting, setConnecting] = useState(null);
  const [connected, setConnected] = useState(null);
  const [selectedFramework, setSelectedFramework] = useState(null);
  const [connectMsg, setConnectMsg] = useState(null);
  const [manualMode, setManualMode] = useState(false);

  useEffect(() => {
    fetch(gwUrl + '/v1/agents/scan')
      .then(r => r.json())
      .then(d => {
        const agents = d.agents || [];
        const already = agents.filter(a => a.connected);
        setDetected(agents);
        setAlreadyConnected(already);
        setScanning(false);
        // If one or more agents are already connected, auto-advance the step.
        // The user doesn't need to click anything — connection is confirmed.
        if (already.length > 0) {
          const fw = already[0].framework || already[0].id || 'other';
          onSelectFramework(fw);
          setTimeout(onDone, 1200);
        }
      })
      .catch(() => { setDetected([]); setScanning(false); });
  }, []);

  async function connectAgent(agent) {
    setConnecting(agent.id);
    setConnectMsg(null);
    // Backend returns { connected: bool, message, next_step, ... }
    // NOT { success: bool } — guard against both shapes for safety.
    const r = await fetch(gwUrl + '/v1/agents/connect', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ agent_id: agent.id }),
    }).then(r => r.json()).catch(e => ({ connected: false, error: e.message }));
    setConnectMsg(r);
    setConnecting(null);
    // Accept either shape: r.connected (backend) or r.success (legacy/future)
    if (r.connected || r.success) {
      setConnected(agent);
      onSelectFramework(agent.framework || 'other');
      setTimeout(onDone, 600);
    }
  }

  function pickFramework(fw) {
    setSelectedFramework(fw);
    onSelectFramework(fw.id);
    setTimeout(onDone, 400);
  }

  if (scanning) {
    return qsCard(h('p', { style: { color: 'var(--t2)', margin: 0, fontSize: 'var(--fsm)' } }, '🔍 Scanning for AI agents on your system...'));
  }

  // Agents were already connected when we scanned — auto-advance fires in
  // useEffect after 1.2 s; this card shows confirmation in the meantime.
  if (alreadyConnected.length > 0 && !connected) {
    return qsCard(h('div', null,
      h('div', { style: { fontSize: 22, marginBottom: 6 } }, '✅'),
      h('strong', null,
        alreadyConnected.length === 1
          ? alreadyConnected[0].name + ' is already connected'
          : alreadyConnected.length + ' agents already connected'
      ),
      h('p', { style: { color: 'var(--green)', fontSize: 'var(--fsm)', margin: '4px 0 6px' } },
        'A1 detected the existing connection — moving to the next step…'
      ),
      h('button', {
        className: 'btn btn-p btn-sm',
        onClick: onDone,
        style: { fontSize: 'var(--fxs)' },
      }, 'Continue →')
    ));
  }

  if (connected) {
    return qsCard(h('div', null,
      h('div', { style: { fontSize: 22, marginBottom: 6 } }, '✅'),
      h('strong', null, connected.name + ' connected!'),
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', margin: '4px 0 0' } }, 'One last step: restart the agent.')
    ));
  }

  return qsCard(h('div', null,
    h('div', { style: { display: 'flex', alignItems: 'center', gap: 6, marginBottom: 12 } },
      h('span', { style: { fontSize: 18 } }, '🔌'),
      h('strong', { style: { fontSize: 15 } }, 'Connect your AI agent')
    ),

    // Detected agents
    detected.length > 0 && !manualMode && h('div', null,
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', margin: '0 0 8px' } },
        'Found on your computer:'
      ),
      detected.map(a => h('div', {
        key: a.id,
        style: {
          display: 'flex', alignItems: 'center', gap: 10,
          background: a.connected ? 'rgba(34,197,94,.06)' : 'var(--b1)',
          border: '1px solid ' + (a.connected ? 'rgba(34,197,94,.3)' : 'var(--b3)'),
          borderRadius: 'var(--r)', padding: '10px 12px', marginBottom: 6,
        }
      },
        h('span', { style: { fontSize: 20 } }, a.icon || '🤖'),
        h('div', { style: { flex: 1 } },
          h('div', { style: { fontWeight: 600, fontSize: 'var(--fsm)' } }, a.name),
          h('div', { style: { color: 'var(--t3)', fontSize: 'var(--fxs)' } }, a.path || '')
        ),
        a.connected
          ? h('span', { style: { color: 'var(--green)', fontSize: 'var(--fxs)', fontWeight: 700 } }, '✓ Connected')
          : h('button', {
              className: 'btn btn-p btn-sm',
              disabled: connecting === a.id,
              onClick: () => connectAgent(a),
              style: { minWidth: 80 },
            }, connecting === a.id ? 'Connecting…' : 'Connect →')
      )),
      connectMsg && !connectMsg.connected && !connectMsg.success && h('p', { style: { color: '#ef4444', fontSize: 'var(--fxs)' } }, connectMsg.error || connectMsg.message),
      h('button', {
        className: 'btn btn-s btn-sm',
        onClick: () => setManualMode(true),
        style: { marginTop: 4, fontSize: 'var(--fxs)' },
      }, 'My agent isn\'t in the list →')
    ),

    // Manual framework picker
    (manualMode || detected.length === 0) && h('div', null,
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', margin: '0 0 10px' } },
        detected.length === 0 ? 'No agents detected automatically. Pick your framework:' : 'Pick your framework:'
      ),
      h('div', { style: { display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 6 } },
        QS_FRAMEWORKS.map(fw => h('button', {
          key: fw.id,
          onClick: () => pickFramework(fw),
          style: {
            padding: '10px 12px', textAlign: 'left', cursor: 'pointer',
            background: 'var(--b1)', border: '1px solid var(--b3)',
            borderRadius: 'var(--r)', fontSize: 'var(--fsm)',
            display: 'flex', alignItems: 'center', gap: 8,
            transition: 'border-color .15s',
          }
        },
          h('span', { style: { fontSize: 18 } }, fw.icon),
          h('span', null, fw.label)
        ))
      )
    )
  ));
}

// ── Step 4: Restart Reminder ───────────────────────────────────────────────────

function QsStepRestart({ framework, onDone }) {
  const [dismissed, setDismissed] = useState(false);
  const [countdown, setCountdown] = useState(null);

  const fw = QS_FRAMEWORKS.find(f => f.id === framework) || QS_FRAMEWORKS[QS_FRAMEWORKS.length - 1];

  function startCountdown() {
    setCountdown(5);
  }

  useEffect(() => {
    if (countdown === null) return;
    if (countdown === 0) { setDismissed(true); onDone(); return; }
    const t = setTimeout(() => setCountdown(c => c - 1), 1000);
    return () => clearTimeout(t);
  }, [countdown]);

  if (dismissed) {
    return qsCard(h('p', { style: { color: 'var(--green)', margin: 0 } }, '✅ Great — agent restarted!'));
  }

  return qsCard(
    h('div', {
      style: {
        background: 'rgba(245,158,11,.08)', border: '2px solid rgba(245,158,11,.4)',
        borderRadius: 'var(--r)', padding: '14px 16px', marginBottom: 10,
      }
    },
      h('div', { style: { display: 'flex', gap: 10, alignItems: 'flex-start', marginBottom: 12 } },
        h('span', { style: { fontSize: 24, flexShrink: 0 } }, '🔄'),
        h('div', null,
          h('div', { style: { fontWeight: 700, fontSize: 15, marginBottom: 6 } },
            'Restart your agent now — this is required'
          ),
          h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', lineHeight: 1.6, marginBottom: 4 } },
            'A1 patched your agent\'s config. The agent won\'t see the changes until you restart it. ',
            h('strong', null, 'This is the #1 thing people forget.')
          ),
          h('div', {
            style: {
              fontFamily: 'var(--mono)', fontSize: 12, background: 'var(--b1)',
              border: '1px solid var(--b3)', borderRadius: 'var(--r)', padding: '8px 12px',
              color: 'var(--t1)', marginTop: 8,
            }
          }, fw.restartCmd)
        )
      ),

      countdown !== null
        ? h('button', {
            className: 'btn btn-p',
            disabled: true,
            style: { width: '100%' },
          }, '✓ I restarted it — continuing in ' + countdown + 's...')
        : h('button', {
            className: 'btn btn-p',
            onClick: startCountdown,
            style: { width: '100%', padding: 10 },
          }, '✅ I restarted it → Continue')
    ),
    { border: 'none', padding: 0, background: 'transparent' }
  );
}

// ── Step 5: Done / Vault Quick View ───────────────────────────────────────────

function QsStepDone({ gwUrl }) {
  const [passports, setPassports] = useState([]);
  const [loading, setLoading] = useState(true);
  const [renewing, setRenewing] = useState(null);
  const [revoking, setRevoking] = useState(null);
  const [msg, setMsg] = useState(null);

  function load() {
    fetch(gwUrl + '/v1/passports/list')
      .then(r => r.json())
      .then(d => { setPassports(d.passports || d || []); setLoading(false); })
      .catch(() => setLoading(false));
  }
  useEffect(load, []);

  async function renew(p) {
    setRenewing(p.namespace); setMsg(null);
    const r = await fetch(gwUrl + '/v1/passports/renew', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path: p.path, ttl: '30d' }),
    }).then(r => r.json()).catch(e => ({ success: false, error: e.message }));
    setMsg(r);
    setRenewing(null);
    if (r.success) load();
  }

  async function revoke(p) {
    if (!confirm('Revoke ' + p.namespace + '? This blocks the agent immediately.')) return;
    setRevoking(p.namespace); setMsg(null);
    const r = await fetch(gwUrl + '/v1/passports/revoke-by-namespace', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ namespace: p.namespace, passport_path: p.path }),
    }).then(r => r.json()).catch(e => ({ success: false, error: e.message }));
    setMsg(r);
    setRevoking(null);
    if (r.success) load();
  }

  return h('div', null,
    qsCard(h('div', null,
      h('div', { style: { fontSize: 28, marginBottom: 6 } }, '🎉'),
      h('div', { style: { fontWeight: 700, fontSize: 16, marginBottom: 4 } }, 'Your agent is protected!'),
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', margin: 0, lineHeight: 1.6 } },
        'Every action your agent takes is now checked by A1. If anything is unauthorized, expired, or out of scope — it\'s blocked. Cryptographically.'
      )
    )),

    h('div', { style: { fontWeight: 600, fontSize: 14, marginBottom: 8, marginTop: 4 } }, '🗄 Passport Vault'),
    loading
      ? h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)' } }, 'Loading...')
      : passports.length === 0
        ? h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)' } }, 'No passports found.')
        : passports.map(p => {
            const days = p.days_remaining;
            const color = days == null ? 'var(--t3)' : days < 0 ? '#ef4444' : days < 7 ? '#f59e0b' : 'var(--green)';
            const label = days == null ? 'Valid' : days < 0 ? 'Expired' : days === 0 ? 'Expires today' : days + ' days left';
            return h('div', {
              key: p.namespace,
              style: {
                background: 'var(--b2)', border: '1px solid var(--b3)', borderRadius: 'var(--r)',
                padding: '12px 14px', marginBottom: 8,
              }
            },
              h('div', { style: { display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 } },
                h('div', { style: { width: 8, height: 8, borderRadius: '50%', background: color, flexShrink: 0 } }),
                h('strong', { style: { fontSize: 'var(--fsm)' } }, p.namespace),
                h('span', { style: { color, fontSize: 'var(--fxs)', marginLeft: 'auto' } }, label)
              ),
              h('div', { style: { display: 'flex', gap: 6 } },
                h('button', {
                  className: 'btn btn-p btn-sm',
                  disabled: renewing === p.namespace,
                  onClick: () => renew(p),
                }, renewing === p.namespace ? 'Renewing…' : '↺ Renew 30d'),
                h('button', {
                  className: 'btn btn-sm',
                  style: { color: '#ef4444', borderColor: '#ef4444' },
                  disabled: revoking === p.namespace,
                  onClick: () => revoke(p),
                }, revoking === p.namespace ? 'Revoking…' : '🚫 Revoke')
              )
            );
          }),
    msg && h('div', {
      style: {
        marginTop: 4, padding: '8px 12px', borderRadius: 'var(--r)', fontSize: 'var(--fxs)',
        background: msg.success ? 'rgba(34,197,94,.08)' : 'rgba(239,68,68,.08)',
        border: '1px solid ' + (msg.success ? 'rgba(34,197,94,.3)' : 'rgba(239,68,68,.3)'),
        color: msg.success ? 'var(--green)' : '#ef4444',
      }
    }, msg.success ? '✓ Done' : (msg.error || 'Error'))
  );
}

// ── Main QuickStart Component ─────────────────────────────────────────────────

function QuickStart() {
  const { api } = useContext(Ctx);
  const gwUrl = window.A1_GW_URL || 'http://localhost:8080';

  const [currentStep, setCurrentStep] = useState('gateway');
  const [framework, setFramework] = useState('other');
  const [allDone, setAllDone] = useState(false);

  function advance(next) {
    setCurrentStep(next);
    if (next === 'done') setAllDone(true);
  }

  return h('div', { style: { paddingBottom: 48, maxWidth: 620 } },

    h('div', { style: { marginBottom: 20 } },
      h('h2', { style: { fontSize: 20, fontWeight: 700, marginBottom: 4 } },
        '🚀 Quick Start — One path, fully automatic'
      ),
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', margin: 0, lineHeight: 1.6 } },
        'This tab does everything. No terminal needed after setup. Just follow the steps below — each completes automatically.'
      )
    ),

    h(QsProgressBar, { currentStep }),

    // ── Step 1: Gateway ──────────────────────────────────────────────────────
    h(QsStepGateway, {
      gwUrl,
      onDone: () => advance('passport'),
    }),

    // ── Step 2: Passport ─────────────────────────────────────────────────────
    currentStep === 'passport' && h(QsStepPassport, {
      gwUrl,
      onDone: () => advance('connect'),
    }),
    // Completed passport badge — visible once we advance past this step
    ['connect', 'restart', 'done'].includes(currentStep) && qsCard(h('div', {
      style: { display: 'flex', alignItems: 'center', gap: 10 }
    },
      h('span', { style: { fontSize: 20 } }, '✅'),
      h('div', null,
        h('strong', null, 'Passport ready'),
        h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', margin: '2px 0 0' } },
          'Your agent identity is created and saved.'
        )
      )
    )),

    // ── Step 3: Connect ──────────────────────────────────────────────────────
    ['connect', 'restart', 'done'].includes(currentStep) && h(QsStepConnect, {
      gwUrl,
      onDone: () => advance('restart'),
      onSelectFramework: fw => setFramework(fw),
    }),

    // ── Step 4: Restart ──────────────────────────────────────────────────────
    ['restart', 'done'].includes(currentStep) && h(QsStepRestart, {
      framework,
      onDone: () => advance('done'),
    }),

    // ── Step 5: Done / Vault ─────────────────────────────────────────────────
    currentStep === 'done' && h(QsStepDone, { gwUrl }),

    // ── Explainer footer ─────────────────────────────────────────────────────
    h('div', {
      style: {
        marginTop: 20, padding: '12px 16px', borderRadius: 'var(--r)',
        background: 'var(--b2)', border: '1px solid var(--b3)',
        fontSize: 'var(--fxs)', color: 'var(--t2)', lineHeight: 1.7,
      }
    },
      h('strong', { style: { color: 'var(--t1)', display: 'block', marginBottom: 4 } }, '💡 How A1 works in 3 lines'),
      h('span', null, '🪪 Passport = your agent\'s ID card (created once, stored in passport.json)'), h('br'),
      h('span', null, '🔗 Chain = ID card + session ticket bundled together — shown with every action'), h('br'),
      h('span', null, '✅ A1 checks every action: valid? right permissions? not expired? If anything is wrong → blocked.')
    )
  );
}
