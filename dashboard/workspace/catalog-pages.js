// Bounded per-view transport. Payload trees, software combinations and logs stay lazy.
const loomCatalogPages=new Map();let loomCatalogRevision=null;let loomActivityView=null;const loomCatalogOpening=new Set();
const loomCatalogKinds={models:'model',datasets:'dataset',experiments:'experiment',profiles:'profile',components:'component',tools:'tool',runtimes:'runtime',dependencies:'dependency',scenarios:'scenario',queue:'job'};
function loomCatalogQuery(kind){const view=kind==='job'?'all':catalogData.presentation_view||'research';return {kinds:[kind],view:view==='builtin'?'all':view,role:kind==='dependency'||view==='research'?'main':'all',...(view==='builtin'?{origin_type:'builtin'}:{}),...(state.extensionQuery||state.dependencyQuery?{text:state.extensionQuery||state.dependencyQuery}:{}),limit:50};}
function loomCatalogPageKey(kind){return JSON.stringify(loomCatalogQuery(kind));}
function loomCatalogMerge(value){
 const existing=new Map((catalogData.all_entities||catalogData.entities).map(e=>[e.id,e]));
 loomPresentationView=catalogData.presentation_view||'research';
 const incoming=loomCatalogProjection(value);
 for(const e of incoming.all_entities||incoming.entities)existing.set(e.id,e);
 // Old list pages can be refetched; keep the current record and recent browsing bounded.
 while(existing.size>4000){const id=existing.keys().next().value;if(id===state.entity){const value=existing.get(id);existing.delete(id);existing.set(id,value);}else existing.delete(id);}
 const entities=[...existing.values()];
 const selected=entities.filter(e=>catalogData.presentation_view==='all'||(catalogData.presentation_view==='builtin'?e.owner_kind==='built-in':catalogData.presentation_view==='operations'?e.presentation?.scope==='operations':e.presentation?.visible!==false));
 catalogData={...catalogData,entities:selected,all_entities:entities};
}
async function loomCatalogLoad(kind,more=false,query=loomCatalogQuery(kind)){
 if(!catalogData.capabilities?.catalog_pages)return;
 const key=JSON.stringify(query);
 let page=loomCatalogPages.get(key);
 if(page?.busy||page?.loaded&&!more||more&&!page?.cursor)return;
 if(!page){page={query,cursor:null,busy:false,loaded:false,error:null};loomCatalogPages.set(key,page);}
 page.busy=true;page.error=null;
 try{
  const result=await loomSearchAction('system','dashboard_page',{...page.query,...(more?{cursor:page.cursor}:{})});
  if(loomCatalogPages.get(key)!==page)return;
  loomCatalogMerge(result);page.cursor=result.next_cursor;page.loaded=true;page.partial=result.partial;if(result.partial&&!more&&(page.retries||0)<4){page.retries=(page.retries||0)+1;setTimeout(()=>{if(loomCatalogPages.get(key)===page){page.loaded=false;loomCatalogLoad(kind,false,page.query);}},500*page.retries);}
 }catch(error){page.error=error.message;}finally{page.busy=false;if(loomCatalogPages.get(key)===page)catalogRefreshViews();}
}
function loomCatalogPager(kind){
 if(!catalogData.capabilities?.catalog_pages)return '';
 const query=loomCatalogQuery(kind),key=JSON.stringify(query),page=loomCatalogPages.get(key);
 if(!page?.loaded&&!page?.busy&&!page?.error)queueMicrotask(()=>loomCatalogLoad(kind,false,query));
 return '<div class="pa-catalog-pagination" role="status">'+(page?.error?esc(page.error)+' <button data-catalog-more="'+esc(kind)+'">Retry</button>':page?.busy?'…':page?.cursor?'<button data-catalog-more="'+esc(kind)+'">More</button>':page?.partial?'Indexing…':'')+'</div>';
}
const loomPagedCatalogList=catalogList;
catalogList=function(kind){return loomPagedCatalogList(kind)+loomCatalogPager(kind);};
const loomPagedSoftware=loomSoftwareView;
loomSoftwareView=function(kind){return loomPagedSoftware(kind)+loomCatalogPager(kind);};
const loomPagedProfiles=loomProfilesView;
loomProfilesView=function(){return loomPagedProfiles()+loomCatalogPager('profile');};
const loomPagedDependencies=loomDependenciesView;
loomDependenciesView=function(){return loomPagedDependencies()+loomCatalogPager('dependency');};
const loomPagedScenarios=loomScenarioView;
loomScenarioView=function(){return loomPagedScenarios()+loomCatalogPager('scenario');};
const loomPagedRender=render;
render=function(){
 if(loomCatalogRevision!==catalogData.revision){loomCatalogPages.clear();loomCatalogRevision=catalogData.revision;}
 loomPagedRender();
 const kind=loomCatalogKinds[state.page];
 if(kind&&!state.entity)loomCatalogPager(kind);
 if(catalogData.capabilities?.catalog_pages&&state.entity&&!catEntity(state.entity)&&!loomCatalogOpening.has(state.entity)){
  const id=state.entity;loomCatalogOpening.add(id);queueMicrotask(()=>loomCatalogOpen(id).catch(()=>{}).finally(()=>loomCatalogOpening.delete(id)));
 }
 const view=catalogData.presentation_view==='builtin'?'all':catalogData.presentation_view||'research';
 if(catalogData.capabilities?.catalog_pages&&loomActivityView!==view){
  loomActivityView=view;
  queueMicrotask(async()=>{try{const activity=await loomSearchAction('system','dashboard_activity',{view});if(loomActivityView===view){catalogData.activity=activity;catalogRefreshViews();}}catch{}});
 }
 const queue=root.querySelector('.pa-queue-caption');
 if(state.page==='queue'&&queue&&!root.querySelector('[data-catalog-queue-more]')){const v=document.createElement('div');v.dataset.catalogQueueMore='';v.innerHTML=loomCatalogPager('job');queue.after(v);}
};
document.addEventListener('click',event=>{const b=event.target.closest('[data-catalog-more]');if(!b)return;event.preventDefault();const pane=b.closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;state=activeTab().view;const kind=b.dataset.catalogMore,page=loomCatalogPages.get(loomCatalogPageKey(kind));if(page?.error){page.loaded=false;page.error=null;}loomCatalogLoad(kind,!!page?.cursor);});
const loomPagedActivitySummary=activitySummary;
activitySummary=function(entities,clock=Date.now(),through=clock){
 if(!catalogData.activity?.days)return loomPagedActivitySummary(entities,clock,through);
 return new Map(catalogData.activity.days.map(d=>{
  const jobs=new Map(d.jobs.map(j=>[j.id,{e:catEntity(j.id)||{id:j.id,label:j.id,kind:'job',properties:{record_role:'loom_job'}},time:j.time*1000}]));
  Object.defineProperty(jobs,'size',{value:d.job_count});
  const assets=new Set();Object.defineProperty(assets,'size',{value:d.asset_count});
  const unknownTime=new Set();Object.defineProperty(unknownTime,'size',{value:d.unknown_time_count});
  return [d.day,{jobs,assets,milliseconds:d.milliseconds,unknownTime,live:d.live,stale:d.live&&catalogData.activity.through*1000<clock-60000}];
 }));
};
async function loomCatalogOpen(id){
 const tabKeyBefore=activeTab().id;
 if(/^(history|dependency):[0-9]+$/.test(id)){
  const value=await loomSearchAction('system','dashboard_entity',{item_id:id.slice(id.indexOf(':')+1)});
  loomCatalogMerge(value);if(activeTab().id===tabKeyBefore)openView(catView(value.entities[0]?.id||id));catalogRefreshViews();return;
 }
 let native=null;
 if(id.startsWith('profile:')) {
  native={kind:'profile',identity:{kind:id.slice(8)}};
 } else if(/^(component|tool):/.test(id)) {
  const p=id.split(':');
  if(p.length===4)native={kind:p[0],identity:{owner:p[1],id:p[2],revision:Number(p[3].replace(/^r/,''))}};
 } else if(id.startsWith('runtime:')) {
  const p=id.split(':');
  if(p.length===3)native={kind:'runtime',identity:{id:p[1],revision:Number(p[2])}};
 }
 let hit;
 if(native){try{hit=await loomSearchAction('results','resolve_item',{native_ref:native});}catch{}}
 if(!hit){const name=native?.identity?.id||native?.identity?.kind||id;const page=await loomSearchAction('results','search_items',{text:name,view:'all',role:'all',limit:10});hit=page.items.find(e=>(e.native_ref?.identity?.id===name||e.native_ref?.identity?.kind===name)&&(!native?.identity?.revision||e.native_ref.identity.revision===native.identity.revision));}
 if(!hit)throw Error('Record unavailable');
 const value=await loomSearchAction('system','dashboard_entity',{item_id:hit.item_id,...(hit.revision_id?{revision_id:hit.revision_id}:{})});
 loomCatalogMerge(value);if(activeTab().id===tabKeyBefore)openView(catView(catEntity(id)?id:value.entities[0]?.id||id));catalogRefreshViews();
}
root.addEventListener('click',event=>{const b=event.target.closest('[data-entity]');if(!catalogData.capabilities?.catalog_pages||!b||catEntity(b.dataset.entity))return;event.preventDefault();event.stopImmediatePropagation();loomCatalogOpen(b.dataset.entity).catch(error=>{b.title=error.message;});},true);
