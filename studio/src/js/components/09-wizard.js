// ─────────────────────────────────────────────────────────────────────────────
// PROTECT MY AGENT — Non-tech wizard
// ─────────────────────────────────────────────────────────────────────────────

// ─── ConceptTip: inline hoverable mental-model explainer ────────────────────
const CONCEPTS={
  'passport':    {title:'What is a Passport?',    body:'A cryptographic ID card for your AI agent. It lists exactly what the agent is allowed to do and for how long. Think of it as a signed permission slip from you to your agent.'},
  'capability':  {title:'What is a Capability?',  body:'A named permission — like "email.send" or "files.read". Your agent can only do things listed in its passport. If it tries anything else, A1 blocks it immediately.'},
  'chain':       {title:'What is a Chain?',        body:'A chain of signed certificates proving that the human → orchestrator → executor path is all authorized. A1 verifies the whole chain before allowing any action.'},
  'namespace':   {title:'What is a Namespace?',    body:'A unique name for your agent (like "trading-bot" or "email-helper"). It\'s the agent\'s identity — like a username. Used to track and revoke access.'},
  'TTL':         {title:'What is TTL?',             body:'Time To Live — how long the passport is valid. After this time, the agent stops working until you renew. "30d" = 30 days. Choose longer for personal use, shorter for sensitive tasks.'},
  'receipt':     {title:'What is a ProvableReceipt?',body:'A cryptographic proof that A1 verified the authorization before the action ran. Think of it as a tamper-proof audit log entry — it proves who authorized what and when.'},
};

function ConceptTip({term,concept}){
  const[show,setShow]=useState(false);
  const c=CONCEPTS[concept||term]||{title:term,body:''};
  return h('span',{className:'ctip'},
    h('span',{className:'ctip-trigger',
      onMouseEnter:()=>setShow(true),
      onMouseLeave:()=>setShow(false)},term),
    show&&h('div',{className:'ctip-box'},
      h('div',{className:'ctip-box-title'},c.title),
      h('div',null,c.body)));
}

// ─── Capability keyword suggester ────────────────────────────────────────────
const CAP_KEYWORDS={
  email:['email.send','email.read'],mail:['email.send','email.read'],
  gmail:['email.send','email.read'],smtp:['email.send'],
  file:['files.read','files.write'],document:['files.read','files.write'],
  read:['files.read'],write:['files.write'],
  trade:['trade.equity','trade.crypto'],stock:['trade.equity'],
  equity:['trade.equity'],crypto:['trade.crypto'],bitcoin:['trade.crypto'],
  polymarket:['trade.polymarket','portfolio.read'],bet:['trade.polymarket'],predict:['trade.polymarket'],
  portfolio:['portfolio.read'],balance:['portfolio.read'],wallet:['portfolio.read'],
  search:['web.search'],browse:['web.browse','web.search'],
  web:['web.search','web.browse'],internet:['web.search'],google:['web.search'],
  twitter:['social.post'],tweet:['social.post'],'x.com':['social.post'],
  linkedin:['social.post'],social:['social.post'],post:['social.post'],
  code:['code.execute','shell.exec'],run:['code.execute'],script:['code.execute','shell.exec'],
  shell:['shell.exec'],terminal:['shell.exec'],command:['shell.exec'],
  database:['database.read','database.write'],sql:['database.read','database.write'],
  db:['database.read','database.write'],
  api:['api.call'],'rest':['api.call'],http:['api.call'],
  calendar:['calendar.write'],schedule:['calendar.write'],meeting:['calendar.write'],
  payment:['payments.send'],pay:['payments.send'],stripe:['payments.send'],
  memory:['memory.write'],remember:['memory.write'],store:['memory.write'],
  screenshot:['browser.screenshot'],browser:['web.browse','browser.screenshot'],
  compute:['compute.run'],cloud:['compute.run'],job:['compute.run'],
  delegate:['agent.delegate'],agent:['agent.delegate'],
};

function suggestCaps(text){
  const words=text.toLowerCase().split(/[\s,\.;]+/).filter(Boolean);
  const found=new Set();
  words.forEach(w=>{
    Object.entries(CAP_KEYWORDS).forEach(([kw,caps])=>{
      if(w.includes(kw)||kw.includes(w))caps.forEach(c=>found.add(c));
    });
  });
  return [...found].slice(0,8);
}

const W_CAPS=[
  {k:'files.read',        e:'📁',l:'Read Files',         d:'Access documents and files'},
  {k:'files.write',       e:'✏️',l:'Write Files',         d:'Create and edit files'},
  {k:'code.execute',      e:'💻',l:'Run Code',             d:'Execute scripts and programs'},
  {k:'web.search',        e:'🌐',l:'Search the Web',       d:'Look things up online'},
  {k:'web.browse',        e:'🖥',l:'Browse Pages',         d:'Visit and interact with websites'},
  {k:'email.send',        e:'📧',l:'Send Emails',          d:'Compose and send emails'},
  {k:'email.read',        e:'📬',l:'Read Emails',          d:'Read and process inbox'},
  {k:'database.read',     e:'📊',l:'Read Data',            d:'Query databases and spreadsheets'},
  {k:'database.write',    e:'💾',l:'Write Data',           d:'Save and modify records'},
  {k:'trade.equity',      e:'📈',l:'Trade Stocks',         d:'Place equity buy/sell orders'},
  {k:'trade.crypto',      e:'₿', l:'Trade Crypto',         d:'Buy/sell crypto assets'},
  {k:'trade.polymarket',  e:'🎲',l:'Polymarket',           d:'Place prediction market positions'},
  {k:'portfolio.read',    e:'💼',l:'View Portfolio',       d:'Check balances and holdings'},
  {k:'api.call',          e:'🔌',l:'Call External APIs',   d:'Connect to third-party services'},
  {k:'social.post',       e:'📢',l:'Post to Social Media', d:'Publish to Twitter/X, LinkedIn etc'},
  {k:'calendar.write',    e:'📅',l:'Manage Calendar',      d:'Create and edit calendar events'},
  {k:'shell.exec',        e:'⌨️',l:'Shell Commands',        d:'Run terminal commands'},
  {k:'payments.send',     e:'💳',l:'Send Payments',        d:'Initiate payment transactions'},
  {k:'agent.delegate',    e:'🤖',l:'Delegate to Agents',   d:'Hand off tasks to sub-agents'},
  {k:'memory.write',      e:'🗄️',l:'Manage Memory',        d:'Store and recall information'},
  {k:'browser.screenshot',e:'📷',l:'Take Screenshots',     d:'Capture browser state'},
  {k:'compute.run',       e:'⚡',l:'Run Compute Jobs',     d:'Provision and run cloud compute'},
];
const W_DURS=[
  {v:'1d', l:'1 day'},
  {v:'7d', l:'1 week'},
  {v:'30d',l:'30 days (recommended)'},
  {v:'90d',l:'3 months'},
  {v:'1y', l:'1 year'},
];
const W_AGENTS=[
  {v:'claude-code',l:'Claude Code',       e:'🤖'},
  {v:'openai',     l:'OpenAI Agents',     e:'🟢'},
  {v:'langchain',  l:'LangChain',         e:'🦜'},
  {v:'langgraph',  l:'LangGraph',         e:'🔗'},
  {v:'crewai',     l:'CrewAI',           e:'⛵'},
  {v:'autogen',    l:'AutoGen',           e:'🔄'},
  {v:'llamaindex', l:'LlamaIndex',        e:'🦙'},
  {v:'typescript', l:'TypeScript/Node',   e:'📘'},
  {v:'go',         l:'Go',               e:'🐹'},
  {v:'rest',       l:'Any (REST API)',    e:'🌐'},
  {v:'ironclaw',   l:'IronClaw / Custom', e:'⚙️'},
  {v:'other',      l:'Ask Claude Code',   e:'✨'},
];

function wizCliCmd(name,caps,ttl){
  const ns=(name||'my-agent').toLowerCase().replace(/\s+/g,'-').replace(/[^a-z0-9-]/g,'');
  return 'a1 passport issue \\\n  --namespace '+ns+' \\\n  --allow "'+( caps.length?caps.join(','):'files.read' )+'" \\\n  --ttl '+ttl+' \\\n  --out passport.json';
}

function wizCode(av,name,caps,ttl){
  const ns=(name||'my-agent').toLowerCase().replace(/\s+/g,'-').replace(/[^a-z0-9-]/g,'');
  const cap1=caps[0]||'files.read';
  const capStr=caps.join(', ')||'files.read';
  const T={
    'claude-code':'# Paste this prompt into Claude Code:\n\n"""\nI have an A1 passport at ./passport.json\nAgent namespace: '+ns+'\nAllowed capabilities: '+capStr+'\nA1 gateway: http://localhost:8080\n\nPlease:\n1. Run: pip install a1identity\n2. Add @a1_guard(client=PassportClient("http://localhost:8080"), capability="'+cap1+'") to my tool functions\n3. Show me the full integration\n"""\n\n# Claude Code will write everything for you!',
    'openai':'from a1.passport import a1_guard, PassportClient\n\nclient = PassportClient(\n    gateway_url="http://localhost:8080",\n    passport_path="./passport.json",\n)\n\n@a1_guard(client=client, capability="'+cap1+'")\nasync def my_tool(query: str, signed_chain: dict, executor_pk_hex: str) -> str:\n    # Your tool logic here — a1_guard verifies authorization first\n    return f"Done: {query}"',
    'langchain':'from a1.langchain_tool import A1AuthorizationTool\nfrom a1.passport import PassportClient\n\nclient = PassportClient(\n    gateway_url="http://localhost:8080",\n    passport_path="./passport.json",\n)\n\ntool = A1AuthorizationTool(\n    name="my_tool",\n    description="My A1-protected tool.",\n    intent_name="'+cap1+'",\n    client=client,\n    func=my_tool_fn,\n    chain=agent_chain,\n    executor_pk_hex=agent_pk,\n)',
    'langgraph':'from a1.langgraph_tool import a1_node\nfrom a1.passport import PassportClient\n\nclient = PassportClient(\n    gateway_url="http://localhost:8080",\n    passport_path="./passport.json",\n)\n\n@a1_node(intent_name="'+cap1+'", client=client, propagate_receipt=True)\nasync def my_node(state: dict) -> dict:\n    # Node logic here — authorization verified before execution\n    return state',
    'crewai':'from a1.crewai_tool import A1AuthorizationTool\n\ntool = A1AuthorizationTool(\n    func=my_tool_fn,\n    intent_name="'+cap1+'",\n    gateway_url="http://localhost:8080",\n    chain=agent_chain,\n    executor_pk_hex=agent_pk,\n)',
    'autogen':'from a1.autogen_tool import build_a1_function_tool\nfrom a1.passport import PassportClient\n\nclient = PassportClient(\n    gateway_url="http://localhost:8080",\n    passport_path="./passport.json",\n)\n\ntool = build_a1_function_tool(\n    fn=my_tool_fn,\n    intent_name="'+cap1+'",\n    client=client,\n    chain=agent_chain,\n    executor_pk_hex=agent_pk,\n)',
    'llamaindex':'from a1.llamaindex_tool import a1_llamaindex_tool\nfrom a1.passport import PassportClient\n\nclient = PassportClient(\n    gateway_url="http://localhost:8080",\n    passport_path="./passport.json",\n)\n\ntool = a1_llamaindex_tool(\n    fn=my_tool_fn,\n    intent_name="'+cap1+'",\n    client=client,\n    name="my_tool",\n    description="My protected tool.",\n)',
    'typescript':'import { withA1Passport, PassportClient } from "a1/passport";\n\nconst client = new PassportClient({\n    gatewayUrl: "http://localhost:8080",\n    passportPath: "./passport.json",\n});\n\nconst guardedTool = withA1Passport(myTool, {\n    client,\n    capability: "'+cap1+'",\n});\n\n// guardedTool verifies authorization before executing myTool',
    'go':'import a1 "github.com/dyologician/a1/sdk/go/a1"\n\nclient := a1.NewClient("http://localhost:8080", nil)\n\nguarded := a1.WithPassport(client, "'+cap1+'", myToolFunc)\n// guarded() verifies authorization then calls myToolFunc',
    'rest':'# Works in any language — just HTTP\ncurl -X POST http://localhost:8080/v1/authorize \\\\\n  -H "Content-Type: application/json" \\\\\n  -d \'{\n    "chain": <your-signed-chain>,\n    "intent_name": "'+cap1+'",\n    "executor_pk_hex": "<agent-pk-hex>"\n  }\'',
    'ironclaw':'# Add to your IronClaw / custom agent config:\n# gateway: http://localhost:8080\n# passport: ./passport.json\n# capabilities: '+capStr+'\n\n# Then use the REST API for authorization:\n# POST http://localhost:8080/v1/authorize\n# See: https://github.com/dyologician/a1 for examples',
    'other':'# Tell your AI assistant to integrate A1:\n\n"""\nAdd A1 authorization to my agent.\nPassport file: ./passport.json\nGateway: http://localhost:8080\nCapabilities: '+capStr+'\n\nInstall: pip install a1identity\nDocs: https://github.com/dyologician/a1\n"""',
  };
  return T[av]||T['other'];
}

function ProtectAgent({ prefill, onPrefillConsumed }){
  const{api}=useContext(Ctx);
  const[step,setStep]=useState(1);
  const[name,setName]=useState('');
  const[caps,setCaps]=useState(['files.read','web.search']);
  const[custom,setCustom]=useState('');
  const[ttl,setTtl]=useState('30d');
  const[agentType,setAgentType]=useState('claude-code');
  const[c1,setC1]=useState(false);
  const[c2,setC2]=useState(false);
  const[issuingPassport,setIssuingPassport]=useState(false);
  const[issueResult,setIssueResult]=useState(()=>{
    try{const s=sessionStorage.getItem('a1_wizard_issue');return s?JSON.parse(s):null;}catch{return null;}
  });
  const[conn,setConn]=useState(null);
  const[testing,setTesting]=useState(false);
  const[livePassport,setLivePassport]=useState(null);
  // Track prefill so we can show a contextual banner and not lose the data
  const[prefillData,setPrefillData]=useState(null); // stores the prefill once received

  // Auto-test gateway connection whenever we land on step 2.
  useEffect(()=>{
    if(step!==2||conn)return;
    (async()=>{
      const r=await api('GET','/health');
      setConn(r.ok?'ok':'err');
    })();
  },[step]);

  // When entering step 4, fetch the actual passport record from the gateway.
  useEffect(()=>{
    if(step!==4)return;
    (async()=>{
      const ns=issueResult?.namespace||(name||'my-agent').toLowerCase().replace(/\s+/g,'-').replace(/[^a-z0-9-]/g,'');
      // Use list endpoint and find by namespace (read endpoint requires path, not namespace)
      const r=await api('GET','/v1/passports/list');
      if(r.ok&&r.data&&Array.isArray(r.data.passports)){
        const found=r.data.passports.find(p=>(p.namespace||'').toLowerCase()===ns.toLowerCase());
        if(found)setLivePassport(found);
      }
    })();
  },[step]);

  // Persist issueResult so it survives tab-switching and soft reloads.
  useEffect(()=>{
    try{
      if(issueResult&&issueResult.success)sessionStorage.setItem('a1_wizard_issue',JSON.stringify(issueResult));
    }catch{}
  },[issueResult]);

  // Apply prefill from "Protect this agent" shortcut in Connect Agents.
  // Store a copy in prefillData so we can show a contextual banner.
  useEffect(() => {
    if (!prefill) return;
    setPrefillData(prefill); // capture for banner
    if (prefill.name) setName(prefill.name);
    if (prefill.caps && prefill.caps.length > 0) setCaps(prefill.caps);
    if (prefill.agentType) setAgentType(prefill.agentType);
    setStep(1); // always start at step 1 so user sees the guided flow
    if (typeof onPrefillConsumed === 'function') onPrefillConsumed();
  }, [prefill]);

  function toggleCap(k){setCaps(p=>p.includes(k)?p.filter(x=>x!==k):[...p,k]);}
  function addCustom(){
    const c=custom.trim().toLowerCase().replace(/\s+/g,'.');
    if(c&&!caps.includes(c))setCaps(p=>[...p,c]);
    setCustom('');
  }
  const cmd=wizCliCmd(name,caps,ttl);
  const code=wizCode(agentType,name,caps,ttl);
  function copy1(){navigator.clipboard.writeText(cmd).then(()=>{setC1(true);setTimeout(()=>setC1(false),2000);});}
  function copy2(){navigator.clipboard.writeText(code).then(()=>{setC2(true);setTimeout(()=>setC2(false),2000);});}
  async function testGw(){
    setTesting(true);setConn(null);
    const r=await api('GET','/health');
    setConn(r.ok?'ok':'err');setTesting(false);
  }
  const customCaps=caps.filter(c=>!W_CAPS.find(wc=>wc.k===c));

  return h('div',{style:{paddingBottom:40,width:'100%'}},

    // Progress bar
    h('div',{className:'wiz-prog'},
      [1,2,3,4].map(s=>h('div',{key:s,className:'wiz-prog-s'+(s<=step?' on':'')}))),

    // ── STEP 1: Configure ──────────────────────────────────────
    step===1&&h('div',null,
      h('h2',{style:{fontSize:18,fontWeight:700,marginBottom:4}},'🛡️ Protect your AI agent'),

      // ── Guided banner when user came from "Protect this agent" in Connect Agents ──
      prefillData&&h('div',{className:'wiz-info gr',style:{marginBottom:16,borderColor:'rgba(99,102,241,.3)',background:'rgba(99,102,241,.06)'}},
        h('span',{style:{fontSize:22}},'👋'),
        h('div',{style:{flex:1}},
          h('div',{style:{fontWeight:700,fontSize:'var(--fsm)',marginBottom:3,color:'var(--accent)'}},
            'Nice! Let\'s protect '+(prefillData.name || 'your agent')+'.'),
          h('div',{style:{color:'var(--t2)',fontSize:'var(--fxs)',lineHeight:1.6}},
            'We\'ve detected ',h('strong',null,prefillData.name||'your agent'),' from Connect Agents and pre-filled the name below. ',
            h('strong',null,'Give it a name'),' (if not already filled), ',
            h('strong',null,'choose what it\'s allowed to do'),', and click Generate — that\'s it.'))),

      h(NudgeTip, { tipKey: 'one_passport_per_agent' }),
      h(NudgeTip, { tipKey: 'passport_safety' }),
      h('p',{style:{color:'var(--t2)',fontSize:'var(--fsm)',marginBottom:24,lineHeight:1.6}},
        'Creates a cryptographic ',h(ConceptTip,{term:'passport',concept:'passport'}),
        ' for your agent — defining its ',h(ConceptTip,{term:'capabilities',concept:'capability'}),
        '. No account. Runs 100% locally.'),

      h('div',{className:'sg'},
        h('div',{className:'sg-head'},'1. Name your agent ',h('span',{style:{color:'#ef4444',fontWeight:700}},'*required')),
        h('div',{className:'sg-body'},
          h('div',{className:'field'},
            h('label',{className:'lbl'},'Agent name'),
            h('input',{className:'inp',type:'text',
              placeholder:'e.g. Trading Bot, Research Agent, My Assistant',
              value:name,onChange:e=>setName(e.target.value),
              style:{fontSize:'var(--fbase)',borderColor:(!name.trim()&&caps.length>0)?'rgba(239,68,68,.5)':''},
              autoFocus: !!prefillData,
            }),
            !name.trim()&&h('p',{style:{fontSize:'var(--fxs)',color:'#ef4444',marginTop:4}},
              '⚠ A name is required — it becomes the agent\'s cryptographic identity (namespace).'),
            name.trim()&&h('p',{style:{fontSize:'var(--fxs)',color:'var(--t2)',marginTop:4,fontFamily:'var(--mono)'}},
              'Namespace: ',h('strong',null,name.trim().toLowerCase().replace(/\s+/g,'-').replace(/[^a-z0-9-]/g,'')))))),
      h('div',{className:'sg',style:{marginTop:12}},
        h('div',{className:'sg-head'},'2. What should it be allowed to do?'),
        h('div',{className:'sg-body'},
          h('div',{className:'field',style:{marginBottom:10}},
            h('label',{className:'lbl'},'Describe what your agent does (optional — we\'ll suggest capabilities)'),
            h('div',{style:{display:'flex',gap:6}},
              h('input',{className:'inp',id:'cap-desc-inp',type:'text',
                placeholder:'e.g. "sends emails and searches the web for research"',
                style:{fontSize:'var(--fsm)'},
                onKeyDown:e=>{
                  if(e.key!=='Enter')return;
                  const s=suggestCaps(e.target.value);
                  s.forEach(c=>{if(!caps.includes(c))setCaps(p=>[...p,c]);});
                }}),
              h('button',{className:'btn btn-s btn-sm',
                onClick:()=>{
                  const inp=document.getElementById('cap-desc-inp');
                  if(!inp)return;
                  const s=suggestCaps(inp.value);
                  s.forEach(c=>{if(!caps.includes(c))setCaps(p=>[...p,c]);});
                }},'Suggest')),
            h('p',{style:{color:'var(--t2)',fontSize:'var(--fxs)',marginTop:3}},
              'Or just check the boxes below manually:')),
          h('div',{className:'wiz-grid2'},
            W_CAPS.map(c=>h('label',{key:c.k,className:'wiz-cap'+(caps.includes(c.k)?' on':'')},
              h('input',{type:'checkbox',checked:caps.includes(c.k),onChange:()=>toggleCap(c.k)}),
              h('div',null,
                h('div',{className:'wiz-cap-lbl'},c.e+' '+c.l),
                h('div',{className:'wiz-cap-desc'},c.d))))),
          h('div',{style:{display:'flex',gap:6,marginTop:4}},
            h('input',{className:'inp inp-mono',style:{flex:1,fontSize:'var(--fsm)'},
              placeholder:'+ custom capability: e.g. payments.send',
              value:custom,onChange:e=>setCustom(e.target.value),
              onKeyDown:e=>e.key==='Enter'&&addCustom()}),
            h('button',{className:'btn btn-s btn-sm',onClick:addCustom},'Add')),
          customCaps.length>0&&h('div',{className:'wiz-tags'},
            customCaps.map(c=>h('span',{key:c,className:'wiz-tag'},
              c,h('button',{className:'wiz-tx',onClick:()=>setCaps(p=>p.filter(x=>x!==c))},'✕')))))),

      h('div',{className:'sg',style:{marginTop:12}},
        h('div',{className:'sg-head'},'3. How long should this be valid? ',h('span',{style:{fontWeight:400,color:'var(--t2)'}},h(ConceptTip,{term:'(what is TTL?)',concept:'TTL'}))),
        h('div',{className:'sg-body'},
          h('div',{style:{display:'flex',gap:7,flexWrap:'wrap'}},
            W_DURS.map(d=>h('button',{key:d.v,
              className:'wiz-dpill'+(ttl===d.v?' on':''),
              onClick:()=>setTtl(d.v)},d.l))))),

      h('div',{style:{marginTop:20,display:'flex',gap:8,alignItems:'center',flexWrap:'wrap'}},
        h('button',{className:'btn btn-p',
          onClick:()=>{if(name.trim()&&caps.length>0)setStep(2);},
          disabled:!name.trim()||caps.length===0,
          style:{fontSize:'var(--fbase)',padding:'10px 22px'}},
          'Continue to create passport →'),
        !name.trim()&&h('span',{style:{color:'#ef4444',fontSize:'var(--fsm)'}},'⚠ Enter an agent name to continue'),
        name.trim()&&caps.length===0&&h('span',{style:{color:'var(--t2)',fontSize:'var(--fsm)'}},'Select at least one capability'))),

    // ── STEP 2: Create passport (no terminal needed) ────────────────────────
    step===2&&h('div',null,
      h('h2',{style:{fontSize:18,fontWeight:700,marginBottom:4}},'📋 Create your passport'),
      h('p',{style:{color:'var(--t2)',fontSize:'var(--fsm)',marginBottom:20}},'A1 creates the passport file for you. No terminal. No CLI.'),

      h('div',{className:'sg'},
        h('div',{className:'sg-head'},'1. Check A1 is running'),
        h('div',{className:'sg-body'},
          h('div',{style:{display:'flex',gap:8,alignItems:'center'}},
            h('button',{className:'btn btn-s btn-sm',onClick:testGw,disabled:testing},testing?'Checking…':'▶ Test'),
            conn==='ok'&&h('span',{style:{color:'var(--green)',fontSize:'var(--fsm)',fontFamily:'var(--mono)'}},'✓ A1 is running'),
            conn==='err'&&h('span',{style:{color:'var(--red)',fontSize:'var(--fsm)'}},'Not running — run ',
              h('code',{style:{fontFamily:'var(--mono)',background:'var(--b1)',padding:'1px 5px',borderRadius:3}},'a1 start'),
              ' first')))),

      h('div',{className:'sg',style:{marginTop:12}},
        h('div',{className:'sg-head'},'2. Create your passport'),
        h('div',{className:'sg-body'},
          !name&&h('div',{style:{marginBottom:8,padding:'6px 10px',background:'rgba(251,191,36,.08)',border:'1px solid rgba(251,191,36,.3)',borderRadius:'var(--r)',fontSize:'var(--fxs)',color:'#b45309'}},
            '⚠ You didn\'t enter a name on step 1. Your passport will be named ',
            h('code',null,'my-agent'),'. You can go back and add a name.'),
          h('p',{style:{fontSize:'var(--fsm)',color:'var(--t2)',marginBottom:10}},
            (name?h('strong',null,name):h('span',null,'my-agent')),
            ' · '+(caps.length)+' capabilities'+(caps.length?' ('+caps.slice(0,3).join(', ')+(caps.length>3?'…':'')+')':''),
            ' · '+ttl),
          issueResult&&issueResult.success&&h('div',null,
            h('div',{className:'wiz-info gr',style:{marginBottom:8}},
              h('span',{style:{fontSize:20}},'✅'),
              h('div',{style:{flex:1}},
                h('div',{style:{fontWeight:600,marginBottom:4}},'Passport created!'),
                h('div',{style:{fontFamily:'var(--mono)',fontSize:'var(--fxs)',color:'var(--t2)',lineHeight:2}},
                  h('div',null,'📁 ',h('strong',null,issueResult.path||'~/.a1/passports/')),
                  h('div',null,'🔑 '+(issueResult.public_key_hex||'').slice(0,24)+'…',
                    h('span',{style:{color:'var(--t2)'}},' (public key — safe to share)')),
                  h('div',null,'⏰ Valid for: '+ttl)))),
            h('div',{className:'git-warn'},
              h('span',{style:{fontSize:18}},'🔒'),
              h('div',{style:{flex:1}},
                h('div',{className:'git-warn-title'},'Keep passport.json safe'),
                h('div',{style:{color:'var(--t2)',fontSize:'var(--fxs)',lineHeight:1.6,marginBottom:6}},
                  'This file contains your agent\'s private key. ',
                  h('strong',null,'Do not commit it to Git.'),
                  ' Do not share it. Back it up somewhere safe.'),
                h('div',{style:{display:'flex',gap:6}},
                  h('button',{className:'btn btn-s btn-sm',
                    onClick:async()=>{
                      const path=issueResult.path||'';
                      const dir=path.substring(0,path.lastIndexOf('/'));
                      if(!dir)return;
                      // Check if in git repo and add to .gitignore
                      const r=await fetch((window.A1_GW_URL||'http://localhost:8080')+'/v1/system/gitignore-add',{
                        method:'POST',headers:{'Content-Type':'application/json'},
                        body:JSON.stringify({directory:dir,pattern:'passport.json'}),
                      }).then(r=>r.json()).catch(()=>({success:false}));
                      if(r.success)alert('✓ Added "passport.json" to .gitignore at: '+r.gitignore_path);
                      else alert('Not in a Git repo or .gitignore already updated.');
                    }},'Add to .gitignore'),
                  h('button',{className:'btn btn-s btn-sm',
                    onClick:()=>navigator.clipboard.writeText(issueResult.path||'')},'Copy path'))))),
          issueResult&&!issueResult.success&&h('div',{className:'wiz-info',style:{marginBottom:10,borderColor:'rgba(239,68,68,.3)',background:'rgba(239,68,68,.04)'}},
            h('span',{style:{fontSize:18}},'❌'),
            h('div',null,
              h('div',{style:{fontWeight:600,marginBottom:3,color:'#ef4444'}},'Could not create passport'),
              h('div',{style:{color:'var(--t2)',fontSize:'var(--fxs)'}},(issueResult.error||'Unknown error. Is A1 running?')))),
          h('div',{style:{display:'flex',gap:8,flexWrap:'wrap'}},
            h('button',{
              className:'btn btn-p',
              style:{fontSize:'var(--fbase)',padding:'10px 20px'},
              disabled:issuingPassport||caps.length===0,
              onClick:async()=>{
                setIssuingPassport(true);setIssueResult(null);
                const ns=(name||'my-agent').toLowerCase().replace(/\s+/g,'-').replace(/[^a-z0-9-]/g,'');
                const ctrl=new AbortController();
                const timer=setTimeout(()=>ctrl.abort(),30000);
                let r;
                try{
                  r=await fetch((window.A1_GW_URL||'http://localhost:8080')+'/v1/passports/issue',{
                    method:'POST',headers:{'Content-Type':'application/json'},
                    body:JSON.stringify({namespace:ns,capabilities:caps,ttl}),
                    signal:ctrl.signal,
                  }).then(res=>res.json());
                  clearTimeout(timer);
                }catch(e){
                  clearTimeout(timer);
                  if(e.name==='AbortError'){
                    // Gateway may have written the file but not responded — poll to confirm
                    try{
                      const list=await fetch((window.A1_GW_URL||'http://localhost:8080')+'/v1/passports/list').then(res=>res.json()).catch(()=>null);
                      const found=list&&Array.isArray(list.passports)&&list.passports.find(p=>p.namespace===ns);
                      r=found?{success:true,namespace:ns,path:found.path||'',public_key_hex:found.public_key_hex||''}:{success:false,error:'Request timed out. Is A1 running?'};
                    }catch{r={success:false,error:'Request timed out. Is A1 running?'};}
                  }else{r={success:false,error:e.message};}
                }
                setIssueResult(r);setIssuingPassport(false);
                if(r&&r.success){
                  window.dispatchEvent(new CustomEvent('a1-passport-changed'));
                  setTimeout(()=>setStep(3),1800);
                }
              }
            },issuingPassport?'Creating…':(issueResult&&issueResult.success)?'✅ Created — moving to connect…':'🛡 Create Passport'),
            issueResult&&issueResult.success&&h('button',{className:'btn btn-s btn-sm',
              onClick:()=>navigator.clipboard.writeText(issueResult.path||'')},'Copy path ↗')))),

      h('div',{style:{marginTop:20,display:'flex',gap:8,alignItems:'center',flexWrap:'wrap'}},
        h('button',{className:'btn btn-s',onClick:()=>setStep(1)},'← Back'),
        h('button',{className:'btn btn-p',
          style:{fontSize:'var(--fbase)',padding:'10px 22px'},
          disabled:!(issueResult&&issueResult.success),
          onClick:()=>setStep(3)},
          (issueResult&&issueResult.success)?'Next → Connect →':'Create passport first ↑'),
        // Escape hatch — if user already created a passport (e.g. navigated away
        // then came back), let them confirm against the gateway and proceed.
        !(issueResult&&issueResult.success)&&h('button',{
          className:'btn btn-s',
          style:{fontSize:'var(--fxs)',opacity:.75},
          onClick:async()=>{
            const ns=(name||'my-agent').toLowerCase().replace(/\s+/g,'-').replace(/[^a-z0-9-]/g,'');
            const r=await fetch((window.A1_GW_URL||'http://localhost:8080')+'/v1/passports/list')
              .then(r=>r.json()).catch(()=>null);
            const found=r&&Array.isArray(r.passports)&&r.passports.find(p=>p.namespace===ns);
            if(found){
              setIssueResult({success:true,namespace:ns,path:found.path||'',public_key_hex:found.public_key_hex||''});
            } else {
              // Couldn't confirm but let them through anyway — don't trap them
              setStep(3);
            }
          }
        },'Already made one? →'))),

    // ── STEP 3: Choose method (MCP zero-code or decorator) ───────────────────
    step===3&&h('div',null,
      h('h2',{style:{fontSize:18,fontWeight:700,marginBottom:4}},'🔌 How do you want to connect?'),
      h('p',{style:{color:'var(--t2)',fontSize:'var(--fsm)',marginBottom:20}},'Not sure? Pick the first option — it works for most people with no code required.'),

      h('div',{style:{display:'grid',gridTemplateColumns:'1fr 1fr',gap:12,marginBottom:16}},
        h('div',{onClick:()=>setAgentType('mcp'),style:{border:'2px solid '+(agentType==='mcp'?'var(--green)':'var(--b1)'),borderRadius:'var(--r)',padding:16,cursor:'pointer',background:agentType==='mcp'?'rgba(34,197,94,.05)':'var(--s2)',transition:'all .2s'}},
          h('div',{style:{fontSize:22,marginBottom:6}},'⚡'),
          h('div',{style:{fontWeight:700,fontSize:'var(--fbase)',marginBottom:4}},'No code — one config file'),
          h('div',{style:{fontSize:'var(--fxs)',color:'var(--t2)',lineHeight:1.6}},'A1 writes a small file to your agent folder. The agent picks it up automatically — nothing to install, nothing to code.'),
          agentType==='mcp'&&h('div',{style:{marginTop:6,color:'var(--green)',fontSize:'var(--fxs)',fontFamily:'var(--mono)'}},'✓ Selected')),
        h('div',{onClick:()=>{if(agentType==='mcp')setAgentType('claude-code');},style:{border:'2px solid '+(agentType!=='mcp'?'var(--green)':'var(--b1)'),borderRadius:'var(--r)',padding:16,cursor:'pointer',background:agentType!=='mcp'?'rgba(34,197,94,.05)':'var(--s2)',transition:'all .2s'}},
          h('div',{style:{fontSize:22,marginBottom:6}},'🧩'),
          h('div',{style:{fontWeight:700,fontSize:'var(--fbase)',marginBottom:4}},'I write code — add to my project'),
          h('div',{style:{fontSize:'var(--fxs)',color:'var(--t2)',lineHeight:1.6}},'Add one decorator or function to your existing code. Pick your language below.'),
          agentType!=='mcp'&&h('div',{style:{marginTop:6,color:'var(--green)',fontSize:'var(--fxs)',fontFamily:'var(--mono)'}},'✓ Selected'))),

      agentType==='mcp'&&h('div',null,
        h('div',{className:'sg'},
          h('div',{className:'sg-head'},'Add this file to your project folder'),
          h('div',{className:'sg-body'},
            h('p',{style:{fontSize:'var(--fsm)',color:'var(--t2)',marginBottom:8}},'Create ',h('code',{style:{fontFamily:'var(--mono)',background:'var(--b1)',padding:'1px 5px',borderRadius:3}},'.mcp.json'),' in your project:'),
            h('div',{className:'wiz-code'},
              h('pre',null,'{\n  "mcpServers": {\n    "a1": {\n      "type": "http",\n      "url": "http://localhost:8080/mcp"\n    }\n  }\n}'),
              h('button',{className:'btn btn-s btn-sm wiz-copy-btn',onClick:()=>{navigator.clipboard.writeText('{\n  "mcpServers": {\n    "a1": {\n      "type": "http",\n      "url": "http://localhost:8080/mcp"\n    }\n  }\n}');}},'Copy')),
            h('div',{className:'wiz-info gr',style:{marginTop:8}},
              h('span',{style:{fontSize:18}},'✅'),
              h('div',null,
                h('div',{style:{fontWeight:600,marginBottom:3}},"That's everything"),
                h('div',{style:{color:'var(--t2)',lineHeight:1.6,fontSize:'var(--fxs)'}},
                  'Claude Code detects .mcp.json automatically. No restart. No code changes. Tell it: ',
                  h('em',null,'"Check with a1_authorize before executing this action."')))))),
        h('div',{className:'sg',style:{marginTop:12}},
          h('div',{className:'sg-head'},'Then ask Claude Code'),
          h('div',{className:'sg-body'},
            h('div',{className:'wiz-code'},
              h('pre',null,'"Before you do anything, call a1_authorize\nwith capability: '+(caps[0]||'files.read')+'"'),
              h('button',{className:'btn btn-s btn-sm wiz-copy-btn',onClick:()=>navigator.clipboard.writeText('"Before you do anything, call a1_authorize with capability: '+(caps[0]||'files.read')+'"')},'Copy'))))),

      agentType!=='mcp'&&h('div',null,
        h('div',{className:'sg'},
          h('div',{className:'sg-head'},'Which AI agent / framework?'),
          h('div',{className:'sg-body'},
            h('div',{className:'wiz-grid3'},
              W_AGENTS.filter(a=>a.v!=='other').map(a=>h('button',{key:a.v,className:'wiz-agent'+(agentType===a.v?' on':''),onClick:()=>setAgentType(a.v)},h('div',{className:'wiz-agent-icon'},a.e),h('div',{style:{fontSize:'var(--fxs)',fontWeight:500}},a.l)))))),
        agentType&&h('div',{className:'sg',style:{marginTop:12}},
          h('div',{className:'sg-head'},'Integration code — '+(W_AGENTS.find(a=>a.v===agentType)?.l||'Custom')),
          h('div',{className:'sg-body'},
            h('div',{className:'wiz-code'},
              h('pre',null,wizCode(agentType,name,caps,ttl)),
              h('button',{className:'btn btn-s btn-sm wiz-copy-btn',onClick:copy2},c2?'✓ Copied':'Copy')),
            agentType!=='claude-code'&&h('div',{className:'wiz-info',style:{marginTop:8}},
              h('span',{style:{fontSize:18}},'📦'),
              h('div',null,
                h('div',{style:{fontWeight:600,marginBottom:3}},'Install first'),
                h('pre',{style:{fontFamily:'var(--mono)',fontSize:'var(--fxs)',color:'var(--t2)',marginTop:4}},
                  agentType==='typescript'?'npm install a1-ai':agentType==='go'?'go get github.com/dyologician/a1/sdk/go/a1':'pip install a1identity')))))),

      h('div',{style:{marginTop:20,display:'flex',gap:8}},
        h('button',{className:'btn btn-s',onClick:()=>setStep(2)},'← Back'),
        h('button',{className:'btn btn-p',onClick:()=>setStep(4),style:{fontSize:'var(--fbase)',padding:'10px 22px'}},
          'Verify it works →'))),

    // ── STEP 4: Verify ────────────────────────────────────────────────────────
    step===4&&h('div',null,
      h('h2',{style:{fontSize:18,fontWeight:700,marginBottom:4}},'✅ Verify & confirm'),
      h('p',{style:{color:'var(--t2)',fontSize:'var(--fsm)',marginBottom:20}},'Test the connection and confirm everything is working.'),

      h('div',{className:'sg'},
        h('div',{className:'sg-head'},'Live gateway test'),
        h('div',{className:'sg-body'},
          h('div',{style:{display:'flex',gap:8,alignItems:'center',marginBottom:8}},
            h('button',{className:'btn btn-s btn-sm',onClick:testGw,disabled:testing},testing?'Testing…':'▶ Test connection'),
            conn==='ok'&&h('span',{style:{color:'var(--green)',fontFamily:'var(--mono)',fontSize:'var(--fsm)'}},'✓ Connected'),
            conn==='err'&&h('span',{style:{color:'var(--red)',fontFamily:'var(--mono)',fontSize:'var(--fsm)'}},'✕ Not reachable')),
          conn==='ok'&&h('div',{className:'wiz-info gr'},
            h('span',{style:{fontSize:18}},'🎉'),
            h('div',null,
              h('div',{style:{fontWeight:600,marginBottom:3}},'A1 is live!'),
              h('div',{style:{color:'var(--t2)',lineHeight:1.6,fontSize:'var(--fxs)'}},
                'Gateway is responding. Your agent can now verify authorization. Check the ',
                h('strong',null,'Live Log'),' tab to see activity in real time.'))))),

      h('div',{className:'sg',style:{marginTop:12}},
        h('div',{className:'sg-head'},'Your protected agent — confirmed on gateway'),
        h('div',{className:'sg-body'},
          livePassport
            // Real data fetched from gateway — show it as confirmed
            ?h('div',{style:{fontFamily:'var(--mono)',fontSize:'var(--fxs)',lineHeight:2,color:'var(--t2)'}},
                h('div',{style:{marginBottom:6,color:'var(--green)',fontWeight:600,fontSize:'var(--fsm)'}},
                  '✅ Passport confirmed on gateway'),
                h('div',null,h('span',{style:{color:'var(--text)'}},'Namespace:    '),livePassport.namespace||'—'),
                h('div',null,h('span',{style:{color:'var(--text)'}},'Capabilities: '),(livePassport.capabilities||[]).join(', ')||'—'),
                h('div',null,h('span',{style:{color:'var(--text)'}},'Public key:   '),(livePassport.public_key_hex||'').slice(0,32)+'…'),
                h('div',null,h('span',{style:{color:'var(--text)'}},'Path:         '),livePassport.path||'~/.a1/passports/'),
                h('div',null,h('span',{style:{color:'var(--text)'}},'Method:       '),(agentType==='mcp'?'MCP (zero-code)':W_AGENTS.find(a=>a.v===agentType)?.l||agentType)))
            // Fallback: gateway didn't return data (endpoint not found, etc.) — show wizard state with note
            :h('div',null,
                h('div',{style:{marginBottom:8,padding:'6px 10px',background:'rgba(251,191,36,.08)',border:'1px solid rgba(251,191,36,.3)',borderRadius:'var(--r)',fontSize:'var(--fxs)',color:'#b45309'}},
                  '⚠ Could not confirm this passport on the gateway yet. The info below is from your setup session.'),
                h('div',{style:{fontFamily:'var(--mono)',fontSize:'var(--fxs)',lineHeight:2,color:'var(--t2)'}},
                  h('div',null,h('span',{style:{color:'var(--text)'}},'Namespace:   '),(name||issueResult?.namespace||'(not set — go back and enter a name)')),
                  h('div',null,h('span',{style:{color:'var(--text)'}},'Capabilities:'),(caps.join(', ')||'(none)')),
                  h('div',null,h('span',{style:{color:'var(--text)'}},'Expires:     '),ttl,' from now'),
                  h('div',null,h('span',{style:{color:'var(--text)'}},'Method:      '),(agentType==='mcp'?'MCP (zero-code)':W_AGENTS.find(a=>a.v===agentType)?.l||agentType)))))),

      h('div',{style:{marginTop:20,display:'flex',gap:8}},
        h('button',{className:'btn btn-s',onClick:()=>setStep(3)},'← Back'),
        h('button',{className:'btn btn-s',onClick:()=>{
          setStep(1);setName('');setCaps(['files.read','web.search']);setTtl('30d');setConn(null);setAgentType('mcp');setIssueResult(null);setLivePassport(null);
          try{sessionStorage.removeItem('a1_wizard_issue');}catch{}
        }},'+ Protect another agent')))

  );
}

