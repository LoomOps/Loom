// Queue controls use the same server snapshot and eligibility as dispatch.
let loomQueueBusy = false;
let loomQueueNotice = '';
const loomQueueEntries = () => catalogData.queue?.entries || [];
const loomQueueEntry = id => loomQueueEntries().find(row => row.id === id);
function loomQueueReason(row) {
  return ({queue_paused:'운영자가 큐를 일시 중단함',ready:'실행 가능',held:'보류됨',executing:'실행 중',finalizing:'결과 처리 중',dependencies:'선행 결과 대기',
    resource_busy:'실행 슬롯 대기',retry_delay:'재시도 대기',recovery_reservation:'앞선 Job 복구 대기',recovery_waiting:'복구 응답 대기',
    recovery_claimed:'복구 진행 중',recovery_deferred:'복구 보류',recovery_exhausted:'복구 횟수 소진'})[row.reason] || row.reason;
}
function loomQueueControls(id) {
  const row = loomQueueEntry(id);
  if (!row || !(row.can_reorder || row.can_hold || row.can_resume)) return '';
  const disabled=loomQueueBusy?' disabled':'';
  const button=(action,text,position='')=>'<button class="pa-button" data-queue-action="'+action+'" data-queue-id="'+esc(id)+'" data-queue-position="'+position+'"'+disabled+'>'+text+'</button>';
  const choices=loomQueueEntries().filter(r=>r.id!==id && r.position!==null);
  return '<details class="pa-queue-control"><summary>순서·보류</summary><div class="pa-queue-control-body">'
    +(row.can_reorder?button('reorder','맨 앞으로','next')+button('reorder','맨 뒤로','tail'):'')
    +(row.can_resume?button('resume','보류 해제'):row.can_hold?button('hold','보류'):'')
    +(row.can_reorder && choices.length?'<label>기준 Job <select data-queue-reference aria-label="기준 Job">'+choices.map(r=>'<option value="'+esc(r.id)+'">'+esc('#'+r.position+' · '+(r.owner_profile?.display_name || r.owner)+' · '+r.id)+'</option>').join('')+'</select></label>'+button('reorder','앞에 배치','before')+button('reorder','뒤에 배치','after'):'')
    +'</div></details>';
}
const loomQueueWorkItems=workItems;
workItems=function(){
  const items=loomQueueWorkItems(), known=new Set(items.map(e=>e.id));
  return items.concat(loomQueueEntries().filter(r=>!known.has(r.id)).map(r=>({id:r.id,label:r.id,kind:'job',loom_queue_shared:true,
    properties:{record_role:'loom_job',state:r.state,owner:r.owner}})));
};
function loomQueueCompare(a,b) {
  const rank=id=>{const r=loomQueueEntry(id);return r?(r.position??0):Number.MAX_SAFE_INTEGER;};
  return rank(a.id)-rank(b.id) || (queueTimes(b).created??0)-(queueTimes(a).created??0);
}
const loomQueueWorkRow=workRow;
workRow=function(entity,current){
  const row=loomQueueWorkRow(entity,current), q=loomQueueEntry(entity.id);
  if(current && q){
    if(entity.loom_queue_shared)row[0]='<div class="pa-queue-name">'+esc(q.id)+'<small>'+esc(q.owner)+' · 공유 큐</small></div>';
    row[0]+='<small class="pa-queue-position">'+(q.position===null?'실행 중':'#'+q.position)+' · '+esc(loomQueueReason(q))+'</small>'+loomQueueControls(q.id);
  }
  const owner=entity.owner_profile?.display_name || entity.properties?.owner_name || q?.owner_profile?.display_name || entity.owner || entity.properties?.owner;
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
 return (q?panel('Queue', '<p>'+(q.position===null?'실행 중':'#'+q.position)+' · '+esc(loomQueueReason(q))+'</p>'+loomQueueControls(job.id)):'')+body
   +(history.length?panel('순서·보류 이력',catTable(['시각','사용자','변경','사유'],history.slice(-30).reverse().map(h=>[new Date(h.recorded*1000).toLocaleString(),h.actor,h.action,h.reason||'—'].map(esc)))):'');
};
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
 loomQueueNotice=error===403?'이 Job을 변경할 권한이 없습니다.':error?'변경을 확인하지 못했습니다. 큐를 새로고침해 주세요.':data?.changed?'큐 변경 완료':data?.reason==='not_waiting'?'이미 실행되었거나 대기 상태가 아닙니다.':'이미 요청한 상태입니다.';
 loomEntityReads.clear();scheduleCatalogRender();
});
