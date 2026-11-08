"""Keep the original workspace interaction, rendering expanded Work logs only."""


def patch_logs(inner):
    replacements = [
        ('data-work-group="${esc(e.id)}" open><summary>',
         'data-work-group="${esc(e.id)}" ${state.openWorkGroups?.includes(e.id)?\'open\':\'\'}><summary>'),
        ('</summary>${workStepLog(e)}</details>',
         '</summary>${state.openWorkGroups?.includes(e.id)?workStepLog(e):\'\'}</details>'),
        ('d.open=d.dataset.workGroup?!v.closedWorkGroups?.includes(d.dataset.workGroup):!!v.openDetails?.includes(i);',
         'd.open=d.dataset.workGroup?!!v.openWorkGroups?.includes(d.dataset.workGroup):!!v.openDetails?.includes(i);'),
        ("const v=activeTab(p).view,closed=new Set(v.closedWorkGroups||[]);d.open?closed.delete(d.dataset.workGroup):closed.add(d.dataset.workGroup);v.closedWorkGroups=[...closed];",
         """if(!d.isConnected)return;
 const v=activeTab(p).view,opened=new Set(v.openWorkGroups||[]),id=d.dataset.workGroup;
 d.open?opened.add(id):opened.delete(id);v.openWorkGroups=[...opened];
 if(d.open&&!d.querySelector('.pa-execution-log')){
  const record=catEntity(id),previous=state;state=v;
  try{if(record)d.insertAdjacentHTML('beforeend',workStepLog(record));}finally{state=previous;}
 }else if(!d.open){d.querySelector('.pa-execution-log')?.remove();}
 persist();"""),
        ("openView({page:'queue',workLogId:id});else{v.workLogId=id;render();persist();}",
         "openView({page:'queue',workLogId:id,openWorkGroups:[id]});else{v.workLogId=id;v.openWorkGroups=[...new Set([...(v.openWorkGroups||[]),id])];render();persist();}"),
    ]
    for old, new in replacements:
        if inner.count(old) != 1:
            raise ValueError(f'Original log patch anchor changed: {old[:90]}')
        inner = inner.replace(old, new, 1)
    return inner
