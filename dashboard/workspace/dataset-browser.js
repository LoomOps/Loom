// Task context / subgroup taxonomy / data lineage are independent dimensions.
// This renderer consumes declared organization; it never classifies by names.
function loomDatasetKey(e){return e.dataset_mirror_key||e.id;}
function loomDatasetImported(e){return e.reporting?.activity==='dataset_import'||e.origin?.type==='import';}
function loomDatasetOriginBadge(e){return loomDatasetImported(e)?'<span class="pa-dataset-origin">Imported</span>':'';}
function loomDatasetSplitTag(split){
 const counts=loomDatasetSplitCounts(split);
 return counts?{text:'split-'+counts.join('/'),title:loomResearchSplit(split)}:null;
}
function loomDatasetTagFact(splits){
 const tags=new Map();
 for(const split of splits){
  const tag=loomDatasetSplitTag(split);
  if(tag)tags.set(tag.text,tag);
 }
 return {id:'tag',label:'Tag',value:'Missing',status:'recorded',tags:[...tags.values()]};
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
 const facts=loomResearchFactMap(e),unit=facts.get('count_unit')?.value;
 const count=!unit||/^(episodes?|demonstrations?|trajector(?:y|ies)|rollouts?)$/i.test(unit)?facts.get('count'):null;
 const split=facts.get('split');
 const fields=[split?loomDatasetSplitFact(split):loomDatasetTagFact([]),
               {value:'Missing',...count,id:'count',label:'Episodes'}];
 return '<article role="listitem" class="pa-catalog-item pa-scientific-card pa-dataset-row" data-output-id="'+esc(e.id)+'" data-dataset-depth="'+depth+'">'
  +'<div class="pa-row-main pa-dataset-name">'+catLink(e.id)+'</div>'
  +loomResearchFactsHTML(fields,true)+'</article>';
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
