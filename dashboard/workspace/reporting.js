const loomReportingPriorOutcome=loomRepresentativeOutcome;
function loomCheckpointHeadline(sweep){
 const best=sweep.best_steps||[];
 const step=sweep.selected_step??(best.length===1?best[0]:null);
 const row=sweep.checkpoints?.find(c=>c.step===step);
 if(row?.valid){
  const label=sweep.rule==='prespecified'?'Selected':sweep.rule==='best_success_rate'?'Best':'Best observed';
  return {label:label+' · step '+loomReportNumber(row.step),step:row.step,numerator:row.successes,denominator:row.valid};
 }
 if(step==null&&best.length>1&&sweep.comparable){
  const tied=(sweep.checkpoints||[]).filter(c=>best.includes(c.step));
  const first=tied[0];
  if(first?.valid&&tied.length===best.length&&tied.every(c=>c.valid===first.valid&&c.successes===first.successes)){
   return {label:'Best observed · tied',tied:true,numerator:first.successes,denominator:first.valid};
  }
 }
 return null;
}
loomRepresentativeOutcome=function(e){
 const sweep=e.reporting?.checkpoint_sweep,p=sweep?loomCheckpointHeadline(sweep):e.reporting?.primary_outcome;
 if(!p)return sweep?[]:loomReportingPriorOutcome(e);
 return [{id:'whole_task_success',label:p.label,numerator:p.numerator,denominator:p.denominator,value:p.denominator?p.numerator/p.denominator:null,kind:'rate',population:'Valid cases',unit:'ratio'}];
};
resultOutcomeRows=loomRepresentativeOutcome;
// Exact report bindings drive bounded views. Legacy records remain readable.
const loomReportOverviewBodies=new Map();
const loomReportLegacyMetrics=new Map();
const loomReportViews=new Map(),loomReportPages=new Map(),loomReportPending=new Set();
const loomReportPointer=(value,pointer)=>pointer===''?value:String(pointer||'').split('/').slice(1).reduce((v,k)=>v?.[k.replace(/~1/g,'/').replace(/~0/g,'~')],value);
function loomReportState(e){if(!loomReportViews.has(e.id))loomReportViews.set(e.id,{tab:'overview',phase:'all',split:'all'});return loomReportViews.get(e.id);}
function loomCheckpointRows(e,rows){
 const step=loomReportState(e).checkpoint;
 return step==null||step==='all'?rows:rows.filter(row=>String(row.factors?.update)===step);
}
function loomCheckpointPicker(e){
 const groups=e.reporting?.checkpoint_sweep?.checkpoints;
 if(!groups||groups.length<2)return '';
 const step=loomReportState(e).checkpoint;
 return '<div class="pa-report-controls"><label>Checkpoint<select data-report-filter="checkpoint"><option value="all">All checkpoints</option>'+groups.map(g=>'<option value="'+esc(g.step)+'"'+(String(g.step)===step?' selected':'')+'>Step '+esc(loomReportNumber(g.step))+'</option>').join('')+'</select></label></div>';
}
function loomCheckpointOverview(sweep,detail=false){
 const headline=loomCheckpointHeadline(sweep);
 if(!detail)return headline?loomResearchSection('Outcome',catProperties({[headline.label]:headline.numerator+' / '+headline.denominator+' · '+loomReportNumber(100*headline.numerator/headline.denominator)+'%'})):'';
 const rows=sweep.checkpoints.map(g=>{
  const badge=g.step===sweep.selected_step?(sweep.rule==='prespecified'?'Selected':'Best'):sweep.best_steps?.includes(g.step)?'Best observed':'';
  return [esc(loomReportNumber(g.step)),esc(g.valid?g.successes+' / '+g.valid+' · '+loomReportNumber(100*g.successes/g.valid)+'%':'—'),esc(g.executed),esc(badge)];
 });
 return loomResearchSection('Checkpoint results',catTable(['Step','Success / valid','Executed','Selection'],rows)
  +(sweep.best_steps?.length>1?'<p class="pa-report-note">Tied on the recorded cohort</p>':'')
  +(!sweep.comparable?'<p class="pa-report-note">Checkpoint cohorts differ or are incomplete</p>':''));
}
function loomReportTabs(e){
 if(e.kind==='dataset'){
  const members=e.reporting?.reports?.some(r=>r.role==='dataset_statistics'&&r.fields?.some(f=>f.id==='members'&&f.format==='table'));
  const coverage=e.reporting?.reports?.some(r=>['sdg_conditions','sdg_readback','distribution'].includes(r.role));
  return [['overview','Overview'],...(coverage?[['distribution','Distribution']]:[]),...(members?[['episodes','Episodes']]:[])];
 }
 if(e.kind==='model')return [['overview','Overview'],['training','Training']];
 return [['overview','Overview'],['results','Results'],['cases','Cases']];
}
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
  }catch(error){loomReportPages.set(key,{...cached,error:error.message||'Report unavailable'});}
  finally{loomReportPending.delete(key);while(loomReportPages.size>160)loomReportPages.delete(loomReportPages.keys().next().value);loomReportRefresh(e);}
 });
}
function loomReportNumber(v){return typeof v==='number'?v.toLocaleString('en-US',{maximumSignificantDigits:7}):v==null?'Missing':String(v);}
function loomReportCell(v){return v==null?'Missing':typeof v==='object'?(v.id?String(v.id)+(v.revision?' · r'+v.revision:''):JSON.stringify(v)):loomReportNumber(v);}
function loomReportChart(rows,f,format,selectedStep,inspectedStep,members){
 const points=rows.map(row=>({x:loomReportPointer(row,f.x_pointer),y:loomReportPointer(row,f.y_pointer),id:row.id??row.step,group:[row.phase==null||row.phase==='all'?'Overall':'Subtask '+row.phase,row.split].filter(Boolean).join(' · ')})).filter(p=>Number.isFinite(p.x)&&Number.isFinite(p.y));
 if(!points.length)return '<p class="pa-report-missing">Not measured</p>';
 const xs=points.map(p=>p.x),ys=points.map(p=>p.y),minX=Math.min(...xs),maxX=Math.max(...xs),minY=Math.min(...ys),maxY=Math.max(...ys),dx=maxX-minX||1,dy=maxY-minY||1;
 const pairedLength=format==='scatter'&&['mm','cm','m'].includes(f.unit);
 const left=84,bottom=206,scale=Math.min(500/dx,180/dy);
 const xWidth=pairedLength?dx*scale:500,yHeight=pairedLength?dy*scale:180,top=bottom-yHeight;
 const x=v=>left+(v-minX)/dx*xWidth,y=v=>bottom-(v-minY)/dy*yHeight;
 const groups=[...new Set(points.map(p=>p.group))];if(groups.length>8)return '<p class="pa-report-missing">Choose a subtask or split to plot these measurements.</p>';
 const line=format==='series'?groups.map((g,index)=>'<polyline fill="none" stroke-dasharray="'+(['','6 3','2 3','8 2 2 2'][index%4])+'" points="'+points.filter(p=>p.group===g).sort((a,b)=>a.x-b.x).map(p=>x(p.x)+','+y(p.y)).join(' ')+'"/>').join(''):'';
 const legend=format==='series'||groups.length>1?'<div class="pa-report-series">'+groups.map((g,i)=>'<span><i style="border-bottom-style:'+(['solid','dashed','dotted','dashed'][i%4])+'"></i>'+esc(g==='Overall'?f.label:g)+'</span>').join('')+'</div>':'';
 const selected=Number.isFinite(selectedStep)&&selectedStep>=minX&&selectedStep<=maxX;
 const marker=selected?'<line class="pa-report-checkpoint" x1="'+x(selectedStep)+'" y1="'+top+'" x2="'+x(selectedStep)+'" y2="'+bottom+'"/>':'';
 const dots=points.map(p=>{
  const title=p.id+' · '+p.group+' · X '+loomReportNumber(p.x)+' · Y '+loomReportNumber(p.y);
  const dot='<circle cx="'+x(p.x)+'" cy="'+y(p.y)+'" r="'+(p.x===inspectedStep?6:3)+'"><title>'+esc(title)+'</title></circle>';
  const member=members?.get(p.id);
  return member?'<a href="#episode" data-report-episode="'+esc(loomReportMemberKey({id:member}))+'" aria-label="'+esc(title)+'">'+dot+'</a>':dot;
 }).join('');
 const ticks=(min,max,length)=>min===max?[min]:Array.from({length:length<100?2:5},(_,i)=>min+(max-min)*i/(length<100?1:4));
 const tickLabel=v=>esc(v!==0&&(Math.abs(v)<0.001||Math.abs(v)>=1e7)?v.toExponential(2):v.toLocaleString('en-US',{maximumSignificantDigits:4}));
 const xTicks=ticks(minX,maxX,xWidth).map(v=>'<text x="'+x(v)+'" y="'+(bottom+19)+'" text-anchor="middle">'+tickLabel(v)+'</text>').join('');
 const yTicks=ticks(minY,maxY,yHeight).map(v=>'<line class="pa-report-grid" x1="'+left+'" x2="'+(left+xWidth)+'" y1="'+y(v)+'" y2="'+y(v)+'"/><text x="'+(left-8)+'" y="'+y(v)+'" text-anchor="end" dominant-baseline="middle">'+tickLabel(v)+'</text>').join('');
 const unit=f.unit?' ('+f.unit+')':'';
 const xLabel=esc((f.x_label||'X')+(pairedLength?unit:'')),yLabel=esc((f.y_label||'Y')+unit);
 const axes='<line class="pa-report-axis" x1="'+left+'" y1="'+bottom+'" x2="'+(left+xWidth)+'" y2="'+bottom+'"/><line class="pa-report-axis" x1="'+left+'" y1="'+top+'" x2="'+left+'" y2="'+bottom+'"/>';
 const labels='<text class="pa-report-axis-label" x="'+(left+xWidth/2)+'" y="247" text-anchor="middle">'+xLabel+'</text><text class="pa-report-axis-label" transform="translate(14 '+(top+yHeight/2)+') rotate(-90)" text-anchor="middle">'+yLabel+'</text>';
 return '<div class="pa-report-chart">'+legend+'<svg viewBox="0 0 620 260" role="'+(members?.size?'group':'img')+'" aria-label="'+esc(f.label+' · '+points.length+' recorded points')+'">'+yTicks+xTicks+axes+labels+line+marker+dots+'</svg></div>';
}
function loomReportMore(page,r,f,label){
 return page.next_offset!=null?'<button class="pa-button" data-report-more="'+esc(r.requirement_id)+'" data-report-field="'+esc(f.id)+'">'+esc(label)+'</button>':'';
}
function loomReportPageControls(page,r,f,rows,all){
 const more=loomReportMore(page,r,f,"Load more records");
 return more||rows!==page.total?'<div class="pa-report-page"><span>'+rows+' shown · '+all+' / '+(page.total??all)+' loaded</span>'+more+'</div>':'';
}
function loomReportMember(e,r,f,rows,page){
 const label=e.kind==='dataset'?'Episode':'Case';
 const more=loomReportMore(page,r,f,'Load more '+label.toLowerCase()+'s');
 if(!rows.length)return '<p class="pa-report-missing">No recorded members on this page</p>'+more;
 const state=loomReportState(e),active=state.member?rows.find(row=>loomReportMemberKey(row)===state.member):rows[0];
 const placeholder=active?'':'<option disabled selected>Selected record is outside the loaded rows</option>';
 const options=rows.map(row=>'<option value="'+esc(loomReportMemberKey(row))+'"'+(active===row?' selected':'')+'>'+esc(row.id??row.condition_id)+(row.seed!=null?' · seed '+esc(row.seed):'')+'</option>').join('');
 const menu='<div class="pa-report-controls pa-report-member-controls"><label>'+label+'<select data-report-filter="member">'+placeholder+options+'</select></label>'+more+'</div>';
 if(!active)return menu;
 const fields=f.columns?.length?f.columns.map(c=>[c.label,loomReportCell(loomReportPointer(active,c.pointer))+(c.unit?' '+c.unit:'')]):[];
 const nested=Object.entries(active).filter(([,value])=>value&&typeof value==='object');
 return menu+catProperties(Object.fromEntries(fields))
  +(nested.length?'<details><summary>Recorded evidence</summary>'+catTable(['Field','Value'],nested.map(([key,value])=>[esc(key),esc(JSON.stringify(value))]))+'</details>':'');
}
const loomReportMemberKey=row=>JSON.stringify([row.id??row.condition_id,row.seed]);
function loomReportConditionMembers(e){
 if(!loomReportTabs(e).some(([tab])=>tab==='episodes'))return null;
 const members=new Map();
 for(const report of e.reporting?.reports||[]){
  if(report.role!=='sdg_readback')continue;
  for(const field of report.fields||[]){
   if(field.id!=='observations')continue;
   const rows=loomReportPages.get(loomReportKey(e,report,field))?.value;
   for(const row of rows||[]){
    if(typeof row.id!=='string'||typeof row.member_id!=='string')continue;
    if(!members.has(row.id))members.set(row.id,row.member_id);
    else if(members.get(row.id)!==row.member_id)members.set(row.id,null);
   }
  }
 }
 return members;
}
function loomReportCaseGraph(e){
 const r=e.reporting,criteria=r?.evaluation_summary?.criteria;
 const report=r?.reports?.find(report=>report.state==='recorded'&&report.role==='evaluation_cases');
 const field=report?.fields?.find(f=>f.id==='cases'&&f.format==='table');
 if(!criteria?.length||!field)return '';
 const page=loomReportPages.get(loomReportKey(e,report,field));
 if(!page){loomReportLoad(e,report,field);return '<p class="pa-report-missing">Loading cases…</p>';}
 if(page.error)return '<p class="pa-catalog-warning">'+esc(page.error)+' <button class="pa-button" data-report-retry="'+esc(report.requirement_id)+'" data-report-field="'+esc(field.id)+'">Retry</button></p>';
 const state=loomReportState(e),index=Number(state.criterion)||0,rule=criteria[index]||criteria[0];
 const label=r=>r.pointer.split('/').at(-1).replaceAll('_',' ');
 const points=loomCheckpointRows(e,page.value).flatMap(row=>{
  const m=loomReportPointer(row,rule.pointer);
  return m?.unit===rule.unit&&Number.isFinite(m?.value)?[{row,value:m.value}]:[];
 });
 const selector='<label>Measurement<select data-report-filter="criterion">'+criteria.map((rule,i)=>'<option value="'+i+'"'+(i===index?' selected':'')+'>'+esc(label(rule)+' · '+rule.unit)+'</option>').join('')+'</select></label>';
 if(!points.length)return loomResearchSection('Case measurements',selector+'<p class="pa-report-missing">No recorded values on this page</p>'+loomReportPageControls(page,report,field,0,page.value.length));
 const thresholds=rule.op==='between'?[rule.min,rule.max]:[rule.value];
 const values=[...points.map(p=>p.value),...thresholds],min=Math.min(...values),max=Math.max(...values);
 const x=v=>42+(max===min?0.5:(v-min)/(max-min))*483;
 const markers=thresholds.map(value=>'<line class="pa-report-checkpoint" x1="'+x(value)+'" x2="'+x(value)+'" y1="12" y2="105"/>').join('');
 const dots=points.map(({row,value},i)=>{
  const name=row.id+' · '+loomReportNumber(value)+' '+rule.unit;
  return '<a href="#case" data-report-case="'+esc(loomReportMemberKey(row))+'" aria-label="'+esc(name)+'"><circle cx="'+x(value)+'" cy="'+(18+i%10*8)+'" r="4"><title>'+esc(name)+'</title></circle></a>';
 }).join('');
 const chart='<div class="pa-report-chart pa-report-case-chart"><svg viewBox="0 0 565 128" role="group" aria-label="'+esc(label(rule)+' measurements')+'">'+markers+dots+'<line class="pa-report-axis" x1="42" y1="105" x2="525" y2="105"/><text x="42" y="122">'+esc(loomReportNumber(min))+'</text><text x="525" y="122" text-anchor="end">'+esc(loomReportNumber(max)+' '+rule.unit)+'</text></svg></div>';
 const criterion='<p class="pa-report-note">Protocol threshold · '+esc(({lte:'≤',gte:'≥',between:''})[rule.op]||'')+' '+thresholds.map(loomReportNumber).map(esc).join(' – ')+' '+esc(rule.unit)+'</p>';
 return loomResearchSection('Case measurements',selector+chart+criterion+loomReportPageControls(page,report,field,points.length,page.value.length));
}
function loomReportDistributionChart(rows,f){
 if(!rows.length)return '<p class="pa-report-missing">Missing</p>';
 if(f.format==='coverage_matrix'){
  const columns=[...new Set(rows.map(cell=>cell.column))],labels=[...new Set(rows.map(cell=>cell.row))];
  const cells=new Map(rows.map(cell=>[JSON.stringify([cell.row,cell.column]),cell]));
  if(columns.length>12||labels.length>30)return catTable(['Condition','Group','Admitted','Assigned'],rows.map(cell=>[esc(cell.row),esc(cell.column),esc(cell.admitted),esc(cell.assigned)]));
  return catTable([f.y_label||'Condition',...columns],labels.map(label=>[esc(label),...columns.map(column=>{
   const c=cells.get(JSON.stringify([label,column]));
   return c?esc(c.admitted+' / '+c.assigned):'—';
  })]))+'<p class="pa-report-note">Admitted / assigned · — indicates no recorded cell</p>';
 }
 const data=rows.map(row=>({label:loomReportPointer(row,f.x_pointer),value:loomReportPointer(row,f.y_pointer)}));
 const maximum=Math.max(...data.map(row=>row.value),1);
 return '<div class="pa-report-bars">'+data.map(row=>'<div><span>'+esc(row.label)+'</span><progress max="'+maximum+'" value="'+row.value+'" aria-label="'+esc(row.label)+'"></progress><strong>'+esc(loomReportNumber(row.value))+(f.unit?' '+esc(f.unit):'')+'</strong></div>').join('')+'</div>';
}
function loomReportField(e,r,f){
 if(f.state==='missing')return '<p class="pa-report-missing">Missing</p>';
 if(f.format==='properties')return catProperties(Object.fromEntries(Object.entries(f.value||{}).map(([k,v])=>[k,loomReportCell(v)])));
 if(!['table','series','scatter','instructions','bar','coverage_matrix'].includes(f.format))return '<dl class="pa-entity-facts"><div><dt>'+esc(f.label)+'</dt><dd>'+esc(loomReportCell(f.value))+(f.unit?' '+esc(f.unit):'')+'</dd></div></dl>';
 const key=loomReportKey(e,r,f),page=loomReportPages.get(key);if(!page){loomReportLoad(e,r,f);return '<p class="pa-report-missing">Loading…</p>';}
 if(page.error)return '<p class="pa-catalog-warning">'+esc(page.error)+' <button class="pa-button" data-report-retry="'+esc(r.requirement_id)+'" data-report-field="'+esc(f.id)+'">Retry</button></p>';
 const state=loomReportState(e),all=Array.isArray(page.value)?page.value:[];
 const bySeries=f.format==='series';
 const rows=bySeries?all.filter(row=>(state.phase==='all'||row.phase==null||String(row.phase)===state.phase)&&(state.split==='all'||row.split==null||row.split===state.split)):r.role==='evaluation_cases'&&f.format==='table'?loomCheckpointRows(e,all):all;
 if((state.tab==='episodes'||state.tab==='cases')&&f.format==='table')return loomReportMember(e,r,f,rows,page);
 const controls=loomReportPageControls(page,r,f,rows.length,all.length);
 if(['bar','coverage_matrix'].includes(f.format))return loomReportDistributionChart(rows,f)+controls;
 let body='';
 if(f.format==='instructions'&&!rows.length)return '<p class="pa-report-missing">'+((r.fields||[]).some(f=>f.id==='mode'&&f.value==='none')?'Not applicable':'Missing')+'</p>';
 if(f.format==='instructions')body=rows.map(row=>'<article class="pa-report-instruction"><h3>'+esc(row.phase!=null?'Subtask '+row.phase:row.id)+'</h3><blockquote>'+esc(row.text)+'</blockquote></article>').join('');
 else {
  const columns=f.columns?.length?f.columns:(f.format==='series'?[{label:'Step',pointer:f.x_pointer},{label:'Value',pointer:f.y_pointer}] : [{label:'Record',pointer:'/id'}]);
  const actualColumns=bySeries?[...columns,{label:'Subtask',pointer:'/phase'},{label:'Split',pointer:'/split'}]:columns;
  const table=catTable(actualColumns.map(c=>c.label),rows.map(row=>actualColumns.map(c=>esc(loomReportCell(loomReportPointer(row,c.pointer)))+(c.unit?' '+esc(c.unit):''))));
  const selectedStep=bySeries?r.fields?.find(field=>field.id==='selected_step')?.value:undefined;
  const selection=r.fields?.find(field=>field.id==='selection')?.value;
  const members=f.format==='scatter'&&r.role==='sdg_conditions'?loomReportConditionMembers(e):null;
  body=(bySeries||f.format==='scatter')?loomReportChart(rows,f,f.format,selectedStep,Number(state.step),members)+'<details><summary>Recorded values</summary>'+table+'</details>':table;
  if(Number.isFinite(selectedStep))body+='<p class="pa-report-selection">'+(selection==='best_validation'?'Best':'Selected')+' checkpoint · step '+esc(loomReportNumber(selectedStep))+'</p>';
  if(bySeries&&state.step&&state.step!=='all'){
   const observations=rows.filter(row=>String(loomReportPointer(row,f.x_pointer))===state.step);
   if(observations.length)body+=catTable(actualColumns.map(c=>c.label),observations.map(row=>actualColumns.map(c=>esc(loomReportCell(loomReportPointer(row,c.pointer)))+(c.unit?' '+esc(c.unit):''))));
  }
 }
 return body+controls;
}
function loomReportContract(e){
 const v=e.reporting;if(!v?.contract_ref)return '<p class="pa-report-missing">Reporting contract · Missing</p>';
 const rows=(v.reports||[]).map(r=>[esc(r.label),esc(r.role),r.required?'Required':'Optional',r.state==='recorded'?'Recorded':'Missing']);
 return catProperties({Contract:v.contract_ref.id,Revision:v.contract_ref.revision,SHA256:v.contract_ref.sha256,Activity:v.activity})+catTable(['Report','Role','Coverage','State'],rows)+(v.adaptations?.length?'<details><summary>Excluded / replaced requirements</summary>'+catTable(['Requirement','Disposition','Reason','Analysis impact'],v.adaptations.map(a=>[esc(a.requirement_id),esc(a.disposition),esc(a.reason),esc(a.analysis_impact)]))+'<pre>'+esc(JSON.stringify(v.adaptations,null,2))+'</pre></details>':'');
}
function loomReportSection(e,report,fields){
 if(report.state!=='recorded')return loomResearchSection(report.label,'<p class="pa-report-missing">Missing</p>');
 if(!fields.length)return '';
 return loomResearchSection(report.label,fields.map(f=>{
  const needsHeading=['table','series','scatter','instructions','bar','coverage_matrix','properties'].includes(f.format)&&f.label!==report.label;
  return '<div class="pa-report-field">'+(needsHeading?'<h3>'+esc(f.label)+'</h3>':'')+loomReportField(e,report,f)+'</div>';
 }).join(''));
}
function loomPolicyLearningSection(e,report,fields){
 const charts=fields.filter(f=>f.format==='series');
 if(report.state!=='recorded'||!charts.length)return loomReportSection(e,report,fields);
 const details=fields.filter(f=>f.format!=='series');
 const body=charts.map((f,i)=>'<div class="pa-report-field">'+(i?'<h3>'+esc(f.label)+'</h3>':'')+loomReportField(e,report,f)+'</div>').join('');
 const metadata=details.map(f=>loomReportField(e,report,f)).join('');
 return loomResearchSection(charts[0].label,body+(metadata?'<details class="pa-report-metric-details"><summary>Metric details</summary>'+metadata+'</details>':''));
}
function loomReportCatalogInput(input,label){
 const text=esc(label||input.label);
 if(input.item_id&&input.revision_id){
  const id='revision:'+input.item_id+':'+input.revision_id;
  return '<a class="pa-table-link" href="'+esc(paiEntityURL(id))+'" data-entity="'+esc(id)+'">'+text+'</a>';
 }
 return '<button type="button" class="pa-table-link" data-catalog-input-ref="'+esc(JSON.stringify(input.ref))+'">'+text+'</button>';
}
function loomReportLineage(e){
 if(!e.catalog_inputs||!['model','experiment','run'].includes(e.kind))return '';
 const fields=e.kind==='model'?[['datasets','Training dataset']]:[['datasets','Evaluation dataset'],['policies','Evaluated policy']];
 const rows=fields.map(([field,label])=>{
  const inputs=loomCatalogInputs(e,field);
  return [esc(label),inputs.length?inputs.map(input=>{
   const tag=field==='datasets'?loomDatasetSplitTag(input.split):null;
   return '<div class="pa-catalog-inputs">'+loomReportCatalogInput(input,field==='datasets'?loomInputLabel(input):input.label)+(tag?loomSplitTagHTML(tag):'')+'</div>';
  }).join(''):'Missing'];
 });
 const previous=e.kind==='model'&&(e.relations||[]).find(relation=>relation.kind==='revises');
 if(previous)rows.push(['Previous revision',loomReportCatalogInput({ref:{kind:'entity',identity:{id:previous.id,revision:previous.revision}}},'r'+previous.revision)]);
 return loomResearchSection('Catalog inputs',catTable(['Input','Record'],rows));
}
function loomReportOverview(e){
 const r=e.reporting,reports=r?.reports||[],parts=[loomReportLineage(e),loomReportOverviewBodies.get(e.id)||loomResearchOverviewContent(e)];
 if(r?.checkpoint_sweep)parts.unshift(loomCheckpointOverview(r.checkpoint_sweep));
 else if(['experiment','run'].includes(e.kind)){
  const outcomes=loomRepresentativeOutcome(e);
  if(outcomes.length)parts.unshift(loomResearchSection('Outcome',catTable(['Metric','Value','Population'],outcomes.map(m=>[esc(m.label),resultOutcomeValue(m),esc(m.population||m.denominator_label||'—')]))));
 }
 for(const [alias,claim] of Object.entries(e.science?.deliverables||{})){
  const sections=[];
  if(claim.sessions?.length){
   const rows=claim.sessions.map(session=>{
    const configuration=session.configuration;
    const exact='<button class="pa-button" data-report-input="'+esc(configuration.id)+'" data-report-input-revision="'+esc(configuration.revision)+'">'+esc(catEntity(configuration.id)?.label||configuration.id)+' · r'+esc(configuration.revision)+'</button>';
    const readings=catTable(['Field','Measured'],Object.entries(session.readback).map(([field,value])=>[esc(field),esc(loomReportCell(value))]));
    return [esc(session.id),esc(session.domain==='real'?'Real':'Simulation'),exact,esc(session.member_count),'<details><summary>Readback</summary>'+readings+'</details>'];
   });
   sections.push(catTable(['Session','Domain','Setup','Members','Readback'],rows));
  }
  if(claim.hypothesis)sections.push(catProperties({Hypothesis:claim.hypothesis.statement,Population:claim.hypothesis.population,
   Metric:claim.hypothesis.metric,Analysis:claim.hypothesis.analysis,Result:claim.analysis?.summary||'Unrecorded'}));
  if(sections.length)parts.push(loomResearchSection('Experimental evidence · '+esc(alias),sections.join('')));
 }
 if(e.supplement?.base){
  const base=e.supplement.base,job=e.producer?.job_id;
  parts.push(loomResearchSection('Evaluation history',catTable(['Revision','Publication','Job'],[
   [esc(loomReportIdentity(e).revision),'Checkpoint supplement',job?catLink(job):'—'],
   [esc(base.revision),'<button class="pa-button" data-report-input="'+esc(base.id)+'" data-report-input-revision="'+esc(base.revision)+'" data-report-exact>Previous revision</button>','—']
  ])));
 }
 const otherInputs=(r?.inputs||[]).filter(i=>!['datasets','policies'].some(field=>loomCatalogInputs(e,field).some(({ref})=>ref.kind==='entity'&&ref.identity.id===i.id&&ref.identity.revision===i.revision)));
 if(otherInputs.length)parts.push(loomResearchSection('Accepted inputs',catTable(['Input','Record','Revision'],otherInputs.map(i=>[esc(i.input),'<button class="pa-button" data-report-input="'+esc(i.id)+'" data-report-input-revision="'+esc(i.revision)+'">'+esc(catEntity(i.id)?.label||i.id)+'</button>',esc(i.revision)]))));
 for(const report of reports){
  const fields=report.fields||[];
  if(report.role==='policy_inputs'){
   const facts=loomResearchFactMap(e),equivalent={step:'selected_step',robot_dof:'robot_dof',action_dimension:'action_dimension',chunk:'action_chunk',state_dimension:'state_dimension',controlled_dof:'controlled_dof'};
   parts.push(loomReportSection(e,report,fields.filter(f=>f.format!=='table'&&(!Object.hasOwn(equivalent,f.id)||facts.get(equivalent[f.id])?.value!==f.value))));
  }
  if(report.role==='instructions')parts.push(loomReportSection(e,report,fields.filter(f=>f.format==='instructions')));
  if(report.role==='dataset_statistics'){
   const checks=fields.filter(f=>!['count','splits','members'].includes(f.id));
   if(checks.length)parts.push(loomReportSection(e,{...report,label:'Collection checks'},checks));
  }
  if(report.role==='evaluation_cases'&&report.state==='recorded')parts.push(loomResearchSection('Protocol','<button class="pa-button" data-report-protocol="'+esc(report.requirement_id)+'">Protocol definition</button><div data-report-protocol-detail></div>'));
 }
 if(r?.exclusions?.some(x=>x.role==='instructions'))parts.push(loomResearchSection('Instructions','<p class="pa-report-missing">Not applicable</p>'));
 const template=e.metadata_status?.template_ref||e.metadata?.template_ref,contract=r?.contract_ref;
 const footer='<details class="pa-report-contract"><summary>Template · '+esc(template?template.id+' · r'+template.revision:'Unrecorded')+' / Contract · '+esc(contract?contract.id+' · r'+contract.revision:'Unrecorded')+'</summary>'+loomReportContract(e)+'</details>';
 return loomResearchOverviewCard(parts.join(''))+footer;
}
function loomReportTrainingFilters(e,reports){
 const state=loomReportState(e);
 const series=reports.flatMap(r=>(r.fields||[]).filter(f=>f.format==='series').map(f=>({f,rows:loomReportPages.get(loomReportKey(e,r,f))?.value||[]})));
 const dimensions=[['phase','Subtask','All subtasks',series.flatMap(({rows})=>rows.map(row=>row.phase))],['split','Split','All splits',series.flatMap(({rows})=>rows.map(row=>row.split))],['step','Step','All steps',series.flatMap(({f,rows})=>rows.map(row=>loomReportPointer(row,f.x_pointer)))]];
 const filters=dimensions.map(([key,label,all,values])=>{
  const options=[...new Set(values.filter(v=>v!=null).map(String))].filter(v=>v!=='all');
  if(options.length===0)return '';
  if(key==='step')options.sort((a,b)=>Number(a)-Number(b));
  return '<label>'+label+'<select data-report-filter="'+key+'">'+['all',...options].map(v=>'<option value="'+esc(v)+'"'+(state[key]===v?' selected':'')+'>'+esc(v==='all'?all:key==='phase'?'Subtask '+v:v)+'</option>').join('')+'</select></label>';
 }).join('');
 return filters?'<div class="pa-report-controls">'+filters+'</div>':'';
}
function loomReportContent(e){
 const {tab}=loomReportState(e),r=e.reporting;
 if(tab==='overview')return '<div class="pa-report-overview">'+loomReportOverview(e)+'</div>';
 const roles={training:['learning_series','action_metrics'],results:['evaluation_cases','evaluation_metrics'],cases:['evaluation_cases'],episodes:['dataset_statistics'],distribution:['sdg_conditions','sdg_readback','distribution']};
 const reports=(r?.reports||[]).filter(report=>roles[tab]?.includes(report.role));
 const legacy=['training','results'].includes(tab)?loomReportLegacyMetrics.get(e.id)||'':'';
 const sweep=r?.checkpoint_sweep,summary=tab==='results'&&!sweep?r?.evaluation_summary:null;
 const failures=Object.entries(summary?.recorded_failure_subtasks||{});
 const failureBreakdown=failures.length?loomResearchSection('Recorded failure subtasks',catTable(['Subtask','Valid cases with recorded failure'],failures.map(([name,count])=>[esc(name),esc(count)]))):'';
 const sections=reports.map(report=>{
  let fields=report.fields||[];
  if(tab==='episodes')fields=fields.filter(f=>f.id==='members');
  if(tab==='cases')fields=fields.filter(f=>f.id==='cases'||f.format==='table');
  if(tab==='results'&&report.role==='evaluation_cases')fields=sweep?[]:fields.filter(f=>f.format!=='table');
  if(tab==='training'&&report.role==='learning_series')return loomPolicyLearningSection(e,report,fields.filter(f=>f.id!=='selected_step'));
  return loomReportSection(e,report,fields);
 }).join('');
 const checkpoints=tab==='results'&&sweep?loomCheckpointOverview(sweep,true):'';
 return (tab==='training'?loomReportTrainingFilters(e,reports):'')+checkpoints+(['results','cases'].includes(tab)?loomCheckpointPicker(e):'')+sections+failureBreakdown+(tab==='results'?loomReportCaseGraph(e):'')+(legacy?'<details class="pa-report-legacy"'+(sections&&tab==='training'?'':' open')+'><summary>Recorded metrics</summary>'+legacy+'</details>':'')+(!sections&&!legacy&&!checkpoints?'<p class="pa-report-missing">Missing</p>':'');
}
const loomReportPreviousDetail=catalogDetail;
catalogDetail=function(id){
 const e=catEntity(id),html=loomReportPreviousDetail(id);if(!e||!['dataset','model','experiment','run'].includes(e.kind)||!catalogData.capabilities?.reporting)return html;
 const t=document.createElement('template');t.innerHTML=html;
 const role=e.kind==='model'?modelFilterMeta(e).role:null;
 const policy=role&&role!=='Observer'&&(e.reporting||['VLA','Policy'].includes(role));
 if(policy){const label=t.content.querySelector('.pa-heading .pa-eyebrow');if(label)label.textContent='POLICY';}
 const overview=t.content.querySelector('.pa-research-overview');if(!overview)return t.innerHTML;
 if(policy){
  const role=overview.querySelector('[data-research-fact=model_role]');
  if(role?.querySelector('dd')?.textContent.toLowerCase()==='policy')role.remove();
 }
 const metrics=t.content.querySelector('[data-output-metrics]'),toolbar=t.content.querySelector('.pa-metrics-toolbar');
 const legacyResults=!e.metrics?.length&&!e.reporting?.checkpoint_sweep&&['experiment','run'].includes(e.kind)?catResultMetrics(e):[];
 const recorded=legacyResults.length?catTable(['Metric','Value','Population'],legacyResults.map(m=>[esc(m.label),resultOutcomeValue(m),esc(m.population||m.denominator_label||'—')])):'';
 loomReportLegacyMetrics.set(e.id,e.metrics?.length?(toolbar?.outerHTML||'')+(metrics?.outerHTML||''):recorded);
 metrics?.remove();toolbar?.remove();
 loomReportOverviewBodies.set(e.id,overview.innerHTML);while(loomReportOverviewBodies.size>160){const id=loomReportOverviewBodies.keys().next().value;loomReportOverviewBodies.delete(id);loomReportLegacyMetrics.delete(id);}
 const view=loomReportState(e);const parent=document.createElement('section');parent.className='pa-report-workspace'+(policy?' pa-report-policy':e.kind==='dataset'?' pa-report-dataset':e.kind==='model'?'':' pa-report-experiment');parent.dataset.reportOutput=e.id;
 parent.innerHTML='<nav class="pa-report-tabs" aria-label="Output details">'+loomReportTabs(e).map(([id,label])=>'<button data-report-tab="'+id+'" aria-current="'+(view.tab===id?'page':'false')+'">'+label+'</button>').join('')+'</nav><div data-report-content>'+loomReportContent(e)+'</div>';
 overview.replaceWith(parent);return t.innerHTML;
};
root.addEventListener('click',async event=>{
 const workspace=event.target.closest('[data-report-output]'),e=workspace&&catEntity(workspace.dataset.reportOutput);if(!e)return;
 const point=event.target.closest('[data-report-case],[data-report-episode]');if(point){event.preventDefault();Object.assign(loomReportState(e),{tab:point.dataset.reportCase?'cases':'episodes',member:point.dataset.reportCase||point.dataset.reportEpisode});render();return;}
 const input=event.target.closest('[data-report-input]');if(input){try{const pin=await loomSearchAction('results','resolve_item',{native_ref:{kind:'entity',identity:{id:input.dataset.reportInput,revision:Number(input.dataset.reportInputRevision)}}});if(input.hasAttribute('data-report-exact')){await loomCatalogOpen('revision:'+pin.item_id+':'+pin.revision_id);return;}const value=await loomSearchAction('system','dashboard_entity',{item_id:pin.item_id,revision_id:pin.revision_id});loomCatalogMerge(value);if(value.entities?.[0])openView(catView(value.entities[0].id));}catch(error){input.textContent=error.message;}return;}
 const lineage=event.target.closest('[data-catalog-input-ref]');
 if(lineage){
  try{
   const pin=await loomSearchAction('results','resolve_item',{native_ref:JSON.parse(lineage.dataset.catalogInputRef)});
   await loomCatalogOpen('revision:'+pin.item_id+':'+pin.revision_id);
  }catch(error){lineage.textContent=error.message;}
  return;
 }
 const tab=event.target.closest('[data-report-tab]');if(tab){loomReportState(e).tab=tab.dataset.reportTab;render();return;}
 const paging=event.target.closest('[data-report-more],[data-report-retry]');
 if(paging){
  const r=e.reporting?.reports?.find(r=>r.requirement_id===(paging.dataset.reportMore||paging.dataset.reportRetry)),f=r?.fields?.find(f=>f.id===paging.dataset.reportField);
  if(!f)return;
  const retry=paging.hasAttribute('data-report-retry');
  if(retry)loomReportPages.delete(loomReportKey(e,r,f));
  loomReportLoad(e,r,f,!retry);return;
 }
 const protocol=event.target.closest('[data-report-protocol]');if(protocol){
  protocol.disabled=true;const target=workspace.querySelector('[data-report-protocol-detail]');
  try{const page=await loomSearchAction('results','read_report',{...loomReportIdentity(e),requirement_id:protocol.dataset.reportProtocol,pointer:'/protocol_ref'});const p=await loomSearchAction('results','get_report_definition',{id:page.value.id,revision:page.value.revision});if(target.isConnected)target.innerHTML=catProperties({Protocol:p.label,Mode:p.definition.mode,Environment:p.definition.environment,Controller:p.definition.controller,Evaluator:p.definition.scope?.evaluator?.kind,'Policy GT':p.definition.scope?.gt_access?.policy,'Planner GT':p.definition.scope?.gt_access?.planner,'Scoring GT':p.definition.scope?.gt_access?.scoring,'Policy active during hold':p.definition.scope?.policy_active_during_hold,'Goal frame':p.definition.scope?.goal_frame})+catTable(['Measurement','Condition','Threshold','Unit'],(p.definition.predicate.all||[]).map(rule=>[esc(rule.pointer),esc(rule.op),esc(rule.op==='between'?rule.min+' – '+rule.max:rule.value),esc(rule.unit)]));}
  catch(error){if(target.isConnected)target.textContent=error.message;}finally{if(protocol.isConnected)protocol.disabled=false;}
 }
});
root.addEventListener('change',event=>{const workspace=event.target.closest('[data-report-output]'),field=event.target.dataset.reportFilter;if(!workspace||!field)return;const e=catEntity(workspace.dataset.reportOutput),view=loomReportState(e);view[field]=event.target.value;if(field==='checkpoint')view.member=null;render();});

root.addEventListener('click',event=>{if(event.target.closest('[data-report-contracts]'))loomRegistryBrowser('report_contract');});
