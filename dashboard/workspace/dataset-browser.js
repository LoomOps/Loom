// Task context / subgroup taxonomy / data lineage are independent dimensions.
// This renderer consumes declared organization; it never classifies by names.
const loomDatasetRelations={subset:'Subset',condition_selection:'Conditions',includes:'Partial reuse',relabel:'Relabel',resample:'Resample',sensor_rerender:'Sensor re-render',augment:'Augment',merge:'Merge'};
function loomDatasetKey(e){return e.dataset_mirror_key||e.id;}
function loomDatasetImported(e){return e.reporting?.activity==='dataset_import'||e.origin?.type==='import';}
function loomDatasetOriginBadge(e){return loomDatasetImported(e)?'<span class="pa-dataset-origin">Imported</span>':'';}
function loomDatasetRow(e,depth=0){
 const facts=loomResearchCardFacts(e),ready=facts.find(f=>f.id==='bc_ready');
 const stateLabel=typeof ready?.value==='boolean'?'<span class="pa-dataset-bc '+(ready.value?'is-ready':'is-not-ready')+'">BC '+(ready.value?'ready':'not ready')+'</span>':'';
 const mainFacts=facts.filter(f=>['count','frames','inputs','split','train_snapshots','validation_snapshots','physics','dynamics'].includes(f.id));
 const ancestry=(e.dataset_lineage?.parents||[]).map(link=>'<small class="pa-dataset-relation">'+esc(loomDatasetRelations[link.transformation?.kind]||'Derived')+' from '+esc(loomDatasetParentEntity(link)?.label||link.label||link.parent?.identity?.id||'')+'</small>').join('');
 return '<article role="listitem" class="pa-catalog-item pa-scientific-card pa-dataset-row" data-output-id="'+esc(e.id)+'" data-dataset-depth="'+depth+'">'
  +'<div class="pa-row-main pa-dataset-name"><div><div class="pa-dataset-title-line">'+catLink(e.id)+loomDatasetOriginBadge(e)+stateLabel+'</div>'+ancestry+'</div></div>'
  +(mainFacts.length?loomResearchFactsHTML(mainFacts,true):'')+'</article>';
}
function loomDatasetTree(rows){
 const canonical=new Map(rows.map(e=>[loomDatasetKey(e),e])),parents=new Set(),children=new Map(),seen=new Set(),ordered=[];
 for(const e of rows){
  const links=e.dataset_lineage?.parents||[],chosen=links.length===1?links[0]:links.find(p=>p.primary),exact=chosen&&loomDatasetParentEntity(chosen),parent=exact&&canonical.get(loomDatasetKey(exact));
  if(parent&&parent!==e){parents.add(loomDatasetKey(e));const key=loomDatasetKey(parent);if(!children.has(key))children.set(key,[]);children.get(key).push(e);}
 }
 for(const e of [...rows.filter(e=>!parents.has(loomDatasetKey(e))),...rows]){
  const stack=[{e,depth:0}];
  while(stack.length){const {e,depth}=stack.pop(),key=loomDatasetKey(e);if(seen.has(key))continue;seen.add(key);
   ordered.push(loomDatasetRow(e,depth));
   for(const child of [...(children.get(key)||[])].reverse())stack.push({e:child,depth:depth+1});
  }
 }
 return '<div class="pa-dataset-tree" role="list">'+ordered.join('')+'</div>';
}
function loomUniqueDatasets(rows){
 const unique=new Map();for(const e of rows){const key=loomDatasetKey(e),old=unique.get(key);if(!old||!old.dataset_organization&&e.dataset_organization)unique.set(key,e);}
 return [...unique.values()];
}
function loomDatasetGroups(rows){
 return loomCatalogTaskGroups(rows,loomCatalogTask,items=>{
  const groups=new Map();
  for(const e of items){
   const task=loomCatalogTask(e),organization=e.dataset_organization;
   const path=organization?.task_ref?.id===task?.id?(organization?.path||[]):[];
   const key=JSON.stringify(path.map(node=>node.id));
   if(!groups.has(key))groups.set(key,{label:path.map(node=>node.label).join(' / '),rows:[]});
   groups.get(key).rows.push(e);
  }
  return [...groups.values()].map(group=>group.label?
   '<section class="pa-dataset-subgroup"><h4>'+esc(group.label)+'</h4>'+loomDatasetTree(group.rows)+'</section>':
   loomDatasetTree(group.rows)).join('');
 });
}
function loomDatasetList(){
 const rows=loomUniqueDatasets(catKind('dataset'));
 const list=rows.length?loomDatasetGroups(rows):'<div class="pa-empty">표시할 데이터셋이 없습니다.</div>';
 return heading('CATALOG','Datasets','')+catStatus()
  +panel(`<span data-dataset-result-count>${rows.length}개 데이터셋</span>`,list+loomCatalogPager('dataset','Load more datasets'));
}
const loomDatasetPriorList=catalogList;
catalogList=function(kind){return kind==='dataset'?loomDatasetList():loomDatasetPriorList(kind);};
