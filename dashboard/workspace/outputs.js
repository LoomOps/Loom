// One output workspace; typed records and legacy deep links keep their identity.
const loomOutputPages={datasets:'Datasets',experiments:'Experiments',models:'Models'};
function loomIsOutputPage(page){return Object.hasOwn(loomOutputPages,page)||page==='comparison'||page==='scenarios';}
function loomOutputTabs(){
 const selected=state.page==='comparison'?'experiments':state.page;
 return '<nav class="pa-output-tabs" aria-label="Outputs">'+Object.entries(loomOutputPages).map(([page,label])=>'<button data-output-page="'+page+'" aria-current="'+(selected===page?'page':'false')+'">'+label+'</button>').join('')+'</nav>';
}
function loomOutputBreadcrumb(view){
 if(!loomIsOutputPage(view.page))return '작업실 / '+viewTitle(view);
 const label=view.entity||view.model?viewTitle(view):null;
 const section=view.page==='comparison'?'Experiments / Comparison':loomOutputPages[view.page]||'Task';
 return '작업실 / Outputs / '+section+(label?' / '+label:'');
}
function loomOutputChrome(){
 return '<div class="pa-output-header">'+loomOutputTabs()+(['experiments','comparison'].includes(state.page)&&!state.entity
  ?'<div class="pa-output-actions">'+(state.page==='comparison'
   ?'<button class="pa-button" data-output-page="experiments">Results</button>'
   :'<button class="pa-button" data-output-page="comparison">'+icon('git-compare-arrows')+'Comparison</button>')+'</div>':'')+'</div>';
}
function loomOutputNavigate(page){
 if(!loomIsOutputPage(page))return;
 const pane=activePane(),tab=activeTab();
 const view=viewCopy({...tab.view,page,entity:null,model:null,query:'',routeOwner:null,routeAttempt:null,routeArtifact:null});
 if(tab.view.entity||tab.view.model){openView(view);return;}
 const key=tabKey(view),existing=pane.tabs.find(t=>t!==tab&&t.id===key);
 // Switching output sections reuses this document rather than spawning tabs.
 if(existing){existing.view=view;pane.active=existing.id;}
 else{tab.id=key;tab.view=view;pane.active=key;}
 menu=null;render();persist();
}
const loomOutputOpenView=openView;
openView=function(view,...args){
 const current=activeTab()?.view;
 if(current&&loomIsOutputPage(current.page)&&loomIsOutputPage(view.page)&&view.researchTask===undefined&&current.researchTask)view={...view,researchTask:current.researchTask};
 return loomOutputOpenView(view,...args);
};
const loomOutputDatasets=datasets;
datasets=function(){return loomOutputChrome()+loomOutputDatasets();};
const loomOutputModels=modelList;
modelList=function(){return loomOutputChrome()+loomOutputModels();};
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
 return loomOutputChrome()+loomTaskToolbar()+body+pages;
};
root.addEventListener('click',event=>{
 const button=event.target.closest('[data-output-page]');if(!button)return;
 event.preventDefault();const pane=button.closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;
 loomOutputNavigate(button.dataset.outputPage);
});
