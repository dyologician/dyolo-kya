// ─────────────────────────────────────────────────────────────────────────────
// MY PASSPORTS — agent list + low-level passport inspect + revoke by fingerprint
// ─────────────────────────────────────────────────────────────────────────────
function MyPassports() {
  const { api, settings } = useContext(Ctx);
  const [passportPath, setPassportPath] = useState('./passport.json');
  const [inspecting, setInspecting]     = useState(false);
  const [inspectResult, setInspectResult] = useState(null);
  const [agents, setAgents]             = useState(null);
  const [scanning, setScanning]         = useState(false);
  const [revoking, setRevoking]         = useState(false);
  const [revokeFingerprint, setRevokeFingerprint] = useState('');
  const [revokeResult, setRevokeResult] = useState(null);

  useEffect(() => {
    setScanning(true);
    api('GET', '/v1/agents/scan').then(r => {
      setAgents(r.ok ? (r.data.agents || []).filter(a => a.connected) : []);
      setScanning(false);
    });
  }, []);

  async function inspectPassport() {
    if (!passportPath.trim()) return;
    setInspecting(true); setInspectResult(null);
    const resp = await fetch((settings.gwUrl || 'http://localhost:8080') + '/mcp', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0', id: 1,
        method: 'tools/call',
        params: { name: 'a1_inspect_passport', arguments: { passport_path: passportPath.trim() } },
      }),
    }).then(r => r.json()).catch(e => ({ error: e.message }));
    setInspectResult(resp); setInspecting(false);
  }

  async function revokeByFingerprint() {
    if (!revokeFingerprint.trim()) return;
    setRevoking(true); setRevokeResult(null);
    const resp = await fetch((settings.gwUrl || 'http://localhost:8080') + '/mcp', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0', id: 2,
        method: 'tools/call',
        params: { name: 'a1_revoke', arguments: { fingerprint: revokeFingerprint.trim() } },
      }),
    }).then(r => r.json()).catch(e => ({ error: e.message }));
    setRevokeResult(resp); setRevoking(false);
  }

  const inspectText = inspectResult?.result?.content?.map(c => c.text || '').join('\n') || null;
  const inspectErr  = inspectResult?.error;
  const revokeText  = revokeResult?.result?.content?.map(c => c.text || '').join('\n') || null;
  const revokeErr   = revokeResult?.error;

  return h('div', { style: { paddingBottom: 40, width: '100%' } },
    h('h2', { style: { fontSize: 18, fontWeight: 700, marginBottom: 4 } }, '🗂️ Passports (Raw Tools)'),
    h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', marginBottom: 8, lineHeight: 1.6 } },
      'Low-level tools for inspecting and revoking passports by file path or fingerprint.'),

    // Vault shortcut banner
    h('div', { className: 'wiz-info gr', style: { marginBottom: 16 } },
      h('span', { style: { fontSize: 18 } }, '🗄️'),
      h('div', null,
        h('div', { style: { fontWeight: 600, marginBottom: 3 } }, 'Looking for the easy way?'),
        h('div', { style: { color: 'var(--t2)', lineHeight: 1.5, fontSize: 'var(--fxs)' } },
          'The Passport Vault shows all passports with one-click Renew and Revoke buttons — no fingerprints needed.'),
        h('button', { className: 'btn btn-p btn-sm', style: { marginTop: 8 },
          onClick: () => window.dispatchEvent(new CustomEvent('a1-vault-sub', { detail: 'vault' })) },
          'Open Passport Vault →')
      )
    ),

    // Connected agents live scan
    h('div', { className: 'sg' },
      h('div', { className: 'sg-head' }, 'Connected agents' + (agents !== null ? ' (' + agents.length + ')' : '')),
      h('div', { className: 'sg-body' },
        scanning && h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)' } }, 'Scanning…'),
        !scanning && agents !== null && agents.length === 0 &&
          h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)' } }, 'No agents connected yet. Go to "Connect Agents" to connect one.'),
        !scanning && agents && agents.map(ag => h('div', { key: ag.id, className: 'pp-card active', style: { display: 'flex', gap: 12, alignItems: 'center' } },
          h('div', { style: { fontSize: 22 } }, ag.icon),
          h('div', { style: { flex: 1 } },
            h('div', { style: { fontWeight: 600, fontSize: 'var(--fsm)' } }, '🟢 ' + ag.name),
            h('div', { className: 'pp-label' }, ag.install_path || ''),
            h('div', { className: 'pp-label', style: { marginTop: 3 } }, ag.config_file || ''))
        ))
      )
    ),

    // Passport inspector
    h('div', { className: 'sg', style: { marginTop: 12 } },
      h('div', { className: 'sg-head' }, 'Inspect a passport file by path'),
      h('div', { className: 'sg-body' },
        h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', marginBottom: 8 } },
          'Enter the full path to any passport.json. A1 reads it and shows namespace, capabilities, and expiry.'),
        h('div', { style: { display: 'flex', gap: 8, marginBottom: 8 } },
          h('input', {
            className: 'inp inp-mono', style: { flex: 1 },
            placeholder: '/home/user/.a1/passports/trading-bot.json',
            value: passportPath,
            onChange: e => setPassportPath(e.target.value),
            onKeyDown: e => e.key === 'Enter' && inspectPassport(),
          }),
          h('button', { className: 'btn btn-p btn-sm', onClick: inspectPassport, disabled: inspecting || !passportPath.trim() },
            inspecting ? 'Reading…' : 'Inspect')
        ),
        inspectText && h('div', { className: 'ag-result ok' },
          h('pre', { style: { margin: 0, fontFamily: 'var(--mono)', fontSize: 'var(--fxs)', lineHeight: 1.8, whiteSpace: 'pre-wrap' } }, inspectText)),
        inspectErr && h('div', { className: 'ag-result err' },
          h('div', { style: { fontWeight: 600, marginBottom: 3 } }, 'Error'),
          h('div', { style: { fontSize: 'var(--fsm)', color: 'var(--t2)' } }, typeof inspectErr === 'object' ? inspectErr.message : String(inspectErr)),
          h('div', { style: { marginTop: 6, fontSize: 'var(--fxs)', color: 'var(--t2)' } }, 'Check the file path and make sure the passport.json exists there.')
        )
      )
    ),

    // Renew hint
    h('div', { className: 'wiz-info', style: { marginTop: 12 } },
      h('span', { style: { fontSize: 18 } }, '↺'),
      h('div', null,
        h('div', { style: { fontWeight: 600, marginBottom: 3 } }, 'To renew an expired passport'),
        h('div', { style: { color: 'var(--t2)', lineHeight: 1.6, fontSize: 'var(--fxs)' } },
          'Open the Passport Vault and click Renew next to the expired agent. ',
          'Or go to "Protect My Agent" and re-issue with the same name and capabilities.')
      )
    ),

    // Revoke by fingerprint
    h('div', { className: 'sg', style: { marginTop: 12 } },
      h('div', { className: 'sg-head' }, 'Revoke a certificate by fingerprint'),
      h('div', { className: 'sg-body' },
        h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', marginBottom: 8 } },
          'Revoke immediately using the 64-character hex fingerprint from the Live Log or a passport inspect. ',
          'For easier revoke-by-agent-name, use the Passport Vault.'),
        h('div', { style: { display: 'flex', gap: 8, marginBottom: 8 } },
          h('input', {
            className: 'inp inp-mono', style: { flex: 1 },
            placeholder: '64-character hex fingerprint',
            value: revokeFingerprint,
            onChange: e => setRevokeFingerprint(e.target.value),
          }),
          h('button', {
            className: 'btn btn-sm',
            style: { background: 'rgba(239,68,68,.1)', color: '#ef4444', border: '1px solid rgba(239,68,68,.3)', borderRadius: 'var(--r)', padding: '6px 12px', cursor: 'pointer', fontWeight: 600, fontSize: 'var(--fxs)', whiteSpace: 'nowrap' },
            disabled: revoking || !revokeFingerprint.trim(),
            onClick: revokeByFingerprint,
          }, revoking ? 'Revoking…' : 'Revoke')
        ),
        revokeText && h('div', { className: 'ag-result ok' }, h('pre', { style: { margin: 0, fontFamily: 'var(--mono)', fontSize: 'var(--fxs)', lineHeight: 1.8, whiteSpace: 'pre-wrap' } }, revokeText)),
        revokeErr  && h('div', { className: 'ag-result err' },
          h('div', { style: { fontWeight: 600, marginBottom: 3 } }, 'Error'),
          h('div', { style: { fontSize: 'var(--fsm)', color: 'var(--t2)' } }, typeof revokeErr === 'object' ? revokeErr.message : String(revokeErr))
        )
      )
    )
  );
}
