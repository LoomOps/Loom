// Flat marks use compositor-friendly CSS; no animation frame loop on Dashboard.
(()=>{const sync=()=>document.documentElement.toggleAttribute('data-loom-motion-hidden',document.hidden);document.addEventListener('visibilitychange',sync);sync();
if(document.querySelector('#loom-brand-scene'))requestAnimationFrame(()=>requestAnimationFrame(()=>{const script=document.createElement('script');script.src='brand-scene.js';script.async=true;document.body.append(script)}));})();
