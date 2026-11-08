function loomRebuildExplorer() {
  const nodes=new Map([['storage',{id:'storage',name:'Data Storage',folder:true}]]);
  const observed=resourceData?.sections?.storage?.data;
  const roots=new Set(Array.isArray(observed)?observed.map(item=>item.path).filter(path=>/^data\/[^/]+$/.test(path)):[]);
  const files=catalogData.entities.flatMap(entity=>(entity.loom_files||[]).map(file=>({file,entity})));
  // The catalog indexes registered files, not the host's arbitrary filesystem.
  if(!Array.isArray(observed))for(const {file} of files){
    const match=/^data\/([^/]+)\//.exec(file.path||'');if(match)roots.add('data/'+match[1]);
  }
  for(const path of [...roots].sort())nodes.set('storage/'+path,{id:'storage/'+path,name:path,parent:'storage',folder:true});
  for(const {file,entity} of files) {
    const path=file.path||'',mount=[...roots].find(root=>path.startsWith(root+'/'));
    if(!mount)continue;
    const parts=path.slice(mount.length+1).split('/').filter(Boolean);
    if(parts.some(part=>part==='.'||part==='..'))continue;
    let parent='storage/'+mount;
    for(let i=0;i<parts.length;i++) {
      const id='storage/'+mount+'/'+parts.slice(0,i+1).join('/');
      let node=nodes.get(id);
      if(!node){node={id,name:parts[i],parent,folder:i<parts.length-1,entries:[]};nodes.set(id,node);}
      if(i===parts.length-1)node.entries.push({file,entity});
      parent=id;
    }
  }
  fileNodes.splice(0,fileNodes.length,...nodes.values());
}
const loomExplorerRefresh=catalogRefreshViews;
catalogRefreshViews=function(...args) {const result=loomExplorerRefresh(...args);loomRebuildExplorer();return result;};

function loomFileStatus(status) {
  return ({available:'사용 가능',intentionally_not_migrated:'이관 제외',missing:'파일 없음',corrupt:'무결성 불일치',too_large:'미리보기 용량 초과'})[status]||status||'미확인';
}

files=function() {
  loomRebuildExplorer();
  const current=fileNodes.find(node=>node.id===state.fileSelected)||fileNodes[0];
  const trail=fileTrail(current), selected=new Set(trail.map(node=>node.id));
  const favorites=fileNodes.filter(node=>node.parent==='storage');
  const chosen=current.entries?.find(entry=>entry.file.status==='available')||current.entries?.[0];
  const description=chosen ? '<dt>저장 경로</dt><dd class="pa-mono">'+esc(chosen.file.target_ref||'저장된 사본 없음')+'</dd>'
    +'<dt>원본 참조</dt><dd class="pa-mono">'+esc(chosen.file.source_ref)+'</dd><dt>상태</dt><dd>'+esc(loomFileStatus(chosen.file.status))+'</dd>'
    +(chosen.file.size!=null?'<dt>크기</dt><dd>'+Number(chosen.file.size).toLocaleString()+' bytes</dd>':'')
    : '<dt>항목</dt><dd>'+fileNodes.filter(node=>node.parent===current.id).length+'</dd>';
  const previews=chosen?.entity.loom_media?.filter(item=>item.id===chosen.file.preview_id);
  const preview=previews?.length?loomMediaPanel(chosen.entity,previews):'';
  let body=heading('WORKSPACE','Explorer','연결된 데이터 스토리지의 등록 파일을 확인합니다.')
    +`<section class="pa-finder"><div class="pa-finder-toolbar"><span>${icon('columns-3')} 열 보기</span><span class="pa-muted-label">읽기 전용 · 선택한 파일만 불러옵니다</span></div><div class="pa-finder-dock"><aside class="pa-finder-favorites">${favorites.map(node=>`<button data-column-node="${esc(node.id)}" class="${selected.has(node.id)?'active':''}">${icon('folder')}${esc(node.name==='references'?'이관되지 않은 참조':node.name)}</button>`).join('')}</aside><div class="pa-columns-strip" aria-label="폴더 열 탐색">${trail.filter(node=>node.folder).slice(-2).map(folder=>`<div class="pa-finder-column"><div class="pa-column-title">${esc(folder.name)}</div><div class="pa-column-items" role="listbox" aria-label="${esc(folder.name)} 항목">${fileNodes.filter(node=>node.parent===folder.id).sort((a,b)=>Number(b.folder)-Number(a.folder)||a.name.localeCompare(b.name)).map(node=>`<button class="pa-column-node ${current.id===node.id?'is-current':''}" role="option" aria-selected="${selected.has(node.id)}" data-column-node="${esc(node.id)}" title="${esc(node.name)}">${icon(node.folder?'folder':'file-text')}<span>${esc(node.name)}</span>${node.folder?icon('chevron-right'):''}</button>`).join('')||'<div class="pa-empty-column">등록된 파일이 없습니다.</div>'}</div></div>`).join('')}<aside class="pa-finder-preview" aria-label="선택 항목 정보">${preview}<div class="pa-file-symbol">${icon(current.folder?'folder':'file-text')}</div><h3>${esc(current.name)}</h3><dl class="pa-props">${description}</dl>${chosen?`<button class="pa-button" data-loom-explorer-entity="${esc(chosen.entity.id)}">연결된 ${esc(chosen.entity.label||'기록')} 열기</button>`:''}</aside></div></div><div class="pa-finder-path" aria-label="현재 경로">${trail.map(node=>`<button data-column-node="${esc(node.id)}">${esc(node.name)}</button>`).join('<span>›</span>')}</div></section>`;
  return body;
};
root.addEventListener('click',event=>{
  const button=event.target.closest('[data-loom-explorer-entity]');if(!button)return;
  const entity=catEntity(button.dataset.loomExplorerEntity);if(!entity)return;
  const page={model:'models',dataset:'datasets',experiment:'experiments'}[entity.kind]||'queue';
  openView({page,entity:entity.id});
});
