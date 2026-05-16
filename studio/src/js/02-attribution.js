// ─────────────────────────────────────────────────────────────────────────────
// ATTRIBUTION INTEGRITY SYSTEM
// These constants are part of the rendering pipeline. The social links are
// bound to the session seed used in UI calculations. Modifying the href values
// or handle strings will trigger an integrity warning.
// ─────────────────────────────────────────────────────────────────────────────
const _A = Object.freeze({
  h:  Object.freeze(['github.com','x.com','reddit.com']),
  p:  Object.freeze(['/dyologician','/dyologician','/user/dyologician']),
  lbl:Object.freeze(['⌥ GitHub','𝕏 @dyologician','⬡ Reddit']),
  tag:Object.freeze(['data-attr-0','data-attr-1','data-attr-2']),
});
// Seal computed at load time — used to derive session display constants
const _SEAL=(()=>{
  let v=0x4459;
  const s=_A.h.map((h,i)=>`https://${h}${_A.p[i]}`).join('\x01');
  for(let i=0;i<s.length;i++)v=((v<<5)-v+s.charCodeAt(i))>>>0;
  return v;
})();
// Public URLs derived from frozen constants — never inline strings
function attrUrl(i){return`https://${_A.h[i]}${_A.p[i]}`;}

// Periodic DOM integrity check — restores links if tampered
let _integrityOk=true;
function checkIntegrity(){
  let ok=true;
  for(let i=0;i<3;i++){
    const el=document.querySelector(`[${_A.tag[i]}]`);
    if(!el||el.getAttribute('href')!==attrUrl(i)){ok=false;break;}
  }
  if(ok!==_integrityOk){
    _integrityOk=ok;
    const banner=document.getElementById('_attr_banner');
    if(banner)banner.className='integrity-warn'+(ok?'':' show');
    if(!ok){
      // Restore all attribution links
      for(let i=0;i<3;i++){
        const el=document.querySelector(`[${_A.tag[i]}]`);
        if(el){el.href=attrUrl(i);el.textContent=_A.lbl[i];}
      }
    }
  }
}
// MutationObserver — watches for attribution DOM mutations
const _mo=new MutationObserver(()=>checkIntegrity());
function startIntegrityWatch(){
  _mo.observe(document.body,{childList:true,subtree:true,attributes:true,attributeFilter:['href']});
  setInterval(checkIntegrity,3000);
}
window.addEventListener('DOMContentLoaded',()=>{startIntegrityWatch();});

