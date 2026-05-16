// ─────────────────────────────────────────────────────────────────────────────
// PASSPORT VAULT — central passport management
// ─────────────────────────────────────────────────────────────────────────────

const RENEW_OPTS = [
  { v: '7d',  l: '7 days' },
  { v: '30d', l: '30 days' },
  { v: '90d', l: '3 months' },
  { v: '1y',  l: '1 year' },
];

function timeUntil(unixSec) {
  const diff = unixSec - Math.floor(Date.now() / 1000);
  if (diff <= 0) return { label: 'Expired', urgent: true };
  if (diff < 86400)     return { label: Math.ceil(diff / 3600) + 'h left', urgent: true };
  if (diff < 86400 * 7) return { label: Math.ceil(diff / 86400) + ' days left', urgent: true };
  return { label: Math.ceil(diff / 86400) + ' days left', urgent: false };
}

function PassportCard({ pp, gwUrl, onRefresh }) {
  const [renewing, setRenewing]       = useState(false);
  const [renewTtl, setRenewTtl]       = useState('30d');
  const [renewResult, setRenewResult] = useState(null);
  const [revokeMode, setRevokeMode]   = useState(false);
  const [revoking, setRevoking]       = useState(false);
  const [revokeResult, setRevokeResult] = useState(null);
  const [copied, setCopied]           = useState(false);
  const [expanded, setExpanded]       = useState(false);

  const exp    = pp.expiration_unix;
  const timing = exp ? timeUntil(exp) : null;
  const status = pp.status === 'expired' ? 'expired' : (timing?.urgent ? 'expiring' : 'valid');

  const STATUS_COLOR = { valid: 'var(--green)', expiring: '#ca8a04', expired: '#ef4444' };
  const STATUS_DOT   = { valid: '🟢', expiring: '🟡', expired: '🔴' };

  async function renew() {
    setRenewing(true); setRenewResult(null);
    const r = await fetch(gwUrl + '/v1/passports/renew', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path: pp.path, ttl: renewTtl }),
    }).then(r => r.json()).catch(e => ({ success: false, error: e.message }));
    setRenewResult(r);
    setRenewing(false);
    if (r.success) setTimeout(onRefresh, 800);
  }

  async function revoke() {
    setRevoking(true); setRevokeResult(null);
    const r = await fetch(gwUrl + '/v1/passports/revoke-by-namespace', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ namespace: pp.namespace, passport_path: pp.path }),
    }).then(r => r.json()).catch(e => ({ success: false, error: e.message }));
    setRevokeResult(r);
    setRevoking(false);
    if (r.success) {
      // Record in local revoke history
      try {
        const history = JSON.parse(localStorage.getItem('a1_revoke_history') || '[]');
        history.unshift({ namespace: pp.namespace, fingerprint: r.fingerprint_hex || '', revokedAt: new Date().toISOString(), path: pp.path || '' });
        localStorage.setItem('a1_revoke_history', JSON.stringify(history.slice(0, 50)));
      } catch (_) {}
      // Notify all tabs that passport state changed
      window.dispatchEvent(new CustomEvent('a1-passport-changed'));
      setTimeout(onRefresh, 800);
    }
  }

  function copyPath() {
    navigator.clipboard.writeText(pp.path);
    setCopied(true); setTimeout(() => setCopied(false), 1500);
  }

  const dotColor = STATUS_COLOR[status];

  return h('div', { className: 'pp-card' + (status === 'expired' ? ' expired' : ''), style: { marginBottom: 10, border: '1px solid var(--b3)', borderRadius: 'var(--r)', overflow: 'hidden' } },

    // Header row — always visible
    h('div', { style: { display: 'flex', alignItems: 'center', gap: 10, padding: '10px 14px', cursor: 'pointer', background: 'var(--b1)' }, onClick: () => setExpanded(e => !e) },
      h('div', { style: { fontSize: 20 } }, STATUS_DOT[status]),
      h('div', { style: { flex: 1, minWidth: 0 } },
        h('div', { style: { fontWeight: 700, fontSize: 'var(--fsm)', color: 'var(--t1)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' } },
          pp.namespace || pp.filename),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 2 } },
          pp.capabilities.slice(0, 4).join(' · ') + (pp.capabilities.length > 4 ? ' +' + (pp.capabilities.length - 4) + ' more' : ''))
      ),
      h('div', { style: { textAlign: 'right', flexShrink: 0 } },
        timing && h('div', { style: { fontSize: 'var(--fxs)', color: dotColor, fontWeight: 600 } }, timing.label),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t3)', marginTop: 2 } }, expanded ? '▲ less' : '▼ more')
      )
    ),

    // Expanded body
    expanded && h('div', { style: { padding: '12px 14px', borderTop: '1px solid var(--b3)', display: 'flex', flexDirection: 'column', gap: 12 } },

      // File path
      h('div', null,
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginBottom: 4 } }, 'Passport file location'),
        h('div', { style: { display: 'flex', gap: 6, alignItems: 'center' } },
          h('code', { style: { fontFamily: 'var(--mono)', fontSize: 'var(--fxs)', color: 'var(--t1)', background: 'var(--b2)', padding: '3px 7px', borderRadius: 4, flex: 1, wordBreak: 'break-all' } }, pp.path),
          h('button', { className: 'btn btn-s btn-sm', onClick: copyPath, style: { flexShrink: 0 } }, copied ? '✓ Copied' : 'Copy path')
        )
      ),

      // All capabilities
      pp.capabilities.length > 0 && h('div', null,
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginBottom: 6 } }, 'Allowed capabilities'),
        h('div', { style: { display: 'flex', flexWrap: 'wrap', gap: 5 } },
          pp.capabilities.map(c => h('span', { key: c, style: { fontFamily: 'var(--mono)', fontSize: 'var(--fxs)', background: 'rgba(99,102,241,.12)', color: 'var(--accent)', padding: '2px 8px', borderRadius: 20, border: '1px solid rgba(99,102,241,.2)' } }, c))
        )
      ),

      // Actions row
      h('div', { style: { display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' } },

        // Renew
        !revokeMode && h('div', { style: { display: 'flex', gap: 5, alignItems: 'center' } },
          h('select', {
            value: renewTtl, onChange: e => setRenewTtl(e.target.value),
            style: { fontSize: 'var(--fxs)', padding: '5px 8px', border: '1px solid var(--b3)', borderRadius: 'var(--r)', background: 'var(--b1)', color: 'var(--t1)', cursor: 'pointer' }
          }, RENEW_OPTS.map(o => h('option', { key: o.v, value: o.v }, o.l))),
          h('button', { className: 'btn btn-p btn-sm', onClick: renew, disabled: renewing },
            renewing ? 'Renewing…' : '↺ Renew passport')
        ),

        // Revoke trigger (not shown when RevokeConfirm is open)
        !revokeMode && h('button', {
          className: 'btn btn-sm',
          onClick: () => setRevokeMode(true),
          style: { background: 'rgba(239,68,68,.07)', color: '#ef4444', border: '1px solid rgba(239,68,68,.2)', borderRadius: 'var(--r)', padding: '5px 12px', cursor: 'pointer', fontWeight: 600, fontSize: 'var(--fxs)' }
        }, 'Revoke…')
      ),

      // Inline revoke confirmation
      revokeMode && h(RevokeConfirm, {
        agentName: pp.namespace || pp.filename,
        revoking,
        onConfirm: revoke,
        onCancel: () => { setRevokeMode(false); setRevokeResult(null); },
      }),

      // Result feedback
      renewResult && h('div', { className: renewResult.success ? 'ag-result ok' : 'ag-result err' },
        renewResult.success
          ? h('div', null,
              h('div', { style: { fontWeight: 600, marginBottom: 3 } }, '✅ Passport renewed'),
              h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)' } }, 'New expiry: ' + renewResult.expires_at + '. Restart your agent to pick up the new file.'))
          : h('div', null,
              h('div', { style: { fontWeight: 600, color: '#ef4444', marginBottom: 3 } }, 'Renewal failed'),
              h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)' } }, renewResult.error))
      ),

      revokeResult && h('div', { className: revokeResult.success ? 'ag-result ok' : 'ag-result err' },
        revokeResult.success
          ? h('div', { style: { fontWeight: 600 } }, '✅ Access revoked. Agent is blocked immediately.')
          : h('div', null,
              h('div', { style: { fontWeight: 600, color: '#ef4444', marginBottom: 3 } }, 'Revoke failed'),
              h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)' } }, revokeResult.error))
      )
    )
  );
}

function PassportVault() {
  const { settings } = useContext(Ctx);
  const gwUrl = settings.gwUrl || 'http://localhost:8080';

  const [passports, setPassports]   = useState(null);
  const [directory, setDirectory]   = useState('');
  const [loading, setLoading]       = useState(false);
  const [loadErr, setLoadErr]       = useState(null);
  const [filter, setFilter]         = useState('all');

  async function load() {
    setLoading(true); setLoadErr(null);
    const r = await fetch(gwUrl + '/v1/passports/list')
      .then(r => r.json())
      .catch(e => ({ error: e.message }));
    if (r.error) {
      setLoadErr(r.error); setPassports(null);
    } else {
      setPassports(r.passports || []);
      setDirectory(r.directory || '');
    }
    setLoading(false);
  }

  useEffect(() => { load(); }, []);

  const filtered = !passports ? [] : passports.filter(p => {
    if (filter === 'expired') return p.status === 'expired';
    if (filter === 'valid')   return p.status === 'valid';
    return true;
  });

  const expiredCount = passports ? passports.filter(p => p.status === 'expired').length : 0;

  return h('div', { style: { paddingBottom: 40, width: '100%' } },
    h('h2', { style: { fontSize: 18, fontWeight: 700, marginBottom: 4 } }, '🗄️ Passport Vault'),
    h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', marginBottom: 10, lineHeight: 1.6 } },
      'All your agent passports in one place. Renew expiring ones or revoke access — no fingerprints needed.'),

    h(NudgeTip, { tipKey: 'renew_early' }),

    h(PassportBackup, { gwUrl }),

    // Expired warning
    expiredCount > 0 && h('div', { style: { background: 'rgba(239,68,68,.07)', border: '1px solid rgba(239,68,68,.25)', borderRadius: 'var(--r)', padding: '10px 14px', marginBottom: 12, display: 'flex', gap: 10, alignItems: 'center' } },
      h('span', { style: { fontSize: 20 } }, '🔴'),
      h('div', null,
        h('div', { style: { fontWeight: 700, color: '#ef4444', fontSize: 'var(--fsm)' } }, expiredCount + ' passport' + (expiredCount > 1 ? 's' : '') + ' expired'),
        h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', marginTop: 2 } }, 'Agents using expired passports will fail to authorize. Expand the card below and click Renew.')
      )
    ),

    // Toolbar
    h('div', { style: { display: 'flex', gap: 8, marginBottom: 12, alignItems: 'center', flexWrap: 'wrap' } },
      h('div', { style: { display: 'flex', gap: 4 } },
        ['all', 'valid', 'expired'].map(f =>
          h('button', { key: f, onClick: () => setFilter(f),
            style: { padding: '4px 12px', fontSize: 'var(--fxs)', fontWeight: filter === f ? 700 : 400, borderRadius: 20, border: '1px solid var(--b3)', background: filter === f ? 'var(--accent)' : 'var(--b1)', color: filter === f ? '#fff' : 'var(--t2)', cursor: 'pointer' }
          }, f === 'all' ? 'All' : f === 'valid' ? '🟢 Valid' : '🔴 Expired')
        )
      ),
      h('div', { style: { flex: 1 } }),
      h('button', { className: 'btn btn-s btn-sm', onClick: load, disabled: loading }, loading ? 'Refreshing…' : '↻ Refresh'),
      h('button', { className: 'btn btn-p btn-sm', onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'wizard' })) }, '+ New passport')
    ),

    // Directory hint
    directory && h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t3)', marginBottom: 10 } },
      '📁 Passport folder: ', h('code', { style: { fontFamily: 'var(--mono)', color: 'var(--t2)' } }, directory)
    ),

    // Error state
    loadErr && h('div', { className: 'wiz-info', style: { borderColor: 'rgba(239,68,68,.3)', background: 'rgba(239,68,68,.04)', marginBottom: 12 } },
      h('span', { style: { fontSize: 18 } }, '❌'),
      h('div', null,
        h('div', { style: { fontWeight: 600, color: '#ef4444', marginBottom: 3 } }, 'Could not load passports'),
        h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)' } }, loadErr),
        h('div', { style: { marginTop: 6, fontSize: 'var(--fxs)' } },
          'Make sure A1 is running. Go to ',
          h('span', { style: { color: 'var(--accent)', cursor: 'pointer', fontWeight: 600 },
            onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'lifecycle' })) }, 'Start / Stop'),
          ' to start it.')
      )
    ),

    // Loading
    loading && !passports && h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', textAlign: 'center', padding: 24 } }, 'Loading passports…'),

    // Empty state
    !loading && passports && filtered.length === 0 && h('div', { className: 'wiz-info', style: { textAlign: 'center' } },
      h('span', { style: { fontSize: 18 } }, filter === 'expired' ? '🟢' : '📭'),
      h('div', null,
        filter === 'expired'
          ? h('div', { style: { fontWeight: 600 } }, 'No expired passports')
          : h('div', null,
              h('div', { style: { fontWeight: 600, marginBottom: 4 } }, 'No passports found'),
              h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)' } }, 'Create your first passport using "Protect My Agent".'),
              h('button', { className: 'btn btn-p btn-sm', style: { marginTop: 8 },
                onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'wizard' })) }, '→ Protect My Agent')
            )
      )
    ),

    // Passport cards
    filtered.map(pp => h(PassportCard, { key: pp.path, pp, gwUrl, onRefresh: load })),

    // Footer guidance
    passports && passports.length > 0 && h('div', { className: 'wiz-info', style: { marginTop: 16 } },
      h('span', { style: { fontSize: 18 } }, '💡'),
      h('div', null,
        h('div', { style: { fontWeight: 600, marginBottom: 3 } }, 'Where is my passport file?'),
        h('div', { style: { color: 'var(--t2)', lineHeight: 1.6, fontSize: 'var(--fxs)' } },
          'Passport files live in your home folder under ', h('code', { style: { fontFamily: 'var(--mono)' } }, '~/.a1/passports/'), '. ',
          'When you run "Protect My Agent", a new file is saved there automatically. ',
          'Point your agent\'s PassportClient to that path.')
      )
    )
  );
}
