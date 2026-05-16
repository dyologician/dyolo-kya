// ─────────────────────────────────────────────────────────────────────────────
// LOCAL AI CONNECT — one-click Ollama / LM Studio / llama.cpp integration
// Probes known local ports, lists running models, generates ready-to-paste code
// ─────────────────────────────────────────────────────────────────────────────

const LOCAL_PROVIDERS = [
  {
    id:      'ollama',
    label:   'Ollama',
    port:    11434,
    probe:   '/api/tags',
    models:  d => (d.models || []).map(m => m.name),
    baseUrl: 'http://localhost:11434',
    icon:    '🦙',
    desc:    'Run Llama, Mistral, Gemma and more locally',
  },
  {
    id:      'lmstudio',
    label:   'LM Studio',
    port:    1234,
    probe:   '/v1/models',
    models:  d => (d.data || []).map(m => m.id),
    baseUrl: 'http://localhost:1234',
    icon:    '🧩',
    desc:    'OpenAI-compatible local inference server',
  },
  {
    id:      'llamacpp',
    label:   'llama.cpp server',
    port:    8000,
    probe:   '/v1/models',
    models:  d => (d.data || []).map(m => m.id),
    baseUrl: 'http://localhost:8000',
    icon:    '🔩',
    desc:    'Raw llama.cpp HTTP server',
  },
];

// ── Code generators ───────────────────────────────────────────────────────────

function genPython(provider, model, gwUrl) {
  const isOllama = provider.id === 'ollama';
  return `import a1

client = a1.Client(gateway_url="${gwUrl}")
passport = a1.Passport.load("~/.a1/passports/my-agent.json")

${isOllama
  ? `import ollama

with a1.guard(passport, capabilities=["read", "write"]):
    response = ollama.chat(
        model="${model || 'llama3'}",
        messages=[{"role": "user", "content": "Hello"}],
    )
    print(response["message"]["content"])`
  : `from openai import OpenAI

openai_client = OpenAI(base_url="${provider.baseUrl}/v1", api_key="local")

with a1.guard(passport, capabilities=["read", "write"]):
    response = openai_client.chat.completions.create(
        model="${model || 'local-model'}",
        messages=[{"role": "user", "content": "Hello"}],
    )
    print(response.choices[0].message.content)`
}
`;
}

function genTypescript(provider, model, gwUrl) {
  return `import { A1Client, loadPassport, guard } from "a1";
import OpenAI from "openai";

const passport = await loadPassport("~/.a1/passports/my-agent.json");

const localClient = new OpenAI({
  baseURL: "${provider.baseUrl}/v1",
  apiKey: "local",
});

await guard(passport, ["read", "write"], async () => {
  const response = await localClient.chat.completions.create({
    model: "${model || 'local-model'}",
    messages: [{ role: "user", content: "Hello" }],
  });
  console.log(response.choices[0].message.content);
});
`;
}

function genMcp(gwUrl) {
  return `{
  "mcpServers": {
    "a1": {
      "type": "http",
      "url": "${gwUrl}/mcp"
    }
  }
}`;
}

function genLangchain(provider, model, gwUrl) {
  return `from langchain_openai import ChatOpenAI
from langchain_core.messages import HumanMessage
import a1

client = a1.Client(gateway_url="${gwUrl}")
passport = a1.Passport.load("~/.a1/passports/my-agent.json")

llm = ChatOpenAI(
    base_url="${provider.baseUrl}/v1",
    api_key="local",
    model="${model || 'local-model'}",
)

from a1.langchain_tool import a1_langchain_guard

@a1_langchain_guard(passport, capabilities=["read", "write"])
def run_chain(query: str):
    return llm.invoke([HumanMessage(content=query)])

result = run_chain("What is A1?")
print(result.content)
`;
}

const CODE_TABS = [
  { id: 'python',     label: 'Python',     gen: genPython },
  { id: 'typescript', label: 'TypeScript', gen: genTypescript },
  { id: 'langchain',  label: 'LangChain',  gen: genLangchain },
  { id: 'mcp',        label: '.mcp.json',  gen: (p, m, gw) => genMcp(gw) },
];

// ── Provider probe card ────────────────────────────────────────────────────────

function ProviderCard({ provider, gwUrl, selected, onSelect }) {
  const [status, setStatus] = useState('idle');
  const [models, setModels] = useState([]);

  useEffect(() => {
    setStatus('probing');
    const url = provider.baseUrl + provider.probe;
    fetch(url, { signal: AbortSignal.timeout(2000) })
      .then(r => r.json())
      .then(d => {
        const m = provider.models(d);
        setModels(m);
        setStatus(m.length > 0 ? 'running' : 'empty');
      })
      .catch(() => setStatus('offline'));
  }, [provider.id]);

  const isRunning = status === 'running' || status === 'empty';

  return h('div', {
    style: {
      border: '1px solid ' + (selected ? 'var(--accent)' : 'var(--b3)'),
      borderRadius: 'var(--r)',
      background: selected ? 'rgba(99,102,241,.07)' : 'var(--b1)',
      padding: '12px 14px', cursor: isRunning ? 'pointer' : 'default',
      opacity: status === 'offline' ? .5 : 1,
      transition: 'border-color .15s, background .15s',
    },
    onClick: () => isRunning && onSelect(provider, models),
  },
    h('div', { style: { display: 'flex', alignItems: 'center', gap: 10 } },
      h('span', { style: { fontSize: 22 } }, provider.icon),
      h('div', { style: { flex: 1 } },
        h('div', { style: { fontWeight: 700, fontSize: 'var(--fsm)' } }, provider.label),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', marginTop: 1 } }, provider.desc)
      ),
      h('div', {
        style: {
          fontSize: 'var(--fxs)', fontWeight: 700, padding: '2px 9px',
          borderRadius: 12,
          background: status === 'running' ? 'rgba(34,197,94,.1)' : status === 'probing' ? 'var(--b2)' : 'rgba(239,68,68,.09)',
          color: status === 'running' ? 'var(--green)' : status === 'probing' ? 'var(--t2)' : '#ef4444',
          border: '1px solid ' + (status === 'running' ? 'rgba(34,197,94,.3)' : status === 'probing' ? 'var(--b3)' : 'rgba(239,68,68,.25)'),
          whiteSpace: 'nowrap',
        }
      }, status === 'running' ? '● Running' : status === 'probing' ? '… Probing' : status === 'empty' ? '○ No models' : '○ Offline')
    ),

    isRunning && models.length > 0 && h('div', {
      style: { display: 'flex', flexWrap: 'wrap', gap: 4, marginTop: 8 }
    },
      models.slice(0, 6).map(m => h('span', {
        key: m,
        style: {
          fontFamily: 'var(--mono)', fontSize: 'var(--fxs)',
          background: 'var(--b2)', color: 'var(--t1)',
          padding: '2px 8px', borderRadius: 20, border: '1px solid var(--b3)',
        }
      }, m)),
      models.length > 6 && h('span', {
        style: { fontSize: 'var(--fxs)', color: 'var(--t2)', alignSelf: 'center' }
      }, '+' + (models.length - 6) + ' more')
    ),

    isRunning && h('div', {
      style: { marginTop: 8, fontSize: 'var(--fxs)', color: selected ? 'var(--accent)' : 'var(--t2)', fontWeight: selected ? 700 : 400 }
    }, selected ? '✓ Selected — see code below' : 'Click to generate integration code')
  );
}

// ── Code viewer ───────────────────────────────────────────────────────────────

function CodeViewer({ provider, models, gwUrl }) {
  const [codeTab, setCodeTab]   = useState('python');
  const [model,   setModel]     = useState(models[0] || '');
  const [copied,  setCopied]    = useState(false);

  const tab = CODE_TABS.find(t => t.id === codeTab);
  const code = tab ? tab.gen(provider, model, gwUrl) : '';

  function copy() {
    navigator.clipboard.writeText(code);
    setCopied(true); setTimeout(() => setCopied(false), 1500);
  }

  return h('div', {
    style: {
      marginTop: 16, border: '1px solid var(--accent)',
      borderRadius: 'var(--r)', overflow: 'hidden',
    }
  },
    // Header
    h('div', {
      style: {
        background: 'rgba(99,102,241,.1)', padding: '8px 14px',
        display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap',
        borderBottom: '1px solid rgba(99,102,241,.2)',
      }
    },
      h('span', { style: { fontWeight: 700, fontSize: 'var(--fsm)', color: 'var(--accent)' } },
        provider.icon + ' ' + provider.label + ' integration code'),

      // Model picker
      models.length > 0 && h('select', {
        value: model, onChange: e => setModel(e.target.value),
        style: {
          fontSize: 'var(--fxs)', padding: '3px 7px', marginLeft: 'auto',
          border: '1px solid var(--b3)', borderRadius: 'var(--r)',
          background: 'var(--b1)', color: 'var(--t1)', cursor: 'pointer',
        }
      }, models.map(m => h('option', { key: m, value: m }, m)))
    ),

    // Tab strip
    h('div', {
      style: {
        display: 'flex', background: 'var(--b2)',
        borderBottom: '1px solid var(--b3)',
      }
    },
      CODE_TABS.map(t => h('button', {
        key: t.id,
        onClick: () => setCodeTab(t.id),
        style: {
          padding: '5px 13px', fontSize: 'var(--fxs)', fontWeight: codeTab === t.id ? 700 : 400,
          color: codeTab === t.id ? 'var(--t1)' : 'var(--t2)',
          background: codeTab === t.id ? 'var(--b1)' : 'transparent',
          border: 'none', cursor: 'pointer',
          borderBottom: codeTab === t.id ? '2px solid var(--accent)' : '2px solid transparent',
        }
      }, t.label))
    ),

    // Code block
    h('div', { style: { position: 'relative' } },
      h('pre', {
        style: {
          fontFamily: 'var(--mono)', fontSize: 'var(--fxs)', lineHeight: 1.65,
          padding: '14px 16px', background: 'var(--b2)', margin: 0,
          overflowX: 'auto', color: 'var(--t1)', whiteSpace: 'pre',
          maxHeight: 320, overflowY: 'auto',
        }
      }, code),
      h('button', {
        onClick: copy,
        style: {
          position: 'absolute', top: 8, right: 8,
          padding: '3px 10px', fontSize: 'var(--fxs)', fontWeight: 600,
          background: copied ? 'var(--green)' : 'var(--b3)', color: 'var(--t1)',
          border: '1px solid var(--b2)', borderRadius: 'var(--r)', cursor: 'pointer',
        }
      }, copied ? '✓ Copied' : '⎘ Copy')
    )
  );
}

// ── Root component ─────────────────────────────────────────────────────────────

function LocalLlmConnect() {
  const { settings } = useContext(Ctx);
  const gwUrl = settings.gwUrl || 'http://localhost:8080';

  const [selection, setSelection] = useState(null);
  const [models,    setModels]    = useState([]);

  function onSelect(provider, providerModels) {
    if (selection?.id === provider.id) {
      setSelection(null); setModels([]);
    } else {
      setSelection(provider); setModels(providerModels);
    }
  }

  return h('div', { style: { paddingBottom: 40, width: '100%' } },

    h('h2', { style: { fontSize: 18, fontWeight: 700, marginBottom: 4 } }, '🤖 Local AI Connect'),
    h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', marginBottom: 4, lineHeight: 1.6 } },
      'Connect A1 to a locally-running model — Ollama, LM Studio, or llama.cpp. ',
      'Detection is automatic. Pick a provider, pick a model, copy the code.'),

    h('div', {
      style: {
        padding: '8px 12px', borderRadius: 'var(--r)', marginBottom: 16,
        background: 'rgba(99,102,241,.08)', border: '1px solid rgba(99,102,241,.2)',
        fontSize: 'var(--fxs)', color: 'var(--t2)', lineHeight: 1.6,
      }
    },
      '💡 No cloud. No API key. Your data never leaves your machine. A1 still provides full cryptographic authorization.'),

    // Provider cards
    h('div', { style: { display: 'flex', flexDirection: 'column', gap: 8, marginBottom: 4 } },
      LOCAL_PROVIDERS.map(p => h(ProviderCard, {
        key: p.id, provider: p, gwUrl,
        selected: selection?.id === p.id,
        onSelect,
      }))
    ),

    // Instructions if nothing running
    h('div', {
      style: { fontSize: 'var(--fxs)', color: 'var(--t3)', marginBottom: 4, marginTop: 6 }
    }, 'All offline? Install Ollama at ollama.com, then run: ',
      h('code', { style: { fontFamily: 'var(--mono)', color: 'var(--t2)' } }, 'ollama pull llama3 && ollama serve')),

    // Generated code
    selection && h(CodeViewer, { provider: selection, models, gwUrl }),

    // Passport reminder
    h('div', { className: 'wiz-info', style: { marginTop: 20 } },
      h('span', { style: { fontSize: 18 } }, '🛡'),
      h('div', null,
        h('div', { style: { fontWeight: 600, marginBottom: 3 } }, 'Passport required for authorization'),
        h('div', { style: { fontSize: 'var(--fxs)', color: 'var(--t2)', lineHeight: 1.6 } },
          'The code above uses a passport file from ', h('code', { style: { fontFamily: 'var(--mono)' } }, '~/.a1/passports/'), '. ',
          'Create one under '),
        h('span', {
          style: { fontSize: 'var(--fxs)', color: 'var(--accent)', cursor: 'pointer', fontWeight: 600 },
          onClick: () => window.dispatchEvent(new CustomEvent('a1-navigate', { detail: 'wizard' })),
        }, '→ Protect My Agent')
      )
    )
  );
}
