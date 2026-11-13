// Read-only software inventory. Registration is not a runtime health verdict.
const loomProfileUsage=p=>catalogData.presentation_view==='research'&&p.research_usage_stats?p.research_usage_stats:p.usage_stats;
const loomExtensionTypes={
 execution_profile:{label:'Profile',icon:'workflow',role:'Execution combination',description:''},
 component:{label:'Component',icon:'blocks',role:'Job lifecycle',description:''},
 tool:{label:'Tool',icon:'square-terminal',role:'Research software',description:''},
 runtime:{label:'Runtime',icon:'box',role:'Runtime',description:''}
};
const loomHubPages={profiles:'Profiles',components:'Components',tools:'Tools',runtimes:'Runtimes'};
function loomIsHubPage(page){return Object.hasOwn(loomHubPages,page);}
function loomHubTabs(){
 const context=state.hubContext&&catEntity(state.hubContext),p=context?.loom_execution_profile;
 return `<nav class="pa-hub-tabs" aria-label="Runtimes">${Object.entries(loomHubPages).map(([page,label])=>`<button data-hub-page="${page}" aria-current="${state.page===page?'page':'false'}">${icon(({profiles:'workflow',components:'blocks',tools:'square-terminal',runtimes:'box'})[page])}${label}</button>`).join('')}</nav>`+(p&&state.entity!==context.id?`<div class="pa-hub-context">${catLink(context.id,loomProfileName(p))}<span>› ${esc(loomHubPages[state.page])}</span></div>`:'');
}
function loomHubNavigate(next){
 const pane=activePane(),tab=activeTab(),current=catEntity(state.entity);
 const context=Object.hasOwn(next,'hubContext')?next.hubContext:(current?.kind==='execution_profile'?current.id:state.hubContext);
 const view=viewCopy({...next,hubContext:context}),key=tabKey(view),existing=pane.tabs.find(t=>t!==tab&&t.id===key);
 if(existing){existing.view=view;pane.active=key;}else{tab.id=key;tab.view=view;pane.active=key;}
 menu=null;render();persist();
}
function loomHubOrigin(d){
 if(d.deprecated)return 'deprecated';
 if(d.origin?.type==='managed-directory')return 'managed';
 if(d.origin?.type==='external-mount')return 'external';
 if(['image','bundled-extension'].includes(d.origin?.type)||['image','image-os','isaac-extension'].includes(d.scope))return 'image';
 return 'unknown';
}
function loomHubHighlights(env){
 let deps=env.primary_dependencies;
 if(!deps){const item=catEntity(env.entity)?.loom_runtime;deps=item?.primary_dependencies;}
 // Old API fixtures may have only a single declared dependency. Large raw
 // inventories never become a substitute for curated representative entries.
 if(!deps)deps=(env.dependencies?.length<=8?env.dependencies:[])||[];
 return [...new Map(deps.map(d=>[d.name.toLowerCase()+':'+d.version,d])).values()].map(d=>`<span class="pa-profile-dependency" title="${esc(d.scope||'')}">${esc(({'isaacsim':'Isaac Sim','isaaclab':'Isaac Lab','isaac-python':'Kit Python','torch':'PyTorch','lerobot':'LeRobot','python':'Python'})[d.name.toLowerCase()]||d.name)} <b>${esc(d.version.split('+release.')[0])}</b></span>`).join('');
}
function loomHubIO(p){
 const input=Object.keys(p.inputs||{}),output=Object.entries(p.outputs||{}).map(([key,kind])=>key===kind?kind:key+' · '+kind);
 return `<div class="pa-hub-io"><div><small>Input</small>${input.slice(0,5).map(x=>`<span title="${esc(x)}">${esc(x.replaceAll('_',' '))}</span>`).join('')||'—'}${input.length>5?`<details><summary>All inputs</summary>${input.slice(5).map(x=>`<span>${esc(x.replaceAll('_',' '))}</span>`).join(' · ')}</details>`:''}</div><span aria-hidden="true">→</span><div><small>Output</small>${output.map(x=>`<span>${esc(x)}</span>`).join('')||'—'}</div></div>`;
}
function loomHubFamily(e){const p=e.loom_execution_profile;return p.family||p.kind;}
function loomHubProfilePicker(p){
 const others=loomExtensionRows().filter(e=>e.kind==='execution_profile'&&loomHubFamily(e)===(p.family||p.kind));
 return others.length>1?`<select data-profile-revision aria-label="Profile revision">${others.map(e=>`<option value="${esc(e.id)}" ${e.loom_execution_profile.kind===p.kind?'selected':''}>r${esc(e.loom_execution_profile.origin.revision||'—')}${e.loom_execution_profile.curation?.preferred?' · Preferred':''}${e.loom_execution_profile.curation?.state==='archived'?' · Archived':''}</option>`).join('')}</select>`:'';
}
let loomHubSequence=0;const loomHubRequests=new Map();
function loomHubAction(action,args){
 const id='runtime-hub-'+(++loomHubSequence);
 return new Promise((resolve,reject)=>{const timeout=setTimeout(()=>{loomHubRequests.delete(id);reject(Error('Request timed out'));},35000);loomHubRequests.set(id,{resolve,reject,timeout});parent.postMessage({type:'loom-runtime-action',id,action,arguments:args},'*');});
}
window.addEventListener('message',event=>{
 if(event.source!==parent||event.data?.type!=='loom-runtime-action-result')return;
 const r=loomHubRequests.get(event.data.id);if(!r)return;clearTimeout(r.timeout);loomHubRequests.delete(event.data.id);
 event.data.ok?r.resolve(event.data.value):r.reject(Error(event.data.value?.message||event.data.value?.error||'Request failed'));
});
function loomHubDialog(title,body){
 root.querySelector('[data-hub-dialog]')?.remove();const dialog=document.createElement('dialog');dialog.className='pa-hub-dialog';dialog.dataset.hubDialog='';
 dialog.innerHTML=`<header><h2>${esc(title)}</h2><button class="pa-button" data-hub-close aria-label="Close">×</button></header>${body}<p data-hub-result role="status"></p>`;
 dialog.querySelector('[data-hub-close]').onclick=()=>{dialog.close();dialog.remove();};root.append(dialog);dialog.showModal();return dialog;
}

function loomExtensionRows(){return catalogData.entities.filter(e=>['component','tool','runtime','execution_profile'].includes(e.kind));}
function loomHistoricalExtensionEntity(id){
 const key=String(id).match(/^(tool|component):([^:]+):([^:]+):r(\d+)$/);if(!key)return null;
 const current=loomExtensionRows().find(e=>e.kind===key[1]&&e.loom_extension?.owner===key[2]&&e.loom_extension?.id===key[3]);
 return current&&loomExtensionVersions(current).some(v=>v.revision===Number(key[4]))?{...current,id,loom_requested_revision:Number(key[4])}:null;
}
function loomExtensionOwner(e){return e.loom_runtime?'Administrator':e.owner_profile?.display_name||e.owner_name||e.loom_extension?.owner_name||e.loom_extension?.owner||e.owner||e.namespace||'미기록';}
function loomExtensionOwnerKey(e){return e.loom_runtime?'administrator':e.owner_profile?.id||e.loom_execution_profile?.owner_profile?.id||e.loom_extension?.owner||e.owner||e.namespace||'unrecorded';}
function loomExtensionProvisioning(e){
 if(e.loom_runtime)return '관리자 설치';
 if(e.owner_kind==='built-in'||e.loom_extension?.owner_kind==='built-in'||e.loom_execution_profile?.owner_kind==='built-in')return 'Built-in';
 if(e.loom_execution_profile?.origin?.type==='installation')return '설치 설정';
 return e.loom_extension||e.loom_execution_profile?'사용자 등록':'출처 미기록';
}
function loomExtensionByline(e){const source=loomExtensionProvisioning(e);return `<span data-extension-provisioning>${esc(source)}</span>`+(source==='Built-in'?'':`<span>${esc(loomExtensionOwner(e))}</span>`);}
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
  +`<span class="pa-extension-revision">${esc(revision==null?(e.loom_runtime?'—':e.loom_execution_profile?'고정 Profile':'보존 기록'):'r'+revision)}</span>`;
}
function loomComponentsView(){return loomSoftwareView('component');}
function loomToolsView(){return loomSoftwareView('tool');}
function loomRuntimesView(){return loomSoftwareView('runtime');}
function loomSoftwareView(kind){
 let rows=loomExtensionRows().filter(e=>e.kind===kind);
 if(kind==='runtime'&&!state.hubArchive)rows=rows.filter(e=>{const r=e.loom_runtime;return r.curation?.state!=='archived'&&((r.legacy||r.manifest.reconciliation)?r.legacy||r.aliases?.length:!rows.some(other=>other.loom_runtime.id===r.id&&(other.loom_runtime.revision||0)>(r.revision||0)));});
 const query=state.extensionQuery||'',owner=state.extensionOwner||'',tag=state.softwareTag||'';
 const owners=[...new Map(rows.map(e=>[loomExtensionOwnerKey(e),loomExtensionOwner(e)])).entries()].sort((a,b)=>a[1].localeCompare(b[1]));
 const shown=rows.filter(e=>(!tag||e.loom_extension?.classification?.tags?.includes(tag))&&(!owner||loomExtensionOwnerKey(e)===owner)&&
  [e.label,e.id,e.summary,loomExtensionOwner(e),e.loom_extension?.id,e.loom_extension?.classification?.label,...(e.loom_extension?.classification?.tags||[]),e.loom_execution_profile?.kind,...loomProfilePurposeTerms(e.loom_execution_profile)].join(' ').toLocaleLowerCase().includes(query.toLocaleLowerCase()))
  .sort((a,b)=>String(a.label).localeCompare(String(b.label))||a.id.localeCompare(b.id));
 const filters=`<div class="pa-extension-toolbar"><label class="pa-extension-search">${icon('search')}<input type="search" data-extension-query aria-label="소프트웨어 검색" placeholder="검색" value="${esc(query)}"></label><select data-extension-owner aria-label="등록자"><option value="">전체 등록자</option>${owners.map(([key,value])=>`<option value="${esc(key)}" ${owner===key?'selected':''}>${esc(value)}</option>`).join('')}</select>${kind==='runtime'?`<label class="pa-hub-toggle"><input type="checkbox" data-hub-archive ${state.hubArchive?'checked':''}>History</label>`:''}</div>`;
 const tags=[...new Set(rows.flatMap(e=>e.loom_extension?.classification?.tags||[]))];
 const tagFilter=tags.length?`<div class="pa-software-filter">${tags.map(t=>`<button data-software-tag="${esc(t)}" aria-pressed="${tag===t}">${loomSoftwareTags({tags:[t]})}</button>`).join('')}</div>`:'';
 const cards=shown.map(e=>{
  if(e.loom_runtime)return loomRuntimeCard(e);

  const item=e.loom_extension,version=loomExtensionVersions(e)[0],files=Object.keys(version?.source_pins||{}).length;
  const profiles=e.kind==='component'?version?.profiles?.length:version?.used_by?.length;
  const connected=catalogData.presentation_view==='research'&&e.research_context?e.research_context.profiles.map(kind=>({kind,stages:[]})):version?.connected_profiles,stageNames=[...new Set((connected||[]).flatMap(p=>p.stages||[]))];
  return `<article class="pa-extension-card" data-extension-card="${esc(e.id)}"><div class="pa-extension-card-head">${loomExtensionBadges(e,item?.revision)}${loomExtensionByline(e)}</div><h3>${catLink(e.id,item?.classification?.label||e.label)}</h3>${loomSoftwareTags(item?.classification||version?.classification)}${stageNames.length?`<div class="pa-hub-stage-labels">${stageNames.map(esc).join(' · ')}</div>`:''}<div class="pa-extension-card-facts"><span>Profiles ${connected?.length??profiles??0}</span>${version?.pinned_jobs?`<span>Pinned Jobs ${(catalogData.presentation_view==='research'&&e.research_context?e.research_context.jobs.length:version.pinned_jobs.length)}</span>`:''}</div></article>`;
 }).join('');
 const empty=rows.length?'<h3>조건에 맞는 항목이 없습니다.</h3><button class="pa-button" data-extension-clear>필터 초기화</button>':'<h3>등록된 소프트웨어가 없습니다.</h3>';
 const runtimeStatus=catalogData.runtime_catalog;
 const notice=runtimeStatus?.status==='unavailable'?'<p class="pa-subtle" role="status">Runtime 목록을 조회할 수 없습니다.</p>':'';
 const attempts=runtimeStatus?.installations||[];
 const installations=attempts.length&&['all','runtime'].includes(kind)?panel('Runtime 설치 기록',catTable(['Runtime','상태','설치 위치'],attempts.map(a=>[esc(a.runtime),esc(a.state+(a.container_returned===false?' · 컨테이너 반환 미확정':'')),`<code>${esc(a.path||a.id)}</code>`]))+(runtimeStatus.installations_truncated?'<p class="pa-subtle">최근 100건 표시</p>':'')):'';
 return loomHubTabs()+heading('RUNTIMES',loomExtensionTypes[kind].label+'s','')+catStatus()+notice
  +'<section class="pa-panel">'+filters+tagFilter+`<div class="pa-extension-results" role="status">${shown.length} / ${rows.length}</div><div class="pa-extension-grid">${cards||'<div class="pa-extension-empty">'+empty+'</div>'}</div>`+'</section>'+installations;
}
function loomExtensionOpenLink(entity,revision,label){
 return `<button class="pa-link" data-extension-open="${esc(entity.id)}" data-extension-revision="${esc(revision)}">${esc(label||entity.label)} ↗</button>`;
}
function loomExtensionToolLink(pin,alias){
 const target=loomExtensionRows().find(e=>e.kind==='tool'&&e.loom_extension?.id===pin.id&&e.loom_extension?.owner===pin.owner);
 const label=(alias?alias+' · ':'')+(pin.classification?.label||pin.id)+' · r'+pin.revision;
 return target?loomExtensionOpenLink(target,pin.revision,label):esc(label);
}
function loomExtensionProfile(profile){
 const tools=Object.entries(profile.tools||{});
 return `<article class="pa-extension-profile"><h3>${catEntity('profile:'+profile.kind)?catLink('profile:'+profile.kind):esc(profile.kind)}</h3><details><summary>실행 계약</summary><div class="pa-extension-stages">${(profile.stages||[]).map(stage=>`<div><b>${esc(stage.stage==='runtime'?'run':stage.stage)}</b><span>${esc(String(stage.lane||'cpu').toUpperCase())} · ${stage.runtime_pin?catLink('runtime:'+stage.runtime_pin.id+':'+stage.runtime_pin.revision,stage.runtime_pin.id+' · r'+stage.runtime_pin.revision):esc(stage.runtime||'미기록')}</span><code>${esc((stage.command||[]).join(' '))}</code></div>`).join('')}</div>${tools.length?'<div class="pa-extension-pins">'+tools.map(([alias,pin])=>loomExtensionToolLink(pin,alias)).join('')+'</div>':''}${profile.limits?.timeout_seconds?`<small>시간 제한 ${esc(profile.limits.timeout_seconds)}s · 최대 ${esc(profile.limits.max_attempts)} attempts</small>`:''}</details></article>`;
}

function loomJobInventoryLink(id){return catLink(catEntity(id)?id:catEntity('job:'+id)?'job:'+id:id,id);}
function loomProfileUsageLabels(){return {
 categories:catalogData.job_contract?.categories||{sdg:'SDG',evaluation:'Evaluation',fine_tuning:'Fine-tuning',data_preparation:'Data Preparation',pipeline:'Pipeline',other:'Other'},
 modes:catalogData.job_contract?.execution_modes||{full:'Full',pilot:'Pilot-test',smoke:'Smoke-test'}
};}
function loomProfilePurposeTerms(profile){
 if(!profile)return [];
 const terms=[...(profile.usage?.categories||[]),...(profile.usage?.execution_modes||[]),...(profile.observed_usage?.categories||[]),...(profile.observed_usage?.execution_modes||[])],labels=loomProfileUsageLabels();
 return [...terms,...terms.map(k=>labels.categories[k]||labels.modes[k]||k)];
}
function loomProfilePurpose(profile,observed=false){
 const inferred=!profile.usage&&!observed;
 const use=observed||inferred?profile.observed_usage:profile.usage;
 const {categories,modes}=loomProfileUsageLabels();
 const values=[...(use?.categories||[]).map(k=>categories[k]||k),...(use?.execution_modes||[]).map(k=>modes[k]||k)];
 return values.length?values.map(v=>`<span class="pa-profile-purpose ${inferred?'is-observed':''}" ${inferred?'title="Job 기록 기준"':''}>${esc(v)}</span>`).join(''):'';
}
function loomSoftwareTags(metadata){
 const labels={sdg:'SDG',training:'Training',evaluation:'Evaluation',validation:'Validation','data-processing':'Data processing','motion-planning':'Motion planning',policy:'Policy',control:'Control',simulation:'Simulation',rendering:'Rendering',sensing:'Sensing',perception:'Perception',metrics:'Metrics',packaging:'Packaging','platform-check':'Platform check',utility:'Utility'};
 return metadata?.tags?.length?`<div class="pa-software-tags">${metadata.tags.map(t=>`<span>${esc(labels[t]||t)}</span>`).join('')}</div>`:'';
}
function loomProfileName(p){const owner=p.origin?.owner;return p.label!==p.kind?p.label:owner&&p.kind.startsWith(owner+'.')?p.kind.slice(owner.length+1):p.kind;}
function loomProfileEnvironment(e){
 if(e.type==='builtin')return esc(e.id);
 if(e.entity)return catLink(e.entity,(e.label||e.id)+(e.revision?' · r'+e.revision:''));
 const legacy='runtime:'+e.id+':legacy';
 return e.id&&catEntity(legacy)?catLink(legacy,e.id):esc(e.id||e.image||'—');
}

function loomProfilesView(){
 const all=loomExtensionRows().filter(e=>e.kind==='execution_profile'),query=(state.extensionQuery||'').toLowerCase(),owner=state.extensionOwner||'';
 const rows=all.filter(e=>(state.hubArchive||e.loom_execution_profile.curation?.state!=='archived')&&(!owner||loomExtensionOwnerKey(e)===owner)&&[e.label,loomProfileName(e.loom_execution_profile),e.id,loomExtensionOwner(e),...loomProfilePurposeTerms(e.loom_execution_profile)].join(' ').toLowerCase().includes(query));
 const families=new Map();for(const e of rows){const k=loomHubFamily(e);if(!families.has(k))families.set(k,[]);families.get(k).push(e);}
 const choices=[...families.values()].map(es=>es.sort((a,b)=>Number(!!b.loom_execution_profile.curation?.preferred)-Number(!!a.loom_execution_profile.curation?.preferred)||(b.loom_execution_profile.last_execution?.started||0)-(a.loom_execution_profile.last_execution?.started||0)||(b.loom_execution_profile.origin.revision||0)-(a.loom_execution_profile.origin.revision||0))[0]);
 const owners=[...new Map(all.map(e=>[loomExtensionOwnerKey(e),loomExtensionOwner(e)])).entries()];
 const toolbar=`<div class="pa-extension-toolbar"><label class="pa-extension-search">${icon('search')}<input type="search" data-extension-query aria-label="Profiles search" placeholder="Search" value="${esc(state.extensionQuery||'')}"></label><select data-extension-owner aria-label="Profile owner"><option value="">All owners</option>${owners.map(([id,label])=>`<option value="${esc(id)}" ${owner===id?'selected':''}>${esc(label)}</option>`).join('')}</select><label class="pa-hub-toggle"><input type="checkbox" data-hub-archive ${state.hubArchive?'checked':''}>Archived</label></div>`;
 const groups=[['research','Research'],['platform-check','Platform checks'],['other','Other']].map(([group,label])=>{
  const items=choices.filter(e=>(e.loom_execution_profile.group||(e.owner_kind==='built-in'?'platform-check':'research'))===group).sort((a,b)=>loomProfileName(a.loom_execution_profile).localeCompare(loomProfileName(b.loom_execution_profile)));
  if(!items.length)return '';
  return `<section class="pa-hub-group"><h2>${label}</h2><div class="pa-profile-list">${items.map(e=>{
   const p=e.loom_execution_profile,envs=[...new Map(p.stages.map(s=>[s.environment.entity||s.environment.id,s.environment])).values()];
   return `<article class="pa-profile-choice" data-extension-card="${esc(e.id)}"><div class="pa-profile-choice-title"><h3>${catLink(e.id,loomProfileName(p))}</h3>${loomHubProfilePicker(p)}<div class="pa-profile-purposes">${loomProfilePurpose(p)}</div></div>${loomHubIO(p)}<div class="pa-hub-combination"><div><small>Component</small>${p.component?loomExtensionOpenLink({id:p.component.entity},p.component.revision,p.component.classification?.label||p.component.id):'—'}</div><div><small>Tools</small><span>${p.tools.map(t=>loomExtensionToolLink(t)).join(' · ')||'—'}</span></div><div><small>Runtimes</small><span>${envs.map(loomProfileEnvironment).join(' · ')}</span></div></div><footer><span>${loomProfileUsage(p)?.executed_jobs??p.jobs?.length??0} Jobs${loomProfileUsage(p)?.reuses?' · Reused '+loomProfileUsage(p).reuses:''}</span>${p.last_execution?`<span>${esc(p.last_execution.state)} · ${esc(new Date(p.last_execution.started*1000).toLocaleDateString())}</span>`:''}<div class="pa-extension-card-head">${loomExtensionByline(e)}</div></footer></article>`;
  }).join('')}</div></section>`;
 }).join('');
 return loomHubTabs()+heading('RUNTIMES','Profiles','')+catStatus()+toolbar+(groups||'<div class="pa-extension-empty">No matching profiles</div>');
}

function loomExecutionProfileDetail(entity){
 const p=entity.loom_execution_profile;
 const stageNames={prepare:'준비',runtime:'실행',finalize:'결과 정리',on_complete:'성공 시',on_error:'실패 시',on_cancel:'취소 시'};
 const software=[...(p.component?[`<div class="pa-profile-component" data-software-kind="component"><small>Component</small>${loomExtensionOpenLink({id:p.component.entity,label:p.component.id},p.component.revision,(p.component.classification?.label||p.component.id)+' · r'+p.component.revision)}${loomSoftwareTags(p.component.classification)}</div>`]:[]),...(p.tools||[]).map(pin=>`<div data-software-kind="tool" title="${esc(pin.alias)}"><small>${esc(pin.role?.role||pin.alias)}</small>${loomExtensionToolLink(pin)}${loomSoftwareTags(pin.classification)}</div>`)];
 const stages=(p.stages||[]).map(s=>{
  const env=s.environment;
  const packages=loomHubHighlights(env);
  return `<li data-profile-stage="${esc(s.stage)}" class="${s.stage.startsWith('on_')?'is-conditional':''}"><div class="pa-profile-stage-name"><strong>${esc(stageNames[s.stage]||s.stage)}</strong><small>${esc(s.lane.toUpperCase())}</small></div><div class="pa-profile-stage-environment">${loomProfileEnvironment(env)}${packages?`<div class="pa-profile-dependencies">${packages}</div>`:''}${p.tools.filter(t=>t.role?.stages.includes(s.stage)).length?`<div class="pa-hub-stage-tools">${p.tools.filter(t=>t.role?.stages.includes(s.stage)).map(t=>loomExtensionToolLink(t,t.role.role)).join(' · ')}</div>`:''}</div></li>`;
 }).join('');
 const requirements=[...(p.component?Object.entries(p.component.requires||{}).map(([name,version])=>[p.component.id,name,version]):[]),...(p.tools||[]).flatMap(t=>Object.entries(t.requires||{}).map(([name,version])=>[t.alias,name,version]))];
 const contracts=catProperties({Inputs:p.inputs,Outputs:p.outputs,Steps:p.steps,Limits:p.limits});
 const jobs=(p.jobs||[]).map(j=>[loomJobInventoryLink(j.id),j.logical_job&&j.logical_job!==j.id?loomJobInventoryLink(j.logical_job):'—',esc(j.state),esc(j.category||'미기록'),esc(j.execution_mode||'미기록'),j.pin_matches===true?'일치':j.pin_matches===false?'다른 pin':'미기록']);
 const observed=p.observed_usage?.categories?.length||p.observed_usage?.execution_modes?.length;
 return loomHubTabs()+heading('PROFILE',esc(loomProfileName(p)),'','<button class="pa-button" data-extension-home>Profiles</button>')+catStatus()
  +`<div class="pa-profile-meta"><div class="pa-profile-purposes" data-profile-declared>${loomProfilePurpose(p)}</div><div class="pa-extension-card-head">${loomProfileUsage(p)?.reuses?`<span>재사용 ${loomProfileUsage(p).reuses}</span>`:''}${loomExtensionByline(entity)}</div></div>`
  +loomHubIO(p)+`<div class="pa-hub-actions">${loomHubProfilePicker(p)}<button class="pa-button" data-profile-prepare>Prepare Job</button><button class="pa-button" data-profile-compare>Compare</button>${catalogData.capabilities?.curate?'<button class="pa-button" data-hub-edit>Edit</button>':''}</div>`
  +panel('실행 흐름',`${software.length?`<div class="pa-profile-software">${p.component?software[0]:''}${p.tools?.length?`<section class="pa-profile-tools"><h3>Tools</h3><div>${software.slice(p.component?1:0).join('')}</div></section>`:''}</div>`:''}<ol class="pa-profile-flow">${stages||'<li class="pa-subtle">Stage 미기록</li>'}</ol>`)
  +(requirements.length?`<details class="pa-execution-options"><summary>소프트웨어 요구사항</summary>${catTable(['소프트웨어','Dependency','Version'],requirements.map(row=>row.map(esc)))}</details>`:'')
  +`<details class="pa-execution-options"><summary>입출력 · 실행 설정</summary>${contracts}</details>`
  +`<details class="pa-execution-options"><summary>실행 기록 · ${(p.jobs||[]).length}</summary>${observed?`<div class="pa-profile-purposes" data-profile-observed title="실제 Job에 기록된 분류">${loomProfilePurpose(p,true)}</div>`:''}${jobs.length?catTable(['실행 기록','논리 Job','상태','용도','모드','현재 pin'],jobs):'<p class="pa-empty">사용 기록이 없습니다.</p>'}</details>`
  +`<details class="pa-execution-options"><summary>등록 정보</summary>${catProperties({Kind:p.kind,SHA256:p.sha256||'—',Namespace:p.namespace,'등록 경로':p.origin.type,Description:p.description||'—'})}</details>`;
}

function loomRuntimeCard(entity){
 const item=entity.loom_runtime,manifest=item.manifest;
 return `<article class="pa-extension-card" data-extension-card="${esc(entity.id)}"><div class="pa-extension-card-head">${loomExtensionBadges(entity,item.revision)}<span>${esc(item.legacy?'Unverified':manifest.reconciliation?'Metadata checked':manifest.validation?.status==='passed'?'Checks passed':'Unverified')}</span></div><h3>${catLink(entity.id,item.curation?.label||entity.label)}</h3><div class="pa-profile-dependencies">${loomHubHighlights({...item,dependencies:manifest.dependencies})}</div><div class="pa-extension-card-facts"><span>Profiles ${catalogData.presentation_view==='research'&&entity.research_context?entity.research_context.profiles.length:(item.used_by_profiles||[]).length}</span>${item.usage_stats?.executed_jobs?`<span>Pinned Jobs ${item.usage_stats.executed_jobs}</span>`:''}</div></article>`;
}
function loomRuntimeDetail(entity){
 const item=entity.loom_runtime,manifest=item.manifest,policy=manifest.runtime||{},filter=state.inventoryOrigin||'managed',query=(state.inventoryQuery||'').toLowerCase();
 const deps=(manifest.dependencies||[]).filter(d=>(filter==='all'||loomHubOrigin(d)===filter)&&[d.name,d.version,d.scope].join(' ').toLowerCase().includes(query));
 const dependencies=catTable(['Package','Version','Scope','Origin'],deps.map(d=>[esc(d.name),esc(d.version),esc(d.scope||'—'),esc(loomHubOrigin(d))]));
 const profiles=(item.used_by_profiles||[]).map(p=>`<div class="pa-row">${catLink('profile:'+p.kind,loomProfileName(catEntity('profile:'+p.kind)?.loom_execution_profile||{kind:p.kind,label:p.kind}))}<span>${esc(p.stages.join(', '))}</span></div>`).join('')||'<p class="pa-empty">No linked profiles</p>';
 const jobs=(item.executed_jobs||[]).map(loomJobInventoryLink).join(' · ')||'—';
 const versions=loomExtensionRows().filter(e=>e.kind==='runtime'&&e.loom_runtime.id===item.id);
 return loomHubTabs()+heading('RUNTIME',esc(item.curation?.label||entity.label),'','<button class="pa-button" data-extension-home>Runtimes</button>')+catStatus()
  +`<div class="pa-hub-actions">${catalogData.capabilities?.curate&&item.revision?'<button class="pa-button" data-hub-edit>Edit</button>':''}</div>`
  +panel('Dependencies',`<div class="pa-profile-dependencies">${loomHubHighlights({...item,dependencies:manifest.dependencies})}</div>`)
  +panel('Profiles',profiles)
  +`<details class="pa-execution-options"><summary>Pinned Job executions</summary>${jobs}</details>`
  +`<details class="pa-execution-options"><summary>Revisions</summary>${versions.map(e=>catLink(e.id,'r'+(e.loom_runtime.revision||'—')+(e.loom_runtime.aliases?.length?' · Current':''))).join(' · ')}</details>`
  +`<details class="pa-execution-options"><summary>Checks</summary><pre>${esc(JSON.stringify({installation:manifest.installation,validation:manifest.validation},null,2))}</pre></details>`
  +`<details class="pa-execution-options" data-runtime-inventory><summary>Installed inventory</summary><div class="pa-extension-toolbar"><input type="search" data-inventory-query placeholder="Search packages" aria-label="Inventory search" value="${esc(state.inventoryQuery||'')}"><select data-inventory-origin aria-label="Inventory origin">${Object.entries({managed:'Managed directory',image:'Image',external:'External mounts',deprecated:'Deprecated',unknown:'Unclassified',all:'All'}).map(([value,label])=>`<option value="${value}" ${filter===value?'selected':''}>${label}</option>`).join('')}</select></div>${dependencies}</details>`
  +`<details class="pa-execution-options"><summary>Runtime settings</summary>${catProperties({ID:item.id,Revision:item.revision??'—',SHA256:item.sha256||'—',Image:policy.image,Network:policy.network||'none',CPU:policy.cpus,Memory:policy.memory})}</details>`;
}

function loomExtensionDetail(entity){
 const item=entity.loom_extension,versions=loomExtensionVersions(entity),snapshot=versions.find(v=>v.revision===(state.extensionRevision??entity.loom_requested_revision))||versions[0];
 const classification=snapshot.classification||(snapshot.revision===item.revision?item.classification:null);
 const type=loomExtensionTypes[entity.kind],pins=snapshot.source_pins||{},profiles=snapshot.profiles||[],consumers=snapshot.used_by||[];
 const filter=state.extensionSourceQuery||'',files=Object.entries(pins).filter(([name])=>name.toLocaleLowerCase().includes(filter.toLocaleLowerCase()));
 const history=versions.map(v=>`<button data-extension-select="${esc(v.revision)}" aria-pressed="${v.revision===snapshot.revision}"><strong>r${esc(v.revision)}${v.revision===item.revision?'<small>Latest</small>':''}</strong><span>${Object.keys(v.source_pins||{}).length} files${entity.kind==='component'?' · '+(v.profiles||[]).length+' profiles':''}</span></button>`).join('');
 const linked=snapshot.connected_profiles;
 const relations=entity.kind==='component'?panel('Execution profiles <small>'+profiles.length+'</small>',profiles.map(loomExtensionProfile).join('')||'<div class="pa-empty">이 revision에는 실행 Profile이 없습니다.</div>')
  :panel('이 버전을 사용하는 Profiles <small>'+consumers.length+'</small>',consumers.length?consumers.map(consumer=>{
   const target=catEntity(consumer.component);
   return `<article class="pa-extension-consumer"><b>${esc(consumer.kind)}</b><span>alias · ${esc(consumer.alias)} · ${esc(consumer.owner)}</span>${target?loomExtensionOpenLink(target,Number(consumer.kind.match(/\.r(\d+)$/)?.[1]),target.label):'<small>독립 Profile</small>'}</article>`;
  }).join(''):'<div class="pa-empty">이 revision을 참조하는 등록 Profile이 없습니다. </div>');
 const source=panel('Source files <small>'+Object.keys(pins).length+'</small>',`<input class="pa-extension-file-search" type="search" data-extension-source-query aria-label="소스 파일 검색" placeholder="파일 이름 검색" value="${esc(filter)}">`
  +(files.length?catTable(['File','SHA-256'],files.slice(0,100).map(([name,hash])=>[`<code>${esc(name)}</code>`,`<code class="pa-extension-hash">${esc(hash)}</code>`])):'<div class="pa-empty">표시할 소스 파일이 없습니다.</div>')+(files.length>100?`<p class="pa-subtle">${files.length}개 중 100개 표시 · 파일 이름으로 범위를 좁힐 수 있습니다.</p>`:''));
 const connected=linked?panel('Profiles',linked.length?linked.map(p=>`<article class="pa-extension-consumer"><b>${catLink('profile:'+p.kind,loomProfileName(catEntity('profile:'+p.kind)?.loom_execution_profile||{kind:p.kind,label:p.kind}))}</b><span>${esc([...(p.aliases||[]),...(p.stages||[])].join(' · '))}</span></article>`).join(''):'<div class="pa-empty">No linked profiles</div>'):'';
 const metadata=snapshot.revision===item.revision?item.manifest:{revision:snapshot.revision,source_pins:pins,origin:snapshot.origin,profiles};
 return loomHubTabs()+heading(type.label.toUpperCase(),esc(classification?.label||snapshot.label||entity.label),'','<button class="pa-button" data-extension-home>목록으로</button>')+catStatus()
  +loomSoftwareTags(classification)
  +`<div class="pa-extension-detail-badges">${loomExtensionBadges(entity,snapshot.revision)}<label>Revision <select data-extension-version aria-label="표시할 revision">${versions.map(v=>`<option value="${esc(v.revision)}" ${v.revision===snapshot.revision?'selected':''}>r${esc(v.revision)}${v.revision===item.revision?' · latest':''}</option>`).join('')}</select></label></div>`
  +`<div class="pa-extension-detail-grid"><main>${connected}${linked?`<details class="pa-execution-options"><summary>Execution contracts</summary>${relations}</details>`:relations}<details class="pa-execution-options"><summary>Source files</summary>${source}</details>${panel('등록 정보',`<details><summary>Metadata 확인</summary><pre>${esc(JSON.stringify(metadata,null,2))}</pre></details>`)}</main><aside>${panel('Revision history',history)}<details class="pa-execution-options"><summary>등록 정보</summary>${panel('Identity',catProperties({책임자:entity.owner_profile?.display_name||item.owner_name||item.owner,Namespace:item.owner,ID:item.id||entity.id,Role:type.role,'Source files':Object.keys(pins).length,...(snapshot.sha256?{SHA256:snapshot.sha256}:{})}))}</details>${snapshot.origin?panel('Origin',catProperties(snapshot.origin)):''}</aside></div>`;
}
function loomExtensionView(target){const pane=target.closest('[data-pane]');if(pane)ws.active=pane.dataset.pane;state=activeTab().view;return state;}
function loomExtensionNavigate(next){loomHubNavigate(next);}
root.addEventListener('click',event=>{
 const button=event.target.closest('[data-extension-kind],[data-extension-clear],[data-extension-select],[data-extension-home],[data-extension-open],[data-software-tag]');if(!button)return;
 const view=loomExtensionView(button);
 if(button.hasAttribute('data-software-tag'))view.softwareTag=view.softwareTag===button.dataset.softwareTag?'':button.dataset.softwareTag;
 if(button.hasAttribute('data-extension-kind')){return loomExtensionNavigate({page:({execution_profile:'profiles',component:'components',tool:'tools',runtime:'runtimes'})[button.dataset.extensionKind]||'tools',entity:null});}
 if(button.hasAttribute('data-extension-clear'))Object.assign(view,{extensionQuery:'',extensionOwner:'',extensionKind:'all',softwareTag:''});
 if(button.hasAttribute('data-extension-select')){view.extensionRevision=Number(button.dataset.extensionSelect);view.extensionSourceQuery='';}
 if(button.hasAttribute('data-extension-home'))return loomExtensionNavigate({page:({execution_profile:'profiles',component:'components',runtime:'runtimes',tool:'tools'})[catEntity(view.entity)?.kind]||'tools',entity:null,hubContext:null,extensionKind:'all'});
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
helpDescriptions.profiles='Profile별 용도와 stage별 실행환경·소프트웨어 조합을 확인합니다.';
helpDescriptions.tools='';helpDescriptions.components='';helpDescriptions.runtimes='';
const loomSoftwareRender=render;
render=function(){
 // Restore pre-inventory document tabs without resetting their selected revision.
 for(const pane of ws.panes)for(const tab of pane.tabs){
  const legacy=({component:'components',runtime:'runtimes',execution_profile:'profiles'})[tab.view.extensionKind];
  if(!tab.view.entity&&tab.view.page==='tools'&&legacy){const previous=tab.id;tab.view.page=legacy;delete tab.view.extensionKind;tab.id=tabKey(tab.view);if(pane.active===previous)pane.active=tab.id;}
  if(catEntity(tab.view.entity)?.kind==='execution_profile')tab.view.page='profiles';
  else if(/^(tool|component):[^:]+:[^:]+:r\d+$/.test(tab.view.entity||'')||['tool','component','runtime'].includes(catEntity(tab.view.entity)?.kind))tab.view.page=({component:'components',runtime:'runtimes',tool:'tools'})[catEntity(tab.view.entity)?.kind]||'tools';
 }
 return loomSoftwareRender();
};
// Preserve the selected Profile while following exact Component/Tool/Runtime links.
root.addEventListener('click',event=>{
 const button=event.target.closest('[data-hub-page]'),link=event.target.closest('[data-entity]');
 if(button){event.preventDefault();event.stopImmediatePropagation();loomHubNavigate({page:button.dataset.hubPage,entity:null,hubContext:null});return;}
 if(link&&!event.metaKey&&!event.ctrlKey&&!event.shiftKey&&event.button===0&&loomExtensionTypes[catEntity(link.dataset.entity)?.kind]){
  event.preventDefault();event.stopImmediatePropagation();loomHubNavigate(catView(link.dataset.entity));
 }
},true);
root.addEventListener('change',event=>{
 if(event.target.matches('[data-profile-revision]')){const e=event.target;loomExtensionView(e);loomHubNavigate(catView(e.value));}
 if(event.target.matches('[data-hub-archive]')){const e=event.target;loomExtensionView(e).hubArchive=e.checked;render();persist();}
 if(event.target.matches('[data-inventory-origin]')){const e=event.target;loomExtensionView(e).inventoryOrigin=e.value;render();persist();root.querySelector('[data-runtime-inventory]')?.setAttribute('open','');}
});
root.addEventListener('input',event=>{
 if(!event.target.matches('[data-inventory-query]'))return;const e=event.target,value=e.value,start=e.selectionStart;loomExtensionView(e).inventoryQuery=value;render();root.querySelector('[data-runtime-inventory]')?.setAttribute('open','');const next=root.querySelector('[data-inventory-query]');next?.focus();next?.setSelectionRange(start,start);
});
root.addEventListener('click',async event=>{
 const button=event.target.closest('[data-profile-prepare],[data-profile-compare],[data-hub-edit]');if(!button)return;
 loomExtensionView(button);const entity=catEntity(state.entity),p=entity?.loom_execution_profile;
 if(button.hasAttribute('data-profile-prepare')&&p){
  const dialog=loomHubDialog('Prepare Job',`<form data-profile-draft><div class="pa-hub-fields">${['key','owner','notify'].map(k=>`<label>${k}<input name="${k}" required autocomplete="off"></label>`).join('')}<label>Category<select name="category" required><option value="">Select category</option>${Object.entries(loomProfileUsageLabels().categories).map(([k,v])=>`<option value="${k}" ${(p.usage?.categories||p.observed_usage?.categories||[])[0]===k?'selected':''}>${esc(v)}</option>`).join('')}</select></label><label>Mode<select name="mode" required><option value="">Select mode</option>${Object.entries(loomProfileUsageLabels().modes).map(([k,v])=>`<option value="${k}" ${(p.usage?.execution_modes||p.observed_usage?.execution_modes||[])[0]===k?'selected':''}>${esc(v)}</option>`).join('')}</select></label></div><label>Parameters<textarea name="params" rows="8">${esc(JSON.stringify(p.defaults||{},null,2))}</textarea></label><details><summary>Input schema</summary><pre>${esc(JSON.stringify(p.params_schema||{},null,2))}</pre></details><button class="pa-button" type="submit">Check</button><button class="pa-button" type="button" data-draft-copy disabled>Copy MCP request</button></form><pre data-profile-checks></pre>`);
  let checked=null;const form=dialog.querySelector('form'),copy=dialog.querySelector('[data-draft-copy]'),status=dialog.querySelector('[data-hub-result]');
  form.addEventListener('input',()=>{checked=null;copy.disabled=true;});form.onsubmit=async ev=>{
   ev.preventDefault();checked=null;copy.disabled=true;
   try{
    const params=JSON.parse(form.elements.params.value),result=await loomHubAction('inspect_profile',{kind:p.kind,params});
    dialog.querySelector('[data-profile-checks]').textContent=result.checks.map(x=>[x.stage,x.software,x.name,x.version?x.version+' → '+(x.actual_versions||[]).join(', '):null,x.status].filter(Boolean).join(' · ')).join('\n');
    status.textContent=result.params.status==='valid'?'Parameters valid':result.params.message||'Parameters not checked';
    if(result.params.status==='valid'){
     checked={jsonrpc:'2.0',id:1,method:'tools/call',params:{name:'jobs',arguments:{action:'submit',arguments:{...result.draft,key:form.elements.key.value,owner:form.elements.owner.value,notify:form.elements.notify.value,category:form.elements.category.value,execution_mode:form.elements.mode.value}}}};copy.disabled=false;
    }
   }catch(error){status.textContent=error.message;}
  };
  form.querySelector('button[type=submit]').onclick=ev=>{ev.preventDefault();if(form.reportValidity())form.onsubmit(ev);};
  copy.onclick=()=>{if(checked)parent.postMessage({type:'loom-doc-copy',text:JSON.stringify(checked,null,2)},'*');};
 }else if(button.hasAttribute('data-profile-compare')&&p){
  const rows=loomExtensionRows().filter(e=>e.kind==='execution_profile'&&e.id!==entity.id);
  const dialog=loomHubDialog('Compare Profiles',`<select data-compare-profile aria-label="Compare with">${rows.map(e=>`<option value="${esc(e.loom_execution_profile.kind)}">${esc(loomProfileName(e.loom_execution_profile))} · ${esc(e.loom_execution_profile.origin.revision||'—')}</option>`).join('')}</select><button class="pa-button" data-compare-run>Compare</button><pre data-compare-result></pre>`);
  dialog.querySelector('[data-compare-run]').onclick=async()=>{try{const result=await loomHubAction('compare_profiles',{left:p.kind,right:dialog.querySelector('select').value});dialog.querySelector('[data-compare-result]').textContent=result.execution_equal?'Same execution definition':result.changes.map(x=>x.path+' · '+x.change).join('\n');}catch(error){dialog.querySelector('[data-hub-result]').textContent=error.message;}};
 }else if(button.hasAttribute('data-hub-edit')&&catalogData.capabilities?.curate){
  const runtime=entity?.loom_runtime;if(!p&&!runtime)return;
  const current=p?.curation||runtime.curation||{};
  const selectors=runtime?(runtime.manifest.dependencies||[]).filter(d=>!d.deprecated&&d.kind!=='system'&&d.scope!=='isaac-extension'):[];
  const unique=[...new Map(selectors.map(d=>[JSON.stringify({name:d.name,...(d.scope?{scope:d.scope}:{})}),d])).entries()];
  const dialog=loomHubDialog('Edit',`<form data-hub-editor><label>Name<input name="label" maxlength="100" required value="${esc(current.label||p?.label||entity.label)}"></label>${p?`<div class="pa-hub-fields"><fieldset><legend>Purpose</legend>${Object.entries(loomProfileUsageLabels().categories).map(([k,v])=>`<label><input type="checkbox" name="categories" value="${esc(k)}" ${(current.categories||p.usage?.categories||[]).includes(k)?'checked':''}>${esc(v)}</label>`).join('')}</fieldset><fieldset><legend>Mode</legend>${Object.entries(loomProfileUsageLabels().modes).map(([k,v])=>`<label><input type="checkbox" name="modes" value="${esc(k)}" ${(current.execution_modes||p.usage?.execution_modes||[]).includes(k)?'checked':''}>${esc(v)}</label>`).join('')}</fieldset></div><label>Group<select name="group">${['research','platform-check','other'].map(k=>`<option ${k===(current.group||p.group)?'selected':''}>${k}</option>`).join('')}</select></label><label><input type="checkbox" name="preferred" ${current.preferred?'checked':''}>Preferred</label><details><summary>Tool roles</summary>${p.tools.map(t=>`<fieldset data-tool-role="${esc(t.alias)}"><legend>${esc(t.classification?.label||t.id)}</legend><input data-role-name aria-label="Role" value="${esc(current.tool_roles?.[t.alias]?.role||'')}">${p.stages.map(s=>`<label><input type="checkbox" value="${esc(s.stage)}" ${(current.tool_roles?.[t.alias]?.stages||[]).includes(s.stage)?'checked':''}>${esc(s.stage)}</label>`).join('')}</fieldset>`).join('')}</details><details><summary>Dependencies by stage</summary>${p.stages.map(stage=>{
 const runtime=catEntity(stage.environment.entity)?.loom_runtime,custom=Object.hasOwn(current.stage_dependencies||{},stage.stage),chosen=current.stage_dependencies?.[stage.stage]||[];
 const deps=(runtime?.manifest.dependencies||[]).filter(d=>!d.deprecated&&d.kind!=='system'&&d.scope!=='isaac-extension');
 const selectors=[...new Map(deps.map(d=>[JSON.stringify({name:d.name,...(d.scope?{scope:d.scope}:{})}),d])).entries()];
 return `<fieldset data-stage-dependencies="${esc(stage.stage)}"><legend>${esc(stage.stage)}</legend><label><input type="checkbox" data-stage-custom ${custom?'checked':''} ${runtime?'':'disabled'}>Custom</label><div>${selectors.map(([key,d])=>`<label><input type="checkbox" data-stage-dependency value="${esc(key)}" ${chosen.some(x=>JSON.stringify(x)===key)?'checked':''} ${custom?'':'disabled'}>${esc(d.name)} ${esc(d.version)} <small>${esc(d.scope||'')}</small></label>`).join('')||'—'}</div></fieldset>`;
 }).join('')}</details>`:`<fieldset><legend>Representative dependencies</legend>${unique.map(([key,d])=>`<label><input type="checkbox" name="dependencies" value="${esc(key)}" ${(current.primary_dependencies||runtime.manifest.primary_dependencies||[]).some(x=>JSON.stringify(x)===key)?'checked':''}>${esc(d.name)} ${esc(d.version)} <small>${esc(d.scope||'')}</small></label>`).join('')}</fieldset>`}<label><input type="checkbox" name="archived" ${current.state==='archived'?'checked':''}>Archived</label><button class="pa-button" type="submit">Save</button></form>`);
  const form=dialog.querySelector('form');form.addEventListener('change',event=>{if(event.target.matches('[data-stage-custom]'))event.target.closest('fieldset').querySelectorAll('[data-stage-dependency]').forEach(e=>e.disabled=!event.target.checked);});form.onsubmit=async ev=>{ev.preventDefault();try{
   const args={type:p?'profile':'runtime',id:p?.kind||runtime.id,...(!p?{revision:runtime.revision}:{}),label:form.elements.label.value,state:form.elements.archived.checked?'archived':'active',evidence:'Edited through the authenticated Runtimes workspace.'};
   if(p){args.categories=[...form.querySelectorAll('[name=categories]:checked')].map(e=>e.value);args.execution_modes=[...form.querySelectorAll('[name=modes]:checked')].map(e=>e.value);args.group=form.elements.group.value;args.preferred=form.elements.preferred.checked;args.stage_dependencies=Object.fromEntries([...form.querySelectorAll('[data-stage-dependencies]')].filter(e=>e.querySelector('[data-stage-custom]').checked).map(e=>[e.dataset.stageDependencies,[...e.querySelectorAll('[data-stage-dependency]:checked')].map(c=>JSON.parse(c.value))]));args.tool_roles=Object.fromEntries([...form.querySelectorAll('[data-tool-role]')].filter(e=>e.querySelector('[data-role-name]').value.trim()).map(e=>[e.dataset.toolRole,{role:e.querySelector('[data-role-name]').value.trim(),stages:[...e.querySelectorAll('input[type=checkbox]:checked')].map(c=>c.value)}]));}
   else args.primary_dependencies=[...form.querySelectorAll('[name=dependencies]:checked')].map(e=>JSON.parse(e.value));
   await loomHubAction('curate',args);dialog.close();dialog.remove();
  }catch(error){dialog.querySelector('[data-hub-result]').textContent=error.message;}};
  form.querySelector('button[type=submit]').onclick=ev=>{ev.preventDefault();if(form.reportValidity())form.onsubmit(ev);};
 }
});
