// ─────────────────────────────────────────────────────────────────────────────
// REACT APP
// ─────────────────────────────────────────────────────────────────────────────
const{useState,useEffect,useCallback,useRef,useMemo,createContext,useContext}=React;
const h=React.createElement;
const Ctx=createContext(null);

function useApi(settings,addLog){
  return useCallback(async(method,path,body)=>{
    const url=settings.gwUrl+path;
    const start=Date.now();
    const opts={method,headers:{'Content-Type':'application/json'}};
    if(settings.adminSecret)opts.headers['Authorization']='Bearer '+settings.adminSecret;
    if(body)opts.body=JSON.stringify(body);
    let status=0,data={},ok=false;
    try{const r=await fetch(url,opts);status=r.status;ok=r.ok;data=await r.json().catch(()=>({}));}
    catch(err){data={error:err.message};}
    const ms=Date.now()-start;
    addLog({id:Date.now()+Math.random(),method,path,status,ok,ms,req:body||null,res:data,ts:new Date()});
    return{ok,data,status};
  },[settings,addLog]);
}

// ── TruncId ───────────────────────────────────────────────────────────────────
function TruncId({val,showFull}){
  const[open,setOpen]=useState(false);
  const[copied,setCopied]=useState(false);
  if(!val)return null;
  const show=(showFull||open)?val:val.slice(0,14)+(val.length>14?'…':'');
  function copy(){navigator.clipboard.writeText(val).then(()=>{setCopied(true);setTimeout(()=>setCopied(false),1100);});}
  return h('span',{className:'chip','data-help':'trun-id'},
    h('span',{className:'chip-val'},show),
    val.length>14&&!showFull&&h('button',{className:'chip-btn',onClick:()=>setOpen(o=>!o)},open?'▲':'▼'),
    h('button',{className:'chip-btn',onClick:copy},copied?'✓':'⎘')
  );
}

function Alert({msg,type}){
  if(!msg)return null;
  return h('div',{className:'alert alert-'+(type==='error'?'err':'ok')},msg);
}

function ToggleRow({label,sub,checked,onChange}){
  return h('div',{className:'toggle-row'},
    h('div',null,h('div',{className:'tog-lbl'},label),sub&&h('div',{className:'tog-sub'},sub)),
    h('label',{className:'tog'},h('input',{type:'checkbox',checked,onChange:e=>onChange(e.target.checked)}),h('div',{className:'tog-track'}),h('div',{className:'tog-thumb'}))
  );
}

function LogEntry({e}){
  const[open,setOpen]=useState(false);
  const ts=e.ts.toLocaleTimeString('en-US',{hour12:false,hour:'2-digit',minute:'2-digit',second:'2-digit'});
  const has=e.req||e.res;
  return h('div',null,
    h('div',{className:'log-entry',onClick:()=>has&&setOpen(o=>!o)},
      h('span',{className:'log-time'},ts),
      h('span',{className:'log-method log-'+e.method.toLowerCase()},e.method),
      h('span',{className:'log-path'},e.path),
      h('span',{className:'log-status log-s-'+(e.ok?'ok':'err')},e.status||'—'),
      h('span',{className:'log-ms'},e.ms+'ms')
    ),
    open&&has&&h('div',{className:'log-detail'},
      e.req&&h('div',null,h('div',{style:{fontSize:9,color:'var(--t2)',padding:'4px 0'}},'REQUEST'),h('pre',null,JSON.stringify(e.req,null,2))),
      e.res&&h('div',null,h('div',{style:{fontSize:9,color:'var(--t2)',padding:'4px 0'}},'RESPONSE'),h('pre',null,JSON.stringify(e.res,null,2)))
    )
  );
}

// ── Attribution links (protected) ─────────────────────────────────────────────
function SocialLinks(){
  return h('div',{className:'social-row','data-help':'social-attr'},
    h('a',{className:'social-link',href:attrUrl(0),target:'_blank',rel:'noopener','data-attr-0':''},_A.lbl[0]),
    h('a',{className:'social-link',href:attrUrl(1),target:'_blank',rel:'noopener','data-attr-1':''},_A.lbl[1]),
    h('a',{className:'social-link',href:attrUrl(2),target:'_blank',rel:'noopener','data-attr-2':''},_A.lbl[2])
  );
}

