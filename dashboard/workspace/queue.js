// Queue controls use the same server snapshot and eligibility as dispatch.
let loomQueueBusy = false;
let loomQueueNotice = '';
const loomQueueEntries = () => {
  const queue=catalogData.queue||{};
  return [...(queue.entries||[]),...(queue.logical_entries||[]).filter(row=>row.entry_type==='job'||!row.executions?.length)];
};
const loomQueueEntry = id => loomQueueEntries().find(row => row.id === id);
function loomQueueReason(row) {
  if(row.reason==='executing')return row.state==='preparing'?'준비 중':'실행 중';
  return ({awaiting_runtime_binding:'Runtime 연결 대기',awaiting_plan:'실행 계획 확정 대기',awaiting_deliverables:'산출물 검증 대기',queue_paused:'운영자가 큐를 일시 중단함',ready:'실행 가능',held:'보류됨',executing:'실행 중',finalizing:'결과 처리 중',dependencies:'선행 결과 대기',
    resource_busy:'실행 슬롯 대기',retry_delay:'재시도 대기',recovery_reservation:'앞선 Job 복구 대기',recovery_waiting:'복구 응답 대기',
    recovery_claimed:'복구 진행 중',recovery_deferred:'복구 보류',recovery_exhausted:'복구 횟수 소진',recovery_required:'내부 실행 복구 필요'})[row.reason] || row.reason;
}
function loomQueuePosition(row) {
  const reason=loomQueueReason(row);
  return row.position===null?esc(reason):'#'+row.position+' · '+esc(reason);
}
function loomHistoricalWorkPanel(rows) {
 const items=rows.filter(e=>e.migration?.read_only && !queueState(e).info[3]);
 return items.length?panel('이관된 미종료 기록 · '+items.length,'<p>이관 당시 상태를 보존한 읽기 전용 기록입니다. 현재 실행 큐에는 포함되지 않습니다.</p>'+catTable(['Job','기록된 상태','Task Level','실행 시간'],items.map(e=>workRow(e,false).filter((_,i)=>i<3||i===6)))):'';
}
function loomTimingNotice(rows) {
 return rows.some(e=>(e.execution_intervals||[]).some(x=>x.may_include_waiting))
   ?'<p class="pa-queue-caption">일부 과거 기록에는 stage 시간이 없어 Attempt 경과 시간을 표시합니다. 대기 시간이 포함될 수 있습니다.</p>':'';
}
function loomQueueControls(id) {
  const row = loomQueueEntry(id);
  if (!row || !(row.can_reorder || row.can_resume)) return '';
  const disabled=loomQueueBusy?' disabled':'';
  const button=(action,text,position='')=>'<button class="pa-button" data-queue-action="'+action+'" data-queue-id="'+esc(id)+'" data-queue-position="'+position+'"'+disabled+'>'+text+'</button>';
  const resume=row.can_resume?button('resume','보류 해제'):'';
  if(!row.can_reorder)return '<div class="pa-queue-control">'+resume+'</div>';
  const choices=loomQueueEntries().filter(r=>r.id!==id && r.position!==null);
  return '<details class="pa-queue-control"><summary>'+(resume?'큐 관리':'순서 변경')+'</summary><div class="pa-queue-control-body">'
    +button('reorder','맨 앞으로','next')+button('reorder','맨 뒤로','tail')+resume
    +(choices.length?'<label>기준 Job <select data-queue-reference aria-label="기준 Job">'+choices.map(r=>'<option value="'+esc(r.id)+'">'+esc('#'+r.position+' · '+(r.owner_profile?.display_name || r.owner)+' · '+r.id)+'</option>').join('')+'</select></label>'+button('reorder','앞에 배치','before')+button('reorder','뒤에 배치','after'):'')
    +'</div></details>';
}
workItems=function(pageIds=null){
  // Resource occupancy and execution history are independent of catalog view.
  const items=(catalogData.all_entities||catalogData.entities).filter(e=>e.kind==='job'&&!e.loom_registry
    && ['work','loom_job'].includes(e.properties?.record_role));
  // Imported catalog and queue snapshots can describe the same logical Work.
  // Prefer its queue projection; both exact source records remain addressable.
  const logical=new Map();
  for(const e of items){
    if(pageIds&&!pageIds.has(e.id))continue;
    const key=e.properties?.record_role==='work'&&['queue','catalog'].includes(e.loom_history?.source)?'historical-work:'+e.loom_history.id:e.id;
    if(!logical.has(key)||e.loom_history?.source==='queue')logical.set(key,e);
  }
  const displayed=[...logical.values()];
  const known=new Set(displayed.map(e=>e.id));
  const jobs=displayed.filter(e=>!loomJobWrapper(e)&&e.reviewed_work?.visible!==0&&!e.unit?.job_id&&!e.properties?.consolidated_into);
  if(pageIds)return jobs;
  return jobs.concat(loomQueueEntries().filter(r=>!known.has(r.id)).map(r=>({
    id:r.id,label:r.label||r.id,kind:'job',loom_queue_shared:true,presentation:r.presentation,
    properties:{record_role:'loom_job',state:r.state,owner:r.owner,category:r.category,execution_mode:r.execution_mode}
  })));
};
function loomQueueCompare(a,b) {
  const rank=id=>{const r=loomQueueEntry(id);return r?(r.position??0):Number.MAX_SAFE_INTEGER;};
  return rank(a.id)-rank(b.id) || (queueTimes(b).created??0)-(queueTimes(a).created??0);
}
function loomWorkTypeBadge(type, jobId) {
  if(type==='job')return '<span class="pa-work-type" data-work-type="job">Job</span>';
  if(type==='unit'&&!jobId)return '<span class="pa-work-type" data-work-type="unit">Standalone Unit</span>';
  return '';
}
const loomQueueWorkRow=workRow;
workRow=function(entity,current){
  const row=loomQueueWorkRow(entity,current), q=loomQueueEntry(entity.id);
  if(catalogData.capabilities?.job_units){
    if(!current)row[0]=row[0].replace('<div class="pa-queue-name">','<div class="pa-queue-name">'+loomWorkTypeBadge(entity.entry_type,entity.unit?.job_id));
    if(entity.entry_type)row[0]=row[0].replace(/(\d+) Steps/,(_,n)=>entity.entry_type==='unit'?(Number(n)?n+' observed units':''):n+' Units');
  }
  const tasks=loomTaskLabels(entity);
  if(current && q){
    if(entity.loom_queue_shared)row[0]='<div class="pa-queue-name">'+esc(q.label||q.spec?.label||q.id)+'<small>'+esc(q.owner)+' · 공유 큐</small></div>';
    row[0]+='<small class="pa-queue-position">'+loomQueuePosition(q)+'</small>'+loomQueueControls(q.id);
  }
  const mode=entity.properties?.execution_mode;
  if(mode&&!(mode==='pilot'&&entity.reviewed_work?.has_regular_output))row[0]+='<small class="pa-queue-owner">'+esc(loomJobContract().execution_modes?.[mode]||mode)+'</small>';
  const owner=entity.owner_profile?.display_name || entity.properties?.owner_name || q?.owner_profile?.display_name || entity.owner || entity.properties?.owner;
  if(tasks)row[0]+='<div class="pa-queue-tasks"><span>Task ·</span><span>'+tasks+'</span></div>';
  if(owner)row[0]+='<small class="pa-queue-owner">Owner · '+esc(owner)+'</small>';
  return row;
};
const loomQueueStateBefore=queueState;
queueState=function(entity){
 const q=loomQueueEntry(entity.id),base=loomQueueStateBefore(entity);
 if(q?.held)return {raw:base.raw,info:['paused',q.state==='preparing'?'Preparing · Hold requested':'Held','Ⅱ',false]};
 if(q && ['failed','interrupted'].includes(q.state))return {raw:q.state,info:['recovery',loomQueueReason(q),'!',false]};
 return base;
};
const loomQueuePanelBefore=loomJobPanel;
loomJobPanel=function(job,entry){
 const body=loomQueuePanelBefore(job,entry),q=loomQueueEntry(job.id);
 const history=entry?.data?.queue_history||[];
 return (q?panel('Queue', '<p>'+loomQueuePosition(q)+'</p>'+loomQueueControls(job.id)):'')+body
   +(history.length?panel('순서·보류 이력',catTable(['시각','사용자','변경','사유'],history.slice(-30).reverse().map(h=>[new Date(h.recorded*1000).toLocaleString(),h.actor,h.action,h.reason||'—'].map(esc)))):'');
};
function loomCurrentQueuePanel(rows,table){
 const waiting=catalogData.capabilities?.live_queue&&!catalogData.queue;
 if(catalogData.capabilities?.job_units){
  const queue=catalogData.queue,notice=waiting||loomHomeErrors.queue?loomHomeStatus('queue'):'';
  const record=q=>catEntity(q.id)||{
   id:q.id,label:q.label||q.spec?.label||q.id,kind:'job',entry_type:q.entry_type,
   loom_queue_shared:true,properties:{state:q.state,owner:q.owner}
  };
  const jobs=(queue?.job_entries||[]).map(record).sort(loomQueueCompare),units=queue?.unit_entries||[];
  const body=units.map(unit=>{
   const active=(unit.progress?.steps||[]).filter(step=>step.state==='running').map(step=>esc(step.label)).join(' · ');
   const label=loomWorkTypeBadge('unit',unit.job_id)+catLink(unit.id,unit.unit?.label||unit.label)+loomQueueControls(unit.id);
   const gpu=unit.gpu_access?.mode ? 'GPU · '+esc(unit.gpu_access.mode) : unit.lane==='gpu'?'GPU':'CPU';
   return [label,unit.job_id?catLink(unit.job_id):'—',queueBadge(record(unit)),gpu,active||esc(loomQueueReason(unit))];
  });
  return panel('Job Queue · '+jobs.length,notice+(jobs.length?table(jobs,true):queue?'<div class="pa-empty">대기 또는 실행 중인 Job이 없습니다.</div>':''))
    +panel('Unit Queue · '+units.length,body.length?'<div class="pa-queue-table">'+catTable(['Unit','Job','상태','자원','진행'],body)+'</div>':queue?'<div class="pa-empty">대기 또는 실행 중인 Unit이 없습니다.</div>':'');
 }
 if(waiting)return panel('현재 큐',loomHomeStatus('queue'));
 if(catalogData.capabilities?.live_queue){
  const ids=new Set(loomQueueEntries().map(row=>row.id));
  rows=rows.filter(row=>ids.has(row.id));
 }
 const notice=catalogData.capabilities?.live_queue&&loomHomeErrors.queue?loomHomeStatus('queue'):'';
 return panel('현재 큐 · '+rows.length,notice+(rows.length?table(rows,true):'<div class="pa-empty">현재 등록된 Job이 없습니다.</div>'));
}
function loomQueueFeedback(){const paused=catalogData.queue?.dispatch;return (paused?.paused?'<p class="pa-queue-feedback" role="status">큐 일시 중단 · '+esc(paused.reason||'운영 작업')+'</p>':'')+'<p class="pa-queue-feedback" role="status">'+esc(loomQueueNotice)+'</p>';}
helpDescriptions.queue='대기 순서와 대기 이유를 확인합니다. 실행 중 Job은 중단하지 않으며 준비·의존성·실행 슬롯에 따라 시작 순서는 달라질 수 있습니다.';
root.addEventListener('click',event=>{
 const button=event.target.closest('[data-queue-action]');if(!button || loomQueueBusy)return;
 event.stopPropagation();
 const args={id:button.dataset.queueId};
 if(button.dataset.queueAction==='reorder'){
  args.position=button.dataset.queuePosition;
  if(['before','after'].includes(args.position))args.reference=button.closest('.pa-queue-control').querySelector('[data-queue-reference]').value;
 }
 loomQueueBusy=true;loomQueueNotice='큐 변경 중';
 for(const b of root.querySelectorAll('[data-queue-action]'))b.disabled=true;
 parent.postMessage({type:'loom-queue-action',action:button.dataset.queueAction,arguments:args},'*');
});
window.addEventListener('message',event=>{
 if(event.source!==parent || event.data?.type!=='loom-queue-result')return;
 loomQueueBusy=false;
 const {data,error}=event.data;
 loomQueueNotice=error===403?'이 Job을 변경할 권한이 없습니다.':error?'변경을 확인하지 못했습니다. 큐를 새로고침해 주세요.':Array.isArray(data?.entries)||data?.changed?'큐 변경 완료':data?.reason==='not_waiting'?'이미 실행되었거나 대기 상태가 아닙니다.':'이미 요청한 상태입니다.';
 loomEntityReads.clear();scheduleCatalogRender();
});
