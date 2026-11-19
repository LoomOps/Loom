"""Derive the Loom workspace from the sanitized synthetic workspace template."""
import argparse
import hashlib
import html
import os
import tempfile
from pathlib import Path
import re
from lazy_logs import patch_logs

HERE = Path(__file__).resolve().parent
SHA = "a40bec12f62ca9ec01b0fd0039e306be939ae38cca6513b74a13817f6361e426"


def replace_once(value, old, new):
    if value.count(old) != 1:
        raise ValueError(f"Expected one occurrence of {old[:100]!r}")
    return value.replace(old, new, 1)


def build(original):
    raw = original.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SHA:
        raise ValueError("Synthetic template does not match its pinned SHA-256")
    outer = raw.decode()
    match = re.search(r'data-srcdoc="([^"]*)"', outer)
    inner = html.unescape(match[1])
    inner = patch_logs(inner)
    # Heading fields are text. panel() titles may contain authored count markup,
    # so only the source-owned metric label is escaped at that call site.
    inner = inner.replace('${eyebrow}', '${esc(eyebrow)}').replace('${title}</h1>', '${esc(title)}</h1>')
    inner = replace_once(inner, 'aria-label="${title} 도움말"', 'aria-label="${esc(title)} 도움말"')
    inner = replace_once(inner, "heading('JOB',esc(e.label),", "heading('JOB',e.label,")
    inner = replace_once(inner, 'panel(m.label,contents', 'panel(esc(m.label),contents')
    inner = replace_once(inner, '${ws.panes.reduce((s,p)=>s+p.tabs.length,0)} tabs',
                         '탭 ${ws.panes.reduce((s,p)=>s+p.tabs.length,0)}개')
    inner = inner.replace('${esc(m.definition)}', '${esc(catValue(m.definition))}')
    inner = inner.replace('${esc(m.aggregation)}', '${esc(catValue(m.aggregation))}')
    inner = replace_once(inner, '<div class="pa-analysis-grid">${(e.metrics||[]).map(m=>catMetric(m,e)).join(\'\')}</div>',
                         '<div class="pa-analysis-grid" data-output-metrics>${(e.metrics||[]).map(m=>catMetric(m,e)).join(\'\')}</div>')
    # Historical numeric fields are untrusted display values, not HTML.
    for expression in ('m.numerator', 'm.denominator', "m.sample_n??'—'", "m.cohort_n??'—'", 'p.common_n', "rows[0]?.denominator??'—'"):
        inner = inner.replace('${'+expression+'}', '${esc('+expression+')}')
    inner = replace_once(inner, "r.numerator+' / '+r.denominator+'회'", "esc(r.numerator)+' / '+esc(r.denominator)+'회'")
    inner = replace_once(inner, '`${x.numerator} / ${x.denominator}', '`${esc(x.numerator)} / ${esc(x.denominator)}')
    inner = replace_once(inner,
        "${o.denominator!=null?' · '+o.numerator+' / '+o.denominator:o.count!=null?' · n='+o.count:''}",
        "${esc(o.denominator!=null?' · '+o.numerator+' / '+o.denominator:o.count!=null?' · n='+o.count:'')}")
    inner = replace_once(inner, "if(n.dataset.workGroup)return 'group:'+n.dataset.workGroup;",
        "if(n.hasAttribute('data-catalog-task-card'))return 'task-card:'+n.dataset.catalogTaskCard;if(n.hasAttribute('data-research-task'))return 'task-filter:'+n.dataset.researchTask;if(n.hasAttribute('data-policy-family'))return 'policy-family:'+n.dataset.policyFamily;if(n.dataset.workGroup)return 'group:'+n.dataset.workGroup;")
    inner = replace_once(inner, "등록된 원본 산출물 기반 · 읽기 전용", "")
    inner = inner.replace("['component','컴포넌트']", "['component','컴포넌트'],['tool','Tool']")
    inner = inner.replace("component:'컴포넌트'}", "component:'컴포넌트',tool:'Tool'}")
    inner = inner.replace("if(e.kind==='component')return catComponent(e);", "if(e.kind==='scenario')return loomScenarioDetail(e);if(e.loom_execution_profile)return loomExecutionProfileDetail(e);if(e.loom_dependency)return loomDependencyDetail(e);if(e.loom_runtime)return loomRuntimeDetail(e);if(e.loom_extension)return loomExtensionDetail(e);if(e.kind==='component')return catComponent(e);")
    inner = replace_once(inner, "['mcp','MCP','plug']", "['mcp','Loom Use','plug']")
    inner = replace_once(inner, '<div class="pa-activity-track"><div class="pa-activity-months">',
                         '<div class="pa-activity-track">${activityYears(weeks)}<div class="pa-activity-months">')
    inner = replace_once(inner, "catSyncControl();resourceInterest();", "catSyncControl();resourceInterest();scheduleActivityLayout();")
    begin = inner.index("function initActivityScroll(){")
    end = inner.index("function activityView(", begin)
    inner = inner[:begin] + (HERE / "activity.js").read_text() + "\n" + inner[end:]
    inner = replace_once(inner, "if(e.target.classList?.contains('pa-pane-body'))catalogLastScroll=performance.now();",
                         "if(e.target.matches?.('.pa-pane-body,.pa-activity-graph,.pa-activity-list'))catalogLastScroll=performance.now();")
    inner = inner.replace('lucide.createIcons(', 'loomCreateIcons(')
    inner = inner.replace('</head>', '<script>'+(HERE/'icons.js').read_text()+'</script></head>', 1)
    inner = replace_once(inner, 'const icon=n=>`<i data-lucide="${n}" aria-hidden="true"></i>`;', 'const icon=n=>n=== "tailscale"?loomTailscaleIcon():`<i data-lucide="${n}" aria-hidden="true"></i>`;')
    inner = replace_once(inner, " target.innerHTML=markup;target.open=!!opened;", " const next=target.cloneNode(false);next.innerHTML=markup;patchResourceNode(target,next,true);target.open=!!opened;")
    inner = replace_once(inner, "function patchResourceNode(current,next,keyed=false){", """function patchResourceNode(current,next,keyed=false){
 if(current.nodeType===1&&next.nodeType===1){
  if(current.localName==='svg'&&next.localName==='i'&&current.getAttribute('data-lucide')&&current.getAttribute('data-lucide')===next.getAttribute('data-lucide'))return;
  if(current.hasAttribute('data-loom-media-display')&&current.getAttribute('data-loom-media-display')===next.getAttribute('data-loom-media-display')&&current.querySelector('video,img,.pa-readable-file'))return;
  if(keyed&&current.hasAttribute('data-container-row')&&current.dataset.containerRow===next.dataset.containerRow){next=next.cloneNode(true);next.setAttribute('aria-expanded',current.getAttribute('aria-expanded'));}
  if(keyed&&current.hasAttribute('data-container-extra')&&current.dataset.containerExtra===next.dataset.containerExtra){next=next.cloneNode(true);next.hidden=current.hidden;}
 }""")
    inner = replace_once(inner, " if(n.dataset.modelFilter)return", " if(n.hasAttribute('data-container-row'))return 'container:'+n.dataset.containerRow;\n if(n.hasAttribute('data-container-extra'))return 'container-detail:'+n.dataset.containerExtra;\n if(n.dataset.modelFilter)return")

    # Decorative skins are client-owned. Telemetry/catalog reconciliation must
    # retain them, otherwise each sample removes and re-adds the card background.
    inner = replace_once(inner, " const keepOpen=keyed&&current.nodeName==='DETAILS';", """ const skin=current.querySelector(':scope > .loom-surface-skin');
 if(skin&&[...current.classList].some(c=>c!=='loom-surface'&&next.classList.contains(c))){
  next.classList.add('loom-surface');
  if(!next.querySelector(':scope > .loom-surface-skin'))next.append(skin.cloneNode(true));
 }
 const keepOpen=keyed&&current.nodeName==='DETAILS';""")

    inner = replace_once(inner, '${typeLabel[x.e.kind]}</span>', '${typeLabel[x.e.kind]}${x.e.loom_registry?\' · 등록\':\'\'}</span>')
    inner = replace_once(inner,
        "if(v.page!=='queue'||v.entity)openView({page:'queue',workLogId:id,openWorkGroups:[id]});else{v.workLogId=id;v.openWorkGroups=[...new Set([...(v.openWorkGroups||[]),id])];render();persist();}",
        "openView({page:'queue',entity:id});")
    inner = inner.replace("<b>큐 예약</b> ${reservations}", "<b>이 환경의 실행 슬롯</b> ${reservations}${l?` · 대기 ${l.queued_jobs}건 · 조회 권한 내 작업`:''}")
    inner = inner.replace("'등록된 작업의 예약 없음'", "'현재 할당 없음'")
    inner = replace_once(inner, "resCard(v.path,s.storage,", "resCard(v.path,{...s.storage,status:v.status||s.storage?.status},")
    inner = inner.replace("[{path:'data/0'},{path:'data/1'}]", "[{path:'Storage'}]")
    # Missing identity/location fields are omitted, not pending measurements.
    inner = replace_once(inner, "<p>${esc(v?.name||'계측 대기')}</p>",
                         "${v?.name?`<p>${esc(v.name)}</p>`:''}")
    inner = replace_once(inner, "<p>${esc(v.mount||'계측 대기')}${v.bulk_mount_present===false?' · 별도 마운트 미확인':''}</p>",
                         "${v.mount||v.bulk_mount_present===false?`<p>${[v.mount,v.bulk_mount_present===false?'별도 마운트 미확인':null].filter(Boolean).map(esc).join(' · ')}</p>`:''}")

    inner = inner.replace("s?.status==='stale'?'이전 값':'수집 불가'", "s?.status==='stale'?'이전 값':s?.status==='warming'?'샘플 준비 중':'수집 불가'")
    inner = replace_once(inner, '<!-- LOOM_BRAND_LOGO -->', (HERE.parent/'branding/logo.svg').read_text().strip())
    inner = replace_once(inner, "const mode=['light','dark','system'].includes(ws.theme)?ws.theme:'system';", "const mode=window.loomThemePreference||'system';")
    inner = replace_once(inner, "ws.theme=b.dataset.theme;applyTheme();persist();", "window.loomThemePreference=b.dataset.theme;ws.theme=b.dataset.theme;applyTheme();parent.postMessage({type:'loom-theme-choice',mode:b.dataset.theme},'*');persist();")
    inner, replaced = re.subn(
        r"let catalogData=catTitleSnapshot\(\{.*?\}\);\nlet catalogConnection=",
        "let catalogData=catTitleSnapshot({entities:[],sources:[],revision:null});\nlet catalogConnection=",
        inner, count=1, flags=re.DOTALL,
    )
    if replaced != 1:
        raise ValueError("Original embedded catalog seed was not found")
    inner = inner.replace("https://dashboard.example.invalid/", "")
    inner = inner.replace("실시간 연결", "최근 조회 성공").replace("스냅샷 · 서버 연결 중", "카탈로그 연결 중")
    inner = re.sub(r"connect-src [^;]+;", "connect-src 'none';", inner, count=1)
    inner = inner.replace(
        "['agent','Agent','bot']]", "['tools','Tools','wrench'],['docker','Docker','boxes'],['mcp','MCP','plug'],['access','Tailscale','tailscale']]",
    ).replace(
        "files};", "files,scenarios:loomScenarioView,profiles:loomProfilesView,components:loomComponentsView,runtimes:loomRuntimesView,dependencies:loomDependenciesView,tools:loomToolsView,docker:loomDockerView,mcp:loomMcpView,access:loomAccessView};",
    ).replace(
        "issues:'issues',agent:'agent'}", "issues:'issues',agent:'queue',profiles:'profiles',components:'components',runtimes:'runtimes',dependencies:'dependencies',tools:'tools',docker:'docker',mcp:'mcp',tailnet:'access',tailscale:'access'}",
    )
    if "['tools','Tools','wrench']" not in inner:
        inner=replace_once(inner,"['docker','Docker','boxes']","['tools','Tools','wrench'],['docker','Docker','boxes']")
    inner=replace_once(inner,"['tools','Tools','wrench']","['profiles','Runtimes',loomRuntimeIcon]")
    inner=replace_once(inner,"['models','Models','box'],['datasets','Datasets','database'],['experiments','Experiments','flask-conical'],['comparison','Comparison','git-compare-arrows']","['datasets','Catalog','archive'],['comparison','Comparison',loomComparisonIcon]")
    inner=inner.replace("icon('git-compare-arrows')", "icon(loomComparisonIcon)")
    inner=inner.replace("heading('ANALYSIS','Comparison','')+controls", "heading('ANALYSIS','Comparison','')+loomComparisonTaskFilter()+loomTaskToolbar()+controls")
    inner=replace_once(inner, "(selected.length?'__unlinked__':models[0]?.id)",
        "comparisonModel(comparisonRuns()[0]||{})?.id||(unlinked?'__unlinked__':models[0]?.id)")
    # The default selection uses available evaluations, whose unlinked flag is
    # needed before choosing a model rather than after building the controls.
    inner=replace_once(inner, " const chosen=state.compareModel", " const unlinked=comparisonRuns().some(e=>!comparisonModel(e));\n const chosen=state.compareModel")
    inner=replace_once(inner, " const unlinked=comparisonRuns().some(e=>!comparisonModel(e));\n const available=", " const available=")
    inner=replace_once(inner,"dashboard:'queue',explorer:'files',models:'models',datasets:'datasets'","dashboard:'queue',explorer:'files',outputs:'datasets',catalog:'datasets',models:'models',datasets:'datasets'")
    inner=inner.replace("'작업실 / '+viewTitle(state)","loomOutputBreadcrumb(state)")
    inner=replace_once(inner,"catalogData=catTitleSnapshot(e.data.catalog);","catalogData=loomCatalogRefresh(e.data.catalog);")
    inner=replace_once(inner,"  patchResourceNode(body,next,true);","  loomPreserveViewport(body,()=>patchResourceNode(body,next,true));")
    inner=replace_once(inner," if(n.dataset.modelFilter)return"," if(n.dataset.outputId)return 'output:'+n.dataset.outputId;\n if(n.dataset.modelFilter)return")
    # The editable list implementation lives in outputs.js.
    model_filters_start = inner.index("// Per-document filters are projections of source metadata")
    model_filters_end = inner.index("catalogRefreshViews();parent.postMessage({type:'pai-catalog-ready'},'*');", model_filters_start)
    inner = inner[:model_filters_start] + inner[model_filters_end:]

    inner=replace_once(inner,"const interested=!!root.querySelector('[data-resource-panel]');","const interested=!!root.querySelector('[data-resource-panel],.pa-finder,.pa-sidebar-bottom');")
    inner=replace_once(inner,"const catKind=(k,includeDiscarded=false)=>catalogData.entities.filter(e=>e.kind===k&&(includeDiscarded||!catIsDiscarded(e)));","let catKind=(k,includeDiscarded=false)=>loomTaskItems(catalogData.entities.filter(e=>e.kind===k&&(includeDiscarded||!catIsDiscarded(e))),k);")
    inner=replace_once(inner,"return {page:e?.kind==='model'?", "return {page:e?.kind==='scenario'?'scenarios':e?.kind==='dependency'?'dependencies':e?.kind==='execution_profile'?'profiles':e?.kind==='component'?'components':e?.kind==='runtime'?'runtimes':e?.kind==='tool'?'tools':e?.kind==='model'?")
    inner=replace_once(inner,"const catEntity=id=>catalogData.entities.find(e=>e.id===id)||(typeof workStepRecord==='function'?workStepRecord(id):null);",
        "const catEntity=id=>(catalogData.all_entities||catalogData.entities).find(e=>e.id===id)||(typeof workStepRecord==='function'?workStepRecord(id):null)||loomHistoricalExtensionEntity(id);")
    inner=replace_once(inner,"state.page===id?'active':''","(state.page===id||id==='profiles'&&loomIsHubPage(state.page)||id==='datasets'&&loomIsOutputPage(state.page))?'active':''")
    inner=replace_once(inner,"state.page===id?'page':'false'","(state.page===id||id==='profiles'&&loomIsHubPage(state.page)||id==='datasets'&&loomIsOutputPage(state.page))?'page':'false'")
    inner=replace_once(inner,"(navs.find(n=>n[0]===v.page)?.[1]||'Dashboard')","({scenarios:'Tasks',datasets:'Datasets',experiments:'Experiments',models:'Policies',comparison:'Comparison',profiles:'Profiles',components:'Components',tools:'Tools',runtimes:'Runtimes',dependencies:'Dependencies'}[v.page]||navs.find(n=>n[0]===v.page)?.[1]||'Dashboard')")
    inner = replace_once(inner, "panel(`현재 큐 · ${current.length}`,current.length?table(current,true):'<div class=\"pa-empty\">현재 등록된 Job이 없습니다.</div>')", "loomCurrentQueuePanel(current,table)")
    inner = replace_once(inner, "table(ended.slice(0,state.queueHistoryLimits?.all||10),false)",
                         "loomTerminalWorkTable(ended.slice(0,state.queueHistoryLimits?.all||10),table)")
    marker = "catalogRefreshViews();parent.postMessage({type:'pai-catalog-ready'},'*');"
    inner_additions = "\n".join((HERE / name).read_text() for name in ("mcp.js", "frame.js", "extensions.js", "dependencies.js", "scenarios.js", "details.js", "queue.js", "research.js", "job-activity.js", "text-preview.js", "media.js", "explorer.js", "surfaces.js", "search.js", "projection.js", "catalog-pages.js", "outputs.js", "research-metadata.js", "dataset-browser.js", "reporting.js", "registry-lifecycle.js"))
    inner = replace_once(inner, marker, inner_additions + "\n" + (HERE.parent/"branding/brand.js").read_text() + "\n" + marker)
    inner = inner.replace("</head>", "<style>" + (HERE / "frame.css").read_text() + (HERE / "reporting.css").read_text() + "</style></head>", 1)
    style = (HERE / "operations-desk.css").read_text() + (HERE.parent/"branding/brand.css").read_text()
    inner = replace_once(inner, "</body>", '<style id="pa-operations-desk">' + style + "</style></body>")
    outer = outer[:match.start(1)] + html.escape(inner, quote=True) + outer[match.end(1):]
    outer = replace_once(outer, "dashboard:'queue',explorer:'files',models:'models',datasets:'datasets'",
                         "dashboard:'queue',explorer:'files',outputs:'datasets',catalog:'datasets',models:'models',datasets:'datasets'")
    begin = outer.index("// Parent owns transport. Sandboxed opaque iframe gets data, never credentials.")
    end = outer.index("})();", begin) + len("})();")
    outer = outer[:begin] + (HERE.parent / "theme/theme.js").read_text() + "\n" + (HERE / "projection.js").read_text() + "\n" + (HERE / "parent.js").read_text() + outer[end:]
    # Per-browser-tab history survives reload without sharing documents across
    # tabs or persisting credentials/catalog data in localStorage.
    outer = outer.replace("[key]:{epoch:current?.epoch,index:current?.index}",
                          "[key]:{epoch:current?.epoch,index:current?.index,workspace:current?.workspace}")
    outer = replace_once(outer, " const routeURL=()=>paiRouteHash(selectedRoute(current?.workspace));", """ const savedWorkspace=history.state?.[key]?.workspace;
 if(validWorkspace(savedWorkspace)&&JSON.stringify(savedWorkspace).length<=262144&&
    (!pendingRoute||paiRouteHash(pendingRoute)===paiRouteHash(selectedRoute(savedWorkspace)))){
  current={epoch,index:0,workspace:savedWorkspace};pendingRoute=null;
 }
 const routeURL=()=>paiRouteHash(selectedRoute(current?.workspace));""")
    outer = outer.replace(
        "connect-src blob: data: https://loom-machine.example.ts.net wss://loom-machine.example.ts.net;",
        "connect-src 'self' blob: data:;",
    )
    outer = replace_once(outer, '<iframe id="codex-visualization"', '<iframe hidden id="codex-visualization"')
    outer = replace_once(outer, "</head>", "<style>" + (HERE / "parent.css").read_text() + (HERE.parent/"branding/brand.css").read_text() + "</style></head>")
    outer = replace_once(outer, "</iframe>", "</iframe>" + (HERE / "access.html").read_text().replace("<!-- LOOM_BRAND_LOGO -->", (HERE.parent/"branding/logo.svg").read_text()))
    if any(host in outer for host in ("loom-machine.example.ts.net", "dashboard.example.invalid")):
        raise ValueError("The Example Target workspace must not reference the previous service origins")
    return outer


def inputs():
    """File-set membership is an input, including newly added source assets."""
    files={p for p in HERE.iterdir() if p.is_file() and p.suffix in {'.py','.js','.css','.html'}}
    for directory in ('branding','theme'):
        files.update(p for p in (HERE.parent/directory).rglob('*') if p.is_file())
    return sorted(files)


def write_output(path, content):
    """Publish complete bytes once; preserve the previous output on failure/no-op."""
    path=Path(path);data=content.encode()
    if path.exists() and path.read_bytes()==data:return False
    path.parent.mkdir(parents=True,exist_ok=True)
    mode=path.stat().st_mode & 0o777 if path.exists() else 0o644
    fd,name=tempfile.mkstemp(prefix='.'+path.name+'-',dir=path.parent)
    try:
        with os.fdopen(fd,'wb') as stream:stream.write(data);stream.flush();os.fsync(stream.fileno())
        os.chmod(name,mode);os.replace(name,path)
    finally:
        if os.path.exists(name):os.unlink(name)
    return True


if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("original", type=Path, nargs="?", default=HERE / "template.html")
    cli.add_argument("--inputs", action="store_true", help="Print declared generator input paths")
    cli.add_argument("--check", action="store_true", help="Check the generated output without rewriting it")
    cli.add_argument("--output", type=Path, default=HERE.parent / "public" / "workspace.html")
    args = cli.parse_args()
    if args.inputs:
        import json
        print(json.dumps([str(p.relative_to(HERE.parent.parent)) for p in inputs()]))
    elif args.check:
        if not args.output.is_file() or args.output.read_bytes()!=build(args.original).encode():
            raise SystemExit("Generated dashboard is stale")
    else:
        write_output(args.output,build(args.original))
