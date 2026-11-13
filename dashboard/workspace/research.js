// Display modes share server-computed presentation; direct evidence stays readable.
const loomResearchRender=render;
render=function(){
 loomResearchRender();
 const bar=root.querySelector('#pa-breadcrumb');
 if(bar&&!root.querySelector('[data-research-view]')){
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
 if(record?.workflow&&record.result?.outputs)result+=panel('Outputs',catTable(['Output','Record'],Object.entries(record.result.outputs).map(([name,ref])=>[esc(name),catLink(ref.job+'.'+ref.entity)])));
 if(nodes.length)result=panel('Executions',catTable(['Step','State','Attempts','Execution'],nodes.map(n=>[
  esc(n.label),esc(n.state),esc(n.attempts.length),catLink(n.job,n.id)+loomQueueControls(n.job)
 ])))+result;
 const related=(catalogData.all_entities||catalogData.entities).filter(e=>e.presentation?.parent_job===job.id&&e.kind!=='job');
 if(related.length)result+=panel('Supporting records',catTable(['Record','Kind'],related.map(e=>[catLink(e.id),esc(e.kind)])));
 return result;
};
