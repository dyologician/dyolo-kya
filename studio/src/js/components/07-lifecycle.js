// ─────────────────────────────────────────────────────────────────────────────
// LIFECYCLE — start/stop/status/restart, with offline agent awareness
// ─────────────────────────────────────────────────────────────────────────────
function Lifecycle() {
  const { api, settings } = useContext(Ctx);
  const gwUrl = (settings && settings.gwUrl) || window.A1_GW_URL || 'http://localhost:8080';
  const [health, setHealth]                   = useState(null);
  const [checking, setChecking]               = useState(false);
  const [action, setAction]                   = useState(null);
  const [stopping, setStopping]               = useState(false);
  const [stopMsg, setStopMsg]                 = useState(null);
  const [installingAutostart, setInstalling]  = useState(false);
  const [autostartResult, setAutostartResult] = useState(null);
  const [connectedAgents, setConnectedAgents] = useState(null);
  // Restart state
  const [restarting, setRestarting]           = useState(false);
  const [restartMsg, setRestartMsg]           = useState(null);
  // Background poll ref — keeps watching for gateway after manual-start prompt
  const manualPollRef = useRef(null);
  // Stopped-state poll ref — backs the "Watching for A1…" indicator in the Start card
  const stoppedPollRef = useRef(null);

  async function check() {
    setChecking(true);
    const r = await api('GET', '/health');
    setHealth(r.ok ? 'running' : 'stopped');
    setChecking(false);
  }

  async function loadAgents() {
    const r = await api('GET', '/v1/agents/scan');
    if (r.ok) setConnectedAgents((r.data.agents || []).filter(a => a.connected));
  }

  async function stopGateway() {
    setStopping(true); setStopMsg(null);
    try {
      await fetch(gwUrl + '/v1/system/shutdown', {
        method: 'POST', signal: AbortSignal.timeout(3000),
      });
    } catch (_) { /* gateway shut down before responding — that's expected */ }
    setStopping(false);
    setStopMsg('A1 stopped. Run ./setup.sh or double-click setup.sh to start again.');
    setHealth('stopped');
  }

  async function forceStopAll() {
    setStopping(true); setStopMsg(null);
    try {
      await fetch(gwUrl + '/v1/system/shutdown', {
        method: 'POST', signal: AbortSignal.timeout(3000),
      });
    } catch (_) {}
    try {
      await fetch(gwUrl + '/v1/system/force-stop', {
        method: 'POST', signal: AbortSignal.timeout(5000),
      });
    } catch (_) {}
    setStopping(false);
    setStopMsg('All A1 containers stopped. Run ./setup.sh to start again.');
    setHealth('stopped');
  }

  // One-click restart: shutdown → poll until healthy again (autostart/launchd
  // will restart the process automatically if it was set up).
  async function restartGateway() {
    setRestarting(true);
    setRestartMsg({ phase: 'stopping' });

    // 1. Tell the gateway to shut down
    try {
      await fetch(gwUrl + '/v1/system/shutdown', {
        method: 'POST', signal: AbortSignal.timeout(4000),
      });
    } catch (_) { /* expected — gateway exits before sending response */ }

    // 2. Wait a moment for the old process to fully exit
    await new Promise(r => setTimeout(r, 1500));

    // 3. Poll until the gateway comes back (up to 60 seconds)
    setRestartMsg({ phase: 'waiting' });
    let came_back = false;
    for (let i = 0; i < 60; i++) {
      await new Promise(r => setTimeout(r, 1000));
      try {
        const r = await fetch(gwUrl + '/health', { signal: AbortSignal.timeout(2000) });
        if (r.ok) { came_back = true; break; }
      } catch (_) { /* still restarting */ }
    }

    setRestarting(false);
    if (came_back) {
      setRestartMsg({ phase: 'done' });
      setHealth('running');
      setTimeout(() => setRestartMsg(null), 4000);
    } else {
      // Autostart wasn't configured — ask user to start manually.
      // IMPORTANT: keep polling in the background so the page auto-advances
      // the moment the user runs ./setup.sh — no manual refresh needed.
      setRestartMsg({ phase: 'manual' });
      setHealth('stopped');

      if (manualPollRef.current) clearInterval(manualPollRef.current);
      manualPollRef.current = setInterval(async () => {
        try {
          const probe = await fetch(gwUrl + '/health', { signal: AbortSignal.timeout(2000) });
          if (probe.ok) {
            clearInterval(manualPollRef.current);
            manualPollRef.current = null;
            setRestartMsg({ phase: 'done' });
            setHealth('running');
            setTimeout(() => setRestartMsg(null), 4000);
          }
        } catch (_) { /* gateway still offline — keep waiting */ }
      }, 3000);
    }
  }

  useEffect(() => { check(); loadAgents(); }, []);

  // When gateway is stopped, poll every 4 s so the UI auto-updates the moment
  // the user runs ./setup.sh — without any manual refresh.
  useEffect(() => {
    if (health === 'stopped' && !restarting) {
      if (!stoppedPollRef.current) {
        stoppedPollRef.current = setInterval(async () => {
          try {
            const r = await fetch(gwUrl + '/health', { signal: AbortSignal.timeout(2000) });
            if (r.ok) {
              clearInterval(stoppedPollRef.current);
              stoppedPollRef.current = null;
              setHealth('running');
              setStopMsg(null);
            }
          } catch (_) { /* still offline */ }
        }, 4000);
      }
    } else {
      if (stoppedPollRef.current) {
        clearInterval(stoppedPollRef.current);
        stoppedPollRef.current = null;
      }
    }
  }, [health, restarting]);

  // Clean up all background polls on unmount
  useEffect(() => () => {
    if (manualPollRef.current) clearInterval(manualPollRef.current);
    if (stoppedPollRef.current) clearInterval(stoppedPollRef.current);
  }, []);

  const running = health === 'running';

  return h('div', { style: { paddingBottom: 40, width: '100%' } },
    h('h2', { style: { fontSize: 18, fontWeight: 700, marginBottom: 4 } }, '⚡ A1 Status & Control'),
    h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', marginBottom: 16, lineHeight: 1.6 } },
      'Start, stop, or restart A1 from here. No terminal needed for most actions.'),

    // ── Status indicator ───────────────────────────────────────────────────
    h('div', { className: 'status-bar' },
      h('div', { className: 'status-dot ' + (checking || restarting ? 'yellow pulse' : running ? 'green pulse' : 'red') }),
      h('div', { style: { flex: 1 } },
        checking   ? h('span', { style: { color: 'var(--t2)' } }, 'Checking…') :
        restarting ? h('span', { style: { color: '#f59e0b' } }, 'Restarting A1…') :
        running    ? h('strong', null, 'A1 is running — your agents are protected') :
                     h('span', { style: { color: '#ef4444', fontWeight: 700 } }, 'A1 is not running')
      ),
      h('button', { className: 'btn btn-s btn-sm', onClick: check, disabled: checking || restarting }, checking ? '…' : 'Refresh')
    ),

    // ── Offline agent impact notice ────────────────────────────────────────
    !running && !checking && connectedAgents && connectedAgents.length > 0 &&
      h('div', {
        style: { background: 'rgba(239,68,68,.06)', border: '1px solid rgba(239,68,68,.22)', borderRadius: 'var(--r)', padding: '10px 14px', marginBottom: 12, display: 'flex', gap: 10 }
      },
        h('span', { style: { fontSize: 20, flexShrink: 0 } }, '⚠️'),
        h('div', null,
          h('div', { style: { fontWeight: 700, color: '#ef4444', fontSize: 'var(--fsm)', marginBottom: 4 } },
            connectedAgents.length + ' connected agent' + (connectedAgents.length > 1 ? 's' : '') + ' affected'),
          connectedAgents.map(a => h('div', { key: a.id, style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginBottom: 2 } }, '• ' + a.name)),
          h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 4 } },
            'Start A1 below to restore authorization for these agents.')
        )
      ),

    // ── Stop message (shown after stopping) ─────────────────────────────────
    stopMsg && h('div', {
      style: { background: 'rgba(34,197,94,.06)', border: '1px solid rgba(34,197,94,.25)', borderRadius: 'var(--r)', padding: '10px 14px', marginBottom: 12, fontSize: 'var(--fsm)', color: 'var(--t2)' }
    }, '✅ ' + stopMsg),

    // ── Restart status messages ──────────────────────────────────────────────
    restartMsg && restartMsg.phase === 'waiting' && h('div', {
      style: { background: 'rgba(245,158,11,.06)', border: '1px solid rgba(245,158,11,.25)', borderRadius: 'var(--r)', padding: '10px 14px', marginBottom: 12, fontSize: 'var(--fsm)', color: '#f59e0b' }
    }, '⏳ Waiting for A1 to restart… (up to ~60s if first boot)'),

    restartMsg && restartMsg.phase === 'done' && h('div', {
      style: { background: 'rgba(34,197,94,.06)', border: '1px solid rgba(34,197,94,.25)', borderRadius: 'var(--r)', padding: '10px 14px', marginBottom: 12, fontSize: 'var(--fsm)', color: 'var(--green)' }
    }, '✅ A1 restarted successfully — agents are protected again.'),

    restartMsg && restartMsg.phase === 'manual' && h('div', {
      style: { background: 'rgba(239,68,68,.06)', border: '1px solid rgba(239,68,68,.25)', borderRadius: 'var(--r)', padding: '12px 14px', marginBottom: 12 }
    },
      h('div', { style: { fontWeight: 700, color: '#ef4444', fontSize: 'var(--fsm)', marginBottom: 6 } },
        '⚠️ A1 didn\'t restart automatically'
      ),
      h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginBottom: 8 } },
        'Auto-start may not be enabled. Double-click ',
        h('strong', null, 'setup.sh'),
        ' in your A1 folder, or run:'
      ),
      h('div', {
        style: { display: 'flex', alignItems: 'center', gap: 8, fontFamily: 'var(--mono)', fontSize: 11,
          background: 'var(--b1)', border: '1px solid var(--b3)', borderRadius: 'var(--r)', padding: '6px 10px', marginBottom: 10 }
      },
        h('span', { style: { flex: 1 } }, './setup.sh'),
        h('button', {
          className: 'btn btn-p btn-sm', style: { padding: '2px 8px', fontSize: 10 },
          onClick: () => navigator.clipboard.writeText('./setup.sh').catch(() => {}),
        }, 'Copy')
      ),
      // Live reconnect indicator — background poll fires every 3 s automatically.
      // This banner disappears the moment A1 comes back online. No refresh needed.
      h('div', {
        style: { display: 'flex', alignItems: 'center', gap: 8, padding: '8px 10px',
          background: 'rgba(245,158,11,.07)', border: '1px solid rgba(245,158,11,.25)',
          borderRadius: 'var(--r)', fontSize: 'var(--fxs)', color: '#f59e0b' }
      },
        h('div', {
          style: { width: 8, height: 8, borderRadius: '50%', background: '#f59e0b', flexShrink: 0,
            animation: 'pulse 1.4s ease-in-out infinite' }
        }),
        'Waiting for A1 to start… this page updates automatically — no refresh needed'
      )
    ),

    // ── Action cards ────────────────────────────────────────────────────────
    h('div', { style: { display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, marginBottom: 12 } },

      // Stop / Start card
      h('div', {
        className: 'lc-banner',
        style: { margin: 0, borderColor: running ? 'rgba(239,68,68,.25)' : 'rgba(34,197,94,.25)', background: running ? 'rgba(239,68,68,.04)' : 'rgba(34,197,94,.04)' },
      },
        h('div', { className: 'lc-icon' }, running ? '🛑' : '▶️'),
        h('div', null,
          h('div', { className: 'lc-title', style: { color: running ? '#ef4444' : 'var(--green)' } }, running ? 'Stop A1' : 'Start A1'),
          h('div', { className: 'lc-body' },
            running
              ? 'Stops the gateway. Agents cannot authorize until you start it again.'
              : 'Starts the A1 gateway. Protected agents resume normal operation.'),
          h('div', { className: 'lc-actions' },
            running
              ? h('div', { style: { display: 'flex', flexDirection: 'column', gap: 5 } },
                  h('button', {
                    className: 'btn btn-sm',
                    style: { background: 'rgba(239,68,68,.15)', color: '#ef4444', border: 'none', borderRadius: 'var(--r)', padding: '5px 12px', cursor: 'pointer', fontSize: 'var(--fxs)', fontWeight: 600 },
                    disabled: stopping || restarting,
                    onClick: stopGateway,
                  }, stopping ? 'Stopping…' : 'Stop A1'),
                  h('button', {
                    className: 'btn btn-sm',
                    style: { background: 'rgba(239,68,68,.08)', color: '#ef4444', border: '1px solid rgba(239,68,68,.25)', borderRadius: 'var(--r)', padding: '4px 10px', cursor: 'pointer', fontSize: 10 },
                    disabled: stopping || restarting,
                    onClick: forceStopAll,
                    title: 'Stops all A1 Docker containers, including ones started from other folders',
                  }, '⚡ Force stop all containers')
                )
              : h('div', { style: { display: 'flex', flexDirection: 'column', gap: 6 } },
                  h('div', { style: { fontFamily: 'var(--mono)', fontSize: 11, background: 'var(--b1)', border: '1px solid var(--b3)', borderRadius: 'var(--r)', padding: '5px 8px', color: 'var(--t1)', display: 'flex', gap: 6, alignItems: 'center' } },
                    h('span', null, './setup.sh'),
                    h('button', { className: 'btn btn-s btn-sm', style: { padding: '2px 7px', fontSize: 10 }, onClick: () => navigator.clipboard.writeText('./setup.sh') }, 'Copy')
                  ),
                  h('div', { style: { display: 'flex', alignItems: 'center', gap: 5, fontSize: 'var(--fxs)', color: '#f59e0b' } },
                    h('div', { style: { width: 6, height: 6, borderRadius: '50%', background: '#f59e0b', animation: 'pulse 1.4s ease-in-out infinite', flexShrink: 0 } }),
                    'Watching for A1…'
                  )
                )
          )
        )
      ),

      // Restart card — one-click, no terminal
      h('div', {
        className: 'lc-banner',
        style: { margin: 0, cursor: restarting ? 'default' : 'pointer' },
        onClick: !restarting ? restartGateway : undefined,
      },
        h('div', { className: 'lc-icon' }, restarting ? '⏳' : '↺'),
        h('div', null,
          h('div', { className: 'lc-title' }, 'Restart'),
          h('div', { className: 'lc-body' }, 'Use if something seems stuck or after changing settings.'),
          h('div', { className: 'lc-actions' },
            h('button', {
              className: 'btn btn-s btn-sm',
              disabled: restarting,
              onClick: e => { e.stopPropagation(); if (!restarting) restartGateway(); },
            }, restarting ? '↺ Restarting…' : '↺ Restart A1')
          )
        )
      )
    ),

    // ── Auto-start (prominent) ─────────────────────────────────────────────
    h('div', { style: {
      background: autostartResult?.success
        ? 'rgba(34,197,94,.06)'
        : 'rgba(99,102,241,.07)',
      border: '1px solid ' + (autostartResult?.success ? 'rgba(34,197,94,.3)' : 'rgba(99,102,241,.3)'),
      borderRadius: 'var(--r)', padding: '14px 16px', marginBottom: 12,
    }},
      h('div', { style: { display: 'flex', alignItems: 'flex-start', gap: 12 } },
        h('span', { style: { fontSize: 28, flexShrink: 0 } }, autostartResult?.success ? '✅' : '🔒'),
        h('div', { style: { flex: 1 } },
          h('div', { style: { fontWeight: 700, fontSize: 14, marginBottom: 4 } },
            autostartResult?.success ? 'Auto-start enabled — agents stay protected forever' : 'Keep A1 running automatically (recommended)'),
          h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', lineHeight: 1.6, marginBottom: 10 } },
            autostartResult?.success
              ? 'A1 will start automatically every time you log in. Your agents never go offline.'
              : 'Without this, A1 stops when you reboot or close the terminal — and all your agents lose authorization. One click to fix it forever.'),
          autostartResult?.success
            ? h('div', { style: { fontFamily: 'var(--mono)', fontSize: 'var(--fxs)', color: 'var(--t2)' } },
                autostartResult.path ? 'Service: ' + autostartResult.path : '')
            : h('div', { style: { display: 'flex', gap: 8, flexWrap: 'wrap' } },
                h('button', {
                  className: 'btn btn-p', disabled: installingAutostart,
                  onClick: async () => {
                    setInstalling(true); setAutostartResult(null);
                    const r = await fetch(gwUrl + '/v1/system/autostart', { method: 'POST' })
                      .then(r => r.json()).catch(e => ({ success: false, error: e.message }));
                    setAutostartResult(r); setInstalling(false);
                  }
                }, installingAutostart ? 'Installing…' : '⚡ Enable auto-start — one click'),
                h('button', {
                  className: 'btn btn-s btn-sm', style: { alignSelf: 'center' },
                  onClick: async () => {
                    const r = await fetch(gwUrl + '/v1/system/autostart', { method: 'DELETE' })
                      .then(r => r.json()).catch(e => ({ success: false, error: e.message }));
                    setAutostartResult(r);
                  }
                }, 'Remove')
              ),
          autostartResult && !autostartResult.success && h('div', { style: { marginTop: 8, color: '#ef4444', fontSize: 'var(--fxs)' } },
            '❌ ' + autostartResult.error)
        )
      )
    ),

    // ── How to exit / disconnect ────────────────────────────────────────────
    h('div', { className: 'sg', style: { marginTop: 12 } },
      h('div', { className: 'sg-head' }, 'How to exit or disconnect'),
      h('div', { className: 'sg-body' },
        h('div', { style: { display: 'flex', flexDirection: 'column', gap: 10 } },
          [
            { title: 'Close this Studio tab', body: 'Just close the browser tab. A1 keeps running — your agents stay protected.' },
            { title: 'Stop A1 completely', body: ['Click "Stop A1" above, or run ', h('code', { style: { fontFamily: 'var(--mono)', background: 'var(--b1)', padding: '1px 5px', borderRadius: 3 } }, './setup.sh stop'), ' in your terminal.'] },
            { title: 'Disconnect a specific agent', body: 'Go to "Connect Agents" → find the agent → click Disconnect.' },
            { title: 'Start A1 again later', body: ['Run ', h('code', { style: { fontFamily: 'var(--mono)', background: 'var(--b1)', padding: '1px 5px', borderRadius: 3 } }, './setup.sh'), ' or double-click setup.sh — all your passports and settings are preserved.'] },
          ].map(({ title, body }) =>
            h('div', { key: title },
              h('div', { style: { fontWeight: 600, fontSize: 'var(--fsm)', marginBottom: 3 } }, title),
              h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', lineHeight: 1.6 } }, body)
            )
          )
        )
      )
    ),

    h(GuidedNext, { currentTab: 'lifecycle' }),

    // Setup failure guide — shown when A1 won't start
    !running && !checking && h('div', { className: 'sg', style: { marginTop: 12 } },
      h('div', { className: 'sg-head' }, '🚑 A1 won\'t start? Try these in order'),
      h('div', { className: 'sg-body' },
        h('div', { style: { display: 'flex', flexDirection: 'column', gap: 10 } },
          [
            {
              n: '1', title: 'Port conflict — another app is using port 8080',
              body: 'Run this to find the conflicting process:',
              code: 'lsof -i :8080',
              fix: 'Then: kill -9 <PID>   or set A1_PORT=8081 in your environment.',
            },
            {
              n: '2', title: 'Docker Desktop not installed',
              body: 'If the A1 binary didn\'t work, you need Docker Desktop. Install it free:',
              code: '# Mac:  https://docs.docker.com/desktop/install/mac-install/\n# Windows: https://docs.docker.com/desktop/install/windows-install/\n# Linux: https://docs.docker.com/desktop/install/linux-install/\n\n# Then start the gateway:\ndocker run -d -p 8080:8080 ghcr.io/dyologician/a1-gateway:2.8.0',
              fix: 'After installing Docker Desktop, open it once to start the engine, then run the command above.',
              dockerLink: true,
            },
            {
              n: '3', title: 'Binary not in PATH',
              body: 'Check the binary is installed:',
              code: 'which a1\n# If not found:\ncurl -sSL https://get.a1.dev | sh',
              fix: 'Then open a new terminal and try again.',
            },
            {
              n: '4', title: 'Permission denied',
              body: 'Make the binary executable:',
              code: 'chmod +x ~/.a1/bin/a1',
              fix: '',
            },
            {
              n: '5', title: 'Still stuck',
              body: 'Ask for help with the full error:',
              code: 'docker compose logs gateway 2>&1 | tail -30',
              fix: 'Paste the output at github.com/dyologician/A1/issues',
            },
          ].map(item => h('div', { key: item.n, style: { borderLeft: '2px solid var(--b3)', paddingLeft: 12 } },
            h('div', { style: { fontWeight: 700, fontSize: 'var(--fsm)', marginBottom: 4 } }, item.n + '. ' + item.title),
            h('div', { style: { color: 'var(--t2)', fontSize: 'var(--fxs)', marginBottom: 5 } }, item.body),
            item.dockerLink && h('div', { style: { display: 'flex', gap: 6, flexWrap: 'wrap', marginBottom: 8 } },
              h('a', { href: 'https://docs.docker.com/desktop/install/mac-install/', target: '_blank', rel: 'noreferrer', className: 'btn btn-p btn-sm' }, '⬇ Install Docker — Mac'),
              h('a', { href: 'https://docs.docker.com/desktop/install/windows-install/', target: '_blank', rel: 'noreferrer', className: 'btn btn-p btn-sm' }, '⬇ Install Docker — Windows'),
              h('a', { href: 'https://docs.docker.com/desktop/install/linux-install/', target: '_blank', rel: 'noreferrer', className: 'btn btn-s btn-sm' }, '⬇ Linux')
            ),
            h('div', { className: 'wiz-code' },
              h('pre', { style: { margin: 0, whiteSpace: 'pre-wrap', wordBreak: 'break-word', fontFamily: 'var(--mono)', fontSize: 'var(--fxs)' } }, item.code),
              h('button', { className: 'btn btn-s btn-sm wiz-copy-btn', onClick: () => navigator.clipboard.writeText(item.code) }, 'Copy')
            ),
            item.fix && h('div', { style: { color: 'var(--t3)', fontSize: 'var(--fxs)', marginTop: 4 } }, item.fix)
          ))
        )
      )
    )
  );
}
