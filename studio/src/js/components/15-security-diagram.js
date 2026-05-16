// ─────────────────────────────────────────────────────────────────────────────
// 15-security-diagram.js — Animated wiring diagrams
// Three views: Attack Simulation / Task Execution / Agent Delegation
// ─────────────────────────────────────────────────────────────────────────────

// ── Attack data (accurate to real AI agent attack surface) ────────────────────

const ATTACKS = [
  {
    id: 'injection', target: 'agent',
    icon: '[VULN]', label: 'Prompt Injection',
    desc: 'Malicious payload in RAG pipeline overwrites system prompt, hijacking tool execution and parameter mapping.',
    cols: {
      none:  { blocks: false, reason: 'LLM outputs raw JSON; backend executes blindly with zero intent validation.' },
      token: { blocks: false, reason: 'Token authenticates identity, not the cryptographic validity of the specific generation.' },
      a1:    { blocks: true,  reason: 'Pre-declared schema enforced per-call. Out-of-scope executions rejected at the gateway.' },
    },
  },
  {
    id: 'stolen', target: 'operator',
    icon: '[EXF]', label: 'Credential Exfiltration',
    desc: 'Attacker extracts API keys via environment variable leaks, CI/CD exposure, or SSRF against metadata endpoints.',
    cols: {
      none:  { blocks: false, reason: 'No credentials required. Endpoints accept unauthenticated traffic.' },
      token: { blocks: false, reason: 'Static bearer tokens provide persistent access. No hardware binding.' },
      a1:    { blocks: true,  reason: 'Ed25519 keypair is hardware-bound. Token bytes useless without the private enclave key.' },
    },
  },
  {
    id: 'delegation', target: 'agent',
    icon: '[ESC]', label: 'Privilege Escalation',
    desc: 'Low-privilege sub-agent attempts to grant itself operational capabilities beyond parent delegation limits.',
    cols: {
      none:  { blocks: false, reason: 'Unrestricted flat access layer. No structural concept of delegation depth.' },
      token: { blocks: false, reason: 'Tokens encode flat identity. They cannot mathematically narrow capability scope.' },
      a1:    { blocks: true,  reason: 'Cryptographic Narrowing: Child capability mask must be a provable subset of parent mask.' },
    },
  },
  {
    id: 'replay', target: 'network',
    icon: '[RPL]', label: 'Replay Attack',
    desc: 'Adversary captures valid agent payload and re-transmits it to force duplicate state changes or transactions.',
    cols: {
      none:  { blocks: false, reason: 'Backend blindly processes duplicate requests indefinitely.' },
      token: { blocks: false, reason: 'JWT remains valid for entire TTL window, allowing unlimited replay loops.' },
      a1:    { blocks: true,  reason: 'Nonce + intent hash consumed per execution. Duplicates rejected in O(1).' },
    },
  },
  {
    id: 'impersonation', target: 'operator',
    icon: '[SPOOF]', label: 'Agent Impersonation',
    desc: 'Attacker bypasses orchestration, forging direct requests to backend infrastructure disguised as a trusted agent.',
    cols: {
      none:  { blocks: false, reason: 'Infrastructure accepts arbitrary traffic without caller verification.' },
      token: { blocks: false, reason: 'Stolen JWT allows complete impersonation with identical audit trails.' },
      a1:    { blocks: true,  reason: 'ZK Receipt requires unbroken chain-of-custody. Forged requests fail cryptographic verification.' },
    },
  },
  {
    id: 'mitm', target: 'network',
    icon: '[TAMPER]', label: 'Payload Tampering',
    desc: 'In-transit modification of signed agent parameters (e.g., swapping payment destination or database query).',
    cols: {
      none:  { blocks: false, reason: 'No payload integrity validation.' },
      token: { blocks: false, reason: 'Token secures transport, not the specific JSON payload bytes.' },
      a1:    { blocks: true,  reason: 'Payload hash embedded in ZK proof. Single-bit modifications invalidate the transaction.' },
    },
  },
  {
    id: 'autonomy', target: 'agent',
    icon: '[DEV]', label: 'Scope Deviation',
    desc: 'Agent reasoning loop diverges, executing unauthorized destructive actions (wiping data, mass API calls).',
    cols: {
      none:  { blocks: false, reason: 'Agent has unrestricted API access. Trust is based purely on LLM reasoning.' },
      token: { blocks: false, reason: 'Token provides broad access. Agent decides what to call, not a strict policy.' },
      a1:    { blocks: true,  reason: 'Capabilities permanently sealed at passport issuance. Unauthorized tools cryptographically drop.' },
    },
  },
  {
    id: 'confused_deputy', target: 'network',
    icon: '[DEP]', label: 'Confused Deputy',
    desc: 'Malicious instructions embedded in untrusted user data trick a high-privilege agent into executing actions.',
    cols: {
      none:  { blocks: false, reason: 'No boundary between orchestrator privilege and raw user data.' },
      token: { blocks: false, reason: 'Orchestrator token executes all downstream calls uniformly.' },
      a1:    { blocks: true,  reason: 'Delegation chain validation requires explicit human authorization for state changes.' },
    },
  },
];

const COLS_META = [
  { label: 'No Auth',    sub: 'No identity check',   color: '#ef4444', bg: 'rgba(239,68,68,.08)',  id: 'none'  },
  { label: 'Token Auth', sub: 'API key / Bearer JWT', color: '#f59e0b', bg: 'rgba(245,158,11,.08)', id: 'token' },
  { label: 'With A1',   sub: 'Cryptographic chain',   color: '#22c55e', bg: 'rgba(34,197,94,.08)',  id: 'a1'   },
];

// ── Shared drawing utilities ──────────────────────────────────────────────────

function gC(dark) {
  return {
    bg: dark?'#09090b':'#fafafa', fg: dark?'#f4f4f5':'#18181b',
    fg2: dark?'#a1a1aa':'#71717a', nodeBg: dark?'#18181b':'#ffffff',
    nodeBorder: dark?'#27272a':'#e4e4e7', wire: dark?'#27272a':'#e4e4e7',
    wireFlow: dark?'#52525b':'#a1a1aa',
    packet:'#38bdf8', accent:'#818cf8',
    green:'#10b981', amber:'#f59e0b', red:'#f43f5e',
  };
}

function rr(ctx, x, y, w, h, r) {
  // Sharp enterprise borders, minor smoothing
  ctx.beginPath();
  r = 2; // Override rounding
  ctx.moveTo(x+r,y); ctx.lineTo(x+w-r,y); ctx.lineTo(x+w,y);
  ctx.lineTo(x+w,y+h-r); ctx.lineTo(x+w,y+h);
  ctx.lineTo(x+r,y+h); ctx.lineTo(x,y+h);
  ctx.lineTo(x,y+r); ctx.lineTo(x,y);
  ctx.closePath();
}

function dot(ctx, x, y, c) {
  // Enterprise block packets instead of circles
  ctx.fillStyle=c; ctx.fillRect(x-3, y-3, 6, 6);
}

function chip(ctx, mx, my, text, stroke, bg, C) {
  const w = Math.max(52, text.length*5.6+14);
  ctx.fillStyle=bg||C.bg; ctx.fillRect(mx-w/2,my-8,w,16);
  ctx.strokeStyle=stroke; ctx.lineWidth=1; ctx.strokeRect(mx-w/2,my-8,w,16);
  ctx.fillStyle=stroke; ctx.font='600 7px "IBM Plex Mono",monospace';
  ctx.textAlign='center'; ctx.textBaseline='middle'; ctx.fillText(text,mx,my+1);
}

function dotGrid(ctx, W, H, dark) {
  ctx.fillStyle=dark?'rgba(255,255,255,0.025)':'rgba(0,0,0,0.025)';
  for(let i=0;i<W;i+=14) for(let j=0;j<H;j+=14) {
    ctx.fillRect(i,j,1,1);
    if(i%70===0 && j%70===0) { ctx.fillText('+', i-2, j+2); }
  }
}

// ── ATTACK SIMULATION ─────────────────────────────────────────────────────────

function AttackDiagram() {
  const ref=useRef(null), raf=useRef(null);
  const [attack,setAttack]=useState(null);
  const dark=document.documentElement.getAttribute('data-theme')!=='light';
  const W=860,H=480;
  const CX=[145,380,615], CW=195, NW=167, NH=58;
  const NY={op:65,ag:185,gw:305,sv:415};

  function draw(ctx,t) {
    const C=gC(dark), spd=t/1000;
    ctx.fillStyle=C.bg; ctx.fillRect(0,0,W,H);
    dotGrid(ctx,W,H,dark);

    CX.forEach((cx,ci)=>{
      const m=COLS_META[ci];
      ctx.fillStyle=C.nodeBg; ctx.globalAlpha=0.4; ctx.fillRect(cx-CW/2,36,CW,H-40); ctx.globalAlpha=1;
      ctx.strokeStyle=dark?'#27272a':'#e5e5e5'; ctx.lineWidth=1; ctx.strokeRect(cx-CW/2,36,CW,H-40);
      ctx.fillStyle=m.color; ctx.fillRect(cx-CW/2,36,CW,22);
      ctx.font='700 9px "IBM Plex Mono",monospace'; ctx.textAlign='center'; ctx.textBaseline='middle';
      ctx.fillStyle=dark?'#000':'#fff'; ctx.fillText(m.label.toUpperCase(),cx,47);
      ctx.font='400 7.5px "IBM Plex Mono",monospace'; ctx.fillStyle=C.fg2; ctx.fillText(m.sub,cx,64);
    });

    const isAtk=!!attack;

    CX.forEach((cx,ci)=>{
      const m=COLS_META[ci];
      const blocked=isAtk&&attack.cols[m.id].blocks;
      const passed=isAtk&&!attack.cols[m.id].blocks;

      const wire=(y0,y1,active,off=0)=>{
        ctx.beginPath(); ctx.moveTo(cx,y0); ctx.lineTo(cx,y1);
        ctx.strokeStyle=active?C.packet:C.wire; ctx.lineWidth=active?2:1;
        ctx.setLineDash(active?[4,7]:[3,7]); ctx.lineDashOffset=-(spd*55)-off; ctx.stroke(); ctx.setLineDash([]);
      };

      wire(NY.op+NH/2,NY.ag-NH/2,!isAtk,0);
      wire(NY.ag+NH/2,NY.gw-NH/2,!isAtk,25);
      wire(NY.gw+NH/2,NY.sv-NH/2,!isAtk&&!blocked,50);

      const node=(ny,id,title,s1,s2,state)=>{
        const x=cx-NW/2,y=ny-NH/2;
        let bdr=C.nodeBorder,bg=C.nodeBg;
        if(state==='attacked'){bdr='#ef4444';bg='rgba(239,68,68,.06)';}
        if(state==='shielded'){bdr='#22c55e';bg='rgba(34,197,94,.06)';}
        if(state==='compromised'){bdr='#ef4444';bg='rgba(239,68,68,.18)';}
        rr(ctx,x,y,NW,NH,6); ctx.fillStyle=bg; ctx.fill();
        ctx.strokeStyle=bdr; ctx.lineWidth=state?2:1; ctx.stroke();
        ctx.fillStyle=bdr; ctx.fillRect(x,y,NW,14);
        ctx.fillStyle=state?(dark?'#000':'#fff'):C.fg;
        ctx.font='700 8px "IBM Plex Mono",monospace'; ctx.textAlign='left'; ctx.fillText('['+id+'] '+title,x+6,y+7);
        [s1,s2].forEach((s,i)=>{
          ctx.strokeStyle=C.wire; ctx.lineWidth=1; ctx.strokeRect(x+6,y+20+i*16,NW-12,12);
          ctx.fillStyle=state==='attacked'?C.red:state==='shielded'?C.green:C.fg2;
          ctx.font='400 7.5px "IBM Plex Mono",monospace'; ctx.textAlign='left'; ctx.fillText(s,x+10,y+26+i*16);
        });
      };

      let gwT='OPEN PROXY',g1='Unrestricted Route',g2='No Validation';
      if(m.id==='token'){gwT='API GATEWAY';g1='JWT Expiry Check';g2='Bearer Validation';}
      if(m.id==='a1')  {gwT='A1 ZERO-TRUST';g1='ZK Receipt Verify';g2='Capability Matrix';}

      node(NY.op,'OP','OPERATOR ENV','Auth Context','Local Secrets',isAtk&&attack.target==='operator'?'attacked':null);
      node(NY.ag,'AG','SWARM AGENT','Prompt Engine','Tool Router',isAtk&&attack.target==='agent'?'attacked':null);
      node(NY.gw,'GW',gwT,g1,g2,isAtk&&attack.target==='network'?(blocked?'shielded':'attacked'):(blocked?'shielded':null));
      node(NY.sv,'RS','TARGET INFRA','PostgreSQL DB','Internal API',passed?'compromised':null);

      if(!isAtk){
        const pay={none:'{cmd}',token:'{jwt:ey..}',a1:'{zk:0x2a}'};
        const p=pay[m.id];
        [[NY.op+NH/2,NY.ag-NH/2],[NY.ag+NH/2,NY.gw-NH/2],[NY.gw+NH/2,NY.sv-NH/2]].forEach(([y0,y1],wi)=>{
          for(let pk=0;pk<3;pk++){
            const pct=((spd*0.45+pk/3+wi*0.15)%1);
            const py=y0+pct*(y1-y0);
            chip(ctx,cx,py,p,m.id==='a1'?C.accent:C.packet,null,C);
          }
        });
      }

      if(isAtk){
        let atkY=NY.ag;
        if(attack.target==='operator')atkY=NY.op;
        if(attack.target==='network')atkY=NY.gw-42;
        const pct=((spd*1.6)%1), x0=18, x1=cx-NW/2+10;
        const curX=x0+pct*(x1-x0);
        ctx.beginPath(); ctx.moveTo(x0,atkY); ctx.lineTo(x1,atkY);
        ctx.strokeStyle='rgba(239,68,68,.18)'; ctx.lineWidth=1; ctx.stroke();
        ctx.beginPath(); ctx.moveTo(x0,atkY); ctx.lineTo(curX,atkY);
        ctx.strokeStyle='#ef4444'; ctx.lineWidth=2;
        ctx.setLineDash([4,4]); ctx.lineDashOffset=-(spd*90); ctx.stroke(); ctx.setLineDash([]);
        if(!blocked||pct<0.88){
          chip(ctx,curX,atkY,'['+attack.id.slice(0,7).toUpperCase()+']','#ef4444',null,C);
        }
        if(blocked){
          const bx=cx-NW/2+6;
          ctx.strokeStyle='#22c55e'; ctx.lineWidth=2.5;
          ctx.beginPath(); ctx.moveTo(bx,atkY-18); ctx.lineTo(bx,atkY+18); ctx.stroke();
          ctx.strokeStyle='rgba(34,197,94,.35)'; ctx.lineWidth=1.5;
          ctx.beginPath(); ctx.moveTo(bx+5,atkY-24); ctx.lineTo(bx+5,atkY+24); ctx.stroke();
          ctx.fillStyle='#22c55e'; ctx.font='700 8px "IBM Plex Mono",monospace';
          ctx.textAlign='left'; ctx.fillText('BLOCK: CRYPTO_REJECT',cx-18,atkY-28);
        }
        if(passed){
          const pp=((spd*0.9)%1), py=atkY+pp*(NY.sv-atkY);
          ctx.strokeStyle='rgba(239,68,68,.6)'; ctx.lineWidth=2;
          ctx.beginPath(); ctx.moveTo(cx,atkY); ctx.lineTo(cx,py); ctx.stroke();
          if(pp>0.85){
            ctx.globalAlpha=Math.min(1,(pp-0.85)*7);
            ctx.fillStyle='rgba(239,68,68,.2)'; ctx.fillRect(cx-NW/2+8,NY.sv-NH/2+8,NW-16,NH-16);
            ctx.globalAlpha=1;
            ctx.fillStyle='#ef4444'; ctx.font='700 9px "IBM Plex Mono",monospace';
            ctx.textAlign='center'; ctx.fillText('CRITICAL_COMPROMISE',cx,NY.sv);
          }
        }
      }
    });

    if(isAtk){
      let atkY=NY.ag;
      if(attack.target==='operator')atkY=NY.op;
      if(attack.target==='network')atkY=NY.gw-42;
      ctx.fillStyle=dark?'#0a0a0a':'#fff'; ctx.fillRect(8,atkY-22,82,44);
      ctx.strokeStyle='#ef4444'; ctx.lineWidth=1.5; ctx.strokeRect(8,atkY-22,82,44);
      ctx.fillStyle='#ef4444'; ctx.fillRect(8,atkY-22,82,13);
      ctx.fillStyle=dark?'#000':'#fff'; ctx.font='700 7px "IBM Plex Mono",monospace';
      ctx.textAlign='left'; ctx.fillText('VEC: '+attack.id.toUpperCase(),13,atkY-15);
      ctx.fillStyle='#ef4444'; ctx.font='400 7px "IBM Plex Mono",monospace';
      ctx.fillText('> init exploit',13,atkY-2); ctx.fillText('> bypass auth',13,atkY+9);
      if(Math.floor(t/200)%2===0) ctx.fillRect(72,atkY+4,4,8);
    }

    ctx.font='700 8.5px "IBM Plex Mono",monospace'; ctx.fillStyle=gC(dark).fg2;
    ctx.textAlign='left'; ctx.textBaseline='middle';
    const lx=CX[2]+CW/2+14;
    ['L1: OP_ENV','L2: AGENT_ORCH','L3: TRUST_GATE','L4: TARGET_INFRA'].forEach((l,i)=>{
      const y=[NY.op,NY.ag,NY.gw,NY.sv][i];
      ctx.strokeStyle=gC(dark).wire; ctx.lineWidth=1;
      ctx.beginPath(); ctx.moveTo(CX[2]+CW/2+4,y); ctx.lineTo(lx-4,y); ctx.stroke();
      ctx.fillText(l,lx,y);
    });
  }

  useEffect(()=>{
    const canvas=ref.current; if(!canvas) return;
    const dpr=window.devicePixelRatio||1;
    canvas.width=W*dpr; canvas.height=H*dpr;
    const ctx=canvas.getContext('2d'); ctx.scale(dpr,dpr);
    let start=null;
    const frame=ts=>{if(!start)start=ts; draw(ctx,ts-start); raf.current=requestAnimationFrame(frame);};
    raf.current=requestAnimationFrame(frame);
    return ()=>cancelAnimationFrame(raf.current);
  },[attack,dark]);

  return h('div',{style:{display:'flex',flexDirection:'column',gap:0}},
    h('div',{style:{width:'100%',background:dark?'#0a0a0a':'#f4f4f5',borderRadius:'var(--r) var(--r) 0 0',border:'1px solid var(--b1)',borderBottom:'none',overflow:'hidden'}},
      h('canvas',{ref,style:{width:'100%',height:'auto',display:'block',maxWidth:W+'px'}})
    ),
    h('div',{style:{display:'flex',gap:6,flexWrap:'wrap',padding:'10px 12px',background:'var(--s1)',border:'1px solid var(--b1)',borderRadius:'0 0 var(--r) var(--r)'}},
      h('span',{style:{fontSize:'var(--fxs)',color:'var(--t2)',fontWeight:600,alignSelf:'center',marginRight:4}},'Simulate attack:'),
      ATTACKS.map(atk=>h('button',{key:atk.id,onClick:()=>setAttack(a=>a?.id===atk.id?null:atk),style:{padding:'4px 10px',fontSize:'var(--fxs)',cursor:'pointer',border:'1px solid '+(attack?.id===atk.id?'#ef4444':'var(--b3)'),borderRadius:20,background:attack?.id===atk.id?'rgba(239,68,68,.12)':'var(--b1)',color:attack?.id===atk.id?'#ef4444':'var(--t2)',fontWeight:attack?.id===atk.id?700:400,transition:'all .15s'}},atk.icon+' '+atk.label)),
      attack&&h('button',{onClick:()=>setAttack(null),style:{marginLeft:'auto',fontSize:'var(--fxs)',color:'var(--t3)',background:'none',border:'none',cursor:'pointer',padding:'4px 8px'}},'✕ clear')
    ),
    attack&&h('div',{style:{marginTop:10,padding:'12px 16px',border:'1px solid rgba(239,68,68,.25)',borderRadius:'var(--r)',background:'rgba(239,68,68,.04)'}},
      h('div',{style:{fontWeight:700,fontSize:'var(--fsm)',color:'#ef4444',marginBottom:6}},attack.icon+' '+attack.label),
      h('div',{style:{color:'var(--t2)',fontSize:'var(--fxs)',lineHeight:1.7,marginBottom:10}},attack.desc),
      h('div',{style:{display:'grid',gridTemplateColumns:'1fr 1fr 1fr',gap:8}},
        COLS_META.map(col=>{
          const info=attack.cols[col.id];
          return h('div',{key:col.id,style:{padding:'8px 10px',borderRadius:'var(--r)',background:info.blocks?'rgba(34,197,94,.07)':'rgba(239,68,68,.07)',border:'1px solid '+(info.blocks?'rgba(34,197,94,.25)':'rgba(239,68,68,.25)')}},
            h('div',{style:{fontWeight:700,fontSize:'var(--fxs)',marginBottom:4,color:col.color}},col.label),
            h('div',{style:{fontSize:'var(--fxs)',fontWeight:700,marginBottom:3,color:info.blocks?'#22c55e':'#ef4444'}},info.blocks?'🛡 Blocked':'✗ Attack succeeds'),
            h('div',{style:{fontSize:'var(--fxs)',color:'var(--t2)',lineHeight:1.5}},info.reason)
          );
        })
      )
    )
  );
}

// ── TASK EXECUTION FLOW ───────────────────────────────────────────────────────

const TASKS=[
  {id:'email',  label:'Send Email',    payload:'{to,subject,body}'},
  {id:'trade',  label:'Execute Trade', payload:'{sym:AAPL,qty:500}'},
  {id:'delete', label:'Delete Files',  payload:'{path:/data/**}'},
  {id:'query',  label:'Query DB',      payload:'{sql:SELECT *}'},
];

function TaskFlowDiagram(){
  const ref=useRef(null),raf=useRef(null);
  const [task,setTask]=useState(TASKS[0]);
  const dark=document.documentElement.getAttribute('data-theme')!=='light';
  // Widen overall grid coordinates to give wires actual physical breathing room
  const W=960,H=390;
  const NX=[120,370,620,870],NW=150,NH=48;
  const ROWS=[
    {id:'none', label:'NO AUTH',   color:'#f43f5e',y:70, nodes:['[USR] Human','[AGT] Agent Core','[NET] Open Route','[API] Target Tool'],
     wires:['{cmd:'+task.id+'}','[SPOOFED_REQ]','[EXEC_RAW]'],
     desc:'Execution succeeds with zero verification. Traceability and accountability are non-existent.', outcome:'[CRITICAL] ✗ Untraceable'},
    {id:'token',label:'TOKEN AUTH',color:'#f59e0b',y:195,nodes:['[USR] Human','[AGT] Agent Core','[GWY] JWT Verify','[API] Target Tool'],
     wires:['{cmd:'+task.id+'}','{jwt:ey..}','[BEARER_OK]'],
     desc:'Identity established, but intent is untracked. Stolen keys result in total compromise.', outcome:'[WARN] ⚠ Intent Unverified'},
    {id:'a1',   label:'A1 PROTOCOL',color:'#10b981',y:320,nodes:['[USR] Human','[AGT] Agent Core','[A1] ZK Gateway','[API] Target Tool'],
     wires:['{cmd:'+task.id+'}','{zk_proof+nonce}','{tx_receipt}'],
     desc:'Cryptographic chain-of-custody established. O(1) duplicate rejection. Fully immutable.', outcome:'[SECURE] ✅ Immutable Chain'},
  ];

  function draw(ctx,t){
    const C=gC(dark),spd=t/1000;
    ctx.fillStyle=C.bg; ctx.fillRect(0,0,W,H);
    dotGrid(ctx,W,H,dark);

    // Draw vertical dividers perfectly centered between the expanded nodes
    for(let i=0; i<NX.length-1; i++) {
      const midX = (NX[i] + NX[i+1]) / 2;
      ctx.strokeStyle=dark?'rgba(255,255,255,0.06)':'rgba(0,0,0,0.06)';
      ctx.lineWidth=1; ctx.setLineDash([4,8]);
      ctx.beginPath(); ctx.moveTo(midX, 28); ctx.lineTo(midX, H-20); ctx.stroke(); ctx.setLineDash([]);
    }

    const hdrs=['1. INITIATION','2. AGENT LOGIC','3. AUTHORIZATION','4. EXECUTION'];
    NX.forEach((nx,i)=>{
      ctx.font='600 10px "IBM Plex Mono",monospace'; ctx.textAlign='center';
      ctx.textBaseline='middle'; ctx.fillStyle=C.fg2; ctx.fillText(hdrs[i],nx,16);
    });

    ROWS.forEach(row=>{
      const cy=row.y;
      const isA1=row.id==='a1';

      // Row Label background tab (Left side, minimal visual weight)
      rr(ctx,4,cy-NH/2,24,NH,2);
      ctx.fillStyle=row.color+'18'; ctx.fill();
      ctx.strokeStyle=row.color; ctx.lineWidth=1.5; ctx.stroke();
      
      // Vertical text for row label to save space and look technical
      ctx.fillStyle=row.color; ctx.font='700 8px "IBM Plex Mono",monospace'; ctx.textAlign='center'; ctx.textBaseline='middle';
      ctx.save(); ctx.translate(16, cy); ctx.rotate(-Math.PI/2); ctx.fillText(row.id.toUpperCase(), 0, 0); ctx.restore();

      row.nodes.forEach((lbl,ni)=>{
        const nx=NX[ni];
        rr(ctx,nx-NW/2,cy-NH/2,NW,NH,2);
        
        // Make A1 Gateway visually distinct from raw checks
        if (ni===2 && isA1) {
           ctx.fillStyle = 'rgba(16,185,129,0.08)';
           ctx.fill();
           ctx.strokeStyle = '#10b981'; ctx.lineWidth = 2; ctx.stroke();
        } else {
           ctx.fillStyle=C.nodeBg; ctx.fill();
           ctx.strokeStyle=ni===2?row.color:C.nodeBorder; ctx.lineWidth=ni===2?2:1; ctx.stroke();
        }
        
        ctx.font='600 11px "IBM Plex Mono",monospace'; ctx.fillStyle=C.fg;
        ctx.textAlign='center'; ctx.textBaseline='middle';
        ctx.fillText(lbl,nx,cy-5);
        
        if(ni===2){
          const subs={none:'[NO_VERIFICATION]',token:'[EXPIRY_CHECK_ONLY]',a1:'[ZK_CAP_NONCE_VERIFY]'};
          ctx.font='500 8px "IBM Plex Mono",monospace'; ctx.fillStyle=row.color;
          ctx.fillText(subs[row.id],nx,cy+10);
        }
      });

      for(let wi=0;wi<NX.length-1;wi++){
        const x0=NX[wi]+NW/2, x1=NX[wi+1]-NW/2;
        const wc=wi===1?row.color:C.wireFlow;
        const midX=(x0+x1)/2;

        ctx.beginPath(); ctx.moveTo(x0,cy); ctx.lineTo(x1,cy);
        ctx.strokeStyle=wc; ctx.lineWidth=wi===1?2:1.5;
        ctx.setLineDash([4,6]); ctx.lineDashOffset=-(spd*50)-(wi*22); ctx.stroke(); ctx.setLineDash([]);

        const wLabel=row.wires[wi]||'';
        if(wLabel){
          chip(ctx,midX,cy,wLabel,wc,null,C);
        }
        
        // Center the floating warnings directly over the middle of the wire length
        if (row.id === 'none' && wi === 1) {
           ctx.fillStyle = row.color; ctx.font='700 9px "IBM Plex Mono",monospace'; ctx.textAlign='center'; ctx.fillText('⚠ INJECTION / BYPASS', midX, cy - 18);
        }
        if (row.id === 'token' && wi === 1) {
           ctx.fillStyle = row.color; ctx.font='700 9px "IBM Plex Mono",monospace'; ctx.textAlign='center'; ctx.fillText('⚠ EXFILTRATED KEY', midX, cy - 18);
        }

        for(let p=0;p<2;p++){
          const pct=((spd*0.55+p/2+wi*0.22)%1);
          const px=x0+pct*(x1-x0);
          const pc=wi===1?(isA1?C.accent:row.id==='none'?C.red:C.amber):C.packet;
          dot(ctx,px,cy,pc);
        }
      }
    });
  }

  useEffect(()=>{
    const canvas=ref.current; if(!canvas) return;
    const dpr=window.devicePixelRatio||1;
    canvas.width=W*dpr; canvas.height=H*dpr;
    const ctx=canvas.getContext('2d'); ctx.scale(dpr,dpr);
    let start=null;
    const frame=ts=>{if(!start)start=ts; draw(ctx,ts-start); raf.current=requestAnimationFrame(frame);};
    raf.current=requestAnimationFrame(frame);
    return ()=>cancelAnimationFrame(raf.current);
  },[task,dark]);

  return h('div',{style:{display:'flex',flexDirection:'column',gap:0}},
    h('div',{style:{width:'100%',overflow:'hidden',background:dark?'#09090b':'#fafafa',border:'1px solid var(--b1)',borderBottom:'none'}},
      h('canvas',{ref,style:{width:'100%',height:'auto',display:'block',maxWidth:W+'px'}})
    ),
    h('div',{style:{display:'flex',gap:8,flexWrap:'wrap',padding:'12px 16px',background:dark?'#09090b':'#fafafa',border:'1px solid var(--b1)',borderTop:'none'}},
      h('span',{style:{fontFamily:'"IBM Plex Mono", monospace', fontSize:'11px', color:'var(--t2)', fontWeight:600, alignSelf:'center', marginRight:8, letterSpacing:'0.5px'}},'SIMULATE_PAYLOAD >'),
      TASKS.map(tk=>h('button',{key:tk.id,onClick:()=>setTask(tk),style:{padding:'6px 12px',fontFamily:'"IBM Plex Mono", monospace', fontSize:'11px',cursor:'pointer',border:'1px solid '+(task.id===tk.id?'var(--accent)':'var(--b2)'),borderRadius:2,background:task.id===tk.id?'rgba(129,140,248,.12)':'transparent',color:task.id===tk.id?'var(--accent)':'var(--t2)',fontWeight:task.id===tk.id?600:400,transition:'none', borderLeft:task.id===tk.id?'3px solid var(--accent)':'1px solid var(--b2)'}},'[' + tk.id.toUpperCase() + '] ' + tk.label))
    ),
    h('div',{style:{display:'grid',gridTemplateColumns:'1fr 1fr 1fr',gap:8,marginTop:10}},
      ROWS.map(row=>h('div',{key:row.id,style:{padding:'12px 16px',border:'1px solid '+row.color+'44',borderRadius:2,background:row.color+'08'}},
        h('div',{style:{fontWeight:700,fontFamily:'"IBM Plex Mono", monospace',fontSize:'12px',color:row.color, marginBottom:2, letterSpacing:'0.5px'}},row.label),
        h('div',{style:{fontWeight:700,fontFamily:'"IBM Plex Mono", monospace',fontSize:'10px',color:row.color, marginBottom:8}},row.outcome),
        h('div',{style:{fontSize:'var(--fsm)',color:'var(--t2)',lineHeight:1.6}},row.desc)
      ))
    )
  );
}

// ── AGENT DELEGATION CHAIN ────────────────────────────────────────────────────

const SCENARIOS=[
  {id:'none', label:'NO AUTH',   color:'#f43f5e',y:70,
   wires:['[ANY]','[ANY]','[ANY]','[RAW_EXEC]'],
   caps:[1.0, 1.0, 1.0, 1.0],
   desc:'Agents endlessly spawn sub-agents with 100% root privileges. Complete loss of control.', outcome:'[CRITICAL] ✗ Cascade Privilege Escalation'},
  {id:'token',label:'TOKEN AUTH',color:'#f59e0b',y:205,
   wires:['{jwt}','{jwt}','{jwt}','{jwt+cmd}'],
   caps:[1.0, 1.0, 1.0, 1.0],
   desc:'Bearer token passed down. Every sub-agent runs as orchestrator. Mathematical narrowing impossible.', outcome:'[WARN] ⚠ Token Re-use = Root Access'},
  {id:'a1',   label:'A1 PROTOCOL',color:'#10b981',y:340,
   wires:['{pass}','{crt_1}','{crt_2}','{zk_tx}'],
   caps:[1.0, 0.65, 0.35, 0.15],
   desc:'ZK-verified delegation. Child capability mask must be strict mathematical subset of parent.', outcome:'[SECURE] ✅ Provable Capability Narrowing'},
];

function DelegationDiagram(){
  const ref=useRef(null),raf=useRef(null);
  const dark=document.documentElement.getAttribute('data-theme')!=='light';
  const W=860,H=420;
  const NX=[80, 240, 400, 560, 740],NW=130,NH=46;

  function draw(ctx,t){
    const C=gC(dark),spd=t/1000;
    ctx.fillStyle=C.bg; ctx.fillRect(0,0,W,H);
    dotGrid(ctx,W,H,dark);

    NX.forEach((nx, i)=>{
      if(i<NX.length-1) {
        ctx.strokeStyle=dark?'rgba(255,255,255,0.05)':'rgba(0,0,0,0.05)';
        ctx.lineWidth=1; ctx.setLineDash([4,8]);
        ctx.beginPath(); ctx.moveTo(nx+NW/2 + 15, 28); ctx.lineTo(nx+NW/2 + 15, H-30); ctx.stroke(); ctx.setLineDash([]);
      }
    });

    const hdrs=['L0: HUMAN','L1: MASTER','L2: NODE A','L3: NODE B','TARGET API'];
    NX.forEach((nx,i)=>{
      ctx.font='600 9px "IBM Plex Mono",monospace'; ctx.textAlign='center';
      ctx.textBaseline='middle'; ctx.fillStyle=C.fg2; ctx.fillText(hdrs[i],nx,16);
    });

    const labels=['[USR] Root','[AGT] Orchestrator','[SUB] Worker 1','[SUB] Worker 2','[API] Execution'];

    SCENARIOS.forEach(sc=>{
      const cy=sc.y;
      const isA1=sc.id==='a1';
      const isNone=sc.id==='none';

      rr(ctx,4,cy-NH/2,64,NH,2);
      ctx.fillStyle=sc.color+'18'; ctx.fill(); ctx.strokeStyle=sc.color; ctx.lineWidth=1.5; ctx.stroke();
      ctx.font='700 8px "IBM Plex Mono",monospace'; ctx.textAlign='center';
      ctx.textBaseline='middle'; ctx.fillStyle=sc.color; ctx.fillText(sc.label,36,cy);

      labels.forEach((lbl,ni)=>{
        const nx=NX[ni];
        const isExec=ni===labels.length-1;

        rr(ctx,nx-NW/2,cy-NH/2,NW,NH,2);
        ctx.fillStyle=C.nodeBg; ctx.fill();
        ctx.strokeStyle=isExec?(isA1?C.green:C.red):C.nodeBorder;
        ctx.lineWidth=isExec?2:1; ctx.stroke();

        // Capability Bar visualization
        if(ni>0 && ni<labels.length-1){
          const capVal=sc.caps[ni];
          const barW = NW - 8;
          
          ctx.fillStyle=isA1?'rgba(16,185,129,.15)':(isNone?'rgba(244,63,94,.15)':'rgba(245,158,11,.15)');
          ctx.fillRect(nx-NW/2+4, cy-NH/2+4, barW, 6);
          
          ctx.fillStyle=isA1?C.green:(isNone?C.red:C.amber);
          ctx.fillRect(nx-NW/2+4, cy-NH/2+4, barW * capVal, 6);
          
          ctx.font='500 7px "IBM Plex Mono",monospace'; ctx.textAlign='right';
          ctx.fillStyle=C.fg;
          ctx.fillText(`CAP: ${Math.round(capVal*100)}%`, nx+NW/2-6, cy-NH/2+8);
        }

        ctx.font='600 10px "IBM Plex Mono",monospace'; ctx.fillStyle=C.fg;
        ctx.textAlign='center'; ctx.textBaseline='middle';
        ctx.fillText(lbl, nx, cy+5);

        if(ni<labels.length-1){
          const x0=nx+NW/2, x1=NX[ni+1]-NW/2;
          const hop_colors=['#818cf8','#a78bfa','#c084fc','#10b981'];
          const wc=isA1?hop_colors[ni]:sc.color+'88';
          
          ctx.beginPath(); ctx.moveTo(x0,cy); ctx.lineTo(x1,cy);
          ctx.strokeStyle=wc; ctx.lineWidth=isA1?2:1.5;
          ctx.setLineDash([4,6]); ctx.lineDashOffset=-(spd*48)-(ni*20); ctx.stroke(); ctx.setLineDash([]);

          const midX=(x0+x1)/2;
          chip(ctx,midX,cy,sc.wires[ni],wc,null,C);

          for(let p=0;p<2;p++){
            const pct=((spd*0.5+p/2+ni*0.25)%1);
            const px=x0+pct*(x1-x0);
            dot(ctx,px,cy,isA1?hop_colors[ni]:isNone?C.red:C.amber);
          }
        }
      });

      ctx.font='600 8.5px "IBM Plex Mono",monospace'; ctx.textAlign='left';
      ctx.textBaseline='middle'; ctx.fillStyle=isA1?C.green:isNone?C.red:C.amber;
      ctx.fillText(sc.outcome, NX[0]-NW/2, cy + NH/2 + 12);
    });

    const lx=12,ly=H-16;
    ctx.font='600 8px "IBM Plex Mono",monospace'; ctx.fillStyle=C.fg2;
    ctx.textAlign='left'; ctx.textBaseline='middle';
    ctx.fillText('CRYPTOGRAPHIC NARROWING DEPTH →',lx,ly);
    ['#818cf8','#a78bfa','#c084fc','#10b981'].forEach((c,i)=>{
      ctx.fillStyle=c; ctx.fillRect(lx+190+i*34,ly-4,28,8);
    });
  }

  useEffect(()=>{
    const canvas=ref.current; if(!canvas) return;
    const dpr=window.devicePixelRatio||1;
    canvas.width=W*dpr; canvas.height=H*dpr;
    const ctx=canvas.getContext('2d'); ctx.scale(dpr,dpr);
    let start=null;
    const frame=ts=>{if(!start)start=ts; draw(ctx,ts-start); raf.current=requestAnimationFrame(frame);};
    raf.current=requestAnimationFrame(frame);
    return ()=>cancelAnimationFrame(raf.current);
  },[dark]);

  return h('div',{style:{display:'flex',flexDirection:'column',gap:0}},
    h('div',{style:{width:'100%',overflow:'hidden',background:dark?'#09090b':'#fafafa',border:'1px solid var(--b1)'}},
      h('canvas',{ref,style:{width:'100%',height:'auto',display:'block',maxWidth:W+'px'}})
    ),
    h('div',{style:{display:'grid',gridTemplateColumns:'1fr 1fr 1fr',gap:8,marginTop:10}},
      SCENARIOS.map(sc=>h('div',{key:sc.id,style:{padding:'10px 14px',border:'1px solid '+sc.color+'44',borderRadius:2,background:sc.color+'08'}},
        h('div',{style:{fontWeight:700,fontFamily:'"IBM Plex Mono", monospace',fontSize:'10px',color:sc.color,marginBottom:6,letterSpacing:'0.5px'}},sc.label),
        h('div',{style:{fontSize:'var(--fxs)',color:'var(--t2)',lineHeight:1.6}},sc.desc)
      ))
    )
  );
}

// ── Main wrapper with three tabs ──────────────────────────────────────────────

function SecurityDiagram(){
  const [view,setView]=useState('attack');
  const VIEWS=[
    {id:'attack',   label:'⚡ Attack Simulation',    hint:'Simulate 8 real AI agent attacks across No Auth / Token / A1'},
    {id:'task',     label:'🔧 Task Execution Flow',   hint:'What travels on the wire when an agent executes a task'},
    {id:'delegate', label:'🔗 Delegation Chain',      hint:'How multi-agent delegation degrades (or stays accountable) per scenario'},
  ];
  return h('div',null,
    h('div',{style:{display:'flex',gap:8,flexWrap:'wrap',marginBottom:14}},
      VIEWS.map(v=>h('button',{key:v.id,onClick:()=>setView(v.id),title:v.hint,style:{padding:'7px 16px',fontSize:'var(--fsm)',fontWeight:view===v.id?700:500,cursor:'pointer',border:'2px solid '+(view===v.id?'var(--accent)':'var(--b3)'),borderRadius:8,background:view===v.id?'rgba(99,102,241,.12)':'var(--b1)',color:view===v.id?'var(--accent)':'var(--t2)',transition:'all .15s',whiteSpace:'nowrap'}},v.label))
    ),
    view==='attack'   && h(AttackDiagram,null),
    view==='task'     && h(TaskFlowDiagram,null),
    view==='delegate' && h(DelegationDiagram,null)
  );
}
