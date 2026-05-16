// ─────────────────────────────────────────────────────────────────────────────
// REVOKE CONFIRM — two-step inline revoke with reversibility timeline
// ─────────────────────────────────────────────────────────────────────────────

function RevokeConfirm({ agentName, onConfirm, onCancel, revoking }) {
  const [step, setStep] = useState(1);

  // Step 1: What does revoke actually mean?
  if (step === 1) {
    return h('div', {
      style: {
        marginTop: 8, padding: '14px 16px',
        border: '1px solid rgba(239,68,68,.22)',
        borderRadius: 'var(--r)',
        background: 'rgba(239,68,68,.04)',
      }
    },
      h('div', { style: { fontWeight: 700, color: '#ef4444', fontSize: 'var(--fsm)', marginBottom: 8 } },
        '⚠ Revoke "' + agentName + '"?'),

      h('div', { style: { display: 'flex', flexDirection: 'column', gap: 6, marginBottom: 12 } },
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t1)', lineHeight: 1.7 } },
          h('strong', null, 'What happens: '), 'The agent is blocked immediately from authorizing any actions.'),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t1)', lineHeight: 1.7 } },
          h('strong', { style: { color: 'var(--green)' } }, 'Is it permanent? No.'),
          ' You can issue a new passport at any time. The whole process takes about 60 seconds.'),
        h('div', {
          style: {
            display: 'flex', gap: 6, alignItems: 'center', flexWrap: 'wrap',
            padding: '8px 10px', background: 'var(--b2)', borderRadius: 'var(--r)', marginTop: 2,
          }
        },
          h('span', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', fontWeight: 600 } }, 'Recovery path:'),
          ...[
            'Revoke',
            '→ Protect My Agent',
            '→ New passport (30 sec)',
            '→ Reconnect agent',
          ].map((s, i, arr) => h('span', {
            key: i,
            style: {
              fontSize: 'var(--fxs)', color: i === 0 ? '#ef4444' : i === arr.length - 1 ? 'var(--green)' : 'var(--t2)',
              fontWeight: i === 0 || i === arr.length - 1 ? 700 : 400,
            }
          }, s))
        )
      ),

      h('div', { style: { display: 'flex', gap: 8 } },
        h('button', {
          style: {
            background: 'rgba(239,68,68,.1)', color: '#ef4444',
            border: '1px solid rgba(239,68,68,.28)', borderRadius: 'var(--r)',
            padding: '6px 14px', cursor: 'pointer', fontWeight: 700,
            fontSize: 'var(--fxs)',
          },
          onClick: () => setStep(2),
        }, 'Yes, revoke access'),
        h('button', { className: 'btn btn-s btn-sm', onClick: onCancel }, 'Cancel')
      )
    );
  }

  // Step 2: Final confirm with recovery reminder
  return h('div', {
    style: {
      marginTop: 8, padding: '14px 16px',
      border: '1px solid rgba(239,68,68,.35)',
      borderRadius: 'var(--r)',
      background: 'rgba(239,68,68,.07)',
    }
  },
    h('div', { style: { fontWeight: 700, color: '#ef4444', fontSize: 'var(--fsm)', marginBottom: 4 } },
      'Last step — confirm revoke'),
    h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', lineHeight: 1.65, marginBottom: 12 } },
      'After revoking, go to ',
      h('strong', null, 'Protect My Agent'),
      ' to issue a new passport and reconnect. Takes about 60 seconds.'),
    h('div', { style: { display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' } },
      h('button', {
        style: {
          background: '#ef4444', color: '#fff', border: 'none',
          borderRadius: 'var(--r)', padding: '6px 18px',
          cursor: revoking ? 'not-allowed' : 'pointer',
          fontWeight: 700, fontSize: 'var(--fxs)',
          opacity: revoking ? .6 : 1,
        },
        disabled: revoking,
        onClick: onConfirm,
      }, revoking ? 'Revoking…' : 'Revoke now'),
      h('button', { className: 'btn btn-s btn-sm', onClick: onCancel, disabled: revoking }, 'Cancel'),
      !revoking && h('span', { style: { fontSize: 'var(--fxs)', color: 'var(--green)', fontWeight: 600 } },
        '✓ Re-issue a new passport in 60 sec')
    )
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// PASSPORT PROBLEM GUIDE — inline contextual help when a passport has issues
// ─────────────────────────────────────────────────────────────────────────────

function PassportProblemGuide({ issue, agentName, onRenewClick, onProtectClick }) {
  const [open, setOpen] = useState(false);

  const config = {
    expired: {
      label: 'Passport expired — agent is blocked',
      color: '#ef4444',
      bg: 'rgba(239,68,68,.06)',
      border: 'rgba(239,68,68,.22)',
      icon: '🔴',
      title: 'Why your agent stopped working',
      body: 'The passport reached its expiry date. The agent cannot authorize any actions until you renew. Renewing takes about 30 seconds and does not delete anything.',
      steps: [
        'Select a new duration from the dropdown above.',
        'Click "Renew passport" — the file updates automatically.',
        'Restart your agent (Claude Code, OpenClaw, etc.).',
        'Done — your agent is protected again.',
      ],
      action: { label: '↺ Renew now', fn: onRenewClick },
    },
    expiring: {
      label: 'Passport expires soon',
      color: '#ca8a04',
      bg: 'rgba(202,138,4,.06)',
      border: 'rgba(202,138,4,.22)',
      icon: '🟡',
      title: 'Renew now to avoid any interruption',
      body: 'The passport will expire soon. Renew it now so your agent keeps working — no downtime, no data loss.',
      steps: [
        'Select a new duration from the dropdown.',
        'Click "Renew passport".',
        'Restart your agent to pick up the renewed file.',
      ],
      action: { label: '↺ Renew now', fn: onRenewClick },
    },
    none: {
      label: 'No passport found — agent is unprotected',
      color: 'var(--t2)',
      bg: 'var(--b2)',
      border: 'var(--b3)',
      icon: '⚪',
      title: 'This agent has no A1 passport yet',
      body: 'Without a passport, A1 cannot verify who authorized this agent\'s actions. Creating one takes about 60 seconds.',
      steps: [
        'Click "Protect this agent" below.',
        'Choose the capabilities this agent needs.',
        'Connect and restart the agent.',
      ],
      action: { label: '🛡 Protect this agent', fn: onProtectClick },
    },
  };

  const cfg = config[issue];
  if (!cfg) return null;

  return h('div', { style: { marginTop: 6 } },
    h('div', {
      style: { display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer', padding: '4px 0' },
      onClick: () => setOpen(o => !o),
    },
      h('span', { style: { fontSize: 13 } }, cfg.icon),
      h('span', { style: { fontSize: 'var(--fxs)', color: cfg.color, fontWeight: 600 } }, cfg.label),
      h('span', { style: { fontSize: 'var(--fxs)', color: 'var(--t3)', marginLeft: 'auto' } }, open ? '▲' : '▼ what to do')
    ),
    open && h('div', {
      style: {
        marginTop: 6, padding: '10px 12px',
        border: '1px solid ' + cfg.border, borderRadius: 'var(--r)', background: cfg.bg,
      }
    },
      h('div', { style: { fontWeight: 600, fontSize: 'var(--fsm)', marginBottom: 4 } }, cfg.title),
      h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', lineHeight: 1.6, marginBottom: 8 } }, cfg.body),
      h('div', { style: { display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 10 } },
        cfg.steps.map((s, i) => h('div', { key: i, style: { display: 'flex', gap: 8, alignItems: 'flex-start' } },
          h('div', {
            style: {
              width: 18, height: 18, borderRadius: '50%', background: cfg.color,
              color: '#fff', fontSize: 10, fontWeight: 700, display: 'flex',
              alignItems: 'center', justifyContent: 'center', flexShrink: 0, marginTop: 1,
            }
          }, i + 1),
          h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t1)', lineHeight: 1.5 } }, s)
        ))
      ),
      h('button', {
        className: 'btn btn-p btn-sm',
        onClick: () => { setOpen(false); cfg.action.fn(); },
      }, cfg.action.label)
    )
  );
}
