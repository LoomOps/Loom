// One output workspace; typed records and legacy deep links keep their identity.
const loomOutputPages={datasets:'Datasets',models:'Policies',experiments:'Experiments'};
function loomIsOutputPage(page){return Object.hasOwn(loomOutputPages,page)||page==='scenarios';}
function loomIsResearchPage(page){return loomIsOutputPage(page)||page==='comparison';}
function loomOutputTabs(){
 const selected=state.page;
 return '<nav class="pa-output-tabs" aria-label="Catalog">'+Object.entries(loomOutputPages).map(([page,label])=>'<button data-output-page="'+page+'" aria-current="'+(selected===page?'page':'false')+'">'+label+'</button>').join('')+'</nav>';
}
function loomOutputBreadcrumb(view){
 if(!loomIsOutputPage(view.page))return '작업실 / '+viewTitle(view);
 const label=view.entity||view.model?viewTitle(view):null;
 const section=loomOutputPages[view.page]||'Task';
 return '작업실 / Catalog / '+section+(label?' / '+label:'');
}
function loomOutputChrome(){
 return '<div class="pa-output-header">'+loomOutputTabs()+'</div>';
}
function loomOutputNavigate(page){
 if(!loomIsOutputPage(page))return;
 const pane=activePane(),tab=activeTab();
 const view=viewCopy({...tab.view,page,entity:null,model:null,query:'',routeOwner:null,routeAttempt:null,routeArtifact:null});
 const key=tabKey(view),existing=pane.tabs.find(t=>t!==tab&&t.id===key);
 if(tab.view.entity||tab.view.model){if(existing)existing.view=view;openView(view);return;}
 // Switching output sections reuses this document rather than spawning tabs.
 if(existing){existing.view=view;pane.active=existing.id;}
 else{tab.id=key;tab.view=view;pane.active=key;}
 menu=null;render();persist();
}
const loomOutputOpenView=openView;
openView=function(view,...args){
 const current=activeTab()?.view;
 const task=view.page==='scenarios'&&view.entity?catEntity(view.entity):null;
 if(task?.loom_scenario)view={...view,researchTask:{id:task.id,revision:task.loom_scenario.revision,label:task.label}};
 if(current&&loomIsResearchPage(current.page)&&loomIsResearchPage(view.page)&&view.researchTask===undefined&&current.researchTask)view={...view,researchTask:current.researchTask};
 return loomOutputOpenView(view,...args);
};
const loomOutputDatasets=datasets;
datasets=function(){return loomOutputChrome()+loomOutputDatasets();};
const loomOutputModels=modelList;
modelList=function(){return loomOutputChrome()+loomOutputModels().replace(/>Models</g,'>Policies<');};
experiments=function(){
 const discarded=state.resultDispositionFilter==='discarded';
 const all=[...catKind('experiment',true),...catKind('run',true)];
 const rows=all.filter(e=>catIsDiscarded(e)===discarded);
 const filters='<section class="pa-model-filters" aria-label="Experiment filters"><div class="pa-model-filter-top"><div class="pa-model-role-group" role="group" aria-label="Record status">'
  +[['current','Current',false],['discarded','Discarded',true]].map(([value,label,isDiscarded])=>'<button class="pa-pill '+(discarded===isDiscarded?'active':'')+'" data-result-disposition-filter="'+value+'" aria-pressed="'+(discarded===isDiscarded)+'">'+label+'<span>'+all.filter(e=>catIsDiscarded(e)===isDiscarded).length+'</span></button>').join('')+'</div></div></section>';
 return loomOutputChrome()+loomTaskToolbar()+heading('CATALOG','Experiments','')+catStatus()+filters
  +panel(rows.length+' / '+all.length+'개 실험','<div>'+ (rows.map(e=>resultCard(e)+(discarded?resultDispositionPanel(e):'')).join('')||'<div class="pa-empty">표시할 실험이 없습니다.</div>')+'</div>')+loomCatalogPager('experiment');
};
const loomOutputDetail=catalogDetail;
catalogDetail=function(id){return (loomIsOutputPage(state.page)?loomOutputChrome():'')+loomOutputDetail(id);};
function loomCompareSetups(entity){
 return Object.values(entity?.science?.deliverables||{}).flatMap(claim=>claim.sessions||[]);
}
const loomOutputCompareCompatible=comparisonCompatible;
comparisonCompatible=function(a,b){
 if(!loomOutputCompareCompatible(a,b))return false;
 const keys=e=>[...new Set(loomCompareSetups(e).map(s=>JSON.stringify([s.domain,s.configuration.id,s.configuration.revision,s.configuration.sha256])))].sort();
 return JSON.stringify(keys(a))===JSON.stringify(keys(b));
};
const loomOutputComparison=comparison;
comparison=function(){
 // Comparison can be entered directly, before either list has been loaded.
 const pages=['model','experiment'].map(kind=>loomCatalogPager(kind,kind==='model'?'Load more policies':'Load more experiments')).join('');
 const pending=(state.compareRuns||[]).filter(id=>!catEntity(id)),selected=[...(state.compareRuns||[])],base=state.compareBase;
 for(const id of pending)if(!loomCatalogOpening.has(id)){
  loomCatalogOpening.add(id);
  queueMicrotask(()=>loomCatalogOpen(id,false).catch(error=>{
   if(error.code==='unknown_item'){for(const pane of ws.panes)for(const tab of pane.tabs)if(tab.view.compareRuns)tab.view.compareRuns=tab.view.compareRuns.filter(value=>value!==id);}
  }).finally(()=>loomCatalogOpening.delete(id)));
 }
 const body=loomOutputComparison();
 // A bounded bootstrap is not evidence that a selected result was deleted.
 if(pending.length){state.compareRuns=selected;state.compareBase=base;}
 const setups=selected.flatMap(id=>{
  const entity=catEntity(id);
  return loomCompareSetups(entity).map(s=>[catLink(id),esc(s.id),esc(s.domain==='real'?'Real':'Simulation'),
   esc(s.configuration.id+' · r'+s.configuration.revision),esc(s.member_count),esc(JSON.stringify(s.readback))]);
 });
 return body+(setups.length?panel('Experimental setups',catTable(['Experiment','Session','Domain','Setup','Members','Readback'],setups)):'')+pages;
};
root.addEventListener('click',event=>{
 const button=event.target.closest('[data-output-page]');if(!button)return;
 event.preventDefault();const pane=button.closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;
 loomOutputNavigate(button.dataset.outputPage);
});

// Browse summaries use declared semantics, never names or a guessed best score.
function loomOutputDevelopment(e){
 const p=e.properties||{};
 return e.presentation?.role==='support'||e.presentation?.role==='test'||p.purpose==='engineering'||p.evaluation_class==='engineering'||e.dataset_overview?.role==='diagnostic_observations'
  ||(e.kind==='model'&&modelFilterMeta(e).role==='Observer');
}
function loomOutputPurpose(){return state.outputPurpose||((catalogData.presentation_view||'all')==='research'?'research':'all');}
const loomOutputClasses=()=>Array.isArray(state.outputClasses)?state.outputClasses:['regular'];
const loomOutputKinds=catKind;
catKind=function(kind,discarded=false){
 if(!['model','dataset','experiment','run'].includes(kind))return loomOutputKinds(kind,discarded);
 if(catalogData.capabilities?.job_units){
  const classes=loomOutputClasses(),page=loomCatalogPages.get(loomCatalogPageKey(kind));
  const ids=page?.committed?new Set(page.ids):null;
  return loomTaskItems((catalogData.all_entities||catalogData.entities).filter(e=>e.kind===kind&&(discarded||!catIsDiscarded(e))
   &&(!ids||ids.has(e.id))&&classes.includes(e.catalog_class||(loomOutputDevelopment(e)?'reference':'regular'))),kind);
 }
 const view=catalogData.presentation_view||'all';
 const rows=loomTaskItems((catalogData.all_entities||catalogData.entities).filter(e=>e.kind===kind&&(discarded||!catIsDiscarded(e))&&(view==='all'||(view==='operations'?e.presentation?.scope==='operations':view==='builtin'?e.owner_kind==='built-in':e.presentation?.scope!=='operations'))),kind);
 const purpose=loomOutputPurpose();
 return rows.filter(e=>purpose==='all'||(purpose==='development')===loomOutputDevelopment(e))
  .sort((a,b)=>Number(loomOutputDevelopment(a))-Number(loomOutputDevelopment(b))||loomOutputRank(a)-loomOutputRank(b));
};
const loomOutputToolbar=loomTaskToolbar;
loomTaskToolbar=function(){
 if(catalogData.capabilities?.job_units)return loomOutputToolbar()+'<div class="pa-filter" aria-label="Catalog class">'
  +[['regular','Regular'],['reference','Reference']].map(([value,label])=>'<label class="pa-pill"><input type="checkbox" data-output-class="'+value+'"'+(loomOutputClasses().includes(value)?' checked':'')+'> '+label+'</label>').join('')+'</div>';
 return loomOutputToolbar()+'<div class="pa-filter" aria-label="Output purpose">'
 +[['all','All'],['research','Research'],['development','Development']].map(([value,label])=>'<button class="pa-pill '+(loomOutputPurpose()===value?'active':'')+'" aria-pressed="'+(loomOutputPurpose()===value)+'" data-output-purpose="'+value+'">'+label+'</button>').join('')+'</div>';};
root.addEventListener('click',event=>{const b=event.target.closest('[data-output-purpose]');if(!b)return;const pane=b.closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;activeTab().view.outputPurpose=b.dataset.outputPurpose;render();persist();});

root.addEventListener('change',event=>{
 const input=event.target.closest('[data-output-class]');if(!input)return;
 const pane=input.closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;
 const view=activeTab().view,classes=new Set(view.outputClasses||['regular']);
 if(input.checked)classes.add(input.dataset.outputClass);else classes.delete(input.dataset.outputClass);
 view.outputClasses=['regular','reference'].filter(value=>classes.has(value));render();persist();
});

function loomRepresentativeOutcome(e){
 const rows=catResultMetrics(e),primary=rows.filter(r=>r.role==='primary_outcome');
 if(primary.length===1)return primary;
 // Multiple declared primaries describe different populations, not one score.
 if(primary.length>1)return [];
 const highlights=e.outcome_highlights||[];
 if(highlights.length===1){const matches=rows.filter(r=>r.id===highlights[0].id&&r.definition===highlights[0].definition);if(matches.length===1)return matches;}
 return rows.length===1?rows:[];
}
resultOutcomeRows=loomRepresentativeOutcome;

function loomOutputRank(e){
 if(e.kind==='dataset')return ({training_observations:0,diagnostic_observations:2,evaluation_conditions:3}[e.dataset_overview?.role]??1);
 if(e.kind==='model')return ({VLA:0,Policy:1,Observer:3}[modelFilterMeta(e).role]??2);
 return e.properties?.importance==='major'?0:1;
}

root.addEventListener('click',event=>{if(event.target.closest('[data-metadata-templates]'))loomRegistryBrowser('metadata_template');});
