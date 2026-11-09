"""Compact tool discovery with on-demand action schemas."""
GROUPS={
 'jobs':{'description':'Inspect or control Jobs. For submission schema use system.describe.', 'actions':{'list':'list_jobs','get':'get_job','submit':'submit_job','cancel':'cancel_job','logs':'get_job_logs','queue':'queue_view','reorder':'reorder_job','hold':'hold_job','resume':'resume_job','assign_owner':'assign_job_owner'}},
 'recovery':{'description':'Claim, report progress, resume, defer or reactivate a failed Job.', 'actions':{a:'recover_job' for a in ('claim','progress','resume','defer','reactivate')}},
 'events':{'description':'Read durable completion/error events and acknowledge receipt.', 'actions':{'list':'list_events','ack':'ack_event'}},
 'components':{'description':'Discover execution profiles or register a user-owned component revision.', 'actions':{'list':'component_list','get':'component_get','register':'component_register','write':'component_write','profiles':'list_execution_profiles','register_profile':'profile_register','get_profile':'profile_get','list_tools':'tool_list','get_tool':'tool_get','register_tool':'tool_register'}},
 'results':{'description':'Read artifacts/history, reindex returned results or append annotations.', 'actions':{'list':'list_registry','get':'get_entity','reindex':'reindex_job','reconnect':'reconnect_location','annotate':'annotate_job','history':'list_history','historical':'get_history','assign_owner':'assign_entity_owner'}},
 'system':{'description':'Read service/container status and compact API action schemas.', 'actions':{'ssh':'ssh_connection','users':'list_users','status':'system_status','updates':'release_status','check_updates':'check_releases','docker':'docker_status','contract':'get_job_contract','describe':'describe_action','guide':'read_guide'}}}

def tools(actor,known):
    result=[]
    for name,group in GROUPS.items():
        actions=[a for a,op in group['actions'].items() if op in ('describe_action','read_guide') or op.startswith('tool_') and ('submit' if op=='tool_register' else 'read') in actor['scopes'] or op.startswith('profile_') and ('submit' if op=='profile_register' else 'read') in actor['scopes'] or op.startswith('component_') and ('submit' if op in ('component_register','component_write') else 'read') in actor['scopes'] or op in known and known[op][0] in actor['scopes']]
        if actions:result.append({'name':name,'description':group['description'],'inputSchema':{'type':'object','properties':{'action':{'type':'string','enum':actions},'arguments':{'type':'object','description':'Use system.describe with group/action for the exact schema.'}},'required':['action'],'additionalProperties':False}})
    return result

def action_schema(group,action,known,schema_for):
    op=GROUPS.get(group,{}).get('actions',{}).get(action)
    if not op:raise ValueError('Unknown action')
    if op.startswith('tool_'):
        from tool_store import action_schema as tool_action_schema
        return tool_action_schema(action)
    if op=='read_guide':
        return {'type':'object','properties':{'id':{'type':'string'},'known_revision':{'type':'string'}},'required':[],'additionalProperties':False}
    if op=='describe_action':
        return {'type':'object','properties':{'group':{'type':'string','enum':list(GROUPS)},'action':{'type':'string'}},'required':[],
                'additionalProperties':False,'description':'Omit both fields for the full documentation index; provide group for its compact action index, or group and action for one schema.'}
    if op.startswith('profile_'):
        from profile_store import action_schema as profile_schema
        return profile_schema(action)
    if op.startswith('component_'):
        from component_store import action_schema as component_schema
        return component_schema(action)
    schema=schema_for(op)
    if group=='recovery':
        schema['properties'].pop('action',None);schema['required'].remove('action')
        if action in ('progress','resume'):schema['required'].append('lease')
    return schema
