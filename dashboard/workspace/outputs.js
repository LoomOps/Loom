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

function loomOutputMetadata(e){
 const p={...(e.properties||{}),...(e.metadata||{})},protocol=e.protocol||p.protocol||{},d=e.dataset_overview||{};
 const value=(...keys)=>keys.map(k=>p[k]??protocol[k]).find(v=>v!==undefined&&v!==null&&v!=='');
 const rows={};const add=(label,v)=>{if(v!==undefined&&v!==null&&v!==''&&(!Array.isArray(v)||v.length))rows[label]=v;};
 const inputs=e.inputs||p.inputs||d.inputs||[];
 add('Environment',value('environment','evaluation_environment','environment_type','simulator','platform'));
 if(p.real_robot===true)add('Environment','Real');else if(p.simulation===true)add('Environment','Sim');
 add('Protocol',typeof e.protocol==='string'?e.protocol:protocol.label||protocol.id||value('protocol_ref','goal_protocol_ref'));
 if(['run','experiment'].includes(e.kind)){
  add('Evaluation',value('evaluation_scope','evaluation_type','evaluation_mode','performed_scope','scope'));
  if(p.offline_only===true)add('Evaluation','Offline');
  add('Success criteria',value('success_criteria','success_condition','success_definition','success_predicate'));
  for(const [key,label,unit] of [['distance_tolerance_mm','Position tolerance','mm'],['angle_tolerance_deg','Angle tolerance','°'],['continuous_s','Continuous hold','s'],['active_hold_s','Hold window','s']])if(protocol[key]!=null)add(label,protocol[key]+' '+unit);
  add('Evaluator',value('evaluator','evaluation_tool','observer_identity','common_observer','observer'));
  add('Split',value('split'));add('Cohort',value('cohort','population'));
  add('Planned cases',value('planned_cases','planned'));add('Valid cases',value('valid_cases','valid'));add('Infrastructure failures',value('infra'));add('Pending',value('pending'));
 }else if(e.kind==='dataset'){
  add('Physics',value('physics','physics_config','physics_parameters','physics_settings'));
  add('Physics rate',value('physics_hz'));add('Control rate',value('control_hz','frequency_hz'));
  add('Annotation',value('annotation','annotations','annotation_method','labeling','teacher_source_revision'));
  add('Source demos',value('source_human_parent_count','source_parent_count'));add('Generation',value('generation_method','method'));
 }else if(e.kind==='model'){
  const m=modelFilterMeta(e);if(m.role!=='미분류')add('Model type',m.role);if(m.family!=='미기록')add('Architecture',m.family);
  add('Training',value('method','training_method'));add('Selected step',value('selected_step'));add('Updates',value('optimizer_updates'));
 }
 const instruction=Array.isArray(inputs)?inputs.find(x=>x.kind==='instruction'):null;
 if(instruction)add('Instruction',instruction.status==='absent'?'None':instruction.status==='present'?(instruction.mode||instruction.value||'Present'):null);
 return rows;
}
function loomOutputFacts(e){return Object.entries(loomOutputMetadata(e)).filter(([,value])=>['string','number','boolean'].includes(typeof value)).map(([label,value])=>({role:'research_metadata',label,value}));}
const loomOutputFactsBefore=catFacts;
catFacts=function(e){const facts=loomOutputFactsBefore(e);const seen=new Set(facts.map(f=>f.label));return [...facts,...loomOutputFacts(e).filter(f=>!seen.has(f.label)).slice(0,4)];};
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
const loomFocusedResultCard=resultCard;
resultCard=function(e){
 const metrics=catResultMetrics(e),primary=loomRepresentativeOutcome(e),facts=loomOutputFacts(e).slice(0,4);
 let html=loomFocusedResultCard(e);
 const metadata=facts.length?'<dl class="pa-output-context">'+facts.map(f=>'<div><dt>'+esc(f.label)+'</dt><dd>'+esc(f.value)+'</dd></div>').join('')+'</dl>':'';
 const detail=metrics.length>primary.length?'<details class="pa-output-metrics"><summary>Metrics · '+metrics.length+'</summary>'+catTable(['Metric','Value','Population'],metrics.map(m=>[esc(m.label),resultOutcomeValue(m),esc(m.population||m.denominator_label||m.aggregation||'—')]))+'</details>':'';
 return html.replace('</article>',metadata+detail+'</article>');
};
const loomFocusedOutputDetail=catalogDetail;
catalogDetail=function(id){
 const e=catEntity(id);let html=loomFocusedOutputDetail(id);
 if(!e||!['model','dataset','experiment','run','job'].includes(e.kind))return html;
 const t=document.createElement('template');t.innerHTML=html;const title=t.content.querySelector('.pa-heading');if(!title)return html;
 if(e.kind!=='job'){
  const metadata=loomOutputMetadata(e),box=document.createElement('div');
  if(Object.keys(metadata).length){box.innerHTML=panel(e.kind==='dataset'?'Collection':e.kind==='model'?'Model':'Evaluation',catProperties(metadata));title.after(...box.children);}
  // Scientific properties lead; full producer metadata remains available below.
  for(const panelNode of t.content.querySelectorAll('.pa-panel')){
   if(panelNode.querySelector(':scope > .pa-panel-head > h2')?.textContent!=='속성')continue;
   const details=document.createElement('details');details.className='pa-output-source';details.innerHTML='<summary>Source metadata</summary>'+panelNode.innerHTML;panelNode.replaceWith(details);
  }
 }
 if(['run','experiment'].includes(e.kind)){
  for(const grid of t.content.querySelectorAll('.pa-metric-grid,.pa-outcome-highlights'))grid.remove();
  const primary=loomRepresentativeOutcome(e);
  if(primary.length)title.insertAdjacentHTML('afterend',panel('Outcome',catTable(['Metric','Value','Population'],primary.map(m=>[esc(m.label),resultOutcomeValue(m),esc(m.population||m.denominator_label||'—')]))));
 }
 const record=document.createElement('details');record.className='pa-output-source';record.innerHTML='<summary>Record</summary>'+catProperties({Owner:e.owner_profile?.display_name||e.owner||'—',ID:e.id});t.content.append(record);
 const task=loomTaskProperty(e);if(task)title.insertAdjacentHTML('beforebegin',task);
 return t.innerHTML;
};

function loomOutputRank(e){
 if(e.kind==='dataset')return ({training_observations:0,diagnostic_observations:2,evaluation_conditions:3}[e.dataset_overview?.role]??1);
 if(e.kind==='model')return ({VLA:0,Policy:1,Observer:3}[modelFilterMeta(e).role]??2);
 return e.properties?.importance==='major'?0:1;
}
