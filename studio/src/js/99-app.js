const TABS = [
  { id: 'quickstart', label: 'Quick Setup',      icon: '🚀', group: 'Get Started',
    hint: 'Automated setup — A1 walks you through everything, no commands needed' },
  { id: 'wizard',     label: 'Protect My Agent', icon: '🛡', group: 'Get Started',
    hint: 'Create a cryptographic passport for any AI agent — manual control' },
  { id: 'agents',     label: 'Connect Agents',   icon: '🔌', group: 'Get Started',
    hint: 'Link A1 to Claude Code, ChatGPT, LangChain, and more' },
  { id: 'chat',       label: 'Test Connection',  icon: '✅',  group: 'Get Started',
    hint: 'Send a test message to confirm your agent is protected' },
  { id: 'gallery',    label: 'Examples',         icon: '🧪', group: 'Get Started',
    hint: 'One-click example agents — pre-filled passport, capabilities, and code' },

  { id: 'vault',      label: 'Passports',        icon: '🗂', group: 'Manage',
    hint: 'All passports — sorted by urgency, renew or revoke, backup and restore' },
  { id: 'lifecycle',  label: 'Start / Stop',     icon: '⚡',  group: 'Manage',
    hint: 'Start, stop, or restart A1 · enable auto-start on login' },
  { id: 'errors',     label: 'Error Help',       icon: '🔎', group: 'Manage',
    hint: 'Plain-English explanations and fix steps for any A1 error' },

  { id: 'assistant',  label: 'AI Tools',         icon: '🧠', group: 'Advanced',
    hint: 'AI Assistant + Local LLM — ask about A1, connect Ollama, LM Studio, llama.cpp' },
  { id: 'integrate',  label: 'AI Integration',   icon: '🤝', group: 'Advanced',
    hint: 'Automatically add A1 to your existing agent source files' },
  { id: 'direct',     label: 'Direct Connect',   icon: '⚙',  group: 'Advanced', devOnly: true,
    hint: 'Low-level MCP probe and relay for custom agent setups' },
  { id: 'howitworks', label: 'How It Works',     icon: '📖', group: 'Advanced',
    hint: 'The cryptographic identity model behind A1' },

  { id: 'devtools',   label: 'Dev Tools',        icon: '⌥',  group: 'Developer', devOnly: true,
    hint: 'Gateway monitor, live log, raw passport ops, swarms, DID & VC, authorize testing, compliance' },

  { id: 'settings',   label: 'Settings',         icon: '⚙',  group: 'Config' },
];

const GROUPS = ['Get Started', 'Manage', 'Advanced', 'Developer', 'Config'];

function App() {
  const [tab, setTab]                 = useState(!hasOnboarded() ? 'quickstart' : 'vault');
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
    if (typeof dest === 'string') setTab(dest);
    else if (dest?.tab) { setTab(dest.tab); if (dest.prefill) setWizardPrefill(dest.prefill); }
    setMobileSb(false);
  }

  useEffect(() => {
    function onNav(e) { if (e.detail) navigate(e.detail); }
    window.addEventListener('a1-navigate', onNav);
    return () => window.removeEventListener('a1-navigate', onNav);
  }, []);

  const addLog = useCallback(e => {
    setLogs(prev => { const next = [...prev, e]; return next.length > settings.logMax ? next.slice(-settings.logMax) : next; });
  }, [settings.logMax]);

  const api = useApi(settings, addLog);

  const poll = useCallback(async () => {
    const r = await api('GET', '/health');
    setHealth(r.ok ? r.data : null);
  }, [api]);

  useEffect(() => { poll(); const t = setInterval(poll, settings.pollMs); return () => clearInterval(t); }, [poll, settings.pollMs]);

  function updateSettings(s) { setSettings(s); saveS(s); applyScaling(s); }

  const errC = logs.filter(l => !l.ok).length;
  const ctx  = { settings, api, addLog };

  function closeOnboard() { setShowOnboard(false); setOnboarded(); }

  const CONTENT = {
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
        h('div', { className: 'sb-logo' },
          h('div', { className: 'logo-mark' }, 'A1'),
          h('div', { className: 'logo-sub' }, 'Studio')
        ),

        h('div', { style: { padding: '0 12px 8px', display: 'flex', alignItems: 'center', gap: 6 } },
          h('div', { className: 'dot dot-pulse dot-' + (health ? 'green' : 'red'), style: { width: 7, height: 7 } }),
          h('span', { style: { fontSize: 'var(--fxs)', color: health ? 'var(--green)' : '#ef4444', fontWeight: 600 } },
            health ? 'A1 running' : 'A1 offline'),
          !health && h('span', {
            style: { fontSize: 'var(--fxs)', color: 'var(--accent)', cursor: 'pointer', marginLeft: 2 },
            onClick: () => setTab('lifecycle')
          }, '→ fix')
        ),

        h('div', { className: 'sb-nav' },
          ...GROUPS.flatMap(g => {
            const groupTabs = TABS.filter(t => t.group === g && (!t.devOnly || settings.developerMode));
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
      .then(d => {
        const agents = d.agents || [];
        setCount(agents.filter(a => a.connected).length);
      })
      .catch(() => {});
    // Refresh every 30 seconds
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
