function RevokeHistory() {
  const [history, setHistory] = React.useState([]);
  const [cleared, setCleared] = React.useState(false);

  React.useEffect(() => {
    try {
      setHistory(JSON.parse(localStorage.getItem('a1_revoke_history') || '[]'));
    } catch (_) {}
  }, []);

  function clear() {
    localStorage.removeItem('a1_revoke_history');
    setHistory([]); setCleared(true);
  }

  if (history.length === 0) {
    return h('div', { style: { padding: '24px 0', color: 'var(--t2)', textAlign: 'center', fontSize: 'var(--fsm)' } },
      cleared ? '✅ Revoke history cleared.' : 'No revocations yet. Revoked passports will appear here.'
    );
  }

  return h('div', null,
    h('div', { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 } },
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', margin: 0 } }, history.length + ' revocation' + (history.length > 1 ? 's' : '') + ' recorded locally.'),
      h('button', { className: 'btn btn-s btn-sm', onClick: clear }, 'Clear history')
    ),
    history.map((item, i) => h('div', {
      key: i,
      style: { background: 'var(--b2)', border: '1px solid var(--b3)', borderRadius: 'var(--r)', padding: '10px 14px', marginBottom: 8 }
    },
      h('div', { style: { display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: 4 } },
        h('strong', { style: { fontSize: 'var(--fsm)', color: 'var(--t1)' } }, item.namespace || 'Unknown'),
        h('span', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)' } }, new Date(item.revokedAt).toLocaleString())
      ),
      item.fingerprint && h('div', { style: { fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--t2)', marginTop: 4, wordBreak: 'break-all' } },
        'fp: ' + item.fingerprint
      )
    ))
  );
}

function PassportsHub() {
  const [sub, setSub] = React.useState('dashboard');
  // Key bumped on cross-tab change to force PassportDashboard to re-mount and reload
  const [refreshKey, setRefreshKey] = React.useState(0);

  // Listen for sub-tab navigation events (e.g. "Open Passport Vault →" buttons)
  React.useEffect(() => {
    function onVaultSub(e) { if (e.detail) setSub(e.detail); }
    window.addEventListener('a1-vault-sub', onVaultSub);
    return () => window.removeEventListener('a1-vault-sub', onVaultSub);
  }, []);

  // Listen for cross-tab passport state changes (revoke, issue, renew)
  // so the dashboard stays live even when changes happen on other tabs.
  React.useEffect(() => {
    function onPassportChange() { setRefreshKey(k => k + 1); }
    window.addEventListener('a1-passport-changed', onPassportChange);
    return () => window.removeEventListener('a1-passport-changed', onPassportChange);
  }, []);

  return h('div', null,
    h('div', { style: { display: 'flex', gap: 8, marginBottom: 16, flexWrap: 'wrap' } },
      h('button', {
        className: 'btn ' + (sub === 'dashboard' ? 'btn-p' : 'btn-s') + ' btn-sm',
        onClick: () => setSub('dashboard')
      }, '🗂 All Passports'),
      h('button', {
        className: 'btn ' + (sub === 'vault' ? 'btn-p' : 'btn-s') + ' btn-sm',
        onClick: () => setSub('vault')
      }, '🗄️ Vault & Backup'),
      h('button', {
        className: 'btn ' + (sub === 'tools' ? 'btn-p' : 'btn-s') + ' btn-sm',
        onClick: () => setSub('tools')
      }, '🔧 Agent Tools'),
      h('button', {
        className: 'btn ' + (sub === 'history' ? 'btn-p' : 'btn-s') + ' btn-sm',
        onClick: () => setSub('history')
      }, '🗑 Revoke History')
    ),
    sub === 'dashboard' && h(PassportDashboard, { key: refreshKey }),
    sub === 'vault'     && h(PassportVault, null),
    sub === 'tools'     && h(MyPassports, null),
    sub === 'history'   && h(RevokeHistory, null)
  );
}
