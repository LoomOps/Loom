// One scientific summary for cards and details. Registered-source extraction and
// provenance live in Rust; this module formats their values, never host files.
const loomResearchSections={context:'Context',collection:'Collection',observations:'Observations',physics:'Physics & control',annotations:'Annotations',instructions:'Instructions',model:'Model',training:'Training',validation:'Validation',evaluation:'Evaluation',population:'Population',scoring:'Success criteria'};
const loomResearchMissing=new Set(['미기록','미측정','Not recorded','Unknown']);
function loomResearchSummary(e){
 if(e.research_summary?.schema==='loom-research-summary/v1')return e.research_summary;
 // Compatibility with older servers/fixtures, restricted to recorded fields.
 const p={...(e.properties||{}),...(e.metadata||{})},d=e.dataset_overview||{},facts=[];
 const add=(id,label,value,section='context',unit=null)=>{if(value!==undefined&&value!==null&&value!=='')facts.push({id,label,value,section,unit,status:'recorded',evidence:[]});};
 for(const [id,key,label,section,unit] of [
  ['architecture','architecture','Architecture','model'],['model_role','model_role','Model type','model'],['initialization','initialization','Initialization','training'],['training_method','method','Training','training'],['selected_step','selected_step','Selected step','training','updates'],['optimizer_updates','optimizer_updates','Training steps','training','updates'],['outputs','output_semantics','Prediction','model'],
  ['physics_hz','physics_hz','Physics','physics','Hz'],['control_hz','control_hz','Control','physics','Hz'],['annotation','annotation_method','Annotation','annotations'],['source_demos','source_human_parent_count','Source demonstrations','collection'],['evaluation_mode','evaluation_scope','Evaluation','evaluation'],['evaluator','common_observer','Evaluator','evaluation'],['split','split','Split','population'],['valid','valid','Valid','population','cases'],['infra','infra','Infrastructure failures','population','cases'],['validation_mean_error','validation_mean_point_error_m','Validation point error','validation','m'],['validation_p95_error','validation_point_p95_m','Validation P95 error','validation','m']])add(id,label,p[key],section,unit);
 if(e.kind==='dataset'){add('count','Count',Object.hasOwn(d,'count')?d.count:p.episodes??p.count,'collection');add('count_unit','Unit',d.count_unit||p.count_unit||(p.episodes!=null?'episodes':null),'collection');add('frames','Frames',Object.hasOwn(d,'frames')?d.frames:p.frames,'collection');add('split','Split',d.splits?.length?d.splits:p.split_clips||p.split,'collection');}
 const protocol=e.protocol||p.protocol||{};
 for(const [id,label,unit] of [['distance_tolerance_mm','Position tolerance','mm'],['angle_tolerance_deg','Angle tolerance','°'],['continuous_s','Continuous hold','s'],['active_hold_s','Hold window','s']])add(id,label,protocol[id],'scoring',unit);
 const inputs=e.inputs?.length?e.inputs:p.inputs?.length?p.inputs:d.inputs||[];
 return {schema:'loom-research-summary/v1',kind:e.kind,facts,inputs,sources:[]};
}
function loomResearchFactMap(e){return new Map(loomResearchSummary(e).facts.map(f=>[f.id,f]));}
function loomResearchNumber(v){return Math.abs(v)>0&&Math.abs(v)<.001?Number(v).toExponential(3):Number(v).toLocaleString('en-US',{maximumFractionDigits:3});}
function loomResearchSplit(v){
 const items=Array.isArray(v)?v:Object.entries(v||{}).map(([id,value])=>({id,...(typeof value==='number'?{count:value}:Array.isArray(value)?{count:value.length}:value)}));
 const rank={train:0,validation:1,val:1,test:2};
 return [...items].sort((a,b)=>(rank[a.id]??3)-(rank[b.id]??3)).map(x=>{
  const n=x.count??x.adopted;return x.id+' '+(n==null?'—':loomResearchNumber(n));
 }).join(' / ');
}
function loomResearchValue(f,{compact=false}={}){
 if(f.status==='conflict')return 'Conflicting records';
 let v=f.value,unit=f.unit||'';
 if(f.id==='split'&&typeof v==='object')return loomResearchSplit(v);
 if(f.id==='architecture'&&typeof v==='string')return v.split(':')[0];
 if(f.id==='outputs'){
  if(Array.isArray(v))return v.join(' · ');
  if(v&&typeof v==='object')return Object.entries(v).map(([name,x])=>name+(x.shape?' ['+x.shape.join('×')+']':'')).join(' · ');
 }
 if(f.id==='model_role'&&typeof v==='string')return ({observer:'Observer',vla:'VLA',policy:'Policy',world_model:'World model'})[v]||v;
 if(f.id==='training_method'&&typeof v==='string')return v.split(';')[0];
 if(typeof v==='boolean')return v?'Yes':'No';
 if(typeof v==='number'){
  if(unit==='m'&&['validation_mean_error','validation_p95_error','point_mean_error','point_p95_error','hold_center_drift_m','height_tolerance_m'].includes(f.id)){v*=1000;unit='mm';}
  return loomResearchNumber(v)+(unit?' '+unit:'');
 }
 if(Array.isArray(v))return v.map(x=>typeof x==='object'?JSON.stringify(x):String(x)).join(' · ');
 if(v&&typeof v==='object')return JSON.stringify(v);
 return String(v??'—')+(unit?' '+unit:'');
}
function loomResearchInputs(e){return loomResearchSummary(e).inputs||[];}
function loomResearchInputName(input){
 const kinds={rgb:'RGB',depth:'Depth',rgbd:'RGB-D',state:'State',action:'Action',depth_validity:'Depth mask',camera_intrinsics:'Intrinsics',camera_transform:'Camera pose',instruction:'Instruction',joint_state:'Joint state',object_ground_truth:'Object GT',vision:'Vision'};
 if(input.kind==='vision'){
  const v=Array.isArray(input.value)?input.value.join(' · '):input.value;
  const modes=String(v||'').match(/RGB-D|RGB|Depth/gi);if(modes?.length)return [...new Set(modes)].join(' · ');
 }
 return kinds[input.kind]||input.label||input.kind;
}
function loomResearchInputSummary(e){
 const items=loomResearchInputs(e).filter(x=>x.status==='present'&&!['instruction','camera_intrinsics','camera_transform','depth_validity'].includes(x.kind));
 if(!items.length)return null;
 const names=[...new Set(items.map(x=>loomResearchInputName(x)+(x.shape?.length?' ['+x.shape.join('×')+']':x.kind==='state'&&typeof x.value==='string'&&/^[0-9]+D$/i.test(x.value)?' '+x.value:'')))];return names.join(' · ');
}
function loomResearchInstruction(e){
 const direct=loomResearchFactMap(e).get('instruction_mode');if(direct)return loomResearchValue(direct);
 const i=loomResearchInputs(e).find(x=>x.kind==='instruction');if(!i)return null;
 if(i.status==='absent')return 'None';if(i.status!=='present')return null;
 let v=i.mode||i.value;if(['사용','used','Present','present','Yes'].includes(v))v=null;return v?({fixed:'Fixed',phase_conditioned:'Phase-conditioned',overall:'Task-level'}[v]||String(v)):'Language-conditioned';
}
function loomResearchCardFacts(e){
 const map=loomResearchFactMap(e),isDataset=e.kind==='dataset',isModel=e.kind==='model';
 const order=isDataset?['count','frames','train_snapshots','validation_snapshots','inputs','split','physics','dynamics','annotation','annotation_source','instruction_mode','source_demos']:
  isModel?['model_role','architecture','inputs','outputs','validation_mean_error','validation_p95_error','test_action_mse','test_action_mae','test_action_rmse','selected_step','train_frames','validation_frames','train_snapshots','validation_snapshots','training_method','initialization','instruction_mode']:
  ['outcome','terminal_reason','evaluation_mode','evaluator','valid','split','validation_mean_error','validation_p95_error','point_mean_error','point_p95_error','selected_step','optimizer_updates','selection_validation_loss','test_action_rmse','distance_tolerance_mm','hold_center_drift_m','height_tolerance_m','angle_tolerance_deg','rotation_tolerance_deg','continuous_s','hold_seconds','physics_hz','control_hz','infra'];
 const input=loomResearchInputSummary(e);if(input)map.set('inputs',{id:'inputs',label:'Inputs',value:input,status:'recorded'});
 const instruction=loomResearchInstruction(e);if(instruction)map.set('instruction_mode',{id:'instruction_mode',label:'Instruction',value:instruction,status:'recorded'});
 if(isDataset){
  const physics=['physics_hz','control_hz','rgb_hz','state_hz'].map(id=>map.get(id)).filter(Boolean);
  if(physics.length)map.set('physics',{id:'physics',label:'Physics / sampling',value:physics.map(f=>f.label+' '+loomResearchValue(f)).join(' · '),status:'recorded'});
  const dynamics=['payload_mass','collision','robot_gravity_disabled'].map(id=>map.get(id)).filter(Boolean);
  if(dynamics.length)map.set('dynamics',{id:'dynamics',label:'Dynamics',value:dynamics.map(f=>f.id==='robot_gravity_disabled'?'Robot gravity '+(f.value?'off':'on'):f.label+' '+loomResearchValue(f)).join(' · '),status:'recorded'});
 }
 const count=map.get('count'),unit=map.get('count_unit')?.value;
 if(count)map.set('count',{...count,label:({episodes:'Episodes',clips:'Clips',cases:'Cases',snapshots:'Snapshots'})[unit]||'Count'});
 return order.map(id=>map.get(id)).filter(f=>f&&f.status!=='conflict').slice(0,isDataset?10:isModel?9:10);
}
const loomResearchOldModelMeta=modelFilterMeta;
modelFilterMeta=function(e){
 const result=loomResearchOldModelMeta(e),facts=loomResearchFactMap(e),role=facts.get('model_role'),architecture=facts.get('architecture');
 if(role?.status==='recorded')result.role=loomResearchValue(role);
 if(architecture?.status==='recorded')result.family=loomResearchValue(architecture,{compact:true});
 const inputs=loomResearchInputs(e).filter(i=>i.status==='present');
 const rgb=inputs.some(i=>i.kind==='rgb'||i.kind==='rgbd'||i.kind==='vision'&&/RGB/i.test(String(i.value||'')));
 const depth=inputs.some(i=>i.kind==='depth'||i.kind==='rgbd'||i.kind==='vision'&&/RGB-D/i.test(String(i.value||'')));
 if(rgb||depth)result.vision=rgb&&depth?'RGB-D':rgb?'RGB':'Depth';
 const instruction=loomResearchInstruction(e);if(instruction)result.instruction=instruction;
 return result;
};
catInputProperties=function(e){
 const inputs=loomResearchInputs(e).filter(x=>x.status==='present');
 return '<div class="pa-input-properties">'+inputs.map(x=>'<span class="pa-input-property"><small>'+esc(loomResearchInputName(x))+'</small><b>'+esc(x.shape?.length?x.shape.join('×'):Array.isArray(x.value)?x.value.join(' · '):x.value||x.unit||'')+'</b></span>').join('')+'</div>';
};
catFacts=function(e){
 if(['model','dataset','experiment','run'].includes(e.kind))return loomResearchCardFacts(e).map(f=>({role:f.id,label:f.label,value:loomResearchValue(f,{compact:true})}));
 return loomPriorCatalogFacts(e);
};
function loomResearchFactsHTML(facts,compact=false){return '<dl class="pa-entity-facts pa-scientific-facts '+(compact?'is-compact':'')+'">'+facts.map(f=>'<div data-research-fact="'+esc(f.id)+'"><dt>'+esc(f.label)+'</dt><dd title="'+esc(loomResearchValue(f))+'">'+esc(loomResearchValue(f,{compact}))+'</dd></div>').join('')+'</dl>';}
function loomResearchCard(e,result=false){
 const outcomes=result?loomRepresentativeOutcome(e):[],facts=loomResearchCardFacts(e),metrics=result?catResultMetrics(e):[];
 return '<article class="pa-catalog-item pa-scientific-card '+(e.kind==='dataset'?'pa-dataset-item':result?'pa-result-card':'')+'" data-output-id="'+esc(e.id)+'">'
  +loomTaskProperty(e)+'<div class="pa-row-main">'+catLink(e.id)+(result?resultDispositionPanel(e,true):'')+'</div>'
  +(outcomes.length?'<div class="pa-result-score">'+outcomes.map(m=>'<div class="pa-result-score-item"><small>'+esc(m.label)+'</small><strong>'+resultOutcomeValue(m)+'</strong></div>').join('')+'</div>':'')
  +loomResearchFactsHTML(facts,true)
  +(metrics.length>outcomes.length?'<details class="pa-output-metrics"><summary>Metrics · '+metrics.length+'</summary>'+catTable(['Metric','Value','Population'],metrics.map(m=>[esc(m.label),resultOutcomeValue(m),esc(m.population||m.denominator_label||m.aggregation||'—')]))+'</details>':'')+'</article>';
}
const loomResearchOldRow=catListRow;
catListRow=function(e){return ['model','dataset'].includes(e.kind)?loomResearchCard(e):loomResearchOldRow(e);};
resultCard=function(e){return loomResearchCard(e,true);};
function loomResearchOverview(e){
 const s=loomResearchSummary(e),map=loomResearchFactMap(e),inputs=s.inputs.filter(x=>x.status==='present');
 const order=e.kind==='model'?['model','validation','training','inputs','observations','physics','instructions','context']:
  e.kind==='dataset'?['collection','physics','observations','annotations','instructions','context']:
  ['evaluation','validation','training','scoring','population','physics','context','annotations','observations','instructions'];
 let body='<section class="pa-research-overview" aria-label="Research properties">';
 for(const id of order){
  if(id==='inputs'){
   if(inputs.length)body+=panel('Inputs',catTable(['Signal','Shape / value','Unit','Dtype','Preprocessing'],inputs.map(x=>[esc(x.label||loomResearchInputName(x)),esc(x.shape?.join('×')||(Array.isArray(x.value)?x.value.join(' · '):x.value)||'—'),esc(x.unit||'—'),esc(x.dtype||'—'),esc(x.preprocessing||'—')])));
   continue;
  }
  const facts=s.facts.filter(f=>f.section===id&&f.id!=='observation_schema'&&f.id!=='count_unit');
  if(id==='model'&&inputs.length)facts.push({id:'inputs',label:'Inputs',value:loomResearchInputSummary(e),status:'recorded'});
  if(!facts.length)continue;
  body+=panel(loomResearchSections[id],loomResearchFactsHTML(facts));
 }
 const instruction=loomResearchInstruction(e);if(instruction&&!map.has('instruction_mode'))body+=panel('Instructions',catProperties({Mode:instruction}));
 const sourceRows=s.facts.flatMap(f=>(f.evidence||[]).map(ev=>[esc(f.label),esc(ev.source?.ref||'Record metadata'),esc(ev.pointer)]));
 body+='<details class="pa-output-source"><summary>Sources</summary>'+catTable(['Fact','Source','JSON pointer'],sourceRows)
  +(s.sources?.some(x=>x.status!=='verified')?'<p class="pa-catalog-warning">Some registered metadata files could not be verified.</p>':'')+'</details></section>';
 return body;
}
const loomResearchOldDetail=catalogDetail;
catalogDetail=function(id){
 const e=catEntity(id);let html=loomResearchOldDetail(id);if(!e||!['model','dataset','run','experiment'].includes(e.kind))return html;
 const t=document.createElement('template');t.innerHTML=html;const title=t.content.querySelector('.pa-heading');if(!title)return html;
 // The canonical overview replaces generic flat highlights and arbitrary scalar
 // tiles. Full metrics, lineage and original producer metadata remain below.
 for(const node of t.content.querySelectorAll('.pa-entity-facts,.pa-model-meta,.pa-metric-grid,.pa-outcome-highlights'))node.remove();
 for(const node of t.content.querySelectorAll('.pa-panel')){
  const name=node.querySelector(':scope > .pa-panel-head > h2')?.textContent;
  if(['속성','Dataset Properties'].includes(name)){const details=document.createElement('details');details.className='pa-output-source';details.innerHTML='<summary>Source metadata</summary>'+node.innerHTML;node.replaceWith(details);}
 }
 title.insertAdjacentHTML('afterend',loomResearchOverview(e));
 title.insertAdjacentHTML('beforebegin',loomTaskProperty(e));
 const primary=['run','experiment'].includes(e.kind)?loomRepresentativeOutcome(e):[];
 if(primary.length)title.insertAdjacentHTML('afterend',panel('Outcome',catTable(['Metric','Value','Population'],primary.map(m=>[esc(m.label),resultOutcomeValue(m),esc(m.population||m.denominator_label||'—')]))));
 const record=document.createElement('details');record.className='pa-output-source';record.innerHTML='<summary>Record</summary>'+catProperties({Owner:e.owner_profile?.display_name||e.owner||'—',ID:e.id});t.content.append(record);
 return t.innerHTML;
};
