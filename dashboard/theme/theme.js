// Shared preference for the trusted Dashboard parent and entry page.
(() => {
  const valid = value => ['light','dark','system'].includes(value);
  function read() {
    const value = document.cookie.split(';').map(v=>v.trim()).find(v=>v.startsWith('loom_theme='))?.slice(11);
    return valid(value)?value:'system';
  }
  function apply(mode=read()) {
    if(!valid(mode)) mode='system';
    document.documentElement.style.colorScheme=mode==='system'?'light dark':mode;
    document.documentElement.dataset.loomTheme=mode;
    document.querySelectorAll('[data-loom-theme]').forEach(el=>el.setAttribute('aria-pressed',String(el.dataset.loomTheme===mode)));
    window.dispatchEvent(new CustomEvent('loom-theme-updated',{detail:mode}));
    return mode;
  }
  function set(mode) {
    if(!valid(mode)) return;
    document.cookie=`loom_theme=${mode}; Path=/; Max-Age=31536000; SameSite=Lax${location.protocol==='https:'?'; Secure':''}`;
    apply(mode);
  }
  window.LoomTheme={read,apply,set};
  apply();
  addEventListener('pageshow',()=>apply());
  addEventListener('focus',()=>apply());
  document.addEventListener('visibilitychange',()=>{if(!document.hidden)apply();});
  document.addEventListener('click',event=>{const control=event.target.closest('[data-loom-theme]');if(control)set(control.dataset.loomTheme);});
})();
