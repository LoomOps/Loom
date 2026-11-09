// Read-only software inventory. Registration is not a runtime health verdict.
const loomExtensionTypes={
 component:{label:'Component',icon:'blocks',role:'Job lifecycle',description:'Job의 실행과 완료 계약을 정의합니다.'},
 tool:{label:'Tool',icon:'square-terminal',role:'Research software',description:'Job 안에서 호출하는 플래너·정책·연구 소프트웨어입니다.'}
};
function loomExtensionRows(){return catalogData.entities.filter(e=>e.kind==='component'||e.kind==='tool');}
function loomHistoricalExtensionEntity(id){
 const key=String(id).match(/^(tool|component):([^:]+):([^:]+):r(\d+)$/);if(!key)return null;
 const current=loomExtensionRows().find(e=>e.kind===key[1]&&e.loom_extension?.owner===key[2]&&e.loom_extension?.id===key[3]);
 return current&&loomExtensionVersions(current).some(v=>v.revision===Number(key[4]))?{...current,id,loom_requested_revision:Number(key[4])}:null;
}
function loomExtensionOwner(e){return e.loom_extension?.owner||e.owner||e.namespace||'미기록';}
function loomExtensionVersions(e){
 const item=e.loom_extension;
 if(!item)return [];
 return item.revisions?.length?item.revisions:[{revision:item.revision,sha256:item.sha256,label:e.label,
  description:item.manifest?.description,source_pins:item.manifest?.source_pins||{},
  profiles:Object.entries(item.profiles||{}).map(([kind,p])=>({kind,tools:p.tool_pins||{},stages:[],limits:p.limits||{}})),used_by:[]}];
}
function loomExtensionBadges(e,revision){
 const type=loomExtensionTypes[e.kind];
 return `<span class="pa-extension-kind" data-kind="${e.kind}">${icon(type.icon)}${type.label}</span>`
  +`<span class="pa-extension-revision">${esc(revision==null?'보존 기록':'r'+revision)}</span>`;
}
function loomToolsView(){
 const rows=loomExtensionRows(),kind=['component','tool'].includes(state.extensionKind)?state.extensionKind:'all';
 const query=state.extensionQuery||'',owner=state.extensionOwner||'';
 const owners=[...new Set(rows.map(loomExtensionOwner))].sort((a,b)=>a.localeCompare(b));
 const shown=rows.filter(e=>(kind==='all'||e.kind===kind)&&(!owner||loomExtensionOwner(e)===owner)&&
  [e.label,e.id,e.summary,loomExtensionOwner(e),e.loom_extension?.id].join(' ').toLocaleLowerCase().includes(query.toLocaleLowerCase()))
  .sort((a,b)=>String(a.label).localeCompare(String(b.label))||a.id.localeCompare(b.id));
 const summary=`<div class="pa-extension-summary">${Object.entries(loomExtensionTypes).map(([key,type])=>`<div><span>${icon(type.icon)}${type.label}s</span><strong>${rows.filter(e=>e.kind===key).length}</strong><p>${esc(type.description)}</p></div>`).join('')}</div>`;
 const filters=`<div class="pa-extension-toolbar"><div class="pa-extension-filters" role="group" aria-label="소프트웨어 종류">${[['all','전체'],['component','Components'],['tool','Tools']].map(([key,label])=>`<button type="button" data-extension-kind="${key}" aria-pressed="${kind===key}">${label}</button>`).join('')}</div><label class="pa-extension-search">${icon('search')}<input type="search" data-extension-query aria-label="Components & Tools 검색" placeholder="이름, ID, 소유자 검색" value="${esc(query)}"></label><select data-extension-owner aria-label="소프트웨어 소유자"><option value="">모든 소유자</option>${owners.map(value=>`<option value="${esc(value)}" ${owner===value?'selected':''}>${esc(value)}</option>`).join('')}</select></div>`;
 const cards=shown.map(e=>{
  const item=e.loom_extension,version=loomExtensionVersions(e)[0],files=Object.keys(version?.source_pins||{}).length;
  const profiles=e.kind==='component'?version?.profiles?.length:version?.used_by?.length;
  return `<article class="pa-extension-card" data-extension-card="${esc(e.id)}"><div class="pa-extension-card-head">${loomExtensionBadges(e,item?.revision)}<span>${esc(loomExtensionOwner(e))}</span></div><h3>${catLink(e.id,e.label)}</h3><code class="pa-extension-id">${esc(item?.id||e.id)}</code><p>${esc(e.summary||version?.description||'설명이 등록되지 않았습니다.')}</p><div class="pa-extension-card-facts"><span>${icon('files')}${item?'파일 '+files+'개':'보존된 카탈로그 기록'}</span>${item?`<span>${icon('git-branch')}revision ${loomExtensionVersions(e).length}개</span><span>${icon('workflow')}${e.kind==='tool'?'연결 Profile':'Profile'} ${profiles??0}개</span>`:''}</div></article>`;
 }).join('');
 const empty=rows.length?'<h3>조건에 맞는 항목이 없습니다.</h3><p>검색어나 소유자·종류 필터를 바꿔보세요.</p><button class="pa-button" data-extension-clear>필터 초기화</button>':'<h3>등록된 Component나 Tool이 없습니다.</h3><p>Job의 실행 계약은 Component로, 그 안에서 호출하는 연구 소프트웨어는 Tool로 등록하면 여기에 표시됩니다.</p>';
 return heading('SOFTWARE','Components & Tools','실행 계약과 연구 소프트웨어를 구분하고, 고정된 버전과 연결 관계를 확인합니다.')+catStatus()+summary
  +panel('등록된 소프트웨어',filters+`<div class="pa-extension-results" role="status">${shown.length} / ${rows.length}개 · 항목별 최신 revision</div><div class="pa-extension-grid">${cards||'<div class="pa-extension-empty">'+empty+'</div>'}</div>`);
}
function loomExtensionOpenLink(entity,revision,label){
 return `<button class="pa-link" data-extension-open="${esc(entity.id)}" data-extension-revision="${esc(revision)}">${esc(label||entity.label)} ↗</button>`;
}
function loomExtensionToolLink(pin,alias){
 const target=loomExtensionRows().find(e=>e.kind==='tool'&&e.loom_extension?.id===pin.id&&e.loom_extension?.owner===pin.owner);
 const label=(alias?alias+' → ':'')+pin.owner+'/'+pin.id+' · r'+pin.revision;
 return target?loomExtensionOpenLink(target,pin.revision,label):esc(label);
}
function loomExtensionProfile(profile){
 const tools=Object.entries(profile.tools||{});
 return `<article class="pa-extension-profile"><h3>${esc(profile.kind)}</h3><div class="pa-extension-stages">${(profile.stages||[]).map(stage=>`<div><b>${esc(stage.stage==='runtime'?'run':stage.stage)}</b><span>${esc(String(stage.lane||'cpu').toUpperCase())} · ${esc(stage.runtime||'미기록')}</span><code>${esc((stage.command||[]).join(' '))}</code></div>`).join('')}</div>${tools.length?'<div class="pa-extension-pins">'+tools.map(([alias,pin])=>loomExtensionToolLink(pin,alias)).join('')+'</div>':''}${profile.limits?.timeout_seconds?`<small>시간 제한 ${esc(profile.limits.timeout_seconds)}s · 최대 ${esc(profile.limits.max_attempts)} attempts</small>`:''}</article>`;
}
function loomExtensionDetail(entity){
 const item=entity.loom_extension,versions=loomExtensionVersions(entity),snapshot=versions.find(v=>v.revision===(state.extensionRevision??entity.loom_requested_revision))||versions[0];
 const type=loomExtensionTypes[entity.kind],pins=snapshot.source_pins||{},profiles=snapshot.profiles||[],consumers=snapshot.used_by||[];
 const filter=state.extensionSourceQuery||'',files=Object.entries(pins).filter(([name])=>name.toLocaleLowerCase().includes(filter.toLocaleLowerCase()));
 const history=versions.map(v=>`<button data-extension-select="${esc(v.revision)}" aria-pressed="${v.revision===snapshot.revision}"><strong>r${esc(v.revision)}${v.revision===item.revision?'<small>Latest</small>':''}</strong><span>${Object.keys(v.source_pins||{}).length} files${entity.kind==='component'?' · '+(v.profiles||[]).length+' profiles':''}</span></button>`).join('');
 const relations=entity.kind==='component'?panel('Execution profiles <small>'+profiles.length+'</small>',profiles.map(loomExtensionProfile).join('')||'<div class="pa-empty">이 revision에는 실행 Profile이 없습니다.</div>')
  :panel('이 버전을 사용하는 Profiles <small>'+consumers.length+'</small>',consumers.length?consumers.map(consumer=>{
   const target=catEntity(consumer.component);
   return `<article class="pa-extension-consumer"><b>${esc(consumer.kind)}</b><span>alias · ${esc(consumer.alias)} · ${esc(consumer.owner)}</span>${target?loomExtensionOpenLink(target,Number(consumer.kind.match(/\.r(\d+)$/)?.[1]),target.label):'<small>독립 Profile</small>'}</article>`;
  }).join(''):'<div class="pa-empty">이 revision을 참조하는 등록 Profile이 없습니다. Tool 등록만으로 Job에 자동 연결되지는 않습니다.</div>');
 const source=panel('Source files <small>'+Object.keys(pins).length+'</small>',`<input class="pa-extension-file-search" type="search" data-extension-source-query aria-label="소스 파일 검색" placeholder="파일 이름 검색" value="${esc(filter)}">`
  +(files.length?catTable(['File','SHA-256'],files.slice(0,100).map(([name,hash])=>[`<code>${esc(name)}</code>`,`<code class="pa-extension-hash">${esc(hash)}</code>`])):'<div class="pa-empty">표시할 소스 파일이 없습니다.</div>')+(files.length>100?`<p class="pa-subtle">${files.length}개 중 100개 표시 · 파일 이름으로 범위를 좁힐 수 있습니다.</p>`:''));
 const metadata=snapshot.revision===item.revision?item.manifest:{revision:snapshot.revision,source_pins:pins,origin:snapshot.origin,profiles};
 return heading('SOFTWARE',esc(snapshot.label||entity.label),esc(snapshot.description||entity.summary||type.description),'<button class="pa-button" data-extension-home>목록으로</button>')+catStatus()
  +`<p class="pa-extension-description">${esc(snapshot.description||entity.summary||type.description)}</p>`
  +`<div class="pa-extension-detail-badges">${loomExtensionBadges(entity,snapshot.revision)}<span>${esc(type.role)}</span><label>Revision <select data-extension-version aria-label="표시할 revision">${versions.map(v=>`<option value="${esc(v.revision)}" ${v.revision===snapshot.revision?'selected':''}>r${esc(v.revision)}${v.revision===item.revision?' · latest':''}</option>`).join('')}</select></label></div>`
  +`<div class="pa-extension-detail-grid"><main>${relations}${source}${panel('등록 정보',`<details><summary>Metadata 확인</summary><pre>${esc(JSON.stringify(metadata,null,2))}</pre></details>`)}</main><aside>${panel('Revision history',history)}${panel('Identity',catProperties({Owner:entity.owner_profile?.display_name||item.owner_name||item.owner,Namespace:item.owner,ID:item.id||entity.id,Role:type.role,'Source files':Object.keys(pins).length,...(snapshot.sha256?{SHA256:snapshot.sha256}:{})}))}${snapshot.origin?panel('Origin',catProperties(snapshot.origin)):''}</aside></div>`;
}
function loomExtensionView(target){const pane=target.closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;state=activeTab().view;return state;}
function loomExtensionNavigate(next){const existing=activePane().tabs.find(t=>t.id===tabKey(next));if(existing)Object.assign(existing.view,next);openView(next);}
root.addEventListener('click',event=>{
 const button=event.target.closest('[data-extension-kind],[data-extension-clear],[data-extension-select],[data-extension-home],[data-extension-open]');if(!button)return;
 const view=loomExtensionView(button);
 if(button.hasAttribute('data-extension-kind'))view.extensionKind=button.dataset.extensionKind;
 if(button.hasAttribute('data-extension-clear'))Object.assign(view,{extensionQuery:'',extensionOwner:'',extensionKind:'all'});
 if(button.hasAttribute('data-extension-select')){view.extensionRevision=Number(button.dataset.extensionSelect);view.extensionSourceQuery='';}
 if(button.hasAttribute('data-extension-home'))return loomExtensionNavigate({page:'tools',entity:null,extensionKind:catEntity(view.entity)?.kind||'all'});
 if(button.hasAttribute('data-extension-open'))return loomExtensionNavigate({...catView(button.dataset.extensionOpen),extensionRevision:Number(button.dataset.extensionRevision)});
 render();persist();
});
function loomExtensionInput(event){
 const field=event.target.closest('[data-extension-query],[data-extension-owner],[data-extension-source-query]');if(!field)return;
 const view=loomExtensionView(field),name=field.hasAttribute('data-extension-query')?'extensionQuery':field.hasAttribute('data-extension-owner')?'extensionOwner':'extensionSourceQuery';
 view[name]=field.value;const start=field.selectionStart,end=field.selectionEnd,selector=field.hasAttribute('data-extension-query')?'[data-extension-query]':field.hasAttribute('data-extension-source-query')?'[data-extension-source-query]':'[data-extension-owner]';
 render();persist();const next=root.querySelector('.pa-document-pane.is-active '+selector);next?.focus();if(next?.type==='search'&&start!=null)next.setSelectionRange(start,end);
}
root.addEventListener('input',event=>{if(!event.isComposing&&event.target.matches('[data-extension-query],[data-extension-source-query]'))loomExtensionInput(event);});
root.addEventListener('change',event=>{
 if(event.target.matches('[data-extension-version]')){const view=loomExtensionView(event.target);view.extensionRevision=Number(event.target.value);view.extensionSourceQuery='';render();persist();}
 else if(event.target.matches('[data-extension-owner]'))loomExtensionInput(event);
});
helpDescriptions.tools='Components는 Job 실행·완료 계약, Tools는 Job 안에서 호출하는 연구 소프트웨어입니다. 등록된 revision과 Profile 연결을 확인합니다.';
