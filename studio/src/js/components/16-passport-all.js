// ─────────────────────────────────────────────────────────────────────────────
// PASSPORT DASHBOARD — unified view of every passport across all namespaces
// ─────────────────────────────────────────────────────────────────────────────

const SORT_MODES = [
  { id: 'urgency',   label: 'Most urgent first' },
  { id: 'expiry',    label: 'Expiry date' },
  { id: 'name',      label: 'Name A → Z' },
];

function urgencyScore(pp) {
  if (pp.status === 'expired') return 0;
  const diff = (pp.expiration_unix || Infinity) - Math.floor(Date.now() / 1000);
  if (diff < 86400)     return 1;
  if (diff < 86400 * 7) return 2;
  return 3;
}

function ppSortKey(pp, mode) {
  if (mode === 'urgency') return urgencyScore(pp) * 1e12 + (pp.expiration_unix || 1e15);
  if (mode === 'expiry')  return pp.expiration_unix || 1e15;
  return (pp.namespace || pp.filename || '').toLowerCase();
}

function timeRemaining(unixSec) {
  const diff = unixSec - Math.floor(Date.now() / 1000);
  if (diff <= 0)            return { label: 'Expired', level: 'expired' };
  if (diff < 3600)          return { label: Math.ceil(diff / 60) + ' min', level: 'critical' };
  if (diff < 86400)         return { label: Math.ceil(diff / 3600) + 'h', level: 'critical' };
  if (diff < 86400 * 7)    return { label: Math.ceil(diff / 86400) + ' days', level: 'warn' };
  if (diff < 86400 * 30)   return { label: Math.ceil(diff / 86400) + ' days', level: 'ok' };
  return { label: Math.ceil(diff / 86400) + ' days', level: 'ok' };
}

const LEVEL_COLOR = {
  expired:  '#ef4444',
  critical: '#ef4444',
  warn:     '#ca8a04',
  ok:       'var(--green)',
};

const LEVEL_BG = {
  expired:  'rgba(239,68,68,.07)',
  critical: 'rgba(239,68,68,.07)',
  warn:     'rgba(202,138,4,.07)',
  ok:       'rgba(34,197,94,.06)',
};

const RENEW_TTL_OPTS = [
  { v: '7d', l: '7 days' },
  { v: '30d', l: '30 days' },
  { v: '90d', l: '3 months' },
  { v: '1y',  l: '1 year' },
];

// ── Single-row passport entry in the dashboard ─────────────────────────────

function DashRow({ pp, gwUrl, onRefresh }) {
  const [expanded,   setExpanded]   = useState(false);
  const [renewTtl,   setRenewTtl]   = useState('30d');
  const [renewing,   setRenewing]   = useState(false);
  const [renewMsg,   setRenewMsg]   = useState(null);
  const [revoking,   setRevoking]   = useState(false);
  const [revokeStep, setRevokeStep] = useState(0);
  const [revokeMsg,  setRevokeMsg]  = useState(null);
  const [copied,     setCopied]     = useState(false);

  const timing = pp.expiration_unix ? timeRemaining(pp.expiration_unix) : null;
  const level  = timing ? timing.level : 'ok';
  const name   = pp.namespace || pp.filename || 'unnamed';

  async function renew() {
    setRenewing(true); setRenewMsg(null);
    const r = await fetch(gwUrl + '/v1/passports/renew', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ path: pp.path, ttl: renewTtl }),
    }).then(r => r.json()).catch(e => ({ success: false, error: e.message }));
    setRenewMsg(r);
    setRenewing(false);
    if (r.success) { setTimeout(onRefresh, 700); setExpanded(false); }
  }

  async function execRevoke() {
    setRevoking(true); setRevokeMsg(null);
    const r = await fetch(gwUrl + '/v1/passports/revoke-by-namespace', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ namespace: pp.namespace, passport_path: pp.path }),
    }).then(r => r.json()).catch(e => ({ success: false, error: e.message }));
    setRevokeMsg(r);
    setRevoking(false);
    if (r.success) {
      // Store revoke event in localStorage for history view
      try {
        const history = JSON.parse(localStorage.getItem('a1_revoke_history') || '[]');
        history.unshift({
          namespace: pp.namespace,
          fingerprint: r.fingerprint_hex || '',
          revokedAt: new Date().toISOString(),
          path: pp.path || '',
        });
        localStorage.setItem('a1_revoke_history', JSON.stringify(history.slice(0, 50)));
      } catch (_) {}
      // Notify Connect Agents, and any other tab listening, that passport state changed
      window.dispatchEvent(new CustomEvent('a1-passport-changed'));
      setTimeout(onRefresh, 700); setExpanded(false);
    }
  }

  function copyPath() {
    navigator.clipboard.writeText(pp.path);
    setCopied(true); setTimeout(() => setCopied(false), 1400);
  }

  const statusIcon = level === 'expired' ? '🔴' : level === 'critical' || level === 'warn' ? '🟡' : '🟢';

  return h('div', {
    className: 'dash-row',
    style: {
      border: '1px solid var(--b3)',
      borderLeft: '3px solid ' + LEVEL_COLOR[level],
      borderRadius: 'var(--r)',
      background: 'var(--b1)',
      marginBottom: 7,
      overflow: 'hidden',
    }
  },

    // ── Summary bar (always visible) ─────────────────────────────────────────
    h('div', {
      style: {
        display: 'flex', alignItems: 'center', gap: 10,
        padding: '9px 14px', cursor: 'pointer',
      },
      onClick: () => setExpanded(e => !e),
    },
      h('span', { style: { fontSize: 16, flexShrink: 0 } }, statusIcon),

      h('div', { style: { flex: 1, minWidth: 0 } },
        h('div', {
          style: {
            fontWeight: 700, fontSize: 'var(--fsm)', color: 'var(--t1)',
            whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis',
          }
        }, name),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 1 } },
          pp.capabilities.slice(0, 5).join(' · ') +
          (pp.capabilities.length > 5 ? ' +' + (pp.capabilities.length - 5) : ''))
      ),

      // Timing badge
      timing && h('div', {
        style: {
          padding: '2px 9px', borderRadius: 12, fontSize: 'var(--fxs)', fontWeight: 700,
          background: LEVEL_BG[level], color: LEVEL_COLOR[level],
          border: '1px solid ' + LEVEL_COLOR[level] + '44',
          flexShrink: 0, whiteSpace: 'nowrap',
        }
      }, timing.label),

      // Quick-renew button (visible without expanding)
      !expanded && level !== 'ok' && h('button', {
        className: 'btn btn-p btn-sm',
        style: { flexShrink: 0, marginLeft: 4, fontSize: 'var(--fxs)' },
        onClick: e => { e.stopPropagation(); setExpanded(true); },
      }, 'Renew'),

      h('span', { style: { color: 'var(--t3)', fontSize: 'var(--fxs)', marginLeft: 6, flexShrink: 0 } },
        expanded ? '▲' : '▼')
    ),

    // ── Expanded panel ────────────────────────────────────────────────────────
    expanded && h('div', {
      style: {
        padding: '12px 14px',
        borderTop: '1px solid var(--b3)',
        display: 'flex', flexDirection: 'column', gap: 12,
      }
    },

      // File path
      h('div', null,
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginBottom: 4 } }, 'Passport file'),
        h('div', { style: { display: 'flex', gap: 6, alignItems: 'center' } },
          h('code', {
            style: {
              fontFamily: 'var(--mono)', fontSize: 'var(--fxs)', color: 'var(--t1)',
              background: 'var(--b2)', padding: '3px 8px', borderRadius: 4,
              flex: 1, wordBreak: 'break-all',
            }
          }, pp.path),
          h('button', { className: 'btn btn-s btn-sm', onClick: copyPath, style: { flexShrink: 0 } },
            copied ? '✓ Copied' : 'Copy')
        )
      ),

      // All capabilities
      pp.capabilities.length > 0 && h('div', null,
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginBottom: 5 } }, 'Capabilities'),
        h('div', { style: { display: 'flex', flexWrap: 'wrap', gap: 4 } },
          pp.capabilities.map(c => h('span', {
            key: c,
            style: {
              fontFamily: 'var(--mono)', fontSize: 'var(--fxs)',
              background: 'rgba(99,102,241,.1)', color: 'var(--accent)',
              padding: '2px 8px', borderRadius: 20,
              border: '1px solid rgba(99,102,241,.18)',
            }
          }, c))
        )
      ),

      // Renew section
      level !== 'ok' && h('div', {
        style: {
          padding: '10px 12px', borderRadius: 'var(--r)',
          background: LEVEL_BG[level], border: '1px solid ' + LEVEL_COLOR[level] + '33',
        }
      },
        h('div', {
          style: { fontWeight: 600, fontSize: 'var(--fsm)', color: LEVEL_COLOR[level], marginBottom: 8 }
        }, level === 'expired' ? 'This passport has expired — agent is blocked' : 'Renew before it expires'),
        h('div', { style: { display: 'flex', gap: 6, alignItems: 'center', flexWrap: 'wrap' } },
          h('select', {
            value: renewTtl, onChange: e => setRenewTtl(e.target.value),
            style: {
              fontSize: 'var(--fxs)', padding: '5px 8px',
              border: '1px solid var(--b3)', borderRadius: 'var(--r)',
              background: 'var(--b1)', color: 'var(--t1)', cursor: 'pointer',
            }
          }, RENEW_TTL_OPTS.map(o => h('option', { key: o.v, value: o.v }, o.l))),
          h('button', {
            className: 'btn btn-p btn-sm', onClick: renew, disabled: renewing,
          }, renewing ? 'Renewing…' : '↺ Renew passport'),
        ),
        renewMsg && h('div', {
          style: {
            marginTop: 8, fontSize: 'var(--fxs)', fontWeight: 600,
            color: renewMsg.success ? 'var(--green)' : '#ef4444',
          }
        }, renewMsg.success
          ? '✅ Renewed. Restart your agent to pick up the new file.'
          : '❌ ' + (renewMsg.error || 'Renew failed.'))
      ),

      // Revoke section
      h('div', null,
        revokeStep === 0 && h('button', {
          className: 'btn btn-sm',
          onClick: () => setRevokeStep(1),
          style: {
            background: 'rgba(239,68,68,.07)', color: '#ef4444',
            border: '1px solid rgba(239,68,68,.2)', borderRadius: 'var(--r)',
            padding: '5px 14px', cursor: 'pointer', fontWeight: 600, fontSize: 'var(--fxs)',
          }
        }, 'Revoke access…'),

        revokeStep === 1 && h('div', {
          style: {
            padding: '10px 12px', border: '1px solid rgba(239,68,68,.25)',
            borderRadius: 'var(--r)', background: 'rgba(239,68,68,.05)',
          }
        },
          h('div', { style: { fontWeight: 700, color: '#ef4444', fontSize: 'var(--fsm)', marginBottom: 4 } },
            '⚠ Revoke "' + name + '"?'),
          h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', lineHeight: 1.7, marginBottom: 10 } },
            'The agent will be blocked immediately. ',
            h('strong', null, 'This is reversible'), ' — issue a new passport any time from Protect My Agent. The whole process takes about 60 seconds.'),
          h('div', { style: { display: 'flex', gap: 8, alignItems: 'center' } },
            h('button', {
              className: 'btn btn-sm',
              onClick: execRevoke, disabled: revoking,
              style: {
                background: '#ef4444', color: '#fff', border: 'none',
                borderRadius: 'var(--r)', padding: '5px 16px',
                cursor: revoking ? 'not-allowed' : 'pointer', fontWeight: 700,
                fontSize: 'var(--fxs)', opacity: revoking ? .6 : 1,
              }
            }, revoking ? 'Revoking…' : 'Revoke now'),
            h('button', {
              className: 'btn btn-s btn-sm',
              onClick: () => setRevokeStep(0), disabled: revoking,
            }, 'Cancel'),
            !revoking && h('span', { style: { fontSize: 'var(--fxs)', color: 'var(--t3)' } },
              '→ re-issue any time after')
          ),
          revokeMsg && !revokeMsg.success && h('div', {
            style: { marginTop: 8, fontSize: 'var(--fxs)', color: '#ef4444', fontWeight: 600 }
          }, '❌ ' + (revokeMsg.error || 'Revoke failed.'))
        )
      )
    )
  );
}

// ── Stats bar ─────────────────────────────────────────────────────────────────

function DashStats({ passports }) {
  const total    = passports.length;
  const expired  = passports.filter(p => p.status === 'expired').length;
  const expiring = passports.filter(p => {
    if (p.status === 'expired') return false;
    const diff = (p.expiration_unix || Infinity) - Math.floor(Date.now() / 1000);
    return diff < 86400 * 7;
  }).length;
  const valid = total - expired - expiring;

  const tiles = [
    { label: 'Total',    value: total,    color: 'var(--t1)',    bg: 'var(--b2)' },
    { label: 'Protected', value: valid,   color: 'var(--green)', bg: 'rgba(34,197,94,.08)' },
    { label: 'Expiring', value: expiring, color: '#ca8a04',      bg: 'rgba(202,138,4,.08)' },
    { label: 'Expired',  value: expired,  color: '#ef4444',      bg: 'rgba(239,68,68,.08)' },
  ];

  return h('div', {
    style: {
      display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)',
      gap: 8, marginBottom: 16,
    }
  },
    tiles.map(t => h('div', {
      key: t.label,
      style: {
        background: t.bg, border: '1px solid var(--b3)',
        borderRadius: 'var(--r)', padding: '10px 14px', textAlign: 'center',
      }
    },
      h('div', { style: { fontSize: 22, fontWeight: 800, color: t.color, fontFamily: 'var(--mono)', lineHeight: 1 } }, t.value),
      h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 4 } }, t.label)
    ))
  );
}

// ── Main component ────────────────────────────────────────────────────────────

function PassportDashboard() {
  const { settings } = useContext(Ctx);
  const gwUrl = settings.gwUrl || 'http://localhost:8080';

  const [passports, setPassports] = useState(null);
  const [loading,   setLoading]   = useState(false);
  const [loadErr,   setLoadErr]   = useState(null);
  const [search,    setSearch]    = useState('');
  const [sort,      setSort]      = useState('urgency');
  const [filterExp, setFilterExp] = useState('all');

  async function load() {
    setLoading(true); setLoadErr(null);
    const r = await fetch(gwUrl + '/v1/passports/list')
      .then(r => r.json())
      .catch(e => ({ error: e.message }));
    if (r.error) { setLoadErr(r.error); setPassports(null); }
    else         { setPassports(r.passports || []); }
    setLoading(false);
  }

  useEffect(() => { load(); }, [gwUrl]);

  const visible = useMemo(() => {
    if (!passports) return [];
    let list = passports;
    if (filterExp === 'expired')  list = list.filter(p => p.status === 'expired');
    if (filterExp === 'expiring') list = list.filter(p => {
      if (p.status === 'expired') return false;
      const diff = (p.expiration_unix || Infinity) - Math.floor(Date.now() / 1000);
      return diff < 86400 * 7;
    });
    if (filterExp === 'valid') list = list.filter(p => {
      if (p.status === 'expired') return false;
      const diff = (p.expiration_unix || Infinity) - Math.floor(Date.now() / 1000);
      return diff >= 86400 * 7;
    });
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter(p =>
        (p.namespace || '').toLowerCase().includes(q) ||
        (p.filename  || '').toLowerCase().includes(q) ||
        p.capabilities.some(c => c.toLowerCase().includes(q))
      );
    }
    return [...list].sort((a, b) => {
      const ka = ppSortKey(a, sort);
      const kb = ppSortKey(b, sort);
      return typeof ka === 'string' ? ka.localeCompare(kb) : ka - kb;
    });
  }, [passports, search, sort, filterExp]);

  const urgentCount = passports
    ? passports.filter(p => urgencyScore(p) <= 1).length
    : 0;

  return h('div', { style: { paddingBottom: 40, width: '100%' } },

    h('h2', { style: { fontSize: 18, fontWeight: 700, marginBottom: 4 } }, '🗂 All Passports'),
    h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', marginBottom: 16, lineHeight: 1.6 } },
      'Every passport across all your agents — sorted by urgency, searchable, one click to act.'),

    // Urgent banner
    urgentCount > 0 && h('div', {
      style: {
        display: 'flex', gap: 10, alignItems: 'center',
        padding: '10px 14px', borderRadius: 'var(--r)', marginBottom: 14,
        background: 'rgba(239,68,68,.07)', border: '1px solid rgba(239,68,68,.25)',
      }
    },
      h('span', { style: { fontSize: 20 } }, '🔴'),
      h('div', null,
        h('div', { style: { fontWeight: 700, color: '#ef4444', fontSize: 'var(--fsm)' } },
          urgentCount + ' passport' + (urgentCount > 1 ? 's' : '') + ' need attention'),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 2 } },
          'Expand a row below and click Renew. Takes less than 30 seconds.')
      )
    ),

    // Stats bar
    passports && passports.length > 0 && h(DashStats, { passports }),

    // Toolbar
    h('div', {
      style: { display: 'flex', gap: 8, marginBottom: 12, alignItems: 'center', flexWrap: 'wrap' }
    },
      h('input', {
        type: 'text', value: search, placeholder: 'Search by name or capability…',
        onChange: e => setSearch(e.target.value),
        style: {
          flex: 1, minWidth: 160, fontSize: 'var(--fxs)', padding: '6px 10px',
          border: '1px solid var(--b3)', borderRadius: 'var(--r)',
          background: 'var(--b2)', color: 'var(--t1)', outline: 'none',
        }
      }),

      h('select', {
        value: filterExp, onChange: e => setFilterExp(e.target.value),
        style: {
          fontSize: 'var(--fxs)', padding: '5px 8px',
          border: '1px solid var(--b3)', borderRadius: 'var(--r)',
          background: 'var(--b1)', color: 'var(--t1)', cursor: 'pointer',
        }
      },
        h('option', { value: 'all' },      'All'),
        h('option', { value: 'valid' },    '🟢 Valid'),
        h('option', { value: 'expiring' }, '🟡 Expiring'),
        h('option', { value: 'expired' },  '🔴 Expired')
      ),

      h('select', {
        value: sort, onChange: e => setSort(e.target.value),
        style: {
          fontSize: 'var(--fxs)', padding: '5px 8px',
          border: '1px solid var(--b3)', borderRadius: 'var(--r)',
          background: 'var(--b1)', color: 'var(--t1)', cursor: 'pointer',
        }
      }, SORT_MODES.map(m => h('option', { key: m.id, value: m.id }, m.label))),

      h('button', {
        className: 'btn btn-s btn-sm', onClick: load, disabled: loading,
      }, loading ? '…' : '↻'),

      h('button', {
        className: 'btn btn-p btn-sm',
        onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'wizard' })),
      }, '+ New')
    ),

    // Error
    loadErr && h('div', { className: 'wiz-info', style: { borderColor: 'rgba(239,68,68,.3)', background: 'rgba(239,68,68,.04)', marginBottom: 12 } },
      h('span', { style: { fontSize: 18 } }, '❌'),
      h('div', null,
        h('div', { style: { fontWeight: 600, color: '#ef4444', marginBottom: 3 } }, 'Could not load passports'),
        h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)' } }, loadErr),
        h('button', {
          className: 'btn btn-s btn-sm', style: { marginTop: 8 },
          onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'lifecycle' })),
        }, '→ Check Start / Stop')
      )
    ),

    // Loading skeleton
    loading && !passports && h('div', {
      style: { color: 'var(--t2)', fontSize: 'var(--fsm)', textAlign: 'center', padding: 32 }
    }, 'Loading passports…'),

    // Empty
    !loading && passports && visible.length === 0 && h('div', { className: 'wiz-info', style: { textAlign: 'center' } },
      h('span', { style: { fontSize: 20 } }, search ? '🔍' : '📭'),
      h('div', null,
        search
          ? h('div', null,
              h('div', { style: { fontWeight: 600, marginBottom: 4 } }, 'No match for "' + search + '"'),
              h('button', { className: 'btn btn-s btn-sm', style: { marginTop: 6 }, onClick: () => setSearch('') }, '✕ Clear search')
            )
          : h('div', null,
              h('div', { style: { fontWeight: 600, marginBottom: 4 } }, 'No passports yet'),
              h('button', {
                className: 'btn btn-p btn-sm', style: { marginTop: 8 },
                onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'wizard' })),
              }, '→ Protect My Agent')
            )
      )
    ),

    // Passport rows
    visible.map(pp => h(DashRow, { key: pp.path, pp, gwUrl, onRefresh: load })),

    // Backup / restore link
    passports && passports.length > 0 && h('div', {
      style: {
        marginTop: 20, padding: '10px 14px', borderRadius: 'var(--r)',
        border: '1px solid var(--b3)', background: 'var(--b1)',
        display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap',
      }
    },
      h('div', { style: { flex: 1 } },
        h('div', { style: { fontWeight: 600, fontSize: 'var(--fsm)' } }, '💾 Backup your passports'),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 2 } },
          'Export all passports to an encrypted file before reinstalling or switching machines.')
      ),
      h('button', {
        className: 'btn btn-s btn-sm',
        onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'vault' })),
      }, '→ Passport Vault')
    )
  );
}
