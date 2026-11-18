"""Build/check current documentation against a compiled native registry. No host state."""
import argparse
import io
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote
import zipfile

ROOT=Path(__file__).resolve().parents[1]
GENERATED=ROOT/'docs/reference'
INSTRUCTION_SURFACES={
 'loom_use/catalog.json':('agent','guide routing metadata'),
 'loom_use/client-release.json':('client','exact published client descriptor'),
 'rust/contracts/action-docs.json':('caller','native action descriptions, examples and ownership'),
 'rust/contracts/job-contract.json':('caller','Job contract descriptions'),
 'rust/contracts/legacy-actions.json':('caller','retained input shapes composed by the native registry'),
 'rust/crates/core/src/http.rs':('agent','MCP initialization instructions'),
 'rust/crates/core/src/onboarding.rs':('agent','scoped guide/reference serving'),
 'rust/crates/reference/src/lib.rs':('agent','compiled guide/template payload'),
 'rust/crates/primitives/src/reference.rs':('agent','shared MCP entry instructions'),
 'ops/mcp_docs.py':('compatibility','retained Python reference descriptions'),
 'ops/onboarding.py':('compatibility','retained Python guide distribution'),
 'ops/ops.py':('compatibility','retained MCP initialization instructions'),
 'deploy/install-kit.py':('administrator-agent','generated installation AGENTS instruction'),
}

def markdown(root):
    # Read versioned/untracked source, never target trees or private ignored evidence.
    result=subprocess.run(['git','-C',str(root),'ls-files','-co','--exclude-standard','--','*.md'],capture_output=True,text=True)
    if result.returncode==0:return sorted(set(result.stdout.splitlines()))
    return sorted(str(p.relative_to(root)) for p in root.rglob('*.md') if not {'.git','target','evidence','__pycache__'}.intersection(p.parts))

def classification(path,root=ROOT):
    if path.endswith('THIRD_PARTY_NOTICES.md'):return ('legal','preserved','license notices')
    if path.startswith('docs/reference/'):return ('caller','generated','compiled native registry')
    if path.startswith(('docs/rust-port/','docs/item-index/')) or path=='docs/features/VERIFICATION-HISTORY.md':return ('maintainer','historical','recorded design/audit revision')
    if path.startswith('docs/'):
        try:
            with (root/path).open() as source:heading=source.read(1024)
        except OSError:heading=''
        if re.search(r'^Status: proposed\b',heading,re.M):return ('maintainer','proposed','explicit design proposal; not an installed capability')
    if path.startswith('docs/features/'):return ('maintainer','current','feature catalog and native implementation')
    if path.endswith('SKILL.md') or path=='AGENTS.md':return ('agent','current','repository/client entry instructions')
    if path.startswith(('loom_use/','rust/contracts/')):return ('agent','current','running native guide catalog and handlers')
    if path=='INSTALL.md' or path.startswith('deploy/installer/'):return ('administrator-agent','current','matching installer and actual step evidence')
    if path.startswith(('deploy/','ops/','tests/')):return ('maintainer','current','active native service and retained integration entry points')
    if path=='CHANGELOG.md':return ('reader','historical','recorded releases')
    return ('reader','current','current implementation and linked task reference')

def inventory(root=ROOT):
    docs=[]
    for path in markdown(root):
        audience,status,authority=classification(path,root)
        docs.append({'path':path,'audience':audience,'status':status,'authority':authority})
    for path,(audience,authority) in sorted(INSTRUCTION_SURFACES.items()):
        if (root/path).exists():docs.append({'path':path,'audience':audience,'status':'compatibility' if audience=='compatibility' else 'current','authority':authority})
    return {'schema':'loom-documentation/v1','documents':docs}

def heading_ids(text):
    counts={};anchors=set(re.findall(r'(?:id|name)=["\']([^"\']+)',text))
    prose=re.sub(r'(?ms)^(```|~~~).*?^\1[^\n]*\n?', '',text)
    for heading in re.findall(r'^#{1,6} (.+)$',prose,re.M):
        heading=re.sub(r'\[([^]]+)\]\([^)]*\)',r'\1',heading)
        slug=re.sub(r'[^\w\- ]','',heading.lower()).replace(' ','-')
        n=counts.get(slug,0);counts[slug]=n+1
        anchors.add(slug+(f'-{n}' if n else ''))
    return anchors

def local_link_errors(root,paths):
    errors=[]
    for path in paths:
        text=(root/path).read_text()
        prose=re.sub(r'(?ms)^(```|~~~).*?^\1[^\n]*\n?', '',text)
        for target in re.findall(r'\]\(([^\s)]+)\)',prose):
            if ':' in target or target.startswith('/') or '<' in target:continue
            ref,_,anchor=unquote(target).partition('#')
            destination=(root/path).parent.joinpath(ref) if ref else root/path
            if not destination.exists():errors.append(f'{path}: missing local link {ref}')
            elif anchor and destination.suffix=='.md' and anchor not in heading_ids(destination.read_text()):errors.append(f'{path}: missing heading {target}')
    return errors

def load_bundle(binary):
    data=subprocess.run([str(Path(binary).resolve()),'--guide-bundle'],check=True,capture_output=True,timeout=30).stdout
    with zipfile.ZipFile(io.BytesIO(data)) as z:return {name:z.read(name) for name in z.namelist()}

def reference_files(bundle):
    reference=json.loads(bundle['mcp-reference.json']);files={}
    index=['# API and MCP reference','','Generated from the compiled native action registry and action documentation. Regenerate with `python3 deploy/documentation.py build --binary target/debug/loom-ops`; do not edit generated action pages. The running installation may differ: use `system.describe` there.','','## Transport','','MCP uses stateless `POST /mcp`: initialize, tools/list and tools/call. A tool call has `params: {"name":"<group>","arguments":{"action":"<action>","arguments":{...}}}`. Read operation JSON from `result.content[0].text`, check `result.isError`, and keep `_meta.loom` separate from the visible version footer. Do not concatenate text blocks into JSON.','','HTTP uses `POST /api/actions` with `{"group":"<group>","action":"<action>","arguments":{...}}` and returns the operation JSON directly. Inspect HTTP status and the error body. Both transports use the same application registry, scopes and validation. Use the installation\'s configured credential or trusted Tailscale ingress; browser cookies and arbitrary identity headers are not credentials.','','## Discovery and authorization','','`system.status` identifies the target and principal. `system.guide` returns task routes and version markers. `system.describe({group,action})` returns one action; group-only returns the group reference and empty arguments return the full scope-filtered reference. A documented action may be absent for a caller without its scope.','','Read scope includes discovery; submit permits the documented mutation/maintenance actions; cancel and events are separate scopes. A read-scoped update check can request background release metadata, not installation. Shared Owner attribution is not access control. Exact Runtime grants, event recipients, recovery leases, admission and filesystem availability impose additional handler preconditions. Scope alone does not guarantee success.','','## Action groups','','| Group | Actions | Reference |','|---|---:|---|']
    for group in reference['groups']:
        name=group['name'];index.append(f"| `{name}` | {len(group['actions'])} | [{group['description']}]({name}.md) |")
        lines=[f'# {name}', '',group['description'],'','The examples are argument shapes, not live instructions. Replace symbolic identities and hashes with discovered exact records and satisfy the handler preconditions before a mutation. Nested schema constraints are retained below; response notes are descriptive, not a formal response schema.','','## Actions','']
        lines+=['- ['+a['action']+'](#'+a['action']+')' for a in group['actions']]
        for action in group['actions']:
            impl=action['implementation'];source,symbol=impl['entrypoint'].split('#')
            lines += ['', '## '+action['action'],'',action['description'],'',f"Scope: `{action['scope']}`. Owner: [{impl['feature']}](../features/{impl['feature']}.md). Implementation: [{source}#{symbol}](../../{source}).",'','### Inputs','','| Field | Required | Type / values | Default |','|---|---|---|---|']
            schema=action['schema']
            for field,spec in schema.get('properties',{}).items():
                typ=spec.get('enum',spec.get('type','see schema'));default=json.dumps(spec.get('default'),ensure_ascii=False) if 'default' in spec else '—'
                lines.append(f"| `{field}` | {'yes' if field in schema.get('required',[]) else 'no'} | `{json.dumps(typ,ensure_ascii=False)}` | `{default}` |")
            lines+=['','<details><summary>Exact input JSON Schema</summary>','','```json',json.dumps(schema,indent=2,ensure_ascii=False),'```','','</details>','','### Example arguments','','```json',json.dumps(action['example'],indent=2,ensure_ascii=False),'```', '', action.get('example_note',''),'','### Result and failures','',action['returns']]
            lines+=['',' '.join(action.get('errors',[])) or 'Shared authentication, schema, scope and handler preconditions still apply.']
        files[name+'.md']='\n'.join(lines)+'\n';files[name+'.json']=json.dumps(group,indent=2,ensure_ascii=False)+'\n'
    index+=['','See [concepts](../CONCEPTS.md) for object identity/state distinctions, [task guides](../../loom_use/entry.md) for workflows and [implementation boundaries](../IMPLEMENTATION.md) for maintainers. Input schema validation does not prove successful execution, byte availability or scientific semantics.']
    http=(ROOT/'rust/crates/core/src/http.rs').read_text()
    routes=re.findall(r'\("(GET|POST)", "([^"\n]+)"\) => \("([^"\n]+)", "([^"\n]+)"\)',http)
    route_lines=['# HTTP routes','','The unified `POST /api/actions` surface covers every action in the group reference. These convenience routes are extracted from the native HTTP dispatcher. GET uses query arguments; POST uses the action argument object as JSON. The convenience query parser converts offset/limit/after/revision to integers and include_consolidated to a boolean; other query values remain strings. Use /api/actions with typed JSON for arrays, objects and other boolean selectors. Identity-bearing path parameters are URL-decoded. See the exact action schema for accepted fields and scopes.','','| Method | Path | Action |','|---|---|---|']
    route_lines += [f'| `{method}` | `{path}` | [`{group}.{action}`]({group}.md#{action}) |' for method,path,group,action in routes]
    route_lines += ['', '## Special routes', '', '| Method | Path | Boundary |', '|---|---|---|',
        '| GET | `/` | Unauthenticated service discovery only |',
        '| POST | `/mcp` | JSON-RPC initialize, tools/list, tools/call and protocol notifications; operation data is in the first text block |',
        '| POST | `/api/actions` | Unified group/action/arguments dispatcher |',
        '| GET | `/api/principal` | Authenticated principal/scopes, no credential value |',
        '| GET | `/api/guide`, `/api/guide/{id}`, `/api/guide/bundle` | Current manifest/index, one guide or scope-filtered ZIP |',
        '| GET | `/api/jobs/{id}` | jobs.get, excluding contract/queue/profiles reserved paths |',
        '| GET | `/api/registry/{id}` | results.get; optional numeric revision, default 1 |',
        '| GET | `/api/items/{id}`, `/api/items/{id}/relations` | Exact Item or paged relations; IDs remain decimal strings |',
        '| POST | `/api/items/{id}/files` | Paged registered inventory; path supplies item_id |',
        '| GET / POST | `/api/files/{id}` / `/api/files/{id}/read` | Exact file metadata / bounded verified byte range |',
        '| GET | `/api/dashboard/bootstrap`, `/api/dashboard/catalog`, `/api/dashboard/revision`, `/api/dashboard/resources` | Browser projections and compatibility catalog; bootstrap plus paged actions avoids full catalog expansion |',
        '| POST | `/api/events/delivery` | Scoped delivery receipt; separate from recipient ACK and repair lease |',
        '| GET / POST | `/api/admin/tokens` | Signed Dashboard Admin claim; list metadata / issue one-time token |',
        '| POST | `/api/admin/tokens/revoke` | Signed Dashboard Admin claim; revoke by exact token ID |',
        '', 'These are Ops routes, implemented by [http.rs](../../rust/crates/core/src/http.rs) and the service transport. Dashboard is a separate authenticated relay with browser session/CSRF requirements and a restricted set of writes; a Dashboard origin is not an arbitrary Ops proxy. Public Gateway has no operational API. See [Dashboard boundaries](../../dashboard/README.md) and [access enrollment](../ACCESS-ENROLLMENT.md).',
        '', 'MCP is stateless POST. Supported negotiated protocol versions are 2024-11-05, 2025-03-26 and 2025-06-18. An Accept header allowing only text/event-stream receives one message event; this is not a background notification subscription. Notifications return HTTP 202 without an operation reply. Protocol errors and action errors are distinct. Authentication/scope, invalid-input, conflict and unavailable-source failures retain their HTTP/error envelope; a transport success is not Job completion.']
    files['http.md']='\n'.join(route_lines)+'\n'
    index += ['', 'See the [HTTP route map](http.md) for convenience and administrator integration endpoints.']
    files['README.md']='\n'.join(index)+'\n';return files

def check_reference(bundle,root=ROOT):
    import jsonschema
    errors=[];reference=json.loads(bundle['mcp-reference.json']);docs=json.loads((root/'rust/contracts/action-docs.json').read_text());actual=set()
    features={f['id'] for f in json.loads((root/'docs/features/catalog.json').read_text())['features']}
    for group in reference['groups']:
        for action in group['actions']:
            key=group['name']+'.'+action['action'];actual.add(key)
            for error in jsonschema.Draft202012Validator(action['schema']).iter_errors(action['example']):errors.append(f'{key} example: {error.message}')
            impl=action.get('implementation',{});source,_,symbol=impl.get('entrypoint','').partition('#')
            if impl.get('feature') not in features:errors.append(f'{key}: unknown feature')
            if not source or not (root/source).is_file() or not re.search(r'\bfn\s+'+re.escape(symbol)+r'\s*[<(]',(root/source).read_text()):errors.append(f'{key}: missing implementation {source}#{symbol}')
    if set(docs)!=actual:errors.append('Action description coverage differs from compiled registry: '+str(sorted(set(docs)^actual)))
    catalog=json.loads((root/'loom_use/catalog.json').read_text());names=set()
    for doc in catalog['documents']:
        if doc['id'] in names:errors.append('Duplicate guide '+doc['id'])
        names.add(doc['id'])
        if bundle.get(doc['id']+'.md')!=(root/doc['source']).read_bytes():errors.append('Stale/missing compiled guide '+doc['id'])
    return errors

def build(binary):
    bundle=load_bundle(binary);errors=check_reference(bundle)
    if errors:raise ValueError('\n'.join(errors))
    GENERATED.mkdir(parents=True,exist_ok=True)
    for path,content in reference_files(bundle).items():(GENERATED/path).write_text(content)
    (ROOT/'docs/documentation-catalog.json').write_text(json.dumps(inventory(),indent=2)+'\n')

def check(binary=None):
    errors=[];path=ROOT/'docs/documentation-catalog.json'
    if not path.exists() or json.loads(path.read_text())!=inventory():errors.append('Documentation inventory is missing/stale; run build.')
    paths=markdown(ROOT);errors+=local_link_errors(ROOT,paths)
    if binary:
        bundle=load_bundle(binary);errors+=check_reference(bundle)
        files=reference_files(bundle)
        if set(files)!={p.name for p in GENERATED.iterdir() if p.is_file()}:errors.append('Generated API reference file set differs')
        for name,content in files.items():
            if not (GENERATED/name).exists() or (GENERATED/name).read_text()!=content:errors.append('Stale generated reference '+name)
    return errors

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('operation',choices=['build','check']);p.add_argument('--binary',type=Path);a=p.parse_args()
    if a.operation=='build':
        if not a.binary:p.error('build requires --binary from this checkout')
        build(a.binary)
    errors=check(a.binary)
    if errors:print('\n'.join(errors),file=sys.stderr);return 1
    print(json.dumps({'documents':len(markdown(ROOT)),'instruction_surfaces':len(INSTRUCTION_SURFACES),'native_reference_checked':bool(a.binary),'successful':True}));return 0
if __name__=='__main__':raise SystemExit(main())
