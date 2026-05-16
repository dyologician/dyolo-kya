// ─────────────────────────────────────────────────────────────────────────────
// TAB CONTENTS
// ─────────────────────────────────────────────────────────────────────────────
function Overview({health,logs}){
  const{settings}=useContext(Ctx);
  const gwUrl=settings.gwUrl||'http://localhost:8080';
  const recent=logs.slice(-6).reverse();
  const errC=logs.filter(l=>!l.ok).length;
  const avgMs=logs.length?Math.round(logs.reduce((a,l)=>a+l.ms,0)/logs.length):0;
  return h('div',null,
    h(ProtectionStatusBanner, { gwUrl }),
    h('div',{className:'g4 mb14'},
      h('div',{className:'card'},h('div',{className:'ctitle'},'Gateway'),h('div',{className:'bignum'},h('span',{className:'dot dot-'+(health?'green':'red'),style:{marginRight:8}}),health?'Online':'Offline'),h('div',{className:'bigsub'},health?'v'+(health.version||'2.8.0'):settings.gwUrl)),
      h('div',{className:'card'},h('div',{className:'ctitle'},'Requests'),h('div',{className:'bignum'},logs.length),h('div',{className:'bigsub'},'this session')),
      h('div',{className:'card'},h('div',{className:'ctitle'},'Errors'),h('div',{className:'bignum',style:{color:errC>0?'var(--red)':'var(--text)'}},errC),h('div',{className:'bigsub'},errC>0?'check live log':'clean')),
      h('div',{className:'card'},h('div',{className:'ctitle'},'Avg Latency'),h('div',{className:'bignum'},avgMs>0?avgMs+'ms':'—'),h('div',{className:'bigsub'},'round-trip'))
    ),
    h('div',{className:'g2 mb14'},
      h('div',{className:'card'},
        h('div',{className:'ctitle'},'A1 — One Identity. Full Provenance.'),
        h('p',{style:{fontSize:'var(--fsm)',color:'var(--t2)',marginBottom:14,lineHeight:'1.7'}},'The cryptographic identity layer that closes the Recursive Delegation Gap. Every agent action carries an irrefutable chain of custody from the executing agent back to the authorizing human.'),
        h(SocialLinks,null)
      ),
      health?h('div',{className:'card','data-help':'gateway-id'},
        h('div',{className:'ctitle'},'Gateway Identity'),
        h('div',{className:'field'},h('div',{className:'lbl'},'DID'),h(TruncId,{val:health.signing_pk_hex?'did:a1:'+health.signing_pk_hex:null,showFull:settings.showFullIds})),
        h('div',{className:'field'},h('div',{className:'lbl'},'Signing Key'),h(TruncId,{val:health.signing_pk_hex,showFull:settings.showFullIds})),
        h('div',{className:'field'},h('div',{className:'lbl'},'Protocol'),h('span',{className:'badge badge-dim'},'dyolo v2.8.0')),
        h('div',{className:'field'},h('div',{className:'lbl'},'Storage'),h('span',{className:'badge badge-dim'},health.storage_backend||'memory')),
        h('div',{style:{display:'flex',gap:6,marginTop:4,flexWrap:'wrap'}},
          h('span',{className:'badge '+(health.webhook_enabled?'badge-ok':'badge-dim')},(health.webhook_enabled?'✓':'✗')+' Webhook'),
          h('span',{className:'badge '+(health.jwt_exchange_enabled?'badge-ok':'badge-dim')},(health.jwt_exchange_enabled?'✓':'✗')+' JWT Exchange'),
          h('span',{className:'badge '+(health.multi_tenant_enabled?'badge-ok':'badge-dim')},(health.multi_tenant_enabled?'✓':'✗')+' Multi-Tenant')
        )
      ):h('div',{className:'card'},h('div',{className:'empty'},'Gateway offline — check Settings → Gateway URL'))
    ),
    h('div',{className:'card','data-help':'overview'},
      h('div',{className:'sec-head'},
        h('div',{className:'sec-title'},'Activity'),
        h('div',{style:{display:'flex',alignItems:'center',gap:5}},h('div',{className:'dot dot-green dot-pulse'}),h('span',{style:{fontSize:'var(--fxs)',color:'var(--t2)',fontFamily:'var(--mono)'}},'live'))
      ),
      recent.length===0?h('div',{className:'empty'},'No requests yet. Use the tabs to interact with the gateway.'):
      h('div',{className:'log-stream',style:{height:200}},recent.map(e=>h(LogEntry,{key:e.id,e})))
    )
  );
}

function LiveLogs({logs,onClear}){
  const{settings}=useContext(Ctx);
  const[filter,setFilter]=useState('');
  const[autoS,setAutoS]=useState(settings.autoScroll);
  const ref=useRef(null);
  const filtered=useMemo(()=>{
    if(!filter)return logs;
    const f=filter.toLowerCase();
    return logs.filter(l=>l.path.includes(f)||l.method.toLowerCase().includes(f)||String(l.status).includes(f));
  },[logs,filter]);
  useEffect(()=>{if(autoS&&ref.current)ref.current.scrollTop=ref.current.scrollHeight;},[filtered,autoS]);
  const okC=logs.filter(l=>l.ok).length;
  const errC=logs.filter(l=>!l.ok).length;
  return h('div',{'data-help':'live-log',style:{width:'100%'}},
    h('div',{className:'log-wrap'},
      h('div',{className:'log-bar'},
        h('span',{className:'log-bar-title'},'Live Request Log'),
        okC>0&&h('span',{className:'badge badge-ok'},okC+' ok'),
        errC>0&&h('span',{className:'badge badge-err'},errC+' err'),
        h('span',{style:{fontSize:'var(--fxs)',color:'var(--t2)',fontFamily:'var(--mono)',marginLeft:4}},logs.length+' total')
      ),
      h('div',{style:{padding:'6px 10px',borderBottom:'1px solid var(--b1)',display:'flex',alignItems:'center',gap:6,background:'var(--s2)'},'data-help':'log-filter'},
        h('input',{className:'inp inp-mono',style:{flex:1,padding:'4px 8px'},placeholder:'filter by path / method / status…',value:filter,onChange:e=>setFilter(e.target.value)}),
        h('button',{className:'btn btn-sm btn-s',style:{fontFamily:'var(--mono)'},onClick:()=>setAutoS(a=>!a)},autoS?'⏸':'▶'),
        h('button',{className:'btn btn-sm btn-d',onClick:onClear},'clear')
      ),
      h('div',{ref,className:'log-stream',style:{height:460}},
        filtered.length===0?h('div',{className:'empty'},'No requests. Make API calls from other tabs.'):
        filtered.slice().reverse().map(e=>h(LogEntry,{key:e.id,e}))
      )
    ),
    h('p',{style:{fontSize:'var(--fxs)',color:'var(--t2)',marginTop:9,fontFamily:'var(--mono)'}},'All requests made by this Studio session. Click an entry to expand. For gateway-level logs, tail the a1-gateway process stdout.')
  );
}

function Passports(){
  const{api,settings}=useContext(Ctx);
  const[name,setName]=useState('');
  const[caps,setCaps]=useState('trade.equity,portfolio.read');
  const[ttl,setTtl]=useState('30');
  const[sk,setSk]=useState('');
  const[msg,setMsg]=useState(null);
  const[msgT,setMsgT]=useState('ok');
  const[result,setResult]=useState(null);
  const PRESETS=['trade.equity','portfolio.read','portfolio.write','audit.read','email.send','data.export','web.search','api.call'];
  async function issue(){
    if(!name){setMsg('Agent name required.');setMsgT('error');return;}
    setMsg(null);
    const key=sk||Array.from(crypto.getRandomValues(new Uint8Array(32))).map(b=>b.toString(16).padStart(2,'0')).join('');
    const r=await api('POST','/v1/cert/issue',{passport_namespace:name,capabilities:caps.split(',').map(s=>s.trim()).filter(Boolean),ttl_seconds:parseInt(ttl)*86400,signing_key_hex:key});
    if(r.ok){setMsg('Passport issued.');setMsgT('ok');setResult({...r.data,_sk:key});}
    else{setMsg(r.data.error||'Failed.');setMsgT('error');}
  }
  return h('div',{'data-help':'passports',style:{width:'100%'}},
    h('div',{className:'sec-head'},h('div',{className:'sec-title'},'Issue Passport')),
    h(Alert,{msg,type:msgT}),
    h('div',{className:'card mb14'},
      h('div',{className:'g2'},
        h('div',{className:'field'},h('label',{className:'lbl'},'Agent Name'),h('input',{className:'inp',placeholder:'acme-trading-bot',value:name,onChange:e=>setName(e.target.value)})),
        h('div',{className:'field'},h('label',{className:'lbl'},'TTL (days)'),h('input',{className:'inp inp-mono',type:'number',min:1,max:365,value:ttl,onChange:e=>setTtl(e.target.value)}))
      ),
      h('div',{className:'field'},h('label',{className:'lbl'},'Capabilities'),h('input',{className:'inp inp-mono',value:caps,onChange:e=>setCaps(e.target.value)})),
      h('div',{style:{display:'flex',flexWrap:'wrap',gap:4,marginBottom:11}},
        PRESETS.map(c=>h('button',{key:c,className:'btn btn-xs btn-s',onClick:()=>setCaps(p=>p?(p.includes(c)?p:p+','+c):c)},'+ '+c))
      ),
      h('div',{className:'field'},h('label',{className:'lbl'},'Signing Key (optional — auto-generated if blank)'),h('input',{className:'inp inp-mono',placeholder:'64-char hex Ed25519 seed',value:sk,onChange:e=>setSk(e.target.value)})),
      h('button',{className:'btn btn-p',onClick:issue},'Issue Passport')
    ),
    result&&h('div',{className:'card'},
      h('div',{className:'ctitle',style:{marginBottom:10}},'Result — copy the signing key now'),
      h('div',{className:'field'},h('label',{className:'lbl'},'Signing Key (save this)'),h(TruncId,{val:result._sk,showFull:true})),
      h('pre',{style:{background:'var(--s2)',border:'1px solid var(--b1)',borderRadius:'var(--r)',padding:11,fontSize:9,overflow:'auto',maxHeight:200,color:'var(--t2)',fontFamily:'var(--mono)'}},
        JSON.stringify(Object.fromEntries(Object.entries(result).filter(([k])=>k!=='_sk')),null,2)
      )
    )
  );
}

function Swarms(){
  const{api,settings}=useContext(Ctx);
  const[name,setName]=useState('');
  const[caps,setCaps]=useState('trade.equity,portfolio.read');
  const[ttl,setTtl]=useState('30');
  const[key,setKey]=useState('');
  const[swarmId,setSwarmId]=useState('');
  const[agentPk,setAgentPk]=useState('');
  const[role,setRole]=useState('worker');
  const[roleCaps,setRoleCaps]=useState('trade.equity');
  const[mTtl,setMTtl]=useState('3600');
  const[members,setMembers]=useState([]);
  const[msg,setMsg]=useState(null);
  const[msgT,setMsgT]=useState('ok');
  async function create(){
    if(!name||!key){setMsg('Name + signing key required.');setMsgT('error');return;}
    const r=await api('POST','/v1/swarm/create',{swarm_name:name,capabilities:caps.split(',').map(s=>s.trim()).filter(Boolean),ttl_days:parseInt(ttl),signing_key_hex:key});
    if(r.ok){setSwarmId(r.data.swarm_id);setMsg('Swarm created.');setMsgT('ok');}
    else{setMsg(r.data.error||'Failed');setMsgT('error');}
  }
  async function addMember(){
    if(!swarmId||!agentPk){setMsg('Create swarm + enter agent key first.');setMsgT('error');return;}
    const r=await api('POST','/v1/swarm/member/add',{swarm_id:swarmId,agent_pk_hex:agentPk,role,capabilities:roleCaps.split(',').map(s=>s.trim()).filter(Boolean),ttl_seconds:parseInt(mTtl),signing_key_hex:key});
    if(r.ok){setMsg('Member added.');setMsgT('ok');}
    else{setMsg(r.data.error||'Failed');setMsgT('error');}
  }
  async function listM(){
    if(!swarmId)return;
    const r=await api('GET',`/v1/swarm/${swarmId}/members`);
    if(r.ok)setMembers(r.data.members||[]);
  }
  return h('div',{'data-help':'swarms',style:{width:'100%'}},
    h('div',{className:'sec-head'},h('div',{className:'sec-title'},'Swarm Passports')),
    h(Alert,{msg,type:msgT}),
    h('div',{className:'card mb14'},
      h('div',{className:'ctitle'},'Create Swarm'),
      h('div',{className:'g2'},
        h('div',{className:'field'},h('label',{className:'lbl'},'Swarm Name'),h('input',{className:'inp',placeholder:'acme-swarm',value:name,onChange:e=>setName(e.target.value)})),
        h('div',{className:'field'},h('label',{className:'lbl'},'TTL (days)'),h('input',{className:'inp inp-mono',type:'number',value:ttl,onChange:e=>setTtl(e.target.value)}))
      ),
      h('div',{className:'field'},h('label',{className:'lbl'},'Root Capabilities'),h('input',{className:'inp inp-mono',value:caps,onChange:e=>setCaps(e.target.value)})),
      h('div',{className:'field'},h('label',{className:'lbl'},'Orchestrator Signing Key (hex)'),h('input',{className:'inp inp-mono',placeholder:'64-char hex',value:key,onChange:e=>setKey(e.target.value)})),
      h('div',{style:{display:'flex',gap:7,flexWrap:'wrap'}},
        h('button',{className:'btn btn-p',onClick:create},'Create Swarm'),
        swarmId&&h('button',{className:'btn btn-s',onClick:listM},'Refresh Members')
      ),
      swarmId&&h('div',{style:{marginTop:10}},h('div',{className:'lbl'},'Swarm ID'),h(TruncId,{val:swarmId,showFull:settings.showFullIds}))
    ),
    swarmId&&h('div',{className:'card mb14'},
      h('div',{className:'ctitle'},'Add Member'),
      h('div',{className:'g2'},
        h('div',{className:'field'},h('label',{className:'lbl'},'Agent Public Key (hex)'),h('input',{className:'inp inp-mono',placeholder:'64-char hex',value:agentPk,onChange:e=>setAgentPk(e.target.value)})),
        h('div',{className:'field'},h('label',{className:'lbl'},'Role'),h('select',{className:'inp',value:role,onChange:e=>setRole(e.target.value)},['orchestrator','worker','supervisor','auditor'].map(r=>h('option',{key:r,value:r},r))))
      ),
      h('div',{className:'g2'},
        h('div',{className:'field'},h('label',{className:'lbl'},'Role Capabilities'),h('input',{className:'inp inp-mono',value:roleCaps,onChange:e=>setRoleCaps(e.target.value)})),
        h('div',{className:'field'},h('label',{className:'lbl'},'TTL (sec)'),h('input',{className:'inp inp-mono',type:'number',value:mTtl,onChange:e=>setMTtl(e.target.value)}))
      ),
      h('button',{className:'btn btn-p',onClick:addMember},'Add Member')
    ),
    members.length>0&&h('div',{className:'card'},
      h('div',{className:'ctitle'},'Members ('+members.length+')'),
      h('table',{className:'tbl'},
        h('thead',null,h('tr',null,h('th',null,'DID'),h('th',null,'Role'),h('th',null,'Expires'))),
        h('tbody',null,members.map((m,i)=>h('tr',{key:i},
          h('td',null,h(TruncId,{val:m.agent_did,showFull:settings.showFullIds})),
          h('td',null,h('span',{className:'badge badge-dim'},m.role)),
          h('td',{style:{fontFamily:'var(--mono)',fontSize:10,color:'var(--t2)'}},new Date(m.expires_at_unix*1000).toLocaleString())
        )))
      )
    )
  );
}

function DidVc(){
  const{api,settings}=useContext(Ctx);
  const[pk,setPk]=useState('');
  const[didRes,setDidRes]=useState(null);
  const[gwDoc,setGwDoc]=useState(null);
  const[vPk,setVPk]=useState('');
  const[vNs,setVNs]=useState('');
  const[vCaps,setVCaps]=useState('');
  const[vTtl,setVTtl]=useState('86400');
  const[vcRes,setVcRes]=useState(null);
  const[vcStr,setVcStr]=useState('');
  const[vcVer,setVcVer]=useState(null);
  const[msg,setMsg]=useState(null);
  const[msgT,setMsgT]=useState('ok');
  const PRE={background:'var(--s2)',border:'1px solid var(--b1)',borderRadius:'var(--r)',padding:9,fontSize:9,overflow:'auto',maxHeight:160,color:'var(--t2)',fontFamily:'var(--mono)',marginTop:9};
  async function resolveGw(){const r=await api('GET','/v1/did/gateway');if(r.ok)setGwDoc(r.data);else{setMsg(r.data.error||'Failed');setMsgT('error');}}
  async function resolve(){if(!pk){setMsg('Enter key hex.');setMsgT('error');return;}const r=await api('GET','/v1/did/'+pk);if(r.ok){setDidRes(r.data);setMsg(null);}else{setMsg(r.data.error||'Failed');setMsgT('error');}}
  async function issueVc(){if(!vPk||!vNs){setMsg('Subject key + namespace required.');setMsgT('error');return;}const r=await api('POST','/v1/vc/issue',{subject_pk_hex:vPk,passport_namespace:vNs,capabilities:vCaps.split(',').map(s=>s.trim()).filter(Boolean),ttl_seconds:parseInt(vTtl)});if(r.ok){setVcRes(r.data);setMsg(null);}else{setMsg(r.data.error||'Failed');setMsgT('error');}}
  async function verifyVc(){let c;try{c=JSON.parse(vcStr);}catch{setMsg('Invalid JSON.');setMsgT('error');return;}const r=await api('POST','/v1/vc/verify',{credential:c});if(r.ok){setVcVer(r.data);setMsg(null);}else{setMsg(r.data.error||'Failed');setMsgT('error');}}
  return h('div',{'data-help':'did',style:{width:'100%'}},
    h(Alert,{msg,type:msgT}),
    h('div',{className:'g2 mb14'},
      h('div',{className:'card'},h('div',{className:'ctitle'},'Gateway DID'),h('p',{style:{fontSize:'var(--fsm)',color:'var(--t2)',marginBottom:10}},"Resolve the gateway's W3C DID Document."),h('button',{className:'btn btn-p',onClick:resolveGw},'Resolve'),gwDoc&&h('pre',{style:PRE},JSON.stringify(gwDoc,null,2))),
      h('div',{className:'card'},h('div',{className:'ctitle'},'Resolve Any DID'),h('div',{className:'field'},h('label',{className:'lbl'},'Public Key (hex)'),h('input',{className:'inp inp-mono',placeholder:'64-char hex',value:pk,onChange:e=>setPk(e.target.value)})),h('button',{className:'btn btn-p',onClick:resolve},'Resolve'),didRes&&h('pre',{style:PRE},JSON.stringify(didRes,null,2)))
    ),
    h('div',{className:'g2 mb14'},
      h('div',{className:'card'},
        h('div',{className:'ctitle'},'Issue VC'),
        h('div',{className:'field'},h('label',{className:'lbl'},'Subject Public Key'),h('input',{className:'inp inp-mono',placeholder:'64-char hex',value:vPk,onChange:e=>setVPk(e.target.value)})),
        h('div',{className:'field'},h('label',{className:'lbl'},'Namespace'),h('input',{className:'inp',placeholder:'acme-bot',value:vNs,onChange:e=>setVNs(e.target.value)})),
        h('div',{className:'g2'},h('div',{className:'field'},h('label',{className:'lbl'},'Capabilities'),h('input',{className:'inp inp-mono',value:vCaps,onChange:e=>setVCaps(e.target.value)})),h('div',{className:'field'},h('label',{className:'lbl'},'TTL (sec)'),h('input',{className:'inp inp-mono',type:'number',value:vTtl,onChange:e=>setVTtl(e.target.value)}))),
        h('button',{className:'btn btn-p',onClick:issueVc},'Issue VC'),
        vcRes&&h('pre',{style:PRE},JSON.stringify(vcRes,null,2))
      ),
      h('div',{className:'card'},
        h('div',{className:'ctitle'},'Verify VC'),
        h('div',{className:'field'},h('label',{className:'lbl'},'Credential JSON'),h('textarea',{className:'inp inp-mono',rows:7,placeholder:'Paste W3C VC JSON…',value:vcStr,onChange:e=>setVcStr(e.target.value)})),
        h('button',{className:'btn btn-p',onClick:verifyVc},'Verify'),
        vcVer&&h('div',{style:{marginTop:9}},h('span',{className:vcVer.valid?'badge badge-ok':'badge badge-err'},vcVer.valid?'✓ Valid':'✗ Invalid'),vcVer.error&&h('p',{style:{fontSize:'var(--fxs)',color:'var(--red)',marginTop:5,fontFamily:'var(--mono)'}},vcVer.error))
      )
    )
  );
}

function Authorize(){
  const{api}=useContext(Ctx);
  const[chain,setChain]=useState('');
  const[intent,setIntent]=useState('trade.equity');
  const[epk,setEpk]=useState('');
  const[result,setResult]=useState(null);
  const[msg,setMsg]=useState(null);
  const[msgT,setMsgT]=useState('ok');
  async function run(){
    let c;try{c=JSON.parse(chain);}catch{setMsg('Invalid chain JSON.');setMsgT('error');return;}
    setMsg(null);
    const r=await api('POST','/v1/authorize',{chain:c,intent_name:intent,executor_pk_hex:epk});
    if(r.ok){setResult(r.data);setMsg('Authorized.');setMsgT('ok');}
    else{setResult(r.data);setMsg(r.data.error||'Failed.');setMsgT('error');}
  }
  return h('div',{'data-help':'authorize',style:{width:'100%'}},
    h('div',{className:'sec-head'},h('div',{className:'sec-title'},'Test Authorization')),
    h(Alert,{msg,type:msgT}),
    h('div',{className:'card'},
      h('div',{className:'g2'},
        h('div',{className:'field'},h('label',{className:'lbl'},'Intent Name'),h('input',{className:'inp inp-mono',value:intent,onChange:e=>setIntent(e.target.value)})),
        h('div',{className:'field'},h('label',{className:'lbl'},'Executor Public Key (hex)'),h('input',{className:'inp inp-mono',placeholder:'64-char hex',value:epk,onChange:e=>setEpk(e.target.value)}))
      ),
      h('div',{className:'field','data-help':'auth-chain'},h('label',{className:'lbl'},'Delegation Chain (JSON)'),h('textarea',{className:'inp inp-mono',rows:7,placeholder:'Paste SignedChain JSON…',value:chain,onChange:e=>setChain(e.target.value)})),
      h('button',{className:'btn btn-p',onClick:run},'Authorize'),
      result&&h('pre',{style:{marginTop:13,background:'var(--s2)',border:'1px solid var(--b1)',borderRadius:'var(--r)',padding:10,fontSize:9,overflow:'auto',maxHeight:240,color:'var(--t2)',fontFamily:'var(--mono)'}},JSON.stringify(result,null,2))
    )
  );
}

function Compliance(){
  const{api}=useContext(Ctx);
  const[scope,setScope]=useState('');
  const[report,setReport]=useState(null);
  const[msg,setMsg]=useState(null);
  const[msgT,setMsgT]=useState('ok');
  async function generate(){
    if(!scope){setMsg('Enter scope.');setMsgT('error');return;}
    const now=Math.floor(Date.now()/1000);
    const r=await api('POST','/v1/governance/audit-report',{scope,period_start_unix:now-86400*30,period_end_unix:now});
    if(r.ok){setReport(r.data);setMsg(null);}else{setMsg(r.data.error||'Failed');setMsgT('error');}
  }
  function dl(){
    if(!report)return;
    const a=document.createElement('a');a.href=URL.createObjectURL(new Blob([JSON.stringify(report,null,2)],{type:'application/json'}));
    a.download='a1-audit-'+report.scope+'-'+new Date().toISOString().slice(0,10)+'.json';a.click();
  }
  return h('div',{'data-help':'compliance',style:{width:'100%'}},
    h('div',{className:'sec-head'},h('div',{className:'sec-title'},'Compliance & Audit')),
    h(Alert,{msg,type:msgT}),
    h('div',{className:'card mb14'},
      h('p',{style:{fontSize:'var(--fsm)',color:'var(--t2)',marginBottom:13,lineHeight:'1.7'}},'Generate a compliance report covering EU AI Act Art. 13/14, NIST AI RMF Govern 1.7, SOC 2 CC6.1/CC7.2, and ISO 27001 A.9.'),
      h('div',{className:'field'},h('label',{className:'lbl'},'Organization / Scope'),h('input',{className:'inp',placeholder:'acme-corp',value:scope,onChange:e=>setScope(e.target.value)})),
      h('div',{style:{display:'flex',gap:7}},h('button',{className:'btn btn-p',onClick:generate},'Generate'),report&&h('button',{className:'btn btn-s',onClick:dl},'↓ JSON'))
    ),
    report&&h('div',{className:'card'},
      h('div',{style:{fontFamily:'var(--mono)',fontSize:'var(--fsm)',fontWeight:700,marginBottom:9}},report.title),
      h('div',{style:{fontSize:'var(--fxs)',color:'var(--t2)',fontFamily:'var(--mono)',marginBottom:12}},'Generated: '+report.generated_at),
      h('div',{className:'g2',style:{marginBottom:12}},
        h('div',null,h('div',{className:'lbl'},'Authorizations'),h('div',{style:{fontFamily:'var(--mono)',fontSize:18,fontWeight:700}},report.total_authorizations)),
        h('div',null,h('div',{className:'lbl'},'Policy Hash'),h(TruncId,{val:report.policy_commitment_hex,showFull:false}))
      ),
      h('div',{className:'lbl',style:{marginBottom:7}},'Standards'),
      h('div',{style:{display:'flex',flexWrap:'wrap',gap:5}},(report.compliance_standards||[]).map((s,i)=>h('span',{key:i,className:'badge badge-ok'},s)))
    )
  );
}

function Settings({settings,onUpdate,health,onShowOnboard}){
  const[loc,setLoc]=useState({...settings});
  const[saved,setSaved]=useState(false);
  function upd(k,v){setLoc(s=>({...s,[k]:v}));}
  function save(){onUpdate(loc);setSaved(true);setTimeout(()=>setSaved(false),1400);}
  const ENV=[
    ['A1_SIGNING_KEY_HEX','32-byte hex Ed25519 seed','Required'],
    ['A1_ADMIN_SECRET','Bearer token for admin endpoints','Production'],
    ['A1_REDIS_URL','Redis nonce + revocation store','Recommended'],
    ['A1_PG_URL','Postgres persistent audit log','Enterprise'],
    ['A1_GOVERNANCE_POLICY_FILE','Path to governance.json','Enterprise'],
    ['A1_NEGOTIATE_CAPABILITIES','Allowed caps for /negotiate (CSV)','AIP'],
    ['A1_RATE_LIMIT_RPS','Per-IP rate limit (default 500)','Optional'],
    ['GATEWAY_ADDR','Bind address (default 0.0.0.0:8080)','Optional'],
    ['RUST_LOG','Log filter (e.g. a1_gateway=info)','Optional'],
  ];

  return h('div',{'data-help':'settings',style:{width:'100%'}},
    h('div',{className:'sec-head'},h('div',{className:'sec-title'},'Settings')),

    h('div',{className:'sg'},
      h('div',{className:'sg-head'},'Mode'),
      h('div',{className:'sg-body'},
        h(ToggleRow,{
          label:'Developer Mode',
          sub:'Show advanced sections: Monitor logs, raw Passports, Swarms, DID & VC, Authorize, Compliance, and Direct Connect. Turn off to keep the sidebar clean.',
          checked:loc.developerMode,
          onChange:v=>{ upd('developerMode',v); onUpdate({...loc,developerMode:v}); }
        }),
        !loc.developerMode&&h('div',{style:{marginTop:8,padding:'8px 10px',background:'var(--s2)',border:'1px solid var(--b1)',borderRadius:'var(--r)',fontSize:'var(--fxs)',color:'var(--t2)',lineHeight:'1.6'}},
          '\u2705 Simple mode — Redis, Postgres, KMS, Swarms, and Compliance are hidden. They\'re only needed for production team deployments. Most people never need them.'
        )
      )
    ),

    h('div',{className:'sg'},
      h('div',{className:'sg-head'},'Gateway Connection'),
      h('div',{className:'sg-body'},
        h('div',{className:'field'},h('label',{className:'lbl'},'Gateway URL'),h('input',{className:'inp inp-mono',value:loc.gwUrl,onChange:e=>upd('gwUrl',e.target.value)})),
        h('div',{className:'field'},
          h('label',{className:'lbl'},'Admin Secret'),
          h('div',{style:{display:'flex',gap:6}},
            h('input',{className:'inp inp-mono',type:loc.showSecret?'text':'password',placeholder:'A1_ADMIN_SECRET',value:loc.adminSecret,onChange:e=>upd('adminSecret',e.target.value),style:{flex:1}}),
            h('button',{className:'btn btn-s btn-sm',onClick:()=>upd('showSecret',!loc.showSecret)},loc.showSecret?'Hide':'Show')
          ),
          h('p',{style:{fontSize:'var(--fxs)',color:'var(--t2)',marginTop:4,fontFamily:'var(--mono)'}},'Stored locally in this browser only.')
        ),
        health&&h('div',{style:{fontSize:'var(--fxs)',fontFamily:'var(--mono)',color:'var(--t2)',padding:'5px 0'}},h('span',{className:'dot dot-green',style:{marginRight:6}}),'Connected \u00b7 '+loc.gwUrl)
      )
    ),

    h('div',{className:'sg'},
      h('div',{className:'sg-head'},'UI Scale & Display'),
      h('div',{className:'sg-body'},
        h('div',{className:'field','data-help':'density-ctrl'},
          h('label',{className:'lbl'},'Density'),
          h('div',{style:{display:'flex',alignItems:'center',gap:6,marginBottom:6}},
            h('div',{className:'density-pills'},
              ['auto','compact','normal','comfortable'].map(d=>h('button',{key:d,className:'density-pill'+(loc.density===d?' on':''),onClick:()=>upd('density',d)},d))
            )
          ),
          h('p',{style:{fontSize:'var(--fxs)',color:'var(--t2)',fontFamily:'var(--mono)'}},'Auto adapts to your screen width automatically.')
        ),
        h('div',{className:'field','data-help':'font-ctrl'},
          h('label',{className:'lbl'},'Font Size \u2014 '+loc.fontSize+'px'),
          h('div',{className:'slider-row'},
            h('input',{type:'range',min:11,max:18,step:1,value:loc.fontSize,onChange:e=>{upd('fontSize',parseInt(e.target.value));applyScaling({...loc,fontSize:parseInt(e.target.value)});}}),
            h('span',{className:'slider-val'},loc.fontSize+'px')
          )
        ),
        h(ToggleRow,{label:'Show Full IDs',sub:'Display complete hex keys and DIDs without truncation',checked:loc.showFullIds,onChange:v=>upd('showFullIds',v)}),
        h(ToggleRow,{label:'Auto-scroll Live Log',sub:'Jump to newest log entries as they arrive',checked:loc.autoScroll,onChange:v=>upd('autoScroll',v)}),
        h('div',{className:'g2',style:{marginTop:10}},
          h('div',{className:'field'},h('label',{className:'lbl'},'Health Poll (ms)'),h('input',{className:'inp inp-mono',type:'number',min:1000,max:60000,step:1000,value:loc.pollMs,onChange:e=>upd('pollMs',parseInt(e.target.value)||4000)})),
          h('div',{className:'field'},h('label',{className:'lbl'},'Max Log Entries'),h('input',{className:'inp inp-mono',type:'number',min:50,max:2000,step:50,value:loc.logMax,onChange:e=>upd('logMax',parseInt(e.target.value)||200)}))
        )
      )
    ),

    h('div',{className:'sg'},
      h('div',{className:'sg-head'},'Help & Onboarding'),
      h('div',{className:'sg-body',style:{padding:'12px 14px'}},
        h('p',{style:{fontSize:'var(--fsm)',color:'var(--t2)',marginBottom:12,lineHeight:'1.6'}},'Use the draggable ? button (bottom-right) to toggle help mode. Hover any dashed element to see what it does.'),
        h('div',{style:{display:'flex',gap:7,flexWrap:'wrap'}},
          h('button',{className:'btn btn-s',onClick:onShowOnboard},'\u229f Re-open Getting Started Guide'),
          h('button',{className:'btn btn-s',onClick:()=>{localStorage.removeItem(LS_OB);}},'-Reset onboarding flag')
        )
      )
    ),

    h('div',{className:'sg'},
      h('div',{className:'sg-head'},'Environment Variables Reference'),
      h('div',null,
        h('table',{className:'tbl'},
          h('thead',null,h('tr',null,h('th',null,'Variable'),h('th',null,'Purpose'),h('th',null,'When'))),
          h('tbody',null,ENV.map(([k,v,w],i)=>h('tr',{key:i},
            h('td',{style:{fontFamily:'var(--mono)',fontSize:'var(--fxs)'}},k),
            h('td',{style:{color:'var(--t2)',fontSize:'var(--fsm)'}},v),
            h('td',null,h('span',{className:'badge badge-dim',style:{fontSize:9}},w))
          )))
        )
      )
    ),

    h('div',{style:{display:'flex',gap:7,marginTop:14}},
      h('button',{className:'btn btn-p',onClick:save},saved?'\u2713 Saved':'Save'),
      h('button',{className:'btn btn-d btn-sm',onClick:()=>{localStorage.removeItem(LS);setLoc({...DEFAULTS});}},'\u21ba Reset All')
    )
  );
}

function DevToolsHub({ health, logs, onClear }) {
  const DEV_TABS = [
    { id: 'overview',    label: 'Overview',    icon: '\uD83D\uDCCA' },
    { id: 'log',         label: 'Live Log',    icon: '\uD83D\uDCE1' },
    { id: 'passports',   label: 'Passports',   icon: '\uD83D\uDEC2' },
    { id: 'swarms',      label: 'Swarms',      icon: '\uD83D\uDC1D' },
    { id: 'did',         label: 'DID & VC',    icon: '\uD83E\uDEAA' },
    { id: 'authorize',   label: 'Authorize',   icon: '\uD83D\uDD12' },
    { id: 'compliance',  label: 'Compliance',  icon: '\uD83D\uDCCB' },
    { id: 'enterprise',  label: 'Enterprise',  icon: '\uD83C\uDFE2' },
  ];

  const [sub, setSub] = React.useState('overview');

  const CONTENT = {
    overview:   h(Overview,        { health, logs }),
    log:        h(LiveLogs,        { logs, onClear }),
    passports:  h(Passports,       null),
    swarms:     h(Swarms,          null),
    did:        h(DidVc,           null),
    authorize:  h(Authorize,       null),
    compliance: h(Compliance,      null),
    enterprise: h(EnterprisePanel, null),
  };

  return h('div', null,
    h('div', {
      style: {
        display: 'flex', gap: 4, marginBottom: 16, flexWrap: 'wrap',
        borderBottom: '1px solid var(--b2)', paddingBottom: 10,
      }
    },
      DEV_TABS.map(t =>
        h('button', {
          key: t.id,
          className: 'btn btn-sm ' + (sub === t.id ? 'btn-p' : 'btn-s'),
          onClick: () => setSub(t.id),
        }, t.icon + '\u2009' + t.label)
      )
    ),
    CONTENT[sub]
  );
}
