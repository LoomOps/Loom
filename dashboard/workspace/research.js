// Display modes share server-computed presentation; direct evidence stays readable.
const loomResearchRender=render;
render=function(){
 loomResearchRender();
 const bar=root.querySelector('#pa-breadcrumb');
 const existing=root.querySelector('[data-research-view]');
 if(['queue','activity'].includes(state.page)){existing?.remove();return;}
 if(bar&&!existing){
  const select=document.createElement('select');select.dataset.researchView='';select.setAttribute('aria-label','Catalog view');
  for(const [value,label] of [['research','Research'],['operations','Operations'],['builtin','Built-in'],['all','All']]){
   const option=document.createElement('option');option.value=value;option.textContent=label;select.append(option);
  }
  select.value=catalogData.presentation_view||'research';bar.after(select);
 }
};
root.addEventListener('change',e=>{
 if(e.target.matches('[data-research-view]'))parent.postMessage({type:'loom-research-view',view:e.target.value},'*');
});
const loomResearchJobPanel=loomJobPanel;
loomJobPanel=function(job,entry){
 const record=entry?.data;
 let result=loomResearchJobPanel(job,entry);
 const nodes=record?.executions||[];
 if(record?.workflow&&record.result?.outputs){
  const outputs=Object.entries(record.result.outputs).flatMap(([name,ref])=>{
   const origin=record.spec?.outputs?.[name];
   const producer=nodes.find(node=>node.id===origin?.node);
   return producer&&ref.id?[[esc(name),catLink(producer.job+'.'+ref.id)]]:[];
  });
  if(outputs.length)result+=panel('Outputs',catTable(['Output','Record'],outputs));
 }
 const science=record?.spec?.outcome?.science;
 if(science){
  const commitments=Object.entries(science.deliverables).map(([alias,value])=>[esc(alias),esc(value.activity),esc(value.template_ref.id+' · r'+value.template_ref.revision),esc(value.contract_ref.id+' · r'+value.contract_ref.revision)]);
  result+=panel('Accepted reporting',catTable(['Deliverable','Activity','Template','Contract'],commitments)
   +(science.hypothesis?catProperties({Hypothesis:science.hypothesis.statement,Population:science.hypothesis.population,Metric:science.hypothesis.metric,Analysis:science.hypothesis.analysis}):''));
 }
 if(nodes.length)result=panel('Executions',catTable([record.workflow?'Node':'Job','State','Attempts','Execution'],nodes.map(n=>[
  esc(n.label),esc(n.state),esc(n.attempts.length),catLink(n.job,n.id)+loomQueueControls(n.job)
 ])))+result;
 if(record?.annotations?.length)result+=panel('Notes',catTable(['Author','Note','Evidence'],record.annotations.map(a=>[
  esc(a.author),esc(a.reason),esc(JSON.stringify(a.evidence))
 ])));
 const family=record?.family;
 if(family){
  const members=(family.members||[]).filter(m=>m.id!==job.id);
  if(family.root!==job.id)result=panel('Related Job',catLink(family.root))+result;
  if(members.length)result+=panel('Related Jobs',catTable(['Job','Parent','State','Pinned Runtime'],members.map(m=>[
   catLink(m.id,m.label||m.id),m.parent?catLink(m.parent):'—',esc(m.state),
   (m.runtime_refs||[]).map(r=>catLink('runtime:'+r.id+':'+r.revision,(r.stage?(r.stage==='runtime'?'run':r.stage)+' · ':'')+r.id+' · r'+r.revision)).join(' · ')||'—'
  ]))+(family.complete?'':'<p class="pa-subtle">Showing 200 records</p>'));
 }
 const related=(catalogData.all_entities||catalogData.entities).filter(e=>e.presentation?.parent_job===job.id&&e.kind!=='job');
 if(related.length)result+=panel('Supporting records',catTable(['Record','Kind'],related.map(e=>[catLink(e.id),esc(e.kind)])));
 return result;
};
