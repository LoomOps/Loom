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
function loomDatasetImported(e){return e.reporting?.activity==='dataset_import'||e.origin?.type==='import';}
function loomDatasetOriginBadge(e){return loomDatasetImported(e)?'<span class="pa-dataset-origin">Imported</span>':'';}
function loomDatasetRow(e,depth,children,expanded,parent,showTask=false){
 const links=e.dataset_lineage?.parents||[],link=parent?links.find(p=>{const exact=loomDatasetParentEntity(p);return exact&&loomDatasetKey(exact)===loomDatasetKey(parent);}):(links.length===1?links[0]:links.find(p=>p.primary));
 const facts=loomResearchCardFacts(e),ready=facts.find(f=>f.id==='bc_ready');
 const stateLabel=typeof ready?.value==='boolean'?'<span class="pa-dataset-bc '+(ready.value?'is-ready':'is-not-ready')+'">BC '+(ready.value?'ready':'not ready')+'</span>':'';
 const mainFacts=facts.filter(f=>['count','frames','inputs','split','train_snapshots','validation_snapshots','physics','dynamics'].includes(f.id));
 const ancestry=link?'<small class="pa-dataset-relation">'+esc(loomDatasetRelations[link.transformation?.kind]||'Derived')+(parent?'':' · '+esc(loomDatasetParentEntity(link)?.label||link.label||link.parent?.identity?.id||''))+'</small>':'';
 return '<article role="listitem" class="pa-catalog-item pa-scientific-card pa-dataset-row" data-output-id="'+esc(e.id)+'" data-dataset-depth="'+depth+'" style="--dataset-indent:'+Math.min(depth,4)*14+'px">'
  +(showTask?loomTaskProperty(e):'')+'<div class="pa-row-main pa-dataset-name">'+(children?'<button class="pa-dataset-expander" data-dataset-node="'+esc(loomDatasetKey(e))+'" aria-expanded="'+expanded+'" aria-label="'+esc((expanded?'Collapse ':'Expand ')+e.label)+'">'+icon(expanded?'chevron-down':'chevron-right')+'</button>':'')
  +'<div>'+catLink(e.id)+loomDatasetOriginBadge(e)+ancestry+stateLabel+'</div></div>'
  +(mainFacts.length?loomResearchFactsHTML(mainFacts,true):'')+'</article>';
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
   const next=children.get(key)||[],expanded=!closed.has(key);body+=loomDatasetRow(e,depth,next.length,expanded,parent);
   if(expanded)for(const child of [...next].reverse())stack.push({e:child,depth:depth+1,parent:e});
   else{const hidden=[...next];while(hidden.length){const c=hidden.pop(),k=loomDatasetKey(c);if(seen.has(k))continue;seen.add(k);hidden.push(...(children.get(k)||[]));}}
  }
 }
 for(const e of roots)append(e);
 // Do not hide a corrupt legacy cycle. Native declarations reject cycles.
 for(const e of rows)if(!seen.has(loomDatasetKey(e)))append(e);
 return '<div class="pa-dataset-tree" role="list">'+body+'</div>';
}
function loomUniqueDatasets(rows){
 const unique=new Map();for(const e of rows){const key=loomDatasetKey(e),old=unique.get(key);if(!old||!old.dataset_organization&&e.dataset_organization)unique.set(key,e);}
 return [...unique.values()];
}
function loomDatasetGroups(rows){
 const families=new Map();
 for(const e of rows){
  const context=loomDatasetContext(e),key=context.task?.id||'unassigned';
  if(!families.has(key))families.set(key,{key,label:context.task?.label?.split('·')[0].trim()||'Other',rows:[],children:new Map()});
  const family=families.get(key);family.rows.push(e);let group=family,prefix=[];
  for(const node of context.path){prefix.push(node.id);const k=JSON.stringify(prefix);if(!group.children.has(node.id))group.children.set(node.id,{key:'subgroup:'+key+':'+k,label:node.label,rows:[],children:new Map()});group=group.children.get(node.id);}
  if(!group.direct)group.direct=[];group.direct.push(e);
 }
 function subgroup(group){
  const content=(group.direct?.length?loomDatasetTree(group.direct):'')+[...group.children.values()].map(subgroup).join('');
  return '<details class="pa-dataset-subgroup" data-dataset-group="'+esc(group.key)+'" '+(loomDatasetGroupOpen(state,group.key)?'open':'')+'><summary data-dataset-group-toggle="'+esc(group.key)+'">'+esc(group.label)+'</summary>'+content+'</details>';
 }
 const contents=family=>(family.direct?.length?loomDatasetTree(family.direct):'')+[...family.children.values()].map(subgroup).join('');
 if(families.size===1&&families.has('unassigned'))return contents(families.get('unassigned'));
 return [...families.values()].sort((a,b)=>Number(a.key==='unassigned')-Number(b.key==='unassigned')||a.label.localeCompare(b.label)).map(f=>{
  const key='task:'+f.key;
  return '<section class="pa-dataset-family"><details data-dataset-family="'+esc(f.key)+'" '+(loomDatasetGroupOpen(state,key)?'open':'')+'><summary data-dataset-group-toggle="'+esc(key)+'"><h2>'+esc(f.label)+'</h2><span>'+f.rows.length+'</span></summary>'+contents(f)+'</details></section>';
 }).join('');
}
const loomDatasetFilterRoles={training_observations:'Training observations',diagnostic_observations:'Diagnostic observations',evaluation_conditions:'Evaluation conditions'};
const loomDatasetFilterEmpty={role:'',input:'',split:'',origin:'',query:''};
function loomDatasetFilterMeta(e){
 const inputs=loomResearchInputs(e).filter(x=>x.status==='present'&&!['instruction','camera_intrinsics','camera_transform','depth_validity'].includes(x.kind));
 const split=loomResearchFactMap(e).get('split'),value=split?.status==='recorded'?split.value:null;
 const groups=Array.isArray(value)?value.filter(x=>x&&typeof x==='object').map(x=>[x.id,x]):Object.entries(value&&typeof value==='object'?value:{});
 const splits=groups.filter(([id,x])=>typeof id==='string'&&x!=null&&!['not_applicable','unrecorded','not_recorded'].includes(typeof x==='string'?x:x.status))
  .map(([id])=>id==='val'?'validation':id);
 const role=e.dataset_overview?.role;
 return {role:Object.hasOwn(loomDatasetFilterRoles,role)?loomDatasetFilterRoles[role]:'미분류',
  inputs:inputs.flatMap(x=>loomResearchInputName(x).split(' · ')),splits,origin:loomDatasetImported(e)?'Imported':e.reporting?.activity==='sdg'?'Generated':'Unrecorded'};
}
function loomDatasetList(){
 const rows=loomUniqueDatasets(catKind('dataset')),f={...loomDatasetFilterEmpty,...state.datasetFilters};
 const candidates=rows.map(e=>({e,...loomDatasetFilterMeta(e)})),query=f.query.trim().toLowerCase();
 const filtered=Boolean(f.role||f.input||f.split||f.origin||query),changed=Object.values(f).some(Boolean);
 const shown=candidates.filter(x=>(!f.role||x.role===f.role)&&(!f.input||x.inputs.includes(f.input))&&(!f.split||x.splits.includes(f.split))&&(!f.origin||x.origin===f.origin)&&(!query||catSearchText(x.e).includes(query))).map(x=>x.e);
 const roles=[...new Set([...candidates.map(x=>x.role),f.role].filter(Boolean))].sort();
 const buttons=roles.length>1||f.role?[['','All',rows.length],...roles.map(role=>[role,role,candidates.filter(x=>x.role===role).length])]:[];
 const pills=buttons.map(([value,label,n])=>`<button class="pa-pill ${f.role===value?'active':''}" data-dataset-role="${esc(value)}" aria-pressed="${f.role===value}">${esc(label)}<span>${n}</span></button>`).join('');
 const select=(key,label,entries)=>modelFilterSelect(key,label,entries,f[key],'data-dataset-filter');
 const filters=`<section class="pa-model-filters" aria-label="데이터셋 필터"><div class="pa-model-filter-top"><div class="pa-model-role-group" role="group" aria-label="데이터셋 종류">${pills}</div><button class="pa-model-filter-reset" data-dataset-reset${changed?'':' disabled'}>초기화</button></div><div class="pa-model-filter-fields pa-dataset-filter-fields">${select('input','입력',candidates.flatMap(x=>x.inputs))}${select('split','Split',candidates.flatMap(x=>x.splits))}${select('origin','Origin',candidates.map(x=>x.origin))}<label class="pa-model-filter-field pa-model-filter-search"><span>검색</span><input type="search" data-dataset-filter="query" aria-label="데이터셋 검색" placeholder="이름·키워드" value="${esc(f.query)}" autocomplete="off"></label></div></section>`;
 // A focused search shows matches even when a lineage branch was collapsed.
 const result=filtered?'<div class="pa-dataset-tree" role="list">'+shown.map(e=>loomDatasetRow(e,0,0,false,null,true)).join('')+'</div>':loomDatasetGroups(shown);
 const empty='<div class="pa-empty">'+(filtered?'조건에 맞는 데이터셋이 없습니다.':'표시할 데이터셋이 없습니다.')+'</div>';
 return heading('CATALOG','Datasets','')+catStatus()+filters
  +panel(`<span data-dataset-result-count>${shown.length} / ${rows.length}개 데이터셋</span>`,result&&shown.length?result:empty)
  +loomCatalogPager('dataset');
}
const loomDatasetPriorList=catalogList;
catalogList=function(kind){return kind==='dataset'?loomDatasetList():loomDatasetPriorList(kind);};
function updateDatasetFilters(target,values,reset=false){
 const pane=ws.panes.find(p=>p.id===target.closest('[data-pane]')?.dataset.pane);if(!pane)return;
 ws.active=pane.id;
 const view=activeTab(pane).view;
 view.datasetFilters=reset?{...loomDatasetFilterEmpty}:{...loomDatasetFilterEmpty,...view.datasetFilters,...values};
 if(reset)render();else refreshCatalogPanes();
 persist();
}
root.addEventListener('click',event=>{const b=event.target.closest('[data-dataset-role],[data-dataset-reset]');if(b)updateDatasetFilters(b,{role:b.dataset.datasetRole},b.hasAttribute('data-dataset-reset'));});
root.addEventListener('change',event=>{const f=event.target.closest('select[data-dataset-filter]');if(f)updateDatasetFilters(f,{[f.dataset.datasetFilter]:f.value});});
root.addEventListener('input',event=>{const f=event.target.closest('input[data-dataset-filter]');if(f&&!event.isComposing)updateDatasetFilters(f,{query:f.value});});
root.addEventListener('click',event=>{
 const summary=event.target.closest('[data-dataset-group-toggle]'),button=event.target.closest('[data-dataset-node]');if(!summary&&!button)return;
 event.preventDefault();event.stopImmediatePropagation();const pane=(summary||button).closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;
 const view=activeTab().view;
 if(summary){const opening=!summary.parentElement.open,closed=new Set(view.closedDatasetGroups||[]);if(opening)closed.delete(summary.dataset.datasetGroupToggle);else closed.add(summary.dataset.datasetGroupToggle);view.closedDatasetGroups=[...closed];if(summary.dataset.datasetGroupToggle==='task:unassigned')view.openOtherDatasets=opening;render();}
 else{const closed=new Set(view.closedDatasetNodes||[]);closed.has(button.dataset.datasetNode)?closed.delete(button.dataset.datasetNode):closed.add(button.dataset.datasetNode);view.closedDatasetNodes=[...closed];render();}
 persist();
},true);
