// Reversible presentation state. Accepted Job pins and source files are preserved.
function loomRegistryTarget(e){
 if(e?.lifecycle?.target&&!e.loom_requested_revision)return e.lifecycle.target;
 if(e?.loom_extension){const x=e.loom_extension;return {kind:e.kind,owner:x.owner,id:x.id,revision:e.loom_requested_revision||x.revision};}
 if(e?.loom_execution_profile)return {kind:'profile',id:e.loom_execution_profile.kind,revision:0};
 if(e?.loom_runtime?.revision>0)return {kind:'runtime',id:e.loom_runtime.id,revision:e.loom_runtime.revision};
 return null;
}
function loomRegistryState(e){return e.lifecycle?.state||e.loom_execution_profile?.curation?.state||e.loom_runtime?.curation?.state||'active';}
function loomRegistryBadge(e){const s=loomRegistryState(e);return s==='active'?'':`<span class="pa-extension-revision" data-registry-status="${esc(s)}">${esc(s==='archived'?'Archived':'Deprecated')}</span>`;}
function loomRegistryVisible(e){const filter=state.registryFilter||'current',s=loomRegistryState(e);return filter==='all'||(filter==='archived'?s==='archived':s!=='archived');}
function loomRegistryFilter(value=state.registryFilter||'current',attribute='data-registry-filter'){
 return `<select ${attribute} aria-label="Registry status"><option value="current" ${value==='current'?'selected':''}>Active</option><option value="archived" ${value==='archived'?'selected':''}>Archived</option><option value="all" ${value==='all'?'selected':''}>All</option></select>`;
}
function loomRegistryArgs(filter){return filter==='archived'?{lifecycle_state:'archived'}:{include_archived:filter==='all'};}
function loomRegistryButtons(t,s='active',protectedItem=false){
 if(!t||protectedItem)return '';
 const encoded=esc(encodeURIComponent(JSON.stringify(t)));
 return `<button class="pa-button" data-registry-target="${encoded}" data-registry-state="${s==='archived'?'active':'archived'}">${s==='archived'?'Restore':'Archive'}</button>`+(s==='active'?`<button class="pa-button" data-registry-target="${encoded}" data-registry-state="deprecated">Deprecate</button>`:s==='deprecated'?`<button class="pa-button" data-registry-target="${encoded}" data-registry-state="active">Restore</button>`:'');
}
const loomRegistryEntityActions=paiEntityActions;
paiEntityActions=function(id){
 const e=catEntity(id),t=loomRegistryTarget(e),buttons=loomRegistryButtons(t,loomRegistryState(e||{}),!catalogData.capabilities?.curate||e?.lifecycle?.builtin||e?.owner_kind==='built-in');
 return (e?loomRegistryBadge(e):'')+loomRegistryEntityActions(id).replace('</div></details>',buttons+'</div></details>');
};
async function loomRegistryPreview(targets,desired){
 const dialog=loomHubDialog(desired==='active'?'Restore registrations':desired==='deprecated'?'Deprecate registrations':'Archive registrations','<div data-registry-preview>…</div>');
 let selected=[...targets],preview=null,sequence=0;
 async function load(){
  const current=++sequence;
  dialog.querySelectorAll('[data-registry-apply] button').forEach(b=>b.disabled=true);
  try{
   const result=await loomHubAction('inspect_lifecycle',{targets:selected,state:desired});if(!dialog.isConnected||current!==sequence)return;preview=result;
   const records=result.targets,related=result.related||[],jobs=result.jobs||[],canApply=records.every(r=>r.can_manage&&!r.builtin);
   dialog.querySelector('[data-registry-preview]').innerHTML=`<div class="pa-registry-selection">${records.map(r=>`<label><input type="checkbox" data-registry-selected value="${esc(encodeURIComponent(JSON.stringify(r.target)))}" checked ${targets.some(t=>Object.entries(t).every(([k,v])=>r.target[k]===v))?'disabled':''}><span>${esc(r.label||r.target.id)} <small>${esc(r.target.kind)} · ${esc(r.lifecycle.state)}${r.builtin?' · Built-in':''}</small></span></label>`).join('')}${related.length?'<h3>Related registrations</h3>'+related.map(r=>`<label><input type="checkbox" data-registry-selected value="${esc(encodeURIComponent(JSON.stringify(r.target)))}" ${!r.can_manage||r.builtin?'disabled':''}><span>${esc(r.label||r.target.id)} <small>${esc(r.target.kind)}${r.builtin?' · Built-in':''}</small></span></label>`).join(''):''}</div>${result.related_truncated?'<p>More related registrations exist. Select a narrower batch to inspect them.</p>':''}<details><summary>Job usage · ${result.job_count??jobs.length}${jobs.some(j=>j.active)?' · In use':''}</summary>${catTable(['Job','State'],jobs.map(j=>[esc(j.id),esc(j.state)]))}${result.jobs_truncated?'<p>Showing the first 100 Jobs.</p>':''}</details><p class="pa-subtle">Definitions, files and Job history are preserved. Related registrations change only when selected.</p>${records.some(r=>r.history?.length)?'<details><summary>Change history</summary>'+catTable(['Registration','State','Reason'],records.flatMap(r=>(r.history||[]).map(h=>[esc(r.label||r.target.id),esc(h.previous+' → '+h.state),esc(h.reason)])))+'</details>':''}<form data-registry-apply><label>Reason<input name="reason" maxlength="2000" required></label><button class="pa-button" type="submit" ${canApply?'':'disabled'}>${desired==='active'?'Restore':desired==='deprecated'?'Deprecate':'Archive'} ${records.length}</button></form>`;
   let submitted=null;const form=dialog.querySelector('[data-registry-apply]');
   form.onsubmit=async event=>{
    event.preventDefault();const button=form.querySelector('button');if(button.disabled)return;button.disabled=true;
    try{
     const reason=new FormData(form).get('reason').trim();if(!reason)throw Error('Enter a reason');
     submitted ||= {targets:JSON.parse(JSON.stringify(selected)),state:desired,expected_digest:preview.digest,reason,key:'registry-'+crypto.randomUUID()};
     const receipt=await loomHubAction('set_lifecycle',submitted);
     for(const e of catalogData.all_entities||catalogData.entities){const t=loomRegistryTarget(e);if(t&&receipt.changed.some(x=>Object.entries(x).every(([k,v])=>t[k]===v)))e.lifecycle={...(e.lifecycle||{}),state:desired,target:t};}
     dialog.close();dialog.remove();
     // Refresh from authority, including any open details, before applying list filters.
     loomCatalogRevision=null;loomCatalogPages.clear();catalogRefreshViews();
    }catch(error){dialog.querySelector('[data-hub-result]').textContent=error.message;button.disabled=false;}
   };
   // The workspace sandbox intentionally disallows native form submission.
   form.querySelector('button').onclick=event=>{event.preventDefault();if(form.reportValidity())form.onsubmit(event);};
   form.onkeydown=event=>{if(event.key==='Enter'&&event.target.matches('input')){event.preventDefault();if(form.reportValidity())form.onsubmit(event);}};
  }catch(error){if(dialog.isConnected&&current===sequence)dialog.querySelector('[data-hub-result]').textContent=error.message;}
 }
 dialog.addEventListener('change',event=>{
  if(!event.target.matches('[data-registry-selected]'))return;
  selected=[...dialog.querySelectorAll('[data-registry-selected]:checked')].map(x=>JSON.parse(decodeURIComponent(x.value)));load();
 });
 await load();
}
root.addEventListener('click',event=>{const b=event.target.closest('[data-registry-target]');if(!b)return;event.preventDefault();loomRegistryPreview([JSON.parse(decodeURIComponent(b.dataset.registryTarget))],b.dataset.registryState);});
root.addEventListener('change',event=>{if(!event.target.matches('[data-registry-filter]'))return;loomExtensionView(event.target).registryFilter=event.target.value;render();persist();});

async function loomRegistryBrowser(initialKind){
 const choices={metadata_template:['Metadata templates','metadata_templates','get_metadata_template'],report_contract:['Report contracts','report_contracts','get_report_contract'],report_definition:['Report definitions','report_definitions','get_report_definition']};
 const dialog=loomHubDialog(choices[initialKind][0],`<div class="pa-extension-toolbar"><select data-registry-kind aria-label="Registry">${Object.entries(choices).map(([k,v])=>`<option value="${k}" ${k===initialKind?'selected':''}>${v[0]}</option>`).join('')}</select>${loomRegistryFilter('current','data-registry-browser-filter')}<label class="pa-hub-toggle"><input type="checkbox" data-registry-browser-history>History</label></div><div data-registry-list data-template-list data-report-contract-list></div><div data-registry-detail data-template-details data-report-contract-detail></div>`);
 let sequence=0,detailSequence=0,offset=0,rows=[],loading=false;
 async function load(reset=false){
  if(loading&&!reset)return;loading=true;const current=++sequence;
  if(reset){rows=[];offset=0;detailSequence++;dialog.querySelector('[data-registry-detail]').innerHTML='';dialog.querySelector('[data-registry-list]').textContent='…';}
  const kind=dialog.querySelector('[data-registry-kind]').value,filter=dialog.querySelector('[data-registry-browser-filter]').value;
  try{
   const page=await loomSearchAction('results',choices[kind][1],{offset,limit:50,include_revisions:filter==='archived'||dialog.querySelector('[data-registry-browser-history]').checked,...loomRegistryArgs(filter)});if(!dialog.isConnected||current!==sequence)return;
   rows.push(...page.items);offset=page.next_offset;
   dialog.querySelector('[data-registry-list]').innerHTML=catTable(['Name','Kind','Revision','State'],rows.map((r,i)=>[`<button class="pa-button" data-registry-record="${i}" ${kind==='metadata_template'?`data-template-id="${esc(r.id)}" data-template-revision="${r.revision}"`:`data-report-contract-id="${esc(r.id)}" data-report-contract-revision="${r.revision}"`}>${esc(r.label||r.id)}</button>`,esc(r.output_kind||kind.replace('report_','')),esc(r.revision),esc(r.origin==='builtin'?'Built-in':r.lifecycle?.state||'active')]))+(offset!=null?'<button class="pa-button" data-registry-more>More</button>':'');
  }catch(error){if(dialog.isConnected&&current===sequence)dialog.querySelector('[data-hub-result]').textContent=error.message;}
  finally{if(current===sequence)loading=false;}
 }
 dialog.addEventListener('change',event=>{if(event.target.matches('[data-registry-kind],[data-registry-browser-filter],[data-registry-browser-history]'))load(true);});
 dialog.addEventListener('click',async event=>{
  if(event.target.closest('[data-registry-more]'))return load();
  const b=event.target.closest('[data-registry-record]');if(!b)return;const current=++detailSequence,kind=dialog.querySelector('[data-registry-kind]').value,r=rows[Number(b.dataset.registryRecord)];
  try{
   const record=await loomSearchAction('results',choices[kind][2],{id:r.id,revision:r.revision});if(!dialog.isConnected||current!==detailSequence)return;
   const t={kind,id:record.id,revision:record.revision};
   const definition=record.definition,fields=definition.fields||[],requirements=definition.effective_requirements||definition.requirements||[];
   const content=kind==='metadata_template'?catTable(['Field','Format','Unit'],fields.map(f=>[esc(f.label),esc(f.format),esc(f.unit||'—')])):kind==='report_contract'?catTable(['Report','Role','Coverage'],requirements.map(r=>[esc(r.label),esc(r.role),r.required?'Required':'Optional'])):'';
   dialog.querySelector('[data-registry-detail]').innerHTML=`<h3>${esc(record.label||record.id)}</h3><div class="pa-hub-actions">${loomRegistryButtons(t,record.lifecycle?.state,record.origin==='builtin'||!catalogData.capabilities?.curate)}<button class="pa-button" data-registry-copy data-template-copy>Copy reference</button></div>${content}<details><summary>Definition</summary><pre>${esc(JSON.stringify(record.definition,null,2))}</pre></details>`;
   dialog.querySelector('[data-registry-copy]').onclick=()=>parent.postMessage({type:'loom-doc-copy',text:JSON.stringify({id:record.id,revision:record.revision,sha256:record.sha256},null,2)},'*');
  }catch(error){if(dialog.isConnected)dialog.querySelector('[data-hub-result]').textContent=error.message;}
 });
 await load();
}
