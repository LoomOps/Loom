// Bounded per-view transport. Payload trees, software combinations and logs stay lazy.
const loomCatalogPages=new Map();let loomCatalogRevision=null;let loomActivityView=null;const loomCatalogOpening=new Set();
// Split panes can show Home alongside an unrelated detail. Read only for visible panes.
function loomHomeVisible(){
 return ws.panes.some(p=>{const v=activeTab(p).view;return !v.entity&&['home','queue'].includes(v.page);});
}
let loomQueueRevision=null,loomQueueRequest=null,loomHomeErrors={},loomWasHomeVisible=false;
function loomLoadHomeData(refreshQueue=false){
 if(!catalogData.capabilities?.catalog_pages)return;
 const revision=loomCatalogStamp('work'),home=loomHomeVisible();
 const enteredHome=home&&!loomWasHomeVisible;loomWasHomeVisible=home;
 if(home)loomCatalogActivity();
 const needsQueue=home||ws.panes.some(p=>{
  const v=activeTab(p).view;return v.entity&&(v.page==='queue'||catEntity(v.entity)?.kind==='job');
 });
 if(!needsQueue||!catalogData.capabilities.live_queue||loomQueueRequest?.revision===revision
   ||(!refreshQueue&&!enteredHome&&loomQueueRevision===revision))return;
 const request={revision};loomQueueRequest=request;
 loomQueueRevision=revision;delete loomHomeErrors.queue;
 loomSearchAction('system','dashboard_queue',catalogData.capabilities?.compact_lists?{compact:true}:{}).then(value=>{
  if(loomQueueRequest!==request||loomCatalogStamp('work')!==revision)return;
  catalogData.queue=value.queue;
  loomCatalogMerge(value);
  catalogRefreshViews();
 }).catch(()=>{
  if(loomQueueRequest===request&&loomCatalogStamp('work')===revision){loomHomeErrors.queue=true;catalogRefreshViews();}
 }).finally(()=>{
  if(loomQueueRequest===request)loomQueueRequest=null;
 });
}
function loomRefreshQueueProgress(){
 if(!document.hidden&&loomHomeVisible()&&catalogData.queue?.unit_entries?.some(unit=>unit.state==='running'))
  loomLoadHomeData(true);
}
setInterval(loomRefreshQueueProgress,300000);
function loomCatalogActivity(){
 const view='all',revision=loomActivityStamp(),key=JSON.stringify([revision,view]);
 if(!loomHomeVisible()||!catalogData.capabilities?.catalog_pages||loomActivityView===key)return;
 loomActivityView=key;delete loomHomeErrors.activity;
 queueMicrotask(async()=>{
  try{
   const activity=await loomSearchAction('system','dashboard_activity',{view});
   if(loomActivityView!==key||loomActivityStamp()!==revision)return;
   if(!Array.isArray(activity?.days))throw new Error('Invalid Activity response');
   catalogData.activity={...activity,view};catalogRefreshViews();
  }catch{if(loomActivityView===key){loomHomeErrors.activity=true;catalogRefreshViews();}}
 });
}
function loomHomeStatus(kind){
 if(kind==='activity'&&!loomHomeErrors.activity)
  return '<div class="pa-registry-status" role="status" data-activity-loading><i aria-hidden="true"></i>Loading…</div>';
 return '<span role="status">'+(loomHomeErrors[kind]?'조회 실패 <button data-home-retry="'+kind+'">Retry</button>':'…')+'</span>';
}
root.addEventListener('click',event=>{
 const button=event.target.closest('[data-home-retry]');if(!button)return;
 if(button.dataset.homeRetry==='activity')loomActivityView=null;else loomQueueRevision=null;
 loomLoadHomeData();catalogRefreshViews();
});
const loomHomeActivityPanel=dashboardActivity;
dashboardActivity=function(){
 const loaded=Array.isArray(catalogData.activity?.days);
 if(loomHomeErrors.activity)
  return panel('Activity',loomHomeStatus('activity'))+(loaded?loomHomeActivityPanel():'');
 if(!loaded&&(catalogData.revision===null||catalogData.capabilities?.catalog_pages))
  return panel('Activity',loomHomeStatus('activity'));
 return loomHomeActivityPanel();
};
function loomCatalogStamp(domain){return catalogData.domains?.[domain]??catalogData.revision;}
function loomActivityStamp(){return JSON.stringify([loomCatalogStamp('work'),loomCatalogStamp('outputs'),loomCatalogStamp('history')]);}
function loomPageDomains(kind){
 if(kind==='job')return ['history','tasks'];
 if(kind==='scenario')return ['tasks'];
 if(kind==='runtime'||kind==='profile')return ['software','runtime_fresh','software_usage','tasks'];
 if(loomSoftwareListKinds.has(kind))return ['software','runtime_fresh','tasks'];
 return ['outputs','tasks'];
}
const loomCatalogTextEdits=new WeakMap();
function loomDebounceCatalogText(view,field,value){
 let pending=loomCatalogTextEdits.get(view);
 if(!pending){pending={value:view[field]||'',timer:null};loomCatalogTextEdits.set(view,pending);}
 clearTimeout(pending.timer);view[field]=value;
 pending.timer=setTimeout(()=>{
  loomCatalogTextEdits.delete(view);
  if(ws.panes.some(p=>activeTab(p).view===view))render();
 },240);
}
const loomSoftwareListKinds=new Set(['profile','component','tool','runtime','dependency']);
const loomCatalogKinds={models:'model',datasets:'dataset',experiments:'experiment',issues:'issue',profiles:'profile',components:'component',tools:'tool',runtimes:'runtime',dependencies:'dependency',scenarios:'scenario',queue:'job',files:'explorer'};
function loomCatalogQuery(kind,filterTask=true){
 const projection=catalogData.capabilities?.compact_lists?{projection:'list'}:{};
 if(catalogData.capabilities?.job_units&&['dataset','model','experiment'].includes(kind)){
  return {kinds:[kind],view:'all',role:'all',limit:50,records_only:true,...projection,catalog_classes:loomOutputClasses(),...(catalogData.capabilities.output_disposition?{output_state:loomOutputDiscarded()?'discarded':'current',...(loomOutputDiscarded()?{history:'all'}:{})}:{}),...(filterTask&&state.researchTask?{task_id:state.researchTask.id}:{})};
 }if(kind==='job')return {kinds:['job'],view:'all',role:'all',...(catalogData.capabilities?.compact_lists?{projection:'history',limit:10}:{limit:50})};if(kind==='explorer')return {kinds:['dataset','model','experiment','artifact'],view:'all',role:'all',limit:50};const view=catalogData.presentation_view||'all',text=loomCatalogTextEdits.get(state)?.value??(state.extensionQuery||state.dependencyQuery);return {...(['profile','component','tool','runtime'].includes(kind)?{...loomRegistryArgs(state.registryFilter||'current'),...(state.registryFilter==='archived'?{history:'all'}:{})}:{}),...(loomSoftwareListKinds.has(kind)?projection:{}),kinds:[kind],view:view==='builtin'?'all':view,role:['model','dataset','experiment'].includes(kind)?((state.outputPurpose||(view==='research'?'research':'all'))==='research'?'main':'all'):kind==='dependency'||view==='research'?'main':'all',...(view==='builtin'?{origin_type:'builtin'}:{}),...(['model','dataset','experiment'].includes(kind)&&state.researchTask?{task_id:state.researchTask.id}:{}),...(text?{text}:{}),limit:50};}
function loomCatalogPageKey(kind){return JSON.stringify(loomCatalogQuery(kind));}
// A bootstrap is a bounded seed, never a replacement for pages from its revision.
function loomCatalogRefresh(value){
 const changed=loomCatalogRevision!==value.revision||
   value.domains&&JSON.stringify(value.domains)!==JSON.stringify(catalogData.domains);
 const previous=catalogData.all_entities||catalogData.entities||[];
 const open=new Set(ws.panes.map(p=>activeTab(p).view.entity).filter(Boolean));
 if(changed){
  const scoped=value.domains&&catalogData.domains;
  const priorDomains=catalogData.domains;
  const affected=domains=>!scoped||domains.some(domain=>value.domains[domain]!==priorDomains[domain]);
  for(const [key,page] of loomCatalogPages){
   const kind=page.query.projection==='history'?'job':page.query.kinds?.[0];
   if(!affected(loomPageDomains(kind)))continue;
   page.controller?.abort();
   // Preserve the last complete page while refreshing only the first page.
   loomCatalogPages.set(key,{query:page.query,ids:page.ids||[],taskIndex:page.taskIndex,committed:page.committed,desired:kind==='job'&&value.capabilities?.compact_lists?undefined:
    Math.min(value.capabilities?.compact_lists?100:4000,page.ids?.length||page.query.limit||50),loaded:false,busy:false,cursor:null,error:null});
  }
  if(affected(['tasks']))loomScenarioGraphs.clear();
  loomCatalogRevision=value.revision;
  for(const id of open)queueMicrotask(async()=>{
   const kind=catEntity(id)?.kind;
   if(!catalogData.capabilities?.catalog_pages||loomCatalogOpening.has(id)
      ||kind&& !affected(kind==='job'?['work','history','tasks']:loomPageDomains(kind==='execution_profile'?'profile':kind)))return;
   if(kind==='job' && catEntity(id)?.loom_job)return;
   loomCatalogOpening.add(id);
   try{await loomCatalogOpen(id,false);}catch(error){
    // Keep the last good detail on a transient transport failure; an explicit
    // missing source removes it rather than retaining a deleted record.
    if(error.code==='unknown_item'||error.code==='not_found'){
     catalogData.all_entities=(catalogData.all_entities||[]).filter(e=>e.id!==id);
     catalogData.entities=catalogData.entities.filter(e=>e.id!==id);catalogRefreshViews();
    }
   }finally{loomCatalogOpening.delete(id);}
  });
 }
 const retained=new Set([...open,...[...loomCatalogPages.values()].flatMap(page=>page.ids||[])]);
 const old=changed?previous.filter(e=>retained.has(e.id)||catalogData.capabilities?.catalog_pages&&['model','dataset','run','experiment'].includes(e.kind)):previous;
 const records=new Map(old.map(e=>[e.id,e]));
 for(const e of value.all_entities||value.entities){const old=records.get(e.id);records.set(e.id,old?.kind==='scenario'&&old.loom_scenario.revision>e.loom_scenario?.revision?old:loomCatalogRetainResearch(old,e,!changed));}
 loomPresentationView=value.presentation_view||catalogData.presentation_view||'all';
 queueMicrotask(loomLoadHomeData);
 const activity=catalogData.activity?.view==='all'?catalogData.activity:value.activity;
 return catTitleSnapshot(loomCatalogProjection({...value,queue:value.queue||catalogData.queue,runtime_catalog:value.runtime_catalog||catalogData.runtime_catalog,activity,entities:[...records.values()]}));
}
function loomCatalogRetainResearch(previous,incoming,sameRevision){
 if(!sameRevision||!previous?.research_summary||!incoming.research_summary)return incoming;
 const old=previous.research_summary,next=incoming.research_summary;
 if((old.materialized||old.sources?.some(s=>s.status==='verified'))&&!next.materialized&&!(next.sources||[]).length){
  const seen=new Set((next.facts||[]).map(f=>f.id));
  return {...incoming,research_summary:{...next,facts:[...(next.facts||[]),...(old.facts||[]).filter(f=>!seen.has(f.id))],inputs:next.inputs?.length?next.inputs:old.inputs,sources:old.sources,...(old.materialized?{materialized:old.materialized}:{})}};
 }
 return incoming;
}
function loomMergeCatalogRecord(previous,incoming){
 if(!previous||!incoming.summary_only||previous.summary_only||!previous.item_revision_id
    ||previous.item_revision_id!==incoming.item_revision_id)return incoming;
 // The exact detail and the compact row share identity. A refresh of the
 // row must preserve the already opened sections and their source references.
 const value={...previous,...incoming,properties:{...previous.properties,...incoming.properties},
  summary_only:false};
 if(previous.reporting)value.reporting={...previous.reporting,...incoming.reporting};
 return loomCatalogRetainResearch(previous,value,true);
}
function loomCatalogMerge(value){
 const existing=new Map((catalogData.all_entities||catalogData.entities).map(e=>[e.id,e]));
 loomPresentationView=catalogData.presentation_view||'all';
 const incoming=loomCatalogProjection(value);
 for(const e of incoming.all_entities||incoming.entities){const old=existing.get(e.id);existing.set(e.id,old?.kind==='scenario'&&old.loom_scenario.revision>e.loom_scenario?.revision?old:loomMergeCatalogRecord(old,e));}
 // Old list pages can be refetched; keep the current record and recent browsing bounded.
 while(existing.size>4000){const id=existing.keys().next().value;if(id===state.entity){const value=existing.get(id);existing.delete(id);existing.set(id,value);}else existing.delete(id);}
 const entities=[...existing.values()];
 const selected=entities.filter(e=>catalogData.presentation_view==='all'||(catalogData.presentation_view==='builtin'?e.owner_kind==='built-in':catalogData.presentation_view==='operations'?e.presentation?.scope==='operations':e.presentation?.visible!==false));
 catalogData={...catalogData,research_tasks:incoming.research_tasks??catalogData.research_tasks,entities:selected,all_entities:entities};
}
async function loomCatalogLoad(kind,more=false,query=loomCatalogQuery(kind)){
 if(!catalogData.capabilities?.catalog_pages||query.catalog_classes?.length===0
    ||kind==='explorer'&&catalogData.capabilities?.compact_lists)return;
 const key=JSON.stringify(query);
 let page=loomCatalogPages.get(key);
 if(page?.busy||page?.loaded&&!more||more&&!page?.cursor)return;
 if(!page){page={query,cursor:null,busy:false,loaded:false,error:null,ids:[]};loomCatalogPages.set(key,page);}
 page.busy=true;page.error=null;
 const controller=new AbortController();page.controller=controller;
 try{
  let cursor=more?page.cursor:null,result,entities=[],taskIndex;
  const desired=more?query.limit:page.desired||query.limit;
  do{
   result=await loomSearchAction('system','dashboard_page',{...page.query,...(cursor?{cursor}:{})},controller.signal);
   if(loomCatalogPages.get(key)!==page)return;
   if(!cursor&&!result.partial)taskIndex=result.task_index;
   entities.push(...result.entities);cursor=result.next_cursor;
  }while(!more&&cursor&&entities.length<desired&&!result.partial&&entities.length<4000);
  // Commit a whole replacement, including previously loaded pages, in one paint.
  const ids=[...new Set(entities.map(e=>e.id))];
  if(kind!=='job'||!result.partial){
   page.ids=more?[...new Set([...page.ids,...ids])]:result.partial&&page.ids.length>ids.length?page.ids:ids;
   loomCatalogMerge({...result,entities});
   if(['runtime','dependency'].includes(kind)&&result.runtime_catalog)catalogData.runtime_catalog=result.runtime_catalog;
   if(taskIndex)page.taskIndex=taskIndex;
   page.cursor=cursor;page.committed=true;
  }
  page.loaded=true;page.partial=result.partial;
  if(result.partial&&(!more||kind==='job')&&(page.retries||0)<4){
   page.retries=(page.retries||0)+1;
   setTimeout(()=>{if(loomCatalogPages.get(key)===page){page.loaded=false;loomCatalogLoad(kind,more,page.query);}},500*page.retries);
  }else if(result.partial){
   page.indexRetryExhausted=true;
   if(kind==='job')page.error='종료 이력을 불러오지 못했습니다.';
  }
 }catch(error){
  if(controller.signal.aborted)return;
  page.error=error.message;
  // A projection can advance between cursor pages. Retain the last complete
  // list and restart from the first page; never splice incompatible snapshots.
  if((page.failures||0)<3){page.failures=(page.failures||0)+1;setTimeout(()=>{
   if(loomCatalogPages.get(key)!==page)return;page.loaded=false;page.error=null;loomCatalogLoad(kind,false,page.query);
  },250*2**(page.failures-1));}
 }finally{
  page.busy=false;if(page.controller===controller)delete page.controller;
  if(!controller.signal.aborted&&loomCatalogPages.get(key)===page)catalogRefreshViews();
 }
}
function loomRegistryListStatus(kind){
 if(!catalogData.capabilities?.catalog_pages)return '';
 const page=loomCatalogPages.get(loomCatalogPageKey(kind));
 if(page?.loaded&&!page.partial&&!page.error)return '';
 const stalled=page?.partial&&page.indexRetryExhausted&&!page.busy;
 const label=page?.error|| (page?.partial?'Indexing…':'Loading…');
 const retry=page?.error||stalled;
 return '<div class="pa-registry-status" role="status">'+(retry?'':'<i aria-hidden="true"></i>')+esc(label)+(retry?' <button data-catalog-restart data-catalog-more="'+esc(kind)+'">Retry</button>':'')+'</div>';
}
function loomCatalogPager(kind,label="More",inlineStatus=false){
 if(kind==='job'&&catalogData.capabilities?.compact_lists)label='더보기';
 if(!catalogData.capabilities?.catalog_pages)return '';
 const query=loomCatalogQuery(kind),key=JSON.stringify(query),page=loomCatalogPages.get(key);
 if(!page?.loaded&&!page?.busy&&!page?.error)queueMicrotask(()=>loomCatalogLoad(kind,false,query));
 if(inlineStatus)return '';
 const pending=page?.busy||kind==='job'&&(!page?.loaded||page.partial);
 return '<div class="pa-catalog-pagination" role="status">'+(page?.error?esc(page.error)+' <button data-catalog-more="'+esc(kind)+'">Retry</button>':pending?'…':page?.cursor?'<button data-catalog-more="'+esc(kind)+'">'+esc(label)+'</button>':'')+'</div>';
}
const loomPagedCatalogList=catalogList;
catalogList=function(kind){return loomPagedCatalogList(kind)+loomCatalogPager(kind);};
const loomPagedSoftware=loomSoftwareView;
loomSoftwareView=function(kind){const html=loomPagedSoftware(kind);return html+loomCatalogPager(kind,'More',html.includes('class="pa-registry-status"'));};
const loomPagedProfiles=loomProfilesView;
loomProfilesView=function(){const html=loomPagedProfiles();return html+loomCatalogPager('profile','More',html.includes('class="pa-registry-status"'));};
const loomPagedDependencies=loomDependenciesView;
loomDependenciesView=function(){const html=loomPagedDependencies();return html+loomCatalogPager('dependency','More',html.includes('class="pa-registry-status"'));};
const loomPagedScenarios=loomScenarioView;
loomScenarioView=function(){return loomPagedScenarios()+loomCatalogPager('scenario');};
const loomPagedRender=render;
render=function(){
 if(loomCatalogRevision!==catalogData.revision)loomCatalogRevision=catalogData.revision;
 loomPagedRender();
 const kind=loomCatalogKinds[state.page];
 if(kind&&!state.entity)loomCatalogPager(kind);
 const detail=state.entity&&catEntity(state.entity);
 const needsDetail=(detail?.summary_only&&!(detail.kind==='job'&&detail.loom_job))||detail?.kind==='model'&&detail.loom_registry&&!detail.deferred_output_sections&&!Object.hasOwn(detail,'policy_training_revisions');
 if(catalogData.capabilities?.catalog_pages&&state.entity&&(!detail||needsDetail)&&!loomCatalogOpening.has(state.entity)){
  const id=state.entity;loomCatalogOpening.add(id);queueMicrotask(()=>loomCatalogOpen(id,!detail).catch(()=>{}).finally(()=>loomCatalogOpening.delete(id)));
 }
 loomLoadHomeData();
 const visible=new Set();
 const view=state;
 for(const pane of ws.panes){
  state=activeTab(pane).view;
  if(state.entity)continue;
  const kind=loomCatalogKinds[state.page];
  if(kind)visible.add(loomCatalogPageKey(kind));
  if(state.page==='comparison')for(const kind of ['model','experiment'])visible.add(loomCatalogPageKey(kind));
 }
 state=view;
 for(const [key,page] of loomCatalogPages){
  const kind=page.query.kinds?.[0];
  if(page.query.text&&loomSoftwareListKinds.has(kind)&&page.busy&&!visible.has(key))page.controller?.abort();
 }
};
document.addEventListener('click',event=>{const b=event.target.closest('[data-catalog-more]');if(!b)return;event.preventDefault();const pane=b.closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;state=activeTab().view;const kind=b.dataset.catalogMore,page=loomCatalogPages.get(loomCatalogPageKey(kind));if(page?.error||page?.partial){page.loaded=false;page.error=null;page.failures=0;page.retries=0;page.indexRetryExhausted=false;}loomCatalogLoad(kind,!!page?.cursor&&!b.hasAttribute('data-catalog-restart'));});
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
async function loomCatalogOpen(id,navigate=true){
 const tabKeyBefore=activeTab().id,known=catEntity(id);
 const section=catalogData.capabilities?.compact_lists?{section:'overview'}:{};
 const exact=/^revision:([0-9]+):([0-9]+)$/.exec(id);
 if(exact){
  const value=await loomSearchAction('system','dashboard_entity',{item_id:exact[1],revision_id:exact[2],records_only:true,...section});
  loomCatalogMerge({...value,entities:value.entities.map(e=>({...e,id,catalog_exact:known?.catalog_exact??true}))});
  if(navigate&&activeTab().id===tabKeyBefore&&catEntity(id))openView(catView(id));
  catalogRefreshViews();return;
 }
 if(known?.item_id){
  const value=await loomSearchAction('system','dashboard_entity',{item_id:known.item_id,...section,...(known.item_revision_id&&known.kind!=='scenario'?{revision_id:known.item_revision_id}:{})});
  loomCatalogMerge(value);if(navigate&&activeTab().id===tabKeyBefore)openView(catView(id));catalogRefreshViews();return;
 }
 if(/^(history|dependency):[0-9]+$/.test(id)){
  const value=await loomSearchAction('system','dashboard_entity',{item_id:id.slice(id.indexOf(':')+1),...section});
  loomCatalogMerge(value);if(navigate&&activeTab().id===tabKeyBefore)openView(catView(value.entities[0]?.id||id));catalogRefreshViews();return;
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
 } else native={kind:'entity',identity:{id}};
 let hit;
 // An unloaded plain ID can belong to a Job or Task, whose ID is not a name
 // search term. Resolve each native authority before falling back to discovery.
 const candidates=native?[native,...(native.kind==='entity'?[{kind:'job',identity:{id}},{kind:'scenario',identity:{id}}]:[])]:[];
 for(const candidate of candidates){try{hit=await loomSearchAction('results','resolve_item',{native_ref:candidate});break;}catch{}}
 if(!hit){const name=native?.identity?.id||native?.identity?.kind||id;const page=await loomSearchAction('results','search_items',{text:name,view:'all',role:'all',limit:10});hit=page.items.find(e=>(e.native_ref?.identity?.id===name||e.native_ref?.identity?.kind===name)&&(!native?.identity?.revision||e.native_ref.identity.revision===native.identity.revision));}
 if(!hit)throw Error('Record unavailable');
 const value=await loomSearchAction('system','dashboard_entity',{item_id:hit.item_id,...section,...(hit.revision_id?{revision_id:hit.revision_id}:{})});
 loomCatalogMerge(value);if(navigate&&activeTab().id===tabKeyBefore)openView(catView(catEntity(id)?id:value.entities[0]?.id||id));catalogRefreshViews();
}
root.addEventListener('click',event=>{const b=event.target.closest('[data-entity]');if(!catalogData.capabilities?.catalog_pages||!b||catEntity(b.dataset.entity))return;event.preventDefault();event.stopImmediatePropagation();loomCatalogOpen(b.dataset.entity).catch(error=>{b.title=error.message;});},true);

function loomTaskItems(items,kind){
 if(!['model','dataset','experiment','run'].includes(kind))return items;
 const listKind=kind==='run'?'experiment':kind,current=loomCatalogKinds[state.page];
 const page=!state.entity&&current===listKind?loomCatalogPages.get(loomCatalogPageKey(listKind)):null;
 if(page?.loaded||page?.ids?.length){const rank=new Map(page.ids.map((id,i)=>[id,i]));items=items.filter(e=>rank.has(e.id)).sort((a,b)=>rank.get(a.id)-rank.get(b.id));}
 return items.filter(e=>!state.researchTask||(e.task_refs||[]).some(t=>t.id===state.researchTask.id))
 .sort((a,b)=>(a.task_refs?.[0]?.label||'\uffff').localeCompare(b.task_refs?.[0]?.label||'\uffff'));
}
function loomTaskOptions(){
 const tasks=new Map((catalogData.research_tasks||[]).map(t=>[t.id,t]));
 // A bounded bootstrap may precede the Task page. Keep known accepted Task
 // metadata available when moving from its detail to a sibling output list.
 for(const entity of loomScenarios()){
  const task=entity.loom_scenario,known=tasks.get(entity.id);
  if(task.archived){tasks.delete(entity.id);continue;}
  if(!known||task.revision>known.revision)tasks.set(entity.id,{id:entity.id,revision:task.revision,label:entity.label});
 }
 return [...tasks.values()].sort((a,b)=>a.label.localeCompare(b.label));
}
function loomTaskLabels(entity){return (entity.task_refs||[]).map(t=>'<a class="pa-task-label" href="#/entities/'+encodeURIComponent(t.id)+'" data-entity="'+esc(t.id)+'">'+esc(t.label)+'</a>').join(' · ');}
function loomTaskProperty(entity){const links=loomTaskLabels(entity);return links?'<div class="pa-output-task"><span>Task</span><span>'+links+'</span></div>':'';}
function loomOutputRow(markup,entity){
 const singleTask=Object.hasOwn(loomOutputPages,state.page)&&!state.entity&&!state.model&&loomCatalogTask(entity)&&new Set(entity.task_refs.map(task=>task.id)).size===1;
 const row=markup.replace('<article ','<article data-output-id="'+esc(entity.id)+'" ');
 return singleTask?row:row.replace(/(<article[^>]*>)/,'$1'+loomTaskProperty(entity));
}
const loomTaskCatalogRow=catListRow;
catListRow=function(entity){return loomOutputRow(loomTaskCatalogRow(entity),entity);};
const loomTaskResultCard=resultCard;
resultCard=function(entity){return loomOutputRow(loomTaskResultCard(entity),entity);};
const loomPagedFiles=files;files=function(){return loomPagedFiles()+loomCatalogPager("explorer");};

// Only background patches use this anchor. User navigation keeps normal scroll behavior.
function loomPreserveViewport(body,patch){
 if(!body.querySelector('[data-output-id]'))return patch();
 const edge=body.getBoundingClientRect(),top=body.scrollTop,left=body.scrollLeft;
 const anchors=[...body.querySelectorAll('[data-output-id]')].filter(e=>e.getClientRects().length)
  .map(e=>({id:e.dataset.outputId,offset:e.getBoundingClientRect().top-edge.top,bottom:e.getBoundingClientRect().bottom-edge.top}))
  .filter(e=>e.bottom>0&&e.offset<edge.height).slice(0,20);
 patch();
 const anchor=anchors.map(a=>({a,node:[...body.querySelectorAll('[data-output-id]')].find(e=>e.dataset.outputId===a.id)})).find(x=>x.node?.getClientRects().length);
 body.scrollTop=anchor?body.scrollTop+anchor.node.getBoundingClientRect().top-body.getBoundingClientRect().top-anchor.a.offset:top;
 body.scrollLeft=left;
}
