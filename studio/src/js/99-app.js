// simpleMode: visible in Simple, Advanced, and Dev
// advancedOnly: hidden in Simple mode
// devOnly: only visible in Dev mode
const TABS = [
  { id: 'dashboard',  label: 'Dashboard',      icon: '📊', group: 'Get Started',
    hint: 'See what A1 is protecting and recent activity' },
  { id: 'quickstart', label: 'Setup',           icon: '🚀', group: 'Get Started',
    hint: 'Step-by-step: create a passport and connect your agent — no code required' },
  { id: 'agents',     label: 'My Agents',       icon: '🤖', group: 'Get Started',
    hint: 'Connect, manage, and monitor your AI agents' },
  { id: 'vault',      label: 'Passports',       icon: '🗂', group: 'Get Started',
    hint: 'View, renew, or revoke agent passports' },
  { id: 'lifecycle',  label: 'Start / Stop',    icon: '⚡', group: 'Get Started',
    hint: 'Start, stop, or restart A1 — enable auto-start on login' },
  { id: 'errors',     label: 'Help',            icon: '🔎', group: 'Get Started',
    hint: 'Plain-English explanations and fix steps for any error' },

  { id: 'wizard',     label: 'Manual Setup',    icon: '🛡', group: 'Advanced', advancedOnly: true,
    hint: 'Step-by-step manual setup — pick capabilities, create passport, connect agent' },
  { id: 'chat',       label: 'Test A1',         icon: '✅', group: 'Advanced', advancedOnly: true,
    hint: 'Call A1 MCP tools directly and see raw JSON responses' },
  { id: 'gallery',    label: 'Examples',        icon: '🧪', group: 'Advanced', advancedOnly: true,
    hint: 'One-click example agents — pre-filled passport, capabilities, and code' },
  { id: 'assistant',  label: 'AI Tools',        icon: '🧠', group: 'Advanced', advancedOnly: true,
    hint: 'AI Assistant + Local LLM — connect Ollama, LM Studio, llama.cpp' },
  { id: 'integrate',  label: 'AI Integration',  icon: '🤝', group: 'Advanced', advancedOnly: true,
    hint: 'Auto-add A1 to your existing agent source files' },
  { id: 'howitworks', label: 'How It Works',    icon: '📖', group: 'Advanced', advancedOnly: true,
    hint: 'The cryptographic identity model behind A1' },
  { id: 'direct',     label: 'Direct Connect',  icon: '⚙', group: 'Advanced', advancedOnly: true, devOnly: true,
    hint: 'Low-level MCP probe and relay for custom agent setups' },

  { id: 'devtools',   label: 'Dev Tools',       icon: '⌥', group: 'Developer', advancedOnly: true, devOnly: true,
    hint: 'Gateway monitor, live log, raw passport ops, swarms, DID & VC, authorize testing, compliance' },

  { id: 'settings',   label: 'Settings',        icon: '⚙', group: 'Config' },
];

const GROUPS = ['Get Started', 'Advanced', 'Developer', 'Config'];

function App() {
  const [tab, setTab]                 = useState(!hasOnboarded() ? 'quickstart' : 'dashboard');
  const [settings, setSettings]       = useState(loadS);
  const [health, setHealth]           = useState(null);
  const [logs, setLogs]               = useState([]);
  const [helpMode, setHelpMode]       = useState(false);
  const [showOnboard, setShowOnboard] = useState(!hasOnboarded());
  const [wizardPrefill, setWizardPrefill] = useState(null);
  const [mobileSb, setMobileSb]       = useState(false);
  const [collapsedGroups, setCollapsedGroups] = useState(() => {
    try { return JSON.parse(localStorage.getItem('a1_sb_collapsed') || '[]'); } catch { return []; }
  });

  function toggleGroup(g) {
    setCollapsedGroups(prev => {
      const next = prev.includes(g) ? prev.filter(x => x !== g) : [...prev, g];
      try { localStorage.setItem('a1_sb_collapsed', JSON.stringify(next)); } catch {}
      return next;
    });
  }

  useEffect(() => { applyScaling(settings); }, [settings]);
  useEffect(() => {
    function onResize() { if (settings.density === 'auto') applyScaling(settings); }
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, [settings]);

  useEffect(() => { document.documentElement.setAttribute('data-theme', settings.theme); }, [settings.theme]);

  useEffect(() => {
    document.getElementById('root')?.classList.toggle('help-mode', helpMode);
  }, [helpMode]);

  function navigate(dest) {
    // In simple mode, 'wizard' is hidden — redirect to quickstart instead
    function resolveTab(t) {
      if (t === 'wizard' && settings.simpleMode && !settings.developerMode) return 'quickstart';
      return t;
    }
    if (typeof dest === 'string') setTab(resolveTab(dest));
    else if (dest?.tab) {
      setTab(resolveTab(dest.tab));
      if (dest.prefill) setWizardPrefill(dest.prefill);
    }
    setMobileSb(false);
  }

  useEffect(() => {
    function onNav(e) { if (e.detail) navigate(e.detail); }
    window.addEventListener('a1-navigate', onNav);
    return () => window.removeEventListener('a1-navigate', onNav);
  }, [settings.simpleMode, settings.developerMode]);

  const addLog = useCallback(e => {
    setLogs(prev => { const next = [...prev, e]; return next.length > settings.logMax ? next.slice(-settings.logMax) : next; });
  }, [settings.logMax]);

  const api = useApi(settings, addLog);

  const poll = useCallback(async () => {
    const r = await api('GET', '/health');
    setHealth(r.ok ? r.data : null);
  }, [api]);

  useEffect(() => { poll(); const t = setInterval(poll, settings.pollMs); return () => clearInterval(t); }, [poll, settings.pollMs]);

  function updateSettings(s) {
    setSettings(s); saveS(s); applyScaling(s);
    // If switching to simple mode while on an advancedOnly tab, go to dashboard
    const goingSimple = s.simpleMode && !s.developerMode;
    if (goingSimple) {
      const currentTabDef = TABS.find(t => t.id === tab);
      if (currentTabDef?.advancedOnly) setTab('dashboard');
    }
  }

  const errC = logs.filter(l => !l.ok).length;
  const ctx  = { settings, api, addLog };

  function closeOnboard() { setShowOnboard(false); setOnboarded(); }

  const CONTENT = {
    dashboard:    h(ActivityDashboard, null),
    quickstart:   h(QuickStart, null),
    wizard:       h(ProtectAgent, { prefill: wizardPrefill, onPrefillConsumed: () => setWizardPrefill(null) }),
    agents:       h(ConnectAgents, null),
    chat:         h(McpTester, null),
    gallery:      h(ExampleGallery, null),
    vault:        h(PassportsHub, null),
    lifecycle:    h(Lifecycle, null),
    errors:       h(ErrorExplainer, null),
    assistant:    h(AiToolsHub, null),
    integrate:    h(AiIntegration, null),
    direct:       h(DirectConnect, null),
    howitworks:   h(HowItWorks, null),
    devtools:     h(DevToolsHub, { health, logs, onClear: () => setLogs([]) }),
    settings:     h(Settings, { settings, onUpdate: updateSettings, health, onShowOnboard: () => setShowOnboard(true) }),
  };

  const currentTabMeta = TABS.find(t => t.id === tab);

  return h(Ctx.Provider, { value: ctx },
    h('div', { id: '_attr_banner', className: 'integrity-warn' }, '⚠ Attribution integrity check failed — studio links have been modified.'),

    h('div', { className: 'sb-overlay' + (mobileSb ? ' open' : ''), onClick: () => setMobileSb(false) }),

    h('div', { id: 'root' },

      h('div', { className: 'sb' + (mobileSb ? ' mobile-open' : ''), 'data-help': 'sidebar' },

        // ── Logo + Mode pill ─────────────────────────────────────────────────
        h('div', { className: 'sb-logo' },
          h('div', { style: { display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 10 } },
            h('div', null,
              h('div', { className: 'logo-mark' }, 'A1'),
              h('div', { className: 'logo-sub' }, 'Studio')
            ),
            h('div', { style: { display: 'flex', alignItems: 'center', gap: 5 } },
              h('div', { className: 'dot dot-pulse dot-' + (health ? 'green' : 'red'), style: { width: 7, height: 7 } }),
              !health && h('span', {
                style: { fontSize: 10, color: 'var(--accent)', cursor: 'pointer', fontFamily: 'var(--mono)' },
                onClick: () => setTab('lifecycle')
              }, 'offline')
            )
          ),
          h('div', { className: 'mode-pill' },
            h('button', {
              className: 'mode-pill-btn' + (settings.simpleMode ? ' active' : ''),
              onClick: () => updateSettings({ ...settings, simpleMode: true, developerMode: false }),
              title: 'Simple — clean view for everyday use'
            }, 'Simple'),
            h('button', {
              className: 'mode-pill-btn' + (!settings.simpleMode && !settings.developerMode ? ' active' : ''),
              onClick: () => updateSettings({ ...settings, simpleMode: false, developerMode: false }),
              title: 'Advanced — full tools for power users'
            }, 'Advanced'),
            h('button', {
              className: 'mode-pill-btn' + (settings.developerMode ? ' active' : ''),
              onClick: () => updateSettings({ ...settings, simpleMode: false, developerMode: true }),
              title: 'Dev — everything including raw tools and monitors'
            }, 'Dev')
          )
        ),

        // ── Nav ──────────────────────────────────────────────────────────────
        h('div', { className: 'sb-nav' },
          ...GROUPS.flatMap(g => {
            const groupTabs = TABS.filter(t =>
              t.group === g &&
              (!t.devOnly || settings.developerMode) &&
              (!t.advancedOnly || !settings.simpleMode || settings.developerMode)
            );
            if (groupTabs.length === 0) return [];
            return [
              h('div', {
                key: 'g-' + g,
                className: 'sb-sec',
                onClick: () => toggleGroup(g),
                style: { cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'space-between', userSelect: 'none' }
              },
                h('span', null, g),
                h('span', { style: { fontSize: 8, opacity: .5, transition: 'transform .15s', transform: collapsedGroups.includes(g) ? 'rotate(-90deg)' : 'none' } }, '▼')
              ),
              ...groupTabs.filter(() => !collapsedGroups.includes(g)).map(t => h('div', {
                key: t.id,
                className: 'sb-item' + (tab === t.id ? ' on' : ''),
                onClick: () => { setTab(t.id); setMobileSb(false); },
                'data-help': t.id,
                title: t.hint || '',
              },
                h('span', { className: 'sb-icon' }, t.icon),
                h('span', { className: 'sb-label' }, t.label),
                t.id === 'devtools' && errC > 0 && h('span', { className: 'err-pip' }, errC),
                t.id === 'vault' && h(VaultBadge, null),
                t.id === 'agents' && h(AgentsBadge, null)
              ))
            ];
          })
        ),

        // ── Footer ───────────────────────────────────────────────────────────
        h('div', { className: 'sb-foot' },
          h('button', {
            className: 'theme-btn', 'data-help': 'theme-btn',
            onClick: () => updateSettings({ ...settings, theme: settings.theme === 'dark' ? 'light' : 'dark' })
          },
            h('span', null, settings.theme === 'dark' ? '○' : '●'),
            h('span', null, settings.theme === 'dark' ? 'Light' : 'Dark')
          )
        )
      ),

      h('div', { className: 'main' },
        h('div', { className: 'topbar' },
          h('div', { style: { display: 'flex', alignItems: 'center', gap: 8 } },
            h('button', { className: 'sb-hamburger', onClick: () => setMobileSb(o => !o) }, '☰'),
            h('div', { className: 'topbar-title' }, currentTabMeta?.label),
          ),
          h('div', { style: { display: 'flex', alignItems: 'center', gap: 8 } },
            currentTabMeta?.hint && h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t3)', maxWidth: 340, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' } },
              currentTabMeta.hint),
            h('div', { className: 'gw-pill' },
              h('div', { className: 'dot dot-pulse dot-' + (health ? 'green' : 'red') }),
              h('span', null, health ? settings.gwUrl : 'offline')
            ),
            logs.length > 0 && h('span', { className: 'req-count' }, logs.length + ' req')
          )
        ),

        h('div', { className: 'content' },
          h(OfflineBanner, { health }),
          CONTENT[tab]
        )
      )
    ),

    h(HelpButton, { helpMode, onToggle: () => setHelpMode(m => !m) }),
    h(TooltipLayer, { helpMode }),
    showOnboard && h(OnboardModal, { onClose: closeOnboard }),
    h('div', { style: { display: 'none' }, 'aria-hidden': 'true' }, h(SocialLinks, null))
  );
}

function VaultBadge() {
  const { settings } = useContext(Ctx);
  const [count, setCount] = useState(0);

  useEffect(() => {
    fetch((settings.gwUrl || 'http://localhost:8080') + '/v1/passports/list')
      .then(r => r.json())
      .then(d => setCount((d.passports || []).filter(p => p.status === 'expired').length))
      .catch(() => {});
  }, []);

  if (count === 0) return null;
  return h('span', { className: 'err-pip', style: { background: '#ca8a04' } }, count);
}

function AgentsBadge() {
  const { settings } = useContext(Ctx);
  const [count, setCount] = useState(0);

  useEffect(() => {
    fetch((settings.gwUrl || 'http://localhost:8080') + '/v1/agents/scan')
      .then(r => r.json())
      .then(d => setCount((d.agents || []).filter(a => a.connected).length))
      .catch(() => {});
    const t = setInterval(() => {
      fetch((settings.gwUrl || 'http://localhost:8080') + '/v1/agents/scan')
        .then(r => r.json())
        .then(d => setCount((d.agents || []).filter(a => a.connected).length))
        .catch(() => {});
    }, 30000);
    return () => clearInterval(t);
  }, []);

  if (count === 0) return null;
  return h('span', {
    className: 'err-pip',
    style: { background: 'var(--green)', color: '#fff' },
    title: count + ' connected agent' + (count > 1 ? 's' : '')
  }, count);
}

ReactDOM.createRoot(document.getElementById('root')).render(h(App));
