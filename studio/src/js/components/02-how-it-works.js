// ─────────────────────────────────────────────────────────────────────────────
// HOW IT WORKS — Infographic-style comparison: No Auth / JWT / A1
// ─────────────────────────────────────────────────────────────────────────────
function HowItWorks(){
  const[tab,setTab]=useState('diagram');

  // ── Column definitions ────────────────────────────────────────────────────
  const COLS=[
    {
      key:'no-auth',
      num:'1',
      title:'No Authorization',
      sub:'Agent acts freely',
      cls:'no-auth',
      dot:true,
      steps:[
        {icon:'👤',label:'Human gives instruction',detail:'Verbally or via prompt — no record kept',type:'neutral'},
        {arrow:true,cls:'red'},
        {icon:'🤖',label:'Agent runs immediately',detail:'No check. No proof. No limits.',type:'neutral'},
        {arrow:true,cls:'red'},
        {icon:'⚡',label:'Tool / action executes',detail:'send_email(), trade(), delete_file()…',type:'neutral'},
        {arrow:true,cls:'red'},
        {icon:'❓',label:'Who authorized this?',detail:'No way to prove it. No audit trail.',type:'bad'},
      ],
      gaps:[
        {icon:'❌',text:'No record of who approved the action'},
        {icon:'❌',text:'Agent can exceed intended scope'},
        {icon:'❌',text:'No way to revoke or expire access'},
        {icon:'❌',text:'Multi-agent chains completely unverifiable'},
      ],
      usefor:'Toy projects only. Never production.',
    },
    {
      key:'jwt-auth',
      num:'2',
      title:'Standard Auth (JWT / API Key)',
      sub:'Token-based identity',
      cls:'jwt-auth',
      dot:true,
      steps:[
        {icon:'👤',label:'Human issues JWT / API key',detail:'Signed token with expiry and some claims',type:'neutral'},
        {arrow:true,cls:'yellow'},
        {icon:'🤖',label:'Agent attaches token to requests',detail:'Bearer token in Authorization header',type:'warn'},
        {arrow:true,cls:'yellow'},
        {icon:'🔑',label:'Service validates token',detail:'Checks signature and expiry only',type:'warn'},
        {arrow:true,cls:'yellow'},
        {icon:'⚡',label:'Action executes',detail:'If token is valid — that\'s the only check',type:'warn'},
      ],
      gaps:[
        {icon:'⚠️',text:'Token can be copied, shared, or stolen'},
        {icon:'⚠️',text:'No capability narrowing — token grants everything'},
        {icon:'⚠️',text:'Multi-agent delegation: which agent actually acted?'},
        {icon:'⚠️',text:'Revocation requires token blacklist infrastructure'},
      ],
      usefor:'Single-agent, low-risk tasks. Breaks under delegation.',
    },
    {
      key:'a1-auth',
      num:'3',
      title:'With A1 (Chain-of-Custody)',
      sub:'Cryptographic proof at every hop',
      cls:'a1-auth',
      dot:true,
      steps:[
        {icon:'👤',label:'Human issues DyoloPassport',detail:'Ed25519-signed identity + capability bitmask',type:'ok'},
        {arrow:true,cls:'green'},
        {icon:'📜',label:'Agent builds DyoloChain',detail:'Delegation certs from human → orchestrator → executor',type:'ok'},
        {arrow:true,cls:'green'},
        {icon:'🛡',label:'NarrowingMatrix enforces scope',detail:'child_mask & parent_mask == child_mask  — O(1)',type:'ok'},
        {arrow:true,cls:'green'},
        {icon:'✅',label:'ProvableReceipt issued',detail:'Tamper-evident proof, independently verifiable',type:'ok'},
      ],
      gaps:[
        {icon:'✓',text:'Cryptographic proof of human authorization at every hop'},
        {icon:'✓',text:'Capability narrowing — agent cannot exceed what it was granted'},
        {icon:'✓',text:'Nonce prevents replay attacks'},
        {icon:'✓',text:'Revoke any cert instantly — takes effect in milliseconds'},
      ],
      usefor:'Multi-agent systems, regulated industries, production AI.',
    },
  ];

  // ── Comparison table rows ─────────────────────────────────────────────────
  const ROWS=[
    {label:'Who authorized the action?',no:'Unknown',jwt:'Whoever owns the token',a1:'Cryptographically proven'},
    {label:'Scope enforcement',no:'None',jwt:'Coarse (role/scope claims)',a1:'NarrowingMatrix — per-capability, O(1)'},
    {label:'Multi-agent delegation',no:'No concept',jwt:'Not possible to verify',a1:'Full chain verified at every hop'},
    {label:'Replay protection',no:'None',jwt:'Expiry only',a1:'Per-intent nonce, atomic consumption'},
    {label:'Revocation',no:'None',jwt:'Token blacklist required',a1:'Fingerprint deny-list, instant'},
    {label:'Audit proof',no:'None',jwt:'Log entries (mutable)',a1:'ProvableReceipt (tamper-evident)'},
    {label:'Works offline / air-gapped',no:'Yes',jwt:'Depends on token validation',a1:'Yes — all verification is local'},
    {label:'Suitable for production AI',no:'❌',jwt:'⚠️ Limited',a1:'✅ Yes'},
  ];

  return h('div',{style:{paddingBottom:32,width:'100%'}},

    // Header
    h('div',{style:{marginBottom:20}},
      h('h2',{style:{fontSize:20,fontWeight:700,marginBottom:4}},'🔍 No Auth vs JWT vs A1 — What Actually Happens'),
      h('p',{style:{color:'var(--t2)',fontSize:'var(--fsm)',lineHeight:1.6}},
        'See exactly what occurs when an AI agent tries to take an action — and where each approach succeeds or fails.')),

    // Tab switcher
    h('div',{style:{display:'flex',gap:8,marginBottom:16}},
      h('button',{className:'btn '+(tab==='diagram'?'btn-p':'btn-s')+' btn-sm',onClick:()=>setTab('diagram')},'Flow Diagram'),
      h('button',{className:'btn '+(tab==='table'?'btn-p':'btn-s')+' btn-sm',onClick:()=>setTab('table')},'Comparison Table'),
      h('button',{className:'btn '+(tab==='live'?'btn-p':'btn-s')+' btn-sm',onClick:()=>setTab('live')},'⚡ Live Attack Demo')),

    // ── DIAGRAM TAB ─────────────────────────────────────────────────────────
    tab==='diagram'&&h('div',null,
      h('div',{className:'inf-wrap'},
        COLS.map(col=>h('div',{key:col.key,className:'inf-col '+col.cls},

          // Header
          h('div',{className:'inf-hdr'},
            h('div',null,
              h('span',{className:'inf-hdr-num'},col.num),
              h('span',{className:'inf-hdr-title'},col.title)),
            h('div',{className:'inf-hdr-sub'},
              h('span',{className:'inf-dot'}),col.sub)),

          // Steps
          h('div',{className:'inf-body'},
            col.steps.map((s,i)=>
              s.arrow
                ?h('div',{key:i,className:'inf-arrow '+s.cls},'↓')
                :h('div',{key:i,className:'inf-step '+s.type},
                    h('div',{className:'inf-step-icon'},s.icon),
                    h('div',{className:'inf-step-text'},
                      h('strong',null,s.label),
                      h('span',null,s.detail))))),

          // Gaps / advantages
          h('div',{className:'inf-gaps'},
            h('div',{className:'inf-gaps-title'},
              col.key==='a1-auth'?'Guarantees':'Gaps'),
            col.gaps.map((g,i)=>
              h('div',{key:i,className:'inf-gap-item'},
                h('span',null,g.icon),' ',g.text))),

          // Use for
          h('div',{className:'inf-usefor'},
            h('span',{className:'inf-usefor-label'},'Use for: '),
            col.usefor)))),

      // Key insight callout
      h('div',{className:'wiz-info gr',style:{marginTop:16}},
        h('span',{style:{fontSize:22}},'💡'),
        h('div',null,
          h('div',{style:{fontWeight:700,fontSize:'var(--fsm)',marginBottom:4}},'The Recursive Delegation Gap'),
          h('div',{style:{color:'var(--t2)',lineHeight:1.7,fontSize:'var(--fsm)'}},
            'When Agent A delegates to Agent B which delegates to Agent C, JWT and API keys completely break down. ',
            'There is no way to prove the final action traces back to the original human. ',
            h('strong',null,'A1 is the only approach that maintains cryptographic proof at every delegation hop.'))))),

    // ── TABLE TAB ───────────────────────────────────────────────────────────
    tab==='table'&&h('div',null,
      h('table',{className:'inf-tbl'},
        h('thead',null,
          h('tr',null,
            h('th',null,'Property'),
            h('th',null,'❌ No Auth'),
            h('th',null,'⚠️ JWT / API Key'),
            h('th',null,'✅ A1'))),
        h('tbody',null,
          ROWS.map((r,i)=>h('tr',{key:i},
            h('td',{style:{fontWeight:500}},r.label),
            h('td',{className:'itbad'},r.no),
            h('td',{className:'itmid'},r.jwt),
            h('td',{className:'itok'},r.a1)))))),

    tab==='live'&&h('div',null,
      h('p',{style:{color:'var(--t2)',fontSize:'var(--fsm)',lineHeight:1.7,marginBottom:14}},
        'Watch how No Auth, Token Auth, and A1 handle real attacks — animated in real time. ',
        h('strong',null,'Click any attack below the diagram'),
        ' to simulate it across all three approaches.'),
      h(SecurityDiagram,null),
      h('div',{className:'wiz-info',style:{marginTop:16}},
        h('span',{style:{fontSize:18}},'💡'),
        h('div',null,
          h('div',{style:{fontWeight:600,marginBottom:3}},'A1 blocks every attack shown'),
          h('div',{style:{color:'var(--t2)',fontSize:'var(--fxs)',lineHeight:1.7}},
            'No Auth fails all 6 attacks. Token Auth fails 5. A1 blocks all 6 using cryptographic narrowing, nonce tracking, TTL expiry, and ZK chain-of-custody.'))))
  );
}
