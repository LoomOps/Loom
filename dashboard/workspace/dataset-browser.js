// Task context / subgroup taxonomy / data lineage are independent dimensions.
// This renderer consumes declared organization; it never classifies by names.
function loomDatasetKey(e){return e.dataset_mirror_key||e.id;}
function loomDatasetOriginBadge(e){
 const label=e.reporting?.activity==='dataset_import'?'Imported':e.origin?.type==='import'
  ?(e.dataset_lineage?.parents?.some(p=>p.parent?.kind==='entity')?'Derived':'Imported'):null;
 return label?'<span class="pa-dataset-origin">'+label+'</span>':'';
}
function loomDatasetTags(organization){
 return (organization?.tags||[]).map(text=>({text,title:text}));
}
function loomDatasetTagFact(inputs){
 const tags=new Map();
 for(const input of inputs)for(const tag of loomDatasetTags(input.organization))tags.set(tag.text,tag);
 return {id:'tag',label:'Tag',value:'-',status:'recorded',tags:[...tags.values()]};
}
function loomDatasetSplitValue(split){
 const counts=loomDatasetSplitCounts(split);
 return counts?counts.map(loomResearchNumber).join('/'):(loomResearchSplit(split)||'Missing');
}
function loomDatasetFacts(e,compact=false){
 const facts=loomResearchFactMap(e),unit=facts.get('count_unit')?.value;
 const count=facts.get('count');
 const episodeCount=!unit||/^(episodes?|demonstrations?|trajector(?:y|ies)|rollouts?)$/i.test(unit)?count:null;
 const split=facts.get('split');
 const quantity=compact||!count?{value:'Missing',...episodeCount,id:'count',label:'Episodes'}:loomResearchCountFact(count,unit);
 const fields=[quantity,
  {value:'Missing',...split,id:'split',label:'Split'},loomDatasetTagFact([{organization:e.dataset_organization}])];
 const sources=(e.dataset_lineage?.parents||[]).filter(link=>link.transformation?.kind==='augment')
  .map(link=>link.label||loomDatasetParentEntity(link)?.label||link.parent?.identity?.id||'Missing');
 if(sources.length)fields.push({id:'augmented',label:'Augmented from',value:[...new Set(sources)].join(' · '),status:'recorded'});
 return fields;
}
// Older producers duplicated a recorded split in both the label and subgroup.
// Recognize that specific pair without changing the recorded identity or taxonomy.
function loomDatasetSplitCounts(split){
 if(!split||typeof split!=='object')return null;
 if(!Array.isArray(split)&&Object.values(split).some(Array.isArray))return null;
 const items=loomResearchSplitItems(split);
 const [train,validation,test]=items,counts=items.map(x=>x.count??x.adopted);
 if(items.length<2||items.length>3||train.id!=='train'||!['validation','val'].includes(validation.id)
    ||(test&&test.id!=='test')||!counts.every(n=>Number.isSafeInteger(n)&&n>=0))return null;
 return counts;
}
function loomDatasetLegacySplit(path,split){
 const counts=loomDatasetSplitCounts(split);
 if(!counts)return null;
 const label='Split '+counts.join('/'),tail=path.at(-1);
 return tail?.id==='split-'+counts.join('-')&&tail.label===label?label:null;
}
function loomDatasetDisplayPath(path,split){return loomDatasetLegacySplit(path,split)?path.slice(0,-1):path;}
function loomDatasetDisplayLabel(label,path,split){
 const legacy=loomDatasetLegacySplit(path,split),suffix=' · '+legacy;
 return legacy&&typeof label==='string'&&label.endsWith(suffix)?label.slice(0,-suffix.length):label;
}
function loomCatalogRecordLabel(e){
 if(e?.kind!=='dataset')return e?.label;
 const f=loomResearchFactMap(e).get('split');
 const split=f?.status==='recorded'?f.value:null,path=e.dataset_organization?.path||[];
 return loomDatasetDisplayLabel(e.label,path,split);
}
function loomDatasetRow(e,depth=0){
 const handle=e.dataset_organization?.handle;
 return '<article role="listitem" class="pa-catalog-item pa-scientific-card pa-dataset-row" data-output-id="'+esc(e.id)+'" data-dataset-depth="'+depth+'">'
  +(handle?'<div class="pa-dataset-handle">'+esc(handle)+'</div>':'')
  +'<div class="pa-row-main pa-dataset-name">'+catLink(e.id)+loomDatasetOriginBadge(e)+loomOutputEditing(e)+'</div>'
  +loomResearchFactsHTML(loomDatasetFacts(e,true),true)+'</article>';
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
   const split=loomResearchFactMap(e).get('split');
   const path=organization?.task_ref?.id===task?.id?loomDatasetDisplayPath(organization?.path||[],split?.status==='recorded'?split.value:null):[];
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
 const rows=loomUniqueDatasets(loomOutputRows(catKind('dataset',true)));
 const list=rows.length?loomDatasetGroups(rows):'<div class="pa-empty">표시할 데이터셋이 없습니다.</div>';
 return heading('CATALOG','Datasets','')+catStatus()+loomOutputStatusTabs()
  +panel(`<span data-dataset-result-count>${rows.length}개 데이터셋</span>`,list+loomCatalogPager('dataset','Load more datasets'));
}
const loomDatasetPriorList=catalogList;
catalogList=function(kind){return kind==='dataset'?loomDatasetList():loomDatasetPriorList(kind);};

root.addEventListener('click',event=>{
 const button=event.target.closest('[data-edit-dataset-metadata]');
 if(!button)return;
 const e=catEntity(button.dataset.editDatasetMetadata),target=loomOutputTarget(e);
 const metadata=e?.dataset_organization||{},task=metadata.task_ref||e?.task_refs?.[0];
 if(!target||!task)return;
 button.closest('details').open=false;
 const dialog=loomHubDialog('Edit dataset metadata',`<form>
  <label>Handle<input name="handle" maxlength="64" value="${esc(metadata.handle||'')}"></label>
  <label>Tags, one per line<textarea name="tags" rows="3">${esc((metadata.tags||[]).join('\n'))}</textarea></label>
  <label>Description<textarea name="description" rows="3" maxlength="4096">${esc(metadata.description??e.description??'')}</textarea></label>
  <label>Custom properties (JSON)<textarea name="properties" rows="4">${esc(JSON.stringify(metadata.properties||{},null,2))}</textarea></label>
  <button type="submit">Save</button>
 </form>`);
 const form=dialog.querySelector('form'),buttonSave=form.querySelector('button'),status=dialog.querySelector('[data-hub-result]');
 buttonSave.addEventListener('click',async event=>{
  event.preventDefault();
  if(!form.reportValidity())return;
  buttonSave.disabled=true;status.textContent='';
  try{
   const data=new FormData(form),properties=JSON.parse(data.get('properties')||'{}');
   if(!properties||typeof properties!=='object'||Array.isArray(properties))throw Error('Enter a JSON object for custom properties.');
   const args={target,task_ref:{id:task.id,revision:task.revision},handle:data.get('handle').trim()||null,
    tags:data.get('tags').split('\n').map(tag=>tag.trim()).filter(Boolean),description:data.get('description'),properties,
    reason:'Edited dataset metadata in the Dashboard'};
   const preview=await loomResultAction('organize_dataset',{...args,dry_run:true});
   const result=await loomResultAction('organize_dataset',{...args,dry_run:false,key:crypto.randomUUID(),expected_digest:preview.digest});
   const current=catEntity(e.id);
   if(current&&loomScenarioSameTarget(loomOutputTarget(current),target)){
    loomCatalogMerge({entities:[{...current,dataset_organization:{...result.organization,authority:'operator'}}]});
    catalogRefreshViews();
   }
   dialog.close();dialog.remove();
  }catch(error){status.textContent=error.message;buttonSave.disabled=false;}
 });
});
