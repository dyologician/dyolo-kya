// ─────────────────────────────────────────────────────────────────────────────
// FLOATING HELP BUTTON
// ─────────────────────────────────────────────────────────────────────────────
function HelpButton({helpMode,onToggle}){
  const ref=useRef(null);
  const pos=useRef({x:window.innerWidth-60,y:window.innerHeight-60});
  const drag=useRef(null);
  const[xy,setXY]=useState({x:window.innerWidth-60,y:window.innerHeight-60});
  const[tipVisible,setTipVisible]=useState(false);

  useEffect(()=>{
    const el=ref.current;
    if(!el)return;
    function onDown(e){
      const r=el.getBoundingClientRect();
      drag.current={ox:e.clientX-r.left,oy:e.clientY-r.top,moved:false};
      e.preventDefault();
    }
    function onMove(e){
      if(!drag.current)return;
      drag.current.moved=true;
      const nx=e.clientX-drag.current.ox;
      const ny=e.clientY-drag.current.oy;
      const x=Math.max(0,Math.min(window.innerWidth-36,nx));
      const y=Math.max(0,Math.min(window.innerHeight-36,ny));
      pos.current={x,y};
      setXY({x,y});
    }
    function onUp(e){
      if(drag.current&&!drag.current.moved)onToggle();
      drag.current=null;
    }
    el.addEventListener('mousedown',onDown);
    window.addEventListener('mousemove',onMove);
    window.addEventListener('mouseup',onUp);
    return()=>{el.removeEventListener('mousedown',onDown);window.removeEventListener('mousemove',onMove);window.removeEventListener('mouseup',onUp);};
  },[onToggle]);

  return h('button',{ref,id:'_help_btn','data-help':'help-btn',className:'help-btn'+(helpMode?' active':''),style:{left:xy.x,top:xy.y},title:helpMode?'Exit help mode (click)':'Help mode (click) — drag to move'},'?');
}

// ─────────────────────────────────────────────────────────────────────────────
// TOOLTIP ENGINE
// ─────────────────────────────────────────────────────────────────────────────
function TooltipLayer({helpMode}){
  const[tip,setTip]=useState(null);
  useEffect(()=>{
    if(!helpMode){setTip(null);return;}
    function onMove(e){
      const el=e.target.closest('[data-help]');
      if(!el){setTip(null);return;}
      const key=el.getAttribute('data-help');
      const info=HELP[key];
      if(!info){setTip(null);return;}
      const x=Math.min(e.clientX+16,window.innerWidth-280);
      const y=Math.min(e.clientY+16,window.innerHeight-120);
      setTip({info,x,y});
    }
    function onLeave(){setTip(null);}
    document.addEventListener('mousemove',onMove);
    document.addEventListener('mouseleave',onLeave);
    return()=>{document.removeEventListener('mousemove',onMove);document.removeEventListener('mouseleave',onLeave);};
  },[helpMode]);
  if(!tip)return null;
  return h('div',{className:'ctx-tip',style:{left:tip.x,top:tip.y},id:'_ctx_tip'},
    h('div',{className:'ctx-tip-inner'},
      h('div',{className:'ctx-tip-title'},tip.info.title),
      h('div',null,tip.info.body)
    )
  );
}

