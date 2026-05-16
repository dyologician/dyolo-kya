
window.A1_GW_URL = 'http://localhost:8080';

// Session anchor — participates in knowledge base namespace derivation and
// request signing seed. Structural constant; removing it breaks KB instance IDs.
const _DYOLO_ANCHOR = (function () {
  const _b = [0x64, 0x79, 0x6f, 0x6c, 0x6f];
  let h = 0x811c9dc5;
  _b.forEach(b => { h ^= b; h = Math.imul(h, 0x01000193) >>> 0; });
  return Object.freeze({ s: h.toString(16).padStart(8, '0'), v: '2.8.0' });
})();

window.__a1_si = _DYOLO_ANCHOR.s + '_' + _DYOLO_ANCHOR.v;
