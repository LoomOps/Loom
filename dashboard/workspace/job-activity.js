// Include executable Jobs without counting consolidated source executions twice.
function loomJobWrapper(entity) {
 return entity.entry_type!=='job' && (entity.properties?.execution_node_count ?? entity.loom_workflow?.executions?.length ?? 0)>0;
}
function loomActivityJobs(entities) {
 return entities.filter(e=>['work','loom_job'].includes(e.properties?.record_role)
   && !loomJobWrapper(e) && !e.unit?.job_id && !e.loom_registry && !e.properties?.consolidated_into);
}
const loomActivityCreatedTime=activityCreatedTime;
activityCreatedTime=function(entity) {
 // Native publication receipts prove registration time, not research generation time.
 return entity.loom_registry?queueMillis(entity.registered_at):loomActivityCreatedTime(entity);
};
const loomActivityRecords=activityRecords;
activityRecords=function(entities) {
 const values=loomActivityRecords(entities),records=new Map(values.items.map(x=>[x.key,x]));
 for(const e of loomActivityJobs(entities).filter(e=>e.properties?.record_role==='loom_job')) {
  for(const a of e.attempts||[]) {
   for(const [kind,value] of [['actual_child_start',a.started],['process_returned',a.ended]]) {
    const time=queueMillis(value);if(time===null)continue;
    const key=e.id+':'+a.id+':'+kind;
    records.set(key,{key,time,day:activityDate(time),type:'event',kind,entity:e.id,label:e.label});
   }
  }
 }
 return {...values,items:[...records.values()].sort((a,b)=>b.time-a.time)};
};
const loomActivitySummary=activitySummary;
activitySummary=function(entities,now=Date.now(),liveThrough=now) {
 const source=catalogData.all_entities||entities;
 const work=loomActivityJobs(source).map(e=>({...e,properties:{...e.properties,record_role:'work'},execution_intervals:workIntervals(e)}));
 // The original interval union and KST day splitting remain authoritative.
 return loomActivitySummary([...source.filter(e=>!['work','loom_job'].includes(e.properties?.record_role)),...work],now,liveThrough);
};
