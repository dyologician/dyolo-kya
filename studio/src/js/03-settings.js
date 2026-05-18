// ─────────────────────────────────────────────────────────────────────────────
// SETTINGS + SCALING
// ─────────────────────────────────────────────────────────────────────────────
const LS='a1_studio_v3';
const LS_OB='a1_studio_onboarded';
const DEFAULTS={
  gwUrl:window.location.origin,adminSecret:'',showSecret:false,
  showFullIds:false,pollMs:4000,logMax:200,autoScroll:true,
  theme:'dark',density:'auto',fontSize:13,
  developerMode:false,
  simpleMode:true,
};

function loadS(){try{const r=localStorage.getItem(LS);if(r)return{...DEFAULTS,...JSON.parse(r),showSecret:false};}catch{}return{...DEFAULTS};}
function saveS(s){try{localStorage.setItem(LS,JSON.stringify({...s,showSecret:false}));}catch{}}
function hasOnboarded(){try{return!!localStorage.getItem(LS_OB);}catch{return false;}}
function setOnboarded(){try{localStorage.setItem(LS_OB,'1');}catch{}}

// Auto-detect good density based on viewport
function autoDetectDensity(){
  const w=window.innerWidth;
  if(w<640)return'compact';
  if(w>1400)return'comfortable';
  return'normal';
}

function applyScaling(settings){
  const density=settings.density==='auto'?autoDetectDensity():settings.density;
  document.documentElement.setAttribute('data-density',density);
  document.documentElement.style.setProperty('--fbase',settings.fontSize+'px');
  document.documentElement.style.setProperty('--fsm',(settings.fontSize-2)+'px');
  document.documentElement.style.setProperty('--fxs',(settings.fontSize-3)+'px');
}

