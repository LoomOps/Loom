// Continuous-corner card skins. Content itself remains unclipped, including menus.
(() => {
  const widgets=['.pa-panel','.pa-resources','.pa-stat','.pa-doc-detail','.pa-resource-card',
    '.pa-result-flow','.pa-outcome-highlights article','.pa-engineering','.pa-result-definitions',
    '.pa-result-discarded','.pa-notice','.pa-catalog-warning','.pa-media','.pa-episode',
    '.pa-pipeline-stage','.pa-finder','.pa-doc-code','.pa-readable-file','.pa-readable-markdown pre',
    '.pa-agent-source','.pa-agent-list-item'];
  const widgetSelector=widgets.join(',');
  const selector=widgets.map(s=>'.pa-pane-body '+s).join(',');
  const cards=new Map();
  const sizes=new WeakMap();
  // Pre-compose filtered card skins before they enter the scrolling viewport.
  // The halo is bounded, rather than promoting the entire long page to layers.
  const nearby=new Map(),nearbyOwner=new WeakMap();
  function prepareNearby(card){
    const pane=card.closest('.pa-pane-body');if(!pane)return;
    let watcher=nearby.get(pane);
    if(!watcher){
      watcher=new IntersectionObserver(entries=>{
        for(const {target,isIntersecting} of entries){
          const skin=cards.get(target);if(skin)skin.style.willChange=isIntersecting?'transform':'auto';
        }
      },{root:pane,rootMargin:'800px 0px'});
      nearby.set(pane,watcher);
    }
    nearbyOwner.set(card,watcher);watcher.observe(card);
  }
  const pending=new Set();
  let frame=0,gradientSequence=0;
  function draw(card){
    const skin=cards.get(card);
    if(!skin || !card.isConnected)return;
    const {width:w,height:h}=card.getBoundingClientRect();
    if(!w || !h)return;
    const size=sizes.get(skin);if(size?.[0]===w&&size?.[1]===h)return;
    sizes.set(skin,[w,h]);
    const primary=card.matches('.pa-panel,.pa-resources,.pa-doc-detail');
    const r=Math.min(primary?38:card.classList.contains('pa-stat')?28:20,w/4,h/4),points=[];
    const inset=.75;
    for(const [cx,cy,start] of [[w-r-inset,r+inset,-90],[w-r-inset,h-r-inset,0],[r+inset,h-r-inset,90],[r+inset,r+inset,180]]){
      for(let i=0;i<=20;i++){
        const a=(start+i*4.5)*Math.PI/180,c=Math.cos(a),s=Math.sin(a);
        points.push(`${(cx+r*Math.sign(c)*Math.pow(Math.abs(c),2/3)).toFixed(2)} ${(cy+r*Math.sign(s)*Math.pow(Math.abs(s),2/3)).toFixed(2)}`);
      }
    }
    skin.setAttribute('viewBox',`0 0 ${w} ${h}`);
    skin.firstChild.setAttribute('d','M'+points.join('L')+'Z');
    const gradient=skin.querySelector('linearGradient');
    if(gradient){
      // Physical angle: left to right, rotated 30 degrees counterclockwise.
      // User-space coordinates keep that angle on both wide and tall tiles.
      const c=Math.sqrt(3)/2,s=.5,half=(w*c+h*s)/2;
      for(const [key,value] of Object.entries({x1:w/2-half*c,y1:h/2+half*s,x2:w/2+half*c,y2:h/2-half*s}))gradient.setAttribute(key,value);
    }
    card.classList.add('loom-surface');
  }
  function flush(){
    frame=0;
    for(const card of cards.keys())if(!card.isConnected){observer.unobserve(card);nearbyOwner.get(card)?.unobserve(card);cards.delete(card);}
    for(const [pane,watcher] of nearby)if(!pane.isConnected){watcher.disconnect();nearby.delete(pane);}
    for(const card of pending)draw(card);
    pending.clear();
  }
  function schedule(card){pending.add(card);if(!frame)frame=requestAnimationFrame(flush);}
  // ResizeObserver already runs before paint. Deferring again to rAF leaves an
  // empty or outdated skin visible for a frame after layout changes.
  const observer=new ResizeObserver(entries=>{for(const {target} of entries)draw(target);});
  function refresh(){
    for(const card of root.querySelectorAll(selector)){
      if(card.matches('.pa-finder-preview > .pa-panel'))continue;
      if(cards.has(card) && cards.get(card).parentElement===card)continue;
      if(cards.has(card))observer.unobserve(card);
      // Reconciled/cloned cards can already carry a skin; adopt it so resize
      // observations update the visible SVG instead of appending a duplicate.
      const skin=card.querySelector(':scope > .loom-surface-skin') || document.createElementNS('http://www.w3.org/2000/svg','svg');
      skin.classList.add('loom-surface-skin');skin.setAttribute('aria-hidden','true');skin.setAttribute('focusable','false');
      if(!skin.firstChild)skin.append(document.createElementNS(skin.namespaceURI,'path'));
      if(card.matches('.pa-resource-card')){
        let gradient=skin.querySelector('linearGradient');
        if(!gradient){
          const defs=document.createElementNS(skin.namespaceURI,'defs');
          gradient=document.createElementNS(skin.namespaceURI,'linearGradient');
          for(const [offset,name] of [['0','light'],['1','shade']]){
            const stop=document.createElementNS(skin.namespaceURI,'stop');
            stop.setAttribute('offset',offset);stop.classList.add('loom-resource-'+name);gradient.append(stop);
          }
          defs.append(gradient);skin.append(defs);
        }
        // A reconciled clone must not share a paint-server ID with its source.
        gradient.id='loom-resource-gradient-'+(++gradientSequence);
        gradient.setAttribute('gradientUnits','userSpaceOnUse');
        skin.firstChild.setAttribute('fill',`url(#${gradient.id})`);
      }
      card.classList.add('loom-surface');card.append(skin);cards.set(card,skin);observer.observe(card);schedule(card);
      if(card.matches('.pa-panel,.pa-resources,.pa-doc-detail'))prepareNearby(card);
    }
    if(!frame)frame=requestAnimationFrame(flush);
  }
  const previousRender=render;
  render=function(){const result=previousRender();refresh();return result;};
  // Async detail/resource refreshes can replace cards without invoking render().
  new MutationObserver(records=>{
    if(records.some(r=>[...r.removedNodes].some(n=>n.classList?.contains('loom-surface-skin')) || [...r.addedNodes,...r.removedNodes].some(n=>n.nodeType===1 && !n.classList?.contains('loom-surface-skin') && (n.matches?.(widgetSelector) || n.querySelector?.(widgetSelector)))))refresh();
  }).observe(root,{childList:true,subtree:true});
  refresh();
})();
