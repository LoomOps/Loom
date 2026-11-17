const loomReportingPriorOutcome=loomRepresentativeOutcome;
loomRepresentativeOutcome=function(e){const p=e.reporting?.primary_outcome;if(!p)return loomReportingPriorOutcome(e);return [{id:'whole_task_success',label:p.label,numerator:p.numerator,denominator:p.denominator,value:p.denominator?p.numerator/p.denominator:null,kind:'rate',population:'Valid cases',unit:'ratio'}];};
resultOutcomeRows=loomRepresentativeOutcome;
// Exact report bindings drive bounded views. Legacy records remain readable.
const loomReportOverviewBodies=new Map();
const loomReportViews=new Map(),loomReportPages=new Map(),loomReportPending=new Set();
const loomReportPointer=(value,pointer)=>pointer===''?value:String(pointer||'').split('/').slice(1).reduce((v,k)=>v?.[k.replace(/~1/g,'/').replace(/~0/g,'~')],value);
function loomReportState(e){if(!loomReportViews.has(e.id))loomReportViews.set(e.id,{tab:'overview',phase:'all',split:'all'});return loomReportViews.get(e.id);}
function loomReportTabs(e){return e.kind==='dataset'?[['overview','Overview'],['distribution','Distribution'],['quality','Quality'],['related','Related'],['contract','Contract']]:e.kind==='model'?[['overview','Overview'],['instructions','Instructions'],['learning','Learning'],['metrics','Metrics'],['inputs','Inputs'],['contract','Contract']]:[['overview','Overview'],['protocol','Protocol'],['metrics','Metrics'],['cases','Cases'],['inputs','Inputs'],['contract','Contract']];}
function loomReportIdentity(e){return {id:e.loom_registry?.id||e.id,revision:e.loom_registry?.revision||e.revision||1};}
function loomReportKey(e,r,f){return JSON.stringify([loomReportIdentity(e),r.requirement_id,r.sha256,f.pointer]);}
function loomReportRefresh(e){for(const n of root.querySelectorAll('[data-report-output]'))if(n.dataset.reportOutput===e.id){n.querySelector('[data-report-content]').innerHTML=loomReportContent(e);}}
function loomReportLoad(e,r,f,more=false){
 const key=loomReportKey(e,r,f);if(loomReportPending.has(key))return;
 const cached=loomReportPages.get(key);if(cached&&!more)return;
 const offset=more?cached?.next_offset:0;if(more&&offset==null)return;
 loomReportPending.add(key);
 queueMicrotask(async()=>{
  try{const page=await loomSearchAction('results','read_report',{...loomReportIdentity(e),requirement_id:r.requirement_id,pointer:f.pointer,offset:offset||0,limit:100});
   loomReportPages.set(key,{...page,value:more?[...(cached.value||[]),...(page.value||[])]:page.value});
  }catch(error){loomReportPages.set(key,{error:error.message||'Report unavailable'});}
  finally{loomReportPending.delete(key);while(loomReportPages.size>160)loomReportPages.delete(loomReportPages.keys().next().value);loomReportRefresh(e);}
 });
}
function loomReportNumber(v){return typeof v==='number'?v.toLocaleString('en-US',{maximumSignificantDigits:7}):v==null?'Missing':String(v);}
function loomReportCell(v){return v==null?'Missing':typeof v==='object'?(v.id?String(v.id)+(v.revision?' · r'+v.revision:''):JSON.stringify(v)):loomReportNumber(v);}
function loomReportChart(rows,f,format){
 const points=rows.map(row=>({x:loomReportPointer(row,f.x_pointer),y:loomReportPointer(row,f.y_pointer),id:row.id??row.step,group:[row.phase??'all',row.split??'all'].join(' · ')})).filter(p=>Number.isFinite(p.x)&&Number.isFinite(p.y));
 if(!points.length)return '<p class="pa-report-missing">Not measured</p>';
 const xs=points.map(p=>p.x),ys=points.map(p=>p.y),minX=Math.min(...xs),maxX=Math.max(...xs),minY=Math.min(...ys),maxY=Math.max(...ys),dx=maxX-minX||1,dy=maxY-minY||1;
 const x=v=>52+(v-minX)/dx*500,y=v=>210-(v-minY)/dy*180;
 const groups=[...new Set(points.map(p=>p.group))];if(groups.length>8)return '<p class="pa-report-missing">Choose a phase or split to plot these measurements.</p>';
 const line=format==='series'?groups.map((g,index)=>'<polyline fill="none" stroke-dasharray="'+(['','6 3','2 3','8 2 2 2'][index%4])+'" points="'+points.filter(p=>p.group===g).sort((a,b)=>a.x-b.x).map(p=>x(p.x)+','+y(p.y)).join(' ')+'"/>').join(''):'';
 const legend=groups.length>1?'<div class="pa-report-series">'+groups.map((g,i)=>'<span><i style="border-bottom-style:'+(['solid','dashed','dotted','dashed'][i%4])+'"></i>'+esc(g)+'</span>').join('')+'</div>':'';
 return '<div class="pa-report-chart">'+legend+'<svg viewBox="0 0 580 260" role="img" aria-label="'+esc(f.label+' · '+points.length+' recorded points')+'"><line class="pa-report-axis" x1="52" y1="210" x2="552" y2="210"/><line class="pa-report-axis" x1="52" y1="30" x2="52" y2="210"/>'+line+points.map(p=>'<circle cx="'+x(p.x)+'" cy="'+y(p.y)+'" r="3"><title>'+esc(p.id+' · '+p.group+' · X '+loomReportNumber(p.x)+' · Y '+loomReportNumber(p.y))+'</title></circle>').join('')+'</svg><div class="pa-report-axes"><span>X · '+esc(f.x_label||'X')+' · '+esc(loomReportNumber(minX))+' – '+esc(loomReportNumber(maxX))+'</span><span>Y · '+esc(f.y_label||'Y')+' · '+esc(loomReportNumber(minY))+' – '+esc(loomReportNumber(maxY))+(f.unit?' '+esc(f.unit):'')+'</span></div></div>';
}
function loomReportField(e,r,f){
 if(f.format==='properties')return catProperties(Object.fromEntries(Object.entries(f.value||{}).map(([k,v])=>[k,loomReportCell(v)])));
 if(!['table','series','scatter','instructions'].includes(f.format))return '<dl class="pa-entity-facts"><div><dt>'+esc(f.label)+'</dt><dd>'+esc(loomReportCell(f.value))+(f.unit?' '+esc(f.unit):'')+'</dd></div></dl>';
 const key=loomReportKey(e,r,f),page=loomReportPages.get(key);if(!page){loomReportLoad(e,r,f);return '<p class="pa-report-missing">Loading…</p>';}
 if(page.error)return '<p class="pa-catalog-warning">'+esc(page.error)+'</p>';
 const state=loomReportState(e),all=Array.isArray(page.value)?page.value:[],rows=all.filter(row=>(state.phase==='all'||String(row.phase)===state.phase)&&(state.split==='all'||row.split===state.split));
 let body='';
 if(f.format==='instructions'&&!rows.length)return '<p class="pa-report-missing">'+((r.fields||[]).some(f=>f.id==='mode'&&f.value==='none')?'Not applicable':'Missing')+'</p>';
 if(f.format==='instructions')body=rows.map(row=>'<article class="pa-report-instruction"><h3>'+esc(row.phase!=null?'Phase '+row.phase:row.id)+'</h3><blockquote>'+esc(row.text)+'</blockquote></article>').join('');
 else {
  const columns=f.columns?.length?f.columns:(f.format==='series'?[{label:'Step',pointer:f.x_pointer},{label:'Value',pointer:f.y_pointer}] : [{label:'Record',pointer:'/id'}]);
  const actualColumns=f.format==='series'?[...columns,{label:'Phase',pointer:'/phase'},{label:'Split',pointer:'/split'}]:columns;
  const table=catTable(actualColumns.map(c=>c.label),rows.map(row=>actualColumns.map(c=>esc(loomReportCell(loomReportPointer(row,c.pointer)))+(c.unit?' '+esc(c.unit):''))));
  body=(f.format==='series'||f.format==='scatter')?loomReportChart(rows,f,f.format)+'<details><summary>Recorded values</summary>'+table+'</details>':table;
 }
 return body+'<div class="pa-report-page"><span>'+rows.length+' shown · '+all.length+' / '+(page.total??all.length)+' loaded</span>'+(page.next_offset!=null?'<button class="pa-button" data-report-more="'+esc(r.requirement_id)+'" data-report-field="'+esc(f.id)+'">More</button>':'')+'</div>';
}
function loomReportContract(e){
 const v=e.reporting;if(!v)return '<p class="pa-report-missing">Reporting contract · Missing</p>';
 const rows=(v.reports||[]).map(r=>[esc(r.label),esc(r.role),r.required?'Required':'Optional',r.state==='recorded'?'Recorded':'Missing']);
 return catProperties({Contract:v.contract_ref.id,Revision:v.contract_ref.revision,SHA256:v.contract_ref.sha256,Activity:v.activity})+catTable(['Report','Role','Coverage','State'],rows)+(v.adaptations?.length?'<details><summary>Excluded / replaced requirements</summary>'+catTable(['Requirement','Disposition','Reason','Analysis impact'],v.adaptations.map(a=>[esc(a.requirement_id),esc(a.disposition),esc(a.reason),esc(a.analysis_impact)]))+'<pre>'+esc(JSON.stringify(v.adaptations,null,2))+'</pre></details>':'');
}
function loomReportContent(e){
 const state=loomReportState(e),tab=state.tab,r=e.reporting;
 if(tab==='contract')return loomReportContract(e);
 if(tab==='related')return loomDatasetParentHTML(e);
 if(tab==='inputs'&&e.kind!=='model')return catTable(['Input','Record','Revision'],(r?.inputs||[]).map(i=>[esc(i.input),'<button class="pa-button" data-report-input="'+esc(i.id)+'" data-report-input-revision="'+esc(i.revision)+'">'+esc(catEntity(i.id)?.label||i.id)+'</button>',esc(i.revision)]));
 if(tab==='overview'){
  const instructions=(r?.reports||[]).filter(r=>r.role==='instructions'&&r.state==='recorded');
  return instructions.map(report=>loomResearchSection('Instructions',(report.fields||[]).filter(f=>f.format==='instructions').map(f=>loomReportField(e,report,f)).join(''))).join('')+((r?.reports||[]).some(r=>r.role==='policy_inputs')?loomResearchSection('Action interface',(r.reports.find(r=>r.role==='policy_inputs').fields||[]).filter(f=>['robot_dof','action_dimension'].includes(f.id)).map(f=>loomReportField(e,{},f)).join('')):'')+(loomReportOverviewBodies.get(e.id)||loomResearchOverview(e));
 }
 const roles={instructions:['instructions'],learning:['learning_series'],metrics:['action_metrics','evaluation_cases'],inputs:['policy_inputs'],protocol:['evaluation_cases'],cases:['evaluation_cases'],distribution:['sdg_conditions','sdg_readback'],quality:['dataset_statistics']};
 const reports=(r?.reports||[]).filter(r=>(roles[tab]||[]).includes(r.role));
 if(!reports.length){const excluded=(r?.exclusions||[]).some(x=>(roles[tab]||[]).includes(x.role));return '<p class="pa-report-missing">'+esc(loomReportTabs(e).find(([id])=>id===tab)?.[1]||tab)+' · '+(excluded?'Not applicable':'Missing')+'</p>';}
 const loaded=reports.flatMap(r=>(r.fields||[]).flatMap(f=>{const v=loomReportPages.get(loomReportKey(e,r,f))?.value;return Array.isArray(v)?v:[]}));
 const phaseOptions=['all',...[...new Set(loaded.map(r=>r.phase).filter(v=>v!=null).map(String))].filter(v=>v!=='all')];
 const splitOptions=['all',...[...new Set(loaded.map(r=>r.split).filter(v=>v!=null).map(String))].filter(v=>v!=='all')];
 const filters=['learning','metrics','cases','distribution'].includes(tab)?'<div class="pa-report-controls"><label>Phase<select data-report-filter="phase">'+phaseOptions.map(v=>'<option value="'+v+'"'+(state.phase===v?' selected':'')+'>'+(v==='all'?'All phases':'Phase '+v)+'</option>').join('')+'</select></label><label>Split<select data-report-filter="split">'+splitOptions.map(v=>'<option value="'+v+'"'+(state.split===v?' selected':'')+'>'+({all:'All splits'}[v]||v)+'</option>').join('')+'</select></label></div>':'';
 return filters+reports.map(report=>{
  let fields=report.fields||[];
  if(tab==='cases')fields=fields.filter(f=>f.format==='table');
  if(tab==='metrics')fields=fields.filter(f=>f.format!=='table'||report.role==='action_metrics');
  if(tab==='protocol')return loomResearchSection('Protocol',catProperties({Reference:r.contract_ref.id,'Report source':report.component})+catTable(['Accepted input','Record','Revision'],(r.inputs||[]).map(i=>[esc(i.input),esc(i.id),esc(i.revision)]))+'<button class="pa-button" data-report-protocol="'+esc(report.requirement_id)+'">Protocol definition</button><div data-report-protocol-detail></div>');
  return loomResearchSection(report.label,report.state!=='recorded'?'<p class="pa-report-missing">Missing</p>':fields.map(f=>'<div class="pa-report-field"><h3>'+esc(f.label)+'</h3>'+loomReportField(e,report,f)+'</div>').join(''));
 }).join('');
}
const loomReportPreviousDetail=catalogDetail;
catalogDetail=function(id){
 const e=catEntity(id),html=loomReportPreviousDetail(id);if(!e||!['dataset','model','experiment','run'].includes(e.kind)||!catalogData.capabilities?.reporting)return html;
 const t=document.createElement('template');t.innerHTML=html;const overview=t.content.querySelector('.pa-research-overview');if(!overview)return html;
 loomReportOverviewBodies.set(e.id,overview.outerHTML);while(loomReportOverviewBodies.size>160)loomReportOverviewBodies.delete(loomReportOverviewBodies.keys().next().value);
 const view=loomReportState(e);const parent=document.createElement('section');parent.className='pa-report-workspace';parent.dataset.reportOutput=e.id;
 parent.innerHTML='<nav class="pa-report-tabs" aria-label="Output details">'+loomReportTabs(e).map(([id,label])=>'<button data-report-tab="'+id+'" aria-current="'+(view.tab===id?'page':'false')+'">'+label+'</button>').join('')+'</nav><div data-report-content>'+loomReportContent(e)+'</div>';
 overview.replaceWith(parent);return t.innerHTML;
};
root.addEventListener('click',async event=>{
 const workspace=event.target.closest('[data-report-output]'),e=workspace&&catEntity(workspace.dataset.reportOutput);if(!e)return;
 const input=event.target.closest('[data-report-input]');if(input){try{const pin=await loomSearchAction('results','resolve_item',{native_ref:{kind:'entity',identity:{id:input.dataset.reportInput,revision:Number(input.dataset.reportInputRevision)}}});const value=await loomSearchAction('system','dashboard_entity',{item_id:pin.item_id,revision_id:pin.revision_id});loomCatalogMerge(value);if(value.entities?.[0])openView(catView(value.entities[0].id));}catch(error){input.textContent=error.message;}return;}
 const tab=event.target.closest('[data-report-tab]');if(tab){Object.assign(loomReportState(e),{tab:tab.dataset.reportTab,phase:'all',split:'all'});render();return;}
 const more=event.target.closest('[data-report-more]');if(more){const r=e.reporting.reports.find(r=>r.requirement_id===more.dataset.reportMore),f=r.fields.find(f=>f.id===more.dataset.reportField);loomReportLoad(e,r,f,true);return;}
 const protocol=event.target.closest('[data-report-protocol]');if(protocol){
  protocol.disabled=true;const target=workspace.querySelector('[data-report-protocol-detail]');
  try{const page=await loomSearchAction('results','read_report',{...loomReportIdentity(e),requirement_id:protocol.dataset.reportProtocol,pointer:'/protocol_ref'});const p=await loomSearchAction('results','get_report_definition',{id:page.value.id,revision:page.value.revision});if(target.isConnected)target.innerHTML=catProperties({Protocol:p.label,Mode:p.definition.mode,Environment:p.definition.environment,Controller:p.definition.controller,Evaluator:p.definition.scope?.evaluator?.kind,'Policy GT':p.definition.scope?.gt_access?.policy,'Planner GT':p.definition.scope?.gt_access?.planner,'Scoring GT':p.definition.scope?.gt_access?.scoring,'Policy active during hold':p.definition.scope?.policy_active_during_hold,'Goal frame':p.definition.scope?.goal_frame})+catTable(['Measurement','Condition','Threshold','Unit'],(p.definition.predicate.all||[]).map(rule=>[esc(rule.pointer),esc(rule.op),esc(rule.op==='between'?rule.min+' – '+rule.max:rule.value),esc(rule.unit)]));}
  catch(error){if(target.isConnected)target.textContent=error.message;}finally{if(protocol.isConnected)protocol.disabled=false;}
 }
});
root.addEventListener('change',event=>{const workspace=event.target.closest('[data-report-output]');if(!workspace||!event.target.dataset.reportFilter)return;const e=catEntity(workspace.dataset.reportOutput);loomReportState(e)[event.target.dataset.reportFilter]=event.target.value;render();});

root.addEventListener('click',async event=>{
 if(!event.target.closest('[data-report-contracts]'))return;
 const dialog=loomHubDialog('Report contracts','<div data-report-contract-list></div><div data-report-contract-detail></div>');
 let offset=0,sequence=0;const rows=[];
 async function load(){const page=await loomSearchAction('results','report_contracts',{offset,limit:50});if(!dialog.isConnected)return;rows.push(...page.items);offset=page.next_offset;dialog.querySelector('[data-report-contract-list]').innerHTML=catTable(['Contract','Kind','Origin','Revision'],rows.map(c=>['<button class="pa-button" data-report-contract-id="'+esc(c.id)+'" data-report-contract-revision="'+esc(c.revision)+'">'+esc(c.label)+'</button>',esc(c.output_kind),c.origin==='builtin'?'Built-in':'Registered',esc(c.revision)]))+(offset!=null?'<button class="pa-button" data-report-contract-more>More</button>':'');}
 dialog.addEventListener('click',async event=>{
  const b=event.target.closest('[data-report-contract-id]');try{
   if(event.target.closest('[data-report-contract-more]')){await load();return;}if(!b)return;const current=++sequence;
   const c=await loomSearchAction('results','get_report_contract',{id:b.dataset.reportContractId,revision:Number(b.dataset.reportContractRevision)});if(!dialog.isConnected||current!==sequence)return;
   const requirements=c.definition.effective_requirements||c.definition.requirements;
   dialog.querySelector('[data-report-contract-detail]').innerHTML='<h3>'+esc(c.label)+'</h3>'+catTable(['Report','Role','Coverage'],requirements.map(r=>[esc(r.label),esc(r.role),r.required?'Required':'Optional']))+'<details><summary>Definition and extension schema</summary><pre>'+esc(JSON.stringify(c.definition,null,2))+'</pre></details>';
  }catch(error){if(dialog.isConnected)dialog.querySelector('[data-hub-result]').textContent=error.message;}
 });
 try{await load();}catch(error){if(dialog.isConnected)dialog.querySelector('[data-hub-result]').textContent=error.message;}
});
