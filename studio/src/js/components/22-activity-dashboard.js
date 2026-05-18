// ── Activity Dashboard — plain-English view for non-technical users ────────────
// Shows: what's been protected, what was blocked, which agents are active,
// recent activity feed. No jargon. No raw JSON. Just what happened.

const CAPABILITY_LABELS = {
  'files.read':         'Read files',
  'files.write':        'Write files',
  'web.search':         'Search the web',
  'web.fetch':          'Browse websites',
  'shell.run':          'Run terminal commands',
  'shell.exec_privileged': 'Run admin commands',
  'network.raw_socket': 'Open raw network connections',
  'process.kill_system':'Kill system processes',
  'trade.equity':       'Execute trades',
  'portfolio.read':     'Read portfolio',
  'email.send':         'Send emails',
  'calendar.write':     'Edit calendar',
  'governance.vote':    'Vote on proposals',
};

function capLabel(cap) {
  return CAPABILITY_LABELS[cap] || cap;
}

function timeAgo(isoStr) {
  if (!isoStr) return '';
  const diff = Date.now() - new Date(isoStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return mins + 'm ago';
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return hrs + 'h ago';
  return Math.floor(hrs / 24) + 'd ago';
}

function ActivityDashboard() {
  const { api, settings } = useContext(Ctx);
  const gwUrl = settings.gwUrl || 'http://localhost:8080';

  const [passports,  setPassports]  = useState([]);
  const [events,     setEvents]     = useState([]);
  const [loading,    setLoading]    = useState(true);
  const [health,     setHealth]     = useState(null);

  async function load() {
    setLoading(true);
    const [ppR, healthR] = await Promise.all([
      api('GET', '/v1/passports/list'),
      fetch(gwUrl + '/healthz').then(r => r.json()).catch(() => null),
    ]);
    if (ppR.ok) setPassports(ppR.data.passports || []);
    setHealth(healthR);
    // Events would come from audit log — show mock summary for now based on passport data
    setLoading(false);
  }

  useEffect(() => { load(); }, []);

  const total    = passports.length;
  const active   = passports.filter(p => p.days_remaining > 0).length;
  const expiring = passports.filter(p => p.days_remaining !== null && p.days_remaining >= 0 && p.days_remaining < 7).length;
  const expired  = passports.filter(p => p.days_remaining < 0).length;
  const running  = health?.status === 'ok';

  return h('div', { style: { paddingBottom: 40 } },

    h('div', { style: { marginBottom: 20 } },
      h('h2', { style: { fontSize: 18, fontWeight: 700, marginBottom: 4 } }, '📊 Activity Dashboard'),
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', lineHeight: 1.6 } },
        'A plain-English view of what A1 is protecting and what\'s been happening.')),

    // ── Status bar ────────────────────────────────────────────────────────────
    h('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 10, marginBottom: 20 } },
      ...[
        { label: 'A1 Status',         value: running ? '🟢 Running' : '🔴 Not running', color: running ? 'var(--green)' : '#ef4444', action: running ? null : 'start' },
        { label: 'Protected agents',  value: active,   color: 'var(--green)' },
        { label: 'Expiring soon',     value: expiring, color: expiring > 0 ? '#ca8a04' : 'var(--t2)' },
        { label: 'Expired',           value: expired,  color: expired > 0 ? '#ef4444' : 'var(--t2)' },
      ].map(s => h('div', { key: s.label, style: { background: 'var(--b2)', border: '1px solid var(--b3)', borderRadius: 'var(--r)', padding: '14px 16px', cursor: s.action ? 'pointer' : 'default' }, onClick: s.action === 'start' ? () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'lifecycle' })) : undefined },
        h('div', { style: { fontSize: 22, fontWeight: 700, color: s.color, marginBottom: 4 } }, s.value),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)' } }, s.label),
        s.action === 'start' && h('div', { style: { fontSize: 10, color: s.color, marginTop: 4 } }, 'Click to start →')
      ))
    ),

    loading && h('div', { className: 'empty' }, 'Loading…'),

    // ── Protected agents list ─────────────────────────────────────────────────
    !loading && passports.length > 0 && h('div', { className: 'sg', style: { marginBottom: 16 } },
      h('div', { className: 'sg-head' }, '🛡 Protected agents (' + total + ')'),
      h('div', { className: 'sg-body' },
        passports.map(pp => {
          const days = pp.days_remaining;
          const ok   = days > 0;
          const warn = ok && days < 7;
          const color = days < 0 ? '#ef4444' : warn ? '#ca8a04' : 'var(--green)';
          const status = days < 0 ? 'Expired' : warn ? days + ' days left' : 'Active';

          return h('div', { key: pp.namespace, style: { padding: '12px 14px', borderBottom: '1px solid var(--b3)', display: 'flex', alignItems: 'flex-start', gap: 12, flexWrap: 'wrap' } },
            h('div', { style: { width: 8, height: 8, borderRadius: '50%', background: color, marginTop: 6, flexShrink: 0 } }),
            h('div', { style: { flex: 1, minWidth: 0 } },
              h('div', { style: { fontWeight: 600, fontSize: 'var(--fsm)', marginBottom: 4 } }, pp.namespace),
              h('div', { style: { display: 'flex', flexWrap: 'wrap', gap: 5, marginBottom: 4 } },
                (pp.capabilities || []).map(cap =>
                  h('span', { key: cap, style: { fontSize: 10, background: 'rgba(99,102,241,.1)', color: 'var(--accent)', padding: '2px 7px', borderRadius: 10, border: '1px solid rgba(99,102,241,.2)' } },
                    capLabel(cap)
                  )
                )
              )
            ),
            h('div', { style: { textAlign: 'right', flexShrink: 0 } },
              h('div', { style: { fontSize: 'var(--fxs)', fontWeight: 600, color } }, status),
              h('button', {
                className: 'btn btn-s btn-sm',
                style: { fontSize: 'var(--fxs)', marginTop: 4 },
                onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'passports' })),
              }, 'Manage →')
            )
          );
        })
      )
    ),

    !loading && passports.length === 0 && h('div', { className: 'sg', style: { marginBottom: 16 } },
      h('div', { className: 'sg-head' }, '🛡 Protected agents'),
      h('div', { className: 'sg-body' },
        h('div', { style: { padding: '20px 14px', textAlign: 'center', color: 'var(--t2)' } },
          h('div', { style: { fontSize: 32, marginBottom: 8 } }, '🔓'),
          h('div', { style: { fontWeight: 600, marginBottom: 6 } }, 'No agents protected yet'),
          h('div', { style: { fontSize: 'var(--fxs)', marginBottom: 14 } }, 'Create a passport to start protecting your AI agents.'),
          h('button', {
            className: 'btn btn-p',
            onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'quickstart' })),
          }, '🛡 Protect my first agent')
        )
      )
    ),

    // ── What A1 does ──────────────────────────────────────────────────────────
    h('div', { className: 'sg' },
      h('div', { className: 'sg-head' }, '🔍 How A1 protects your agents'),
      h('div', { className: 'sg-body' },
        h('div', { style: { display: 'flex', flexDirection: 'column', gap: 10, padding: '4px 0' } },
          ...[
            { icon: '✅', title: 'Allowed actions go through',     body: 'When your agent does something it\'s authorized for (like searching the web), A1 approves it instantly and keeps a receipt.' },
            { icon: '🚫', title: 'Blocked actions are stopped',    body: 'If your agent tries to do something outside its passport — like opening raw network connections — A1 blocks it before it happens.' },
            { icon: '📋', title: 'Every action gets a receipt',    body: 'A tamper-proof record is created for every action. You can see what happened, when, and which agent did it.' },
            { icon: '⏰', title: 'Access expires automatically',   body: 'Passports have a time limit. When they expire, the agent can\'t act until you renew. No forgotten access.' },
            { icon: '🔑', title: 'You can revoke at any time',    body: 'Click Revoke in the Passports tab and the agent immediately loses all authorization. No waiting, no exceptions.' },
          ].map(({ icon, title, body }) =>
            h('div', { key: title, style: { display: 'flex', gap: 12, padding: '10px 14px', background: 'var(--b1)', borderRadius: 'var(--r)', border: '1px solid var(--b3)' } },
              h('div', { style: { fontSize: 20, flexShrink: 0, marginTop: 1 } }, icon),
              h('div', null,
                h('div', { style: { fontWeight: 600, fontSize: 'var(--fsm)', marginBottom: 3 } }, title),
                h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', lineHeight: 1.6 } }, body)
              )
            )
          )
        )
      )
    ),

    h('div', { style: { marginTop: 12, display: 'flex', gap: 8, justifyContent: 'flex-end' } },
      h('button', { className: 'btn btn-s', onClick: load }, '↺ Refresh')
    ),

    !loading && passports.length > 0 && h(ShareCard, { passports })
  );
}


// ── ShareCard — generate a downloadable/shareable A1 protection card ──────────

function ShareCard({ passports }) {
  const [show,        setShow]        = useState(false);
  const [selected,    setSelected]    = useState([]);   // selected agent namespaces
  const [showAgent,   setShowAgent]   = useState(true);
  const [showCaps,    setShowCaps]    = useState(true);
  const [showCount,   setShowCount]   = useState(true);
  const [customNote,  setCustomNote]  = useState('');
  const [generating,  setGenerating]  = useState(false);
  const canvasRef = useRef(null);

  // Init: select all active passports
  useEffect(() => {
    if (show && selected.length === 0) {
      setSelected(passports.filter(p => p.days_remaining > 0).map(p => p.namespace));
    }
  }, [show]);

  function toggleAgent(ns) {
    setSelected(s => s.includes(ns) ? s.filter(x => x !== ns) : [...s, ns]);
  }

  function drawCard() {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const W = 1200, H = 630;
    canvas.width = W;
    canvas.height = H;

    const agents = passports.filter(p => selected.includes(p.namespace));
    const totalCaps = [...new Set(agents.flatMap(p => p.capabilities || []))];

    // ── Background ──
    const bg = ctx.createLinearGradient(0, 0, W, H);
    bg.addColorStop(0,   '#0a0a0f');
    bg.addColorStop(0.5, '#0d0d1a');
    bg.addColorStop(1,   '#0a0f0a');
    ctx.fillStyle = bg;
    ctx.fillRect(0, 0, W, H);

    // Grid lines
    ctx.strokeStyle = 'rgba(99,102,241,0.06)';
    ctx.lineWidth = 1;
    for (let x = 0; x < W; x += 60) { ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, H); ctx.stroke(); }
    for (let y = 0; y < H; y += 60) { ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(W, y); ctx.stroke(); }

    // Glow orbs
    const glow1 = ctx.createRadialGradient(200, 150, 0, 200, 150, 350);
    glow1.addColorStop(0, 'rgba(99,102,241,0.18)');
    glow1.addColorStop(1, 'transparent');
    ctx.fillStyle = glow1;
    ctx.fillRect(0, 0, W, H);

    const glow2 = ctx.createRadialGradient(1000, 480, 0, 1000, 480, 300);
    glow2.addColorStop(0, 'rgba(34,197,94,0.12)');
    glow2.addColorStop(1, 'transparent');
    ctx.fillStyle = glow2;
    ctx.fillRect(0, 0, W, H);

    // ── Top bar ──
    ctx.fillStyle = 'rgba(99,102,241,0.15)';
    ctx.fillRect(0, 0, W, 4);
    const topBar = ctx.createLinearGradient(0, 0, W, 0);
    topBar.addColorStop(0, '#6366f1');
    topBar.addColorStop(0.5, '#22c55e');
    topBar.addColorStop(1, '#6366f1');
    ctx.fillStyle = topBar;
    ctx.fillRect(0, 0, W, 3);

    // ── A1 Logo badge ──
    ctx.fillStyle = 'rgba(99,102,241,0.2)';
    roundRect(ctx, 52, 48, 72, 36, 8);
    ctx.fill();
    ctx.strokeStyle = 'rgba(99,102,241,0.5)';
    ctx.lineWidth = 1;
    roundRect(ctx, 52, 48, 72, 36, 8);
    ctx.stroke();
    ctx.fillStyle = '#6366f1';
    ctx.font = 'bold 18px monospace';
    ctx.textAlign = 'left';
    ctx.fillText('A1', 72, 71);

    // ── Shield icon area ──
    ctx.font = '52px serif';
    ctx.textAlign = 'left';
    ctx.fillText('🛡', 50, 175);

    // ── Headline ──
    ctx.fillStyle = '#ffffff';
    ctx.font = 'bold 52px -apple-system, BlinkMacSystemFont, sans-serif';
    ctx.textAlign = 'left';
    ctx.fillText('My AI agents are', 52, 250);

    const grad = ctx.createLinearGradient(52, 260, 700, 320);
    grad.addColorStop(0, '#6366f1');
    grad.addColorStop(1, '#22c55e');
    ctx.fillStyle = grad;
    ctx.font = 'bold 56px -apple-system, BlinkMacSystemFont, sans-serif';
    ctx.fillText('cryptographically protected.', 52, 320);

    // ── Agent list ──
    if (showAgent && agents.length > 0) {
      let y = 378;
      ctx.fillStyle = 'rgba(255,255,255,0.06)';
      const rowH = 38, rowW = 520;
      agents.slice(0, 4).forEach((p, i) => {
        roundRect(ctx, 52, y + i * (rowH + 6), rowW, rowH, 6);
        ctx.fill();
        // green dot
        ctx.fillStyle = '#22c55e';
        ctx.beginPath();
        ctx.arc(76, y + i * (rowH + 6) + rowH / 2, 5, 0, Math.PI * 2);
        ctx.fill();
        // namespace
        ctx.fillStyle = '#ffffff';
        ctx.font = '15px monospace';
        ctx.textAlign = 'left';
        ctx.fillText(p.namespace, 92, y + i * (rowH + 6) + rowH / 2 + 5);
        // caps count
        if (showCaps && p.capabilities) {
          ctx.fillStyle = 'rgba(99,102,241,0.9)';
          ctx.font = '12px monospace';
          ctx.fillText(p.capabilities.length + ' capabilities', rowW - 60, y + i * (rowH + 6) + rowH / 2 + 4);
        }
        ctx.fillStyle = 'rgba(255,255,255,0.06)';
      });
      if (agents.length > 4) {
        ctx.fillStyle = 'rgba(255,255,255,0.3)';
        ctx.font = '13px monospace';
        ctx.fillText('+' + (agents.length - 4) + ' more', 60, y + 4 * (rowH + 6) + 18);
      }
    }

    // ── Stats right side ──
    if (showCount) {
      const stats = [
        { label: 'Protected', value: agents.length, color: '#22c55e' },
        { label: 'Capabilities', value: totalCaps.length, color: '#6366f1' },
      ];
      stats.forEach((s, i) => {
        const x = 700 + i * 220, y = 220;
        ctx.fillStyle = 'rgba(255,255,255,0.04)';
        roundRect(ctx, x, y, 180, 120, 12);
        ctx.fill();
        ctx.strokeStyle = i === 0 ? 'rgba(34,197,94,0.3)' : 'rgba(99,102,241,0.3)';
        ctx.lineWidth = 1;
        roundRect(ctx, x, y, 180, 120, 12);
        ctx.stroke();
        ctx.fillStyle = s.color;
        ctx.font = 'bold 44px -apple-system, BlinkMacSystemFont, sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText(s.value, x + 90, y + 68);
        ctx.fillStyle = 'rgba(255,255,255,0.4)';
        ctx.font = '13px -apple-system, BlinkMacSystemFont, sans-serif';
        ctx.fillText(s.label, x + 90, y + 94);
      });
    }

    // ── Custom note ──
    if (customNote.trim()) {
      ctx.fillStyle = 'rgba(255,255,255,0.55)';
      ctx.font = 'italic 18px -apple-system, BlinkMacSystemFont, sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText('"' + customNote.trim().slice(0, 80) + '"', 700, 390);
    }

    // ── Bottom bar ──
    ctx.fillStyle = 'rgba(255,255,255,0.07)';
    ctx.fillRect(0, H - 54, W, 54);

    ctx.fillStyle = 'rgba(255,255,255,0.25)';
    ctx.font = '14px monospace';
    ctx.textAlign = 'left';
    ctx.fillText('github.com/dyologician/A1', 52, H - 22);

    ctx.fillStyle = 'rgba(255,255,255,0.15)';
    ctx.font = '13px monospace';
    ctx.textAlign = 'right';
    ctx.fillText('Cryptographic identity & authorization for AI agents', W - 52, H - 22);
  }

  function roundRect(ctx, x, y, w, h, r) {
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + w - r, y);
    ctx.quadraticCurveTo(x + w, y, x + w, y + r);
    ctx.lineTo(x + w, y + h - r);
    ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
    ctx.lineTo(x + r, y + h);
    ctx.quadraticCurveTo(x, y + h, x, y + h - r);
    ctx.lineTo(x, y + r);
    ctx.quadraticCurveTo(x, y, x + r, y);
    ctx.closePath();
  }

  useEffect(() => {
    if (show) setTimeout(drawCard, 50);
  }, [show, selected, showAgent, showCaps, showCount, customNote]);

  function download() {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const a = document.createElement('a');
    a.download = 'a1-protected.png';
    a.href = canvas.toDataURL('image/png');
    a.click();
  }

  if (!show) return h('button', {
    className: 'btn btn-s',
    style: { display: 'flex', alignItems: 'center', gap: 6 },
    onClick: () => setShow(true),
  }, '🔗 Share protection card');

  const active = passports.filter(p => p.days_remaining > 0);

  return h('div', { style: { marginTop: 16, border: '1px solid var(--b3)', borderRadius: 'var(--r)', overflow: 'hidden' } },

    // Header
    h('div', { style: { padding: '12px 16px', background: 'var(--b2)', borderBottom: '1px solid var(--b3)', display: 'flex', alignItems: 'center', gap: 10 } },
      h('div', { style: { fontWeight: 700, fontSize: 'var(--fsm)', flex: 1 } }, '🔗 Share your protection card'),
      h('button', { className: 'btn btn-sm', style: { fontSize: 'var(--fxs)' }, onClick: () => setShow(false) }, '✕')
    ),

    h('div', { style: { display: 'grid', gridTemplateColumns: '280px 1fr', gap: 0 } },

      // Controls
      h('div', { style: { padding: 16, borderRight: '1px solid var(--b3)', display: 'flex', flexDirection: 'column', gap: 14 } },

        // Agent picker
        h('div', null,
          h('div', { style: { fontSize: 'var(--fxs)', fontWeight: 600, marginBottom: 8, color: 'var(--t1)' } }, 'Which agents to include'),
          h('div', { style: { display: 'flex', flexDirection: 'column', gap: 5 } },
            active.map(p => h('label', { key: p.namespace, style: { display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer', fontSize: 'var(--fxs)' } },
              h('input', { type: 'checkbox', checked: selected.includes(p.namespace), onChange: () => toggleAgent(p.namespace) }),
              h('span', { style: { fontFamily: 'var(--mono)' } }, p.namespace)
            ))
          )
        ),

        // Toggle options
        h('div', null,
          h('div', { style: { fontSize: 'var(--fxs)', fontWeight: 600, marginBottom: 8, color: 'var(--t1)' } }, 'What to show'),
          h('div', { style: { display: 'flex', flexDirection: 'column', gap: 6 } },
            ...[
              { key: 'agent',  label: 'Agent names',         val: showAgent,  set: setShowAgent },
              { key: 'caps',   label: 'Capability count',    val: showCaps,   set: setShowCaps },
              { key: 'count',  label: 'Stats (count)',       val: showCount,  set: setShowCount },
            ].map(o => h('label', { key: o.key, style: { display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer', fontSize: 'var(--fxs)' } },
              h('input', { type: 'checkbox', checked: o.val, onChange: () => o.set(v => !v) }),
              o.label
            ))
          )
        ),

        // Custom note
        h('div', null,
          h('div', { style: { fontSize: 'var(--fxs)', fontWeight: 600, marginBottom: 6, color: 'var(--t1)' } }, 'Add a note (optional)'),
          h('input', {
            className: 'inp',
            style: { fontSize: 'var(--fxs)' },
            placeholder: 'e.g. All my trading bots run through A1',
            value: customNote,
            onChange: e => setCustomNote(e.target.value),
            maxLength: 80,
          })
        ),

        // Download button
        h('button', {
          className: 'btn btn-p',
          style: { width: '100%', justifyContent: 'center', marginTop: 4 },
          onClick: download,
          disabled: selected.length === 0,
        }, '⬇ Download card')
      ),

      // Canvas preview
      h('div', { style: { padding: 16, background: 'var(--b1)', display: 'flex', flexDirection: 'column', gap: 8 } },
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t3)', marginBottom: 4 } }, 'Preview'),
        h('canvas', {
          ref: canvasRef,
          style: { width: '100%', borderRadius: 6, border: '1px solid var(--b3)' },
        })
      )
    )
  );
}
