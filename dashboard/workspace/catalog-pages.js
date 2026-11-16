// Bounded per-view transport. Payload trees, software combinations and logs stay lazy.
const loomCatalogPages=new Map();let loomCatalogRevision=null;let loomActivityView=null;const loomCatalogOpening=new Set();
const loomCatalogKinds={models:'model',datasets:'dataset',experiments:'experiment',issues:'issue',profiles:'profile',components:'component',tools:'tool',runtimes:'runtime',dependencies:'dependency',scenarios:'scenario',queue:'job',files:'explorer'};
function loomCatalogQuery(kind){if(kind==='explorer')return {kinds:['dataset','model','experiment','artifact'],view:'all',role:'all',limit:50};const view=kind==='job'?(state.jobHistoryView||catalogData.presentation_view||'research'):catalogData.presentation_view||'research';return {kinds:[kind],view:view==='builtin'?'all':view,role:kind==='job'?'all':kind==='dependency'||view==='research'?'main':'all',...(view==='builtin'?{origin_type:'builtin'}:{}),...(['model','dataset','experiment'].includes(kind)&&state.researchTask?{task_id:state.researchTask.id}:{}),...(state.extensionQuery||state.dependencyQuery?{text:state.extensionQuery||state.dependencyQuery}:{}),limit:50};}
function loomCatalogPageKey(kind){return JSON.stringify(loomCatalogQuery(kind));}
// A bootstrap is a bounded seed, never a replacement for pages from its revision.
function loomCatalogRefresh(value){
 const changed=loomCatalogRevision!==value.revision;
 const previous=catalogData.all_entities||catalogData.entities||[];
 const open=new Set(ws.panes.map(p=>activeTab(p).view.entity).filter(Boolean));
 if(changed){loomCatalogPages.clear();loomScenarioGraphs.clear();loomCatalogRevision=value.revision;loomActivityView=null;
  for(const id of open)queueMicrotask(async()=>{
   if(!catalogData.capabilities?.catalog_pages||loomCatalogOpening.has(id))return;
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
 const old=changed?previous.filter(e=>open.has(e.id)):previous;
 const records=new Map(old.map(e=>[e.id,e]));
 for(const e of value.all_entities||value.entities){const old=records.get(e.id);records.set(e.id,old?.kind==='scenario'&&old.loom_scenario.revision>e.loom_scenario?.revision?old:e);}
 loomPresentationView=value.presentation_view||catalogData.presentation_view||'research';
 return catTitleSnapshot(loomCatalogProjection({...value,entities:[...records.values()]}));
}
function loomCatalogMerge(value){
 const existing=new Map((catalogData.all_entities||catalogData.entities).map(e=>[e.id,e]));
 loomPresentationView=catalogData.presentation_view||'research';
 const incoming=loomCatalogProjection(value);
 for(const e of incoming.all_entities||incoming.entities){const old=existing.get(e.id);existing.set(e.id,old?.kind==='scenario'&&old.loom_scenario.revision>e.loom_scenario?.revision?old:e);}
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
// Models and Experiments use their own filter layouts rather than catalogList.
const loomPagedModels=modelList;
modelList=function(){return loomPagedModels()+loomCatalogPager('model');};
const loomPagedExperiments=experiments;
experiments=function(){return loomPagedExperiments()+loomCatalogPager('experiment');};
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
 if(loomCatalogRevision!==catalogData.revision){loomCatalogPages.clear();loomCatalogRevision=catalogData.revision;loomActivityView=null;}
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
async function loomCatalogOpen(id,navigate=true){
 const tabKeyBefore=activeTab().id,known=catEntity(id);
 if(known?.item_id){
  const value=await loomSearchAction('system','dashboard_entity',{item_id:known.item_id,...(known.item_revision_id&&known.kind!=='scenario'?{revision_id:known.item_revision_id}:{})});
  loomCatalogMerge(value);if(navigate&&activeTab().id===tabKeyBefore)openView(catView(id));catalogRefreshViews();return;
 }
 if(/^(history|dependency):[0-9]+$/.test(id)){
  const value=await loomSearchAction('system','dashboard_entity',{item_id:id.slice(id.indexOf(':')+1)});
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
 }
 let hit;
 if(native){try{hit=await loomSearchAction('results','resolve_item',{native_ref:native});}catch{}}
 if(!hit){const name=native?.identity?.id||native?.identity?.kind||id;const page=await loomSearchAction('results','search_items',{text:name,view:'all',role:'all',limit:10});hit=page.items.find(e=>(e.native_ref?.identity?.id===name||e.native_ref?.identity?.kind===name)&&(!native?.identity?.revision||e.native_ref.identity.revision===native.identity.revision));}
 if(!hit)throw Error('Record unavailable');
 const value=await loomSearchAction('system','dashboard_entity',{item_id:hit.item_id,...(hit.revision_id?{revision_id:hit.revision_id}:{})});
 loomCatalogMerge(value);if(navigate&&activeTab().id===tabKeyBefore)openView(catView(catEntity(id)?id:value.entities[0]?.id||id));catalogRefreshViews();
}
root.addEventListener('click',event=>{const b=event.target.closest('[data-entity]');if(!catalogData.capabilities?.catalog_pages||!b||catEntity(b.dataset.entity))return;event.preventDefault();event.stopImmediatePropagation();loomCatalogOpen(b.dataset.entity).catch(error=>{b.title=error.message;});},true);

function loomTaskItems(items,kind){
 if(!['model','dataset','experiment','run'].includes(kind))return items;
 return items.filter(e=>!state.researchTask||(e.task_refs||[]).some(t=>t.id===state.researchTask.id))
 .sort((a,b)=>(a.task_refs?.[0]?.label||'\uffff').localeCompare(b.task_refs?.[0]?.label||'\uffff'));
}
function loomTaskToolbar(){
 const tasks=catalogData.research_tasks||[],selected=state.researchTask;
 return '<div class="pa-task-filter"><label>Task <select data-research-task><option value="">All tasks</option>'+tasks.map(t=>'<option value="'+esc(t.id+':'+t.revision)+'"'+(selected?.id===t.id&&selected?.revision===t.revision?' selected':'')+'>'+esc(t.label)+'</option>').join('')+'</select></label>'+(catalogData.capabilities?.curate&&catalogData.capabilities?.scenarios?'<button data-scenario-create>New Task</button>':'')+'</div>';
}
function loomTaskLabels(entity){return (entity.task_refs||[]).map(t=>'<button class="pa-task-label" data-entity="'+esc(t.id)+'">'+esc(t.label)+'</button>').join('');}
const loomTaskCatalogRow=catListRow;
catListRow=function(entity){return loomTaskCatalogRow(entity).replace('<div class="pa-row-main">','<div class="pa-row-main">'+loomTaskLabels(entity));};
const loomTaskResultCard=resultCard;
resultCard=function(entity){return loomTaskResultCard(entity).replace('<div class="pa-row-main">','<div class="pa-row-main">'+loomTaskLabels(entity));};
const loomTaskModelList=modelList;
modelList=function(){return loomTaskToolbar()+loomTaskModelList();};
const loomTaskDatasetList=datasets;
datasets=function(){return loomTaskToolbar()+loomTaskDatasetList();};
const loomTaskExperimentList=experiments;
experiments=function(){return loomTaskToolbar()+loomTaskExperimentList();};
root.addEventListener('change',event=>{const field=event.target.closest('[data-research-task]');if(!field)return;const pane=ws.panes.find(p=>p.id===field.closest('[data-pane]')?.dataset.pane);if(!pane)return;const value=field.value;activeTab(pane).view.researchTask=(catalogData.research_tasks||[]).find(t=>t.id+':'+t.revision===value);render();persist();});

const loomPagedFiles=files;files=function(){return loomPagedFiles()+loomCatalogPager("explorer");};
