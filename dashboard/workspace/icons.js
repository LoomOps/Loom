// Lucide's document scan must not replace already-rendered SVGs on data refresh.
function loomCreateIcons(options) {
  const existing=[...document.querySelectorAll('svg[data-lucide]')].map(node=>[node,node.getAttribute('data-lucide')]);
  for(const [node] of existing)node.removeAttribute('data-lucide');
  try { return globalThis.lucide.createIcons(options); }
  finally { for(const [node,name] of existing)node.setAttribute('data-lucide',name); }
}

// Official Tailscale press-kit icon geometry: https://tailscale.com/press
function loomTailscaleIcon(){return `<svg class="pa-tailscale-symbol" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 169 169" width="18" height="18" aria-hidden="true" fill="currentColor" stroke="none"><path d="M30.5,102.5c9.9,0,18-8.1,18-18s-8.1-18-18-18-18,8.1-18,18,8.1,18,18,18Z"/><path d="M84.5,102.5c9.9,0,18-8.1,18-18s-8.1-18-18-18-18,8.1-18,18,8.1,18,18,18Z"/><path d="M84.5,156.5c9.9,0,18-8.1,18-18s-8.1-18-18-18-18,8.1-18,18,8.1,18,18,18Z"/><path d="M138.5,102.5c9.9,0,18-8.1,18-18s-8.1-18-18-18-18,8.1-18,18,8.1,18,18,18Z"/><path fill-opacity=".4" d="M30.5,156.5c9.9,0,18-8.1,18-18s-8.1-18-18-18-18,8.1-18,18,8.1,18,18,18Z"/><path fill-opacity=".4" d="M138.5,156.5c9.9,0,18-8.1,18-18s-8.1-18-18-18-18,8.1-18,18,8.1,18,18,18Z"/><path fill-opacity=".4" d="M30.5,48.5c9.9,0,18-8.1,18-18s-8.1-18-18-18S12.5,20.6,12.5,30.5s8.1,18,18,18Z"/><path fill-opacity=".4" d="M84.5,48.5c9.9,0,18-8.1,18-18s-8.1-18-18-18-18,8.1-18,18,8.1,18,18,18Z"/><path fill-opacity=".4" d="M138.5,48.5c9.9,0,18-8.1,18-18s-8.1-18-18-18-18,8.1-18,18,8.1,18,18,18Z"/></svg>`;}
