// ─────────────────────────────────────────────────────────────────────────────
// DIRECT CONNECT — Three-phase guided integration chat
//
// Phase 1: PROBE   — scan localhost for running agent, open direct line
// Phase 2: GUIDED  — A1 suggests messages, user/auto sends them to agent,
//                    agent replies. A1 injects the integration config in bg.
// Phase 3: CONFIRM — A1 verifies integration, cuts direct line, shows
//                    the new Human → A1 → Agent flow.
// ─────────────────────────────────────────────────────────────────────────────

const DC_PHASE={PROBE:'probe',GUIDED:'guided',CONFIRM:'confirm',LIVE:'live'};

// A1's voice — what A1 says to guide the user and the agent
const A1_SCRIPTS={
  openclaw:{
    intro:"I found OpenClaw running on your machine. I'm going to open a direct line to it so we can get it connected to A1 together. You can talk to OpenClaw directly from here — I'll guide both of you through the setup.",
    step1_user:"Hi OpenClaw! I want to connect you to A1 for cryptographic authorization. Can you confirm you can hear me?",
    step1_hint:"OpenClaw should confirm it can receive messages. If it doesn't reply in 5 seconds, click Retry.",
    step2_user:"OpenClaw, I'm going to drop a .mcp.json config file into your directory now. A1 will handle it automatically — you just need to pick it up on your next restart.",
    step2_hint:"A1 is writing the config file now. You don't need to do anything.",
    step3_user:"OpenClaw, please restart and tell me when you're back up.",
    step3_hint:"After OpenClaw restarts it will load the A1 MCP server automatically.",
    confirm_user:"OpenClaw, can you confirm A1 is connected? Run: a1_check_health",
    confirm_success:"OpenClaw confirmed it can reach A1. The direct line is now closing — all future communication will go through A1's authorization layer.",
    confirm_fail:"OpenClaw hasn't confirmed the connection yet. Try sending the confirm message again.",
  },
  ironclaw:{
    intro:"I found IronClaw running on your machine. IronClaw uses a plugin system — I'll write the A1 plugin config and guide you through loading it.",
    step1_user:"IronClaw, I want to add A1 authorization. Can you respond to confirm you're ready?",
    step1_hint:"IronClaw should acknowledge. If no reply, check that it's running in API mode.",
    step2_user:"IronClaw, I'm writing an a1_plugin.toml to your config directory. It will enable A1 authorization on all tool calls.",
    step2_hint:"A1 is writing the plugin config file.",
    step3_user:"IronClaw, please reload plugins and confirm A1 is active.",
    step3_hint:"IronClaw will reload the plugin and report back.",
    confirm_user:"IronClaw status — is A1 plugin loaded?",
    confirm_success:"IronClaw confirmed A1 authorization is active. Direct line closing. All tool calls now route through A1.",
    confirm_fail:"IronClaw hasn\\'t confirmed A1 plugin load. Try sending the confirm message.",
  },
  generic:{
    intro:"I found an agent running on your machine. I\\'ll guide you through connecting it to A1.",
    step1_user:"Hello! I\\'m setting up A1 authorization. Can you confirm you\\'re ready?",
    step1_hint:"Wait for the agent to respond.",
    step2_user:"I\\'m adding A1 MCP integration now. Check your project directory for .mcp.json.",
    step2_hint:"A1 is writing the config file.",
    step3_user:"Please restart and confirm A1 is connected.",
    step3_hint:"After restart the agent will load A1 automatically.",
    confirm_user:"Confirm A1 health: call a1_check_health via MCP.",
    confirm_success:"Integration confirmed. Direct line closing. Routing through A1.",
    confirm_fail:"Not confirmed yet. Try the confirm message again.",
  },
};

function DirectConnect(){
  const{api,settings}=useContext(Ctx);
  const[phase,setPhase]=useState(DC_PHASE.PROBE);
  const[probing,setProbing]=useState(false);
  const[agents,setAgents]=useState([]);
  const[selected,setSelected]=useState(null);   // AgentEndpoint
  const[msgs,setMsgs]=useState([]);
  const[input,setInput]=useState('');
  const[sending,setSending]=useState(false);
  const[step,setStep]=useState(0);              // 0,1,2,3 within guided phase
  const[checking,setChecking]=useState(false);
  const[connected,setConnected]=useState(false);
  const[customPort,setCustomPort]=useState('');
  const msgsEnd=useRef(null);

  useEffect(()=>{
    if(msgsEnd.current)msgsEnd.current.scrollIntoView({behavior:'smooth'});
  },[msgs]);

  // ── Helpers ────────────────────────────────────────────────────────────────
  function addMsg(role,text,meta){
    setMsgs(p=>[...p,{role,text,ts:Date.now(),...(meta||{})}]);
  }

  function script(agentId){
    return A1_SCRIPTS[agentId]||A1_SCRIPTS.generic;
  }

  // ── Phase 1: PROBE ─────────────────────────────────────────────────────────
  async function probe(){
    setProbing(true);
    setAgents([]);
    const extra=customPort?[parseInt(customPort,10)]:[];
    const r=await api('POST','/v1/agents/probe',{agent_id:'all',extra_ports:extra});
    if(r.ok&&r.data.found.length>0){
      setAgents(r.data.found);
      addMsg('a1',
        r.data.found.length===1
          ?'I found '+r.data.found[0].name+' running on port '+r.data.found[0].port+'. Ready to connect it to A1.'
          :'I found '+r.data.found.length+' agents running. Pick the one you want to connect.');
    }else{
      addMsg('a1','I didn\'t find any AI agents running on common ports. Start your agent first, then click Scan Again. Or enter its port number manually.');
    }
    setProbing(false);
  }

  // ── Select agent and enter guided phase ────────────────────────────────────
  function selectAgent(agent){
    setSelected(agent);
    setStep(0);
    setPhase(DC_PHASE.GUIDED);
    const sc=script(agent.agent_id);
    addMsg('a1',sc.intro);
    // Small delay then show first suggestion
    setTimeout(()=>addMsg('a1',
      'Start by sending this message to '+agent.name+'. I\'ll suggest what to say at each step.',
      {suggest:sc.step1_user,hint:sc.step1_hint}
    ),600);
  }

  // ── Send a message through the relay to the selected agent ─────────────────
  async function sendToAgent(text){
    if(!selected||!text.trim())return;
    setSending(true);
    addMsg('user',text);
    setInput('');

    // A1 status: "relaying…"
    addMsg('a1','Relaying to '+selected.name+'…',{loading:true});

    const r=await api('POST','/v1/agents/relay',{
      base_url:selected.base_url,
      chat_path:selected.chat_path,
      api_style:selected.api_style,
      message:text,
    });

    // Remove loading msg and add real reply
    setMsgs(p=>p.filter(m=>!m.loading));

    if(r.ok&&r.data.reply){
      addMsg('agent',r.data.reply,{agentName:selected.name,latency:r.data.latency_ms});
      // Auto-advance steps
      advanceStep();
    }else{
      addMsg('a1','No reply from '+selected.name+'. '+(r.data?.error||'Check that it\'s running and its chat API is enabled.'),{error:true});
    }
    setSending(false);
  }

  // Write config + advance through guided steps
  async function advanceStep(){
    const newStep=step+1;
    setStep(newStep);
    const sc=script(selected.agent_id);

    if(newStep===1){
      // Step 1 done (agent heard us) — now write the config file automatically
      addMsg('a1','Writing A1 integration config to '+selected.name+'\'s directory now…');
      const r=await api('POST','/v1/agents/connect',{agent_id:selected.agent_id,install_path:selected.base_url});
      if(r.ok&&r.data.connected){
        addMsg('a1','Config written: '+r.data.message+'\n\nFiles: '+(r.data.files_written||[]).join(', '));
        setTimeout(()=>addMsg('a1',
          'Now ask '+selected.name+' to restart so it picks up the new config.',
          {suggest:sc.step3_user,hint:sc.step3_hint}
        ),400);
      }else{
        addMsg('a1','Config write failed: '+(r.data?.message||'unknown error')+'. Try the Connect Agents tab instead.',{error:true});
      }
    }else if(newStep===2){
      // Agent restarted — check integration
      addMsg('a1','Checking if '+selected.name+' has picked up A1…',{loading:true});
      checkIntegration();
    }else if(newStep>=3){
      // Push confirm message suggestion
      const sc2=script(selected.agent_id);
      addMsg('a1','Almost there. Send this to confirm A1 is live:',{suggest:sc2.confirm_user,hint:'A1 will verify the response.'});
    }
  }

  // ── Integration check ──────────────────────────────────────────────────────
  async function checkIntegration(){
    setChecking(true);
    setMsgs(p=>p.filter(m=>!m.loading));

    const r=await api('GET','/v1/agents/integration-check?agent_id='+(selected?.agent_id||'custom'));
    setChecking(false);

    if(r.ok&&r.data.integrated){
      // SUCCESS — close direct line
      setPhase(DC_PHASE.CONFIRM);
      setConnected(true);
      addMsg('a1',script(selected.agent_id).confirm_success);
      // Show the "direct line cut" visual after a moment
      setTimeout(()=>setPhase(DC_PHASE.LIVE),2000);
    }else{
      setMsgs(p=>p.filter(m=>!m.loading));
      addMsg('a1',script(selected.agent_id).confirm_fail,{
        suggest:script(selected.agent_id).confirm_user,
        hint:'Send this to '+selected.name+' — it should call a1_check_health via MCP.'
      });
    }
  }

  // ── Suggested message bar ──────────────────────────────────────────────────
  const lastSuggest=msgs.filter(m=>m.suggest).slice(-1)[0];

  // ─── RENDER ────────────────────────────────────────────────────────────────
  return h('div',{style:{paddingBottom:40,width:'100%'}},

    // Header
    h('div',{style:{marginBottom:16}},
      h('h2',{style:{fontSize:18,fontWeight:700,marginBottom:4}},'⚡ Direct Connect'),
      h('p',{style:{color:'var(--t2)',fontSize:'var(--fsm)',lineHeight:1.6}},
        'Talk to your AI agent directly from A1 Studio. I\'ll guide you through the setup step by step, write the config automatically, then close the direct line once integration is confirmed.')),

    // Phase indicator
    h('div',{style:{display:'flex',gap:8,marginBottom:16,alignItems:'center'}},
      ['probe','guided','confirm','live'].map((p,i)=>
        h('div',{key:p,style:{display:'flex',alignItems:'center',gap:6}},
          h('div',{style:{
            width:28,height:28,borderRadius:'50%',display:'flex',alignItems:'center',
            justifyContent:'center',fontSize:12,fontWeight:700,
            background: phase===p?'var(--green)': ['probe','guided','confirm','live'].indexOf(phase)>i ?'rgba(34,197,94,.25)':'var(--b1)',
            color: phase===p?'#000': ['probe','guided','confirm','live'].indexOf(phase)>i ?'var(--green)':'var(--t2)',
            transition:'all .3s'
          }},i+1),
          h('span',{style:{fontSize:'var(--fxs)',color:phase===p?'var(--text)':'var(--t2)',fontWeight:phase===p?600:400}},
            {probe:'Scan',guided:'Guide',confirm:'Verify',live:'Live'}[p]),
          i<3&&h('div',{style:{width:24,height:1,background:'var(--b1)'}})
        )
      )),

    // ── PHASE: PROBE ──────────────────────────────────────────────────────────
    phase===DC_PHASE.PROBE&&h('div',null,
      h('div',{className:'sg'},
        h('div',{className:'sg-head'},'1. Find your running AI agent'),
        h('div',{className:'sg-body'},
          h('p',{style:{color:'var(--t2)',fontSize:'var(--fsm)',marginBottom:12,lineHeight:1.6}},
            'Make sure your AI agent (OpenClaw, IronClaw, etc.) is running, then click Scan. A1 will find it automatically.'),
          h('div',{style:{display:'flex',gap:8,marginBottom:12}},
            h('button',{className:'btn btn-p',onClick:probe,disabled:probing},
              probing?'Scanning\u2026':'\ud83d\udd0d Scan for running agents'),
            h('input',{className:'inp inp-mono',style:{width:100},
              placeholder:'port e.g. 3000',
              value:customPort,onChange:e=>setCustomPort(e.target.value),
              title:'Custom port if your agent runs on a non-standard port'})),
          agents.length>0&&h('div',{style:{display:'flex',flexDirection:'column',gap:6}},
            agents.map(a=>h('div',{key:a.port,
              className:'ag-card'+(selected?.port===a.port?' connected':''),
              style:{cursor:'pointer'},
              onClick:()=>selectAgent(a)
            },
              h('div',{className:'ag-icon'},
                {openclaw:'\ud83e\udd85',ironclaw:'\ud83e\uddb6',openai:'\ud83d\udfe2',anthropic:'\ud83e\udd16',generic:'\u2699\ufe0f'}[a.agent_id]||'\u2699\ufe0f'),
              h('div',{className:'ag-info'},
                h('div',{className:'ag-name'},a.name,
                  h('span',{className:'ag-badge found'},a.api_style)),
                h('div',{className:'ag-desc'},'localhost:'+a.port+' \u2022 '+a.chat_path),
                a.version&&h('div',{className:'ag-path'},'v'+a.version)),
              h('div',{className:'ag-actions'},
                h('button',{className:'btn btn-p btn-sm',
                  onClick:e=>{e.stopPropagation();selectAgent(a);}},
                  'Connect this one \u2192'))
            ))))),

      msgs.length>0&&h('div',{className:'wiz-info',style:{marginTop:12}},
        h('span',{style:{fontSize:18}},'\ud83e\udd16'),
        h('div',null,
          msgs.filter(m=>m.role==='a1').map((m,i)=>
            h('div',{key:i,style:{color:'var(--t2)',fontSize:'var(--fsm)',lineHeight:1.6,marginBottom:4}},m.text))))),

    // ── PHASE: GUIDED ─────────────────────────────────────────────────────────
    (phase===DC_PHASE.GUIDED||phase===DC_PHASE.CONFIRM)&&selected&&h('div',null,

      // Status bar
      h('div',{className:'status-bar',style:{marginBottom:12}},
        h('div',{className:'status-dot green pulse'}),
        h('span',null,'Direct line open to ',h('strong',null,selected.name),
          ' on localhost:'+selected.port),
        h('span',{style:{color:'var(--t2)',fontSize:'var(--fxs)',marginLeft:'auto'}},
          'via '+selected.api_style+' API')),

      // Chat window
      h('div',{className:'ai-chat',style:{height:360}},
        h('div',{className:'ai-msgs',ref:msgsEnd},
          msgs.map((m,i)=>
            m.loading
              ?h('div',{key:i,className:'ai-msg assistant'},'\u22ef relaying\u2026')
              :m.role==='user'
                ?h('div',{key:i,className:'ai-msg user'},m.text)
                :m.role==='agent'
                  ?h('div',{key:i,style:{alignSelf:'flex-start',maxWidth:'88%',marginBottom:8}},
                      h('div',{style:{fontSize:'var(--fxs)',color:'var(--t2)',fontFamily:'var(--mono)',marginBottom:2}},
                        '\ud83e\udd16 '+selected.name+(m.latency?' \u00b7 '+m.latency+'ms':'')),
                      h('div',{className:'ai-msg assistant'},m.text))
                  :m.role==='a1'
                    ?h('div',{key:i,style:{alignSelf:'flex-start',maxWidth:'88%',marginBottom:8}},
                        h('div',{style:{fontSize:'var(--fxs)',color:'var(--green)',fontFamily:'var(--mono)',marginBottom:2}},
                          '\ud83d\udee1\ufe0f A1'),
                        h('div',{style:{
                          background:'rgba(34,197,94,.07)',border:'1px solid rgba(34,197,94,.2)',
                          borderRadius:'10px 10px 10px 2px',padding:'8px 12px',
                          fontSize:'var(--fsm)',lineHeight:1.6,color:'var(--text)',
                          whiteSpace:'pre-wrap'
                        }},m.text),
                        m.hint&&h('div',{style:{fontSize:'var(--fxs)',color:'var(--t2)',marginTop:4,fontStyle:'italic'}},
                          m.hint))
                    :null
          ),
          sending&&h('div',{className:'ai-typing'},h('span'),h('span'),h('span'))),

        h('div',{className:'ai-input-row'},
          h('textarea',{className:'ai-inp',rows:1,
            placeholder:'Type a message to '+selected.name+'\u2026',
            value:input,
            onChange:e=>{setInput(e.target.value);e.target.style.height='auto';e.target.style.height=Math.min(e.target.scrollHeight,100)+'px';},
            onKeyDown:e=>{if(e.key==='Enter'&&!e.shiftKey){e.preventDefault();sendToAgent(input);}},
            disabled:sending}),
          h('button',{className:'btn btn-p btn-sm',
            onClick:()=>sendToAgent(input),
            disabled:sending||!input.trim()},'Send'))),

      // Suggested message bar
      lastSuggest&&h('div',{style:{marginTop:8,padding:'10px 14px',background:'var(--s2)',border:'1px solid rgba(34,197,94,.2)',borderRadius:'var(--r)',display:'flex',gap:10,alignItems:'flex-start'}},
        h('div',{style:{flex:1}},
          h('div',{style:{fontSize:'var(--fxs)',color:'var(--green)',fontWeight:600,marginBottom:4}},
            '\ud83d\udca1 A1 suggests sending this:'),
          h('div',{style:{fontFamily:'var(--mono)',fontSize:'var(--fxs)',color:'var(--t2)',lineHeight:1.6}},
            lastSuggest.suggest)),
        h('div',{style:{display:'flex',gap:6,flexShrink:0}},
          h('button',{className:'btn btn-s btn-sm',
            onClick:()=>setInput(lastSuggest.suggest)},'Edit first'),
          h('button',{className:'btn btn-p btn-sm',
            disabled:sending,
            onClick:()=>sendToAgent(lastSuggest.suggest)},
            sending?'Sending\u2026':'Send \u2192'))),

      // Manual integration check
      phase===DC_PHASE.GUIDED&&h('div',{style:{marginTop:10,display:'flex',gap:8}},
        h('button',{className:'btn btn-s btn-sm',onClick:checkIntegration,disabled:checking||sending},
          checking?'Checking\u2026':'\u2714 Check integration now'),
        h('span',{style:{color:'var(--t2)',fontSize:'var(--fxs)',alignSelf:'center'}},
          'Click after '+selected.name+' restarts with A1 loaded'))),

    // ── PHASE: LIVE ───────────────────────────────────────────────────────────
    phase===DC_PHASE.LIVE&&selected&&h('div',null,
      h('div',{style:{padding:24,borderRadius:'var(--r)',border:'1px solid rgba(34,197,94,.3)',background:'rgba(34,197,94,.05)',textAlign:'center',marginBottom:16}},
        h('div',{style:{fontSize:48,marginBottom:12}},'\ud83c\udf89'),
        h('div',{style:{fontSize:18,fontWeight:700,marginBottom:8,color:'var(--green)'}},
          selected.name+' is now protected by A1'),
        h('div',{style:{color:'var(--t2)',fontSize:'var(--fsm)',lineHeight:1.7}},
          'The direct line is closed. All future communication flows through A1\'s authorization layer. Every tool call is now cryptographically verified.')),

      // Flow diagram: Human → A1 → Agent
      h('div',{style:{display:'flex',alignItems:'center',justifyContent:'center',gap:0,marginBottom:20}},
        h('div',{style:{padding:'12px 16px',border:'1px solid var(--b1)',borderRadius:'var(--r)',background:'var(--s2)',textAlign:'center',minWidth:110}},
          h('div',{style:{fontSize:20,marginBottom:4}},'\ud83e\uddd1'),
          h('div',{style:{fontSize:'var(--fxs)',fontWeight:600}},'You'),
          h('div',{style:{fontSize:'var(--fxs)',color:'var(--t2)'}},'Human')),
        h('div',{style:{flex:1,height:2,background:'var(--green)',position:'relative',minWidth:40}},
          h('div',{style:{position:'absolute',top:-8,left:'50%',transform:'translateX(-50%)',fontSize:10,color:'var(--green)',fontFamily:'var(--mono)',whiteSpace:'nowrap'}},
            'authorized')),
        h('div',{style:{padding:'12px 16px',border:'2px solid var(--green)',borderRadius:'var(--r)',background:'rgba(34,197,94,.08)',textAlign:'center',minWidth:110}},
          h('div',{style:{fontSize:20,marginBottom:4}},'\ud83d\udee1\ufe0f'),
          h('div',{style:{fontSize:'var(--fxs)',fontWeight:700,color:'var(--green)'}},'A1'),
          h('div',{style:{fontSize:'var(--fxs)',color:'var(--green)'}},'middleware')),
        h('div',{style:{flex:1,height:2,background:'var(--green)',position:'relative',minWidth:40}},
          h('div',{style:{position:'absolute',top:-8,left:'50%',transform:'translateX(-50%)',fontSize:10,color:'var(--green)',fontFamily:'var(--mono)',whiteSpace:'nowrap'}},
            'verified')),
        h('div',{style:{padding:'12px 16px',border:'1px solid var(--b1)',borderRadius:'var(--r)',background:'var(--s2)',textAlign:'center',minWidth:110}},
          h('div',{style:{fontSize:20,marginBottom:4}},
            {openclaw:'\ud83e\udd85',ironclaw:'\ud83e\uddb6',openai:'\ud83d\udfe2'}[selected.agent_id]||'\ud83e\udd16'),
          h('div',{style:{fontSize:'var(--fxs)',fontWeight:600}},selected.name),
          h('div',{style:{fontSize:'var(--fxs)',color:'var(--t2)'}},'localhost:'+selected.port))),

      // Next steps
      h('div',{className:'sg'},
        h('div',{className:'sg-head'},'What happens now'),
        h('div',{className:'sg-body'},
          h('div',{style:{display:'flex',flexDirection:'column',gap:8}},
            [
              ['\u2713','Every tool call is verified','Before '+selected.name+' takes any action, A1 checks the delegation chain and capability scope.'],
              ['\u2713','ProvableReceipts generated','Every authorized action produces a tamper-evident receipt in the Live Log.'],
              ['\u2713','You can revoke instantly','Go to My Passports & Agents to revoke access immediately if needed.'],
              ['\u2713','Check the Live Log','Switch to the Live Log tab to see real-time authorization events as '+selected.name+' works.'],
            ].map(([icon,title,desc],i)=>
              h('div',{key:i,style:{display:'flex',gap:10,alignItems:'flex-start'}},
                h('div',{style:{color:'var(--green)',fontWeight:700,fontSize:'var(--fsm)',flexShrink:0,marginTop:2}},icon),
                h('div',null,
                  h('div',{style:{fontWeight:600,fontSize:'var(--fsm)'}},title),
                  h('div',{style:{color:'var(--t2)',fontSize:'var(--fxs)',marginTop:2}},desc))))))))
  );
}
