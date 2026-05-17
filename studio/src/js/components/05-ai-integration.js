// ─────────────────────────────────────────────────────────────────────────────
// AI INTEGRATION ASSISTANT — real agentic loop via Claude API
// The user provides their Claude API key (session-only, never sent to gateway).
// Claude reads the user's agent skill files, patches them with A1 guards,
// and writes them back. Every file op goes through real gateway endpoints.
// No mock. No simulation. All actual file I/O.
// ─────────────────────────────────────────────────────────────────────────────

const AI_SYSTEM_PROMPT=`You are the A1 Integration Assistant — an expert at adding cryptographic authorization to AI agent code using the A1 library (https://github.com/dyologician/a1).

Your job: help the user add A1 authorization guards to their AI agent's tool/skill functions. You do this by reading their files, patching them minimally, and writing them back.

You have three tools:
- list_agent_files: list code files in a directory (to find the right file)
- read_agent_file: read a file's content
- write_agent_file: write patched content back (always creates a .bak backup first)

INTEGRATION PATTERNS by language:

Python (@a1_guard decorator):
\`\`\`python
from a1.passport import a1_guard, PassportClient
_a1 = PassportClient(gateway_url="http://localhost:8080", passport_path="./passport.json")

@a1_guard(client=_a1, capability="CAPABILITY_NAME")
async def tool_function(param: str, signed_chain: dict, executor_pk_hex: str) -> str:
    # original body unchanged
\`\`\`

TypeScript (withA1Passport):
\`\`\`typescript
import { withA1Passport, PassportClient } from "a1/passport";
const _a1Client = new PassportClient({ gatewayUrl: "http://localhost:8080", passportPath: "./passport.json" });
const originalFn = async (param: string): Promise<string> => { /* original body */ };
export const toolFunction = withA1Passport(originalFn, { client: _a1Client, capability: "CAPABILITY_NAME" });
\`\`\`

Rust (guard_local):
\`\`\`rust
use a1::{DyoloPassport, Intent, SystemClock};
let passport = DyoloPassport::load("passport.json")?;
let intent = Intent::new("CAPABILITY_NAME")?;
let receipt = passport.guard_local(&chain, &agent_pk, &intent)?;
// original code here
\`\`\`

RULES:
1. Read the file first. Understand what framework it uses.
2. Identify every async function or exported function that is a "tool" or "skill" (something the agent calls to take action).
3. Add ONLY the guard decorator/wrapper and the necessary import. Do not refactor, rename, or reformat anything else.
4. Use the capability names from the user's passport. If they haven\'t told you, ask once.
5. After writing, briefly describe what you changed (which functions, which file).
6. If the file is too large or complex, tell the user which specific function to focus on first.
7. Never guess a file path — ask the user if you are not sure.

Be concise. Be accurate. Don't over-explain. The user wants their agent protected, not a lecture.`;

const AI_TOOLS=[
  {
    name:'list_agent_files',
    description:'List code files in a directory on the user\'s machine.',
    input_schema:{
      type:'object',
      properties:{
        path:{type:'string',description:'Directory path (e.g. ~/openclaw/skills or ./src)'}
      },
      required:['path']
    }
  },
  {
    name:'read_agent_file',
    description:'Read the content of a Python, TypeScript, Go, or Rust file.',
    input_schema:{
      type:'object',
      properties:{
        path:{type:'string',description:'Full file path or ~/relative/path'}
      },
      required:['path']
    }
  },
  {
    name:'write_agent_file',
    description:'Write patched content to a file. Always creates a .bak backup first.',
    input_schema:{
      type:'object',
      properties:{
        path:{type:'string',description:'File path to write'},
        content:{type:'string',description:'Complete new file content'},
        backup:{type:'boolean',description:'Create backup before writing (default: true)'}
      },
      required:['path','content']
    }
  }
];

async function executeTool(toolName, toolInput, gwUrl){
  const base=gwUrl||'http://localhost:8080';
  try{
    switch(toolName){
      case 'list_agent_files':{
        const r=await fetch(base+'/v1/agents/list-files?path='+encodeURIComponent(toolInput.path));
        return await r.json();
      }
      case 'read_agent_file':{
        const r=await fetch(base+'/v1/agents/read-file',{
          method:'POST',headers:{'Content-Type':'application/json'},
          body:JSON.stringify({path:toolInput.path}),
        });
        return await r.json();
      }
      case 'write_agent_file':{
        const r=await fetch(base+'/v1/agents/write-file',{
          method:'POST',headers:{'Content-Type':'application/json'},
          body:JSON.stringify({path:toolInput.path,content:toolInput.content,backup:toolInput.backup!==false}),
        });
        return await r.json();
      }
      default:
        return {error:'Unknown tool: '+toolName};
    }
  }catch(e){
    return {error:'Network error: '+e.message};
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// SNIPPET GENERATOR — zero-account, zero-API-key integration code builder
// ─────────────────────────────────────────────────────────────────────────────

const SG_LANGS = [
  { v: 'python',     l: 'Python',         e: '🐍' },
  { v: 'typescript', l: 'TypeScript/Node', e: '📘' },
  { v: 'go',         l: 'Go',             e: '🐹' },
  { v: 'rust',       l: 'Rust',           e: '⚙' },
  { v: 'rest',       l: 'REST (any lang)', e: '🌐' },
];

const SG_CAPS = [
  'files.read','files.write','web.search','web.browse','email.send','email.read',
  'database.read','database.write','trade.equity','trade.crypto','api.call',
  'social.post','shell.exec','calendar.write','payments.send','agent.delegate',
];

function buildSnippet(lang, cap, gwUrl, ppPath) {
  const gw = gwUrl || 'http://localhost:8080';
  const pp = ppPath || './passport.json';
  const c  = cap || 'files.read';
  const snippets = {
    python: `from a1.passport import a1_guard, PassportClient\n\n_a1 = PassportClient(\n    gateway_url="${gw}",\n    passport_path="${pp}",\n)\n\n@a1_guard(client=_a1, capability="${c}")\nasync def your_tool(param: str, signed_chain: dict, executor_pk_hex: str) -> str:\n    # Your tool logic here — A1 verified authorization before this runs\n    return f"Done: {param}"`,
    typescript: `import { withA1Passport, PassportClient } from "a1/passport";\n\nconst _a1 = new PassportClient({\n    gatewayUrl: "${gw}",\n    passportPath: "${pp}",\n});\n\nconst yourTool = withA1Passport(\n    async (param: string): Promise<string> => {\n        // Your tool logic here\n        return \`Done: \${param}\`;\n    },\n    { client: _a1, capability: "${c}" }\n);`,
    go: `import a1 "github.com/dyologician/a1/sdk/go/a1"\n\nfunc main() {\n    client := a1.NewClient("${gw}", nil)\n\n    guarded := a1.WithPassport(client, "${c}", yourToolFunc)\n    // guarded() verifies authorization then calls yourToolFunc\n}`,
    rust: `use a1::{DyoloPassport, Intent, SystemClock};\n\nlet passport = DyoloPassport::load("${pp}")?;\nlet intent   = Intent::new("${c}")?;\nlet receipt  = passport.guard_local(&chain, &agent_pk, &intent)?;\n\n// Your tool logic here — receipt proves authorization`,
    rest: `# Works in any language — just HTTP\ncurl -X POST ${gw}/v1/authorize \\\\\n  -H "Content-Type: application/json" \\\\\n  -d '{\n    "chain": <your-signed-chain>,\n    "intent_name": "${c}",\n    "executor_pk_hex": "<agent-pk-hex>"\n  }'`,
  };
  return snippets[lang] || snippets.rest;
}

function SnippetGenerator({ gwUrl }) {
  const [lang,    setLang]    = useState('python');
  const [cap,     setCap]     = useState('files.read');
  const [ppPath,  setPpPath]  = useState('./passport.json');
  const [copied,  setCopied]  = useState(false);
  const [open,    setOpen]    = useState(false);
  const localLLMs             = useLocalLLM();

  const snippet = buildSnippet(lang, cap, gwUrl, ppPath);

  function copy() {
    navigator.clipboard.writeText(snippet);
    setCopied(true); setTimeout(() => setCopied(false), 1800);
  }

  return h('div', { className: 'sg', style: { marginBottom: 12 } },
    h('div', { className: 'sg-head', style: { cursor: 'pointer', display: 'flex', justifyContent: 'space-between' }, onClick: () => setOpen(o => !o) },
      h('span', null, '📋 Generate code snippet (no account needed)'),
      h('span', { style: { color: 'var(--t3)', fontSize: 'var(--fxs)' } }, open ? '▲ collapse' : '▼ expand')
    ),
    open && h('div', { className: 'sg-body' },
      h(LocalLLMBanner, { localLLMs }),
      h('p', { style: { color: 'var(--t2)', fontSize: 'var(--fsm)', marginBottom: 12, lineHeight: 1.6 } },
        'Pick your language and capability. Copy the generated code into your agent — no AI, no account, no API key needed.'),

      h('div', { style: { display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 10 } },
        // Language picker
        h('div', { style: { display: 'flex', flexDirection: 'column', gap: 4, flex: '1 1 160px' } },
          h('label', { className: 'lbl' }, 'Language'),
          h('div', { style: { display: 'flex', gap: 4, flexWrap: 'wrap' } },
            SG_LANGS.map(l => h('button', { key: l.v,
              className: 'btn btn-sm',
              style: { fontWeight: lang === l.v ? 700 : 400, background: lang === l.v ? 'var(--accent)' : 'var(--b1)', color: lang === l.v ? '#fff' : 'var(--t2)', border: '1px solid var(--b3)', borderRadius: 'var(--r)', padding: '4px 10px', cursor: 'pointer', fontSize: 'var(--fxs)' },
              onClick: () => setLang(l.v),
            }, l.e + ' ' + l.l))
          )
        )
      ),

      h('div', { style: { display: 'flex', gap: 8, marginBottom: 12, flexWrap: 'wrap' } },
        // Capability picker
        h('div', { style: { flex: '1 1 180px' } },
          h('label', { className: 'lbl' }, 'Capability'),
          h('select', {
            value: cap, onChange: e => setCap(e.target.value),
            style: { width: '100%', padding: '6px 8px', border: '1px solid var(--b3)', borderRadius: 'var(--r)', background: 'var(--b1)', color: 'var(--t1)', fontSize: 'var(--fsm)', cursor: 'pointer' },
          }, SG_CAPS.map(c => h('option', { key: c, value: c }, c)))
        ),
        // Passport path
        h('div', { style: { flex: '1 1 220px' } },
          h('label', { className: 'lbl' }, 'Passport file path'),
          h('input', {
            className: 'inp inp-mono',
            value: ppPath, onChange: e => setPpPath(e.target.value),
            placeholder: './passport.json',
          })
        )
      ),

      // Generated snippet
      h('div', { style: { position: 'relative' } },
        h('pre', {
          style: { fontFamily: 'var(--mono)', fontSize: 'var(--fxs)', lineHeight: 1.8, background: 'var(--b2)', padding: '12px 14px', borderRadius: 'var(--r)', overflowX: 'auto', whiteSpace: 'pre-wrap', wordBreak: 'break-word', margin: 0, border: '1px solid var(--b3)' }
        }, snippet),
        h('button', {
          className: 'btn btn-p btn-sm',
          style: { position: 'absolute', top: 8, right: 8 },
          onClick: copy,
        }, copied ? '✓ Copied!' : 'Copy')
      ),

      // Install hint
      h('div', { style: { marginTop: 8, fontSize: 'var(--fxs)', color: 'var(--t2)' } },
        lang === 'python'     && '$ pip install a1identity',
        lang === 'typescript' && '$ npm install a1-ai',
        lang === 'go'         && '$ go get github.com/dyologician/a1/sdk/go/a1',
        lang === 'rust'       && '# Cargo.toml: a1 = { version = "2.8.0", features = ["wire"] }',
        lang === 'rest'       && 'Works with curl, fetch, axios, requests — any HTTP client.',
      ),

      h('div', { className: 'wiz-info', style: { marginTop: 10 } },
        h('span', { style: { fontSize: 16 } }, '📍'),
        h('div', null,
          h('div', { style: { fontWeight: 600, marginBottom: 2 } }, 'Next: place this code in your agent'),
          h('div', { style: { color: 'var(--t2)', lineHeight: 1.6, fontSize: 'var(--fxs)' } },
            'Wrap every tool function your agent uses with this guard. ',
            'Make sure the passport.json path matches the file created by "Protect My Agent".')
        )
      )
    )
  );
}

function AiIntegration(){
  const{settings}=useContext(Ctx);
  const gwUrl=settings.gwUrl||'http://localhost:8080';
  const[apiKey,setApiKey]=useState(()=>sessionStorage.getItem('a1_claude_key')||'');
  const[keyVisible,setKeyVisible]=useState(false);
  const[msgs,setMsgs]=useState([]);
  const[history,setHistory]=useState([]);
  const[input,setInput]=useState('');
  const[running,setRunning]=useState(false);
  const[filesWritten,setFilesWritten]=useState([]);
  const[gwAi,setGwAi]=useState(null); // null=checking, {available,model}=done
  const msgsEnd=useRef(null);

  // Check if gateway has A1_AI_KEY configured (proxy mode — no user key needed)
  useEffect(()=>{
    fetch(gwUrl+'/v1/ai/status')
      .then(r=>r.json())
      .then(d=>setGwAi(d))
      .catch(()=>setGwAi({available:false}));
  },[gwUrl]);

  useEffect(()=>{
    if(msgsEnd.current)msgsEnd.current.scrollIntoView({behavior:'smooth'});
  },[msgs,running]);

  function saveKey(k){
    setApiKey(k);
    if(k)sessionStorage.setItem('a1_claude_key',k);
    else sessionStorage.removeItem('a1_claude_key');
  }

  function addMsg(m){setMsgs(p=>[...p,m]);}

  // Determine which endpoint to use: gateway proxy or direct Anthropic
  const useProxy=gwAi?.available===true;
  const hasUserKey=apiKey.trim().startsWith('sk-ant-');
  const canSend=useProxy||hasUserKey;

  async function callAI(messages,system,tools){
    if(useProxy){
      // Route through gateway — no user key needed
      const resp=await fetch(gwUrl+'/v1/ai/chat',{
        method:'POST',
        headers:{'Content-Type':'application/json'},
        body:JSON.stringify({messages,system,tools,max_tokens:4096}),
      });
      if(!resp.ok){
        const err=await resp.json().catch(()=>({error:{message:resp.statusText}}));
        throw new Error(err.error?.message||'Gateway AI error '+resp.status);
      }
      return resp.json();
    }else{
      // Direct Anthropic call with user's key
      const resp=await fetch('https://api.anthropic.com/v1/messages',{
        method:'POST',
        headers:{
          'Content-Type':'application/json',
          'x-api-key':apiKey,
          'anthropic-version':'2023-06-01',
          'anthropic-dangerous-direct-browser-access':'true',
        },
        body:JSON.stringify({model:'claude-sonnet-4-20250514',max_tokens:4096,system,tools,messages}),
      });
      if(!resp.ok){
        const err=await resp.json().catch(()=>({error:{message:resp.statusText}}));
        throw new Error(err.error?.message||'API error '+resp.status);
      }
      return resp.json();
    }
  }

  async function claudeTurn(newHistory){
    const data=await callAI(newHistory,AI_SYSTEM_PROMPT,AI_TOOLS);
    const textBlocks=data.content.filter(b=>b.type==='text');
    const toolUseBlocks=data.content.filter(b=>b.type==='tool_use');
    if(textBlocks.length>0){
      addMsg({role:'assistant',text:textBlocks.map(b=>b.text).join('\n\n')});
    }
    if(toolUseBlocks.length===0||data.stop_reason==='end_turn'){
      return[...newHistory,{role:'assistant',content:data.content}];
    }
    const toolResults=[];
    for(const block of toolUseBlocks){
      addMsg({role:'tool-call',text:'⚙ Calling '+block.name+' → '+JSON.stringify(block.input).slice(0,120)+(JSON.stringify(block.input).length>120?'…':'')});
      const result=await executeTool(block.name,block.input,gwUrl);
      if(block.name==='write_agent_file'&&result.success){
        setFilesWritten(p=>[...p,{path:result.path,backup:result.backup_path,bytes:result.bytes_written}]);
      }
      const ok=!result.error&&result.success!==false;
      const preview=JSON.stringify(result).slice(0,240);
      addMsg({role:'tool-result',ok,text:(ok?'✓ ':'✕ ')+block.name+'\n'+preview+(preview.length<JSON.stringify(result).length?'…':'')});
      toolResults.push({type:'tool_result',tool_use_id:block.id,content:JSON.stringify(result)});
    }
    return claudeTurn([...newHistory,{role:'assistant',content:data.content},{role:'user',content:toolResults}]);
  }

  async function send(){
    const text=input.trim();
    if(!text||running)return;
    if(!canSend){
      addMsg({role:'sys',text:'Enter your Claude API key above first (or ask your admin to set A1_AI_KEY on the gateway).'});
      return;
    }
    setInput('');setRunning(true);
    addMsg({role:'user',text});
    const newHistory=[...history,{role:'user',content:text}];
    try{
      const finalHistory=await claudeTurn(newHistory);
      setHistory(finalHistory);
    }catch(e){
      addMsg({role:'sys',text:'Error: '+e.message+'\n\nCheck your connection and try again.'});
    }
    setRunning(false);
  }

  function startFresh(){setMsgs([]);setHistory([]);setFilesWritten([]);setInput('');}

  const aiStatus=useProxy
    ?{icon:'🟢',label:'Gateway AI active — no API key needed',color:'var(--green)'}
    :hasUserKey
      ?{icon:'🔑',label:'Using your Claude API key',color:'var(--t2)'}
      :{icon:'🔴',label:'No AI available — use the snippet generator above, or add a key below',color:'#ef4444'};

  return h('div',{style:{paddingBottom:40,width:'100%'}},

    h('h2',{style:{fontSize:18,fontWeight:700,marginBottom:4}},'🤝 AI Integration Assistant'),
    h('p',{style:{color:'var(--t2)',fontSize:'var(--fsm)',lineHeight:1.6,marginBottom:14}},
      'Claude reads your agent\'s skill files and adds A1 guards automatically. ',
      h('strong',null,'Your API key goes directly to Anthropic — never to A1 or any other server.')),

    // Snippet generator — always available, no API key needed
    h(SnippetGenerator, { gwUrl }),

    // Divider
    h('div',{style:{display:'flex',alignItems:'center',gap:10,margin:'4px 0 12px'}},
      h('div',{style:{flex:1,height:1,background:'var(--b3)'}}),
      h('span',{style:{fontSize:'var(--fxs)',color:'var(--t3)',whiteSpace:'nowrap'}},'or use AI to patch your files automatically'),
      h('div',{style:{flex:1,height:1,background:'var(--b3)'}})),

    // AI status indicator
    gwAi!==null&&h('div',{style:{display:'flex',alignItems:'center',gap:8,marginBottom:12,padding:'8px 12px',border:'1px solid var(--b3)',borderRadius:'var(--r)',background:'var(--b1)'}},
      h('span',{style:{fontSize:16}},'●'),
      h('span',{style:{fontSize:'var(--fxs)',color:aiStatus.color,fontWeight:600}},aiStatus.label),
      useProxy&&h('span',{style:{fontSize:'var(--fxs)',color:'var(--t3)',marginLeft:'auto'}},gwAi.model)),

    // API key row — only shown when gateway proxy is not available
    !useProxy&&h('div',{className:'ai-key-row'},
      h('span',{style:{fontSize:'var(--fxs)',color:'var(--t2)',whiteSpace:'nowrap',fontFamily:'var(--mono)'}},'Claude API key'),
      h('input',{
        className:'ai-key-inp',
        type:keyVisible?'text':'password',
        placeholder:'sk-ant-api03-…',
        value:apiKey,
        onChange:e=>saveKey(e.target.value),
        autoComplete:'off',
      }),
      h('button',{className:'btn btn-s btn-sm',onClick:()=>setKeyVisible(p=>!p)},keyVisible?'Hide':'Show'),
      hasUserKey
        ?h('span',{style:{color:'var(--green)',fontSize:'var(--fxs)',fontFamily:'var(--mono)',whiteSpace:'nowrap'}},'✓ Key set')
        :h('a',{href:'https://console.anthropic.com/settings/keys',target:'_blank',rel:'noopener',
            className:'btn btn-s btn-sm',style:{whiteSpace:'nowrap'}},'Get key ↗')),

    // Key notice — only when neither proxy nor user key available
    !useProxy&&!hasUserKey&&h('div',{className:'wiz-info',style:{marginBottom:12}},
      h('span',{style:{fontSize:16}},'🔑'),
      h('div',null,
        h('div',{style:{fontWeight:600,marginBottom:2}},'Two ways to get AI integration:'),
        h('div',{style:{color:'var(--t2)',lineHeight:1.7,fontSize:'var(--fxs)'}},
          h('strong',null,'Option A (self):'),' Get a free Claude API key at ',
          h('a',{href:'https://console.anthropic.com',target:'_blank',rel:'noopener',style:{color:'var(--green)'}},'console.anthropic.com'),
          ' and paste it above.',h('br'),
          h('strong',null,'Option B (admin):'),' Ask your A1 gateway admin to set ',
          h('code',{style:{fontFamily:'var(--mono)',background:'var(--b1)',padding:'1px 5px',borderRadius:3}},'A1_AI_KEY'),
          ' on the gateway — then no one needs their own key.'))),

    // Starter prompts (shown when AI is available but chat is empty)
    history.length===0&&canSend&&h('div',{className:'sg',style:{marginBottom:12}},
      h('div',{className:'sg-head'},'Quick start'),
      h('div',{className:'sg-body'},
        h('p',{style:{color:'var(--t2)',fontSize:'var(--fsm)',marginBottom:8}},'Tell the assistant what you want to protect. Examples:'),
        h('div',{style:{display:'flex',flexDirection:'column',gap:5}},
          [
            'My OpenClaw skills are in ~/openclaw/skills/. Add A1 guards to all of them with capability "files.read".',
            'I have a Python agent at ~/myagent/tools.py that sends emails. Add @a1_guard with capability "email.send".',
            'My IronClaw plugin file is at ~/.ironclaw/plugins/browser.py. Wrap the browse() function with A1.',
            'Show me what files are in ~/openclaw/src/ so I can pick which one to protect.',
          ].map(s=>h('div',{key:s,
            style:{padding:'7px 11px',background:'var(--s3)',borderRadius:'var(--r)',fontSize:'var(--fsm)',cursor:'pointer',color:'var(--t2)',border:'1px solid var(--b1)',transition:'all .15s'},
            onClick:()=>setInput(s),
            onMouseEnter:e=>{e.currentTarget.style.borderColor='var(--green)';e.currentTarget.style.color='var(--text)';},
            onMouseLeave:e=>{e.currentTarget.style.borderColor='var(--b1)';e.currentTarget.style.color='var(--t2)';},
          },s))
        ))),

    // Chat window
    (msgs.length>0||canSend)&&h('div',{className:'ai-chat'},
      h('div',{className:'ai-msgs'},
        msgs.length===0&&h('div',{className:'ai-msg sys'},
          useProxy?'Gateway AI is ready. Type below to start.':'Type below to start. Claude will read your agent files and add A1 guards.'),
        msgs.map((m,i)=>h('div',{key:i,
          className:'ai-msg '+(m.role==='tool-result'?(m.ok?'tool-result':'tool-result err'):m.role)},
          m.text)),
        running&&h('div',{className:'ai-typing'},h('span'),h('span'),h('span')),
        h('div',{ref:msgsEnd})),
      h('div',{className:'ai-input-row'},
        h('textarea',{className:'ai-inp',rows:1,
          placeholder:canSend?'Describe what you want to protect…':'Enter your API key above or use the snippet generator',
          value:input,
          disabled:!canSend||running,
          onChange:e=>{setInput(e.target.value);e.target.style.height='auto';e.target.style.height=Math.min(e.target.scrollHeight,100)+'px';},
          onKeyDown:e=>{if(e.key==='Enter'&&!e.shiftKey){e.preventDefault();send();}}}),
        h('button',{className:'btn btn-p btn-sm',onClick:send,disabled:!canSend||running||!input.trim()},
          running?'…':'Send'))),

    // Files written summary
    filesWritten.length>0&&h('div',{className:'sg',style:{marginTop:12}},
      h('div',{className:'sg-head'},'✓ Files patched ('+filesWritten.length+')'),
      h('div',{className:'sg-body'},
        filesWritten.map((f,i)=>h('div',{key:i,style:{marginBottom:6}},
          h('div',{className:'ai-file-chip'},f.path),
          f.backup&&h('span',{style:{fontSize:'var(--fxs)',color:'var(--t2)',marginLeft:6}},'backup: '+f.backup),
          h('span',{style:{fontSize:'var(--fxs)',color:'var(--t2)',marginLeft:6}},f.bytes+' bytes'))),
        h('div',{className:'wiz-info gr',style:{marginTop:8}},
          h('span',{style:{fontSize:16}},'🔄'),
          h('div',null,
            h('div',{style:{fontWeight:600,marginBottom:2}},'Restart your agent'),
            h('div',{style:{color:'var(--t2)',lineHeight:1.6,fontSize:'var(--fxs)'}},
              'Restart OpenClaw, IronClaw, or your agent process to pick up the patched files. A1 is now enforcing authorization on the patched functions.'))))),

    // Controls
    (msgs.length>0)&&h('div',{style:{marginTop:10,display:'flex',gap:8}},
      h('button',{className:'btn btn-s btn-sm',onClick:startFresh},'Start over'),
      h('button',{className:'btn btn-s btn-sm',onClick:()=>setFilesWritten([])},'Clear file list')),

    // How it works callout
    history.length===0&&h('div',{className:'wiz-info',style:{marginTop:16}},
      h('span',{style:{fontSize:18}},'🔒'),
      h('div',null,
        h('div',{style:{fontWeight:600,marginBottom:3}},'What happens'),
        h('div',{style:{color:'var(--t2)',lineHeight:1.7,fontSize:'var(--fxs)'}},
          '1. You tell Claude where your agent\'s skill/tool files are.',h('br'),
          '2. Claude reads the file through the A1 gateway (runs on your machine).',h('br'),
          '3. Claude adds ',h('code',{style:{fontFamily:'var(--mono)',background:'var(--b1)',padding:'1px 4px',borderRadius:2}},'@a1_guard'),' (Python) or ',h('code',{style:{fontFamily:'var(--mono)',background:'var(--b1)',padding:'1px 4px',borderRadius:2}},'withA1Passport'),' (TypeScript) to each tool function.',h('br'),
          '4. Claude writes the patched file back. A backup is created automatically.',h('br'),
          '5. You restart your agent — A1 is now protecting every patched tool.',h('br',''),
          h('strong',null,'Your API key goes directly to Anthropic. The A1 gateway only handles file reads/writes on your local machine.'))))
  );
}


