// ─────────────────────────────────────────────────────────────────────────────
// A1 CONNECTION TESTER — real MCP calls, real responses, nothing simulated
// Calls POST /mcp (JSON-RPC 2.0). Shows exact request and response JSON.
// ─────────────────────────────────────────────────────────────────────────────

const MCP_TOOLS=[
  {
    id:'a1_check_health',
    label:'Check Gateway Health',
    icon:'🏥',
    desc:'Verify A1 is running and get the gateway public key.',
    args:null,  // no args needed
  },
  {
    id:'a1_list_capabilities',
    label:'List All Capabilities',
    icon:'📋',
    desc:'Show every recognized A1 capability name with descriptions.',
    args:null,
  },
  {
    id:'a1_inspect_passport',
    label:'Inspect Passport File',
    icon:'🛡',
    desc:'Read a passport file from disk and show its namespace, capabilities, and expiry status.',
    args:[{key:'passport_path',label:'Passport file path',placeholder:'./passport.json',required:true}],
  },
  {
    id:'a1_authorize',
    label:'Test Authorization',
    icon:'🔐',
    desc:'Test whether an intent would be authorized. Requires a signed chain and agent public key.',
    args:[
      {key:'intent_name',   label:'Capability / intent',placeholder:'files.read',required:true},
      {key:'executor_pk_hex',label:'Agent public key (hex)',placeholder:'(32-byte hex from a1 keygen)',required:true},
    ],
  },
  {
    id:'a1_revoke',
    label:'Revoke a Certificate',
    icon:'🚫',
    desc:'Revoke a cert by fingerprint. That cert can no longer authorize any actions.',
    args:[{key:'fingerprint',label:'Certificate fingerprint (hex)',placeholder:'(64-char hex)',required:true}],
  },
];

function McpTester(){
  const{api,settings}=useContext(Ctx);
  const[tool,setTool]=useState(MCP_TOOLS[0]);
  const[argVals,setArgVals]=useState({});
  const[running,setRunning]=useState(false);
  const[result,setResult]=useState(null);   // {req, res, ok, ms}
  const[rpcId,setRpcId]=useState(1);
  const[gwHealth,setGwHealth]=useState(null);

  // Check gateway on mount
  useEffect(()=>{
    api('GET','/health').then(r=>setGwHealth(r.ok?r.data:null));
  },[]);

  function selectTool(t){
    setTool(t);
    setArgVals({});
    setResult(null);
  }

  async function callTool(){
    if(running)return;
    setRunning(true);
    setResult(null);

    // Build JSON-RPC 2.0 request
    const args={...(argVals)};
    const reqBody={
      jsonrpc:'2.0',
      id:rpcId,
      method:'tools/call',
      params:{name:tool.id,arguments:args},
    };
    setRpcId(p=>p+1);

    const t0=Date.now();
    try{
      const resp=await fetch((settings.gwUrl||'http://localhost:8080')+'/mcp',{
        method:'POST',
        headers:{'Content-Type':'application/json'},
        body:JSON.stringify(reqBody),
      });
      const ms=Date.now()-t0;
      const data=await resp.json();
      const ok=resp.ok&&!data.error&&!data.result?.isError;
      setResult({req:reqBody,res:data,ok,ms,status:resp.status});
    }catch(e){
      const ms=Date.now()-t0;
      setResult({req:reqBody,res:{error:'Network error: '+e.message},ok:false,ms,status:0});
    }
    setRunning(false);
  }

  const gwOk=!!gwHealth;

  return h('div',{style:{paddingBottom:40,width:'100%'}},

    h('h2',{style:{fontSize:18,fontWeight:700,marginBottom:4}},'🔌 Test A1 Connection'),
    h('p',{style:{color:'var(--t2)',fontSize:'var(--fsm)',marginBottom:12,lineHeight:1.6}},
      'Call A1\'s real MCP tools directly from Studio. Every request goes to the live gateway — ',
      h('strong',null,'no simulation, no mocks.'),' You see the exact JSON that your agent sees.'),

    // Gateway status
    h('div',{className:'status-bar',style:{marginBottom:16}},
      h('div',{className:'status-dot '+(gwOk?'green pulse':'red')}),
      gwOk
        ?h('span',null,'Gateway online · ',h('code',{style:{fontFamily:'var(--mono)',fontSize:'var(--fxs)'}},(settings.gwUrl||'http://localhost:8080')),
            ' · key: ',h('code',{style:{fontFamily:'var(--mono)',fontSize:'var(--fxs)'}},(gwHealth?.signing_pk_hex||'').slice(0,16)+'…'))
        :h('span',{style:{color:'#ef4444'}},'Gateway offline — run ',
            h('code',{style:{fontFamily:'var(--mono)',background:'var(--b1)',padding:'1px 5px',borderRadius:3}},'a1 start'),
            ' then refresh')),

    // Tool selector
    h('div',{className:'sg'},
      h('div',{className:'sg-head'},'Choose an A1 tool to call'),
      h('div',{className:'sg-body'},
        h('div',{style:{display:'grid',gridTemplateColumns:'repeat(3,1fr)',gap:6,marginBottom:12}},
          MCP_TOOLS.map(t=>h('button',{key:t.id,
            onClick:()=>selectTool(t),
            style:{
              padding:'8px 10px',border:'1px solid '+(tool.id===t.id?'var(--green)':'var(--b1)'),
              borderRadius:'var(--r)',background:tool.id===t.id?'rgba(34,197,94,.07)':'var(--s2)',
              cursor:'pointer',color:'var(--text)',textAlign:'left',transition:'all .15s',
            }},
            h('div',{style:{fontSize:14,marginBottom:2}},t.icon),
            h('div',{style:{fontSize:'var(--fxs)',fontWeight:600,lineHeight:1.3}},t.label)))),

        // Selected tool description
        h('div',{style:{color:'var(--t2)',fontSize:'var(--fsm)',marginBottom:tool.args?12:0}},tool.desc),

        // Args
        tool.args&&h('div',{style:{display:'flex',flexDirection:'column',gap:8,marginBottom:12}},
          tool.args.map(a=>h('div',{key:a.key,className:'field'},
            h('label',{className:'lbl'},a.label+(a.required?' *':'')),
            h('input',{className:'inp inp-mono',type:'text',
              placeholder:a.placeholder,
              value:argVals[a.key]||'',
              onChange:e=>setArgVals(p=>({...p,[a.key]:e.target.value}))})))),

        h('div',{style:{display:'flex',gap:8,alignItems:'center'}},
          h('button',{className:'btn btn-p',onClick:callTool,
            disabled:running||!gwOk||(tool.args&&tool.args.filter(a=>a.required).some(a=>!argVals[a.key]?.trim())),
          },running?'Calling…':'▶ Call '+tool.label),
          result&&h('span',{style:{fontSize:'var(--fxs)',fontFamily:'var(--mono)',color:result.ok?'var(--green)':'#ef4444'}},
            (result.ok?'✓':'✕')+' '+result.ms+'ms · HTTP '+result.status)))),

    // Request / Response
    result&&h('div',{className:'sg',style:{marginTop:12}},
      h('div',{className:'sg-head'},'JSON-RPC 2.0 · Request → Response'),
      h('div',{className:'sg-body'},
        h('div',{style:{display:'grid',gridTemplateColumns:'1fr 1fr',gap:10}},
          h('div',null,
            h('div',{style:{fontSize:'var(--fxs)',color:'var(--t2)',fontFamily:'var(--mono)',marginBottom:4}},'→ REQUEST'),
            h('pre',{style:{background:'var(--s3)',padding:10,borderRadius:'var(--r)',fontSize:'var(--fxs)',fontFamily:'var(--mono)',overflow:'auto',maxHeight:240,margin:0,lineHeight:1.7}},
              JSON.stringify(result.req,null,2))),
          h('div',null,
            h('div',{style:{fontSize:'var(--fxs)',color:result.ok?'var(--green)':'#ef4444',fontFamily:'var(--mono)',marginBottom:4}},
              '← '+(result.ok?'SUCCESS':'ERROR')),
            h('pre',{style:{background:'var(--s3)',padding:10,borderRadius:'var(--r)',fontSize:'var(--fxs)',fontFamily:'var(--mono)',overflow:'auto',maxHeight:240,margin:0,lineHeight:1.7}},
              JSON.stringify(result.res,null,2)))),

        // Human-readable interpretation
        result.res?.result?.content&&h('div',{className:'wiz-info gr',style:{marginTop:10}},
          h('span',{style:{fontSize:18}},'📋'),
          h('div',null,
            h('div',{style:{fontWeight:600,marginBottom:3}},'What this means'),
            h('pre',{style:{margin:0,fontFamily:'var(--sans)',fontSize:'var(--fsm)',color:'var(--t2)',whiteSpace:'pre-wrap',lineHeight:1.7}},
              result.res.result.content.map(c=>c.text||'').join('\n')))),

        result.res?.error&&h('div',{className:'wiz-info',style:{marginTop:10,borderColor:'rgba(239,68,68,.25)',background:'rgba(239,68,68,.05)'}},
          h('span',{style:{fontSize:18}},'❌'),
          h('div',null,
            h('div',{style:{fontWeight:600,marginBottom:3,color:'#ef4444'}},'Error from gateway'),
            h('div',{style:{fontSize:'var(--fsm)',color:'var(--t2)',lineHeight:1.6}},
              typeof result.res.error==='object'
                ?result.res.error.message
                :result.res.error),
            h('div',{style:{marginTop:6,fontSize:'var(--fxs)',color:'var(--t2)'}},'Check the Live Log tab for the full request trace.')))
      )),

    // Context explainer
    h('div',{className:'wiz-info',style:{marginTop:16}},
      h('span',{style:{fontSize:18}},'💡'),
      h('div',null,
        h('div',{style:{fontWeight:600,marginBottom:3}},'Why can\'t I chat directly with OpenClaw or IronClaw here?'),
        h('div',{style:{color:'var(--t2)',lineHeight:1.7,fontSize:'var(--fxs)'}},
          'A1 Studio talks to the A1 gateway — not to your agent directly. OpenClaw and IronClaw run as separate processes on your machine. ',
          'To test your agent, open its own interface (OpenClaw\'s dashboard, Claude Code\'s terminal, etc.) and interact there. ',
          'What this panel tests is the A1 authorization layer that sits between your agent and its actions — the part that A1 controls.')))
  );
}


