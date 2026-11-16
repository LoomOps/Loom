function loomDatasetGroupOpen(view,key){return key==='task:unassigned'?view.openOtherDatasets===true:!view.closedDatasetGroups?.includes(key);}
// Task context / subgroup taxonomy / data lineage are independent dimensions.
// This renderer consumes declared organization; it never classifies by names.
const loomDatasetRelations={subset:'Subset',condition_selection:'Conditions',includes:'Partial reuse',relabel:'Relabel',resample:'Resample',sensor_rerender:'Sensor re-render',augment:'Augment',merge:'Merge'};
function loomDatasetContext(e){
 const refs=e.task_refs||[],declared=e.dataset_organization;
 const task=declared?refs.find(r=>r.id===declared.task_ref?.id&&r.revision===declared.task_ref?.revision):refs.length===1?refs[0]:null;
 return {task,path:declared?.path||[]};
}
function loomDatasetKey(e){return e.dataset_mirror_key||e.id;}
function loomDatasetQuantity(e){
 const f=loomResearchFactMap(e),n=f.get('count'),unit=f.get('count_unit')?.value;
 if(n)return '<b>'+esc(loomResearchValue(n))+'</b>'+(unit?'<small>'+esc(({episodes:'Episodes',clips:'Clips',cases:'Cases',snapshots:'Snapshots'})[unit]||unit)+'</small>':'');
 const snapshots=['train_snapshots','validation_snapshots'].map(id=>f.get(id)).filter(Boolean);
 if(snapshots.length)return '<b>Snapshots</b><small>'+snapshots.map(s=>esc((s.id==='train_snapshots'?'Train ':'Val ')+loomResearchValue(s))).join(' · ')+'</small>';
 return '';
}
function loomDatasetCompactRow(e,depth,children,expanded,parent){
 const facts=loomResearchFactMap(e),split=facts.get('split'),inputs=loomResearchInputSummary(e),ready=facts.get('bc_ready'),links=e.dataset_lineage?.parents||[],link=parent?links.find(p=>{const exact=loomDatasetParentEntity(p);return exact&&loomDatasetKey(exact)===loomDatasetKey(parent);}):(links.length===1?links[0]:links.find(p=>p.primary));
 const context=loomResearchCardFacts(e).filter(f=>['physics','dynamics'].includes(f.id)).map(f=>'<small>'+esc(loomResearchValue(f))+'</small>').join('');
 const node=loomDatasetKey(e),stateLabel=ready?.status==='recorded'&&typeof ready.value==='boolean'?'<span class="pa-dataset-bc '+(ready.value?'is-ready':'is-not-ready')+'">BC '+(ready.value?'ready':'not ready')+'</span>':'';
 const ancestry=link?'<small class="pa-dataset-relation">'+esc(loomDatasetRelations[link.transformation?.kind]||'Derived')+(parent?'':' · '+esc(loomDatasetParentEntity(link)?.label||link.label||link.parent?.identity?.id||''))+'</small>':'';
 return '<div role="listitem" class="pa-dataset-row" data-output-id="'+esc(e.id)+'" data-dataset-depth="'+depth+'" style="--dataset-indent:'+Math.min(depth,5)*16+'px">'
  +'<div class="pa-dataset-name">'+(children?'<button class="pa-dataset-expander" data-dataset-node="'+esc(node)+'" aria-expanded="'+expanded+'" aria-label="'+esc((expanded?'Collapse ':'Expand ')+e.label)+'">'+icon(expanded?'chevron-down':'chevron-right')+'</button>':'<span class="pa-dataset-leaf" aria-hidden="true"></span>')
  +'<div>'+catLink(e.id)+ancestry+stateLabel+'</div></div>'
  +'<div class="pa-dataset-quantity" data-column="Records">'+loomDatasetQuantity(e)+(facts.has('frames')?'<small>'+esc(loomResearchValue(facts.get('frames')))+' frames</small>':'')+'</div>'
  +'<div class="pa-dataset-signals" data-column="Inputs">'+esc(inputs||'')+context+'</div>'
  +'<div class="pa-dataset-split" data-column="Split">'+esc(split?loomResearchValue(split):'')+'</div></div>';
}
function loomDatasetTree(rows){
 const canonical=new Map(rows.map(e=>[loomDatasetKey(e),e])),parents=new Map(),children=new Map();
 for(const e of rows){
  const links=e.dataset_lineage?.parents||[],chosen=links.length===1?links[0]:links.find(p=>p.primary),exact=chosen&&loomDatasetParentEntity(chosen),parent=exact&&canonical.get(loomDatasetKey(exact));
  if(parent&&parent!==e){parents.set(loomDatasetKey(e),parent);const key=loomDatasetKey(parent);if(!children.has(key))children.set(key,[]);children.get(key).push(e);}
 }
 const roots=rows.filter(e=>!parents.has(loomDatasetKey(e))),seen=new Set(),closed=new Set(state.closedDatasetNodes||[]);let body='';
 function append(start){
  const stack=[{e:start,depth:0,parent:null}];
  while(stack.length){const {e,depth,parent}=stack.pop(),key=loomDatasetKey(e);if(seen.has(key))continue;seen.add(key);
   const next=children.get(key)||[],expanded=!closed.has(key);body+=loomDatasetCompactRow(e,depth,next.length,expanded,parent);
   if(expanded)for(const child of [...next].reverse())stack.push({e:child,depth:depth+1,parent:e});
   else{const hidden=[...next];while(hidden.length){const c=hidden.pop(),k=loomDatasetKey(c);if(seen.has(k))continue;seen.add(k);hidden.push(...(children.get(k)||[]));}}
  }
 }
 for(const e of roots)append(e);
 // Do not hide a corrupt legacy cycle. Native declarations reject cycles.
 for(const e of rows)if(!seen.has(loomDatasetKey(e)))append(e);
 return '<div class="pa-dataset-columns" aria-hidden="true"><span>Dataset</span><span>Records</span><span>Inputs</span><span>Split</span></div><div class="pa-dataset-tree" role="list">'+body+'</div>';
}
function loomDatasetGroups(rows){
 const unique=new Map();for(const e of rows){const key=loomDatasetKey(e),old=unique.get(key);if(!old||!old.dataset_organization&&e.dataset_organization)unique.set(key,e);}
 const families=new Map();
 for(const e of unique.values()){
  const context=loomDatasetContext(e),key=context.task?.id||'unassigned';
  if(!families.has(key))families.set(key,{key,label:context.task?.label?.split('·')[0].trim()||'Other',rows:[],children:new Map()});
  const family=families.get(key);family.rows.push(e);let group=family,prefix=[];
  for(const node of context.path){prefix.push(node.id);const k=JSON.stringify(prefix);if(!group.children.has(node.id))group.children.set(node.id,{key:'subgroup:'+key+':'+k,label:node.label,rows:[],children:new Map()});group=group.children.get(node.id);}
  if(!group.direct)group.direct=[];group.direct.push(e);
 }
 const closed=new Set(state.closedDatasetGroups||[]);
 function subgroup(group,depth){
  const content=(group.direct?.length?loomDatasetTree(group.direct):'')+[...group.children.values()].map(c=>subgroup(c,depth+1)).join('');
  return '<details class="pa-dataset-subgroup" data-dataset-group="'+esc(group.key)+'" '+(closed.has(group.key)?'':'open')+'><summary data-dataset-group-toggle="'+esc(group.key)+'">'+esc(group.label)+'</summary>'+content+'</details>';
 }
 return [...families.values()].sort((a,b)=>Number(a.key==='unassigned')-Number(b.key==='unassigned')||a.label.localeCompare(b.label)).map(f=>{
  const key='task:'+f.key,open=!closed.has(key)&&f.key!=='unassigned'||(state.openOtherDatasets&&f.key==='unassigned');
  return '<section class="pa-dataset-family"><details data-dataset-family="'+esc(f.key)+'" '+(open?'open':'')+'><summary data-dataset-group-toggle="'+esc(key)+'"><h2>'+esc(f.label)+'</h2><span>'+f.rows.length+'</span></summary>'+(f.direct?.length?loomDatasetTree(f.direct):'')+[...f.children.values()].map(g=>subgroup(g,0)).join('')+'</details></section>';
 }).join('');
}
const loomDatasetPriorList=catalogList;
catalogList=function(kind){return kind==='dataset'?heading('CATALOG','Datasets','')+catStatus()+loomDatasetGroups(catKind('dataset'))+loomCatalogPager('dataset'):loomDatasetPriorList(kind);};
root.addEventListener('click',event=>{
 const summary=event.target.closest('[data-dataset-group-toggle]'),button=event.target.closest('[data-dataset-node]');if(!summary&&!button)return;
 event.preventDefault();event.stopImmediatePropagation();const pane=(summary||button).closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;
 const view=activeTab().view;
 if(summary){const opening=!summary.parentElement.open,closed=new Set(view.closedDatasetGroups||[]);if(opening)closed.delete(summary.dataset.datasetGroupToggle);else closed.add(summary.dataset.datasetGroupToggle);view.closedDatasetGroups=[...closed];if(summary.dataset.datasetGroupToggle==='task:unassigned')view.openOtherDatasets=opening;render();}
 else{const closed=new Set(view.closedDatasetNodes||[]);closed.has(button.dataset.datasetNode)?closed.delete(button.dataset.datasetNode):closed.add(button.dataset.datasetNode);view.closedDatasetNodes=[...closed];render();}
 persist();
},true);
