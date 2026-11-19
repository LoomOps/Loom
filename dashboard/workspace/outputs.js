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
datasets=function(){return loomOutputList(loomOutputDatasets());};
const policyFilterDefaults={family:'',dataset:'',query:''};
function policyText(...values){return values.find(v=>typeof v==='string'&&v.trim())?.trim()||'';}
function policyFamily(recorded){
 const known=[[/^X[-_ ]?VLA\b/i,'X-VLA'],[/^Smol[-_ ]?VLA\b/i,'SmolVLA'],
  [/^Open[-_ ]?VLA\b/i,'OpenVLA'],[/^BC[-_ ]?RNN\b/i,'BC-RNN']];
 return known.find(([pattern])=>pattern.test(recorded))?.[1]||recorded||'미기록';
}
function modelFilterMeta(e){
 const p=e.properties||{},facts=loomResearchFactMap(e),roleFact=facts.get('model_role'),architecture=facts.get('architecture');
 const family=policyText(p.model_family,p.family)||policyFamily(policyText(architecture?.status==='recorded'?loomResearchValue(architecture,{compact:true}):null,
  p.architecture,p.policy_type,e.highlights?.find(x=>x.role==='architecture')?.value));
 const declared=roleFact?.status==='recorded'?loomResearchValue(roleFact):p.model_role??e.model_role??e.metadata?.research?.model?.role??p.role;
 const roles=Array.isArray(declared)?declared.join(' '):policyText(declared);
 let role=roles||'미분류';
 if(/\b(observer|perception|pose[ _-]estimator)\b/i.test(roles))role='Observer';
 else if(/\bvla\b/i.test(roles)||/^(?:X[-_ ]?VLA|SmolVLA|OpenVLA|π[0-9.]+|pi[0-9.]+)(?:\b|$)/i.test(family))role='VLA';
 else if(/\b(policy|action_policy)\b/i.test(roles))role='Policy';
 const datasets=catRelations(e).filter(r=>r.kind==='trained_on'||catEntity(r.id)?.kind==='dataset')
  .map(r=>({id:r.id,label:catEntity(r.id)?.label||r.id}));
 if(!datasets.length)datasets.push({id:policyText(p.training_dataset,p.dataset_name)||'__unrecorded__',label:policyText(p.dataset_name,p.training_dataset)||'미기록'});
 return {role,family,datasets};
}
function policyFilters(view){
 const {family='',dataset='',query=''}=view.modelFilters||{};
 return {family,dataset,query};
}
function policyFilterSelect(key,label,entries,value){
 const options=new Map(entries.map(x=>typeof x==='string'?[x,x]:[x.id,x.label]));
 if(value&&!options.has(value))options.set(value,value==='__unrecorded__'?'미기록':value);
 return '<label class="pa-model-filter-field"><span>'+esc(label)+'</span><select data-model-filter="'+key+'" aria-label="'+esc(label)+' 필터"><option value="">전체</option>'+
  [...options].sort((a,b)=>String(a[1]).localeCompare(String(b[1]),'ko')).map(([id,name])=>'<option value="'+esc(id)+'"'+(id===value?' selected':'')+'>'+esc(name)+'</option>').join('')+'</select></label>';
}
function policyList(){
 const f=policyFilters(state),query=f.query.trim().toLowerCase();
 const records=catKind('model').map(e=>({e,...modelFilterMeta(e)})).filter(x=>x.role!=='Observer');
 const shown=records.filter(x=>(!f.family||x.family===f.family)&&(!f.dataset||x.datasets.some(d=>d.id===f.dataset))&&(!query||catSearchText(x.e).includes(query))).map(x=>x.e);
 const filters='<section class="pa-model-filters" aria-label="Policy filters"><div class="pa-model-filter-fields pa-policy-filter-fields">'
  +policyFilterSelect('dataset','학습 데이터셋',records.flatMap(x=>x.datasets),f.dataset)
  +'<label class="pa-model-filter-field pa-model-filter-search"><span>검색</span><input type="search" data-model-filter="query" aria-label="정책 검색" placeholder="이름·계열·키워드" value="'+esc(f.query)+'" autocomplete="off"></label></div>'
  +'<button type="button" class="pa-model-filter-reset" data-model-reset'+(f.dataset||f.query?'':' hidden')+'>필터 초기화</button></section>';
 const rows=loomCatalogTaskGroups(shown,loomCatalogTask,items=>loomGroupedOutputs(items,e=>catListRow(e).replace('<article ',`<article data-model-entry="${esc(e.id)}" `)));
 const families=[...new Set(records.map(r=>r.family).concat(f.family).filter(Boolean))].sort((a,b)=>a.localeCompare(b));
 const familyIndex=loomCatalogIndex('Model family','data-policy-family',families.map(family=>({id:family,label:family})),f.family);
 return loomOutputList(heading('CATALOG','Policies','')+catStatus()+filters+panel(`<span data-model-result-count>${shown.length} / ${records.length}개 정책</span>`,
  '<div data-model-filter-results>'+(rows||'<div class="pa-empty">'+(records.length?'조건에 맞는 정책이 없습니다.':'표시할 정책이 없습니다.')+'</div>')+'</div>')+loomCatalogPager('model','Load more policies'),familyIndex);
}
modelList=policyList;
function updatePolicyFilter(control){
 const pane=ws.panes.find(p=>p.id===control.closest('[data-pane]')?.dataset.pane);if(!pane)return;
 ws.active=pane.id;const view=activeTab(pane).view;
 const f=policyFilters(view);
 if(control.hasAttribute('data-model-reset'))view.modelFilters={...policyFilterDefaults,family:f.family};
 else if(control.hasAttribute('data-policy-family'))view.modelFilters={...f,family:control.dataset.policyFamily};
 else view.modelFilters={...f,[control.dataset.modelFilter]:control.value};
 refreshCatalogPanes();persist();
}
root.addEventListener('click',event=>{const control=event.target.closest('[data-model-reset],[data-policy-family]');if(control)updatePolicyFilter(control);});
root.addEventListener('change',event=>{const control=event.target.closest('select[data-model-filter]');if(control)updatePolicyFilter(control);});
root.addEventListener('input',event=>{const control=event.target.closest('input[data-model-filter="query"]');if(control&&!event.isComposing)updatePolicyFilter(control);});
function loomCatalogIndex(label,attribute,entries,selected){
 const buttons=[{id:'',label:'All'},...entries].map(entry=>'<button type="button" '+attribute+'="'+esc(entry.id)+'" aria-pressed="'+(selected===entry.id)+'">'+esc(entry.label)+'</button>').join('');
 return '<nav class="pa-catalog-task-index" aria-label="'+label+'"><h2>'+label+'</h2>'+buttons+'</nav>';
}
function loomOutputList(body,secondaryIndex=''){
 const taskIndex=loomCatalogIndex('Tasks','data-research-task',loomTaskOptions(),state.researchTask?.id||'');
 return loomOutputChrome()+'<div class="pa-catalog-task-layout"><div class="pa-catalog-index-stack">'+taskIndex+secondaryIndex+'</div><div class="pa-catalog-task-content">'+loomTaskToolbar()+body+'</div></div>';
}
function loomCatalogTask(e){
 const refs=e.task_refs||[],organization=e.dataset_organization?.task_ref;
 return refs.find(r=>r.id===state.researchTask?.id)||refs.find(r=>r.id===organization?.id&&r.revision===organization?.revision)||refs.find(r=>r.id===e.scenario_ref?.id)||
  (refs.length&&refs.every(r=>r.id===refs[0].id)?refs[0]:null);
}
function loomCatalogTaskGroups(items,taskFor,renderItems){
 const groups=new Map();
 for(const item of items){
  const task=taskFor(item),id=task?.id||'';
  if(!groups.has(id))groups.set(id,{task,items:[]});
  groups.get(id).items.push(item);
 }
 if(groups.size===1&&groups.has(''))return renderItems(groups.get('').items);
 return [...groups.values()].map(group=>'<section class="pa-catalog-task-card" data-catalog-task-card="'+esc(group.task?.id||'')+'"><h3>'+(group.task?loomTaskLabels({task_refs:[group.task]}):'Other')+'</h3><div class="pa-catalog-task-records">'+renderItems(group.items)+'</div></section>').join('');
}
function loomGroupedOutputs(items, renderItem) {
 const groups=new Map(),ordered=[];
 for(const item of items){
  const group=item.evaluation_group;
  if(!group?.id){ordered.push([item]);continue;}
  if(!groups.has(group.id)){groups.set(group.id,[]);ordered.push(groups.get(group.id));}
  groups.get(group.id).push(item);
 }
 return ordered.map(rows=>rows.length===1?renderItem(rows[0]):
  '<section class="pa-evaluation-group" data-evaluation-group="'+esc(rows[0].evaluation_group.id)+'"><h3>'+
  esc(rows[0].evaluation_group.label)+'</h3>'+rows.map(renderItem).join('')+'</section>').join('');
}
experiments=function(){
 const discarded=state.resultDispositionFilter==='discarded';
 const all=[...catKind('experiment',true),...catKind('run',true)];
 const rows=all.filter(e=>catIsDiscarded(e)===discarded);
 const filters='<section class="pa-model-filters" aria-label="Experiment filters"><div class="pa-model-filter-top"><div class="pa-model-role-group" role="group" aria-label="Record status">'
  +[['current','Current',false],['discarded','Discarded',true]].map(([value,label,isDiscarded])=>'<button class="pa-pill '+(discarded===isDiscarded?'active':'')+'" data-result-disposition-filter="'+value+'" aria-pressed="'+(discarded===isDiscarded)+'">'+label+'<span>'+all.filter(e=>catIsDiscarded(e)===isDiscarded).length+'</span></button>').join('')+'</div></div></section>';
 return loomOutputList(heading('CATALOG','Experiments','')+catStatus()+filters
  +panel(rows.length+' / '+all.length+'개 실험','<div>'+ (loomCatalogTaskGroups(rows,loomCatalogTask,items=>loomGroupedOutputs(items,e=>resultCard(e)+(discarded?resultDispositionPanel(e):'')))||'<div class="pa-empty">표시할 실험이 없습니다.</div>')+'</div>')+loomCatalogPager('experiment'));
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
function selectCatalogTask(control,id){
 const pane=control.closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;
 activeTab().view.researchTask=loomTaskOptions().find(task=>task.id===id);
 render();persist();
}
function loomComparisonTaskFilter(){
 const options=loomTaskOptions().map(t=>'<option value="'+esc(t.id)+'"'+(state.researchTask?.id===t.id?' selected':'')+'>'+esc(t.label)+'</option>').join('');
 return '<div class="pa-task-filter"><label>Task <select data-comparison-task><option value="">All tasks</option>'+options+'</select></label></div>';
}
root.addEventListener('change',event=>{
 const control=event.target.closest('[data-comparison-task]');if(control)selectCatalogTask(control,control.value);
});
root.addEventListener('click',event=>{
 const taskButton=event.target.closest('button[data-research-task]');
 if(taskButton){
  selectCatalogTask(taskButton,taskButton.dataset.researchTask);
  root.querySelector('.pa-document-pane.is-active [data-research-task][aria-pressed=true]')?.focus({preventScroll:true});
  return;
 }
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
  return loomTaskItems((catalogData.all_entities||catalogData.entities).filter(e=>e.kind===kind&&!e.catalog_exact&&(discarded||!catIsDiscarded(e))
   &&(!ids||ids.has(e.id))&&classes.includes(e.catalog_class||(loomOutputDevelopment(e)?'reference':'regular'))),kind);
 }
 const view=catalogData.presentation_view||'all';
 const rows=loomTaskItems((catalogData.all_entities||catalogData.entities).filter(e=>e.kind===kind&&(discarded||!catIsDiscarded(e))&&(view==='all'||(view==='operations'?e.presentation?.scope==='operations':view==='builtin'?e.owner_kind==='built-in':e.presentation?.scope!=='operations'))),kind);
 const purpose=loomOutputPurpose();
 return rows.filter(e=>purpose==='all'||(purpose==='development')===loomOutputDevelopment(e))
  .sort((a,b)=>Number(loomOutputDevelopment(a))-Number(loomOutputDevelopment(b))||loomOutputRank(a)-loomOutputRank(b));
};
function loomTaskToolbar(){
 if(catalogData.capabilities?.job_units)return '<div class="pa-filter" aria-label="Catalog class">'
  +[['regular','Regular'],['reference','Reference']].map(([value,label])=>'<label class="pa-pill"><input type="checkbox" data-output-class="'+value+'"'+(loomOutputClasses().includes(value)?' checked':'')+'> '+label+'</label>').join('')+'</div>';
 return '<div class="pa-filter" aria-label="Output purpose">'
 +[['all','All'],['research','Research'],['development','Development']].map(([value,label])=>'<button class="pa-pill '+(loomOutputPurpose()===value?'active':'')+'" aria-pressed="'+(loomOutputPurpose()===value)+'" data-output-purpose="'+value+'">'+label+'</button>').join('')+'</div>';}
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
