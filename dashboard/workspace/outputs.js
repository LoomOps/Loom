// One output workspace; typed records and legacy deep links keep their identity.
const loomOutputPages={datasets:'Datasets',experiments:'Experiments',models:'Policies'};
function loomIsOutputPage(page){return Object.hasOwn(loomOutputPages,page)||page==='scenarios';}
function loomIsResearchPage(page){return loomIsOutputPage(page)||page==='comparison';}
function loomOutputTabs(){
 const selected=state.page;
 return '<nav class="pa-output-tabs" aria-label="Outputs">'+Object.entries(loomOutputPages).map(([page,label])=>'<button data-output-page="'+page+'" aria-current="'+(selected===page?'page':'false')+'">'+label+'</button>').join('')+'</nav>';
}
function loomOutputBreadcrumb(view){
 if(!loomIsOutputPage(view.page))return '작업실 / '+viewTitle(view);
 const label=view.entity||view.model?viewTitle(view):null;
 const section=loomOutputPages[view.page]||'Task';
 return '작업실 / Outputs / '+section+(label?' / '+label:'');
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
const loomOutputExperiments=experiments;
experiments=function(){return loomOutputChrome()+loomOutputExperiments();};
const loomOutputDetail=catalogDetail;
catalogDetail=function(id){return (loomIsOutputPage(state.page)?loomOutputChrome():'')+loomOutputDetail(id);};
const loomOutputComparison=comparison;
comparison=function(){
 // Comparison can be entered directly, before either list has been loaded.
 const pages=['model','experiment'].map(kind=>loomCatalogPager(kind).replace('>More</button>','>More '+(kind==='model'?'models':'experiments')+'</button>')).join('');
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
 return body+pages;
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
function loomOutputPurpose(){return state.outputPurpose||((catalogData.presentation_view||'research')==='research'?'research':'all');}
const loomOutputKinds=catKind;
catKind=function(kind,discarded=false){
 if(!['model','dataset','experiment','run'].includes(kind))return loomOutputKinds(kind,discarded);
 const view=catalogData.presentation_view||'research';
 const rows=loomTaskItems((catalogData.all_entities||catalogData.entities).filter(e=>e.kind===kind&&(discarded||!catIsDiscarded(e))&&(view==='all'||(view==='operations'?e.presentation?.scope==='operations':view==='builtin'?e.owner_kind==='built-in':e.presentation?.scope!=='operations'))),kind);
 const purpose=loomOutputPurpose();
 return rows.filter(e=>purpose==='all'||(purpose==='development')===loomOutputDevelopment(e))
  .sort((a,b)=>Number(loomOutputDevelopment(a))-Number(loomOutputDevelopment(b))||loomOutputRank(a)-loomOutputRank(b));
};
const loomOutputToolbar=loomTaskToolbar;
loomTaskToolbar=function(){return loomOutputToolbar()+'<div class="pa-filter" aria-label="Output purpose">'
 +[['all','All'],['research','Research'],['development','Development']].map(([value,label])=>'<button class="pa-pill '+(loomOutputPurpose()===value?'active':'')+'" aria-pressed="'+(loomOutputPurpose()===value)+'" data-output-purpose="'+value+'">'+label+'</button>').join('')+'</div>';};
root.addEventListener('click',event=>{const b=event.target.closest('[data-output-purpose]');if(!b)return;const pane=b.closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;activeTab().view.outputPurpose=b.dataset.outputPurpose;render();persist();});

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
