// Include executable Jobs without counting consolidated source executions twice.
function loomActivityJobs(entities) {
 return entities.filter(e=>['work','loom_job'].includes(e.properties?.record_role)
   && !e.loom_registry && !e.properties?.consolidated_into);
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
 const source=catalogData.all_entities||entities,jobs=loomActivityJobs(source),parents=new Map(jobs.map(e=>[e.id,e.group_root!==e.id&&e.group_root||e.properties?.workflow_root||e.presentation?.parent_job]));
 for(const e of jobs)for(const n of e.loom_workflow?.executions||[])parents.set(n.job,e.id);
 const root=id=>{const seen=new Set();while(parents.get(id)&&!seen.has(id)){seen.add(id);id=parents.get(id);}return id;};
 const groups=new Map();
 for(const e of jobs){const id=root(e.id),known=groups.get(id),intervals=loomGroupWorkIntervals(e);if(known){known.execution_intervals.push(...intervals);if(['running','preparing'].includes(queueState(e).raw))known.properties.state=queueState(e).raw;}else groups.set(id,{...e,id,properties:{...e.properties,record_role:'work'},execution_intervals:[...intervals]});}
 const work=[...groups.values()];
 // The original interval union and KST day splitting remain authoritative.
 return loomActivitySummary([...source.filter(e=>!['work','loom_job'].includes(e.properties?.record_role)),...work],now,liveThrough);
};
