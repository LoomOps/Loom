"""Deterministic read-only Ops fixture. No production paths or credentials."""
from datetime import datetime,timezone
import json
import runpy
from pathlib import Path
JOB_CONTRACT=runpy.run_path(str(Path(__file__).resolve().parents[2]/"ops/job_contract.py"))["contract"]()
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer


def native(identity, imported=False):
    record={'id':identity,'state':'complete','created':1700000000,'attempt':identity+'-attempt',
        'attempts':[{'id':identity+'-attempt','state':'complete','started':1700000010,'ended':1700000012}],
        'result':{'execution':{'exit_code':0,'process_group_returned':True},'artifact_ready':True,'entities':[]}}
    entity={'id':identity,'kind':'job','label':'Fixture '+identity,'namespace':'fixture',
        'state':'complete','created':record['created'],'attempt':record['attempt'],'attempts':record['attempts'],
        'properties':{'category':'fine_tuning','execution_mode':'pilot','record_role':'loom_job','state':'complete','created_at':record['created'],'started_at':1700000010,'process_exited_at':1700000012},
        'execution_intervals':[{'start':1700000010,'end':1700000012,'count_runtime':True}]}
    if imported:
        entity.update(loom_imported_job={'source':'loom-ops','id':identity},
            loom_history={'source':'loom-ops','kind':'job_view','id':identity,'revision':'fixture-v1'})
    else:entity['loom_job']={'id':identity}
    return entity,record


def fixture():
    host={'name':'TEST FIXTURE','hostname':'fixture','ip':'127.0.0.1','ip_kind':'Test','status':'ok','os':'Fixture OS'}
    jobs=[]
    for i in range(11):
        state='paused' if i==10 else 'complete'
        jobs.append({'id':'work:fixture-'+str(i),'kind':'job','label':'Fixture Job '+str(i),'namespace':'fixture',
            'properties':{'id':'fixture-'+str(i),'record_role':'work','state':state,'created_at':1700000000+i,
                'started_at':1700000010+i,'process_exited_at':1700000020+i,'verification':{'level':'research'}},
            'steps':[],'execution_intervals':[{'start':1700000010+i,'end':1700000020+i,'count_runtime':True}]})
    live,live_record=native('fixture-live');old,old_record=native('fixture-imported',True)
    model={'id':'fixture-model','kind':'model','label':'Fixture Model','namespace':'fixture','properties':{},'metrics':[],'relations':[],'artifacts':[],'phases':[]}
    for item in (live,live_record,old,old_record,model):
        item.update(owner='fixture-user',owner_profile={'id':'fixture-user','display_name':'Fixture Owner','login':'owner@example.test'})
    catalog={'host':host,'revision':'fixture-v1','entities':jobs+[live,old,model],'sources':[{'id':'fixture','status':'ok'}]}
    now=datetime.now(timezone.utc).isoformat()
    def section(data):return {'status':'ok','source':'test-fixture','observed_at':now,'data':data}
    resources={'host':host,'observed_at':now,'sections':{
        'cpu':section({'utilization_percent':12,'logical_cpus':8,'temperature_C':40,'load_1m':1,'sample_window_s':2}),
        'memory':section({'total_bytes':16*2**30,'used_bytes':4*2**30,'available_bytes':12*2**30,'swap_used_bytes':0}),
        'gpus':section([]),'sensors':section([]),'leases':section({'registered':[],'queued_jobs':0}),
        'storage':section([{'path':'data/0','mount':'/loom/data/0','status':'ok','total_bytes':100*2**30,'used_bytes':20*2**30,'free_bytes':80*2**30}])}}
    containers=[{'name':name,'id':name+'-id','state':'running','status':'Up','compose_project':project,'compose_service':'loom-ops',
        'image':'loom-ops:fixture','image_id':'fixture-image','image_status':'available','stats':{'cpu_percent':0,'memory_percent':0}}
        for name,project in [('loom-ops','loom'),('loom-dev-build-loom-ops-1','loom-dev-build')]]
    catalog['job_contract']=JOB_CONTRACT
    return {'catalog':catalog,'resources':resources,'jobs':{live['id']:live_record,old['id']:old_record},
        'docker':{'status':'ok','observed_at':now,'host':host,'engine':{'version':'fixture'},'containers':containers,'errors':[]}}


class MockOps(BaseHTTPRequestHandler):
    def log_message(self,*_):pass
    def send_json(self,value,status=200):
        raw=json.dumps(value).encode();self.send_response(status);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(raw)));self.end_headers();self.wfile.write(raw)
    def do_GET(self):
        self.server.calls.append(('GET',self.path))
        if self.path in self.server.errors:return self.send_json({'error':'fixture error'},self.server.errors[self.path])
        f=self.server.fixture
        routes={'/api/dashboard/catalog':f['catalog'],'/api/dashboard/resources':f['resources'],
            '/api/dashboard/revision':{'revision':'fixture-v1'},'/api/docker':f['docker'],
            '/api/updates':f.get('updates',{'status':'up_to_date','installed':'0.1.0-alpha.14','latest':{'tag':'0.1.0-alpha.14','url':'https://github.com/LoomOps/Loom/releases/tag/v0.1.0-alpha.14'},'checked_at':1794096000,'automatic_install':False}),
            '/api/status':{'machine_id':'fixture','storage_id':'fixture','inventory':{}},
            '/api/principal':{'id':'fixture','scopes':['read']},'/api/registry':[],'/api/events':[]}
        if self.path.startswith('/api/jobs/'):
            row=f['jobs'].get(self.path.removeprefix('/api/jobs/'))
            return self.send_json(row or {'error':'missing'},200 if row else 404)
        return self.send_json(routes[self.path]) if self.path in routes else self.send_json({'error':'unsupported'},404)
    def do_POST(self):
        self.server.calls.append(('POST',self.path))
        body=json.loads(self.rfile.read(int(self.headers.get('Content-Length','0'))))
        if self.path=='/api/updates/check':
            return self.send_json({'accepted':True,'updates':{'status':'checking','checking':True}})
        if self.path=='/api/actions':
            self.server.actions.append(body)
            if self.path in self.server.errors:return self.send_json({'error':'fixture error'},self.server.errors[self.path])
            if body.get('group')=='components' and body.get('action') in self.server.fixture.get('runtime_actions',{}):return self.send_json(self.server.fixture['runtime_actions'][body['action']])
            entries=self.server.fixture['catalog'].get('queue',{}).get('entries',[])
            row=next((r for r in entries if r['id']==body.get('arguments',{}).get('id')),None)
            if row is None or body.get('group')!='jobs':return self.send_json({'error':'fixture deny'},403)
            action=body['action'];args=body['arguments']
            if action in ('hold','resume'):
                row['held']=action=='hold';row['can_resume']=row['held'];row['reason']='held' if row['held'] else 'ready'
            elif action=='reorder':
                entries.remove(row);index=0 if args['position']=='next' else len(entries)
                if args['position'] in ('before','after'):index=next(i for i,r in enumerate(entries) if r['id']==args['reference'])+(args['position']=='after')
                entries.insert(index,row)
                for i,r in enumerate(entries):r['position']=i+1
            else:return self.send_json({'error':'fixture deny'},400)
            return self.send_json({'id':row['id'],'changed':True,'held':row['held'],'position':row['position']})
        if self.path=='/api/history/get':
            row=self.server.fixture['jobs'].get(body.get('id'))
            return self.send_json({'body':{'record':row}}) if row else self.send_json({'error':'missing'},404)
        if self.path=='/mcp':
            if body['method']=='initialize':result={'serverInfo':{'name':'fixture-ops','version':'test'},'protocolVersion':'2025-03-26','capabilities':{'tools':{}}}
            elif body['method']=='tools/list':result={'tools':[{'name':'system_status','inputSchema':{'required':[]}}]}
            elif body['method']=='tools/call' and body.get('params')=={'name':'system','arguments':{'action':'describe','arguments':{}}}:
                actions=[{'group':'jobs','action':a,'title':a,'description':'Fixture <script> source','scope':'submit' if a=='submit' else 'read',
                    'access':'Fixture owner scope','schema':{'type':'object','properties':{'id':{'type':'string'}},'required':['id']},
                    'returns':'Job · id, status','errors':['Fixture error'],'example':{'id':'example-job'}} for a in ['get','submit']]
                docs={'revision':'fixture-v1','groups':[{'name':'jobs','description':'Job operations','actions':actions}],
                    'transport':{'result':'MCP content text JSON','errors':'result.isError'},
                    'workflow':[{'step':'상태 확인','group':'jobs','action':'get','note':'Read only'}]}
                result={'content':[{'type':'text','text':json.dumps(docs)}]}
            else:return self.send_json({'error':'read-only fixture'},405)
            return self.send_json({'jsonrpc':'2.0','id':body['id'],'result':result})
        self.send_json({'error':'read-only fixture'},405)


def make_mock():
    server=ThreadingHTTPServer(('127.0.0.1',0),MockOps)
    server.fixture=fixture();server.errors={};server.calls=[];server.actions=[]
    return server
