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
function loomOutputDiscarded(){return state.resultDispositionFilter==='discarded';}
function loomOutputRows(records){return records.filter(e=>catIsDiscarded(e)===loomOutputDiscarded());}
function loomResetOutputFilters(view){
 delete view.outputDataset;delete view.experimentFamily;
 if(view.modelFilters?.family)view.modelFilters={...view.modelFilters,family:''};
}
function loomSetOutputDisposition(view,value){
 if((view.resultDispositionFilter||'current')!==value)loomResetOutputFilters(view);
 view.resultDispositionFilter=value;
}
function loomOutputStatusTabs(){
 const discarded=loomOutputDiscarded();
 return '<section class="pa-model-filters" aria-label="Record status"><div class="pa-model-filter-top"><div class="pa-model-role-group" role="group" aria-label="Record status">'
  +[['current','Current',false],['discarded','Discarded',true]].map(([value,label,isDiscarded])=>'<button class="pa-pill '+(discarded===isDiscarded?'active':'')+'" data-result-disposition-filter="'+value+'" aria-pressed="'+(discarded===isDiscarded)+'">'+label+'</button>').join('')+'</div></div></section>';
}
const loomOutputDatasets=datasets;
datasets=function(){const tasks=loomOutputTasks('dataset');return loomOutputList(loomOutputDatasets(),tasks);};
const policyFilterDefaults={family:'',query:''};
function policyText(...values){return values.find(v=>typeof v==='string'&&v.trim())?.trim()||'';}
function policyFamily(recorded){
 const known=[[/\bX[-_ ]?VLA\b/i,'X-VLA'],[/\bSmol[-_ ]?VLA\b/i,'SmolVLA'],
  [/\bOpen[-_ ]?VLA\b/i,'OpenVLA'],[/\bBC[-_ ]?RNN\b/i,'BC-RNN']];
 const matches=known.filter(([pattern])=>pattern.test(recorded));
 return matches.length===1?matches[0][1]:recorded||'미기록';
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
 return {role,family};
}
function policyFilters(view){return {...policyFilterDefaults,...view.modelFilters};}
function loomInputKey(input){
 if(input.dataset_mirror_key){
  const task=input.organization?.task_ref?.id, path=input.organization?.path?.map(node=>node.id);
  return JSON.stringify(['dataset-mirror',input.dataset_mirror_key,task,path]);
 }
 const {kind,identity:i}=input.ref;
 return JSON.stringify(kind==='entity'?[kind,i.id,i.revision]:[kind,i.source,i.kind,i.id,i.revision,i.snapshot]);
}
function loomCatalogInputs(e,field){return e.catalog_inputs?.[field]||[];}
function loomInputLabel(input){
 const original=input.organization?.path||[],path=loomDatasetDisplayPath(original,input.split);
 return path.length?path.map(node=>node.label).join(' / '):loomDatasetDisplayLabel(input.label,original,input.split);
}
function loomCatalogDatasetOptions(records){
 const options=new Map();
 for(const record of records)for(const input of loomCatalogInputs(record,'datasets')){
  if(!input.task_refs?.some(task=>task.id===state.researchTask?.id))continue;
  options.set(loomInputKey(input),input);
 }
 const labels=new Map();
 for(const input of options.values()){
  const label=loomInputLabel(input);labels.set(label,(labels.get(label)||0)+1);
 }
 const entries=[...options].map(([id,input])=>{
  const name=loomInputLabel(input),tag=labels.get(name)>1&&loomDatasetSplitLabel(input.split);
  return {id,label:tag?name+' · '+tag:name};
 });
 const counts=new Map();
 for(const {label} of entries)counts.set(label,(counts.get(label)||0)+1);
 for(const entry of entries){
  if(counts.get(entry.label)<2)continue;
  const identity=options.get(entry.id).ref.identity;
  const detail=identity.snapshot?identity.source+' · r'+identity.revision:'r'+identity.revision+' · '+identity.id.slice(0,8);
  entry.label+=' · '+detail;
 }
 entries.sort((a,b)=>a.label.localeCompare(b.label,undefined,{numeric:true}));
 return entries;
}
function loomMatchesCatalogInput(e,field,key){return !key||loomCatalogInputs(e,field).some(input=>loomInputKey(input)===key);}
function loomInputNames(e,field){return [...new Set(loomCatalogInputs(e,field).map(loomInputLabel))].join(' · ')||'Missing';}
function loomUniqueCatalogSnapshots(records){
 const shown=new Map();
 for(const record of records){
  const tasks=state.researchTask?[state.researchTask.id]:(record.task_refs||[]).map(task=>task.id).sort();
  const key=record.catalog_mirror_key?JSON.stringify([record.catalog_mirror_key,record.catalog_class,tasks]):record.id;
  const previous=shown.get(key);
  const inputs=e=>loomCatalogInputs(e,'datasets').length+loomCatalogInputs(e,'policies').length;
  if(!previous||inputs(record)>inputs(previous))shown.set(key,record);
 }
 return [...shown.values()];
}
function loomExperimentFamilies(e){
 return [...new Set(loomCatalogInputs(e,'policies').map(input=>{
  const record=catEntity(input.ref.identity.id);
  if(record?.loom_registry?.revision===input.ref.identity.revision&&record)return modelFilterMeta(record).family;
  return policyText(input.family)||policyFamily(policyText(input.architecture));
 }))];
}
function loomFamilyIndex(families,attribute,selected){
 const entries=[...new Set(families)].sort((a,b)=>a.localeCompare(b))
  .map(family=>({id:family,label:family==='미기록'?'Missing':family}));
 return loomCatalogIndex('Family',attribute,entries,selected);
}
function policyList(){
 const tasks=loomOutputTasks('model');
 const f=policyFilters(state),query=f.query.trim().toLowerCase();
 const records=loomUniqueCatalogSnapshots(loomOutputRows(catKind('model',true))).filter(e=>modelFilterMeta(e).role!=='Observer');
 const complete=loomOutputPageComplete(loomCatalogPages.get(loomCatalogPageKey('model')));
 const datasets=state.researchTask?loomCatalogDatasetOptions(records):[];
 const families=records.map(e=>modelFilterMeta(e).family);
 const dataset=loomOutputDatasetSelection(datasets,complete);
 if(complete&&f.family&&!families.includes(f.family)){
  f.family='';state.modelFilters=f;
 }
 const shown=records.filter(e=>(!f.family||modelFilterMeta(e).family===f.family)&&loomMatchesCatalogInput(e,'datasets',dataset)&&(!query||catSearchText(e).includes(query)));
 const filters='<section class="pa-model-filters" aria-label="Policy filters"><div class="pa-model-filter-fields pa-policy-filter-fields">'
  +'<label class="pa-model-filter-field pa-model-filter-search"><span>검색</span><input type="search" data-model-filter="query" aria-label="정책 검색" placeholder="이름·계열·키워드" value="'+esc(f.query)+'" autocomplete="off"></label></div>'
  +'<button type="button" class="pa-model-filter-reset" data-model-reset'+(f.query?'':' hidden')+'>검색 초기화</button></section>';
 const rows=loomCatalogTaskGroups(shown,loomCatalogTask,items=>items.map(e=>loomCompactOutput(e,false)).join(''));
 const familyIndex=loomFamilyIndex(families,'data-policy-family',f.family);
 const datasetIndex=state.researchTask?loomCatalogIndex('Datasets','data-catalog-dataset',datasets,dataset):'';
 return loomOutputList(heading('CATALOG','Policies','')+catStatus()+loomOutputStatusTabs()+filters+panel(`<span data-model-result-count>${shown.length} / ${records.length}개 정책</span>`,
  '<div data-model-filter-results>'+(rows||'<div class="pa-empty">'+(records.length?'조건에 맞는 정책이 없습니다.':'표시할 정책이 없습니다.')+'</div>')+'</div>')+loomCatalogPager('model','Load more policies'),tasks,datasetIndex+familyIndex);
}
modelList=policyList;
function updatePolicyFilter(control){
 const pane=ws.panes.find(p=>p.id===control.closest('[data-pane]')?.dataset.pane);if(!pane)return;
 ws.active=pane.id;const view=activeTab(pane).view;
 const f=policyFilters(view);
 if(control.hasAttribute('data-policy-family'))view.modelFilters={...f,family:control.dataset.policyFamily};
 else view.modelFilters={...f,query:control.hasAttribute('data-model-reset')?'':control.value};
 refreshCatalogPanes();persist();
}
root.addEventListener('click',event=>{
 const control=event.target.closest('[data-policy-family],[data-model-reset],[data-catalog-dataset],[data-experiment-family]');
 if(!control)return;
 if(control.matches('[data-policy-family],[data-model-reset]')){updatePolicyFilter(control);return;}
 const pane=control.closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;
 const view=activeTab().view;
 if(control.hasAttribute('data-catalog-dataset')){view.outputDataset=control.dataset.catalogDataset;}
 else view.experimentFamily=control.dataset.experimentFamily;
 render();persist();
 root.querySelector('.pa-document-pane.is-active ['+(control.hasAttribute('data-catalog-dataset')?'data-catalog-dataset':'data-experiment-family')+'][aria-pressed=true]')?.focus({preventScroll:true});
});
root.addEventListener('input',event=>{const control=event.target.closest('input[data-model-filter="query"]');if(control&&!event.isComposing)updatePolicyFilter(control);});
function loomCatalogIndex(label,attribute,entries,selected){
 const buttons=[{id:'',label:'All'},...entries].map(entry=>'<button type="button" '+attribute+'="'+esc(entry.id)+'" aria-pressed="'+(selected===entry.id)+'">'+esc(entry.label)+'</button>').join('');
 return '<nav class="pa-catalog-task-index" aria-label="'+label+'"><h2>'+label+'</h2>'+buttons+'</nav>';
}
function loomOutputPageComplete(page){
 return !catalogData.capabilities?.catalog_pages||loomOutputClasses().length===0||page?.loaded&&!page.partial&&!page.cursor;
}
function loomOutputDatasetSelection(datasets,complete){
 if(state.outputDataset&&complete&&!datasets.some(entry=>entry.id===state.outputDataset))delete state.outputDataset;
 return state.outputDataset||'';
}
function loomOutputTasks(kind){
 const query=loomCatalogQuery(kind,false),page=loomCatalogPages.get(JSON.stringify(query));
 const paged=catalogData.capabilities?.catalog_pages&&catalogData.capabilities?.job_units;
 const noClasses=query.catalog_classes?.length===0;
 if(paged&&!noClasses&&!page?.loaded&&!page?.busy&&!page?.error)queueMicrotask(()=>loomCatalogLoad(kind,false,query));
 const records=loomOutputKindRows(kind,true,page);
 if(kind==='experiment')records.push(...loomOutputKindRows('run',true,page));
 const visible=loomOutputRows(records).filter(e=>kind!=='model'||modelFilterMeta(e).role!=='Observer');
 const ids=new Set(visible.flatMap(e=>(e.task_refs||[]).map(task=>task.id)));
 const complete=!paged||loomOutputPageComplete(page);
 const loadedTasks=complete&&kind==='model'?new Set(loomOutputRows(records).flatMap(e=>(e.task_refs||[]).map(task=>task.id))):null;
 for(const id of page?.taskIndex||[])if(!loadedTasks?.has(id))ids.add(id);
 if(state.researchTask&&!ids.has(state.researchTask.id)){
  if(complete||page?.loaded&&page.taskIndex){
   delete state.researchTask;delete state.outputDataset;delete state.experimentFamily;
  }else ids.add(state.researchTask.id);
 }
 return loomTaskOptions().filter(task=>ids.has(task.id));
}
function loomOutputList(body,tasks,secondaryIndex=''){
 const taskIndex=loomCatalogIndex('Tasks','data-research-task',tasks,state.researchTask?.id||'');
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
function loomOutputCheckpoint(e){
 const sweep=e.reporting?.checkpoint_sweep;
 const headline=sweep&&loomCheckpointHeadline(sweep);
 if(headline?.tied)return 'Tied';
 const selected=loomResearchFactMap(e).get('selected_step');
 const step=sweep?headline?.step:selected?.status==='recorded'?selected.value:null;
 return Number.isSafeInteger(step)&&step>=0?'step '+loomReportNumber(step):'Missing';
}
function loomCatalogOutcome(outcome){
 if(!outcome)return 'Missing';
 if(outcome.numerator==null||!outcome.denominator){
  return outcome.kind==='ratio'||outcome.denominator!==undefined?'미측정':outcome.value??outcome.count??outcome.missing_reason??'미측정';
 }
 const percent=Math.round(1000*outcome.numerator/outcome.denominator)/10;
 return outcome.numerator+' / '+outcome.denominator+' ('+percent+'%)';
}
function loomCompactOutput(e,result){
 const dataset=loomInputNames(e,'datasets'),policy=loomInputNames(e,'policies');
 const checkpoint=loomOutputCheckpoint(e);
 const outcome=result?loomRepresentativeOutcome(e)[0]:null;
 const family=result?null:modelFilterMeta(e).family;
 const fields=result?{'Dataset':dataset,'Policy':policy,'Result':loomCatalogOutcome(outcome),'Checkpoint':checkpoint}:
  {'Dataset':dataset,'Model family':family==='미기록'?'Missing':family,'Checkpoint':checkpoint};
 const facts=Object.entries(fields).map(([label,value])=>({id:label.toLowerCase().replaceAll(' ','_'),label,value,status:'recorded',title:label==='Result'?outcome?.label:undefined}));
 if(!result)facts.splice(1,0,loomDatasetTagFact(loomCatalogInputs(e,'datasets').map(input=>input.split)));
 return '<article class="pa-catalog-item pa-scientific-card pa-output-compact" data-output-id="'+esc(e.id)+'"'+(result?'':' data-model-entry="'+esc(e.id)+'"')+'>'
  +'<div class="pa-row-main">'+catLink(e.id)+'</div>'
  +loomResearchFactsHTML(facts,true)+'</article>';
}
experiments=function(){
 const tasks=loomOutputTasks('experiment');
 const records=loomUniqueCatalogSnapshots(loomOutputRows([...catKind('experiment',true),...catKind('run',true)]));
 const complete=loomOutputPageComplete(loomCatalogPages.get(loomCatalogPageKey('experiment')));
 const datasets=state.researchTask?loomCatalogDatasetOptions(records):[];
 const dataset=loomOutputDatasetSelection(datasets,complete);
 const datasetRows=records.filter(e=>loomMatchesCatalogInput(e,'datasets',dataset));
 const families=datasetRows.flatMap(loomExperimentFamilies);
 if(complete&&state.experimentFamily&&!families.includes(state.experimentFamily))delete state.experimentFamily;
 const family=state.experimentFamily||'';
 const rows=datasetRows.filter(e=>!family||loomExperimentFamilies(e).includes(family));
 const index=(state.researchTask?loomCatalogIndex('Datasets','data-catalog-dataset',datasets,dataset):'')+loomFamilyIndex(families,'data-experiment-family',family);
 return loomOutputList(heading('CATALOG','Experiments','')+catStatus()+loomOutputStatusTabs()
  +panel(rows.length+' / '+records.length+'개 실험','<div>'+ (loomCatalogTaskGroups(rows,loomCatalogTask,items=>items.map(e=>loomCompactOutput(e,true)).join(''))||'<div class="pa-empty">표시할 실험이 없습니다.</div>')+'</div>')+loomCatalogPager('experiment'),tasks,index);
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
 const view=activeTab().view;
 if((view.researchTask?.id||'')!==id){view.outputDataset='';view.experimentFamily='';}
 view.researchTask=loomTaskOptions().find(task=>task.id===id);
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
 return loomTaskItems(loomOutputKindRows(kind,discarded,loomCatalogPages.get(loomCatalogPageKey(kind))),kind);
};
function loomOutputKindRows(kind,discarded,page){
 const entities=(catalogData.all_entities||catalogData.entities).filter(e=>e.kind===kind);
 if(catalogData.capabilities?.job_units){
  const classes=loomOutputClasses();
  const ids=page?.committed?new Set(page.ids):null;
  return entities.filter(e=>!e.catalog_exact&&(discarded||!catIsDiscarded(e))
   &&(!ids||ids.has(e.id))&&classes.includes(e.catalog_class||(loomOutputDevelopment(e)?'reference':'regular')));
 }
 const view=catalogData.presentation_view||'all';
 const rows=entities.filter(e=>(discarded||!catIsDiscarded(e))&&(view==='all'||(view==='operations'?e.presentation?.scope==='operations':view==='builtin'?e.owner_kind==='built-in':e.presentation?.scope!=='operations')));
 const purpose=loomOutputPurpose();
 return rows.filter(e=>purpose==='all'||(purpose==='development')===loomOutputDevelopment(e))
  .sort((a,b)=>Number(loomOutputDevelopment(a))-Number(loomOutputDevelopment(b))||loomOutputRank(a)-loomOutputRank(b));
}
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
 view.outputClasses=['regular','reference'].filter(value=>classes.has(value));
 loomResetOutputFilters(view);render();persist();
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
